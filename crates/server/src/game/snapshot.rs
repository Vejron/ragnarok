//! Building each player's snapshot: what the world looks like on the wire, per entity kind, and
//! the per-player part (own state, driver motion) that goes with it.

use super::*;

impl Game {
    /// Standing players (and those flying a drone, who kneel where they stood), then the living ECS
    /// creatures (roosters). Seated players are shown by their vehicle instead.
    pub fn entity_states(&self) -> Vec<EntityState> {
        use crate::entities::{Health, NetKind, Transform, With};
        let entities = &self.world.entities;
        let creatures = entities.ids::<(With<NetKind>, With<Transform>, With<Health>)>().into_iter().filter_map(|id| {
            entities.get::<Health>(id).filter(|health| health.alive())?;
            let (NetKind(kind), transform) = (*entities.get::<NetKind>(id)?, entities.get::<Transform>(id)?);
            let [x, y, z] = transform.position;
            let mut flags = FLAG_GROUNDED;
            if crate::mechanics::calling(&self.world, id) {
                flags |= FLAG_CROWING;
            }
            Some(EntityState { id, kind, x: to_cm(x), y: to_cm(y), z: to_cm(z), yaw: to_mrad(transform.yaw), vy: 0, flags, stance: STANCE_STANDING, knock_x: 0, knock_z: 0 })
        });
        self.world
            .players
            .iter()
            .filter(|(_, player)| player.embodied())
            .map(|(&id, player)| {
                let c = &player.character;
                let mut flags = 0;
                if c.grounded {
                    flags |= FLAG_GROUNDED;
                }
                if !player.alive() {
                    flags |= FLAG_DEAD;
                }
                // Flying a drone, the body kneels over the controls.
                if player.input.buttons() & BUTTON_CROUCH != 0 || player.seat.is_some() {
                    flags |= FLAG_CROUCHING;
                }
                flags |= (player.held().item << FLAG_HELD_SHIFT) & FLAG_HELD_MASK;
                EntityState { id, kind: KIND_STICKMAN, x: to_cm(c.position[0]), y: to_cm(c.position[1]), z: to_cm(c.position[2]), yaw: to_mrad(c.yaw), vy: to_cm_i16(c.vy), flags, stance: if c.chute { STANCE_CHUTE } else if c.prone { STANCE_PRONE } else { STANCE_STANDING }, knock_x: to_cm_i16(c.knock[0]), knock_z: to_cm_i16(c.knock[1]) }
            })
            .chain(creatures)
            .collect()
    }

    pub fn vehicle_states(&self) -> Vec<VehicleState> {
        self.world
            .vehicles
            .iter()
            .filter_map(|(&id, vehicle)| {
                let pose = self.world.physics.car_pose(id)?;
                let [x, y, z] = pose.position;
                let [qx, qy, qz] = pack_quat(pose.rotation);
                let seats = vehicle.seats[..seat_count(vehicle.kind)].iter().map(|seat| seat.unwrap_or(0)).collect();
                // A hitched mortar is drawn folded up for travel.
                let flags = if self.world.physics.towed_by(id).is_some() { TURRET_STOWED } else { 0 };
                let turret = vehicle.gun.map(|gun| Turret { yaw: to_mrad(gun.yaw), pitch: to_mrad(gun.pitch), shots: gun.shots, range: (gun.range * 10.0).round().clamp(0.0, u16::MAX as f32) as u16, flags });
                let damage = (vehicle.damage != [0; 8]).then_some(vehicle.damage);
                let arms = vehicle.arms.map(|arms| {
                    let rear = vehicle.gun.unwrap_or_default();
                    let mut flags = 0;
                    if vehicle.controls.powered && vehicle.controls.lift < -0.5 {
                        flags |= ARMS_CUT;
                    }
                    if arms.jammed {
                        flags |= ARMS_JAMMED;
                    }
                    if rear.jammed {
                        flags |= ARMS_REAR_JAMMED;
                    }
                    Arms { shots: arms.shots, heat: heat_byte(arms.heat), rear_heat: heat_byte(rear.heat), bombs: arms.bombs, flags }
                });
                let thrust = match vehicle.drone {
                    Some(drone) => Some(Thrust { air: battery_byte(&drone), flags: if drone.linked { THRUST_LINKED } else { 0 } }),
                    None => self.world.physics.car_thrust(id).map(|(tank, flags)| Thrust { air: air_byte(tank), flags }),
                };
                // Where each of a crew that shows looks, and what they hold, so they can be drawn sitting.
                let crew = if shared::vehicles::vehicle_def(vehicle.kind).crew == shared::vehicles::Exposure::Sealed {
                    Vec::new()
                } else {
                    vehicle.seats[..seat_count(vehicle.kind)]
                        .iter()
                        .map(|seat| {
                            seat.and_then(|rider| self.world.players.get(&rider)).map_or(Rider::default(), |player| Rider { yaw: player.input.yaw, pitch: player.input.pitch(), held: player.held().item })
                        })
                        .collect()
                };
                Some(VehicleState { id, kind: vehicle.kind, x: to_cm(x), y: to_cm(y), z: to_cm(z), qx, qy, qz, steer: to_axis(pose.steer), seats, turret, hp: vehicle.hp, damage, arms, thrust, crew })
            })
            .collect()
    }

    /// The motion of the vehicle `id` drives, for their own prediction; `None` for anyone else.
    pub fn projectile_states(&self) -> Vec<ProjectileState> {
        // A round is not among them: clients fly it themselves from its `BulletSpawn`.
        self.world
            .in_flight()
            .into_iter()
            .filter(|(_, flying)| flying.kind != PROJECTILE_BULLET)
            .map(|(id, flying)| {
                let [x, y, z] = flying.position.map(to_cm);
                let [vx, vy, vz] = flying.velocity.map(to_cm_i16);
                ProjectileState { id, kind: flying.kind, x, y, z, vx, vy, vz }
            })
            .collect()
    }

    /// Items lying on the ground, ordered by id.
    pub fn pickup_states(&self) -> Vec<PickupState> {
        let world = &self.world;
        world
            .pickup_ids()
            .into_iter()
            .filter_map(|id| {
                let pickup = world.pickup(id).filter(|pickup| pickup.respawn_in == 0)?;
                let [x, y, z] = world.pickup_position(id)?.map(to_cm);
                Some(PickupState { id, item: pickup.item, x, y, z })
            })
            .collect()
    }

    pub(crate) fn drive_state(&self, id: u32) -> Option<DriveState> {
        let seat = self.world.players.get(&id)?.seat?;
        if seat.index != 0 {
            return None;
        }
        let motion = self.world.physics.car_motion(seat.vehicle)?;
        let [vx, vy, vz] = motion.linvel.map(to_cm_i16);
        let [wx, wy, wz] = motion.angvel.map(to_milli_i16);
        Some(DriveState { vx, vy, vz, wx, wy, wz, steer: to_milli_i16(motion.steer), spin: to_milli_i16(motion.spin) })
    }

    pub(super) fn send_snapshots(&mut self) {
        let entities = self.entity_states();
        let vehicles = self.vehicle_states();
        let rockets = self.projectile_states();
        let pickups = self.pickup_states();
        let mut shots = std::mem::take(&mut self.world.shots);
        shots.truncate(MAX_SHOT_EVENTS);
        // Each bullet goes out in a couple of snapshots, so a lost datagram does not lose it.
        let now = self.world.tick;
        self.world.bullets.retain(|b| now.wrapping_sub(b.tick) <= 2 * SNAPSHOT_EVERY);
        let bullets = self.world.bullets.clone();
        let everything = Everything { entities: &entities, vehicles: &vehicles, shots: &shots, rockets: &rockets, pickups: &pickups, bullets: &bullets };
        let ids: Vec<u32> = self.outboxes.keys().copied().collect();
        for id in ids {
            let Some(player) = self.world.players.get(&id) else { continue };
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, drive: self.drive_state(id),
                me: SelfState { hp: player.hp, progress: progress(player), slots: player.hotbar.to_vec() },
            };
            // Seated players ride with their vehicle, so the vehicle's position is theirs. A drone's
            // pilot sees what it sees. An FPV drone's range keeps their body in the picture too; a
            // loitering drone can take it out, which their client, drawing the body itself, does not mind.
            let remote = player.seat.filter(|seat| seat.remote).and_then(|seat| self.world.physics.car_pose(seat.vehicle));
            let here = remote.map_or(player.character.position, |pose| pose.position);
            let at = [here[0], here[2]];
            let (Some(outbox), Some(replica)) = (self.outboxes.get(&id), self.replicas.get_mut(&id)) else { continue };
            let snapshot = replica.snapshot(at, &everything, own);
            // A full outbox means the session is stalled; dropping a snapshot is harmless.
            let _ = outbox.try_send(Outgoing::Datagram(snapshot.into()));
        }
    }
}

/// How far the player's reload or bandage has got, 0..=255, or 0 when neither is going on.
fn progress(player: &Player) -> u8 {
    use shared::items::{BANDAGE_TICKS, def};
    let (left, total) = if player.channel_left > 0 {
        (player.channel_left, BANDAGE_TICKS)
    } else if player.reload_left > 0 {
        (player.reload_left, def(player.held().item).map_or(1, |d| d.reload_ticks.max(1)))
    } else {
        return 0;
    };
    let done = 1.0 - left as f32 / total as f32;
    ((done * 255.0).round() as u8).max(1)
}
