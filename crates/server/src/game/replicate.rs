//! The world as protocol v2 sends it: every replicated entity as the components it has
//! (`shared::replicated`), read from the ECS. Who is listed is as in v1: standing players (and those
//! flying a drone, who kneel where they stood), living creatures, vehicles with a body, projectiles
//! but gun rounds (clients fly those from their `BulletSpawn`), and items lying on the ground.
//! A seated player is a soldier with `seated` and no `place`, sent wherever their vehicle is
//! (`Replica::snapshot`); one whose crew shows also says where they look and what they hold. A drone's
//! pilot stands where they were as well, with their seat.
//!
//! Every player also carries their own `vitals` and `hotbar`, and a driven vehicle its `drive`: the
//! `[owner]` components, which `Replica::snapshot` sends to the owner alone (`Game::owned`).

use super::*;
use crate::entities::{Health, NetKind, Transform, With};
use crate::mechanics::{Struck, Wounds};
use shared::mounts::Show;
use shared::replicated::{self as wire, Replicated, Scene};
use shared::vehicles::{Exposure, vehicle_def};

impl Game {
    /// Everything replicated, ordered by id.
    pub fn replicated(&self) -> Scene {
        let mut scene: Scene = Vec::new();
        scene.extend(self.world.entities.all::<Player>().map(|(id, player)| {
            let shown = if player.embodied() { soldier(player, self.world.entities.alive(id)) } else { self.rider(player) };
            let seated = player.seat.map(|seat| wire::Seated { vehicle: seat.vehicle, seat: seat.index as u8 });
            let one_arm = crate::mechanics::limbs_lost(&self.world, id) & LIMB_ARMS != 0;
            let wounds = self.world.entities.get::<Wounds>(id).copied();
            let limbs = wounds.filter(|wounds| wounds.lost != 0).map(|wounds| wire::Limbs { lost: wounds.wire() });
            let struck = self.world.entities.get::<Struck>(id).map(|s| wire::Struck { count: s.count, part: s.part, amount: s.amount, yaw: s.yaw, x: s.at[0], y: s.at[1], z: s.at[2] });
            let parts = wounds.unwrap_or_default().left();
            let carry = crate::mechanics::carries(&self.world, id).then_some(wire::Carry { what: CARRY_BACKPACK });
            let shown = Replicated { seated, limbs, struck, carry, ..shown };
            let hp = self.world.entities.get::<Health>(id).map_or(0, |health| health.hp);
            (id, Replicated { vitals: Some(wire::Vitals { hp, progress: progress(player, one_arm), spent: player.character.spent, winded: player.character.winded.into(), parts, down: player.character.down }), hotbar: Some(wire::Hotbar { slots: player.hands.hotbar }), ..shown })
        }));
        scene.extend(self.creatures());
        scene.extend(self.world.entities.all::<Vehicle>().filter_map(|(id, vehicle)| Some((id, self.vehicle(id, vehicle)?))));
        scene.extend(self.world.in_flight().into_iter().filter(|(_, flying)| flying.kind != PROJECTILE_BULLET).map(|(id, flying)| {
            let [x, y, z] = flying.position.map(to_cm);
            let [vx, vy, vz] = flying.velocity.map(to_cm_i16);
            (id, Replicated { projectile: Some(wire::Projectile { kind: flying.kind }), place: Some(wire::Place { x, y, z }), velocity: Some(wire::Velocity { vx, vy, vz }), ..Default::default() })
        }));
        scene.extend(self.world.pickup_ids().into_iter().filter_map(|id| {
            let pickup = self.world.pickup(id).filter(|pickup| pickup.respawn_in == 0)?;
            let [x, y, z] = self.world.pickup_position(id)?.map(to_cm);
            Some((id, Replicated { pickup: Some(wire::Pickup { item: pickup.item }), place: Some(wire::Place { x, y, z }), ..Default::default() }))
        }));
        scene.extend(crate::mechanics::backpack_lying(&self.world).into_iter().map(|(id, position)| {
            let [x, y, z] = position.map(to_cm);
            (id, Replicated { kind: Some(wire::Kind { kind: KIND_BACKPACK }), place: Some(wire::Place { x, y, z }), ..Default::default() })
        }));
        scene.sort_unstable_by_key(|(id, _)| *id);
        scene
    }

    /// A player sitting in a vehicle, but for their seat: where they look and what they hold when their
    /// crew shows, and only that they are a soldier when it does not.
    fn rider(&self, player: &Player) -> Replicated {
        let kind = Some(wire::Kind { kind: KIND_SOLDIER });
        let vehicle = player.seat.and_then(|seat| self.world.entities.get::<Vehicle>(seat.vehicle));
        if vehicle.is_none_or(|vehicle| vehicle_def(vehicle.kind).crew == Exposure::Sealed) {
            return Replicated { kind, ..Default::default() };
        }
        let flags = FLAG_CROUCHING | ((player.held().item << FLAG_HELD_SHIFT) & FLAG_HELD_MASK);
        Replicated {
            kind,
            heading: Some(wire::Heading { yaw: player.input.yaw }),
            look: Some(wire::Look { pitch: player.input.pitch() }),
            flags: Some(wire::Flags { flags, stance: STANCE_STANDING }),
            ..Default::default()
        }
    }

    /// The living ECS creatures (roosters): standing where they are, calling or not.
    fn creatures(&self) -> Vec<(u32, Replicated)> {
        let entities = &self.world.entities;
        entities
            .ids::<(With<NetKind>, With<Transform>, With<Health>)>()
            .into_iter()
            .filter(|&id| !entities.dead(id))
            .filter_map(|id| {
                let (NetKind(kind), transform) = (*entities.get::<NetKind>(id)?, entities.get::<Transform>(id)?);
                let [x, y, z] = transform.position.map(to_cm);
                let calling = if crate::mechanics::calling(&self.world, id) { FLAG_CROWING } else { 0 };
                Some((
                    id,
                    Replicated {
                        kind: Some(wire::Kind { kind }),
                        place: Some(wire::Place { x, y, z }),
                        heading: Some(wire::Heading { yaw: to_mrad(transform.yaw) }),
                        flags: Some(wire::Flags { flags: FLAG_GROUNDED | calling, stance: STANCE_STANDING }),
                        ..Default::default()
                    },
                ))
            })
            .collect()
    }

    /// Vehicle `id` as it is drawn: its pose and what it carries. `None` while it has no body.
    fn vehicle(&self, id: u32, vehicle: &Vehicle) -> Option<Replicated> {
        let entities = &self.world.entities;
        let pose = self.world.physics.car_pose(id)?;
        let [x, y, z] = pose.position.map(to_cm);
        let [qx, qy, qz] = pack_quat(pose.rotation);
        let def = vehicle_def(vehicle.kind);
        let driver = entities.get::<Seats>(id).and_then(|seats| seats.rider(0));
        let mounts = entities.get::<Mounts>(id);
        let shown = |show| mounts.and_then(|mounts| mounts.shown(show)).copied();
        // A hitched mortar is drawn folded up for travel.
        let stowed = if self.world.physics.towed_by(id).is_some() { TURRET_STOWED } else { 0 };
        let turret = shown(Show::Turret).map(|gun| {
            let flags = stowed | if gun.alt { TURRET_ALT } else { 0 };
            wire::Turret { yaw: to_mrad(gun.yaw), pitch: to_mrad(gun.pitch), shots: gun.shots, range: (gun.range * 10.0).round().clamp(0.0, u16::MAX as f32) as u16, flags }
        });
        let arms = def.arms().then(|| {
            let (guns, rack, rear) = (shown(Show::Guns).unwrap_or_default(), shown(Show::Rack).unwrap_or_default(), shown(Show::Turret).unwrap_or_default());
            let mut flags = 0;
            if vehicle.controls.powered && vehicle.controls.lift < -0.5 {
                flags |= ARMS_CUT;
            }
            if guns.jammed {
                flags |= ARMS_JAMMED;
            }
            if rear.jammed {
                flags |= ARMS_REAR_JAMMED;
            }
            // A rack counts its rounds as the guns' when there are no guns: a hen's eggs.
            let shots = if shown(Show::Guns).is_some() { guns.shots } else { rack.shots };
            wire::Arms { shots, heat: heat_byte(guns.heat), rear_heat: heat_byte(rear.heat), bombs: rack.ammo, flags }
        });
        let thrust = match entities.get::<Drone>(id) {
            Some(drone) => Some(wire::Thrust { air: battery_byte(drone), flags: if drone.linked { THRUST_LINKED } else { 0 } }),
            None => self.world.physics.car_thrust(id).map(|(tank, flags)| wire::Thrust { air: air_byte(tank), flags }),
        };
        Some(Replicated {
            kind: Some(wire::Kind { kind: vehicle.kind }),
            place: Some(wire::Place { x, y, z }),
            orientation: Some(wire::Orientation { qx, qy, qz }),
            steer: Some(wire::Steer { steer: to_axis(pose.steer) }),
            turret,
            thrust,
            arms,
            health: Some(wire::Health { hp: entities.get::<Health>(id).map_or(0, |health| health.hp) }),
            damage: (vehicle.damage != [0; shared::protocol::DAMAGE_SLOTS]).then_some(wire::Damage { parts: vehicle.damage }),
            drive: driver.filter(|&driver| entities.has::<Player>(driver)).and_then(|_| self.drive(id)),
            ..Default::default()
        })
    }

    /// The exact motion of vehicle `id`, for its driver's own prediction.
    fn drive(&self, id: u32) -> Option<wire::Drive> {
        let motion = self.world.physics.car_motion(id)?;
        let [vx, vy, vz] = motion.linvel.map(to_cm_i16);
        let [wx, wy, wz] = motion.angvel.map(to_milli_i16);
        Some(wire::Drive { vx, vy, vz, wx, wy, wz, steer: to_milli_i16(motion.steer), spin: to_milli_i16(motion.spin) })
    }

    /// The entities player `id` owns: themself, and the vehicle they drive.
    pub(crate) fn owned(&self, id: u32) -> [Option<u32>; 2] {
        let driving = self.world.entities.get::<Player>(id).and_then(|player| player.seat).filter(|seat| seat.index == 0).map(|seat| seat.vehicle);
        [Some(id), driving]
    }
}

/// A standing player (or one flying a drone, kneeling over the controls).
fn soldier(player: &Player, alive: bool) -> Replicated {
    let c = &player.character;
    let mut flags = 0;
    if c.grounded {
        flags |= FLAG_GROUNDED;
    }
    if !alive {
        flags |= FLAG_DEAD;
    }
    if player.input.buttons() & BUTTON_CROUCH != 0 || player.seat.is_some() {
        flags |= FLAG_CROUCHING;
    }
    flags |= (player.held().item << FLAG_HELD_SHIFT) & FLAG_HELD_MASK;
    let stance = if c.down > 0 { STANCE_DOWN } else if c.chute { STANCE_CHUTE } else if c.prone { STANCE_PRONE } else { STANCE_STANDING };
    let [x, y, z] = c.position.map(to_cm);
    Replicated {
        kind: Some(wire::Kind { kind: KIND_SOLDIER }),
        place: Some(wire::Place { x, y, z }),
        heading: Some(wire::Heading { yaw: to_mrad(c.yaw) }),
        motion: Some(wire::Motion { vy: to_cm_i16(c.vy), knock_x: to_cm_i16(c.knock[0]), knock_z: to_cm_i16(c.knock[1]) }),
        flags: Some(wire::Flags { flags, stance }),
        ..Default::default()
    }
}

/// How far the player's reload or bandage has got, 0..=255, or 0 when neither is going on. A reload in
/// `one_arm` takes `ONE_ARM_RELOAD` times as long.
fn progress(player: &Player, one_arm: bool) -> u8 {
    use shared::items::{BANDAGE_TICKS, def};
    let (left, total) = if player.hands.channel_left > 0 {
        (player.hands.channel_left, BANDAGE_TICKS)
    } else if player.hands.reload_left > 0 {
        let slow = if one_arm { ONE_ARM_RELOAD } else { 1 };
        (player.hands.reload_left, def(player.held().item).map_or(1, |d| d.reload_ticks.max(1)) * slow)
    } else {
        return 0;
    };
    let done = 1.0 - left as f32 / total as f32;
    ((done * 255.0).round() as u8).max(1)
}

#[cfg(test)]
mod tests;
