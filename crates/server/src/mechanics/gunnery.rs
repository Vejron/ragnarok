use shared::vec3;
use super::Mechanic;
use shared::armor::Part;
use super::bots::{HEAR_BLAST, HEAR_SHOT, make_noise, make_shot};
use super::combat::{MAX_PITCH, Round, beam, look, rewind_ticks, scatter};
use super::life::first_hurtbox_hit;
use super::projectiles::{self, Flying, Stage};
use crate::entities::With;
use crate::game::{Bot, Mount, Mounts, Player, Seats, Vehicle, World};
use sim::CarPose;
use shared::consts::*;
use shared::items::Falloff;
use shared::mounts::{Aim, Ammo, Eye, Fire, MountDef, Muzzle, Need, Refill};
use shared::protocol::{DAMAGE_TURRET, InputFrame, PROJECTILE_BULLET, PROJECTILE_ROCKET, ShotEvent, TURRET_GONE};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Vehicle weapons: every mount in a vehicle's `VehicleDef::mounts` (`shared::mounts`), worked the same
/// way. The occupant of the mount's seat aims it with their look and fires it with its trigger; their
/// input drives the mount instead of anything on foot, while the driver keeps the hull. What it fires
/// is a projectile (a shell, a rocket, a bomb, an egg; see `projectiles`) or a machine gun round, a
/// hitscan. Nothing here branches on the vehicle's kind: a tank's stabilised turret, a helicopter's
/// nose gun and pods, a plane's ring mount, forward guns and bombs, a walker's chin guns, a Strv S's
/// fixed gun, a mortar's tube, a hen's eggs and a tupp's eyes are each a row of data.
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

/// Where point `at` of a turret at `yaw` on a ring at `ring` is, in the hull frame.
fn on_turret(ring: [f32; 3], [x, y, z]: [f32; 3], yaw: f32) -> [f32; 3] {
    let (sin, cos) = yaw.sin_cos();
    [ring[0] + x * cos + z * sin, ring[1] + y, ring[2] - x * sin + z * cos]
}

/// The elevation of the high lob that lands a bomb `range` metres out on level ground, from `MORTAR_SHELL_SPEED`
/// and `GRAVITY`: the steeper of the two angles that reach it, so the shorter the range the closer to vertical.
/// Clamped to the mortar's elevation travel.
pub fn lob_pitch(range: f32) -> f32 {
    let sine = (GRAVITY * range / (MORTAR_SHELL_SPEED * MORTAR_SHELL_SPEED)).min(1.0);
    (0.5 * (std::f32::consts::PI - sine.asin())).clamp(MORTAR_PITCH_MIN, MORTAR_PITCH_MAX)
}

/// How far a bomb lobbed at elevation `pitch` (radians) lands on level ground, metres: the inverse of
/// [`lob_pitch`], so a tube laid by its elevation shows the range dial that would have laid it.
pub fn lob_range(pitch: f32) -> f32 {
    MORTAR_SHELL_SPEED * MORTAR_SHELL_SPEED * (2.0 * pitch).sin() / GRAVITY
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
    // A beam only goes on from here, so one that loses its driver or falls over dies with the tick.
    let (charged, burning) = (mount.charge, mount.beam);
    (mount.charge, mount.beam) = (0, 0);
    let Some(occupant) = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(def.seat)) else { return mount };
    let Some(input) = world.entities.get::<Player>(occupant).map(|player| player.input) else { return mount };
    if !def.needs.iter().all(|&need| can_work(world, vid, need, pose)) {
        return mount;
    }
    let turret_yaw = world.entities.get::<Mounts>(vid).and_then(|mounts| mounts.turret().map(|turret| turret.yaw)).unwrap_or(0.0);
    let mut reach = mount.reach;
    let want = wanted(world, vid, def, occupant, &input, pose.world_point(pivot(def.muzzle, &mount, turret_yaw)), &mut reach, dt);
    mount.reach = reach;
    turn(&mut mount, def.aim, &input, want, pose, turret_yaw, dt);
    if let Fire::Beam { charge, burn, recover, .. } = def.fire {
        if burning > 0 {
            mount.beam = burning - 1;
            if mount.cooldown == 0 {
                shoot(world, vid, def, &mount, occupant, pose, turret_yaw);
                mount.shots = mount.shots.wrapping_add(1);
                mount.cooldown = def.cooldown;
            }
            if mount.beam == 0 {
                mount.cooldown = recover;
            }
        } else if input.buttons() & def.trigger != 0 && mount.cooldown == 0 {
            if charged + 1 >= charge {
                mount.beam = burn;
            } else {
                mount.charge = charged + 1;
            }
        }
        return mount;
    }
    // Loading the other kind of round takes as long as loading the next one.
    let swap = input.buttons() & BUTTON_AMMO != 0;
    if let Fire::Shell { kind, alt, .. } = def.fire
        && alt != kind
        && swap
        && !mount.swapping
    {
        mount.alt = !mount.alt;
        mount.cooldown = def.cooldown;
    }
    mount.swapping = swap;

    let loaded = def.ammo.is_none() || mount.ammo > 0;
    if input.buttons() & def.trigger == 0 || mount.cooldown > 0 || mount.jammed || !loaded {
        return mount;
    }
    if !def.needs.iter().all(|&need| can_fire(world, vid, need, &mount, pose)) {
        return mount;
    }
    let dir = shoot(world, vid, def, &mount, occupant, pose, turret_yaw);
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
            let speed = vec3::length(pose.velocity);
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
        Need::Untowed | Need::Breech | Need::Height(_) | Need::Clear(_) => true,
    }
}

/// Whether an aimed, loaded mount can fire this tick.
fn can_fire(world: &World, vid: u32, need: Need, mount: &Mount, pose: &CarPose) -> bool {
    match need {
        Need::Untowed => world.physics.towed_by(vid).is_none(),
        Need::Breech => !super::armor::part_gone(world, vid, Part::Breech),
        Need::Height(least) => height(world, pose) > least,
        Need::Clear(arc) => mount.yaw.abs() >= arc,
        Need::Upright | Need::Turret => true,
    }
}

/// Where a mount at `muzzle` turns about, in the hull frame, for a turret at `turret_yaw`: the point its
/// rounds leave from before the barrel's length, and the one its reticle camera sits behind.
pub(super) fn pivot(muzzle: Muzzle, mount: &Mount, turret_yaw: f32) -> [f32; 3] {
    match muzzle {
        Muzzle::One(pivot) | Muzzle::Twin { pivot, .. } => pivot,
        Muzzle::Turns(pivots) => pivots[usize::from(mount.shots) % pivots.len()],
        Muzzle::Pintle { ring, at } => on_turret(ring, at, turret_yaw),
    }
}

/// The way, in the world, that the occupant of mount `def` on vehicle `vid` wants it to point, for a
/// gun turning about `pivot` (world), `dt` after the last time. A mount aimed with the reticle
/// (`MountDef::eye`) points at what the camera's ray meets, as a third-person shooter's gun does: the
/// camera sits behind and above the gun, so a gun only parallel to the look would put its round below
/// the reticle up close and above it far off. Looking down a gun sight the camera is the sight's eye,
/// `SIGHT_EYE_UP` over the pivot. The ray is followed from abreast of the pivot, as nothing behind the gun
/// can be what it is aimed at; it stops at the ground, a wall, a vehicle (not this one) or a soldier as
/// the occupant saw them, or at the end of its reach. How far along it the gun is laid is `reach`, eased
/// to that ([`MOUNT_AIM_EASE`]). A shell's barrel is raised for the fall as far as `MOUNT_ZERO_RANGE`
/// ([`lay`]); further off it falls short, for the gunner to make up. With no reticle, a bot (which has
/// no camera) or a point that would turn the gun sideways, it is the look itself.
#[allow(clippy::too_many_arguments)]
pub(super) fn wanted(world: &World, vid: u32, def: &MountDef, occupant: u32, input: &InputFrame, pivot: [f32; 3], reach: &mut f32, dt: f32) -> [f32; 3] {
    // Past straight up the look would turn round behind.
    let look_dir = look(from_mrad(input.yaw), from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH));
    // A bot has no camera: its look is the way it wants the gun, laid for the fall already.
    let Some(Eye { back, up }) = def.eye.filter(|_| !world.entities.has::<Bot>(occupant)) else { return look_dir };
    let (back, up) = if def.sight && input.buttons() & BUTTON_ALT != 0 { (0.0, SIGHT_EYE_UP) } else { (back, up) };
    let camera = eye_at(world, pivot, look_dir, back, up);
    let far = match def.fire {
        Fire::Bullet { range, .. } | Fire::Beam { range, .. } => range,
        Fire::Shell { .. } | Fire::Rocket { .. } | Fire::Drop { .. } => MOUNT_AIM_RANGE,
    };
    let start = vec3::along(camera, look_dir, back);
    let rewind = rewind_ticks(world.tick, input.view());
    let wall = world.physics.cast_ray(start, look_dir, far, vid).unwrap_or(far);
    let on = first_hurtbox_hit(world, start, look_dir, wall, Some(occupant), rewind, None).map_or(wall, |(_, t)| t).max(0.5);
    *reach = if *reach <= 0.0 { on } else { (reach.ln() + (on.ln() - reach.ln()) * (1.0 - (-dt / MOUNT_AIM_EASE).exp())).exp() };
    let to = vec3::sub(vec3::along(start, look_dir, *reach), pivot);
    let len = vec3::length(to);
    if len < 1e-3 || vec3::dot(to, look_dir) < 0.5 * len {
        return look_dir;
    }
    let straight = to.map(|v| v / len);
    match def.fire {
        // Laid as for a mark as far off as the zero, at most, on the same line.
        Fire::Shell { speed, .. } => lay(straight.map(|v| v * len.min(MOUNT_ZERO_RANGE)), speed, GRAVITY).unwrap_or(straight),
        Fire::Bullet { .. } => lay(straight.map(|v| v * len.min(MOUNT_ZERO_RANGE)), BULLET_SPEED, GRAVITY * BULLET_GRAVITY_SCALE).unwrap_or(straight),
        _ => straight,
    }
}

/// The unit direction a round leaving at `speed` and falling under `gravity` takes to pass through `to`
/// (from where it leaves): the flatter of the two arcs that reach it. `None` when it is out of reach.
pub fn lay(to: [f32; 3], speed: f32, gravity: f32) -> Option<[f32; 3]> {
    let across = to[0].hypot(to[2]);
    if across < 1e-3 {
        return None;
    }
    let (v2, g) = (speed * speed, gravity);
    let root = v2 * v2 - g * (g * across * across + 2.0 * to[1] * v2);
    if root < 0.0 {
        return None;
    }
    let elevation = ((v2 - root.sqrt()) / (g * across)).atan();
    let (sin, cos) = elevation.sin_cos();
    Some([to[0] / across * cos, sin, to[2] / across * cos])
}

/// Where the reticle camera behind a gun at `pivot` is, for a look along `look_dir`: `back` behind on
/// the look's line and `up` above that, and never in the ground, as the client places it
/// (`Scene.aimFrom`).
pub fn eye_at(world: &World, pivot: [f32; 3], look_dir: [f32; 3], back: f32, up: f32) -> [f32; 3] {
    let [x, y, z] = vec3::along(pivot, look_dir, -back);
    [x, (y + up).max(world.map.ground.height_at(x, z) + MOUNT_EYE_CLEARANCE), z]
}

/// The yaw (+ left) and pitch (+ up) of a unit direction `[x, y, z]` given in the hull frame.
pub(super) fn angles([x, y, z]: [f32; 3]) -> (f32, f32) {
    ((-x).atan2(-z), y.clamp(-1.0, 1.0).asin())
}

/// A tick of the occupant turning the mount towards `want` (world), on a vehicle whose turret is at
/// `turret_yaw`. The way is seen from the hull, so a gun keeps its aim while the hull leans and turns.
#[allow(clippy::too_many_arguments)]
fn turn(mount: &mut Mount, aim: Aim, input: &InputFrame, want: [f32; 3], pose: &CarPose, turret_yaw: f32, dt: f32) {
    let (yaw, pitch) = angles(pose.local_dir(want));
    match aim {
        Aim::Fixed => {}
        Aim::Turret { slew, pitch: (min, max), pitch_slew } => {
            mount.yaw = follow(mount.yaw, yaw, slew * dt);
            mount.pitch = step_toward(mount.pitch, pitch.clamp(min, max), pitch_slew * dt);
        }
        Aim::Bore { pitch: (min, max), slew } => {
            mount.pitch = step_toward(mount.pitch, pitch.clamp(min, max), slew * dt);
        }
        Aim::Mount { yaw_limit, pitch: (min, max), slew } => {
            mount.yaw = match yaw_limit {
                Some(limit) => step_toward(mount.yaw, yaw.clamp(-limit, limit), slew * dt),
                None => follow(mount.yaw, yaw, slew * dt),
            };
            mount.pitch = step_toward(mount.pitch, pitch.clamp(min, max), slew * dt);
        }
        Aim::Pintle { yaw_limit, pitch: (min, max), slew } => {
            // The look seen from the turret: when the turret swings away the end of the arc pushes the gun along.
            let yaw = wrap_angle(yaw - turret_yaw).clamp(-yaw_limit, yaw_limit);
            mount.yaw = step_toward(mount.yaw, yaw, slew * dt).clamp(-yaw_limit, yaw_limit);
            mount.pitch = step_toward(mount.pitch, pitch.clamp(min, max), slew * dt);
        }
        Aim::Lob { slew, pitch_slew } => {
            mount.yaw = follow(mount.yaw, wrap_angle(from_mrad(input.yaw) - pose.yaw()), slew * dt);
            let buttons = input.buttons();
            // A gun never laid has its tube put straight to its first lob rather than raised from flat.
            let fresh = mount.range == 0.0;
            if buttons & BUTTON_ALT != 0 {
                // Laid from above (aim held): the look's pitch carries the elevation the gunner wants, worked
                // out on their side for the height of the ground at the mark. The dial follows it, so letting
                // go of aim keeps the lay.
                let pitch = from_mrad(input.pitch()).clamp(lob_pitch(MORTAR_RANGE_MAX), lob_pitch(MORTAR_RANGE_MIN));
                mount.range = lob_range(pitch).clamp(MORTAR_RANGE_MIN, MORTAR_RANGE_MAX);
            } else {
                // A gun that has never been dialled starts at the middle of the range, not at zero.
                let range = if mount.range == 0.0 { MORTAR_RANGE_START } else { mount.range };
                let dial = f32::from(buttons & BUTTON_RANGE_UP != 0) - f32::from(buttons & BUTTON_RANGE_DOWN != 0);
                mount.range = (range + dial * MORTAR_RANGE_RATE * dt).clamp(MORTAR_RANGE_MIN, MORTAR_RANGE_MAX);
            }
            // The tube takes its time to the lob the range calls for, as a turret's barrel does.
            let wanted = if buttons & BUTTON_ALT != 0 { from_mrad(input.pitch()).clamp(lob_pitch(MORTAR_RANGE_MAX), lob_pitch(MORTAR_RANGE_MIN)) } else { lob_pitch(mount.range) };
            mount.pitch = if fresh { wanted } else { step_toward(mount.pitch, wanted, pitch_slew * dt) };
        }
    }
}

/// Fires a round of `def` for `occupant`, and returns the barrel's heading (before any spread).
fn shoot(world: &mut World, vid: u32, def: &MountDef, mount: &Mount, occupant: u32, pose: &CarPose, turret_yaw: f32) -> [f32; 3] {
    let (origin, dir) = match def.muzzle {
        Muzzle::One(_) | Muzzle::Turns(_) => aim_from(pose, pivot(def.muzzle, mount, turret_yaw), mount.yaw, mount.pitch),
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
        Muzzle::Pintle { .. } => aim_from(pose, pivot(def.muzzle, mount, turret_yaw), wrap_angle(turret_yaw + mount.yaw), mount.pitch),
    };
    match def.fire {
        Fire::Shell { kind, alt, speed, length, ttl } => {
            let kind = if mount.alt { alt } else { kind };
            let aimed = scatter(world, dir, def.spread);
            fire(world, kind, vid, origin, aimed, length, speed, ttl, occupant);
        }
        Fire::Rocket { speed, ttl } => {
            // It flies where the barrel points, not where the vehicle is drifting: only the vehicle's
            // speed along it is carried over, so a sideways slide does not bend the rocket's heading.
            let along: f32 = vec3::dot(pose.velocity, dir).max(0.0);
            world.launch(Flying::new(PROJECTILE_ROCKET, occupant, origin, dir.map(|d| d * (speed + along)), ttl).aboard(vid));
        }
        Fire::Bullet { length, damage, vehicle, plane, weapon, range, pen } => {
            let muzzle = vec3::along(origin, dir, length);
            let aimed = scatter(world, dir, def.spread);
            let round = Round { damage, vehicle, plane, weapon, range, falloff: Falloff::NONE, pen };
            machine_gun(world, occupant, vid, muzzle, aimed, round);
        }
        Fire::Beam { length, radius, damage, vehicle, plane, weapon, range, pen, .. } => {
            let muzzle = vec3::along(origin, dir, length);
            let round = Round { damage, vehicle, plane, weapon, range, falloff: Falloff::NONE, pen };
            let back = world.entities.get::<Player>(occupant).map_or(0, |player| rewind_ticks(world.tick, player.input.view()));
            make_noise(world, muzzle, Some(occupant), HEAR_SHOT);
            // Always announced, hit or miss: the beam is drawn to where it ended.
            let (end, _) = beam(world, occupant, muzzle, dir, &round, vid, back, radius);
            let [x, y, z] = end.map(to_cm);
            world.shots.push(ShotEvent { shooter: vid, weapon, x, y, z });
        }
        Fire::Drop { kind, toss, ttl } => {
            let toss = pose.world_dir(toss);
            let velocity = std::array::from_fn(|i| pose.velocity[i] + toss[i]);
            world.launch(Flying::new(kind, occupant, origin, velocity, ttl).aboard(vid));
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
        projectiles::burst_at(world, kind, 0, gunner, vec3::along(origin, dir, t), dir);
        return;
    }
    world.launch(Flying::new(kind, gunner, vec3::along(origin, dir, length), dir.map(|v| v * speed), ttl).aboard(vid));
}

/// One machine gun `round` for `shooter` from vehicle `vid`, out of `muzzle` along `dir`: a bullet that
/// flies and falls as a hand gun's, at soldiers as the shooter saw them. Every round is drawn by the
/// client from the gun's counter; only one that hits something is announced, for the spark.
fn machine_gun(world: &mut World, shooter: u32, vid: u32, muzzle: [f32; 3], dir: [f32; 3], round: Round) {
    let back = world.entities.get::<Player>(shooter).map_or(0, |player| rewind_ticks(world.tick, player.input.view()));
    make_shot(world, muzzle, Some(shooter), dir);
    let ttl = (round.range / BULLET_SPEED * TICK_RATE as f32).ceil() as u32;
    world.launch(Flying::new(PROJECTILE_BULLET, shooter, muzzle, dir.map(|v| v * BULLET_SPEED), ttl).aboard(vid).rewound(back).hitting(round));
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
