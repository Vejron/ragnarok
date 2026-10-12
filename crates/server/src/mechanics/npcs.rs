use super::Mechanic;
use super::combat::ray_capsule;
use crate::game::{Pickup, Rooster, RoosterMode, World};
use shared::consts::*;
use shared::items::ITEM_GOLDEN_EGG;
use shared::quant::from_cm;
use shared::world::project;
use std::f32::consts::{PI, TAU};

/// The Hedemora rooster. It stands about, walks to a random spot within `ROOSTER_WANDER_RADIUS` of
/// its home, stands about again, and now and then stops to crow (`FLAG_CROWING`, which the client
/// turns into the `crow` animation and a "kokoo!"). Shot dead it drops a golden egg, and a minute
/// later a new rooster is at home.
pub struct Npcs;

/// How high above the ground the egg floats, metres, as pickups do.
const EGG_FLOAT: f32 = 0.5;
/// Tries to find an open spot to walk to before it stays put.
const TARGET_ATTEMPTS: usize = 8;
/// Gives up on a walk after this many ticks, so a rooster that cannot get there does not stand
/// and push for ever.
const WALK_TIMEOUT_TICKS: u32 = 12 * TICK_RATE;
/// Standing about lasts this long, ticks.
const IDLE_MIN_TICKS: u32 = TICK_RATE;
const IDLE_SPAN_TICKS: u32 = 3 * TICK_RATE;
/// One in this many stops is a crow.
const CROW_ONE_IN: u64 = 3;
/// A wall this far ahead (at body height) ends a walk, metres.
const WALL_AHEAD: f32 = 0.5;
const ARRIVED: f32 = 0.3;

/// Ground height at `x`, `z` if it is open ground (nothing, such as a roof, above it).
fn open_ground(world: &World, x: f32, z: f32) -> Option<f32> {
    let ground = world.map.ground.height_at(x, z);
    let top = ground + 40.0;
    let floor = world.physics.cast_ray([x, top, z], [0.0, -1.0, 0.0], 80.0, 0).map(|t| top - t)?;
    ((floor - ground).abs() < 0.5).then_some(floor)
}

/// Where the rooster lives: `ROOSTER_HOME` if that is inside the world, else the first spawn.
pub fn home(world: &World) -> [f32; 2] {
    let [x, z] = project(world.map.origin, ROOSTER_HOME.0, ROOSTER_HOME.1);
    let half = from_cm(world.map.half_extent) as f64 - 10.0;
    if x.abs() < half && z.abs() < half {
        return [x as f32, z as f32];
    }
    world.map.spawns.first().map_or([0.0, 0.0], |s| [from_cm(s[0]), from_cm(s[1])])
}

fn random_between(world: &mut World, low: f32, high: f32) -> f32 {
    low + world.rng.next_f64() as f32 * (high - low)
}

/// A random open spot within the wander radius of `home`.
fn pick_target(world: &mut World, home: [f32; 2]) -> Option<[f32; 2]> {
    for _ in 0..TARGET_ATTEMPTS {
        let angle = random_between(world, 0.0, TAU);
        let distance = random_between(world, 1.0, ROOSTER_WANDER_RADIUS);
        let at = [home[0] + angle.cos() * distance, home[1] + angle.sin() * distance];
        if open_ground(world, at[0], at[1]).is_some() {
            return Some(at);
        }
    }
    None
}

fn stand_about(world: &mut World, id: u32) {
    let ticks = IDLE_MIN_TICKS + (world.rng.next_u64() % IDLE_SPAN_TICKS as u64) as u32;
    if let Some(rooster) = world.roosters.get_mut(&id) {
        rooster.mode = RoosterMode::Idle;
        rooster.mode_left = ticks;
    }
}

/// Puts a rooster at home, alive, standing about.
fn place(world: &mut World, id: u32) {
    let Some(home) = world.roosters.get(&id).map(|r| r.home) else { return };
    let y = open_ground(world, home[0], home[1]).unwrap_or_else(|| world.map.ground.height_at(home[0], home[1]));
    let Some(rooster) = world.roosters.get_mut(&id) else { return };
    rooster.position = [home[0], y, home[1]];
    rooster.hp = ROOSTER_HP;
    rooster.respawn_in = 0;
    stand_about(world, id);
}

/// The nearest living rooster the ray meets within `max`, and how far along it is.
pub fn first_rooster_hit(world: &World, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(u32, f32)> {
    let mut best: Option<(u32, f32)> = None;
    for (&id, rooster) in world.roosters.iter().filter(|(_, r)| r.alive()) {
        let [x, y, z] = rooster.position;
        let centre = [x, y + ROOSTER_CAPSULE_CENTRE, z];
        let limit = best.map_or(max, |(_, t)| t);
        if let Some(t) = ray_capsule(origin, dir, centre, ROOSTER_CAPSULE_HALF_HEIGHT, ROOSTER_CAPSULE_RADIUS, limit) {
            best = Some((id, t));
        }
    }
    best
}

/// Takes `amount` health off rooster `id`. At zero it dies: a golden egg is left where it fell.
pub fn damage_rooster(world: &mut World, id: u32, amount: u8) {
    let Some(rooster) = world.roosters.get_mut(&id) else { return };
    if !rooster.alive() || amount == 0 {
        return;
    }
    rooster.hp = rooster.hp.saturating_sub(amount);
    if rooster.hp > 0 {
        return;
    }
    rooster.respawn_in = ROOSTER_RESPAWN_TICKS;
    let [x, y, z] = rooster.position;
    // One uncollected egg at a time: a rooster killed over and over does not carpet the field.
    world.pickups.retain(|_, p| !(p.dropped && p.item == ITEM_GOLDEN_EGG));
    let egg = world.alloc_id();
    world.pickups.insert(egg, Pickup { item: ITEM_GOLDEN_EGG, count: 1, position: [x, y + EGG_FLOAT, z], respawn_in: 0, dropped: true });
    tracing::debug!(rooster = id, egg, "rooster killed");
}

/// Turns `yaw` towards `want` by at most `step`, the short way round.
fn turn_towards(yaw: f32, want: f32, step: f32) -> f32 {
    let delta = (want - yaw + PI).rem_euclid(TAU) - PI;
    yaw + delta.clamp(-step, step)
}

fn step_rooster(world: &mut World, id: u32, dt: f32) {
    let Some(mut rooster) = world.roosters.get(&id).copied() else { return };
    if !rooster.alive() {
        rooster.respawn_in = rooster.respawn_in.saturating_sub(1);
        world.roosters.insert(id, rooster);
        if rooster.respawn_in == 0 {
            place(world, id);
        }
        return;
    }
    rooster.mode_left = rooster.mode_left.saturating_sub(1);
    match rooster.mode {
        RoosterMode::Idle => {
            if rooster.mode_left == 0 {
                if world.rng.next_u64().is_multiple_of(CROW_ONE_IN) {
                    rooster.mode = RoosterMode::Crow;
                    rooster.mode_left = ROOSTER_CROW_TICKS;
                } else if let Some(target) = pick_target(world, rooster.home) {
                    rooster.mode = RoosterMode::Wander;
                    rooster.mode_left = WALK_TIMEOUT_TICKS;
                    rooster.target = target;
                } else {
                    rooster.mode_left = IDLE_MIN_TICKS;
                }
            }
        }
        RoosterMode::Crow => {
            if rooster.mode_left == 0 {
                rooster.mode = RoosterMode::Idle;
                rooster.mode_left = IDLE_MIN_TICKS;
            }
        }
        RoosterMode::Wander => {
            let [x, y, z] = rooster.position;
            let (dx, dz) = (rooster.target[0] - x, rooster.target[1] - z);
            let distance = dx.hypot(dz);
            // Yaw 0 faces -Z, as for players: forward is (-sin yaw, -cos yaw).
            let want = (-dx).atan2(-dz);
            rooster.yaw = turn_towards(rooster.yaw, want, ROOSTER_TURN_RATE * dt);
            let forward = [-rooster.yaw.sin(), -rooster.yaw.cos()];
            let aligned = (turn_towards(rooster.yaw, want, PI) - rooster.yaw).abs() < 0.5 || distance < ARRIVED;
            let blocked = aligned
                && world.physics.cast_ray_hit([x, y + ROOSTER_CAPSULE_CENTRE, z], [forward[0], 0.0, forward[1]], WALL_AHEAD, 0).is_some();
            if distance < ARRIVED || blocked || rooster.mode_left == 0 {
                world.roosters.insert(id, rooster);
                stand_about(world, id);
                return;
            }
            // A rooster only walks once it faces where it is going.
            if aligned {
                let step = (ROOSTER_SPEED * dt).min(distance);
                let (nx, nz) = (x + forward[0] * step, z + forward[1] * step);
                rooster.position = [nx, world.map.ground.height_at(nx, nz), nz];
            }
        }
    }
    world.roosters.insert(id, rooster);
}

impl Mechanic for Npcs {
    fn on_start(&mut self, world: &mut World) {
        let home = home(world);
        let id = world.alloc_id();
        let rooster = Rooster {
            position: [home[0], 0.0, home[1]],
            yaw: 0.0,
            hp: ROOSTER_HP,
            mode: RoosterMode::Idle,
            mode_left: IDLE_MIN_TICKS,
            target: home,
            home,
            respawn_in: 0,
        };
        world.roosters.insert(id, rooster);
        place(world, id);
        tracing::info!(x = home[0], z = home[1], "rooster placed");
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids: Vec<u32> = world.roosters.keys().copied().collect();
        for id in ids {
            step_rooster(world, id, dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use shared::items::{ITEM_COUNT, def};
    use shared::world::{WORLD_VERSION, WorldFile};

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[1000, -500]], ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    fn rooster(game: &Game) -> (u32, Rooster) {
        game.world.roosters.iter().next().map(|(&id, &r)| (id, r)).expect("a rooster is placed on start")
    }

    #[test]
    fn it_lives_at_the_first_spawn_when_its_home_is_outside_the_world() {
        // The default origin is (0, 0) degrees: the coordinate is millions of metres outside.
        let game = game();
        let (_, r) = rooster(&game);
        assert_eq!(r.home, [10.0, -5.0]);
        assert_eq!(r.hp, ROOSTER_HP);
    }

    #[test]
    fn it_lives_at_its_coordinate_when_the_world_covers_it() {
        let map = WorldFile { version: WORLD_VERSION, origin: (63.8372, 20.3261), half_extent: 112_500, spawns: vec![[0, 0]], ..Default::default() };
        let game = Game::new(map, 0).unwrap();
        let (_, r) = rooster(&game);
        // About 396 m north (-Z) and 15 m east of the dev world's centre.
        assert!((r.home[0] - 15.2).abs() < 1.0, "x {}", r.home[0]);
        assert!((r.home[1] + 396.2).abs() < 1.0, "z {}", r.home[1]);
    }

    #[test]
    fn it_meanders_near_home_and_stops_to_crow() {
        let mut game = game();
        let (id, start) = rooster(&game);
        let mut crowed = false;
        let mut walked = false;
        for _ in 0..(120 * TICK_RATE) {
            game.step();
            let r = game.world.roosters[&id];
            let from_home = (r.position[0] - r.home[0]).hypot(r.position[2] - r.home[1]);
            assert!(from_home <= ROOSTER_WANDER_RADIUS + 1.0, "strayed {from_home} m");
            crowed |= r.mode == RoosterMode::Crow;
            walked |= r.mode == RoosterMode::Wander;
        }
        assert!(crowed, "never crowed");
        assert!(walked, "never walked");
        let now = game.world.roosters[&id].position;
        assert_ne!(now, start.position);
    }

    #[test]
    fn a_shot_rooster_drops_a_golden_egg_and_comes_back() {
        let mut game = game();
        let (id, r) = rooster(&game);
        game.world.pickups.clear();
        // A ray through the middle of it.
        let centre = [r.position[0], r.position[1] + ROOSTER_CAPSULE_CENTRE, r.position[2]];
        let hit = first_rooster_hit(&game.world, [centre[0], centre[1], centre[2] + 5.0], [0.0, 0.0, -1.0], 20.0);
        assert_eq!(hit.map(|(who, _)| who), Some(id));

        damage_rooster(&mut game.world, id, ROOSTER_HP);
        assert!(!game.world.roosters[&id].alive());
        let eggs: Vec<_> = game.world.pickups.values().filter(|p| p.item == ITEM_GOLDEN_EGG).collect();
        assert_eq!(eggs.len(), 1);
        assert!(eggs[0].dropped);
        assert!(first_rooster_hit(&game.world, [centre[0], centre[1], centre[2] + 5.0], [0.0, 0.0, -1.0], 20.0).is_none(), "dead is not hit");
        assert!(game.entity_states().iter().all(|e| e.1 != shared::protocol::KIND_ROOSTER));

        for _ in 0..ROOSTER_RESPAWN_TICKS {
            game.step();
        }
        assert!(game.world.roosters[&id].alive());
        assert!(game.entity_states().iter().any(|e| e.1 == shared::protocol::KIND_ROOSTER));
    }

    #[test]
    fn the_egg_is_never_scattered() {
        assert!(def(ITEM_GOLDEN_EGG).is_some_and(|d| d.weight == 0));
        assert_eq!(ITEM_COUNT, ITEM_GOLDEN_EGG + 1);
    }
}
