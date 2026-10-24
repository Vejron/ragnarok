use super::Mechanic;
use super::bots::{HEAR_BLAST, HEAR_SHOT, make_noise};
use super::combat::{Round, hitscan, look, rewind_ticks, scatter};
use super::projectiles::{self, Flying, Stage};
use crate::entities::With;
use crate::game::{Mount, Mounts, Player, Seats, Vehicle, World};
use sim::CarPose;
use shared::consts::*;
use shared::items::Falloff;
use shared::mounts::{Aim, Ammo, Fire, MountDef, Muzzle, Need, Refill};
use shared::protocol::{DAMAGE_TURRET, InputFrame, PROJECTILE_ROCKET, ShotEvent, TURRET_GONE};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Vehicle weapons: every mount in a vehicle's `VehicleDef::mounts` (`shared::mounts`), worked the same
/// way. The occupant of the mount's seat aims it with their look and fires it with its trigger; their
/// input drives the mount instead of anything on foot, while the driver keeps the hull. What it fires
/// is a projectile (a shell, a rocket, a bomb, an egg; see `projectiles`) or a machine gun round, a
/// hitscan. Nothing here branches on the vehicle's kind: a tank's stabilised turret, a helicopter's
/// nose gun and pods, a plane's ring mount, forward guns and bombs, a walker's chin guns, a Strv S's
/// fixed gun, a mortar's tube and a hen's eggs are each a row of data.
pub struct Gunnery;

fn step_toward(current: f32, target: f32, step: f32) -> f32 {
    current + (target - current).clamp(-step, step)
}

/// Turns `yaw` towards `target` the short way round, by at most `step`.
fn follow(yaw: f32, target: f32, step: f32) -> f32 {
    wrap_angle(step_toward(yaw, yaw + wrap_angle(target - yaw), step))
}

/// Where the shell leaves the turret ring, and its unit direction, for a gun at `yaw` / `pitch`
/// (hull frame) on a hull at `pose`. The client draws the barrel and the tracer from the same maths.
pub fn aim(pose: &CarPose, yaw: f32, pitch: f32) -> ([f32; 3], [f32; 3]) {
    aim_from(pose, TANK_TURRET_PIVOT, yaw, pitch)
}

/// The same for a gun whose mount is at `pivot` in the hull frame.
fn aim_from(pose: &CarPose, pivot: [f32; 3], yaw: f32, pitch: f32) -> ([f32; 3], [f32; 3]) {
    let local = [-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()];
    (pose.world_point(pivot), pose.world_dir(local))
}

/// The elevation of the high lob that lands a bomb `range` metres out on level ground, from `MORTAR_SHELL_SPEED`
/// and `GRAVITY`: the steeper of the two angles that reach it, so the shorter the range the closer to vertical.
/// Clamped to the mortar's elevation travel.
pub fn lob_pitch(range: f32) -> f32 {
    let sine = (GRAVITY * range / (MORTAR_SHELL_SPEED * MORTAR_SHELL_SPEED)).min(1.0);
    (0.5 * (std::f32::consts::PI - sine.asin())).clamp(MORTAR_PITCH_MIN, MORTAR_PITCH_MAX)
}

/// A machine gun's barrels cooling for a tick; a jammed gun clears once they are cool enough.
fn cool(heat: &mut f32, jammed: &mut bool) {
    *heat = (*heat - GUN_COOL).max(0.0);
    if *jammed && *heat <= GUN_UNJAM {
        *jammed = false;
    }
}

/// A machine gun's barrels taking a round's worth of `heat`; at the top they jam.
fn heat_up(heat: &mut f32, jammed: &mut bool, by: f32) {
    *heat = (*heat + by).min(255.0);
    if *heat >= 255.0 {
        *jammed = true;
    }
}

/// One tick of mount `def` on vehicle `vid` at `pose`, and the mount as it is after it.
fn work(world: &mut World, vid: u32, def: &MountDef, mut mount: Mount, pose: &CarPose, dt: f32) -> Mount {
    mount.cooldown = mount.cooldown.saturating_sub(1);
    cool(&mut mount.heat, &mut mount.jammed);
    if let Some(ammo) = def.ammo {
        refill(world, &mut mount, ammo, pose);
    }
    if let Aim::Bore { .. } = def.aim {
        mount.yaw = 0.0;
    }
    let Some(occupant) = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(def.seat)) else { return mount };
    let Some(input) = world.entities.get::<Player>(occupant).map(|player| player.input) else { return mount };
    if !def.needs.iter().all(|&need| can_work(world, vid, need, pose)) {
        return mount;
    }
    turn(&mut mount, def.aim, &input, pose, dt);

    let loaded = def.ammo.is_none() || mount.ammo > 0;
    if input.buttons() & def.trigger == 0 || mount.cooldown > 0 || mount.jammed || !loaded {
        return mount;
    }
    if !def.needs.iter().all(|&need| can_fire(world, vid, need, &mount, pose)) {
        return mount;
    }
    let dir = shoot(world, vid, def, &mount, occupant, pose);
    if def.recoil != 0.0 {
        // The hull is shoved back along the barrel, level, so it does not dig in.
        world.physics.kick_car(vid, [-dir[0] * def.recoil, 0.0, -dir[2] * def.recoil]);
    }
    mount.shots = mount.shots.wrapping_add(1);
    mount.cooldown = def.cooldown;
    heat_up(&mut mount.heat, &mut mount.jammed, def.heat);
    if def.ammo.is_some() {
        mount.ammo -= 1;
    }
    mount
}

/// A mount that can run out getting more, whoever is in its seat.
fn refill(world: &World, mount: &mut Mount, ammo: Ammo, pose: &CarPose) {
    match ammo.refill {
        Refill::Parked(ticks) => {
            let speed = pose.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
            let parked = speed < 0.5 && height(world, pose) < 2.0;
            mount.refill = if parked { mount.refill + 1 } else { 0 };
            if mount.refill >= ticks {
                mount.ammo = ammo.full;
            }
        }
        Refill::Regrow(ticks) => {
            mount.refill = if mount.ammo < ammo.full { mount.refill + 1 } else { 0 };
            if mount.refill >= ticks {
                mount.ammo += 1;
                mount.refill = 0;
            }
        }
    }
}

/// How high the vehicle at `pose` is over the ground.
fn height(world: &World, pose: &CarPose) -> f32 {
    let [x, y, z] = pose.position;
    y - world.map.ground.height_at(x, z)
}

/// Whether a mount can be aimed and fired at all: without this it holds still.
fn can_work(world: &World, vid: u32, need: Need, pose: &CarPose) -> bool {
    match need {
        Need::Upright => pose.up_y >= WALKER_FALLEN_UP,
        Need::Turret => world.entities.of::<Vehicle>(vid).damage[DAMAGE_TURRET] != TURRET_GONE,
        Need::Untowed | Need::Height(_) | Need::Clear(_) => true,
    }
}

/// Whether an aimed, loaded mount can fire this tick.
fn can_fire(world: &World, vid: u32, need: Need, mount: &Mount, pose: &CarPose) -> bool {
    match need {
        Need::Untowed => world.physics.towed_by(vid).is_none(),
        Need::Height(least) => height(world, pose) > least,
        Need::Clear(arc) => mount.yaw.abs() >= arc,
        Need::Upright | Need::Turret => true,
    }
}

/// A tick of the occupant's look turning the mount.
fn turn(mount: &mut Mount, aim: Aim, input: &InputFrame, pose: &CarPose, dt: f32) {
    match aim {
        Aim::Fixed => {}
        Aim::Turret { slew, pitch: (min, max), pitch_slew } => {
            mount.yaw = follow(mount.yaw, wrap_angle(from_mrad(input.yaw) - pose.yaw()), slew * dt);
            mount.pitch = step_toward(mount.pitch, from_mrad(input.pitch()).clamp(min, max), pitch_slew * dt);
        }
        Aim::Bore { pitch: (min, max), slew } => {
            mount.pitch = step_toward(mount.pitch, from_mrad(input.pitch()).clamp(min, max), slew * dt);
        }
        Aim::Mount { yaw_limit, pitch: (min, max), slew } => {
            // The look is taken in the world and seen from the hull.
            let local = pose.local_dir(look(from_mrad(input.yaw), from_mrad(input.pitch())));
            let yaw = (-local[0]).atan2(-local[2]);
            let pitch = local[1].clamp(-1.0, 1.0).asin().clamp(min, max);
            mount.yaw = match yaw_limit {
                Some(limit) => step_toward(mount.yaw, yaw.clamp(-limit, limit), slew * dt),
                None => follow(mount.yaw, yaw, slew * dt),
            };
            mount.pitch = step_toward(mount.pitch, pitch, slew * dt);
        }
        Aim::Lob { slew } => {
            mount.yaw = follow(mount.yaw, wrap_angle(from_mrad(input.yaw) - pose.yaw()), slew * dt);
            // A gun that has never been dialled starts at the middle of the range, not at zero.
            let range = if mount.range == 0.0 { MORTAR_RANGE_START } else { mount.range };
            let buttons = input.buttons();
            let dial = f32::from(buttons & BUTTON_RANGE_UP != 0) - f32::from(buttons & BUTTON_RANGE_DOWN != 0);
            mount.range = (range + dial * MORTAR_RANGE_RATE * dt).clamp(MORTAR_RANGE_MIN, MORTAR_RANGE_MAX);
            mount.pitch = lob_pitch(mount.range);
        }
    }
}

/// Fires a round of `def` for `occupant`, and returns the barrel's heading (before any spread).
fn shoot(world: &mut World, vid: u32, def: &MountDef, mount: &Mount, occupant: u32, pose: &CarPose) -> [f32; 3] {
    let (origin, dir) = match def.muzzle {
        Muzzle::One(pivot) => aim_from(pose, pivot, mount.yaw, mount.pitch),
        Muzzle::Turns(pivots) => aim_from(pose, pivots[usize::from(mount.shots) % pivots.len()], mount.yaw, mount.pitch),
        Muzzle::Twin { pivot, x, drop } => {
            let (_, dir) = aim_from(pose, pivot, mount.yaw, mount.pitch);
            // One barrel either side, taking turns: the offset turns with the mount (yaw) and the
            // barrels (pitch).
            let side = if mount.shots.is_multiple_of(2) { -1.0 } else { 1.0 };
            let (sp, cp) = mount.pitch.sin_cos();
            let (sy, cy) = mount.yaw.sin_cos();
            let (bx, by) = (side * x, -drop);
            let (ly, lz) = (by * cp, by * sp);
            let offset = [bx * cy + lz * sy, ly, -bx * sy + lz * cy];
            (pose.world_point(std::array::from_fn(|i| pivot[i] + offset[i])), dir)
        }
    };
    match def.fire {
        Fire::Shell { kind, speed, length, ttl } => {
            let aimed = scatter(world, dir, def.spread);
            fire(world, kind, vid, origin, aimed, length, speed, ttl, occupant);
        }
        Fire::Rocket { speed, ttl } => {
            // It flies where the barrel points, not where the vehicle is drifting: only the vehicle's
            // speed along it is carried over, so a sideways slide does not bend the rocket's heading.
            let along: f32 = (0..3).map(|i| pose.velocity[i] * dir[i]).sum::<f32>().max(0.0);
            world.launch(Flying { kind: PROJECTILE_ROCKET, owner: occupant, source: vid, position: origin, velocity: dir.map(|d| d * (speed + along)), ttl, item: 0, back: 0, flown: 0.0 });
        }
        Fire::Hitscan { length, damage, vehicle, plane, weapon, range } => {
            let muzzle = std::array::from_fn(|i| origin[i] + dir[i] * length);
            let aimed = scatter(world, dir, def.spread);
            let round = Round { damage, vehicle, plane, weapon, range, falloff: Falloff::NONE };
            machine_gun(world, occupant, vid, muzzle, aimed, &round);
        }
        Fire::Drop { kind, toss, ttl } => {
            let toss = pose.world_dir(toss);
            let velocity = std::array::from_fn(|i| pose.velocity[i] + toss[i]);
            world.launch(Flying { kind, owner: occupant, source: vid, position: origin, velocity, ttl, item: 0, back: 0, flown: 0.0 });
        }
    }
    dir
}

/// Fires a shell of `kind` from vehicle `vid` for `gunner`, from `origin` along `dir` (the muzzle's
/// root and the barrel's heading, `length` of barrel ahead) at `speed`: it leaves the muzzle and flies
/// on its own from there, so it takes time to arrive and falls on the way.
#[allow(clippy::too_many_arguments)]
fn fire(world: &mut World, kind: u8, vid: u32, origin: [f32; 3], dir: [f32; 3], length: f32, speed: f32, ttl: u32, gunner: u32) {
    make_noise(world, origin, Some(gunner), HEAR_BLAST);
    // A barrel poking into a wall bursts the shell there, instead of starting it on the far side.
    if let Some(t) = world.physics.cast_ray(origin, dir, length, vid) {
        projectiles::burst_at(world, kind, 0, gunner, std::array::from_fn(|i| origin[i] + dir[i] * t), dir);
        return;
    }
    world.launch(Flying { kind, owner: gunner, source: vid, position: std::array::from_fn(|i| origin[i] + dir[i] * length), velocity: dir.map(|v| v * speed), ttl, item: 0, back: 0, flown: 0.0 });
}

/// One machine gun `round` for `shooter` from vehicle `vid`, out of `muzzle` along `dir`. Every round
/// is drawn by the client from the gun's counter; only one that hit something is announced, for the spark.
fn machine_gun(world: &mut World, shooter: u32, vid: u32, muzzle: [f32; 3], dir: [f32; 3], round: &Round) {
    let back = world.entities.get::<Player>(shooter).map_or(0, |player| rewind_ticks(world.tick, player.input.view()));
    make_noise(world, muzzle, Some(shooter), HEAR_SHOT);
    let (end, struck) = hitscan(world, shooter, muzzle, dir, round, vid, back);
    if struck {
        let [x, y, z] = std::array::from_fn(|i| to_cm(muzzle[i] + dir[i] * end));
        world.shots.push(ShotEvent { shooter, weapon: round.weapon, x, y, z });
    }
}

impl Mechanic for Gunnery {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        projectiles::step(world, dt, Stage::Guns);
        for vid in world.entities.ids::<With<Mounts>>() {
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let defs = world.entities.of::<Mounts>(vid).defs;
            for (index, def) in defs.iter().enumerate() {
                let Some(mount) = world.entities.get::<Mounts>(vid).map(|mounts| mounts.state[index]) else { break };
                let mount = work(world, vid, def, mount, &pose, dt);
                if let Some(mut mounts) = world.entities.get_mut::<Mounts>(vid) {
                    mounts.state[index] = mount;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
