//! Building each player's snapshot: what the world looks like on the wire, per entity kind, and
//! the per-player part (own state, driver motion) that goes with it.

use super::*;

impl Game {
    /// Standing players, then the living roosters. Seated players are shown by their vehicle instead.
    pub fn entity_states(&self) -> Vec<EntityState> {
        let roosters = self.world.roosters.iter().filter(|(_, rooster)| rooster.alive()).map(|(&id, rooster)| {
            let [x, y, z] = rooster.position;
            let mut flags = FLAG_GROUNDED;
            if rooster.mode == RoosterMode::Crow {
                flags |= FLAG_CROWING;
            }
            EntityState(id, KIND_ROOSTER, to_cm(x), to_cm(y), to_cm(z), to_mrad(rooster.yaw), 0, flags, 0, 0)
        });
        self.world
            .players
            .iter()
            .filter(|(_, player)| player.seat.is_none())
            .map(|(&id, player)| {
                let c = &player.character;
                let mut flags = 0;
                if c.grounded {
                    flags |= FLAG_GROUNDED;
                }
                if !player.alive() {
                    flags |= FLAG_DEAD;
                }
                if player.input.buttons() & BUTTON_CROUCH != 0 {
                    flags |= FLAG_CROUCHING;
                }
                flags |= (player.held().0 << FLAG_HELD_SHIFT) & FLAG_HELD_MASK;
                EntityState(
                    id,
                    KIND_STICKMAN,
                    to_cm(c.position[0]),
                    to_cm(c.position[1]),
                    to_cm(c.position[2]),
                    to_mrad(c.yaw),
                    to_cm_i16(c.vy),
                    flags,
                    to_cm_i16(c.knock[0]),
                    to_cm_i16(c.knock[1]),
                )
            })
            .chain(roosters)
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
                let turret = vehicle.gun.map(|gun| Turret(to_mrad(gun.yaw), to_mrad(gun.pitch), gun.shots, (gun.range * 10.0).round().clamp(0.0, u16::MAX as f32) as u16));
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
                    Arms(arms.shots, heat_byte(arms.heat), heat_byte(rear.heat), arms.bombs, flags)
                });
                Some(VehicleState(id, vehicle.kind, to_cm(x), to_cm(y), to_cm(z), qx, qy, qz, to_axis(pose.steer), seats, turret, vehicle.hp, damage, arms))
            })
            .collect()
    }

    /// The motion of the vehicle `id` drives, for their own prediction; `None` for anyone else.
    pub fn projectile_states(&self) -> Vec<ProjectileState> {
        self.world
            .rockets
            .iter()
            .map(|(&id, rocket)| {
                let [x, y, z] = rocket.position.map(to_cm);
                let [vx, vy, vz] = rocket.velocity.map(to_cm_i16);
                ProjectileState(id, rocket.kind, x, y, z, vx, vy, vz)
            })
            .collect()
    }

    /// Items lying on the ground, ordered by id.
    pub fn pickup_states(&self) -> Vec<PickupState> {
        self.world
            .pickups
            .iter()
            .filter(|(_, pickup)| pickup.respawn_in == 0)
            .map(|(&id, pickup)| {
                let [x, y, z] = pickup.position.map(to_cm);
                PickupState(id, pickup.item, x, y, z)
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
        Some(DriveState(vx, vy, vz, wx, wy, wz, to_milli_i16(motion.steer), to_milli_i16(motion.spin)))
    }

    pub(super) fn send_snapshots(&mut self) {
        let entities = self.entity_states();
        let vehicles = self.vehicle_states();
        let rockets = self.projectile_states();
        let pickups = self.pickup_states();
        let mut shots = std::mem::take(&mut self.world.shots);
        shots.truncate(MAX_SHOT_EVENTS);
        let everything = Everything { entities: &entities, vehicles: &vehicles, shots: &shots, rockets: &rockets, pickups: &pickups };
        let ids: Vec<u32> = self.outboxes.keys().copied().collect();
        for id in ids {
            let Some(player) = self.world.players.get(&id) else { continue };
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, drive: self.drive_state(id),
                me: SelfState(player.hp, progress(player), player.hotbar.to_vec()),
            };
            // Seated players ride with their vehicle, so the vehicle's position is theirs.
            let at = [player.character.position[0], player.character.position[2]];
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
        (player.reload_left, def(player.held().0).map_or(1, |d| d.reload_ticks.max(1)))
    } else {
        return 0;
    };
    let done = 1.0 - left as f32 / total as f32;
    ((done * 255.0).round() as u8).max(1)
}
