//! Everything that flies: rockets, shells, bombs, grenades, eggs, flares and rifle rounds, as ECS
//! entities made of what they do rather than which they are.
//!
//! - How it moves: `Gravity`, a rocket `Motor`, a flare's `Sinks`.
//! - What meeting something does, one of: `Strikes` (bursts on what it meets, a shell through the
//!   armor it meets first), `Bounces` (a grenade or an egg), `Sticks` (a flare) or `Round` (a gun's
//!   bullet, which hurts what it meets).
//! - What bursting does: `Burst`, absent for what only goes out.
//! - When it moves: at the end of `Combat`'s tick, the tick it is thrown or fired by hand, or with
//!   `GunStage` at the start of `Gunnery`'s, before the vehicle guns fire.
//!
//! `Projectile::kind` (`PROJECTILE_*`) is what the client draws; nothing here branches on it. A new
//! kind is a new prefab in `prefab`.

use shared::vec3;
use super::armor::{self, Outcome};
use super::combat::{Blast, Direct, Flight, Round as Damage, blast_at, land, strike};
use crate::entities::{Component, Transform, With};
use crate::game::{Player, World};
use shared::armor::Pen;
use shared::consts::*;
use shared::items::{FLARE_DRAG, FLARE_RADIUS, FLARE_SINK, Falloff, GRENADE_BLAST_RADIUS, GRENADE_BOUNCE, GRENADE_DAMAGE, GRENADE_KNOCK, GRENADE_RADIUS, def};
use shared::protocol::{PROJECTILE_BOMB, PROJECTILE_BULLET, PROJECTILE_CANNON, PROJECTILE_EGG, PROJECTILE_FLARE, PROJECTILE_GRENADE, PROJECTILE_HE, PROJECTILE_MORTAR, PROJECTILE_ROCKET, PROJECTILE_SHELL, ShotEvent, WEAPON_AP, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_BULLET, WEAPON_EGG, WEAPON_GRENADE, WEAPON_MORTAR, WEAPON_RIFLE, WEAPON_ROCKET, WEAPON_SHELL};
use shared::quant::to_cm;
use std::collections::BTreeMap;

/// What every projectile has: its kind as the client draws it, who fired it (credited with what it
/// does), the vehicle it left (which it flies through; 0 for none) and the ticks it has left.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub kind: u8,
    pub owner: u32,
    pub source: u32,
    pub ttl: u32,
}

/// m/s.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub [f32; 3]);

/// Falls, at this share of `GRAVITY`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Gravity(pub f32);

/// A rocket motor: pushes along the heading by `accel` m/s² up to `top` m/s.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Motor {
    pub accel: f32,
    pub top: f32,
}

/// Coming down, it is held back: horizontal speed fades at `drag` per second and the fall is at most
/// `max_sink` m/s. A flare under its little chute.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Sinks {
    pub drag: f32,
    pub max_sink: f32,
}

/// Bursts on the first thing it meets, or where it is when its time runs out. `bodies`: soldiers and
/// creatures in its path stop it too. `armor`: a vehicle it meets first judges it (`armor::armor_of`), and may turn
/// it away. `lost_below`: under `KILL_Y` it is simply gone.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Strikes {
    pub bodies: bool,
    pub armor: bool,
    pub lost_below: bool,
}

/// Bounces off what it meets, keeping `keep` of its speed, as a ball of `radius` metres; bursts when
/// its time runs out or it falls out of the world, or at once if it lands faster than `smash` m/s.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Bounces {
    pub radius: f32,
    pub keep: f32,
    pub smash: Option<f32>,
}

/// Stops where it meets something, as a ball of `radius` metres, and stays there until it goes out.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Sticks {
    pub radius: f32,
}

/// A gun's round: which gun (`ITEM_*`, for its damage and falloff), how many ticks back its shooter saw
/// the world (so it meets soldiers where they were in the picture it was aimed in), and how far it has
/// flown, which its damage falls off with.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Round {
    pub item: u8,
    pub back: u32,
    pub flown: f32,
}

/// What bursting does: a blast of `radius` metres for `damage` and `knock`, shown and credited as
/// `weapon`. `upward` throws things up rather than along the way it was going (a grenade's).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub weapon: u8,
    pub radius: f32,
    pub damage: f32,
    pub knock: f32,
    pub upward: bool,
    /// The crater it digs where it bursts on the ground: radius and depth, metres; zero for none.
    pub crater: [f32; 2],
}

/// Moves at the start of `Gunnery`'s tick rather than at the end of `Combat`'s.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GunStage;

/// Which projectiles a pass moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Those with `GunStage`, at the start of `Gunnery`'s tick.
    Guns,
    /// The rest, at the end of `Combat`'s tick.
    Hands,
}

pub fn register(ecs: &mut bevy_ecs::world::World) {
    ecs.register_component::<Projectile>();
    ecs.register_component::<Velocity>();
    ecs.register_component::<Gravity>();
    ecs.register_component::<Motor>();
    ecs.register_component::<Sinks>();
    ecs.register_component::<Strikes>();
    ecs.register_component::<Bounces>();
    ecs.register_component::<Sticks>();
    ecs.register_component::<Round>();
    ecs.register_component::<Burst>();
    ecs.register_component::<GunStage>();
}

/// A projectile as it is launched, and as it is seen in flight (`World::in_flight`). `back` and `flown`
/// are a round's (`PROJECTILE_BULLET`), zero for anything else; so is `item` once it flies, though a
/// launcher's rocket is launched with its item, whose damage its blast takes (`burst_for`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flying {
    pub kind: u8,
    pub owner: u32,
    pub source: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub ttl: u32,
    pub item: u8,
    pub back: u32,
    pub flown: f32,
}

impl Flying {
    /// A projectile of `kind` that `owner` set off from `position`, going for `ttl` ticks, from no
    /// vehicle, of no item and with nothing rewound or flown yet.
    pub fn new(kind: u8, owner: u32, position: [f32; 3], velocity: [f32; 3], ttl: u32) -> Self {
        Flying { kind, owner, source: 0, position, velocity, ttl, item: 0, back: 0, flown: 0.0 }
    }

    /// The same, fired from aboard vehicle `source`.
    pub fn aboard(self, source: u32) -> Self {
        Flying { source, ..self }
    }

    /// The same, of `item`.
    pub fn of(self, item: u8) -> Self {
        Flying { item, ..self }
    }

    /// The same, aimed at the world as it was `back` ticks ago.
    pub fn rewound(self, back: u32) -> Self {
        Flying { back, ..self }
    }
}

const fn blast(weapon: u8, radius: f32, damage: f32, knock: f32) -> Burst {
    Burst { weapon, radius, damage, knock, upward: false, crater: [0.0, 0.0] }
}

/// What a projectile of `kind` does when it bursts; `None` for one that only goes out.
pub fn burst_of(kind: u8) -> Option<Burst> {
    Some(match kind {
        PROJECTILE_ROCKET => Burst { crater: [CRATER_ROCKET_RADIUS, CRATER_ROCKET_DEPTH], ..blast(WEAPON_ROCKET, ROCKET_BLAST_RADIUS, ROCKET_DAMAGE, ROCKET_KNOCK) },
        PROJECTILE_SHELL => Burst { crater: [CRATER_AP_RADIUS, CRATER_AP_DEPTH], ..blast(WEAPON_AP, AP_BLAST_RADIUS, AP_BLAST_DAMAGE, AP_BLAST_KNOCK) },
        PROJECTILE_HE => Burst { crater: [CRATER_SHELL_RADIUS, CRATER_SHELL_DEPTH], ..blast(WEAPON_SHELL, EXPLOSION_RADIUS, EXPLOSION_DAMAGE, EXPLOSION_KNOCK) },
        PROJECTILE_CANNON => Burst { crater: [CRATER_CANNON_RADIUS, CRATER_CANNON_DEPTH], ..blast(WEAPON_AUTOCANNON, CANNON_RADIUS, CANNON_DAMAGE, CANNON_KNOCK) },
        PROJECTILE_BOMB => Burst { crater: [CRATER_BOMB_RADIUS, CRATER_BOMB_DEPTH], ..blast(WEAPON_BOMB, BOMB_RADIUS, BOMB_DAMAGE, BOMB_KNOCK) },
        PROJECTILE_MORTAR => Burst { crater: [CRATER_MORTAR_RADIUS, CRATER_MORTAR_DEPTH], ..blast(WEAPON_MORTAR, MORTAR_RADIUS, MORTAR_DAMAGE, MORTAR_KNOCK) },
        PROJECTILE_GRENADE => Burst { upward: true, crater: [CRATER_GRENADE_RADIUS, CRATER_GRENADE_DEPTH], ..blast(WEAPON_GRENADE, GRENADE_BLAST_RADIUS, GRENADE_DAMAGE, GRENADE_KNOCK) },
        PROJECTILE_EGG => Burst { upward: true, crater: [CRATER_EGG_RADIUS, CRATER_EGG_DEPTH], ..blast(WEAPON_EGG, HEN_EGG_BLAST_RADIUS, HEN_EGG_DAMAGE, HEN_EGG_KNOCK) },
        _ => return None,
    })
}

/// What a projectile of `kind` fired from `item` (or 0) does when it bursts: `burst_of`'s, with the
/// item's own damage when it has one (a Carl Gustaf's rocket hits harder than a helicopter's).
pub fn burst_for(kind: u8, item: u8) -> Option<Burst> {
    let burst = burst_of(kind)?;
    Some(match def(item) {
        Some(d) if d.damage > 0 => Burst { damage: f32::from(d.damage), ..burst },
        _ => burst,
    })
}

/// How much armour a projectile of `kind` goes through when it strikes (`Strikes::armor`): a gun's
/// armour-piercing shell, with a small charge that bursts behind the plate it gets through, and high
/// explosive, which goes through little but bursts big.
pub fn pen_of(kind: u8) -> Pen {
    match kind {
        PROJECTILE_SHELL => Pen::ap(90.0, 90.0, 1.0, 75.0),
        PROJECTILE_HE => Pen::he(15.0, 75.0),
        PROJECTILE_CANNON => Pen::he(20.0, 20.0),
        PROJECTILE_BOMB => Pen::he(60.0, 250.0),
        PROJECTILE_MORTAR => Pen::he(15.0, 81.0),
        _ => Pen::NONE,
    }
}

/// A vehicle gun's shell, bomb or mortar round: falls, and bursts on what it meets, through its armor.
const SHELL: Strikes = Strikes { bodies: true, armor: true, lost_below: true };

/// How a projectile of `kind` flies and what meeting something does, beyond where it is and how fast
/// it goes. Its `Burst` is `burst_of`'s.
fn prefab(kind: u8, round: Round, entity: &mut bevy_ecs::world::EntityWorldMut) {
    let burst = burst_for(kind, round.item);
    match kind {
        PROJECTILE_ROCKET => {
            entity.insert((Motor { accel: ROCKET_ACCEL, top: ROCKET_MAX_SPEED }, Strikes { bodies: true, armor: false, lost_below: false }));
        }
        PROJECTILE_SHELL | PROJECTILE_HE | PROJECTILE_CANNON | PROJECTILE_BOMB | PROJECTILE_MORTAR => {
            entity.insert((Gravity(1.0), SHELL, GunStage));
        }
        PROJECTILE_GRENADE => {
            entity.insert((Gravity(1.0), Bounces { radius: GRENADE_RADIUS, keep: GRENADE_BOUNCE, smash: None }));
        }
        PROJECTILE_EGG => {
            entity.insert((Gravity(1.0), Bounces { radius: HEN_EGG_RADIUS, keep: HEN_EGG_BOUNCE, smash: Some(HEN_EGG_SMASH_SPEED) }));
        }
        PROJECTILE_FLARE => {
            entity.insert((Gravity(1.0), Sinks { drag: FLARE_DRAG, max_sink: FLARE_SINK }, Sticks { radius: FLARE_RADIUS }));
        }
        PROJECTILE_BULLET => {
            entity.insert((Gravity(BULLET_GRAVITY_SCALE), round));
        }
        _ => tracing::warn!(kind, "a projectile of no known kind does nothing"),
    }
    if let Some(burst) = burst {
        entity.insert(burst);
    }
}

/// Sets off what a projectile of `kind` from `owner`'s `item` (or 0) does when it bursts, at `point`
/// heading along `dir`, without it ever flying: one fired into a wall it starts against.
pub fn burst_at(world: &mut World, kind: u8, item: u8, owner: u32, point: [f32; 3], dir: [f32; 3]) {
    if let Some(burst) = burst_for(kind, item) {
        set_off(world, burst, owner, point, dir, None);
    }
}

impl World {
    /// Launches `flying` under a new id, which it returns.
    pub fn launch(&mut self, flying: Flying) -> u32 {
        let id = self.alloc_id();
        self.launch_as(id, flying);
        id
    }

    /// Launches `flying` under id `id` (from `alloc_id`).
    pub fn launch_as(&mut self, id: u32, flying: Flying) {
        let Flying { kind, owner, source, position, velocity, ttl, item, back, flown } = flying;
        let entity = self.entities.spawn(id, (Projectile { kind, owner, source, ttl }, Transform { position, yaw: 0.0 }, Velocity(velocity)));
        prefab(kind, Round { item, back, flown }, &mut self.entities.ecs.entity_mut(entity));
    }

    /// Every projectile in flight, by id, as it is now.
    pub fn in_flight(&self) -> BTreeMap<u32, Flying> {
        self.entities.ids::<With<Projectile>>().into_iter().filter_map(|id| Some((id, self.flying(id)?))).collect()
    }

    /// Projectile `id` as it is now.
    pub fn flying(&self, id: u32) -> Option<Flying> {
        let entities = &self.entities;
        let Projectile { kind, owner, source, ttl } = *entities.get::<Projectile>(id)?;
        let position = entities.get::<Transform>(id)?.position;
        let velocity = entities.get::<Velocity>(id)?.0;
        let Round { item, back, flown } = entities.get::<Round>(id).copied().unwrap_or(Round { item: 0, back: 0, flown: 0.0 });
        Some(Flying { kind, owner, source, position, velocity, ttl, item, back, flown })
    }

}

/// How many projectiles are in flight; `kind` counts only those of one kind. On the entities alone,
/// so it can be asked while a player is borrowed.
pub fn count(entities: &crate::entities::Entities, kind: Option<u8>) -> usize {
    entities.scan::<&Projectile, (), _>(|flying| flying.filter(|p| kind.is_none_or(|kind| p.kind == kind)).count()).unwrap_or(0)
}

/// Moves every projectile of `stage` one tick, in id order, and carries out what each meets.
pub fn step(world: &mut World, dt: f32, stage: Stage) {
    let ids = match stage {
        Stage::Guns => world.entities.ids::<(With<Projectile>, With<GunStage>)>(),
        Stage::Hands => world.entities.ids::<(With<Projectile>, bevy_ecs::query::Without<GunStage>)>(),
    };
    for id in ids {
        let entities = &world.entities;
        if entities.get::<Round>(id).is_some() {
            step_round(world, id, dt);
        } else if let Some(bounces) = entities.get::<Bounces>(id).copied() {
            step_bouncing(world, id, dt, bounces);
        } else if let Some(sticks) = entities.get::<Sticks>(id).copied() {
            step_sticking(world, id, dt, sticks);
        } else if let Some(strikes) = entities.get::<Strikes>(id).copied() {
            step_striking(world, id, dt, strikes);
        }
    }
}

/// What a step works on: the parts of a projectile that change in flight.
struct State {
    owner: u32,
    source: u32,
    ttl: u32,
    position: [f32; 3],
    velocity: [f32; 3],
}

fn read(world: &World, id: u32) -> Option<State> {
    let p = *world.entities.get::<Projectile>(id)?;
    Some(State { owner: p.owner, source: p.source, ttl: p.ttl, position: world.entities.get::<Transform>(id)?.position, velocity: world.entities.get::<Velocity>(id)?.0 })
}

fn write(world: &mut World, id: u32, state: &State) {
    if let Some(mut p) = world.entities.get_mut::<Projectile>(id) {
        p.ttl = state.ttl;
    }
    if let Some(mut t) = world.entities.get_mut::<Transform>(id) {
        t.position = state.position;
    }
    if let Some(mut v) = world.entities.get_mut::<Velocity>(id) {
        v.0 = state.velocity;
    }
}

/// One tick of what moves projectile `id`, before anything is met: its fall, its drag, its motor.
fn fly(world: &World, id: u32, state: &mut State, dt: f32) {
    if let Some(Gravity(scale)) = world.entities.get::<Gravity>(id) {
        state.velocity[1] -= GRAVITY * scale * dt;
    }
    if let Some(sinks) = world.entities.get::<Sinks>(id)
        && state.velocity[1] < 0.0
    {
        let keep = (-sinks.drag * dt).exp();
        state.velocity[0] *= keep;
        state.velocity[2] *= keep;
        state.velocity[1] = state.velocity[1].max(-sinks.max_sink);
    }
    if let Some(motor) = world.entities.get::<Motor>(id) {
        let speed = length(state.velocity);
        let dir = state.velocity.map(|v| v / speed);
        let speed = (speed + motor.accel * dt).min(motor.top.max(speed));
        state.velocity = dir.map(|v| v * speed);
    }
}

fn length(v: [f32; 3]) -> f32 {
    vec3::length(v)
}

/// Takes projectile `id` out of the world and, if it has a `Burst`, sets it off at `point`, heading
/// along `dir`, with `direct` the vehicle it struck square on, if any.
fn burst(world: &mut World, id: u32, owner: u32, point: [f32; 3], dir: [f32; 3], direct: Option<Direct>) {
    let burst = world.entities.get::<Burst>(id).copied();
    world.entities.despawn(id);
    if let Some(burst) = burst {
        set_off(world, burst, owner, point, dir, direct);
    }
}

/// `burst`'s blast, shown and credited.
fn set_off(world: &mut World, burst: Burst, owner: u32, point: [f32; 3], dir: [f32; 3], direct: Option<Direct>) {
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent { shooter: owner, weapon: burst.weapon, x, y, z });
    // A dead or departed owner still gets the blast, just not the credit.
    let attacker = world.entities.has::<Player>(owner).then_some(owner);
    let dir = if burst.upward { [0.0, 1.0, 0.0] } else { dir };
    blast_at(world, Blast { point, radius: burst.radius, damage: burst.damage, knock: burst.knock, dir, attacker, weapon: burst.weapon }, direct);
    // After the blast, so a building it bit has stood on the old ground when it fell.
    world.dig_crater(point, burst.crater[0], burst.crater[1]);
}

/// A rocket or a shell: flies on, and bursts on the first wall, vehicle or (with `bodies`) soldier it
/// meets. With `armor`, a vehicle met first may turn it away instead.
fn step_striking(world: &mut World, id: u32, dt: f32, strikes: Strikes) {
    let Some(mut s) = read(world, id) else { return };
    fly(world, id, &mut s, dt);
    let speed = length(s.velocity);
    let dir = s.velocity.map(|v| v / speed);
    let step = speed * dt;
    let hit = world.physics.cast_ray_hit(s.position, dir, step, s.source);
    let wall = hit.map(|(t, _)| t);
    // Soldiers and creatures are not physics bodies, so test the path against each one.
    let body = if strikes.bodies { super::life::first_hurtbox_hit(world, s.position, dir, wall.unwrap_or(step), Some(s.owner), 0, None) } else { None };
    s.ttl = s.ttl.saturating_sub(1);
    if let Some(t) = body.map(|(_, t)| t).or(wall) {
        let point: [f32; 3] = vec3::along(s.position, dir, t);
        // A solid shot through a person is the end of them, whatever its little charge does.
        if let Some((victim, _)) = body
            && pen_of(world.entities.get::<Projectile>(id).map_or(0, |p| p.kind)).kind == shared::armor::PenKind::Ap
        {
            let attacker = world.entities.has::<Player>(s.owner).then_some(s.owner);
            let weapon = world.entities.get::<Burst>(id).map_or(WEAPON_SHELL, |burst| burst.weapon);
            super::life::damage_entity(world, victim, super::life::Harm::new(PLAYER_MAX_HP, attacker, weapon).from(point));
        }
        // A vehicle was hit first: its armor decides whether it bursts on it or glances off.
        let mut direct = None;
        if let (true, None, Some((_, Some(vid)))) = (strikes.armor, body, hit) {
            let pen = pen_of(world.entities.get::<Projectile>(id).map_or(0, |p| p.kind));
            let attacker = world.entities.has::<Player>(s.owner).then_some(s.owner);
            match armor::armor_of(world, vid, s.position, dir, step, pen.near) {
                Some(outcome @ Outcome::Ricochet { normal }) => {
                    let normal = world.physics.car_pose(vid).map_or(normal, |pose| pose.world_dir(normal));
                    armor::report(world, vid, &outcome, point, dir, pen.calibre, armor::Inside::default(), attacker);
                    let out = armor::reflect(dir, normal);
                    s.velocity = out.map(|v| v * speed * armor::RICOCHET_KEEP);
                    // Just clear of the plate, so the next step does not hit it again.
                    s.position = vec3::along(point, normal, 0.1);
                    write(world, id, &s);
                    return;
                }
                Some(outcome) => {
                    let burst = world.entities.get::<Burst>(id).copied();
                    let (damage, weapon) = burst.map_or((0.0, WEAPON_SHELL), |burst| (burst.damage, burst.weapon));
                    // It bursts on the plate or behind it: a track there is hurt either way...
                    let mut done = outcome.at().map_or_else(armor::Inside::default, |at| armor::on_outside(world, vid, at, pen.calibre, damage, attacker));
                    // ...and one that went through goes on into what is inside.
                    if let (Outcome::Pierced { residual, slit, at, .. }, Some(pose)) = (outcome, world.physics.car_pose(vid)) {
                        let entry = armor::Entry { at, dir: pose.local_dir(dir), residual, calibre: pen.calibre, kind: pen.kind, slit, crew: PLAYER_MAX_HP, weapon, attacker };
                        done.add(armor::inside(world, vid, entry));
                    }
                    armor::report(world, vid, &outcome, point, dir, pen.calibre, done, attacker);
                    direct = Some(Direct { vehicle: vid, scale: outcome.share(pen.kind), turret: outcome.turret() });
                }
                None => {}
            }
        }
        burst(world, id, s.owner, point, dir, direct);
        return;
    }
    s.position = vec3::along(s.position, dir, step);
    if strikes.lost_below && s.position[1] < KILL_Y {
        world.entities.despawn(id);
    } else if s.ttl == 0 {
        burst(world, id, s.owner, s.position, dir, None);
    } else {
        write(world, id, &s);
    }
}

/// A grenade or an egg: falls and bounces, and bursts when its fuse runs out (or an egg on landing hard).
fn step_bouncing(world: &mut World, id: u32, dt: f32, bounces: Bounces) {
    let Some(mut s) = read(world, id) else { return };
    let mut smashed = false;
    s.ttl = s.ttl.saturating_sub(1);
    fly(world, id, &mut s, dt);
    let mut remaining = dt;
    // It can meet a surface more than once in a tick in a corner; three is plenty.
    for _ in 0..3 {
        let speed = length(s.velocity);
        if speed < 1e-3 {
            break;
        }
        let dir = s.velocity.map(|v| v / speed);
        let step = speed * remaining;
        match world.physics.cast_ray_normal(s.position, dir, step + bounces.radius) {
            Some((t, normal)) => {
                let travel = (t - bounces.radius).clamp(0.0, step);
                s.position = vec3::along(s.position, dir, travel);
                // Reflect, keeping some of the speed, and rolling to rest on a floor.
                let along: f32 = vec3::dot(s.velocity, normal);
                if bounces.smash.is_some_and(|smash| -along > smash) {
                    smashed = true;
                    break;
                }
                s.velocity = std::array::from_fn(|i| (s.velocity[i] - 2.0 * along * normal[i]) * bounces.keep);
                if s.velocity.iter().map(|v| v * v).sum::<f32>() < 0.5 {
                    s.velocity = [0.0; 3];
                }
                remaining *= 1.0 - travel / step.max(1e-6);
            }
            None => {
                s.position = vec3::along(s.position, dir, step);
                break;
            }
        }
    }
    if smashed || s.ttl == 0 || s.position[1] < KILL_Y {
        burst(world, id, s.owner, s.position, [0.0, 1.0, 0.0], None);
    } else {
        write(world, id, &s);
    }
}

/// A flare: rises, then sinks under its chute, and burns where it comes to rest until it goes out.
fn step_sticking(world: &mut World, id: u32, dt: f32, sticks: Sticks) {
    let Some(mut s) = read(world, id) else { return };
    s.ttl = s.ttl.saturating_sub(1);
    // Once it has come to rest it stays there.
    if s.velocity != [0.0; 3] {
        fly(world, id, &mut s, dt);
        let speed = length(s.velocity);
        let dir = s.velocity.map(|v| v / speed);
        let step = speed * dt;
        match world.physics.cast_ray(s.position, dir, step + sticks.radius, 0) {
            Some(t) => {
                s.position = vec3::along(s.position, dir, (t - sticks.radius).clamp(0.0, step));
                s.velocity = [0.0; 3];
            }
            None => s.position = vec3::along(s.position, dir, step),
        }
    }
    if s.ttl == 0 || s.position[1] < KILL_Y {
        burst(world, id, s.owner, s.position, [0.0, 1.0, 0.0], None);
    } else {
        write(world, id, &s);
    }
}

/// A gun's round: flies and drops, and hurts the first soldier, creature, vehicle, building or tree it
/// meets, by its gun's damage less what the distance it has flown takes off.
fn step_round(world: &mut World, id: u32, dt: f32) {
    let (Some(mut s), Some(round)) = (read(world, id), world.entities.get::<Round>(id).copied()) else { return };
    let item = def(round.item).copied();
    fly(world, id, &mut s, dt);
    let speed = length(s.velocity);
    let dir = s.velocity.map(|v| v / speed);
    let step = speed * dt;
    // A passenger's round leaves through their own vehicle (`source`) rather than stopping in it.
    let wall = world.physics.cast_ray_hit(s.position, dir, step, s.source);
    let (base, falloff, weapon, pen) = item.map_or((0, Falloff::NONE, WEAPON_RIFLE, Pen::NONE), |d| (d.damage, d.falloff, d.weapon, d.pen));
    // What it does to a vehicle is a chip of what it does to a soldier, a plane no different.
    let chip = (base / 4).max(1);
    let damage = Damage { damage: base, vehicle: chip, plane: chip, weapon, range: step, falloff, pen };
    let attacker = world.entities.has::<Player>(s.owner).then_some(s.owner);
    let flight = Flight { origin: s.position, dir, shooter: Some(s.owner), attacker, back: round.back, flown: round.flown };
    let met = strike(world, &flight, wall, step);
    s.ttl = s.ttl.saturating_sub(1);
    if met.any() {
        world.entities.despawn(id);
        land(world, &flight, &met, &damage);
        let point: [f32; 3] = vec3::along(s.position, dir, met.end);
        let [x, y, z] = point.map(to_cm);
        world.shots.push(ShotEvent { shooter: id, weapon: WEAPON_BULLET, x, y, z });
        return;
    }
    s.position = vec3::along(s.position, dir, step);
    if s.ttl == 0 || s.position[1] < KILL_Y {
        world.entities.despawn(id);
    } else {
        write(world, id, &s);
        if let Some(mut r) = world.entities.get_mut::<Round>(id) {
            r.flown += step;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use shared::world::{WORLD_VERSION, WorldFile};

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        game.world.clear_pickups();
        game
    }

    fn launch(game: &mut Game, kind: u8, position: [f32; 3], velocity: [f32; 3]) -> u32 {
        game.world.launch(Flying::new(kind, 0, position, velocity, 600))
    }

    #[test]
    fn everything_explosive_digs_a_crater_inside_its_blast() {
        for kind in [PROJECTILE_ROCKET, PROJECTILE_SHELL, PROJECTILE_CANNON, PROJECTILE_BOMB, PROJECTILE_MORTAR, PROJECTILE_GRENADE, PROJECTILE_EGG] {
            let [radius, depth] = burst_of(kind).unwrap().crater;
            assert!(radius > 0.0 && depth > 0.0 && depth < radius, "kind {kind}");
        }
        assert!(burst_of(PROJECTILE_BULLET).is_none());
    }

    #[test]
    fn every_kind_does_exactly_one_thing_on_meeting_something_and_moves_at_its_stage() {
        let mut game = game();
        let kinds = [PROJECTILE_ROCKET, PROJECTILE_GRENADE, PROJECTILE_SHELL, PROJECTILE_CANNON, PROJECTILE_BULLET, PROJECTILE_BOMB, PROJECTILE_MORTAR, PROJECTILE_EGG, PROJECTILE_FLARE];
        for kind in kinds {
            let id = launch(&mut game, kind, [0.0, 50.0, 0.0], [0.0, 0.0, -10.0]);
            let e = &game.world.entities;
            let contacts = [e.get::<Strikes>(id).is_some(), e.get::<Bounces>(id).is_some(), e.get::<Sticks>(id).is_some(), e.get::<Round>(id).is_some()];
            assert_eq!(contacts.iter().filter(|&&c| c).count(), 1, "kind {kind}: {contacts:?}");
            let guns = matches!(kind, PROJECTILE_SHELL | PROJECTILE_CANNON | PROJECTILE_BOMB | PROJECTILE_MORTAR);
            assert_eq!(e.get::<GunStage>(id).is_some(), guns, "kind {kind}");
            assert_eq!(e.get::<Burst>(id).copied(), burst_of(kind), "kind {kind}");
        }
        assert_eq!(count(&game.world.entities, None), kinds.len());
        assert_eq!(count(&game.world.entities, Some(PROJECTILE_FLARE)), 1);
    }

    #[test]
    fn a_projectile_is_what_it_is_made_of() {
        // A thing no item fires: a bouncing ball with a rocket motor that bursts like a grenade. It
        // needs no code of its own, only the parts.
        let mut game = game();
        let id = game.world.alloc_id();
        game.world.entities.spawn(
            id,
            (
                Projectile { kind: PROJECTILE_GRENADE, owner: 0, source: 0, ttl: 90 },
                Transform { position: [0.0, 1.0, 0.0], yaw: 0.0 },
                Velocity([0.0, 0.0, -5.0]),
                Gravity(1.0),
                Motor { accel: 20.0, top: 30.0 },
                Bounces { radius: 0.1, keep: 0.5, smash: None },
                burst_of(PROJECTILE_GRENADE).unwrap(),
            ),
        );
        for _ in 0..30 {
            step(&mut game.world, TICK_DT, Stage::Hands);
        }
        let flying = game.world.flying(id).expect("still going");
        assert!(flying.velocity[2] < -5.0 - 1.0, "its motor speeds it up: {:?}", flying.velocity);
        assert!(flying.position[1] > 0.0 && flying.position[1] < 3.0, "it bounces along the ground: {:?}", flying.position);
        for _ in 0..60 {
            step(&mut game.world, TICK_DT, Stage::Hands);
        }
        assert!(game.world.flying(id).is_none(), "its time ran out and it burst");
        assert!(game.world.shots.iter().any(|shot| shot.weapon == shared::protocol::WEAPON_GRENADE));
    }

    #[test]
    fn a_shell_waits_for_the_guns_and_a_grenade_for_the_hands() {
        let mut game = game();
        let shell = launch(&mut game, PROJECTILE_SHELL, [0.0, 50.0, 0.0], [0.0, 0.0, -100.0]);
        let grenade = launch(&mut game, PROJECTILE_GRENADE, [10.0, 50.0, 0.0], [0.0, 0.0, -10.0]);
        step(&mut game.world, TICK_DT, Stage::Hands);
        assert_eq!(game.world.flying(shell).unwrap().position, [0.0, 50.0, 0.0]);
        assert_ne!(game.world.flying(grenade).unwrap().position, [10.0, 50.0, 0.0]);
        step(&mut game.world, TICK_DT, Stage::Guns);
        assert_ne!(game.world.flying(shell).unwrap().position, [0.0, 50.0, 0.0]);
    }
}
