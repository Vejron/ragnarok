//! The world as protocol v2 sends it: every replicated entity as the components it has
//! (`shared::replicated`), read from the ECS. Who is listed is as in v1: standing players (and those
//! flying a drone, who kneel where they stood), living creatures, vehicles with a body, projectiles
//! but gun rounds (clients fly those from their `BulletSpawn`), and items lying on the ground.
//! Seated players are shown by their vehicle's `seats` and `crew`.

// Built and checked against v1's rows from Phase 3.2; on the wire from 3.5, which drops this.
#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use crate::entities::{Health, NetKind, Transform, With};
use shared::mounts::Show;
use shared::replicated::{self as wire, Replicated, Scene};
use shared::vehicles::{Exposure, vehicle_def};

impl Game {
    /// Everything replicated, ordered by id.
    pub fn replicated(&self) -> Scene {
        let mut scene: Scene = Vec::new();
        scene.extend(self.world.entities.all::<Player>().filter(|(_, player)| player.embodied()).map(|(id, player)| (id, stickman(player))));
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
        scene.sort_unstable_by_key(|(id, _)| *id);
        scene
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
        let riders = entities.get::<Seats>(id).map_or(&[][..], |seats| seats);
        let mounts = entities.get::<Mounts>(id);
        let shown = |show| mounts.and_then(|mounts| mounts.shown(show)).copied();
        // A hitched mortar is drawn folded up for travel.
        let stowed = if self.world.physics.towed_by(id).is_some() { TURRET_STOWED } else { 0 };
        let turret = shown(Show::Turret).map(|gun| wire::Turret { yaw: to_mrad(gun.yaw), pitch: to_mrad(gun.pitch), shots: gun.shots, range: (gun.range * 10.0).round().clamp(0.0, u16::MAX as f32) as u16, flags: stowed });
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
        // Where each of a crew that shows looks, and what they hold, so they can be drawn sitting.
        let crew = (def.crew != Exposure::Sealed).then(|| wire::Crew {
            riders: riders
                .iter()
                .map(|seat| seat.and_then(|rider| entities.get::<Player>(rider)).map_or(Rider::default(), |player| Rider { yaw: player.input.yaw, pitch: player.input.pitch(), held: player.held().item }))
                .collect(),
        });
        Some(Replicated {
            kind: Some(wire::Kind { kind: vehicle.kind }),
            place: Some(wire::Place { x, y, z }),
            orientation: Some(wire::Orientation { qx, qy, qz }),
            steer: Some(wire::Steer { steer: to_axis(pose.steer) }),
            seats: Some(wire::Seats { riders: riders.iter().map(|seat| seat.unwrap_or(0)).collect() }),
            turret,
            crew,
            thrust,
            arms,
            health: Some(wire::Health { hp: entities.get::<Health>(id).map_or(0, |health| health.hp) }),
            damage: (vehicle.damage != [0; 8]).then_some(wire::Damage { parts: vehicle.damage }),
            ..Default::default()
        })
    }
}

/// A standing player (or one flying a drone, kneeling over the controls).
fn stickman(player: &Player) -> Replicated {
    let c = &player.character;
    let mut flags = 0;
    if c.grounded {
        flags |= FLAG_GROUNDED;
    }
    if !player.alive() {
        flags |= FLAG_DEAD;
    }
    if player.input.buttons() & BUTTON_CROUCH != 0 || player.seat.is_some() {
        flags |= FLAG_CROUCHING;
    }
    flags |= (player.held().item << FLAG_HELD_SHIFT) & FLAG_HELD_MASK;
    let stance = if c.chute { STANCE_CHUTE } else if c.prone { STANCE_PRONE } else { STANCE_STANDING };
    let [x, y, z] = c.position.map(to_cm);
    Replicated {
        kind: Some(wire::Kind { kind: KIND_STICKMAN }),
        place: Some(wire::Place { x, y, z }),
        heading: Some(wire::Heading { yaw: to_mrad(c.yaw) }),
        motion: Some(wire::Motion { vy: to_cm_i16(c.vy), knock_x: to_cm_i16(c.knock[0]), knock_z: to_cm_i16(c.knock[1]) }),
        flags: Some(wire::Flags { flags, stance }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
