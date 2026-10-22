//! The state the game loop owns: players, vehicles, rockets, the ECS entities and the `World` that holds them,
//! and the `Command`/`Outgoing` messages the sessions exchange with it.

use super::*;
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
/// a flare gun with a full stack of flares, an FPV drone and a loitering drone.
pub fn starting_hotbar() -> [ItemSlot; HOTBAR_SLOTS] {
    let full = |item: u8| ItemSlot { item, count: def(item).map_or(1, |d| d.stack), ammo: def(item).map_or(0, |d| d.mag) };
    [full(ITEM_AK5), full(ITEM_SNIPER), full(ITEM_FLARE), full(ITEM_DRONE), full(ITEM_LOITER)]
}

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
    /// Health; zero means dead and waiting for `respawn_in` to run out.
    pub hp: u8,
    pub respawn_in: u32,
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
#[derive(Clone, Debug, PartialEq)]
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

/// Something loud a bot may hear: where it was, who made it (none for a wreck going up, say), and
/// how far it carries, metres. Walls do not stop it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    pub at: [f32; 3],
    pub by: Option<u32>,
    pub reach: f32,
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

impl Player {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }

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

/// Game-side state of a vehicle. Its rigid body lives in `Physics`, under the same id.
pub struct Vehicle {
    pub kind: u8,
    /// Player in each seat. Seat 0 is the driver.
    pub seats: [Option<u32>; CAR_SEATS],
    pub controls: CarControls,
    /// Where it respawns after falling out of the world.
    pub home: ([f32; 3], f32),
    /// Consecutive ticks spent on its side or roof and not moving.
    pub stuck_ticks: u32,
    /// The turret, for a vehicle that has one (a plane's is the gunner's ring mount).
    pub gun: Option<Gun>,
    /// A plane's forward guns and bombs.
    pub arms: Option<PlaneArms>,
    /// Health. At zero it is a wreck for `WRECK_TICKS`, then it is put back at `home`.
    pub hp: u8,
    pub wreck_in: u32,
    /// Whoever last hurt it, who is credited with the deaths when it blows up.
    pub last_attacker: Option<u32>,
    /// What a tank's turret has left before it comes off (`TANK_TURRET_HP` when whole).
    pub turret_hp: u8,
    /// Crash damage as the snapshot carries it (`DAMAGE_*`); all zeros when unhurt.
    pub damage: [u8; 8],
    /// Ticks before it may throw sparks again.
    pub scrape_in: u8,
    /// A drone's link to its pilot and its battery; `None` for anything else.
    pub drone: Option<Drone>,
}

/// What a drone carries beyond being a vehicle. The `Drones` mechanic owns it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drone {
    /// `KIND_DRONE` or `KIND_LOITER`, which picks its `shared::items::drone_def`.
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

impl Vehicle {
    pub fn wrecked(&self) -> bool {
        self.hp == 0
    }
}

/// A rocket, a grenade or a tank shell in flight. The `Combat` mechanic steps rockets and grenades,
/// `Gunnery` steps shells. A rocket flies straight and bursts on contact; a grenade falls, bounces,
/// and bursts when `ttl` runs out; a shell falls in an arc and bursts on contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rocket {
    /// `PROJECTILE_*`.
    pub kind: u8,
    pub owner: u32,
    /// The vehicle it left, which it flies through; 0 for none.
    pub source: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub ttl: u32,
    /// For a `PROJECTILE_BULLET`: the gun it came from (`ITEM_*`), whose damage, falloff and weapon id it
    /// carries; 0 otherwise.
    pub item: u8,
    /// For a bullet: how many ticks back the shooter saw the world, so it meets stickmen where they
    /// were in the picture it was aimed in.
    pub back: u32,
    /// For a bullet: metres flown, which its damage falls off with.
    pub flown: f32,
}

/// A vehicle's gun: where it points and when it can fire again. The `Gunnery` mechanic owns it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gun {
    /// Turret yaw relative to the hull, radians, + turns left.
    pub yaw: f32,
    /// Barrel elevation, radians, + is up.
    pub pitch: f32,
    /// Ticks until the next shell can be fired.
    pub cooldown: u32,
    /// Shells fired so far, modulo 256.
    pub shots: u8,
    /// Unused: shells fly now, so there is no distance to report. Kept so the wire does not change.
    pub range: f32,
    /// A machine gun's heat, 0..=255: at 255 it jams until it has cooled to `GUN_UNJAM`.
    pub heat: f32,
    pub jammed: bool,
    /// A helicopter's rocket pods: ticks until the next rocket, and rockets fired (which tube is next).
    pub pod_cooldown: u32,
    pub pod_shots: u8,
}

/// What a plane's pilot fires: the two forward guns, which take turns, and the bomb rack. The
/// `Gunnery` mechanic owns it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneArms {
    /// Ticks until the next round.
    pub cooldown: u32,
    /// Rounds fired so far, modulo 256; the left gun fires the even ones.
    pub shots: u8,
    pub heat: f32,
    pub jammed: bool,
    pub bombs: u8,
    /// Ticks until the next bomb can drop.
    pub bomb_cooldown: u32,
    /// Ticks the plane has stood still on the ground, towards a full rack.
    pub rearm: u32,
}

impl Default for PlaneArms {
    fn default() -> Self {
        PlaneArms { cooldown: 0, shots: 0, heat: 0.0, jammed: false, bombs: PLANE_BOMBS, bomb_cooldown: 0, rearm: 0 }
    }
}

impl PlaneArms {
    /// A full rack for a vehicle of `kind`: a plane's bombs, or a hen's clutch of eggs.
    pub fn fresh(kind: u8) -> Self {
        PlaneArms { bombs: if kind == KIND_HEN { HEN_EGGS } else { PLANE_BOMBS }, ..PlaneArms::default() }
    }
}

/// A gun's heat as the wire carries it.
pub(super) fn heat_byte(heat: f32) -> u8 {
    heat.round().clamp(0.0, 255.0) as u8
}

/// A Roadster's air tank, or a hen's stamina, on the wire: 0..=255 of full (`ROADSTER_AIR` and
/// `HEN_STAMINA` are both 1). Mirrored by `airOf` in `vehicle.ts`.
pub(super) fn air_byte(tank: f32) -> u8 {
    (tank / ROADSTER_AIR * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A drone's battery on the wire: 0..=255 of full. Mirrored by the client's drone gauge.
pub(super) fn battery_byte(drone: &Drone) -> u8 {
    (drone.energy as f32 / shared::items::drone_def(drone.kind).energy_ticks as f32 * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Where the standing, living players were at the end of one tick: `(tick, [(id, position)])`.
/// A tick and where everyone was at its end: each player's capsule centre, or their head point when
/// seated (`true`) in a vehicle whose crew show.
type Frame = (u32, Vec<(u32, [f32; 3], bool)>);

/// Everything mechanics may read and change.
pub struct World {
    pub tick: u32,
    pub map: WorldFile,
    pub physics: Physics,
    /// Ordered by id so every pass over players is deterministic.
    pub players: BTreeMap<u32, Player>,
    /// Vehicles share the id space with players.
    pub vehicles: BTreeMap<u32, Vehicle>,
    /// Rockets in flight, which share the id space too.
    pub rockets: BTreeMap<u32, Rocket>,
    /// The families moved to components so far (roosters, items on the ground), which share the id
    /// space too.
    pub entities: crate::entities::Entities,
    /// The minds of the players that are bots, by player id.
    pub bots: BTreeMap<u32, Bot>,
    /// Where a stickman can walk, built as the bots look for ways round things.
    pub nav: crate::navigation::NavGrid,
    pub(super) next_id: u32,
    pub rng: Rng,
    /// Rifle shots since the last snapshot, for drawing tracers. Drained when snapshots go out.
    pub shots: Vec<ShotEvent>,
    /// Bullets fired in the last few ticks, each sent to the clients near it in a couple of snapshots.
    pub bullets: Vec<BulletSpawn>,
    /// Kills since the last tick. Drained and broadcast reliably by `Game::step`.
    pub kills: Vec<Kill>,
    /// Hits since the last tick, each with the player it is for. Drained, summed and sent
    /// reliably to that player by `Game::step`.
    pub hits: Vec<(u32, Hit)>,
    /// Gunshots and blasts since the bots last listened. Drained by `Bots` at the start of each tick.
    pub noises: Vec<Noise>,
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
    /// Where each living player was at the end of each of the last few ticks (standing, or their head
    /// where they sit in the open), newest last, labelled with the tick number a snapshot of that state carries. Lag compensation
    /// rewinds shots against it.
    pub(super) history: VecDeque<Frame>,
}

impl World {
    /// Notes where everyone stands, once a tick has finished.
    pub(super) fn record_history(&mut self) {
        let positions = self
            .players
            .iter()
            .filter(|(_, p)| p.alive())
            .filter_map(|(&id, p)| if p.embodied() { Some((id, p.character.position, false)) } else { self.seated_head(id).map(|(_, head)| (id, head, true)) })
            .collect();
        if self.history.len() > MAX_REWIND_TICKS as usize {
            self.history.pop_front();
        }
        self.history.push_back((self.tick, positions));
    }

    /// Where player `id` was `back` ticks ago, if the record goes that far and had them in it.
    pub fn position_at(&self, id: u32, back: u32) -> Option<[f32; 3]> {
        let tick = self.tick.wrapping_sub(back);
        let (_, positions) = self.history.iter().rev().find(|(t, _)| *t == tick)?;
        positions.iter().find(|(who, _, seated)| *who == id && !seated).map(|(_, position, _)| *position)
    }

    /// Where seated player `id`'s head was `back` ticks ago, if the record goes that far and had them seated in it.
    pub fn head_at(&self, id: u32, back: u32) -> Option<[f32; 3]> {
        let tick = self.tick.wrapping_sub(back);
        let (_, positions) = self.history.iter().rev().find(|(t, _)| *t == tick)?;
        positions.iter().find(|(who, _, seated)| *who == id && *seated).map(|(_, head, _)| *head)
    }

    /// The vehicle player `id` sits in and where their head is in the world, if they sit in it (not
    /// flying it from afar) and its crew show (`shared::vehicles::seat_head`).
    pub fn seated_head(&self, id: u32) -> Option<(u32, [f32; 3])> {
        let seat = self.players.get(&id)?.seat.filter(|seat| !seat.remote)?;
        let vehicle = self.vehicles.get(&seat.vehicle)?;
        let local = shared::vehicles::seat_head(vehicle.kind, seat.index, vehicle.gun.as_ref().map_or(0.0, |gun| gun.yaw))?;
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
        Some(cut)
    }

    /// Building `id` cut into cells, as `physics::building_shell`.
    fn cut_building(&self, id: ObjId) -> Option<Carving> {
        let index = *self.buildings_by_id.get(&id)?;
        let (shell, floor) = sim::building_shell(&self.map, index)?;
        let removed = shell.untouched();
        let hp = shell.fresh_hp();
        Some(Carving { shell, removed, hp, floor })
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
}

/// A building bitten: its identity, where the blast was (cm) and the cells it took.
pub type FreshCarve = (ObjId, [i32; 3], Cut);
