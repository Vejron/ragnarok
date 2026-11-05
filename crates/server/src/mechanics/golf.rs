//! Golf, on a world with a course (`WorldFile::golf`, from `golf.json`). A ball waits on every tee; a
//! player with a club (`ITEM_DRIVER`, `ITEM_PUTTER`) swings on the meter (`shared::golf::Meter`, stepped
//! by `Combat`'s item use): fire held for the power, let go, and pressed again on the line, early to hook
//! and late to slice. The strike hits the ball at their feet (`swing`), which flies, curves and rolls as
//! a projectile (`projectiles::Rolls`, `Spin`), and drops into a cup it rolls slowly over: everyone is
//! told who holed it, and in how many strokes from the hole's tee.

use super::Mechanic;
use super::combat::look;
use super::projectiles::{Flying, Projectile, Rolls, Spin, Velocity};
use crate::entities::{Component, Transform, With};
use crate::game::{Player, World};
use shared::consts::*;
use shared::golf::Strike;
use shared::items::club;
use shared::protocol::{PROJECTILE_GOLF_BALL, ShotEvent, WEAPON_CUP, WEAPON_GOLF};
use shared::quant::{from_mrad, to_cm};

pub struct Golf;

/// A golf ball's card: the hole whose tee it was set on (`None` once it has been holed elsewhere or
/// struck by another) and the strokes it has taken since.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub hole: Option<u8>,
    pub strokes: u8,
}

/// A tee with no ball within this (metres) gets one.
const TEE_CLEAR: f32 = 4.0;
/// How often the tees are seen to, ticks.
const TEE_EVERY: u32 = 2 * TICK_RATE;
/// Most a golfer's feet may be above or below a ball they strike, metres.
const REACH_Y: f32 = 1.2;
/// How far round from where the golfer faces (radians) a ball can be and still be struck. They stand
/// side-on to where they aim, the ball by their right foot, so anywhere but behind them.
const REACH_ANGLE: f32 = 2.2;
/// Where an empty swing's head passes: this far out to the golfer's right, at their feet.
const SWING_OUT: f32 = 0.6;
/// A ball this far (metres) above the ground at the cup is flying over it, not dropping in.
const CUP_ABOVE: f32 = 0.3;
/// How far an overswung ball goes off line by chance, radians at most either way.
const SPRAY: f32 = 0.04;
/// How far a curved drive starts off line the way it curves, radians for a full curve.
const START_LINE: f32 = 0.01;
/// A mishit is topped: it leaves this share of the club's loft.
const TOPPED: f32 = 0.3;
/// A ball laid on a tee lasts as long as nobody takes it; once struck, `GOLF_BALL_TTL_TICKS`.
const ON_TEE_TTL: u32 = u32::MAX / 2;

pub fn register(ecs: &mut bevy_ecs::world::World) {
    ecs.register_component::<Card>();
}

/// Every golf ball, with where it is.
fn balls(world: &World) -> Vec<(u32, [f32; 3])> {
    world.entities.ids::<With<Rolls>>().into_iter().filter_map(|id| Some((id, world.entities.get::<Transform>(id)?.position))).collect()
}

/// The ground, or what stands on it, at `[x, z]`.
fn floor(world: &World, [x, z]: [f32; 2]) -> f32 {
    let ground = world.map.ground.height_at(x, z);
    let top = ground + 40.0;
    world.physics.cast_ray([x, top, z], [0.0, -1.0, 0.0], 80.0, 0).map_or(ground, |t| top - t)
}

/// Lays a ball on the tee of every hole that has none on it.
fn lay_balls(world: &mut World) {
    let Some(course) = world.map.golf.clone() else { return };
    let mut on = balls(world);
    for hole in &course.holes {
        if on.len() >= MAX_GOLF_BALLS {
            break;
        }
        let [x, z] = hole.tee;
        if !world.map.is_inside(to_cm(x), to_cm(z)) || on.iter().any(|(_, at)| (at[0] - x).hypot(at[2] - z) < TEE_CLEAR) {
            continue;
        }
        let at = [x, floor(world, hole.tee) + GOLF_BALL_RADIUS, z];
        let id = world.launch(Flying::new(PROJECTILE_GOLF_BALL, 0, at, [0.0; 3], ON_TEE_TTL));
        world.entities.insert(id, Card { hole: Some(hole.number), strokes: 0 });
        on.push((id, at));
    }
}

/// What a score is called against the hole's par.
fn called(strokes: u8, par: u8) -> &'static str {
    match i16::from(strokes) - i16::from(par) {
        _ if strokes == 1 => "a hole in one!",
        ..=-3 => "an albatross!",
        -2 => "an eagle!",
        -1 => "a birdie",
        0 => "par",
        1 => "a bogey",
        2 => "a double bogey",
        _ => "",
    }
}

/// Drops into its cup each ball rolling slowly over one, and tells everyone.
fn hole_out(world: &mut World) {
    let Some(course) = world.map.golf.as_ref() else { return };
    let dropped: Vec<(u32, [f32; 3], shared::golf::Hole)> = balls(world)
        .into_iter()
        .filter_map(|(id, at)| {
            let hole = course.holes.iter().find(|hole| (at[0] - hole.cup[0]).hypot(at[2] - hole.cup[1]) < GOLF_CUP_RADIUS)?;
            let speed = world.entities.get::<Velocity>(id).map_or(0.0, |v| shared::vec3::length(v.0));
            (speed <= GOLF_CUP_SPEED && at[1] - floor(world, hole.cup) <= CUP_ABOVE).then(|| (id, at, hole.clone()))
        })
        .collect();
    for (id, at, hole) in dropped {
        let owner = world.entities.get::<Projectile>(id).map_or(0, |p| p.owner);
        let card = world.entities.get::<Card>(id).copied().unwrap_or(Card { hole: None, strokes: 0 });
        world.entities.despawn(id);
        let [x, y, z] = [hole.cup[0], at[1], hole.cup[1]].map(to_cm);
        world.shots.push(ShotEvent { shooter: owner, weapon: WEAPON_CUP, x, y, z });
        let Some(name) = world.entities.get::<Player>(owner).map(|player| player.name.clone()) else { continue };
        let text = if card.hole == Some(hole.number) {
            let strokes = if card.strokes == 1 { "1 stroke".to_string() } else { format!("{} strokes", card.strokes) };
            match hole.par.map(|par| (par, called(card.strokes, par))) {
                Some((par, "")) => format!("{name} holed out on {} in {strokes} (par {par})", hole.number),
                Some((par, call)) => format!("{name} holed out on {} in {strokes} (par {par}): {call}", hole.number),
                None => format!("{name} holed out on {} in {strokes}", hole.number),
            }
        } else {
            format!("{name} sank a ball on hole {}", hole.number)
        };
        tracing::info!(%text, "golf");
        world.fresh.said.push(text);
    }
}

/// Player `id` swings the club `item` and strikes as the meter says (`strike`), with `high` for a high
/// pitch: the nearest ball at their feet, not behind them, is sent the way they face.
pub fn swing(world: &mut World, id: u32, item: u8, strike: Strike, high: bool) {
    let Some(club) = club(item) else { return };
    let Some(player) = world.entities.get::<Player>(id) else { return };
    let (at, yaw) = (player.character.position, from_mrad(player.input.yaw));
    let feet = at[1] - STAND_CENTRE;
    let ahead = look(yaw, 0.0);
    let struck = balls(world)
        .into_iter()
        .filter(|(_, ball)| (ball[1] - feet).abs() < REACH_Y)
        .map(|(ball, position)| {
            let (dx, dz) = (position[0] - at[0], position[2] - at[2]);
            let distance = dx.hypot(dz);
            let angle = if distance < 1e-3 { 0.0 } else { ((dx * ahead[0] + dz * ahead[2]) / distance).clamp(-1.0, 1.0).acos() };
            (ball, position, distance, angle)
        })
        .filter(|(.., distance, angle)| *distance < GOLF_REACH && *angle < REACH_ANGLE)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let right = look(yaw - std::f32::consts::FRAC_PI_2, 0.0);
    let head = struck.map_or([at[0] + right[0] * SWING_OUT, feet + GOLF_BALL_RADIUS, at[2] + right[2] * SWING_OUT], |(_, position, ..)| position);
    let [x, y, z] = head.map(to_cm);
    world.shots.push(ShotEvent { shooter: id, weapon: WEAPON_GOLF, x, y, z });
    let Some((ball, ..)) = struck else { return };
    // A positive curve goes right, which is a smaller yaw.
    let spray = if strike.power > 1.0 { (world.rng.next_f64() as f32 * 2.0 - 1.0) * SPRAY } else { 0.0 };
    let rolled = club.loft == 0.0;
    let off = spray - strike.curve * if rolled { GOLF_PUTT_PULL } else { START_LINE };
    let loft = if high { club.high_loft } else { club.loft } * if strike.mishit { TOPPED } else { 1.0 };
    let velocity = look(yaw + off, loft).map(|v| v * club.speed * strike.power);
    // Just clear of the ground, so the first step flies rather than meets it.
    let lift = if loft > 0.0 { 0.02 } else { 0.0 };
    if let Some(mut transform) = world.entities.get_mut::<Transform>(ball) {
        transform.position[1] += lift;
    }
    if let Some(mut v) = world.entities.get_mut::<Velocity>(ball) {
        v.0 = velocity;
    }
    // A ball in the air bends the way it was cut; a putt has no air to bend in.
    if rolled || strike.curve == 0.0 {
        world.entities.remove::<Spin>(ball);
    } else {
        world.entities.insert(ball, Spin(strike.curve * GOLF_SPIN));
    }
    let mut owner_changed = false;
    if let Some(mut projectile) = world.entities.get_mut::<Projectile>(ball) {
        owner_changed = projectile.owner != id;
        projectile.owner = id;
        projectile.ttl = GOLF_BALL_TTL_TICKS;
    }
    let mut card = world.entities.get::<Card>(ball).copied().unwrap_or(Card { hole: None, strokes: 0 });
    // Another's ball is no longer on its round: who hits it now starts their own count, of no hole.
    if owner_changed && card.strokes > 0 {
        card = Card { hole: None, strokes: 0 };
    }
    card.strokes = card.strokes.saturating_add(1);
    world.entities.insert(ball, card);
}

impl Mechanic for Golf {
    fn on_start(&mut self, world: &mut World) {
        lay_balls(world);
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        if world.map.golf.is_none() {
            return;
        }
        hole_out(world);
        if world.tick.is_multiple_of(TEE_EVERY) {
            lay_balls(world);
        }
    }
}

#[cfg(test)]
mod tests;
