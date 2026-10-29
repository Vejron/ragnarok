//! The backpack mode (BACKPACK.md): one Intinor Compact Backpack in a world whose scenario places it.
//! A soldier on foot who walks onto it takes it; while they carry it they heal, walk twice as fast
//! (`Movement` reads `Carry`) and earn a point a second (`Player::held`). It falls where they die or
//! leave, and goes back where the scenario put it when it lands where nobody can reach it or lies
//! untouched for `BACKPACK_RETURN_SECONDS`.
//!
//! Everyone is told a circle it is somewhere inside (`S_BACKPACK`), never where it is: the circle's
//! middle is off by a random amount, and moves on to a new one when the backpack nears its edge. It is
//! `BACKPACK_WIDE` across while the backpack is new to its carrier or lying about, and narrows to
//! `BACKPACK_NARROW` over `BACKPACK_NARROW_SECONDS` in one carrier's hands.

use super::Mechanic;
use super::combat::position_of;
use super::life::open_ground;
use crate::entities::{Component, Health, Transform, With};
use crate::game::{Died, Player, World};
use shared::consts::*;
use shared::protocol::{Backpack as Circle, S_BACKPACK};
use shared::quant::to_cm;

pub struct Backpack;

/// Carried on its back by the soldier who has the backpack.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry;

/// The backpack lying on the ground, at its entity's `Transform` (its feet).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lying;

/// How far off the middle of the circle the backpack may be when it is drawn anew, of its radius.
const OFFSET: f32 = 0.6;
/// The circle is drawn anew once the backpack is this far out from its middle, of its radius.
const EDGE: f32 = 0.9;

/// Where the backpack is and what everyone has been told of it. `World::backpack` is `None` in a world
/// without one.
#[derive(Clone, Debug)]
pub struct BackpackState {
    /// Where the scenario put it, metres `[x, z]`, and where it goes back to.
    pub home: [f32; 2],
    /// Who has it.
    pub carrier: Option<u32>,
    /// Its entity while it lies on the ground.
    pub ground: Option<u32>,
    /// The tick its carrier took it, or it was put down.
    pub since: u32,
    /// The circle as last drawn, metres.
    pub centre: [f32; 2],
    pub radius: f32,
    /// Health healed but not yet whole.
    regen: f32,
    /// The tick everyone was last told the circle, and whether it has changed since.
    told: u32,
    changed: bool,
    /// To be broadcast at the end of the tick (`Game::step`).
    pub fresh: Option<Circle>,
}

impl BackpackState {
    pub fn new(home: [f32; 2]) -> Self {
        BackpackState { home, carrier: None, ground: None, since: 0, centre: home, radius: BACKPACK_WIDE / 2.0, regen: 0.0, told: 0, changed: true, fresh: None }
    }

    /// The circle as everyone was last told it, with `held`: the carrier's seconds.
    pub fn circle(&self, held: u16) -> Circle {
        Circle {
            tag: S_BACKPACK,
            x: to_cm(self.centre[0]),
            z: to_cm(self.centre[1]),
            radius: (self.radius * 100.0).round() as u16,
            carrier: self.carrier.unwrap_or(0),
            held,
        }
    }
}

/// The circle's radius `ticks` after its carrier took it: wide at first, narrowing to its least.
pub fn radius_after(ticks: u32) -> f32 {
    let done = (ticks as f32 / (BACKPACK_NARROW_SECONDS * TICK_RATE as f32)).min(1.0);
    (BACKPACK_WIDE + (BACKPACK_NARROW - BACKPACK_WIDE) * done) / 2.0
}

/// The carrier's seconds with it, as sent.
fn held(world: &World, carrier: Option<u32>) -> u16 {
    carrier.and_then(|id| world.entities.get::<Player>(id)).map_or(0, |player| player.held)
}

/// Where the backpack is: its carrier's place (their vehicle's, seated), or where it lies.
fn whereabouts(world: &World, state: &BackpackState) -> Option<[f32; 3]> {
    match (state.carrier, state.ground) {
        (Some(id), _) => position_of(world, id),
        (None, Some(id)) => world.entities.get::<Transform>(id).map(|transform| transform.position),
        _ => None,
    }
}

/// Lays the backpack down at `[x, floor, z]`.
fn lay(world: &mut World, [x, floor, z]: [f32; 3]) {
    let id = world.alloc_id();
    world.entities.spawn(id, (Transform { position: [x, floor, z], yaw: 0.0 }, Lying));
    let tick = world.tick;
    let state = world.backpack.as_mut().unwrap();
    state.ground = Some(id);
    state.carrier = None;
    state.since = tick;
    state.changed = true;
}

/// Lays the backpack down where the scenario put it.
fn lay_home(world: &mut World) {
    let Some([x, z]) = world.backpack.as_ref().map(|state| state.home) else { return };
    let ground = world.map.ground.height_at(x, z);
    let top = ground + 40.0;
    let floor = world.physics.cast_ray([x, top, z], [0.0, -1.0, 0.0], 80.0, 0).map_or(ground, |t| top - t);
    lay(world, [x, floor, z]);
}

/// The floor under a soldier's body centred at `at`, which they could stand on: none off the world.
fn floor_under(world: &World, at: [f32; 3]) -> Option<f32> {
    if !world.map.is_inside((at[0] * 100.0) as i32, (at[2] * 100.0) as i32) {
        return None;
    }
    let ground = world.map.ground.height_at(at[0], at[2]);
    let floor = world.physics.cast_ray(at, [0.0, -1.0, 0.0], (at[1] - ground).max(0.0) + 1.0, 0).map_or(ground, |t| at[1] - t);
    Some(floor.max(ground))
}

/// Takes the backpack off player `id`, if they have it, and lays it where they are: under their body
/// on foot (on the floor of the room they died in, or the ground they fell to), on open ground beside
/// a vehicle, or back home when there is nowhere to put it.
fn drop_from(world: &mut World, id: u32) {
    if world.backpack.as_ref().and_then(|state| state.carrier) != Some(id) {
        return;
    }
    world.entities.remove::<Carry>(id);
    let player = world.entities.get::<Player>(id);
    let on_foot = player.filter(|player| player.seat.is_none()).map(|player| player.character.position);
    let spot = match (on_foot, position_of(world, id)) {
        (Some(at), _) => floor_under(world, at).map(|floor| [at[0], floor, at[2]]),
        (None, Some(at)) => open_ground(world, at[0], at[2]).map(|floor| [at[0], floor, at[2]]),
        (None, None) => None,
    };
    match spot {
        Some(spot) => lay(world, spot),
        None => lay_home(world),
    }
}

/// Gives the backpack lying at `ground` to player `id`.
fn take(world: &mut World, id: u32, ground: u32) {
    world.entities.despawn(ground);
    world.entities.insert(id, Carry);
    let tick = world.tick;
    let state = world.backpack.as_mut().unwrap();
    state.ground = None;
    state.carrier = Some(id);
    state.since = tick;
    state.regen = 0.0;
    state.radius = radius_after(0);
    // A new carrier is a new circle: the old one says nothing of where they will run.
    state.centre = [f32::INFINITY; 2];
    state.changed = true;
}

/// The living soldier on foot within reach of the backpack lying at `at`, nearest first.
fn reaching(world: &World, at: [f32; 3]) -> Option<u32> {
    world
        .entities
        .all::<Player>()
        .filter(|&(id, player)| player.seat.is_none() && world.entities.alive(id))
        .map(|(id, player)| (id, player.character.position))
        .filter(|(_, p)| (p[1] - at[1]).abs() <= BACKPACK_REACH_Y)
        .map(|(id, p)| (id, (p[0] - at[0]).hypot(p[2] - at[2])))
        .filter(|&(_, d)| d <= BACKPACK_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(id, _)| id)
}

impl Mechanic for Backpack {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let Some(state) = &world.backpack else { return };
        let (carrier, ground, since) = (state.carrier, state.ground, state.since);
        match (carrier, ground) {
            (None, None) => lay_home(world),
            // Gone without a word (it should have been dropped as they left): start again.
            (Some(id), _) if !world.entities.has::<Player>(id) => lay_home(world),
            (Some(id), _) => {
                if let Some(mut health) = world.entities.get_mut::<Health>(id).filter(|health| health.alive()) {
                    let state = world.backpack.as_mut().unwrap();
                    state.regen += BACKPACK_REGEN * dt;
                    // A hair over, so a second's worth of ticks adds up to the whole of it.
                    let whole = (state.regen + 1e-4).floor();
                    state.regen -= whole;
                    health.hp = (health.hp as f32 + whole).min(health.max as f32) as u8;
                    // The head and the limbs mend as fast.
                    super::limbs::heal(world, id, whole as u8);
                }
                if world.tick.wrapping_sub(since).is_multiple_of(TICK_RATE) && world.tick != since {
                    let mut player = world.entities.of_mut::<Player>(id);
                    player.held = player.held.saturating_add(1);
                }
            }
            (None, Some(id)) => {
                let at = world.entities.get::<Transform>(id).map(|transform| transform.position);
                let home = world.backpack.as_ref().unwrap().home;
                let away = at.is_some_and(|at| (at[0] - home[0]).hypot(at[2] - home[1]) > 1.0);
                if away && world.tick.wrapping_sub(since) as f32 >= BACKPACK_RETURN_SECONDS * TICK_RATE as f32 {
                    world.entities.despawn(id);
                    lay_home(world);
                } else if let Some(at) = at
                    && let Some(taker) = reaching(world, at)
                {
                    take(world, taker, id);
                }
            }
        }

        let Some(at) = world.backpack.as_ref().and_then(|state| whereabouts(world, state)) else { return };
        let (tick, offset) = (world.tick, [world.rng.next_f64() as f32, world.rng.next_f64() as f32]);
        let state = world.backpack.as_mut().unwrap();
        state.radius = if state.carrier.is_some() { radius_after(tick.wrapping_sub(state.since)) } else { radius_after(0) };
        let out = (at[0] - state.centre[0]).hypot(at[2] - state.centre[1]);
        // A new carrier's circle starts infinitely far off, so it is drawn anew at once.
        if out.is_nan() || out > state.radius * EDGE {
            let (angle, reach) = (offset[0] * std::f32::consts::TAU, OFFSET * state.radius * offset[1].sqrt());
            state.centre = [at[0] + reach * angle.cos(), at[2] + reach * angle.sin()];
            state.changed = true;
        }
        if state.changed || tick.wrapping_sub(state.told) >= TICK_RATE {
            state.changed = false;
            state.told = tick;
            let carrier = state.carrier;
            let circle = world.backpack.as_ref().unwrap().circle(held(world, carrier));
            world.backpack.as_mut().unwrap().fresh = Some(circle);
        }
    }

    fn on_leave(&mut self, world: &mut World, id: u32) {
        drop_from(world, id);
    }

    /// A carrier who died lets it fall where they lie.
    fn react(&mut self, world: &mut World) {
        let Some(carrier) = world.backpack.as_ref().and_then(|state| state.carrier) else { return };
        if world.happened::<Died>().any(|death| death.victim == carrier) {
            drop_from(world, carrier);
        }
    }
}

/// Whether player `id` carries the backpack.
pub fn carries(world: &World, id: u32) -> bool {
    world.entities.has::<Carry>(id)
}

/// The ground backpack, for replication: its id and where it lies.
pub fn lying(world: &World) -> Vec<(u32, [f32; 3])> {
    world.entities.ids::<With<Lying>>().into_iter().filter_map(|id| Some((id, world.entities.get::<Transform>(id)?.position))).collect()
}

#[cfg(test)]
mod tests;
