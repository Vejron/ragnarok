//! The state the game loop owns: players, vehicles, rockets, the ECS entities and the `World` that holds them,
//! and the `Command`/`Outgoing` messages the sessions exchange with it.

use shared::armor::PARTS;
use super::*;
use bevy_ecs::message::{Message, Messages};
use shared::mounts::{MountDef, Show};
pub use crate::profile::Profile;

pub enum Command {
    /// Admit a player with their chosen settings. Replies with its entity id, or `None` when the
    /// room is full.
    Join { outbox: mpsc::Sender<Outgoing>, reply: oneshot::Sender<Option<u32>>, profile: Profile },
    Leave(u32),
    /// Input frames, with the tick of the newest snapshot the client says it holds.
    Input(u32, u32, Vec<InputFrame>),
    /// Voice signaling from the first player for the second, relayed as is.
    Rtc { from: u32, to: u32, kind: u8, json: String },
}

#[derive(Clone, Debug)]
pub enum Outgoing {
    /// Unreliable and unordered: snapshots.
    Datagram(Bytes),
    /// Reliable, each on its own unidirectional stream: welcome, leave.
    Reliable(Bytes),
}

/// Where a seated player sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub vehicle: u32,
    /// 0 is the driver.
    pub index: usize,
    /// Flown from afar (a drone): the body stays where it stood, frozen but in the world.
    pub remote: bool,
}

/// What a player holds at the start of every life: the AK5 and the sniper rifle with full magazines,
/// a flare gun with a full stack of flares, an FPV drone and bricks to build with. The loitering drone
/// is found lying about.
pub fn starting_hotbar() -> [ItemSlot; HOTBAR_SLOTS] {
    let full = |item: u8| ItemSlot { item, count: def(item).map_or(1, |d| d.stack), ammo: def(item).map_or(0, |d| d.mag) };
    [full(ITEM_AK5), full(ITEM_SNIPER), full(ITEM_FLARE), full(ITEM_DRONE), full(ITEM_BRICKS)]
}

/// A stickman, a connected player's or a bot's: a component on the player's entity. Passes over players
/// go by `Entities::ids::<With<Player>>()`, in id order as the map they were kept in gave them.
#[derive(crate::entities::Component)]
pub struct Player {
    pub character: Character,
    /// While seated the stickman is hidden and rides with the vehicle; input drives the seat.
    pub seat: Option<Seat>,
    /// Ticks until a car can hit this player again.
    pub hit_cooldown: u32,
    /// The input applied this tick.
    pub input: InputFrame,
    pub inputs: JitterBuffer,
    /// Newest client input sequence already applied, echoed in snapshots for reconciliation.
    pub ack: u32,
    /// What is in each hotbar slot: `[item, count, ammo]`.
    pub hotbar: [ItemSlot; HOTBAR_SLOTS],
    /// The slot in hand, as of the last tick.
    pub selected: usize,
    /// Ticks until the item in hand can be used again.
    pub fire_cooldown: u32,
    /// Ticks left of a reload, or of a bandage being applied (`channel_left`). Zero when idle.
    pub reload_left: u32,
    pub channel_left: u32,
    /// Whether fire was held last tick, so a semi-automatic gun needs a fresh press.
    pub trigger_held: bool,
    pub kills: u16,
    pub deaths: u16,
    /// Shown to everyone in place of the id.
    pub name: String,
    /// Outfit colour, below `PLAYER_COLOR_COUNT`.
    pub color: u8,
    /// The player's own spawn points `[x, z]`, centimetres, all usable when they joined. Empty means
    /// a random one of the world's.
    pub spawns: Vec<[i32; 2]>,
    /// Which of `spawns` the next spawn uses; counts up forever and wraps by `spawns.len()`.
    pub next_spawn: usize,
    /// `TEAM_NONE` for people, who are all against all. Players on the same other team never hurt
    /// each other.
    pub team: u8,
}

/// Every player for themselves.
pub const TEAM_NONE: u8 = 0;
/// The team bots are on unless placed on another.
pub const TEAM_BOTS: u8 = 1;
/// Bots a room holds, on top of the `MAX_PLAYERS` people: bots never take a person's place.
pub const MAX_BOTS: usize = 64;
/// How good a shot a bot is unless placed with another skill: 0 is a raw recruit, 1 a marksman.
pub const DEFAULT_SKILL: f32 = 0.5;

/// What a bot is up to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotMode {
    /// Walking its route.
    Patrol,
    /// Running at someone it has seen, or at where it last saw them.
    Pursue,
    /// Lost them: looking round where they were last seen.
    Search,
}

/// The mind of a bot. Its body is the `Player` with the same id; `Bots` writes that player's input
/// each tick, so it walks, rides and dies exactly as a person does.
#[derive(crate::entities::Component, Clone, Debug, PartialEq)]
pub struct Bot {
    pub home: [f32; 2],
    /// Waypoints `[x, z]`, walked round in order.
    pub route: Vec<[f32; 2]>,
    /// The waypoint it is walking to.
    pub next: usize,
    pub mode: BotMode,
    /// Ticks left of a search.
    pub mode_left: u32,
    /// Whom it is after, where they were last seen (the body's centre) and on which tick, and how
    /// fast they seemed to be going then, m/s.
    pub target: Option<u32>,
    pub last_seen: [f32; 3],
    pub seen_tick: u32,
    pub velocity: [f32; 3],
    /// Whether the target was in sight at the last look.
    pub sees: bool,
    /// Facing and looking up or down, radians, turned smoothly rather than snapped.
    pub yaw: f32,
    pub pitch: f32,
    /// The nearest it has been to the next corner of its path since it last got closer, and ticks
    /// since then.
    pub best: f32,
    pub stalled: u32,
    /// The corners `[x, z]` still to walk to on its way to `path_to`, the next first, and ticks
    /// until it looks for a better way.
    pub path: Vec<[f32; 2]>,
    pub path_to: [f32; 2],
    pub replan_in: u32,
    /// How good a shot it is, 0 to 1: how quickly it reacts, how close its aim is, how well it leads.
    pub skill: f32,
    /// The hotbar slot it holds.
    pub slot: u8,
    /// Ticks before it may fire on someone it has just seen.
    pub react: u32,
    /// Ticks it has had its target in sight, unbroken: its aim settles as this grows.
    pub tracked: u32,
    /// How far off its aim is, yaw and pitch, radians: drawn afresh at every look.
    pub error: [f32; 2],
    /// Ticks left of holding the trigger down, then of letting it go, for a gun that fires in bursts.
    pub burst: u32,
    pub pause: u32,
    /// Which way it sidesteps while it stands and fights (-1, 0 or 1), and for how many more ticks.
    pub strafe: i8,
    pub strafe_left: u32,
    /// Where the last hit it took came from (the attacker, or a blast's centre), until it has
    /// turned that way.
    pub hit_from: Option<[f32; 3]>,
    /// What it holds at the start of every life, when not the standard loadout.
    pub loadout: Option<[ItemSlot; HOTBAR_SLOTS]>,
    /// The vehicle it crews and its seat there, when it is one of a tank's crew.
    pub crew: Option<Crew>,
    /// A driver backing out of being stuck: ticks left of it, and how often it has tried on the way
    /// to this goal.
    pub reverse_left: u32,
    pub tries: u32,
}

/// A bot's place in a crew: its vehicle, and the seat it works (0 drives, 1 is the gun).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crew {
    pub vehicle: u32,
    pub seat: usize,
}

impl Bot {
    pub fn new(home: [f32; 2], route: Vec<[f32; 2]>) -> Self {
        Self {
            home,
            route,
            next: 0,
            mode: BotMode::Patrol,
            mode_left: 0,
            target: None,
            last_seen: [0.0; 3],
            seen_tick: 0,
            velocity: [0.0; 3],
            sees: false,
            yaw: 0.0,
            pitch: 0.0,
            best: f32::MAX,
            stalled: 0,
            path: Vec::new(),
            path_to: [0.0; 2],
            replan_in: 0,
            skill: DEFAULT_SKILL,
            slot: 0,
            react: 0,
            tracked: 0,
            error: [0.0; 2],
            burst: 0,
            pause: 0,
            strafe: 0,
            strafe_left: 0,
            hit_from: None,
            loadout: None,
            crew: None,
            reverse_left: 0,
            tries: 0,
        }
    }
}

/// Where a hit on a standing stickman is shown: their chest, this far above the capsule's centre.
const CHEST_Y: f32 = 0.9;

/// What a player's entity carries besides the `Player` to live and die by: full health, coming back
/// `RESPAWN_TICKS` after dying, and a body to hit (their capsule, with the head at the top) with a
/// trail of where it was.
pub fn stickman_life() -> impl crate::entities::Bundle {
    use crate::entities::{Health, Hurtbox, Respawn, Trail};
    let body = Hurtbox { centre_y: 0.0, half_height: CAPSULE_HALF_HEIGHT, radius: CAPSULE_RADIUS, head: Some(HEAD_ZONE_Y), mark_y: CHEST_Y, hit_kind: HIT_BODY };
    (Health::full(PLAYER_MAX_HP), Respawn { delay: RESPAWN_TICKS, left: 0 }, body, Trail::default())
}

impl Player {
    /// Whether the stickman is out in the world, to be seen, hit and blasted: on foot, or standing
    /// still while flying a drone.
    pub fn embodied(&self) -> bool {
        self.seat.is_none_or(|seat| seat.remote)
    }

    /// The slot in hand.
    pub fn held(&self) -> ItemSlot {
        self.hotbar[self.selected.min(HOTBAR_SLOTS - 1)]
    }

    /// Takes away anything half done: a reload or a bandage.
    pub fn interrupt(&mut self) {
        self.reload_left = 0;
        self.channel_left = 0;
    }
}

/// Game-side state of a vehicle, an entity's component. Its rigid body lives in `Physics`, under the
/// same id.
#[derive(crate::entities::Component)]
pub struct Vehicle {
    pub kind: u8,
    pub controls: CarControls,
    /// Where it respawns after falling out of the world.
    pub home: ([f32; 3], f32),
    /// Consecutive ticks spent on its side or roof and not moving.
    pub stuck_ticks: u32,
    /// What a tank's turret has left before it comes off (`TANK_TURRET_HP` when whole).
    pub turret_hp: u8,
    /// Crash damage as the snapshot carries it (`DAMAGE_*`); all zeros when unhurt. An armoured
    /// vehicle's parts are in it too (`DAMAGE_PARTS`, kept by `armor::sync_parts`).
    pub damage: [u8; shared::protocol::DAMAGE_SLOTS],
    /// What each of an armoured vehicle's parts has left (`shared::armor::Part::slot`); full when whole.
    pub parts: [u8; PARTS],
    /// Ticks before it may throw sparks again.
    pub scrape_in: u8,
}

impl Vehicle {
    /// What each part of a fresh `kind` has.
    pub fn fresh_parts(kind: u8) -> [u8; PARTS] {
        let armor = &shared::vehicles::vehicle_def(kind).armor;
        std::array::from_fn(|slot| armor.module(slot).map_or(0, |module| module.hp))
    }

    /// Put back as new: no damage, its turret on and every part whole.
    pub fn renew(&mut self) {
        self.damage = [0; shared::protocol::DAMAGE_SLOTS];
        self.turret_hp = shared::consts::TANK_TURRET_HP;
        self.parts = Vehicle::fresh_parts(self.kind);
    }
}

/// A vehicle whose fuel is alight: it loses health until it is a wreck. `ticks` counts how long it has
/// burnt.
#[derive(crate::entities::Component, Clone, Copy, Debug, PartialEq)]
pub struct Burning {
    pub ticks: u32,
}

/// What a drone carries beyond being a vehicle: a component on the vehicle's entity. The `Drones`
/// mechanic owns it.
#[derive(crate::entities::Component, Clone, Copy, Debug, PartialEq)]
pub struct Drone {
    /// `KIND_DRONE` or `KIND_LOITER`, which picks its `shared::vehicles::drone_def`.
    pub kind: u8,
    /// Who launched it, credited with what it blows up.
    pub owner: u32,
    /// Ticks of flight left in the battery.
    pub energy: u32,
    /// Its owner is flying it. Once the link is lost it falls, and goes off on whatever it lands on.
    pub linked: bool,
    /// The owner's health at launch: losing any of it breaks their concentration, and the link.
    pub pilot_hp: u8,
    /// Ticks a falling drone has left before it goes off by itself.
    pub fall_left: u32,
}

/// Who rides it: the player in each of its seats, as many as it has, seat 0 the driver's. A component
/// on a vehicle's entity; the `Vehicles` mechanic seats and unseats. Indexing it past the last seat
/// panics, so code that does not know how many there are asks `rider`.
#[derive(crate::entities::Component, Clone, Debug, PartialEq, Eq)]
pub struct Seats(Vec<Option<u32>>);

impl Seats {
    /// `count` seats, all free.
    pub fn empty(count: usize) -> Self {
        Seats(vec![None; count])
    }

    /// Who sits in seat `index`; nobody past the last seat.
    pub fn rider(&self, index: usize) -> Option<u32> {
        self.0.get(index).copied().flatten()
    }

    /// Everyone aboard, in seat order.
    pub fn riders(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().flatten().copied()
    }

    pub fn vacant(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// The first free seat.
    pub fn free(&self) -> Option<usize> {
        self.0.iter().position(Option::is_none)
    }

    /// Empties seat `index`, returning who sat there.
    pub fn take(&mut self, index: usize) -> Option<u32> {
        self.0.get_mut(index).and_then(Option::take)
    }

    /// Empties every seat, returning who sat in them.
    pub fn take_all(&mut self) -> Vec<u32> {
        self.0.iter_mut().filter_map(Option::take).collect()
    }
}

impl std::ops::Deref for Seats {
    type Target = [Option<u32>];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Seats {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A vehicle at zero `Health` is a wreck: it burns where it lies for `left` more ticks, then the room
/// puts a fresh one back at its `home`. The `Vehicles` mechanic owns it.
#[derive(crate::entities::Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wreck {
    pub left: u32,
}

/// What a weapon mount (`shared::mounts::MountDef`) is doing: where it points, when it can fire
/// again and what it has left. The `Gunnery` mechanic owns it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mount {
    /// Yaw relative to the hull, radians, + turns left.
    pub yaw: f32,
    /// Elevation, radians, + is up.
    pub pitch: f32,
    /// Ticks until the next round.
    pub cooldown: u32,
    /// Rounds fired so far, modulo 256: which barrel of several is next, and how the client knows to
    /// draw one.
    pub shots: u8,
    /// A mortar's range dial, metres; 0 until it is first dialled.
    pub range: f32,
    /// A machine gun's heat, 0..=255: at 255 it jams until it has cooled to `GUN_UNJAM`.
    pub heat: f32,
    pub jammed: bool,
    /// Rounds left, for a mount that can run out.
    pub ammo: u8,
    /// Ticks towards more ammo (`shared::mounts::Refill`).
    pub refill: u32,
    /// A gun with two kinds of round has its second (`Fire::Shell::alt`) loaded.
    pub alt: bool,
    /// `BUTTON_AMMO` was down last tick: holding it swaps the round once.
    pub swapping: bool,
}

impl Mount {
    /// A mount of `def` as a fresh vehicle has it.
    pub fn fresh(def: &MountDef) -> Self {
        Mount { ammo: def.ammo.map_or(0, |ammo| ammo.full), ..Mount::default() }
    }
}

/// A vehicle's weapons: a component on the entities of the vehicles that have any, holding the mounts'
/// definitions (`VehicleDef::mounts` for a kind's own) and what each is doing, in the order they are
/// worked. The `Gunnery` mechanic owns it.
#[derive(crate::entities::Component, Clone, Debug, PartialEq)]
pub struct Mounts {
    pub defs: &'static [MountDef],
    pub state: Vec<Mount>,
}

impl Mounts {
    /// Mounts of `defs` as a fresh vehicle has them; `None` for no mounts at all.
    pub fn fresh(defs: &'static [MountDef]) -> Option<Self> {
        (!defs.is_empty()).then(|| Mounts { defs, state: defs.iter().map(Mount::fresh).collect() })
    }

    /// The mount the snapshot shows as `show`, if there is one.
    pub fn shown(&self, show: Show) -> Option<&Mount> {
        self.defs.iter().zip(&self.state).find(|(def, _)| def.show == show).map(|(_, mount)| mount)
    }

    /// The mount the snapshot shows as the turret.
    pub fn turret(&self) -> Option<&Mount> {
        self.shown(Show::Turret)
    }

    /// Puts the pilot's arms back as a fresh vehicle's: guns cool and racks full. A turret keeps its
    /// aim and its count.
    pub fn restock(&mut self) {
        for (def, mount) in self.defs.iter().zip(&mut self.state) {
            if def.show.arms() {
                *mount = Mount::fresh(def);
            }
        }
    }
}

/// A gun's heat as the wire carries it.
pub(super) fn heat_byte(heat: f32) -> u8 {
    heat.round().clamp(0.0, 255.0) as u8
}

/// A Roadster's air tank, or a hen's stamina, on the wire: 0..=255 of full (`ROADSTER_AIR` and
/// `HEN_STAMINA` are both 1). The client's codec divides it back (`thrust.air`), and `SimMotor`
/// scales that by `ROADSTER_AIR` for the prediction.
pub(super) fn air_byte(tank: f32) -> u8 {
    (tank / ROADSTER_AIR * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A drone's battery on the wire: 0..=255 of full. Mirrored by the client's drone gauge.
pub(super) fn battery_byte(drone: &Drone) -> u8 {
    (drone.energy as f32 / shared::vehicles::drone_def(drone.kind).energy_ticks as f32 * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Everything mechanics may read and change.
pub struct World {
    pub tick: u32,
    pub map: WorldFile,
    pub physics: Physics,
    /// The families moved to components so far: players (and bots' minds, a `Bot` on the player's
    /// entity), roosters, items on the ground, projectiles and vehicles, in one id space.
    pub entities: crate::entities::Entities,
    /// Where a stickman can walk, built as the bots look for ways round things.
    pub nav: crate::navigation::NavGrid,
    pub(super) next_id: u32,
    pub rng: Rng,
    /// Rifle shots since the last snapshot, for drawing tracers. Drained when snapshots go out.
    pub shots: Vec<ShotEvent>,
    /// What vehicles' armour made of the rounds that met it this tick, for the shooters and the crews
    /// (`S_ARMOR`). Drained every tick.
    pub armor_hits: Vec<crate::mechanics::ArmorHit>,
    /// Marks rounds left on vehicles' armour since the last snapshot, with where each vehicle was (for
    /// who is near enough to be told). Sent with the snapshots (`S_MARKS`).
    pub marks: Vec<(HullMark, [f32; 3])>,
    /// Bullets fired in the last few ticks, each sent to the clients near it in a couple of snapshots.
    pub bullets: Vec<BulletSpawn>,
    /// What the damage path runs the moment something is hurt or dies, from the mechanics' `hooks`.
    pub hooks: crate::events::Hooks,
    /// Solid trees by identity, to look their size up from a collider hit.
    pub(super) trees_by_id: HashMap<ObjId, usize>,
    /// Buildings by identity.
    pub(super) buildings_by_id: HashMap<ObjId, usize>,
    /// Health left on buildings and trees that have been hurt but still stand.
    pub object_hp: HashMap<Obj, u16>,
    /// Everything destroyed so far with the yaw it fell at, oldest first. A joiner is told all of it.
    pub destroyed: Vec<(Obj, i16)>,
    /// What was destroyed since the last tick, with how violently (0..=255, see `combat::damage_object`). Drained and broadcast reliably by `Game::step`.
    pub fresh_destroyed: Vec<(Obj, i16, u8)>,
    /// Buildings hurt since the last tick that need telling about: the damage stage they are in,
    /// and where a blast landed if it leaves a mark. Drained and broadcast reliably by `Game::step`.
    pub fresh_damaged: Vec<FreshDamage>,
    /// Buildings that have had cells blasted out of them: how they are cut and what is gone.
    pub carves: HashMap<ObjId, Carving>,
    /// Bites taken since the last tick, for `Game::step` to broadcast: the building, where the
    /// blast was (centimetres) and the cells it broke and left hanging.
    pub fresh_carved: Vec<FreshCarve>,
    /// Carved buildings waiting to have their structure checked (`settle`), by the tick they are due.
    pub strained: HashMap<ObjId, u32>,
    /// Cells that have lost their hold and are still falling: where each column of them lands and when.
    pub rubble: Vec<Rubble>,
    /// The craters dug in the last `CRATER_FADE_SECONDS` (at most `CRATER_MAX_KEPT`), oldest first, with
    /// the tick each was dug: their marks, for a joiner (`game/craters.rs`). Their heights stay in `map.ground`.
    pub craters: std::collections::VecDeque<(u32, shared::protocol::CraterEntry)>,
    /// Craters dug since the last tick, for `Game::step` to broadcast.
    pub fresh_craters: Vec<shared::protocol::CraterEntry>,
    /// The ground as the importer left it, copied when the first crater lands: what `CRATER_MAX_DEPTH`
    /// is measured from, and what a joiner's `Dug` is the difference from.
    pub pristine: Vec<i32>,
    /// The tiles `(col, row)` whose ground a crater changed.
    pub dug_tiles: std::collections::BTreeSet<(u16, u16)>,
    /// The cubes players have placed with bricks (`game/construct.rs`).
    pub construct: shared::construct::Construct,
    /// What happened to the cubes since the last tick, for `Game::step` to broadcast as one `Built`.
    pub fresh_built: FreshBuilt,
    /// Chunks of cubes whose colliders are out of date, rebuilt once a tick however many cubes changed.
    pub construct_dirty: std::collections::BTreeSet<shared::construct::Pos>,
}

impl World {
    /// Says that `event` happened, for whoever reads that kind (see `crate::events`).
    pub fn send<E: Message>(&mut self, event: E) {
        self.entities.ecs.resource_mut::<Messages<E>>().write(event);
    }

    /// The queue of events of kind `E`, for a reader with its own cursor.
    pub fn events<E: Message>(&self) -> &Messages<E> {
        self.entities.ecs.resource::<Messages<E>>()
    }

    /// The events of kind `E` sent this tick, in the order they were sent.
    pub fn happened<E: Message>(&self) -> impl ExactSizeIterator<Item = &E> {
        self.events::<E>().iter_current_update_messages()
    }

    /// The vehicle player `id` sits in and where their head is in the world, if they sit in it (not
    /// flying it from afar) and its crew show (`shared::vehicles::seat_head`).
    pub fn seated_head(&self, id: u32) -> Option<(u32, [f32; 3])> {
        let seat = self.entities.get::<Player>(id)?.seat.filter(|seat| !seat.remote)?;
        let vehicle = self.entities.get::<Vehicle>(seat.vehicle)?;
        let local = shared::vehicles::seat_head(vehicle.kind, seat.index, self.entities.get::<Mounts>(seat.vehicle).and_then(Mounts::turret).map_or(0.0, |gun| gun.yaw))?;
        Some((seat.vehicle, self.physics.car_pose(seat.vehicle)?.world_point(local)))
    }

    /// The tree `id` names, if it is one that can be hit.
    pub fn tree(&self, id: ObjId) -> Option<&Tree> {
        self.trees_by_id.get(&id).and_then(|&index| self.map.trees.get(index))
    }

    /// How badly the standing building `object` is hurt: 0 (not at all) to `DAMAGE_STAGES`.
    pub fn damage_stage(&self, object: Obj) -> Option<u8> {
        let Obj::Building(id) = object else { return None };
        let full = self.building(id)?.hp() as u32;
        let left = self.object_hp.get(&object).copied().map_or(full, u32::from);
        Some(DAMAGE_STAGE_HEALTH.iter().filter(|&&percent| left * 100 <= full * percent).count() as u8)
    }

    /// Takes the cells within `radius` metres of `point` out of standing building `id`, and every
    /// cell that leaves with nothing under it, and swaps its collider for what is left. Returns what
    /// was cut: `None` when it is not a building that can be carved, or the blast broke nothing.
    pub fn carve(&mut self, id: ObjId, point: [f32; 3], radius: f32) -> Option<Cut> {
        if !self.carves.contains_key(&id) {
            let carving = self.cut_building(id)?;
            self.carves.insert(id, carving);
        }
        let carving = self.carves.get_mut(&id)?;
        let at = point.map(|v| (v * 100.0).round() as i32);
        let cut = carving.shell.blast(&mut carving.removed, at, (radius * 100.0).round() as i32);
        self.finish_cut(id, at, cut)
    }

    /// A round of `damage` hits standing building `id` at `point`: the piece of wall there loses
    /// that much health and breaks out when it has none. As `carve` for what it returns.
    pub fn chip(&mut self, id: ObjId, point: [f32; 3], damage: u8) -> Option<Cut> {
        if !self.carves.contains_key(&id) {
            let carving = self.cut_building(id)?;
            self.carves.insert(id, carving);
        }
        let carving = self.carves.get_mut(&id)?;
        let at = point.map(|v| (v * 100.0).round() as i32);
        let cut = carving.shell.chip(&mut carving.removed, &mut carving.hp, at, damage);
        self.finish_cut(id, at, cut)
    }

    /// Swaps the collider and queues the news for a cut that took something.
    fn finish_cut(&mut self, id: ObjId, at: [i32; 3], cut: Cut) -> Option<Cut> {
        if cut.is_empty() {
            return None;
        }
        let carving = self.carves.get(&id)?;
        let standing = sim::solid_of(&carving.shell.boxes(&carving.removed), &carving.floor);
        self.physics.replace_building(Obj::Building(id), standing);
        self.fresh_carved.push((id, at, cut.clone()));
        self.drop_rubble(id, &cut.fell);
        self.loosen_beside_cells(id, &cut);
        // Whatever it lost is looked at again in a moment: the first cut sets the time, not each one.
        self.strained.entry(id).or_insert(self.tick.wrapping_add(STRAIN_DELAY));
        Some(cut)
    }

    /// Starts the cells `fell` of building `id` falling: one `Rubble` for each column of them, landing
    /// when something let go at that height would reach the building's base.
    fn drop_rubble(&mut self, id: ObjId, fell: &[u32]) {
        let (Some(carving), Some(base)) = (self.carves.get(&id), self.building(id).map(|b| b.base as f32 / 100.0)) else { return };
        let mut columns: HashMap<(i32, i32), (f32, u32)> = HashMap::new();
        for &cell in fell {
            let c = carving.shell.centre(cell);
            let column = columns.entry((c[0], c[2])).or_insert((f32::MIN, 0));
            column.0 = column.0.max(c[1] as f32 / 100.0);
            column.1 += 1;
        }
        let mut landing: Vec<Rubble> = columns
            .into_iter()
            .map(|((x, z), (top, cells))| {
                let fall = (top - base).max(0.0);
                let due = self.tick.wrapping_add((((2.0 * fall / 9.8).sqrt()) * shared::consts::TICK_RATE as f32).ceil() as u32 + 1);
                Rubble { due, at: [x as f32 / 100.0, z as f32 / 100.0], top, ground: base, cells }
            })
            .collect();
        landing.sort_unstable_by(|a, b| (a.due, a.at[0].to_bits(), a.at[1].to_bits()).cmp(&(b.due, b.at[0].to_bits(), b.at[1].to_bits())));
        self.rubble.extend(landing);
    }

    /// Checks the structure of the carved buildings that are due, at most `STRAIN_PER_TICK` a tick
    /// and the longest overdue first, so a room full of wrecks spreads the work over ticks. What a
    /// building cannot hold up crumbles (a `Cut` with only `fell`, told like any other), and it is
    /// looked at again after `STRAIN_DELAY`, until nothing more goes.
    pub fn settle(&mut self) {
        let tick = self.tick;
        let mut due: Vec<(u32, ObjId)> = self.strained.iter().map(|(&id, &at)| (tick.wrapping_sub(at), id)).filter(|&(late, _)| late < u32::MAX / 2).collect();
        due.sort_unstable_by_key(|&(late, id)| (std::cmp::Reverse(late), id));
        for (_, id) in due.into_iter().take(STRAIN_PER_TICK) {
            self.strained.remove(&id);
            let Some(carving) = self.carves.get_mut(&id) else { continue };
            let cut = carving.shell.strain(&mut carving.removed, &carving.whole);
            if cut.is_empty() {
                continue;
            }
            let [x, y, z] = cut.fell.iter().fold([0i64; 3], |sum, &cell| {
                let c = carving.shell.centre(cell);
                [sum[0] + i64::from(c[0]), sum[1] + i64::from(c[1]), sum[2] + i64::from(c[2])]
            });
            let n = cut.fell.len() as i64;
            let at = [(x / n) as i32, (y / n) as i32, (z / n) as i32];
            self.finish_cut(id, at, cut);
        }
    }

    /// Building `id` cut into cells, as `physics::building_shell`.
    pub(super) fn cut_building(&self, id: ObjId) -> Option<Carving> {
        let index = *self.buildings_by_id.get(&id)?;
        let (shell, floor) = sim::building_shell(&self.map, index)?;
        let removed = shell.untouched();
        let hp = shell.fresh_hp();
        let whole = shell.hold(&removed);
        Some(Carving { shell, removed, hp, floor, whole })
    }

    /// The building `id` names.
    pub fn building(&self, id: ObjId) -> Option<&Building> {
        self.buildings_by_id.get(&id).and_then(|&index| self.map.buildings.get(index))
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// A building hurt: its identity, its damage stage and where a blast marked it.
pub type FreshDamage = (ObjId, u8, Option<[f32; 3]>);

/// A building's cells and which of them are gone. Made the first time a blast reaches it.
pub struct Carving {
    pub shell: Shell,
    /// One flag per cell: taken out, by a blast or by having nothing under it.
    pub removed: Vec<bool>,
    /// Health left in each cell against bullets; a blast does not read it.
    pub hp: Vec<u8>,
    /// The floor's triangles, which are never carved but stay in the building's collider.
    pub floor: Vec<Tri>,
    /// How the building was held up when whole, to measure what is left against (`Shell::strain`).
    pub whole: shared::carve::Hold,
}

/// Ticks between a carved building being hurt and its structure being checked: long enough to
/// groan, short enough to be blamed on the blast.
pub const STRAIN_DELAY: u32 = 30;
/// Most buildings whose structure is checked in one tick.
pub const STRAIN_PER_TICK: usize = 2;

/// A column of cells falling from a building: where it comes down (metres), the height of its top and
/// of the ground under it, how many cells are in it, and the tick it lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rubble {
    pub due: u32,
    pub at: [f32; 2],
    pub top: f32,
    pub ground: f32,
    pub cells: u32,
}

/// Cubes placed (what each is), dug out, shot or blasted, and fallen, since the last tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FreshBuilt {
    pub placed: Vec<(shared::construct::Pos, shared::construct::Cube)>,
    pub removed: Vec<shared::construct::Pos>,
    pub fell: Vec<shared::construct::Pos>,
}

impl FreshBuilt {
    pub fn is_empty(&self) -> bool {
        self.placed.is_empty() && self.removed.is_empty() && self.fell.is_empty()
    }
}

/// A building bitten: its identity, where the blast was (cm) and the cells it took.
pub type FreshCarve = (ObjId, [i32; 3], Cut);
