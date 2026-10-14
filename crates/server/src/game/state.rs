//! The state the game loop owns: players, vehicles, roosters, rockets and the `World` that holds them,
//! and the `Command`/`Outgoing` messages the sessions exchange with it.

use super::*;

pub enum Command {
    /// Admit a player. Replies with its entity id, or `None` when the room is full.
    Join { outbox: mpsc::Sender<Outgoing>, reply: oneshot::Sender<Option<u32>> },
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
}

/// What a player holds at the start of every life: the revolver, the AK5, the sniper rifle and the shotgun
/// with full magazines, and a full stack of grenades.
pub fn starting_hotbar() -> [ItemSlot; HOTBAR_SLOTS] {
    let full = |item: u8| ItemSlot { item, count: 1, ammo: def(item).map_or(0, |d| d.mag) };
    let mut hotbar = [ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 }; HOTBAR_SLOTS];
    hotbar[0] = full(ITEM_REVOLVER);
    hotbar[1] = full(ITEM_AK5);
    hotbar[2] = full(ITEM_SNIPER);
    hotbar[3] = ItemSlot { item: ITEM_GRENADE, count: def(ITEM_GRENADE).map_or(1, |d| d.stack), ammo: 0 };
    hotbar[4] = full(ITEM_SHOTGUN);
    hotbar
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
}

impl Player {
    pub fn alive(&self) -> bool {
        self.hp > 0
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

/// An item lying on the ground. A taken one stays in the map, out of sight, until it comes back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub item: u8,
    /// How many of the item taking it gives.
    pub count: u8,
    pub position: [f32; 3],
    /// Ticks until it reappears; zero when it is lying there to be taken.
    pub respawn_in: u32,
    /// Dropped by something that died (a rooster's egg): gone once taken, instead of coming back.
    pub dropped: bool,
}

/// What a rooster is doing. Its `mode_left` counts ticks down to the next decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoosterMode {
    Idle,
    Wander,
    Crow,
}

/// A Hedemora rooster: an NPC that meanders around `home`, crows, and drops a golden egg when it
/// dies. It shares the id space with players. At zero `hp` it is dead and `respawn_in` counts down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rooster {
    /// Feet.
    pub position: [f32; 3],
    pub yaw: f32,
    pub hp: u8,
    pub mode: RoosterMode,
    pub mode_left: u32,
    /// Where it is walking to, `[x, z]`.
    pub target: [f32; 2],
    pub home: [f32; 2],
    pub respawn_in: u32,
}

impl Rooster {
    pub fn alive(&self) -> bool {
        self.hp > 0
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

/// A gun's heat as the wire carries it.
pub(super) fn heat_byte(heat: f32) -> u8 {
    heat.round().clamp(0.0, 255.0) as u8
}

/// Where the standing, living players were at the end of one tick: `(tick, [(id, position)])`.
type Frame = (u32, Vec<(u32, [f32; 3])>);

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
    /// Items on the ground, which share the id space too.
    pub pickups: BTreeMap<u32, Pickup>,
    /// Roosters, which share the id space too.
    pub roosters: BTreeMap<u32, Rooster>,
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
    /// Solid trees by identity, to look their size up from a collider hit.
    pub(super) trees_by_id: HashMap<ObjId, usize>,
    /// Buildings by identity.
    pub(super) buildings_by_id: HashMap<ObjId, usize>,
    /// Health left on buildings and trees that have been hurt but still stand.
    pub object_hp: HashMap<Obj, u16>,
    /// Everything destroyed so far with the yaw it fell at, oldest first. A joiner is told all of it.
    pub destroyed: Vec<(Obj, i16)>,
    /// What was destroyed since the last tick. Drained and broadcast reliably by `Game::step`.
    pub fresh_destroyed: Vec<(Obj, i16)>,
    /// Buildings hurt since the last tick that need telling about: the damage stage they are in,
    /// and where a blast landed if it leaves a mark. Drained and broadcast reliably by `Game::step`.
    pub fresh_damaged: Vec<FreshDamage>,
    /// Where each standing, living player was at the end of each of the last few ticks, newest
    /// last, labelled with the tick number a snapshot of that state carries. Lag compensation
    /// rewinds shots against it.
    pub(super) history: VecDeque<Frame>,
}

impl World {
    /// Notes where everyone stands, once a tick has finished.
    pub(super) fn record_history(&mut self) {
        let positions = self.players.iter().filter(|(_, p)| p.seat.is_none() && p.alive()).map(|(&id, p)| (id, p.character.position)).collect();
        if self.history.len() > MAX_REWIND_TICKS as usize {
            self.history.pop_front();
        }
        self.history.push_back((self.tick, positions));
    }

    /// Where player `id` was `back` ticks ago, if the record goes that far and had them in it.
    pub fn position_at(&self, id: u32, back: u32) -> Option<[f32; 3]> {
        let tick = self.tick.wrapping_sub(back);
        let (_, positions) = self.history.iter().rev().find(|(t, _)| *t == tick)?;
        positions.iter().find(|(who, _)| *who == id).map(|(_, position)| *position)
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
