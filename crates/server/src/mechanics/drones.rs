use super::Mechanic;
use super::combat::{Blast, blast, look};
use super::vehicles::add_vehicle;
use crate::entities::{HurtBy, With};
use crate::game::{Drone, Player, Seat, Seats, Vehicle, World};
use shared::consts::*;
use shared::protocol::{KIND_DRONE, ShotEvent};
use shared::vehicles::{DroneDef, Launch, drone_def};
use sim::half_extents;
use shared::quant::{from_mrad, to_cm};

/// FPV and loitering drones: launched from the hotbar, flown by their owner in place of their own body,
/// and gone in a blast.
///
/// Launching one (`Combat` on fire, with a drone in hand) adds a drone vehicle: a `KIND_DRONE` in front
/// of the player, or a `KIND_LOITER` high above and behind them. It puts them in its seat 0 as a *remote* seat: their input drives the drone through
/// `Vehicles` like any pilot's, while their body stays where it stood, kneeling, still to be shot.
/// It runs before `Vehicles`, so what it decides here takes effect in this tick's step:
///
/// - fire (a fresh press) sets the charge off where the drone is;
/// - use gives up: the drone is simply gone, and the pilot back in their body;
/// - the battery running out, the pilot losing any health, dying or leaving, or the drone going past
///   its range loses the link: the pilot is back in their body, and the drone falls with its
///   motors off and goes off on whatever it lands on (or after its fall time);
/// - a flown drone hitting something hard enough, or coming close to a soldier or a creature, goes off.
///
/// Each kind's numbers are its `drone_def`. Wrecking one (an FPV drone has 1 hp, a loitering drone a
/// few rounds' worth) makes `Combat::wreck` call [`pop`] instead.
pub struct Drones;

/// Launches a drone of `kind` for player `id`, and puts them at its controls. Returns whether one went
/// up: not when they are dead or seated, with no room for it where it starts, or at `MAX_VEHICLES`.
pub fn launch(world: &mut World, id: u32, kind: u8) -> bool {
    let Some(player) = world.entities.get::<Player>(id) else { return false };
    if !world.entities.alive(id) || player.seat.is_some() || world.entities.count::<Vehicle>() >= MAX_VEHICLES {
        return false;
    }
    let (input, hp) = (player.input, world.entities.get::<crate::entities::Health>(id).map_or(0, |health| health.hp));
    let yaw = from_mrad(input.yaw);
    let eye = player.eye();
    let ahead = look(yaw, 0.0);
    let def = drone_def(kind);
    let position = match def.launch {
        Launch::Behind { back, up, .. } => [eye[0] - ahead[0] * back, eye[1] + up, eye[2] - ahead[2] * back],
        Launch::Ahead { ahead: reach, up } => {
            // Up against a wall it goes up in front of the wall.
            let clearance = half_extents(kind)[0] + 0.1;
            let reach = world.physics.cast_ray(eye, ahead, reach + clearance, 0).map_or(reach, |t| (t - clearance).max(0.0));
            [eye[0] + ahead[0] * reach, eye[1] + up, eye[2] + ahead[2] * reach]
        }
    };
    if !world.physics.car_fits(kind, position, yaw) {
        return false;
    }
    let vid = add_vehicle(world, kind, position, yaw);
    if let Launch::Behind { speed, .. } = def.launch {
        // It cannot hover: it starts out already cruising.
        world.physics.set_car_velocity(vid, ahead.map(|v| v * speed));
    }
    world.entities.of_mut::<Seats>(vid)[0] = Some(id);
    world.entities.insert(vid, Drone { kind, owner: id, energy: def.energy_ticks, linked: true, pilot_hp: hp, fall_left: def.fall_ticks });
    let mut player = world.entities.get_mut::<Player>(id).expect("checked above");
    player.seat = Some(Seat { vehicle: vid, index: 0, remote: true });
    player.interrupt();
    tracing::debug!(id, vid, kind, "drone launched");
    true
}

/// Drone `vid` loses its pilot: they are back in their body, and it flies on with its motors off.
fn cut(world: &mut World, vid: u32) {
    let Some(mut drone) = world.entities.get_mut::<Drone>(vid) else { return };
    drone.linked = false;
    let pilot = world.entities.of_mut::<Seats>(vid).take(0);
    world.entities.of_mut::<Vehicle>(vid).controls = Default::default();
    if let Some(mut player) = pilot.and_then(|id| world.entities.get_mut::<Player>(id))
        && player.seat.is_some_and(|seat| seat.vehicle == vid)
    {
        player.seat = None;
    }
}

/// Takes drone `vid` out of the world, and lets its pilot go if it still has one.
fn remove(world: &mut World, vid: u32) {
    cut(world, vid);
    world.entities.despawn(vid);
    world.physics.remove_car(vid);
}

/// Sets drone `vid`'s charge off where it is: a blast of its kind's weapon, credited to its owner.
fn detonate(world: &mut World, vid: u32) {
    let Some(&drone) = world.entities.get::<Drone>(vid) else { return };
    let owner = drone.owner;
    let def = drone_def(drone.kind);
    let attacker = world.entities.has::<Player>(owner).then_some(owner);
    burst(world, vid, attacker, def.weapon, def, def.blast_radius, def.damage, def.knock);
    tracing::debug!(vid, owner, "drone went off");
}

/// Drone `vid` was shot down: it pops, credited to whoever shot it. The pop is drawn as an
/// autocannon round's burst for an FPV drone and a tank shell's for a loitering one, about their sizes.
pub(super) fn pop(world: &mut World, vid: u32) {
    let attacker = world.entities.get::<HurtBy>(vid).map(|by| by.0);
    let kind = world.entities.get::<Drone>(vid).map_or(KIND_DRONE, |drone| drone.kind);
    let def = drone_def(kind);
    burst(world, vid, attacker, def.pop_shown_as, def, def.pop_radius, def.pop_damage, def.knock * 0.3);
    tracing::debug!(vid, "drone shot down");
}

/// Removes drone `vid` and puts a blast where it was, credited as `def.weapon`. It is gone before the
/// blast, which so never finds it.
#[allow(clippy::too_many_arguments)]
fn burst(world: &mut World, vid: u32, attacker: Option<u32>, shown_as: u8, def: &DroneDef, radius: f32, damage: f32, knock: f32) {
    let Some(pose) = world.physics.car_pose(vid) else { return };
    remove(world, vid);
    let [x, y, z] = pose.position.map(to_cm);
    world.shots.push(ShotEvent { shooter: attacker.unwrap_or(0), weapon: shown_as, x, y, z });
    blast(world, Blast { point: pose.position, radius, damage, knock, dir: [0.0, 1.0, 0.0], attacker, weapon: def.weapon });
}

/// What becomes of drone `vid` this tick.
enum Fate {
    Flies,
    Cut,
    Abort,
    Detonate,
    Gone,
}

impl Mechanic for Drones {
    fn on_leave(&mut self, world: &mut World, id: u32) {
        let owned: Vec<u32> = world.entities.all::<Drone>().filter(|(_, drone)| drone.owner == id).map(|(vid, _)| vid).collect();
        for vid in owned {
            remove(world, vid);
        }
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        // What every vehicle hit in the last step.
        let impacts = world.physics.car_impacts();
        let drones = world.entities.ids::<With<Drone>>();
        for vid in drones {
            // An earlier one's blast may have taken this one down already.
            let Some(&drone) = world.entities.get::<Drone>(vid) else { continue };
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let def = drone_def(drone.kind);
            let pilot = world.entities.get::<Player>(drone.owner).filter(|player| player.seat.is_some_and(|seat| seat.vehicle == vid));
            let fate = if pose.position[1] < KILL_Y {
                Fate::Gone
            } else if let (true, Some(pilot)) = (drone.linked, pilot) {
                let buttons = pilot.input.buttons();
                let away = (pose.position[0] - pilot.character.position[0]).hypot(pose.position[2] - pilot.character.position[2]);
                if buttons & BUTTON_FIRE != 0 && !pilot.hands.trigger_held {
                    Fate::Detonate
                } else if buttons & BUTTON_USE != 0 {
                    Fate::Abort
                } else if world.entities.get::<crate::entities::Health>(drone.owner).is_none_or(|health| !health.alive() || health.hp < drone.pilot_hp) || drone.energy <= 1 || away > def.range {
                    Fate::Cut
                } else {
                    Fate::Flies
                }
            } else if drone.linked {
                // Its pilot is gone.
                Fate::Cut
            } else if drone.fall_left == 0 {
                Fate::Detonate
            } else {
                Fate::Flies
            };
            let linked = drone.linked && matches!(fate, Fate::Flies);
            let hit = impacts.iter().any(|impact| impact.car == vid && (!linked || impact.dv > def.impact_dv));
            match fate {
                Fate::Gone => remove(world, vid),
                Fate::Abort => {
                    // The press is spent here, so `Vehicles` does not take it for boarding something.
                    if let Some(mut player) = world.entities.get_mut::<Player>(drone.owner) {
                        player.input.buttons &= !BUTTON_USE;
                    }
                    remove(world, vid);
                    tracing::debug!(vid, "drone recalled");
                }
                Fate::Detonate => detonate(world, vid),
                _ if hit || super::life::hurtbox_near(world, pose.position, def.proximity, Some(drone.owner)) => detonate(world, vid),
                Fate::Cut => cut(world, vid),
                Fate::Flies => {
                    let Some(mut drone) = world.entities.get_mut::<Drone>(vid) else { continue };
                    if drone.linked {
                        drone.energy -= 1;
                    } else {
                        drone.fall_left -= 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
