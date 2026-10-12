//! The authoritative game loop: one task owns the whole world and steps it at a fixed rate.
//! Sessions talk to it only through `Command`s in and `Outgoing` messages out.

use crate::jitter::JitterBuffer;
use crate::mechanics::{Mechanic, default_mechanics};
use crate::physics::{CarControls, Character, Obj, Physics};
use crate::replication::{Everything, Own, Replica};
use crate::rng::Rng;
use anyhow::Result;
use bytes::Bytes;
use shared::consts::*;
use shared::items::{HOTBAR_SLOTS, ITEM_AK5, ITEM_GRENADE, ITEM_NONE, ITEM_REVOLVER, ITEM_SNIPER, def};
use shared::protocol::*;
use shared::quant::{pack_quat, to_axis, to_cm, to_cm_i16, to_milli_i16, to_mrad};
use shared::world::{Building, ObjId, Tree, WorldFile};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

pub enum Command {
    /// Admit a player. Replies with its entity id, or `None` when the room is full.
    Join { outbox: mpsc::Sender<Outgoing>, reply: oneshot::Sender<Option<u32>> },
    Leave(u32),
    /// Input frames, with the tick of the newest snapshot the client says it holds.
    Input(u32, u32, Vec<InputFrame>),
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

/// What a player holds at the start of every life: the revolver, the AK5 and the sniper rifle with full
/// magazines, a full stack of grenades, and one empty slot for whatever turns up.
pub fn starting_hotbar() -> [ItemSlot; HOTBAR_SLOTS] {
    let full = |item: u8| ItemSlot(item, 1, def(item).map_or(0, |d| d.mag));
    let mut hotbar = [ItemSlot(ITEM_NONE, 0, 0); HOTBAR_SLOTS];
    hotbar[0] = full(ITEM_REVOLVER);
    hotbar[1] = full(ITEM_AK5);
    hotbar[2] = full(ITEM_SNIPER);
    hotbar[3] = ItemSlot(ITEM_GRENADE, def(ITEM_GRENADE).map_or(1, |d| d.stack), 0);
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
fn heat_byte(heat: f32) -> u8 {
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
    next_id: u32,
    pub rng: Rng,
    /// Rifle shots since the last snapshot, for drawing tracers. Drained when snapshots go out.
    pub shots: Vec<ShotEvent>,
    /// Kills since the last tick. Drained and broadcast reliably by `Game::step`.
    pub kills: Vec<Kill>,
    /// Hits since the last tick, each with the player it is for. Drained, summed and sent
    /// reliably to that player by `Game::step`.
    pub hits: Vec<(u32, Hit)>,
    /// Solid trees by identity, to look their size up from a collider hit.
    trees_by_id: HashMap<ObjId, usize>,
    /// Buildings by identity.
    buildings_by_id: HashMap<ObjId, usize>,
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
    history: VecDeque<Frame>,
}

impl World {
    /// Notes where everyone stands, once a tick has finished.
    fn record_history(&mut self) {
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

/// `object` as a `Destroyed` entry.
fn destroyed_entry(object: Obj, yaw: i16) -> (u8, u16, u16, u32, i16) {
    match object {
        Obj::Tree(id) => (OBJ_TREE, id.col, id.row, id.index, yaw),
        Obj::Building(id) => (OBJ_BUILDING, id.col, id.row, id.index, yaw),
    }
}

/// Building `id`'s damage `stage`, and where a blast marked it, as a `Damaged` entry.
fn damaged_entry(id: ObjId, stage: u8, mark: Option<[f32; 3]>) -> DamagedEntry {
    let [x, y, z] = mark.map(|p| p.map(|v| (v * 100.0).round() as i32)).unwrap_or([0; 3]);
    (id.col, id.row, id.index, stage, mark.is_some() as u8, x, y, z)
}

pub struct Game {
    pub world: World,
    mechanics: Vec<Box<dyn Mechanic>>,
    outboxes: BTreeMap<u32, mpsc::Sender<Outgoing>>,
    /// What each player has been sent, to diff their next snapshot against.
    replicas: BTreeMap<u32, Replica>,
    world_hash: u32,
}

impl Game {
    pub fn new(map: WorldFile, world_hash: u32) -> Result<Self> {
        let mut game = Self {
            world: World {
                tick: 0,
                physics: Physics::new(&map)?,
                trees_by_id: map.tree_ids.iter().enumerate().filter(|&(index, _)| map.trees[index].solid).map(|(index, &id)| (id, index)).collect(),
                buildings_by_id: map.building_ids.iter().enumerate().map(|(index, &id)| (id, index)).collect(),
                object_hp: HashMap::new(),
                destroyed: Vec::new(),
                fresh_destroyed: Vec::new(),
                fresh_damaged: Vec::new(),
                map,
                players: BTreeMap::new(),
                vehicles: BTreeMap::new(),
                rockets: BTreeMap::new(),
                pickups: BTreeMap::new(),
                roosters: BTreeMap::new(),
                next_id: 1,
                rng: Rng::from_time(),
                shots: Vec::new(),
                kills: Vec::new(),
                hits: Vec::new(),
                history: VecDeque::new(),
            },
            mechanics: default_mechanics(),
            outboxes: BTreeMap::new(),
            replicas: BTreeMap::new(),
            world_hash,
        };
        for mechanic in &mut game.mechanics {
            mechanic.on_start(&mut game.world);
        }
        Ok(game)
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Join { outbox, reply } => {
                let _ = reply.send(self.join(outbox));
            }
            Command::Leave(id) => self.leave(id),
            Command::Input(id, baseline, frames) => {
                if let Some(player) = self.world.players.get_mut(&id) {
                    player.inputs.push(&frames);
                }
                if let Some(replica) = self.replicas.get_mut(&id) {
                    replica.acknowledge(baseline);
                }
            }
        }
    }

    fn join(&mut self, outbox: mpsc::Sender<Outgoing>) -> Option<u32> {
        if self.world.players.len() >= MAX_PLAYERS {
            return None;
        }
        let id = self.world.alloc_id();
        let welcome = encode(&Welcome(S_WELCOME, PROTOCOL_VERSION, id, TICK_RATE as u8, SNAPSHOT_EVERY as u8, self.world_hash));
        if outbox.try_send(Outgoing::Reliable(welcome.into())).is_err() {
            return None;
        }
        self.world.players.insert(
            id,
            Player {
                character: Character::default(),
                seat: None,
                hit_cooldown: 0,
                input: InputFrame::default(),
                inputs: JitterBuffer::default(),
                ack: 0,
                hp: PLAYER_MAX_HP,
                respawn_in: 0,
                hotbar: starting_hotbar(),
                selected: 0,
                fire_cooldown: 0,
                reload_left: 0,
                channel_left: 0,
                trigger_held: false,
                kills: 0,
                deaths: 0,
            },
        );
        let scores = self.world.players.iter().map(|(&id, player)| (id, player.kills, player.deaths)).collect();
        let _ = outbox.try_send(Outgoing::Reliable(encode(&Scores(S_SCORES, scores)).into()));
        // Always sent, even empty: the client takes the first one after the welcome as the catch-up
        // (not news to animate), and snapshots on the datagram channel can overtake it.
        let message = Destroyed(S_DESTROYED, self.world.destroyed.iter().map(|&(object, yaw)| destroyed_entry(object, yaw)).collect());
        let _ = outbox.try_send(Outgoing::Reliable(encode(&message).into()));
        let hurt: Vec<_> = self.world.object_hp.keys().filter_map(|&object| self.world.damage_stage(object).map(|stage| (object, stage))).collect();
        let hurt: Vec<_> = hurt.into_iter().filter_map(|(object, stage)| match object {
            Obj::Building(object_id) if stage > 0 => Some(damaged_entry(object_id, stage, None)),
            _ => None,
        }).collect();
        if !hurt.is_empty() {
            let _ = outbox.try_send(Outgoing::Reliable(encode(&Damaged(S_DAMAGED, hurt)).into()));
        }
        self.outboxes.insert(id, outbox);
        self.replicas.insert(id, Replica::default());
        for mechanic in &mut self.mechanics {
            mechanic.on_join(&mut self.world, id);
        }
        tracing::info!(id, players = self.world.players.len(), "player joined");
        Some(id)
    }

    fn leave(&mut self, id: u32) {
        if !self.world.players.contains_key(&id) {
            return;
        }
        for mechanic in &mut self.mechanics {
            mechanic.on_leave(&mut self.world, id);
        }
        self.world.players.remove(&id);
        self.outboxes.remove(&id);
        self.replicas.remove(&id);
        let leave = Bytes::from(encode(&Leave(S_LEAVE, id)));
        for outbox in self.outboxes.values() {
            let _ = outbox.try_send(Outgoing::Reliable(leave.clone()));
        }
        tracing::info!(id, players = self.world.players.len(), "player left");
    }

    /// One fixed simulation step.
    pub fn step(&mut self) {
        for player in self.world.players.values_mut() {
            let consumed = player.inputs.pop();
            player.input = consumed.input;
            if consumed.fresh {
                player.ack = consumed.input.seq();
            }
        }
        for mechanic in &mut self.mechanics {
            mechanic.on_tick(&mut self.world, TICK_DT);
        }
        self.broadcast_kills();
        self.send_hits();
        self.broadcast_destroyed();
        self.broadcast_damaged();
        self.world.tick = self.world.tick.wrapping_add(1);
        self.world.record_history();
        if self.world.tick.is_multiple_of(SNAPSHOT_EVERY) {
            self.send_snapshots();
        }
    }

    /// Tells everyone about the buildings and trees destroyed this tick, as one message.
    fn broadcast_destroyed(&mut self) {
        if self.world.fresh_destroyed.is_empty() {
            return;
        }
        let entries = self.world.fresh_destroyed.drain(..).map(|(object, yaw)| destroyed_entry(object, yaw)).collect();
        let bytes = Bytes::from(encode(&Destroyed(S_DESTROYED, entries)));
        for outbox in self.outboxes.values() {
            let _ = outbox.try_send(Outgoing::Reliable(bytes.clone()));
        }
    }

    /// Tells everyone about the buildings hurt this tick, as one message.
    fn broadcast_damaged(&mut self) {
        if self.world.fresh_damaged.is_empty() {
            return;
        }
        let entries = self.world.fresh_damaged.drain(..).map(|(id, stage, mark)| damaged_entry(id, stage, mark)).collect();
        let bytes = Bytes::from(encode(&Damaged(S_DAMAGED, entries)));
        for outbox in self.outboxes.values() {
            let _ = outbox.try_send(Outgoing::Reliable(bytes.clone()));
        }
    }

    /// Tells each player about the hits they dealt or took this tick, amounts summed per kind.
    fn send_hits(&mut self) {
        let mut merged: Vec<(u32, Hit)> = Vec::new();
        for (to, hit) in self.world.hits.drain(..) {
            match merged.iter_mut().find(|(t, h)| *t == to && h.1 == hit.1 && h.2 == hit.2 && h.9 == hit.9) {
                Some((_, sum)) => sum.3 = sum.3.saturating_add(hit.3),
                None => merged.push((to, hit)),
            }
        }
        for (to, hit) in merged {
            if let Some(outbox) = self.outboxes.get(&to) {
                let _ = outbox.try_send(Outgoing::Reliable(Bytes::from(encode(&hit))));
            }
        }
    }

    /// Tells everyone about the kills mechanics recorded this tick.
    fn broadcast_kills(&mut self) {
        for kill in self.world.kills.drain(..) {
            let bytes = Bytes::from(encode(&kill));
            for outbox in self.outboxes.values() {
                let _ = outbox.try_send(Outgoing::Reliable(bytes.clone()));
            }
        }
    }

    /// Standing players, then the living roosters. Seated players are shown by their vehicle instead.
    pub fn entity_states(&self) -> Vec<EntityState> {
        let roosters = self.world.roosters.iter().filter(|(_, rooster)| rooster.alive()).map(|(&id, rooster)| {
            let [x, y, z] = rooster.position;
            let mut flags = FLAG_GROUNDED;
            if rooster.mode == RoosterMode::Crow {
                flags |= FLAG_CROWING;
            }
            EntityState(id, KIND_ROOSTER, to_cm(x), to_cm(y), to_cm(z), to_mrad(rooster.yaw), 0, flags, 0, 0)
        });
        self.world
            .players
            .iter()
            .filter(|(_, player)| player.seat.is_none())
            .map(|(&id, player)| {
                let c = &player.character;
                let mut flags = 0;
                if c.grounded {
                    flags |= FLAG_GROUNDED;
                }
                if !player.alive() {
                    flags |= FLAG_DEAD;
                }
                if player.input.buttons() & BUTTON_CROUCH != 0 {
                    flags |= FLAG_CROUCHING;
                }
                flags |= (player.held().0 << FLAG_HELD_SHIFT) & FLAG_HELD_MASK;
                EntityState(
                    id,
                    KIND_STICKMAN,
                    to_cm(c.position[0]),
                    to_cm(c.position[1]),
                    to_cm(c.position[2]),
                    to_mrad(c.yaw),
                    to_cm_i16(c.vy),
                    flags,
                    to_cm_i16(c.knock[0]),
                    to_cm_i16(c.knock[1]),
                )
            })
            .chain(roosters)
            .collect()
    }

    pub fn vehicle_states(&self) -> Vec<VehicleState> {
        self.world
            .vehicles
            .iter()
            .filter_map(|(&id, vehicle)| {
                let pose = self.world.physics.car_pose(id)?;
                let [x, y, z] = pose.position;
                let [qx, qy, qz] = pack_quat(pose.rotation);
                let seats = vehicle.seats[..seat_count(vehicle.kind)].iter().map(|seat| seat.unwrap_or(0)).collect();
                let turret = vehicle.gun.map(|gun| Turret(to_mrad(gun.yaw), to_mrad(gun.pitch), gun.shots, (gun.range * 10.0).round().clamp(0.0, u16::MAX as f32) as u16));
                let damage = (vehicle.damage != [0; 8]).then_some(vehicle.damage);
                let arms = vehicle.arms.map(|arms| {
                    let rear = vehicle.gun.unwrap_or_default();
                    let mut flags = 0;
                    if vehicle.controls.powered && vehicle.controls.lift < -0.5 {
                        flags |= ARMS_CUT;
                    }
                    if arms.jammed {
                        flags |= ARMS_JAMMED;
                    }
                    if rear.jammed {
                        flags |= ARMS_REAR_JAMMED;
                    }
                    Arms(arms.shots, heat_byte(arms.heat), heat_byte(rear.heat), arms.bombs, flags)
                });
                Some(VehicleState(id, vehicle.kind, to_cm(x), to_cm(y), to_cm(z), qx, qy, qz, to_axis(pose.steer), seats, turret, vehicle.hp, damage, arms))
            })
            .collect()
    }

    /// The motion of the vehicle `id` drives, for their own prediction; `None` for anyone else.
    pub fn projectile_states(&self) -> Vec<ProjectileState> {
        self.world
            .rockets
            .iter()
            .map(|(&id, rocket)| {
                let [x, y, z] = rocket.position.map(to_cm);
                let [vx, vy, vz] = rocket.velocity.map(to_cm_i16);
                ProjectileState(id, rocket.kind, x, y, z, vx, vy, vz)
            })
            .collect()
    }

    /// Items lying on the ground, ordered by id.
    pub fn pickup_states(&self) -> Vec<PickupState> {
        self.world
            .pickups
            .iter()
            .filter(|(_, pickup)| pickup.respawn_in == 0)
            .map(|(&id, pickup)| {
                let [x, y, z] = pickup.position.map(to_cm);
                PickupState(id, pickup.item, x, y, z)
            })
            .collect()
    }

    pub(crate) fn drive_state(&self, id: u32) -> Option<DriveState> {
        let seat = self.world.players.get(&id)?.seat?;
        if seat.index != 0 {
            return None;
        }
        let motion = self.world.physics.car_motion(seat.vehicle)?;
        let [vx, vy, vz] = motion.linvel.map(to_cm_i16);
        let [wx, wy, wz] = motion.angvel.map(to_milli_i16);
        Some(DriveState(vx, vy, vz, wx, wy, wz, to_milli_i16(motion.steer), to_milli_i16(motion.spin)))
    }

    fn send_snapshots(&mut self) {
        let entities = self.entity_states();
        let vehicles = self.vehicle_states();
        let rockets = self.projectile_states();
        let pickups = self.pickup_states();
        let mut shots = std::mem::take(&mut self.world.shots);
        shots.truncate(MAX_SHOT_EVENTS);
        let everything = Everything { entities: &entities, vehicles: &vehicles, shots: &shots, rockets: &rockets, pickups: &pickups };
        let ids: Vec<u32> = self.outboxes.keys().copied().collect();
        for id in ids {
            let Some(player) = self.world.players.get(&id) else { continue };
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, drive: self.drive_state(id),
                me: SelfState(player.hp, progress(player), player.hotbar.to_vec()),
            };
            // Seated players ride with their vehicle, so the vehicle's position is theirs.
            let at = [player.character.position[0], player.character.position[2]];
            let (Some(outbox), Some(replica)) = (self.outboxes.get(&id), self.replicas.get_mut(&id)) else { continue };
            let snapshot = replica.snapshot(at, &everything, own);
            // A full outbox means the session is stalled; dropping a snapshot is harmless.
            let _ = outbox.try_send(Outgoing::Datagram(snapshot.into()));
        }
    }
}

/// How far the player's reload or bandage has got, 0..=255, or 0 when neither is going on.
fn progress(player: &Player) -> u8 {
    use shared::items::{BANDAGE_TICKS, def};
    let (left, total) = if player.channel_left > 0 {
        (player.channel_left, BANDAGE_TICKS)
    } else if player.reload_left > 0 {
        (player.reload_left, def(player.held().0).map_or(1, |d| d.reload_ticks.max(1)))
    } else {
        return 0;
    };
    let done = 1.0 - left as f32 / total as f32;
    ((done * 255.0).round() as u8).max(1)
}

/// Runs the game loop until every command sender is gone.
pub async fn run(mut game: Game, mut commands: mpsc::UnboundedReceiver<Command>) {
    let mut ticker = interval(Duration::from_secs_f64(1.0 / TICK_RATE as f64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);
    loop {
        ticker.tick().await;
        loop {
            match commands.try_recv() {
                Ok(command) => game.handle(command),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => return,
            }
        }
        game.step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::WORLD_VERSION;

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
        let (outbox, inbox) = mpsc::channel(256);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        (response.try_recv().unwrap().unwrap(), inbox)
    }

    fn drain(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Outgoing> {
        std::iter::from_fn(|| inbox.try_recv().ok()).collect()
    }

    #[test]
    fn join_sends_welcome_then_snapshots_with_acks() {
        let mut game = game();
        let (id, mut inbox) = join(&mut game);
        let Some(Outgoing::Reliable(welcome)) = drain(&mut inbox).into_iter().next() else { panic!("no welcome") };
        assert_eq!(decode::<Welcome>(&welcome).unwrap().2, id);

        game.handle(Command::Input(id, 0, vec![InputFrame(1, 0, 127, 0, 0, 0, 0, 0), InputFrame(2, 0, 127, 0, 0, 0, 0, 0)]));
        game.step();
        game.step();
        let snapshots: Vec<Snapshot> = drain(&mut inbox)
            .into_iter()
            .filter_map(|out| match out {
                Outgoing::Datagram(bytes) => decode(&bytes),
                Outgoing::Reliable(_) => None,
            })
            .collect();
        let last = snapshots.last().expect("a snapshot every SNAPSHOT_EVERY ticks");
        assert_eq!(last.3, 2, "both inputs acknowledged");
        let stickmen: Vec<_> = last.5.iter().filter(|e| e.1 == KIND_STICKMAN).collect();
        assert_eq!(stickmen.len(), 1);
        assert!(stickmen[0].4 < 0, "walked towards -Z");
    }

    #[test]
    fn room_rejects_players_beyond_capacity() {
        let mut game = game();
        let inboxes: Vec<_> = (0..MAX_PLAYERS).map(|_| join(&mut game)).collect();
        let (outbox, _inbox) = mpsc::channel(4);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        assert_eq!(response.try_recv().unwrap(), None);
        drop(inboxes);
    }

    #[test]
    fn leave_is_broadcast_to_the_others() {
        let mut game = game();
        let (first, _a) = join(&mut game);
        let (_, mut b) = join(&mut game);
        drain(&mut b);
        game.handle(Command::Leave(first));
        let Some(Outgoing::Reliable(leave)) = drain(&mut b).into_iter().next() else { panic!("no leave") };
        assert_eq!(decode::<Leave>(&leave), Some(Leave(S_LEAVE, first)));
        assert_eq!(game.entity_states().iter().filter(|e| e.1 == KIND_STICKMAN).count(), 1);
    }
}
