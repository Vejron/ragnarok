use super::Mechanic;
use super::combat::{Blast, blast, look};
use super::vehicles::add_vehicle;
use crate::game::{Drone, Seat, World};
use shared::consts::*;
use shared::items::*;
use shared::protocol::{KIND_DRONE, ShotEvent, WEAPON_AUTOCANNON, WEAPON_DRONE};
use shared::quant::{from_mrad, to_cm};

/// FPV drones: launched from the hotbar, flown by their owner in place of their own body, and gone
/// in a blast.
///
/// Launching one (`Combat` on fire, with a drone in hand) adds a `KIND_DRONE` vehicle in front of the
/// player and puts them in its seat 0 as a *remote* seat: their input drives the drone through
/// `Vehicles` like any pilot's, while their body stays where it stood, kneeling, still to be shot.
/// It runs before `Vehicles`, so what it decides here takes effect in this tick's step:
///
/// - fire (a fresh press) sets the charge off where the drone is;
/// - use gives up: the drone is simply gone, and the pilot back in their body;
/// - the battery running out, the pilot losing any health, dying or leaving, or the drone going past
///   `DRONE_RANGE` loses the link: the pilot is back in their body, and the drone falls with its
///   motors off and goes off on whatever it lands on (or after `DRONE_FALL_TICKS`);
/// - a flown drone hitting something hard enough, or coming close to a stickman, goes off.
///
/// One hit of anything wrecks it (it has 1 hp); `Combat::wreck` then calls [`pop`] instead.
pub struct Drones;

/// Launches a drone for player `id`, and puts them at its controls. Returns whether one went up:
/// not when they are dead or seated, with no room for it in front of them, or at `MAX_VEHICLES`.
pub fn launch(world: &mut World, id: u32) -> bool {
    let Some(player) = world.players.get(&id) else { return false };
    if !player.alive() || player.seat.is_some() || world.vehicles.len() >= MAX_VEHICLES {
        return false;
    }
    let (input, c, hp) = (player.input, player.character, player.hp);
    let yaw = from_mrad(input.yaw);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
    let ahead = look(yaw, 0.0);
    // Up against a wall it goes up in front of the wall.
    let clearance = DRONE_HALF_EXTENTS[0] + 0.1;
    let reach = world.physics.cast_ray(eye, ahead, DRONE_LAUNCH_AHEAD + clearance, 0).map_or(DRONE_LAUNCH_AHEAD, |t| (t - clearance).max(0.0));
    let position = [eye[0] + ahead[0] * reach, eye[1] + DRONE_LAUNCH_UP, eye[2] + ahead[2] * reach];
    if !world.physics.car_fits(KIND_DRONE, position, yaw) {
        return false;
    }
    let vid = add_vehicle(world, KIND_DRONE, position, yaw);
    let vehicle = world.vehicles.get_mut(&vid).expect("just added");
    vehicle.seats[0] = Some(id);
    vehicle.drone = Some(Drone { owner: id, energy: DRONE_ENERGY_TICKS, linked: true, pilot_hp: hp, fall_left: DRONE_FALL_TICKS });
    let player = world.players.get_mut(&id).expect("checked above");
    player.seat = Some(Seat { vehicle: vid, index: 0, remote: true });
    player.interrupt();
    tracing::debug!(id, vid, "drone launched");
    true
}

/// Drone `vid` loses its pilot: they are back in their body, and it flies on with its motors off.
fn cut(world: &mut World, vid: u32) {
    let Some(vehicle) = world.vehicles.get_mut(&vid) else { return };
    let Some(drone) = &mut vehicle.drone else { return };
    drone.linked = false;
    let pilot = vehicle.seats[0].take();
    vehicle.controls = Default::default();
    if let Some(player) = pilot.and_then(|id| world.players.get_mut(&id))
        && player.seat.is_some_and(|seat| seat.vehicle == vid)
    {
        player.seat = None;
    }
}

/// Takes drone `vid` out of the world, and lets its pilot go if it still has one.
fn remove(world: &mut World, vid: u32) {
    cut(world, vid);
    world.vehicles.remove(&vid);
    world.physics.remove_car(vid);
}

/// Sets drone `vid`'s charge off where it is: a `WEAPON_DRONE` blast credited to its owner.
fn detonate(world: &mut World, vid: u32) {
    let Some(owner) = world.vehicles.get(&vid).and_then(|vehicle| vehicle.drone).map(|drone| drone.owner) else { return };
    let attacker = world.players.contains_key(&owner).then_some(owner);
    burst(world, vid, attacker, WEAPON_DRONE, DRONE_BLAST_RADIUS, DRONE_DAMAGE, DRONE_KNOCK);
    tracing::debug!(vid, owner, "drone went off");
}

/// Drone `vid` was shot down: it pops, credited to whoever shot it. The pop is drawn as an
/// autocannon round's burst, which is about its size.
pub(super) fn pop(world: &mut World, vid: u32) {
    let attacker = world.vehicles.get(&vid).and_then(|vehicle| vehicle.last_attacker);
    burst(world, vid, attacker, WEAPON_AUTOCANNON, DRONE_POP_RADIUS, DRONE_POP_DAMAGE, DRONE_KNOCK * 0.3);
    tracing::debug!(vid, "drone shot down");
}

/// Removes drone `vid` and puts a blast where it was. It is gone before the blast, which so never finds it.
fn burst(world: &mut World, vid: u32, attacker: Option<u32>, shown_as: u8, radius: f32, damage: f32, knock: f32) {
    let Some(pose) = world.physics.car_pose(vid) else { return };
    remove(world, vid);
    let [x, y, z] = pose.position.map(to_cm);
    world.shots.push(ShotEvent { shooter: attacker.unwrap_or(0), weapon: shown_as, x, y, z });
    blast(world, Blast { point: pose.position, radius, damage, knock, dir: [0.0, 1.0, 0.0], attacker, weapon: WEAPON_DRONE });
}

/// Whether `point` is within `DRONE_PROXIMITY` of the axis of a living stickman other than `owner`.
fn near_someone(world: &World, point: [f32; 3], owner: u32) -> bool {
    world.players.iter().filter(|&(&id, player)| id != owner && player.embodied() && player.alive()).any(|(_, player)| {
        let c = player.character.position;
        let dy = (point[1] - c[1]).clamp(-CAPSULE_HALF_HEIGHT, CAPSULE_HALF_HEIGHT);
        let d = [point[0] - c[0], point[1] - c[1] - dy, point[2] - c[2]];
        d.iter().map(|v| v * v).sum::<f32>() < DRONE_PROXIMITY * DRONE_PROXIMITY
    })
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
        let owned: Vec<u32> = world.vehicles.iter().filter(|(_, vehicle)| vehicle.drone.is_some_and(|drone| drone.owner == id)).map(|(&vid, _)| vid).collect();
        for vid in owned {
            remove(world, vid);
        }
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        // What every vehicle hit in the last step.
        let impacts = world.physics.car_impacts();
        let drones: Vec<u32> = world.vehicles.iter().filter(|(_, vehicle)| vehicle.drone.is_some()).map(|(&vid, _)| vid).collect();
        for vid in drones {
            // An earlier one's blast may have taken this one down already.
            let Some(drone) = world.vehicles.get(&vid).and_then(|vehicle| vehicle.drone) else { continue };
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let pilot = world.players.get(&drone.owner).filter(|player| player.seat.is_some_and(|seat| seat.vehicle == vid));
            let fate = if pose.position[1] < KILL_Y {
                Fate::Gone
            } else if let (true, Some(pilot)) = (drone.linked, pilot) {
                let buttons = pilot.input.buttons();
                let away = (pose.position[0] - pilot.character.position[0]).hypot(pose.position[2] - pilot.character.position[2]);
                if buttons & BUTTON_FIRE != 0 && !pilot.trigger_held {
                    Fate::Detonate
                } else if buttons & BUTTON_USE != 0 {
                    Fate::Abort
                } else if !pilot.alive() || pilot.hp < drone.pilot_hp || drone.energy <= 1 || away > DRONE_RANGE {
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
            let hit = impacts.iter().any(|impact| impact.car == vid && (!linked || impact.dv > DRONE_IMPACT_DV));
            match fate {
                Fate::Gone => remove(world, vid),
                Fate::Abort => {
                    // The press is spent here, so `Vehicles` does not take it for boarding something.
                    if let Some(player) = world.players.get_mut(&drone.owner) {
                        player.input.buttons &= !BUTTON_USE;
                    }
                    remove(world, vid);
                    tracing::debug!(vid, "drone recalled");
                }
                Fate::Detonate => detonate(world, vid),
                _ if hit || near_someone(world, pose.position, drone.owner) => detonate(world, vid),
                Fate::Cut => cut(world, vid),
                Fate::Flies => {
                    let Some(drone) = world.vehicles.get_mut(&vid).and_then(|vehicle| vehicle.drone.as_mut()) else { continue };
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
