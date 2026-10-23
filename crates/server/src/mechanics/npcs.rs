use super::Mechanic;
use super::life::{open_ground, place_at_home};
use crate::entities::{Component, DropOnDeath, Health, Home, Hurtbox, NetKind, Respawn, Respawned, Transform, With};
use crate::game::World;
use shared::consts::*;
use shared::items::ITEM_GOLDEN_EGG;
use shared::protocol::{HIT_ROOSTER, KIND_ROOSTER};
use shared::quant::from_cm;
use shared::world::project;
use std::f32::consts::{PI, TAU};

/// Creatures that wander about their home. The Hedemora rooster is one: it stands about, walks to a
/// random spot within its gait's radius of home, stands about again, and now and then stops to crow
/// (`FLAG_CROWING`, which the client turns into the `crow` animation and a "kokoo!"). Being shot,
/// dying, dropping its golden egg and coming back a minute later are `Life`'s, from its components.
pub struct Npcs;

/// Tries to find an open spot to walk to before it stays put.
const TARGET_ATTEMPTS: usize = 8;
/// Gives up on a walk after this many ticks, so a creature that cannot get there does not stand and
/// push for ever.
const WALK_TIMEOUT_TICKS: u32 = 12 * TICK_RATE;
/// Standing about lasts this long, ticks.
const IDLE_MIN_TICKS: u32 = TICK_RATE;
const IDLE_SPAN_TICKS: u32 = 3 * TICK_RATE;
/// A wall this far ahead (at body height) ends a walk, metres.
const WALL_AHEAD: f32 = 0.5;
const ARRIVED: f32 = 0.3;

/// How a wandering creature moves and calls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    /// How far from home it walks, metres.
    pub radius: f32,
    pub speed: f32,
    /// rad/s.
    pub turn_rate: f32,
    /// How long a call lasts, ticks, and one in how many stops is one.
    pub call_ticks: u32,
    pub call_one_in: u64,
}

pub const ROOSTER_GAIT: Gait = Gait { radius: ROOSTER_WANDER_RADIUS, speed: ROOSTER_SPEED, turn_rate: ROOSTER_TURN_RATE, call_ticks: ROOSTER_CROW_TICKS, call_one_in: 3 };

/// What a wandering creature is doing. `mode_left` counts ticks down to the next decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WanderMode {
    Idle,
    Walk,
    /// A rooster's crow.
    Call,
}

/// The mind of a creature that wanders about its `Home`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Wander {
    pub gait: Gait,
    pub mode: WanderMode,
    pub mode_left: u32,
    /// Where it is walking to, `[x, z]`.
    pub target: [f32; 2],
}

impl Wander {
    pub fn new(gait: Gait, home: [f32; 2]) -> Self {
        Wander { gait, mode: WanderMode::Idle, mode_left: IDLE_MIN_TICKS, target: home }
    }
}

/// A Hedemora rooster at `home`: shot dead it drops a golden egg (one lying about at a time), and a
/// minute later it is back at home.
pub fn rooster(home: [f32; 2]) -> impl crate::entities::Bundle {
    (
        NetKind(KIND_ROOSTER),
        Transform { position: [home[0], 0.0, home[1]], yaw: 0.0 },
        Health::full(ROOSTER_HP),
        Hurtbox { centre_y: ROOSTER_CAPSULE_CENTRE, half_height: ROOSTER_CAPSULE_HALF_HEIGHT, radius: ROOSTER_CAPSULE_RADIUS, head: None, mark_y: ROOSTER_CAPSULE_HALF_HEIGHT, hit_kind: HIT_ROOSTER },
        Home(home),
        Respawn { delay: ROOSTER_RESPAWN_TICKS, left: 0 },
        DropOnDeath { item: ITEM_GOLDEN_EGG, count: 1, unique: true },
        Wander::new(ROOSTER_GAIT, home),
        crate::entities::Trail::default(),
    )
}

/// Where the rooster lives: `ROOSTER_HOME` if that is inside the world, else the first spawn.
pub fn home(world: &World) -> [f32; 2] {
    let [x, z] = project(world.map.origin, ROOSTER_HOME.0, ROOSTER_HOME.1);
    let half = from_cm(world.map.half_extent) as f64 - 10.0;
    if x.abs() < half && z.abs() < half && world.map.is_inside((x * 100.0) as i32, (z * 100.0) as i32) {
        return [x as f32, z as f32];
    }
    world.map.spawns.first().map_or([0.0, 0.0], |s| [from_cm(s[0]), from_cm(s[1])])
}

fn random_between(world: &mut World, low: f32, high: f32) -> f32 {
    low + world.rng.next_f64() as f32 * (high - low)
}

/// A random open spot within `radius` of `home`.
fn pick_target(world: &mut World, home: [f32; 2], radius: f32) -> Option<[f32; 2]> {
    for _ in 0..TARGET_ATTEMPTS {
        let angle = random_between(world, 0.0, TAU);
        let distance = random_between(world, 1.0, radius);
        let at = [home[0] + angle.cos() * distance, home[1] + angle.sin() * distance];
        if open_ground(world, at[0], at[1]).is_some() {
            return Some(at);
        }
    }
    None
}

fn stand_about(world: &mut World, id: u32) {
    let ticks = IDLE_MIN_TICKS + (world.rng.next_u64() % IDLE_SPAN_TICKS as u64) as u32;
    if let Some(mut wander) = world.entities.get_mut::<Wander>(id) {
        wander.mode = WanderMode::Idle;
        wander.mode_left = ticks;
    }
}

/// Whether entity `id` is calling (a rooster crowing), which the snapshot shows as `FLAG_CROWING`.
pub fn calling(world: &World, id: u32) -> bool {
    world.entities.get::<Wander>(id).is_some_and(|wander| wander.mode == WanderMode::Call)
}

/// Turns `yaw` towards `want` by at most `step`, the short way round.
pub(super) fn turn_towards(yaw: f32, want: f32, step: f32) -> f32 {
    let delta = (want - yaw + PI).rem_euclid(TAU) - PI;
    yaw + delta.clamp(-step, step)
}

fn step_wanderer(world: &mut World, id: u32, dt: f32) {
    let (Some(mut wander), Some(mut transform), Some(Home(home))) =
        (world.entities.get::<Wander>(id).copied(), world.entities.get::<Transform>(id).copied(), world.entities.get::<Home>(id).copied())
    else {
        return;
    };
    let gait = wander.gait;
    wander.mode_left = wander.mode_left.saturating_sub(1);
    match wander.mode {
        WanderMode::Idle => {
            if wander.mode_left == 0 {
                if world.rng.next_u64().is_multiple_of(gait.call_one_in) {
                    wander.mode = WanderMode::Call;
                    wander.mode_left = gait.call_ticks;
                } else if let Some(target) = pick_target(world, home, gait.radius) {
                    wander.mode = WanderMode::Walk;
                    wander.mode_left = WALK_TIMEOUT_TICKS;
                    wander.target = target;
                } else {
                    wander.mode_left = IDLE_MIN_TICKS;
                }
            }
        }
        WanderMode::Call => {
            if wander.mode_left == 0 {
                wander.mode = WanderMode::Idle;
                wander.mode_left = IDLE_MIN_TICKS;
            }
        }
        WanderMode::Walk => {
            let [x, y, z] = transform.position;
            let (dx, dz) = (wander.target[0] - x, wander.target[1] - z);
            let distance = dx.hypot(dz);
            // Yaw 0 faces -Z, as for players: forward is (-sin yaw, -cos yaw).
            let want = (-dx).atan2(-dz);
            transform.yaw = turn_towards(transform.yaw, want, gait.turn_rate * dt);
            let forward = [-transform.yaw.sin(), -transform.yaw.cos()];
            let aligned = (turn_towards(transform.yaw, want, PI) - transform.yaw).abs() < 0.5 || distance < ARRIVED;
            let body = world.entities.get::<Hurtbox>(id).map_or(0.0, |hurtbox| hurtbox.centre_y);
            let blocked = aligned && world.physics.cast_ray_hit([x, y + body, z], [forward[0], 0.0, forward[1]], WALL_AHEAD, 0).is_some();
            if distance < ARRIVED || blocked || wander.mode_left == 0 {
                write(world, id, wander, transform);
                stand_about(world, id);
                return;
            }
            // It only walks once it faces where it is going.
            if aligned {
                let step = (gait.speed * dt).min(distance);
                let (nx, nz) = (x + forward[0] * step, z + forward[1] * step);
                transform.position = [nx, world.map.ground.height_at(nx, nz), nz];
            }
        }
    }
    write(world, id, wander, transform);
}

fn write(world: &mut World, id: u32, wander: Wander, transform: Transform) {
    if let Some(mut slot) = world.entities.get_mut::<Wander>(id) {
        *slot = wander;
    }
    if let Some(mut slot) = world.entities.get_mut::<Transform>(id) {
        *slot = transform;
    }
}

impl Mechanic for Npcs {
    fn on_start(&mut self, world: &mut World) {
        let home = home(world);
        let id = world.alloc_id();
        world.entities.spawn(id, rooster(home));
        place_at_home(world, id);
        // Placed, it starts out standing about, as after any respawn.
        world.entities.remove::<Respawned>(id);
        stand_about(world, id);
        tracing::info!(x = home[0], z = home[1], "rooster placed");
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for id in world.entities.ids::<With<Wander>>() {
            // Back from the dead this tick (`Life` put it home): it stands about before anything else.
            if world.entities.get::<Respawned>(id).is_some() {
                world.entities.remove::<Respawned>(id);
                stand_about(world, id);
                continue;
            }
            if world.entities.get::<Health>(id).is_some_and(Health::alive) {
                step_wanderer(world, id, dt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::mechanics::life::{damage_entity, first_hurtbox_hit};
    use shared::items::def;
    use shared::world::{WORLD_VERSION, WorldFile};

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[1000, -500]], ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    /// The rooster as its parts: id, where it stands, its health, its home and its mind.
    struct Seen {
        id: u32,
        transform: Transform,
        health: Health,
        home: [f32; 2],
        wander: Wander,
    }

    fn rooster(game: &Game) -> Seen {
        let entities = &game.world.entities;
        let id = *entities.ids::<With<Wander>>().first().expect("a rooster is placed on start");
        Seen {
            id,
            transform: *entities.get::<Transform>(id).unwrap(),
            health: *entities.get::<Health>(id).unwrap(),
            home: entities.get::<Home>(id).unwrap().0,
            wander: *entities.get::<Wander>(id).unwrap(),
        }
    }

    #[test]
    fn it_lives_at_the_first_spawn_when_its_home_is_outside_the_world() {
        // The default origin is (0, 0) degrees: the coordinate is millions of metres outside.
        let game = game();
        let r = rooster(&game);
        assert_eq!(r.home, [10.0, -5.0]);
        assert_eq!(r.health.hp, ROOSTER_HP);
    }

    #[test]
    fn it_lives_at_its_coordinate_when_the_world_covers_it() {
        let map = WorldFile { version: WORLD_VERSION, origin: (63.8372, 20.3261), half_extent: 112_500, spawns: vec![[0, 0]], ..Default::default() };
        let game = Game::new(map, 0).unwrap();
        let r = rooster(&game);
        // About 396 m north (-Z) and 15 m east of the dev world's centre.
        assert!((r.home[0] - 15.2).abs() < 1.0, "x {}", r.home[0]);
        assert!((r.home[1] + 396.2).abs() < 1.0, "z {}", r.home[1]);
    }

    #[test]
    fn it_meanders_near_home_and_stops_to_crow() {
        let mut game = game();
        let start = rooster(&game);
        let mut crowed = false;
        let mut walked = false;
        for _ in 0..(120 * TICK_RATE) {
            game.step();
            let r = rooster(&game);
            let from_home = (r.transform.position[0] - r.home[0]).hypot(r.transform.position[2] - r.home[1]);
            assert!(from_home <= ROOSTER_WANDER_RADIUS + 1.0, "strayed {from_home} m");
            crowed |= r.wander.mode == WanderMode::Call;
            walked |= r.wander.mode == WanderMode::Walk;
        }
        assert!(crowed, "never crowed");
        assert!(walked, "never walked");
        assert_ne!(rooster(&game).transform.position, start.transform.position);
    }

    #[test]
    fn a_lagging_shooter_hits_a_rooster_where_they_saw_it() {
        let mut game = game();
        game.step();
        game.step();
        let r = rooster(&game);
        let p = game.world.entities.of::<crate::entities::Trail>(r.id).at(game.world.tick.wrapping_sub(1), false).expect("a tick of trail");
        // A ray through the middle of where it stood a tick ago, after it has stepped 2 m aside.
        let then = [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2] + 5.0];
        game.world.entities.get_mut::<Transform>(r.id).unwrap().position = [p[0] + 2.0, p[1], p[2]];
        assert!(first_hurtbox_hit(&game.world, then, [0.0, 0.0, -1.0], 20.0, None, 0, None).is_none(), "it is not there now");
        assert_eq!(first_hurtbox_hit(&game.world, then, [0.0, 0.0, -1.0], 20.0, None, 1, None).map(|(who, _)| who), Some(r.id), "it was a tick ago");
    }

    #[test]
    fn a_shot_rooster_drops_a_golden_egg_and_comes_back() {
        let mut game = game();
        let r = rooster(&game);
        game.world.clear_pickups();
        // A ray through the middle of it.
        let p = r.transform.position;
        let centre = [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2]];
        let hit = first_hurtbox_hit(&game.world, [centre[0], centre[1], centre[2] + 5.0], [0.0, 0.0, -1.0], 20.0, None, 0, None);
        assert_eq!(hit.map(|(who, _)| who), Some(r.id));

        assert!(damage_entity(&mut game.world, r.id, ROOSTER_HP, None, 0, None));
        assert!(!rooster(&game).health.alive());
        let eggs: Vec<_> = game.world.pickup_ids().into_iter().filter_map(|pid| game.world.pickup(pid).copied()).filter(|p| p.item == ITEM_GOLDEN_EGG).collect();
        assert_eq!(eggs.len(), 1);
        assert!(eggs[0].dropped);
        assert!(first_hurtbox_hit(&game.world, [centre[0], centre[1], centre[2] + 5.0], [0.0, 0.0, -1.0], 20.0, None, 0, None).is_none(), "dead is not hit");
        assert!(game.seen_people(KIND_ROOSTER).is_empty());

        for _ in 0..ROOSTER_RESPAWN_TICKS {
            game.step();
        }
        let back = rooster(&game);
        assert!(back.health.alive());
        assert_eq!(back.wander.mode, WanderMode::Idle, "back from the dead it stands about first");
        assert!(!game.seen_people(KIND_ROOSTER).is_empty());
    }

    #[test]
    fn a_hitscan_round_and_a_bullet_in_flight_both_hurt_a_rooster() {
        use crate::mechanics::combat::{Round, hitscan};
        use crate::mechanics::projectiles::Flying;
        use shared::items::ITEM_AK5;
        use shared::protocol::{HIT_DEALT, PROJECTILE_BULLET};
        let gun = def(ITEM_AK5).unwrap();
        let shooter = 999;

        let mut game = game();
        let r = rooster(&game);
        let p = r.transform.position;
        let round = Round { damage: 1, vehicle: 1, plane: 1, weapon: gun.weapon, range: gun.range, falloff: gun.falloff };
        let (_, struck) = hitscan(&mut game.world, shooter, [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2] + 5.0], [0.0, 0.0, -1.0], &round, 0, 0);
        assert!(struck);
        assert_eq!(rooster(&game).health.hp, ROOSTER_HP - 1);
        assert!(crate::game::feedback::hits(&game.world).iter().any(|(to, hit)| *to == shooter && hit.role == HIT_DEALT && hit.target == r.id), "the shooter sees the hit");

        let mut game = self::game();
        let r = rooster(&game);
        let p = r.transform.position;
        let at = [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2] + 3.0];
        game.world.launch(Flying { kind: PROJECTILE_BULLET, owner: shooter, source: 0, position: at, velocity: [0.0, 0.0, -300.0], ttl: 30, item: ITEM_AK5, back: 0, flown: 0.0 });
        game.step();
        assert!(rooster(&game).health.hp < ROOSTER_HP, "the bullet met it on its first step");
        assert_eq!(crate::mechanics::projectiles::count(&game.world.entities, None), 0, "and is spent");
    }

    #[test]
    fn a_rocket_bursts_on_a_rooster_in_its_path() {
        use crate::mechanics::projectiles::Flying;
        use shared::protocol::PROJECTILE_ROCKET;
        let mut game = game();
        let p = rooster(&game).transform.position;
        // Level with its middle, over flat ground: without the rooster in the way it flies on.
        let at = [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2] + 6.0];
        game.world.launch(Flying { kind: PROJECTILE_ROCKET, owner: 999, source: 0, position: at, velocity: [0.0, 0.0, -30.0], ttl: 120, item: 0, back: 0, flown: 0.0 });
        for _ in 0..30 {
            game.step();
        }
        assert!(rooster(&game).health.hp < ROOSTER_HP, "the rocket burst on it");
    }

    #[test]
    fn a_car_driven_through_a_rooster_runs_it_down() {
        use shared::protocol::KIND_JEEP;
        let mut game = game();
        let p = rooster(&game).transform.position;
        let (x, z) = (p[0], p[2] + 10.0);
        let y = game.world.map.ground.height_at(x, z) + 1.0;
        let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_JEEP, [x, y, z], 0.0);
        // Yaw 0 faces -Z, at the rooster.
        game.world.physics.set_car_velocity(vid, [0.0, 0.0, -20.0]);
        for _ in 0..60 {
            game.step();
        }
        assert!(rooster(&game).health.hp < ROOSTER_HP, "run down at speed");
    }

    #[test]
    fn a_proximity_fuse_and_a_blasts_hit_marker_count_a_rooster_as_they_do_a_stickman() {
        use crate::game::TEAM_BOTS;
        use crate::mechanics::combat::{Blast, blast};
        use crate::mechanics::life::hurtbox_near;
        use shared::protocol::{HIT_DEALT, WEAPON_GRENADE};
        let mut game = game();
        let r = rooster(&game);
        let p = r.transform.position;
        let centre = [p[0], p[1] + ROOSTER_CAPSULE_CENTRE, p[2]];
        assert!(hurtbox_near(&game.world, [centre[0] + 0.8, centre[1], centre[2]], 1.0));
        assert!(!hurtbox_near(&game.world, [centre[0] + 3.0, centre[1], centre[2]], 1.0));

        let bot = game.add_bot([p[0] + 30.0, p[2]], TEAM_BOTS, Vec::new()).unwrap();
        blast(&mut game.world, Blast { point: centre, radius: 3.0, damage: 10.0, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: Some(bot), weapon: WEAPON_GRENADE });
        assert!(rooster(&game).health.hp < ROOSTER_HP);
        assert!(crate::game::feedback::hits(&game.world).iter().any(|(to, hit)| *to == bot && hit.role == HIT_DEALT && hit.target == r.id), "whoever set it off sees the hit");
    }

    #[test]
    fn killed_again_it_leaves_only_one_egg_lying_about() {
        let mut game = game();
        let id = rooster(&game).id;
        game.world.clear_pickups();
        for _ in 0..3 {
            assert!(damage_entity(&mut game.world, id, ROOSTER_HP, None, 0, None));
            crate::mechanics::life::place_at_home(&mut game.world, id);
        }
        let eggs = game.world.pickup_ids().into_iter().filter(|&pid| game.world.pickup(pid).is_some_and(|p| p.item == ITEM_GOLDEN_EGG)).count();
        assert_eq!(eggs, 1);
    }

    #[test]
    fn the_egg_is_never_scattered() {
        assert!(def(ITEM_GOLDEN_EGG).is_some_and(|d| d.weight == 0));
    }
}
