//! The authoritative game loop: one task owns the whole world and steps it at a fixed rate.
//! Sessions talk to it only through `Command`s in and `Outgoing` messages out.

use crate::entities::With;
use crate::jitter::JitterBuffer;
use crate::mechanics::{Mechanic, default_mechanics};
use sim::{CarControls, Character, Obj, Physics};
use crate::replication::{Everything, Own, Replica};
use crate::rng::Rng;
use crate::scenario::Scenario;
use anyhow::Result;
use bytes::Bytes;
use shared::carve::{Cut, Shell, Tri};
use shared::consts::*;
use shared::items::{HOTBAR_SLOTS, STANDARD_LOADOUT, def};
use shared::protocol::*;
use shared::quant::{from_cm, pack_quat, to_axis, to_cm, to_cm_i16, to_milli_i16, to_mrad};
use shared::world::{Building, ObjId, Tree, WorldFile};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

mod catch_up;
mod construct;
mod craters;
pub use construct::{HALF_CUBE, cube_centre};
pub use craters::Strike;
pub(crate) mod feedback;
mod replicate;
mod snapshot;
mod state;

pub use crate::events::{Damaged, Died, Noise};
pub use state::*;

/// `object` as a `Destroyed` entry.
fn destroyed_entry(object: Obj, yaw: i16, force: u8) -> DestroyedObject {
    let (kind, id) = match object {
        Obj::Tree(id) => (OBJ_TREE, id),
        Obj::Building(id) => (OBJ_BUILDING, id),
        Obj::Line(id) => (OBJ_LINE, id),
    };
    DestroyedObject { kind, col: id.col, row: id.row, index: id.index, yaw, force }
}

/// Player `id`'s `Profiles` entry: who they are, and which side.
fn profile_entry(world: &World, id: u32) -> ProfileEntry {
    let player = &world.entities.of::<Player>(id);
    ProfileEntry { id, name: player.name.clone(), color: player.color, team: player.team, bot: world.entities.has::<Bot>(id) || crate::mechanics::is_zombie(world, id) }
}

/// Building `id`'s damage `stage`, and where a blast marked it, as a `Damaged` entry.
fn damaged_entry(id: ObjId, stage: u8, mark: Option<[f32; 3]>) -> DamagedBuilding {
    DamagedBuilding { col: id.col, row: id.row, index: id.index, stage, mark: mark.map(|p| p.map(to_cm)) }
}

/// Building `id`'s cells `blasted` and `fell`, by a blast `at` (cm), as a `Carved` entry.
fn carved_entry(id: ObjId, point: [i32; 3], blasted: Vec<u32>, fell: Vec<u32>) -> CarvedBuilding {
    CarvedBuilding { col: id.col, row: id.row, index: id.index, point, blasted, fell }
}

/// A `Built` message: the cubes `placed`, `removed` and `fell`, as lattice places.
fn built_message(live: bool, placed: Vec<(shared::construct::Pos, shared::construct::Cube)>, removed: Vec<shared::construct::Pos>, fell: Vec<shared::construct::Pos>) -> Built {
    let at = |[x, y, z]: shared::construct::Pos| CubePos { x, y, z };
    Built {
        tag: S_BUILT,
        live,
        placed: placed.into_iter().map(|([x, y, z], cube)| BuiltCube { x, y, z, color: cube.colour, material: cube.material }).collect(),
        removed: removed.into_iter().map(at).collect(),
        fell: fell.into_iter().map(at).collect(),
    }
}

pub struct Game {
    pub world: World,
    mechanics: Vec<Box<dyn Mechanic>>,
    outboxes: BTreeMap<u32, mpsc::Sender<Outgoing>>,
    /// What each player has been sent, to diff their next snapshot against.
    replicas: BTreeMap<u32, Replica>,
    /// Players whose outbox rejected a reliable message this tick. Reliable messages carry state
    /// that is never repeated (kills, destroyed, damaged), so a client that missed one is out of
    /// sync for good; it is disconnected and rejoins with the catch-up instead.
    stalled: Vec<u32>,
    world_hash: u32,
    /// The client build this server hands out, as `welcome` names it (`served_build` in `main.rs`),
    /// or 0 when it serves none: the client then skips the check.
    pub build: u32,
    /// The world's scenario, if it has one (see `scenario`).
    pub scenario: Option<Scenario>,
}

/// Queues a reliable message, noting the player as stalled when the outbox is full. A closed one
/// needs no note: its session is ending and removes the player itself.
fn send_reliable(outbox: &mpsc::Sender<Outgoing>, id: u32, bytes: &Bytes, stalled: &mut Vec<u32>) {
    if let Err(mpsc::error::TrySendError::Full(_)) = outbox.try_send(Outgoing::Reliable(bytes.clone())) {
        stalled.push(id);
    }
}

impl Game {
    pub fn new(map: WorldFile, world_hash: u32) -> Result<Self> {
        let mut game = Self {
            world: World {
                tick: 0,
                physics: Physics::new(&map)?,
                trees_by_id: map.tree_ids.iter().enumerate().filter(|&(index, _)| map.trees[index].solid).map(|(index, &id)| (id, index)).collect(),
                lines_by_id: shared::world::line_pieces(&map.lines).into_iter().map(|piece| (piece.id, piece)).collect(),
                buildings_by_id: map.building_ids.iter().enumerate().map(|(index, &id)| (id, index)).collect(),
                object_hp: HashMap::new(),
                destroyed: Vec::new(),
                felled: Default::default(),
                fresh: Default::default(),
                carves: HashMap::new(),
                strained: HashMap::new(),
                reshaped: Default::default(),
                rubble: Vec::new(),
                craters: Default::default(),
                dug_tiles: Default::default(),
                pristine: Vec::new(),
                construct: Default::default(),
                construct_dirty: Default::default(),
                backpack: None,
                deliveries: None,
                bus_calls: Vec::new(),
                traffic_calls: Vec::new(),
                traffic: 0.0,
                map,
                entities: Default::default(),
                nav: Default::default(),
                next_id: 1,
                // A test's world is the same every run, so a test that fails does so every time.
                rng: if cfg!(test) { Rng::from_seed(0x9e37_79b9_7f4a_7c15) } else { Rng::from_time() },
                shots: Vec::new(),
                armor_hits: Vec::new(),
                marks: Vec::new(),
                bullets: Vec::new(),
                hooks: Default::default(),
            },
            mechanics: default_mechanics(),
            outboxes: BTreeMap::new(),
            stalled: Vec::new(),
            replicas: BTreeMap::new(),
            world_hash,
            build: 0,
            scenario: None,
        };
        let names: Vec<&str> = game.mechanics.iter().map(|mechanic| mechanic.name()).collect();
        tracing::debug!(mechanics = ?names, "room created");
        for mechanic in &game.mechanics {
            mechanic.hooks(&mut game.world.hooks);
        }
        for mechanic in &mut game.mechanics {
            mechanic.on_start(&mut game.world);
        }
        for bot in std::mem::take(&mut game.world.map.placed_bots) {
            let route = bot.route.iter().map(|point| point.map(from_cm)).collect();
            match game.add_bot(bot.at.map(from_cm), bot.team, route) {
                Some(id) => game.world.entities.get_mut::<Bot>(id).expect("just added").aim.skill = bot.skill.clamp(0.0, 1.0),
                None => tracing::warn!(MAX_BOTS, "too many bots, placed bot skipped"),
            }
        }
        Ok(game)
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Join { outbox, reply, profile } => {
                let _ = reply.send(self.join(outbox, profile));
            }
            Command::Leave(id) => self.leave(id),
            Command::Rtc { from, to, kind, json } => {
                // Signaling is retried by WebRTC itself, so a full outbox just sheds it: unlike
                // game state it must not cost the receiver their session.
                if from != to
                    && self.world.entities.has::<Player>(from)
                    && let Some(outbox) = self.outboxes.get(&to)
                {
                    let _ = outbox.try_send(Outgoing::Reliable(encode(&Rtc { tag: S_RTC, peer: from, kind, json }).into()));
                }
            }
            Command::Hands(id, hands) => {
                if let Some(mut player) = self.world.entities.get_mut::<Player>(id) {
                    player.fists = Some(hands);
                    player.fists_age = 0;
                }
            }
            Command::Phone(id, phone) => crate::mechanics::phone(&mut self.world, id, phone),
            Command::Input(id, baseline, frames) => {
                if let Some(mut player) = self.world.entities.get_mut::<Player>(id) {
                    player.inputs.push(&frames);
                }
                if let Some(replica) = self.replicas.get_mut(&id) {
                    replica.acknowledge(baseline);
                }
            }
        }
    }

    fn join(&mut self, outbox: mpsc::Sender<Outgoing>, profile: Profile) -> Option<u32> {
        // People are the players with a connection; bots have their own room (`MAX_BOTS`).
        if self.outboxes.len() >= MAX_PLAYERS {
            return None;
        }
        let id = self.world.alloc_id();
        let welcome = encode(&Welcome { tag: S_WELCOME, version: PROTOCOL_VERSION, id, tick_rate: TICK_RATE as u8, snapshot_every: SNAPSHOT_EVERY as u8, world_hash: self.world_hash, build: self.build });
        if outbox.try_send(Outgoing::Reliable(welcome.into())).is_err() {
            return None;
        }
        let loadout = profile.loadout.map_or_else(starting_hotbar, hotbar_of);
        let name = if profile.name.is_empty() { format!("Player {id}") } else { profile.name };
        let color = profile.color.unwrap_or((id % PLAYER_COLOR_COUNT as u32) as u8);
        let spawns = profile.spawns.into_iter().filter(|&point| crate::mechanics::spawn_usable(&self.world.map, point)).collect();
        self.world.entities.spawn(id, Player::new(name, color, TEAM_NONE, spawns, loadout));
        self.world.entities.insert(id, soldier_life());
        for message in self.catch_up() {
            send_reliable(&outbox, id, &Bytes::from(message), &mut self.stalled);
        }
        let joiner = encode(&Profiles { tag: S_PROFILES, players: vec![profile_entry(&self.world, id)] });
        self.broadcast(joiner);
        self.outboxes.insert(id, outbox);
        self.replicas.insert(id, Replica::default());
        for mechanic in &mut self.mechanics {
            mechanic.on_join(&mut self.world, id);
        }
        let player = &self.world.entities.of::<Player>(id);
        tracing::info!(id, name = %player.name, spawns = player.spawns.len(), players = self.world.entities.count::<Player>(), "player joined");
        Some(id)
    }

    fn leave(&mut self, id: u32) {
        if !self.world.entities.has::<Player>(id) {
            return;
        }
        for mechanic in &mut self.mechanics {
            mechanic.on_leave(&mut self.world, id);
        }
        // How their inputs and snapshots fared, for telling a bad connection from a bad client.
        let (frames_dropped, underruns) = self.world.entities.get::<Player>(id).map_or((0, 0), |player| (player.inputs.dropped, player.inputs.underruns));
        let snapshots_skipped = self.replicas.get(&id).map_or(0, |replica| replica.skipped);
        self.world.entities.despawn(id);
        self.outboxes.remove(&id);
        self.replicas.remove(&id);
        self.broadcast(encode(&Leave { tag: S_LEAVE, id }));
        tracing::info!(id, frames_dropped, underruns, snapshots_skipped, players = self.world.entities.count::<Player>(), "player left");
    }

    /// The room's mechanic of type `M`, for a test that looks inside it.
    #[cfg(test)]
    pub fn mechanic<M: 'static>(&self) -> Option<&M> {
        self.mechanics.iter().find_map(|mechanic| mechanic.inside()?.downcast_ref::<M>())
    }

    /// Runs `with` on the room's mechanic of type `M` and its world, for a test that reaches into it.
    #[cfg(test)]
    pub fn with_mechanic<M: 'static, R>(&mut self, with: impl FnOnce(&mut M, &mut World) -> R) -> R {
        let mechanic = self.mechanics.iter_mut().find_map(|mechanic| mechanic.inside_mut()?.downcast_mut::<M>()).expect("the room runs it");
        with(mechanic, &mut self.world)
    }

    /// Adds a bot living at `home` on `team` (`TEAM_BOTS` or above) that walks round `route`
    /// (`[x, z]`, metres; empty makes one up). It is a player with no connection: `Bots` writes its
    /// input, and everything else treats it as anyone else. `None` when there are `MAX_BOTS` already.
    pub fn add_bot(&mut self, home: [f32; 2], team: u8, route: Vec<[f32; 2]>) -> Option<u32> {
        let name = format!("Bot {}", self.world.entities.count::<Bot>() + 1);
        self.add_bot_as(name, home, team, route)
    }

    /// `add_bot`, under `name`.
    pub fn add_bot_as(&mut self, name: String, home: [f32; 2], team: u8, route: Vec<[f32; 2]>) -> Option<u32> {
        if self.world.entities.count::<Bot>() >= MAX_BOTS {
            return None;
        }
        let id = self.world.alloc_id();
        let team = team.max(TEAM_BOTS);
        // One outfit per team, so a squad reads as one.
        let color = team.wrapping_mul(5) % PLAYER_COLOR_COUNT;
        self.world.entities.spawn(id, Player::new(name, color, team, vec![home.map(to_cm)], starting_hotbar()));
        let temper = Temper::rolled(|| self.world.rng.next_f64() as f32);
        self.world.entities.insert(id, (soldier_life(), Bot { temper, ..Bot::new(home, route) }));
        for mechanic in &mut self.mechanics {
            mechanic.on_join(&mut self.world, id);
        }
        // Scores are only ever sent whole, to joiners; a bot's starts at zero on every client anyway.
        self.broadcast(encode(&Profiles { tag: S_PROFILES, players: vec![profile_entry(&self.world, id)] }));
        tracing::info!(id, team, x = home[0], z = home[1], "bot added");
        Some(id)
    }

    /// Raises a zombie at `at` (`[x, z]`, metres) on `TEAM_ZOMBIES`: a player with no connection and
    /// empty hands, whose input `Zombies` writes (ZOMBIES.md). Each has a shambling pace of its own,
    /// and one in `ZOMBIE_RUNNER_ONE_IN` runs. `None` when there are `MAX_ZOMBIES` up already.
    pub fn add_zombie(&mut self, at: [f32; 2]) -> Option<u32> {
        use crate::mechanics::Zombie;
        if self.world.entities.count::<Zombie>() >= MAX_ZOMBIES {
            return None;
        }
        let id = self.world.alloc_id();
        let rng = &mut self.world.rng;
        let pace = (f64::from(ZOMBIE_PACE[0]) + rng.next_f64() * f64::from(ZOMBIE_PACE[1] - ZOMBIE_PACE[0])).round() as i8;
        let runner = rng.next_u64().is_multiple_of(u64::from(ZOMBIE_RUNNER_ONE_IN));
        let yaw = rng.next_f64() as f32 * std::f32::consts::TAU;
        self.world.entities.spawn(id, Player::new("Zombie".into(), 0, TEAM_ZOMBIES, vec![at.map(to_cm)], hotbar_of([shared::items::ITEM_NONE; HOTBAR_SLOTS])));
        self.world.entities.insert(id, (soldier_life(), Zombie::new(pace, runner, yaw)));
        for mechanic in &mut self.mechanics {
            mechanic.on_join(&mut self.world, id);
        }
        self.broadcast(encode(&Profiles { tag: S_PROFILES, players: vec![profile_entry(&self.world, id)] }));
        Some(id)
    }

    /// Answers what `Buses` asked for this tick: a driver for each fresh bus, and the dead ones gone.
    fn staff_buses(&mut self) {
        use crate::mechanics::{Bus, BusCall, BusDriver, seat};
        for call in std::mem::take(&mut self.world.bus_calls) {
            match call {
                BusCall::Hire(vid) => {
                    let Some(pose) = self.world.physics.car_pose(vid) else { continue };
                    let id = self.world.alloc_id();
                    let at = [pose.position[0], pose.position[2]];
                    // A civilian: on no team, with empty hands, and no coming back from the dead.
                    self.world.entities.spawn(id, Player::new("Bus driver".into(), 0, TEAM_NONE, vec![at.map(to_cm)], hotbar_of([shared::items::ITEM_NONE; HOTBAR_SLOTS])));
                    self.world.entities.insert(id, (soldier_life(), BusDriver));
                    self.world.entities.remove::<crate::entities::Respawn>(id);
                    for mechanic in &mut self.mechanics {
                        mechanic.on_join(&mut self.world, id);
                    }
                    self.broadcast(encode(&Profiles { tag: S_PROFILES, players: vec![profile_entry(&self.world, id)] }));
                    if seat(&mut self.world, id, vid, 0)
                        && let Some(mut bus) = self.world.entities.get_mut::<Bus>(vid)
                    {
                        bus.driver = Some(id);
                    } else {
                        // Someone took the seat first: the bus is theirs, and `Buses` sees it has no driver.
                        self.leave(id);
                    }
                }
                BusCall::Dismiss(id) => {
                    if self.world.entities.has::<BusDriver>(id) {
                        self.leave(id);
                    }
                }
            }
        }
    }

    /// Answers what `Traffic` and `Civilians` asked for this tick: drivers out of their cars made
    /// civilians, under the same id so that everyone sees the same person get out, and civilians gone.
    fn street(&mut self) {
        use crate::entities::Health;
        use crate::mechanics::{Civilian, TrafficCall, Wounds};
        for call in std::mem::take(&mut self.world.traffic_calls) {
            match call {
                TrafficCall::Afoot { driver: id, feet, yaw, hands_up, threat } => {
                    // What they took at the wheel goes with them.
                    let hp = self.world.entities.get::<Health>(id).map_or(PLAYER_MAX_HP, |health| health.hp);
                    let wounds = self.world.entities.get::<Wounds>(id).copied();
                    self.world.entities.despawn(id);
                    let rng = &mut self.world.rng;
                    let color = (rng.next_u64() % u64::from(PLAYER_COLOR_COUNT)) as u8;
                    let slant = (rng.next_f64() as f32 * 2.0 - 1.0) * crate::mechanics::CIVILIAN_SLANT;
                    // A civilian: on no team, with empty hands, and no coming back from the dead.
                    self.world.entities.spawn(id, Player::new("Civilian".into(), color, TEAM_NONE, vec![[feet[0], feet[2]].map(to_cm)], hotbar_of([shared::items::ITEM_NONE; HOTBAR_SLOTS])));
                    self.world.entities.insert(id, (soldier_life(), Civilian::new(yaw, threat, hands_up, slant)));
                    self.world.entities.remove::<crate::entities::Respawn>(id);
                    for mechanic in &mut self.mechanics {
                        mechanic.on_join(&mut self.world, id);
                    }
                    // Beside the car's door, not at a spawn point.
                    if let Some(mut player) = self.world.entities.get_mut::<Player>(id) {
                        player.character.position = [feet[0], feet[1] + sim::STANDING_Y, feet[2]];
                        player.character.yaw = yaw;
                        player.surrendered = hands_up > 0;
                    }
                    if let Some(mut health) = self.world.entities.get_mut::<Health>(id) {
                        health.hp = hp;
                    }
                    if let Some(wounds) = wounds {
                        self.world.entities.insert(id, wounds);
                    }
                    self.broadcast(encode(&Profiles { tag: S_PROFILES, players: vec![profile_entry(&self.world, id)] }));
                }
                TrafficCall::Gone(id) => {
                    if self.world.entities.has::<Civilian>(id) {
                        self.leave(id);
                    }
                }
            }
        }
    }

    /// Takes zombie `id` out of the game, as if it had left. Anyone else is left alone.
    pub fn remove_zombie(&mut self, id: u32) {
        if crate::mechanics::is_zombie(&self.world, id) {
            self.leave(id);
        }
    }

    /// Parks a tank at `at` and puts a crew of two bots of `team` in it, named `names`: a driver
    /// who drives it round `route` (`[x, z]`, metres; empty is along the nearest road) and a
    /// gunner. Each lives beside where the tank stands, and gets back in when it comes back from
    /// dying. Returns the tank and its driver and gunner, or `None` when there is no room for two
    /// more bots or for the tank, or it does not fit there.
    pub fn add_crew(&mut self, names: [String; 2], at: [f32; 2], team: u8, route: Vec<[f32; 2]>) -> Option<(u32, [u32; 2])> {
        use crate::mechanics::{DRIVER, GUNNER, add_tank, nearest_waypoint, road_route, seat};
        if self.world.entities.count::<Bot>() + 2 > MAX_BOTS {
            return None;
        }
        let route = if route.is_empty() { road_route(&self.world.map, at) } else { route };
        let next = nearest_waypoint(&route, at);
        // Facing the first waypoint, or along the route when it starts right there.
        let toward = route.get(next).filter(|p| (p[0] - at[0]).hypot(p[1] - at[1]) > 3.0).or_else(|| route.get((next + 1) % route.len()));
        let yaw = toward.map_or(0.0, |p| (-(p[0] - at[0])).atan2(-(p[1] - at[1])));
        let vid = add_tank(&mut self.world, at, yaw)?;
        // Their homes: beside the tank, clear of its hull.
        let (sin, cos) = yaw.sin_cos();
        let home = [at[0] + cos * 4.0, at[1] - sin * 4.0];
        let [driver_name, gunner_name] = names;
        let mut crew = [0; 2];
        for (slot, (name, index)) in [(driver_name, DRIVER), (gunner_name, GUNNER)].into_iter().enumerate() {
            let id = self.add_bot_as(name, home, team, route.clone()).expect("room checked above");
            let mut bot = self.world.entities.get_mut::<Bot>(id).expect("just added");
            bot.crew = Some(Crew { vehicle: vid, seat: index });
            bot.next = next;
            bot.yaw = yaw;
            seat(&mut self.world, id, vid, index);
            crew[slot] = id;
        }
        tracing::info!(vid, driver = crew[0], gunner = crew[1], "tank crew added");
        Some((vid, crew))
    }

    /// Takes bot `id` out of the game, as if it had left. Anyone else is left alone.
    pub fn remove_bot(&mut self, id: u32) {
        if self.world.entities.has::<Bot>(id) {
            self.leave(id);
        }
    }

    /// Tells everyone something: a `Notice` of `kind` (`NOTICE_*`).
    pub fn notice(&mut self, kind: u8, text: &str) {
        self.broadcast(encode(&Notice { tag: S_NOTICE, kind, text: text.into() }));
    }

    /// One fixed simulation step.
    pub fn step(&mut self) {
        self.step_timed(None);
    }

    /// `step`, adding to `timed` when given (for the benches) what each mechanic's `on_tick` took, one
    /// entry a mechanic in order, then what came after them up to the snapshots, then the snapshots.
    pub(crate) fn step_timed(&mut self, mut timed: Option<&mut [std::time::Duration]>) {
        let count = self.mechanics.len();
        for id in self.world.entities.ids::<With<Player>>() {
            let mut player = self.world.entities.of_mut::<Player>(id);
            let consumed = player.inputs.pop();
            player.input = consumed.input;
            if consumed.fresh {
                player.ack = consumed.input.seq();
            }
        }
        for (index, mechanic) in self.mechanics.iter_mut().enumerate() {
            let started = timed.is_some().then(std::time::Instant::now);
            mechanic.on_tick(&mut self.world, TICK_DT);
            if let (Some(timed), Some(started)) = (timed.as_deref_mut(), started) {
                timed[index] += started.elapsed();
            }
        }
        let after = timed.is_some().then(std::time::Instant::now);
        self.staff_buses();
        self.street();
        if let Some(mut scenario) = self.scenario.take() {
            scenario.tick(self);
            self.scenario = Some(scenario);
        }
        for mechanic in &mut self.mechanics {
            mechanic.react(&mut self.world);
        }
        self.broadcast_kills();
        self.send_hits();
        self.send_armor_hits();
        self.broadcast_destroyed();
        self.broadcast_damaged();
        self.broadcast_carved();
        self.broadcast_cratered();
        self.broadcast_built();
        self.broadcast_gates();
        self.broadcast_backpack();
        self.send_deliveries();
        for text in std::mem::take(&mut self.world.fresh.said) {
            self.notice(NOTICE_SAY, &text);
        }
        self.drop_stalled();
        crate::events::update(&mut self.world.entities.ecs);
        self.world.tick = self.world.tick.wrapping_add(1);
        crate::mechanics::record_trails(&mut self.world);
        if let (Some(timed), Some(after)) = (timed.as_deref_mut(), after) {
            timed[count] += after.elapsed();
        }
        if self.world.tick.is_multiple_of(SNAPSHOT_EVERY) {
            let started = timed.is_some().then(std::time::Instant::now);
            self.send_snapshots();
            if let (Some(timed), Some(started)) = (timed, started) {
                timed[count + 1] += started.elapsed();
            }
        }
    }

    /// Queues `message` for everyone, reliably.
    fn broadcast(&mut self, message: Vec<u8>) {
        let bytes = Bytes::from(message);
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &bytes, &mut self.stalled);
        }
    }

    /// Disconnects players that missed a reliable message; dropping the outbox ends their session.
    fn drop_stalled(&mut self) {
        for id in std::mem::take(&mut self.stalled) {
            if self.outboxes.contains_key(&id) {
                tracing::warn!(id, "outbox full, dropping player");
                self.leave(id);
            }
        }
    }

    /// Tells everyone about the buildings and trees destroyed this tick, as one message.
    fn broadcast_destroyed(&mut self) {
        if self.world.fresh.destroyed.is_empty() {
            return;
        }
        let entries = self.world.fresh.destroyed.drain(..).map(|(object, yaw, force)| destroyed_entry(object, yaw, force)).collect();
        self.broadcast(encode(&Destroyed { tag: S_DESTROYED, objects: entries }));
    }

    /// Tells everyone which cells blasts took out of buildings this tick, as one message.
    fn broadcast_carved(&mut self) {
        self.world.flush_buildings();
        if self.world.fresh.carved.is_empty() {
            return;
        }
        let entries = self.world.fresh.carved.drain(..).map(|(id, at, cut)| carved_entry(id, at, cut.blasted, cut.fell)).collect();
        self.broadcast(encode(&Carved { tag: S_CARVED, live: true, buildings: entries }));
    }

    /// Tells everyone which craters shells dug this tick, as one message.
    fn broadcast_cratered(&mut self) {
        if self.world.fresh.craters.is_empty() {
            return;
        }
        let craters = std::mem::take(&mut self.world.fresh.craters);
        self.broadcast(encode(&Cratered { tag: S_CRATERED, live: true, craters }));
    }

    /// Gives the cubes that changed this tick their colliders, and tells everyone what changed, as one message.
    fn broadcast_built(&mut self) {
        self.world.flush_construct();
        if self.world.fresh.built.is_empty() {
            return;
        }
        let FreshBuilt { placed, removed, fell } = std::mem::take(&mut self.world.fresh.built);
        self.broadcast(encode(&built_message(true, placed, removed, fell)));
    }

    /// Tells everyone which gates were opened or shut this tick, as one message.
    fn broadcast_gates(&mut self) {
        if self.world.fresh.gates.is_empty() {
            return;
        }
        let gates = self.world.fresh.gates.drain(..).map(|(id, door, open)| GateState { col: id.col, row: id.row, index: id.index, door: door as u16, open }).collect();
        self.broadcast(encode(&Gates { tag: S_GATES, live: true, gates }));
    }

    /// Tells everyone where the backpack's circle is, when its mechanic has something to say.
    fn broadcast_backpack(&mut self) {
        let Some(circle) = self.world.backpack.as_mut().and_then(|state| state.fresh.take()) else { return };
        self.broadcast(encode(&circle));
    }

    /// Tells everyone who delivered what, then each player whose phone changed what it shows now: in
    /// that order, so a job gone from a phone is known to have been delivered, not to have spoiled.
    fn send_deliveries(&mut self) {
        let Some(state) = self.world.deliveries.as_mut() else { return };
        let (phones, delivered) = (std::mem::take(&mut state.phones), std::mem::take(&mut state.delivered));
        for message in delivered {
            self.broadcast(encode(&message));
        }
        for (to, jobs) in phones {
            if let Some(outbox) = self.outboxes.get(&to) {
                send_reliable(outbox, to, &Bytes::from(encode(&jobs)), &mut self.stalled);
            }
        }
    }

    /// Tells everyone about the buildings hurt this tick, as one message.
    fn broadcast_damaged(&mut self) {
        if self.world.fresh.damaged.is_empty() {
            return;
        }
        let entries = self.world.fresh.damaged.drain(..).map(|(id, stage, mark)| damaged_entry(id, stage, mark)).collect();
        self.broadcast(encode(&shared::protocol::Damaged { tag: S_DAMAGED, buildings: entries }));
    }

    /// Tells each player about the hits they dealt or took this tick, amounts summed per kind.
    fn send_hits(&mut self) {
        let mut merged: Vec<(u32, Hit)> = Vec::new();
        for (to, hit) in feedback::hits(&self.world) {
            match merged.iter_mut().find(|(t, h)| *t == to && h.role == hit.role && h.kind == hit.kind && h.target == hit.target) {
                Some((_, sum)) => sum.amount = sum.amount.saturating_add(hit.amount),
                None => merged.push((to, hit)),
            }
        }
        for (to, hit) in merged {
            if let Some(outbox) = self.outboxes.get(&to) {
                send_reliable(outbox, to, &Bytes::from(encode(&hit)), &mut self.stalled);
            }
        }
    }

    /// Tells whoever fired each round that met a vehicle's armour this tick, and its crew, what it did.
    fn send_armor_hits(&mut self) {
        for hit in std::mem::take(&mut self.world.armor_hits) {
            let aboard: Vec<u32> = self.world.entities.get::<Seats>(hit.vehicle).map(|seats| seats.riders().collect()).unwrap_or_default();
            let [x, y, z] = hit.point.map(to_cm);
            let message = |role| Armor { tag: S_ARMOR, role, vehicle: hit.vehicle, result: hit.result, hurt: hit.inside.hurt, gone: hit.inside.gone, killed: hit.inside.killed, x, y, z };
            let to = hit.attacker.filter(|by| !aboard.contains(by)).map(|by| (by, HIT_DEALT)).into_iter().chain(aboard.iter().map(|&rider| (rider, HIT_TAKEN)));
            for (id, role) in to {
                if let Some(outbox) = self.outboxes.get(&id) {
                    send_reliable(outbox, id, &Bytes::from(encode(&message(role))), &mut self.stalled);
                }
            }
        }
    }

    /// Tells everyone about the kills mechanics recorded this tick.
    fn broadcast_kills(&mut self) {
        for kill in feedback::kills(&self.world) {
            self.broadcast(encode(&kill));
        }
    }

}


/// Runs the game loop until every command sender is gone.
pub async fn run(mut game: Game, mut commands: mpsc::Receiver<Command>) {
    let mut ticker = interval(Duration::from_secs_f64(1.0 / TICK_RATE as f64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);
    loop {
        // An empty room holds still until a command comes (a join), so its bots, scenario and
        // physics cost nothing while no one is there to see them.
        if game.outboxes.is_empty() {
            tracing::info!(tick = game.world.tick, "room empty, paused");
            while game.outboxes.is_empty() {
                match commands.recv().await {
                    Some(command) => game.handle(command),
                    None => return,
                }
            }
            ticker.reset();
            tracing::info!(tick = game.world.tick, "room resumed");
        }
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
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        (response.try_recv().unwrap().unwrap(), inbox)
    }

    fn drain(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Outgoing> {
        std::iter::from_fn(|| inbox.try_recv().ok()).collect()
    }

    /// A 100 m square of ground 3 m up, samples 5 m apart.
    fn hilly_game() -> Game {
        let ground = shared::terrain::Ground { origin: -10_000, cell: 500, side: 41, heights: vec![300; 41 * 41] };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], tile_size: 20_000, ground, ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    /// The `Cratered` messages in `outgoing`, as `(live, craters)`.
    fn cratered(outgoing: Vec<Outgoing>) -> Vec<(bool, Vec<CraterEntry>)> {
        outgoing
            .into_iter()
            .filter_map(|out| match out {
                Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_CRATERED) => decode::<Cratered>(&bytes).map(|c| (c.live, c.craters)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_shell_on_the_ground_digs_a_crater_everyone_is_told_of_and_a_joiner_catches_up_on() {
        let mut game = hilly_game();
        let (_, mut first) = join(&mut game);
        drain(&mut first);
        let ground = game.world.map.ground.clone();
        // In the air, on a roof's height, nothing happens.
        assert!(!game.world.dig_crater([0.0, 3.0 + CRATER_REACH + 1.0, 0.0], 5.0, 1.0, Strike::DOWN));
        assert_eq!(game.world.map.ground, ground);
        assert!(game.world.dig_crater([0.0, 3.2, 0.0], 5.6, 1.0, Strike::DOWN));
        assert_eq!(game.world.map.ground.height_at(0.0, 0.0), 2.0, "a metre down in the middle");
        assert_eq!(game.world.map.ground.height_at(20.0, 20.0), 3.0, "far away it is as it was");
        game.step();
        let told = cratered(drain(&mut first));
        assert_eq!(told.len(), 1);
        assert!(told[0].0, "news to the ones already here");
        assert_eq!(told[0].1, vec![CraterEntry { x: 0, z: 0, radius: 560, depth: 100, yaw: 0, dip: 255, contact: 221, solid: false, age: 0 }]);
        // Nothing is told twice.
        game.step();
        assert!(cratered(drain(&mut first)).is_empty());
        let (_, mut late) = join(&mut game);
        let caught_up = cratered(drain(&mut late));
        assert_eq!(caught_up.len(), 1);
        assert!(!caught_up[0].0, "state, not news");
        assert_eq!(caught_up[0].1.len(), 1);
    }

    /// The `Dug` tiles in `outgoing`.
    fn dug(outgoing: &[Outgoing]) -> Vec<DugTile> {
        outgoing
            .iter()
            .filter_map(|out| match out {
                Outgoing::Reliable(bytes) if peek_tag(bytes) == Some(S_DUG) => decode::<Dug>(bytes).map(|d| d.tiles),
                _ => None,
            })
            .flatten()
            .collect()
    }

    #[test]
    fn a_joiner_gets_the_ground_as_dug_after_more_craters_than_are_kept_and_marks_only_while_fresh() {
        let mut game = hilly_game();
        let mut seed = 7u32;
        let mut random = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 180.0 - 90.0
        };
        for _ in 0..CRATER_MAX_KEPT + 300 {
            let (x, z) = (random(), random());
            game.world.dig_crater([x, game.world.map.ground.height_at(x, z), z], 3.5, 0.5, Strike::DOWN);
        }
        let (_, mut late) = join(&mut game);
        let told = drain(&mut late);
        // The one tile, laid on the pristine ground, is the server's ground sample for sample.
        let tiles = dug(&told);
        assert_eq!(tiles.len(), 1);
        let mut heights = game.world.pristine.clone();
        let (mut at, mut runs) = (0usize, tiles[0].runs.iter().copied());
        while let (Some(skip), Some(count)) = (runs.next(), runs.next()) {
            at += skip as usize;
            for _ in 0..count {
                heights[at] += runs.next().unwrap();
                at += 1;
            }
        }
        assert_eq!(heights, game.world.map.ground.heights);
        let marks = cratered(told);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].1.len(), CRATER_MAX_KEPT);
        // Long after, the holes are still sent but the marks have faded.
        game.world.tick += CRATER_FADE_SECONDS * TICK_RATE;
        let (_, mut later) = join(&mut game);
        let told = drain(&mut later);
        assert_eq!(dug(&told), tiles);
        assert!(cratered(told).is_empty());
    }

    #[test]
    fn a_crater_on_a_seam_marks_both_tiles_dug() {
        let ground = shared::terrain::Ground { origin: -10_000, cell: 500, side: 41, heights: vec![300; 41 * 41] };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], tile_size: 10_000, ground, ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        // On the seam x = 0 between tiles (0, 0) and (1, 0).
        assert!(game.world.dig_crater([0.0, 3.0, -30.0], 5.6, 1.0, Strike::DOWN));
        assert_eq!(game.world.dug_tiles.iter().copied().collect::<Vec<_>>(), vec![(0, 0), (1, 0)]);
        assert_eq!(game.world.dug_ground().iter().map(|t| (t.col, t.row)).collect::<Vec<_>>(), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn craters_pile_up_to_a_depth_and_no_further() {
        let mut game = hilly_game();
        for _ in 0..20 {
            game.world.dig_crater([0.0, game.world.map.ground.height_at(0.0, 0.0), 0.0], 5.6, 1.0, Strike::DOWN);
        }
        assert_eq!(game.world.map.ground.height_at(0.0, 0.0), 0.0f32.max(3.0 - CRATER_MAX_DEPTH));
    }

    #[test]
    fn a_burst_above_the_ground_or_on_a_vehicle_marks_a_crater_less_in_the_ground() {
        let mut game = hilly_game();
        let strike = Strike { dir: [1.0, -1.0, 0.0], direct: false, solid: false };
        assert!(game.world.dig_crater([0.0, 3.0, 0.0], 5.6, 1.0, strike));
        assert!(game.world.dig_crater([40.0, 3.0 + CRATER_REACH / 2.0, 0.0], 5.6, 1.0, strike));
        assert!(game.world.dig_crater([-40.0, 3.0, 0.0], 5.6, 1.0, Strike { direct: true, ..strike }));
        // Under the surface counts as in it.
        assert!(game.world.dig_crater([0.0, 2.5, 40.0], 5.6, 1.0, strike));
        let contact: Vec<u8> = game.world.craters.iter().map(|&(_, crater)| crater.contact).collect();
        assert_eq!(contact, vec![255, 128, 128, 255]);
        // The hole is the same however it was dug.
        let depth: Vec<i32> = game.world.craters.iter().map(|&(_, crater)| crater.depth).collect();
        assert_eq!(depth, vec![100; 4]);
    }

    #[test]
    fn join_sends_welcome_then_snapshots_with_acks() {
        let mut game = game();
        let (id, mut inbox) = join(&mut game);
        let Some(Outgoing::Reliable(welcome)) = drain(&mut inbox).into_iter().next() else { panic!("no welcome") };
        assert_eq!(decode::<Welcome>(&welcome).unwrap().id, id);

        game.handle(Command::Input(id, 0, vec![InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0, collective: 0 }, InputFrame { seq: 2, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0, collective: 0 }]));
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
        assert_eq!(last.ack, 2, "both inputs acknowledged");
        // No snapshot was acknowledged, so each brings everything whole: the soldier among it.
        let scene = shared::replicated::apply(&[], &last.updates, &last.removed);
        let soldiers: Vec<_> = scene.iter().filter(|(_, e)| e.kind.as_ref().is_some_and(|kind| kind.kind == KIND_SOLDIER)).collect();
        assert_eq!(soldiers.len(), 1);
        assert!(soldiers[0].1.place.unwrap().z < 0, "walked towards -Z");
    }

    /// A `Profiles` entry as `(id, name, color, team, bot)`.
    type Told = (u32, String, u8, u8, u8);

    /// Every `Profiles` message sent.
    fn profiles(outgoing: Vec<Outgoing>) -> Vec<Vec<Told>> {
        outgoing
            .into_iter()
            .filter_map(|out| match out {
                Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_PROFILES) => decode::<Profiles>(&bytes).map(|profiles| profiles.players.into_iter().map(|p| (p.id, p.name, p.color, p.team, p.bot as u8)).collect()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_joiner_gets_everyones_profile_and_everyone_gets_theirs() {
        let mut game = game();
        let (first, mut first_inbox) = join(&mut game);
        let (outbox, mut inbox) = mpsc::channel(256);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Profile { name: "Åsa".into(), color: Some(5), ..Default::default() } });
        let second = response.try_recv().unwrap().unwrap();
        let first_colour = (first % PLAYER_COLOR_COUNT as u32) as u8;
        assert_eq!(profiles(drain(&mut inbox)), vec![vec![(first, format!("Player {first}"), first_colour, TEAM_NONE, 0), (second, "Åsa".into(), 5, TEAM_NONE, 0)]]);
        assert_eq!(profiles(drain(&mut first_inbox)).last(), Some(&vec![(second, "Åsa".into(), 5, TEAM_NONE, 0)]));
    }

    #[test]
    fn a_bot_is_announced_with_its_team() {
        let mut game = game();
        let (_, mut inbox) = join(&mut game);
        drain(&mut inbox);
        let bot = game.add_bot([0.0, 0.0], 3, vec![]).unwrap();
        assert_eq!(profiles(drain(&mut inbox)), vec![vec![(bot, "Bot 1".to_string(), (3 * 5) % PLAYER_COLOR_COUNT, 3, 1)]]);
        let (outbox, mut late) = mpsc::channel(256);
        let (reply, _response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        assert!(profiles(drain(&mut late))[0].iter().any(|entry| entry.0 == bot && entry.3 == 3 && entry.4 == 1));
    }

    #[test]
    fn room_rejects_players_beyond_capacity() {
        let mut game = game();
        let inboxes: Vec<_> = (0..MAX_PLAYERS).map(|_| join(&mut game)).collect();
        let (outbox, _inbox) = mpsc::channel(4);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        assert_eq!(response.try_recv().unwrap(), None);
        drop(inboxes);
    }

    #[test]
    fn a_player_whose_outbox_is_full_is_dropped_instead_of_missing_a_reliable() {
        let mut game = game();
        let (outbox, _inbox) = mpsc::channel(1);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        let id = response.try_recv().unwrap().unwrap();
        // The welcome already fills the one slot, so the next reliable message cannot be queued.
        game.world.fresh.destroyed.push((Obj::Tree(Default::default()), 0, 0));
        game.step();
        assert!(!game.world.entities.has::<Player>(id));
    }

    #[test]
    fn snapshots_leave_a_stalled_session_room_for_what_must_arrive() {
        let mut game = game();
        // A session held up on a slow reliable write: nothing is drained however long this goes on.
        let (id, _inbox) = {
            let (outbox, inbox) = mpsc::channel(64);
            let (reply, mut response) = oneshot::channel();
            game.handle(Command::Join { outbox, reply, profile: Default::default() });
            (response.try_recv().unwrap().unwrap(), inbox)
        };
        for _ in 0..600 {
            game.step();
        }
        game.world.fresh.destroyed.push((Obj::Tree(Default::default()), 0, 0));
        game.step();
        assert!(game.world.entities.has::<Player>(id), "a harmless backlog of snapshots cost them their session");
    }

    #[test]
    fn a_crews_own_side_cannot_hurt_its_vehicle_but_the_other_side_can() {
        use crate::entities::Health;
        use crate::mechanics::{damage_vehicle, for_tests::add_vehicle, seat};
        let mut game = game();
        let (enemy, _inbox) = join(&mut game);
        let [rider, friend] = [0.0, 8.0].map(|x| game.add_bot([x, 0.0], 3, vec![]).unwrap());
        let vid = add_vehicle(&mut game.world, KIND_JEEP, [20.0, 1.0, 0.0], 0.0);
        assert!(seat(&mut game.world, rider, vid, 0));
        let full = game.world.entities.of::<Health>(vid).hp;
        damage_vehicle(&mut game.world, vid, 10, Some(friend));
        assert_eq!(game.world.entities.of::<Health>(vid).hp, full);
        damage_vehicle(&mut game.world, vid, 10, Some(enemy));
        assert_eq!(game.world.entities.of::<Health>(vid).hp, full - 10);
    }

    #[test]
    fn everyone_inside_dies_when_a_vehicle_goes_up_whoever_set_it_off() {
        use crate::mechanics::for_tests::{Harm, damage_entity};
        let mut game = game();
        let [rider, friend] = [0.0, 8.0].map(|x| game.add_bot([x, 0.0], 3, vec![]).unwrap());
        assert!(!damage_entity(&mut game.world, rider, Harm::new(PLAYER_MAX_HP, Some(friend), 0)));
        assert!(damage_entity(&mut game.world, rider, Harm::new(PLAYER_MAX_HP, Some(friend), 0).unsparing()));
    }

    #[test]
    fn rtc_is_relayed_with_the_sender_filled_in() {
        let mut game = game();
        let (a, _a) = join(&mut game);
        let (b, mut inbox_b) = join(&mut game);
        drain(&mut inbox_b);
        game.handle(Command::Rtc { from: a, to: b, kind: RTC_OFFER, json: "{}".into() });
        let Some(Outgoing::Reliable(bytes)) = drain(&mut inbox_b).into_iter().next() else { panic!("no rtc") };
        assert_eq!(decode::<Rtc>(&bytes), Some(Rtc { tag: S_RTC, peer: a, kind: RTC_OFFER, json: "{}".into() }));
        // Nobody to relay to, and to oneself, is a no-op.
        game.handle(Command::Rtc { from: a, to: 9999, kind: RTC_ICE, json: String::new() });
        game.handle(Command::Rtc { from: b, to: b, kind: RTC_ICE, json: String::new() });
        assert!(drain(&mut inbox_b).is_empty());
    }

    #[test]
    fn leave_is_broadcast_to_the_others() {
        let mut game = game();
        let (first, _a) = join(&mut game);
        let (_, mut b) = join(&mut game);
        drain(&mut b);
        game.handle(Command::Leave(first));
        let Some(Outgoing::Reliable(leave)) = drain(&mut b).into_iter().next() else { panic!("no leave") };
        assert_eq!(decode::<Leave>(&leave), Some(Leave { tag: S_LEAVE, id: first }));
        assert_eq!(game.seen_people(KIND_SOLDIER).len(), 1);
    }

    /// Every building of the world (`RAGNAROK_WORLD`, else the committed city), whole, must pass the
    /// structure check, and a bite out of the middle of each must not bring more than a bite's worth
    /// down. Prints the cost of a check on the biggest.
    /// `RAGNAROK_WORLD=worlds/ersboda-alidhem/world.msgpack cargo test --release -p server -- --ignored --nocapture whole_buildings_hold`
    #[test]
    #[ignore]
    fn whole_buildings_hold() {
        let path = std::env::var_os("RAGNAROK_WORLD")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack"));
        let (map, _) = crate::read_world(&path).unwrap();
        // `RAGNAROK_BITE`: the blast's radius, centimetres.
        let bite: i32 = std::env::var("RAGNAROK_BITE").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
        let (mut checked, mut failed, mut worst) = (0, 0, std::time::Duration::ZERO);
        let (mut bitten, mut crumbled, mut lost) = (0, 0, 0.0);
        for index in 0..map.buildings.len() {
            let Some((shell, _)) = sim::building_shell(&map, index) else { continue };
            let whole = shell.hold(&shell.untouched());
            let mut removed = shell.untouched();
            let start = std::time::Instant::now();
            let cut = shell.strain(&mut removed, &whole);
            worst = worst.max(start.elapsed());
            checked += 1;
            if !cut.is_empty() {
                failed += 1;
                println!("building {index}: {} of {} cells fail whole", cut.fell.len(), shell.solid_count());
            }
            // A rocket's bite at the middle of the wall's height, from the side.
            let (lo, size) = (shell.origin, shell.dims.map(|d| d as i32 * shell.cell));
            let mut removed = shell.untouched();
            let bite = shell.blast(&mut removed, [lo[0], lo[1] + size[1] / 3, lo[2] + size[2] / 2], bite);
            if !bite.blasted.is_empty() {
                bitten += 1;
                let more = shell.strain(&mut removed, &whole).fell.len();
                if more > 0 {
                    crumbled += 1;
                    lost += more as f64 / shell.solid_count() as f64;
                }
            }
        }
        println!("{checked} buildings checked whole, {failed} failed; slowest check {worst:?}; {bitten} bitten at one side, {crumbled} of them crumbled further, {:.1}% of their cells on average", 100.0 * lost / crumbled.max(1) as f64);
        assert_eq!(failed, 0);
    }

    /// A tank battle: a full room on the ground while shells, mortar rounds and bombs rain on a 300 m
    /// square, `SHELLS` of them a tick (default 2, so 120 a second) for `TICKS` ticks (default 1200), as
    /// the crater version of the world, then again with autocannon rounds (a third of the blast, and a
    /// crater a third the size) to tell what the big craters cost. `RAGNAROK_WORLD` picks the world (the committed city by default).
    /// `cargo test --release -p server -- --ignored --nocapture shelling`
    #[test]
    #[ignore]
    fn shelling_tick_cost() {
        use crate::mechanics::for_tests::Flying;
        use shared::protocol::{PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_MORTAR, PROJECTILE_SHELL};
        use std::time::Instant;
        let env = |name: &str, default: usize| std::env::var(name).ok().and_then(|n| n.parse().ok()).unwrap_or(default);
        let (shells, ticks) = (env("SHELLS", 2), env("TICKS", 1200));
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
        let fixture = std::env::var_os("RAGNAROK_WORLD").map(std::path::PathBuf::from).unwrap_or_else(|| manifest.join("fixtures/world/world.msgpack"));
        let budget = Duration::from_secs_f64(1.0 / TICK_RATE as f64);
        for (label, kinds) in [("craters (shell, mortar, bomb)", [PROJECTILE_SHELL, PROJECTILE_MORTAR, PROJECTILE_BOMB]), ("small craters (autocannon)", [PROJECTILE_CANNON; 3])] {
            let (map, _) = crate::read_world(&fixture).expect("a world to shell");
            let mut game = Game::new(map, 0).unwrap();
            game.world.clear_pickups();
            let mut inboxes = Vec::new();
            let mut ids = Vec::new();
            for _ in 0..MAX_PLAYERS - 1 {
                let (outbox, inbox) = mpsc::channel(8192);
                let (reply, mut response) = oneshot::channel();
                game.handle(Command::Join { outbox, reply, profile: Default::default() });
                ids.push(response.try_recv().unwrap().unwrap());
                inboxes.push(inbox);
            }
            // Where the battle is: around the world's middle spawn, on its ground.
            let centre = game.world.map.spawns.get(game.world.map.spawns.len() / 2).map_or([0.0, 0.0], |s| [s[0] as f32 / 100.0, s[1] as f32 / 100.0]);
            for (n, id) in ids.iter().enumerate() {
                let mut player = game.world.entities.get_mut::<Player>(*id).unwrap();
                let (x, z) = (centre[0] + (n % 8) as f32 * 30.0 - 120.0, centre[1] + (n / 8) as f32 * 30.0 - 120.0);
                player.character.position = [x, game.world.map.ground.height_at(x, z) + sim::STANDING_Y + 0.5, z];
            }
            let mut seed = 12345u32;
            let mut random = move || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 8) as f32 / (1u32 << 24) as f32
            };
            let (mut total, mut worst, mut over, mut fired) = (Duration::ZERO, Duration::ZERO, 0usize, 0usize);
            let mut spikes = Vec::new();
            let (mut bytes, mut crater_bytes) = (0usize, 0usize);
            let mut per_tick = Vec::with_capacity(ticks);
            // What each part of the tick took, by mechanic and then after them and the snapshots: in the worst ticks, summed.
            let mut names: Vec<String> = game.mechanics.iter().map(|mechanic| mechanic.name().to_string()).collect();
            names.extend(["after the mechanics".to_string(), "snapshots".to_string()]);
            let mut in_spikes = vec![Duration::ZERO; names.len()];
            let mut parts = vec![Duration::ZERO; names.len()];
            let mut worst_parts = parts.clone();
            for t in 0..ticks {
                for _ in 0..shells {
                    let (x, z) = (centre[0] + (random() - 0.5) * 300.0, centre[1] + (random() - 0.5) * 300.0);
                    let y = game.world.map.ground.height_at(x, z) + 40.0;
                    let kind = kinds[fired % 3];
                    game.world.launch(Flying::new(kind, ids[fired % ids.len()], [x, y, z], [(random() - 0.5) * 10.0, -30.0, (random() - 0.5) * 10.0], 600));
                    fired += 1;
                }
                parts.iter_mut().for_each(|part| *part = Duration::ZERO);
                let started = Instant::now();
                game.step_timed(Some(&mut parts));
                let took = started.elapsed();
                per_tick.push(took);
                if took > budget / 2 {
                    in_spikes.iter_mut().zip(&parts).for_each(|(sum, part)| *sum += *part);
                }
                total += took;
                if took > worst {
                    worst = took;
                    worst_parts.clone_from(&parts);
                }
                if took > budget {
                    over += 1;
                }
                if took > budget / 2 {
                    spikes.push((t, took));
                }
                for inbox in &mut inboxes {
                    while let Ok(out) = inbox.try_recv() {
                        let (Outgoing::Reliable(b) | Outgoing::Datagram(b)) = &out;
                        bytes += b.len();
                        if matches!(out, Outgoing::Reliable(_)) && peek_tag(b) == Some(S_CRATERED) {
                            crater_bytes += b.len();
                        }
                    }
                }
            }
            per_tick.sort();
            let pick = |q: f64| per_tick[((per_tick.len() - 1) as f64 * q) as usize];
            println!("--- {label}: {fired} shots in {ticks} ticks, {} craters dug", game.world.craters.len());
            println!("tick: avg {:?}, p50 {:?}, p99 {:?}, worst {:?}; over the {budget:?} budget: {over}; over half of it: {}", total / ticks as u32, pick(0.5), pick(0.99), worst, spikes.len());
            let mut dearest: Vec<(&String, Duration)> = names.iter().zip(in_spikes.iter().copied()).filter(|(_, took)| !took.is_zero()).collect();
            dearest.sort_by_key(|&(_, took)| std::cmp::Reverse(took));
            println!("in the ticks over half the budget, summed: {:?}", &dearest[..5.min(dearest.len())]);
            let mut worst_of: Vec<(&String, Duration)> = names.iter().zip(worst_parts.iter().copied()).collect();
            worst_of.sort_by_key(|&(_, took)| std::cmp::Reverse(took));
            println!("the worst tick: {:?}", &worst_of[..5.min(worst_of.len())]);
            println!("sent to the room: {} KiB, {} B a player a second, of which {} B a player a second are `cratered`", bytes / 1024, bytes / ids.len() / (ticks / TICK_RATE as usize).max(1), crater_bytes / ids.len() / (ticks / TICK_RATE as usize).max(1));
            if label.starts_with("craters") {
                let started = Instant::now();
                let (outbox, mut inbox) = mpsc::channel(8192);
                let (reply, _response) = oneshot::channel();
                game.handle(Command::Join { outbox, reply, profile: Default::default() });
                let joined = started.elapsed();
                let catch_up: usize = std::iter::from_fn(|| inbox.try_recv().ok()).filter_map(|o| if let Outgoing::Reliable(b) = o { Some(b) } else { None }).filter(|b| peek_tag(b) == Some(S_CRATERED)).map(|b| b.len()).sum();
                println!("a joiner after all that: {joined:?}, {catch_up} B of craters in the catch-up");
            }
        }
    }

    /// What one crater costs on ground of each sampling: the bowl dug into the grid, the new heightfields
    /// built for the tiles it reaches, and the collider swap at the start of the next step.
    /// `cargo test --release -p server -- --ignored --nocapture crater_rebuild`
    #[test]
    #[ignore]
    fn crater_rebuild_cost() {
        use std::time::Instant;
        for (cell_cm, label) in [(500, "5 m"), (200, "2 m"), (100, "1 m")] {
            // A 1536 m square of rolling ground in 256 m tiles (6 × 6 of them).
            let (tile_cm, tiles) = (25_600, 6);
            let side = (tile_cm * tiles / cell_cm + 1) as usize;
            let origin = -(tile_cm * tiles) / 2;
            let heights = (0..side * side).map(|k| 1500 + (((k % side) as f32 * 0.05).sin() * 300.0 + ((k / side) as f32 * 0.04).cos() * 300.0) as i32).collect();
            let ground = shared::terrain::Ground { origin, cell: cell_cm, side: side as u32, heights };
            let map = WorldFile { version: WORLD_VERSION, half_extent: -origin, spawns: vec![[0, 0]], tile_size: tile_cm, ground, ..Default::default() };
            let mut game = Game::new(map, 0).unwrap();
            let (mut dig, mut swap, mut worst) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
            let n = 200;
            for k in 0..n {
                // Mostly inside tiles, a few on seams and corners.
                let (x, z) = (((k * 97) % 1400) as f32 - 700.0 + if k % 5 == 0 { 256.0 - ((k * 97) % 1400) as f32 % 256.0 } else { 0.0 }, ((k * 53) % 1400) as f32 - 700.0);
                let y = game.world.map.ground.height_at(x, z);
                let started = Instant::now();
                game.world.dig_crater([x, y, z], 8.8, 1.6, Strike::DOWN);
                dig += started.elapsed();
                let started = Instant::now();
                game.world.physics.step_cars(TICK_DT);
                let took = started.elapsed();
                swap += took;
                worst = worst.max(took);
            }
            assert!(!game.world.craters.is_empty());
            println!("{label} ground ({side}² samples, {} MiB): a crater digs and builds heightfields in {:?}, the swap takes {:?} (worst {worst:?})", side * side * 4 / (1 << 20), dig / n, swap / n);
        }
    }

    /// Not a regression test: a rough cost of one tick with a full room, to aim optimisation at. Players
    /// are stood on the ground: one buried in it makes the controller depenetrate, which costs five times as much.
    /// `cargo test --release -p server -- --ignored --nocapture full_room`
    #[test]
    #[ignore]
    fn full_room_tick_cost() {
        use std::time::Instant;
        // The committed city if it is there, so buildings and trees cost what they do in play.
        // `RAGNAROK_WORLD` picks another, as for the server (the dev world, or the big one).
        let fixture = std::env::var_os("RAGNAROK_WORLD")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack"));
        let map = match crate::read_world(&fixture) {
            Ok((map, _)) => map,
            Err(_) => WorldFile { version: WORLD_VERSION, half_extent: 200_000, spawns: vec![[0, 0]], ..Default::default() },
        };
        println!("world half extent {} m, {} buildings, {} trees", map.half_extent / 100, map.buildings.len(), map.trees.len());
        let mut game = Game::new(map, 0).unwrap();
        game.world.clear_pickups();
        let mut inboxes = Vec::new();
        let mut ids = Vec::new();
        for _ in 0..MAX_PLAYERS {
            let (outbox, inbox) = mpsc::channel(4096);
            let (reply, mut response) = oneshot::channel();
            game.handle(Command::Join { outbox, reply, profile: Default::default() });
            ids.push(response.try_recv().unwrap().unwrap());
            inboxes.push(inbox);
        }
        // On the world's open ground (its spawn points, in groups of eight a few metres apart) when it has
        // some, so interest radii overlap without everyone seeing everyone; else in a grid.
        let spawns: Vec<[f32; 2]> = game.world.map.spawns.iter().map(|s| [s[0] as f32 / 100.0, s[1] as f32 / 100.0]).collect();
        println!("{} spawn points", spawns.len());
        for (n, id) in ids.iter().enumerate() {
            let mut player = game.world.entities.get_mut::<Player>(*id).unwrap();
            if spawns.is_empty() {
                player.character.position[0] = (n % 8) as f32 * 70.0 - 250.0;
                player.character.position[2] = (n / 8) as f32 * 70.0 - 250.0;
            } else {
                // `SPREAD=1`: one player per spawn point, as far apart as the world allows.
                let spread = std::env::var_os("SPREAD").is_some();
                let [sx, sz] = spawns[if spread { n * spawns.len() / MAX_PLAYERS } else { n / 8 * 7 } % spawns.len()];
                player.character.position[0] = sx + if spread { 0.0 } else { (n % 8) as f32 * 0.8 };
                player.character.position[2] = sz;
            }
            // `ON_ROADS=1`: each beside a road of its own, spread over the whole world, to have traffic near everyone.
            if std::env::var_os("ON_ROADS").is_some() && !game.world.map.roads.is_empty() {
                let roads: Vec<&shared::world::Road> = game.world.map.roads.iter().filter(|road| road.kind <= shared::world::ROAD_MINOR && road.lanes > 0).collect();
                let at = roads[n * roads.len() / MAX_PLAYERS].points[0];
                player.character.position[0] = at[0] as f32 / 100.0 + 6.0;
                player.character.position[2] = at[1] as f32 / 100.0;
            }
            let (x, z) = (player.character.position[0], player.character.position[2]);
            player.character.position[1] = game.world.map.ground.height_at(x, z) + sim::STANDING_Y + 0.5;
        }
        // `BOTS=n`: that many bots as well, spread over the spawn points among the players, on made-up loops.
        let bots: usize = std::env::var("BOTS").ok().and_then(|n| n.parse().ok()).unwrap_or(0);
        for n in 0..bots {
            let at = if spawns.is_empty() { [(n % 8) as f32 * 70.0 - 245.0, (n / 8) as f32 * 70.0 - 245.0] } else { spawns[n * spawns.len() / bots.max(1) % spawns.len()] };
            game.add_bot(at, TEAM_BOTS, Vec::new()).expect("BOTS is at most MAX_BOTS");
        }
        // `TANKS=n`: that many tank crews as well, at the spawn points from the last back, driving along the roads.
        // `VEHICLES=n`: top the room up to n parked jeeps in a grid on the open ground, past `MAX_VEHICLES`
        // if need be, to see what a vehicle costs while nobody drives it.
        let vehicles: usize = std::env::var("VEHICLES").ok().and_then(|n| n.parse().ok()).unwrap_or(0);
        let mut parked = game.world.entities.count::<crate::game::Vehicle>();
        for n in 0..vehicles.saturating_sub(parked) {
            let (x, z) = ((n % 20) as f32 * 12.0 - 120.0, (n / 20) as f32 * 12.0 + 150.0);
            let y = game.world.map.ground.height_at(x, z) + 1.5;
            crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [x, y, z], 0.0);
            parked += 1;
        }
        println!("{parked} vehicles");
        let tanks: usize = std::env::var("TANKS").ok().and_then(|n| n.parse().ok()).unwrap_or(0);
        let mut crews = Vec::new();
        for &at in spawns.iter().rev() {
            if crews.len() == tanks {
                break;
            }
            if let Some((vid, _)) = game.add_crew(["Driver".into(), "Gunner".into()], at, TEAM_BOTS, Vec::new()) {
                crews.push((vid, game.world.physics.car_pose(vid).map(|pose| pose.position)));
            }
        }
        println!("{} people, {bots} bots, {} tanks", ids.len(), crews.len());
        // `TRAFFIC=n`: that many cars for each kilometre of driven lane (TRAFFIC.md), run for `SETTLE` seconds first (60
        // unless given) so that those near the players are shown and driving.
        game.world.traffic = std::env::var("TRAFFIC").ok().and_then(|n| n.parse().ok()).unwrap_or(0.0);
        if game.world.traffic > 0.0 {
            let settle: u32 = std::env::var("SETTLE").ok().and_then(|n| n.parse().ok()).unwrap_or(60);
            let started = Instant::now();
            game.step();
            println!("traffic laid out in {:?}", started.elapsed());
            for _ in 0..settle * TICK_RATE {
                game.step();
            }
            println!("{}", game.mechanic::<crate::mechanics::Traffic>().map(crate::mechanics::Traffic::report).unwrap_or_default());
        }
        let (mut step_total, mut step_max, mut snap_total) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
        let mut after_total = Duration::ZERO;
        let (mut datagram_bytes, mut datagrams) = (0usize, 0usize);
        let ticks = 600u32;
        let mut per_mechanic = vec![Duration::ZERO; game.mechanics.len()];
        // What each mechanic took in this tick, and in the worst tick so far.
        let (mut this_tick, mut worst_tick) = (per_mechanic.clone(), per_mechanic.clone());
        for t in 1..=ticks {
            for (n, id) in ids.iter().enumerate() {
                let fire = if (t as usize + n).is_multiple_of(30) { BUTTON_FIRE } else { 0 };
                let frame = InputFrame { seq: t, strafe: if n % 2 == 0 { 100 } else { -100 }, forward: 127, yaw: (t as i16 * 7 + n as i16 * 50) % 3000, buttons: BUTTON_RUN | fire, pitch: 0, view: t as u16, slot: 0, collective: 0 };
                game.handle(Command::Input(*id, t.saturating_sub(2), vec![frame]));
            }
            let started = Instant::now();
            for id in game.world.entities.ids::<With<Player>>() {
                let mut player = game.world.entities.of_mut::<Player>(id);
                let consumed = player.inputs.pop();
                player.input = consumed.input;
                if consumed.fresh {
                    player.ack = consumed.input.seq();
                }
            }
            for (index, mechanic) in game.mechanics.iter_mut().enumerate() {
                let at = Instant::now();
                mechanic.on_tick(&mut game.world, TICK_DT);
                this_tick[index] = at.elapsed();
                per_mechanic[index] += this_tick[index];
            }
            // What `step` does after the mechanics, but for sending: the reactions, the hits and kills
            // built from the events, and the trails.
            let at = Instant::now();
            for mechanic in &mut game.mechanics {
                mechanic.react(&mut game.world);
            }
            std::hint::black_box((feedback::hits(&game.world), feedback::kills(&game.world)));
            crate::events::update(&mut game.world.entities.ecs);
            game.world.tick = game.world.tick.wrapping_add(1);
            crate::mechanics::record_trails(&mut game.world);
            after_total += at.elapsed();
            let took = started.elapsed();
            step_total += took;
            if took > step_max {
                (step_max, worst_tick) = (took, this_tick.clone());
            }
            if t % SNAPSHOT_EVERY == 0 {
                let started = Instant::now();
                game.send_snapshots();
                snap_total += started.elapsed();
            }
            for inbox in &mut inboxes {
                while let Ok(out) = inbox.try_recv() {
                    if let Outgoing::Datagram(bytes) = out {
                        (datagram_bytes, datagrams) = (datagram_bytes + bytes.len(), datagrams + 1);
                    }
                }
            }
        }
        for (vid, start) in crews {
            let (Some(start), Some(pose)) = (start, game.world.physics.car_pose(vid)) else { continue };
            let gun = game.world.entities.get::<Mounts>(vid).and_then(Mounts::turret).map_or(0, |gun| gun.shots);
            println!("tank {vid}: drove {:.0} m, fired {gun}", (pose.position[0] - start[0]).hypot(pose.position[2] - start[2]));
        }
        let budget = Duration::from_secs_f64(1.0 / TICK_RATE as f64);
        let names: Vec<&str> = game.mechanics.iter().map(|mechanic| mechanic.name()).collect();
        println!("per mechanic per tick ({}): {:?}", names.join(", "), per_mechanic.iter().map(|d| *d / ticks).collect::<Vec<_>>());
        let mut worst: Vec<(&str, Duration)> = names.iter().copied().zip(worst_tick).collect();
        worst.sort_by_key(|&(_, took)| std::cmp::Reverse(took));
        println!("the worst tick's three dearest mechanics: {:?}", &worst[..3.min(worst.len())]);
        println!("after the mechanics (reactions, feedback, events, trails): {:?}", after_total / ticks);
        println!("tick: avg {:?}, worst {:?}; snapshots for the room: avg {:?} per call (every {SNAPSHOT_EVERY} ticks); budget {budget:?}", step_total / ticks, step_max, snap_total / (ticks / SNAPSHOT_EVERY));
        println!("snapshot bytes: {} a player a snapshot, over {datagrams}", datagram_bytes / datagrams.max(1));
        if game.world.traffic > 0.0 {
            println!("{}", game.mechanic::<crate::mechanics::Traffic>().map(crate::mechanics::Traffic::report).unwrap_or_default());
        }
        // What reading the world for snapshots costs, once per snapshot.
        let runs = 200u32;
        let started = Instant::now();
        for _ in 0..runs {
            std::hint::black_box(game.replicated());
        }
        println!("reading the world as components: {:?}", started.elapsed() / runs);
    }

    /// Not a regression test: what the first bite of each building costs, which cuts it into cells
    /// (`World::cut_building`: unpacking them, the anchors, the inside, how it is held up), and the
    /// dearest ones with their grids.
    /// `RAGNAROK_WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture first_cut_cost`
    #[test]
    #[ignore]
    fn first_cut_cost() {
        use std::time::Instant;
        let path = std::env::var_os("RAGNAROK_WORLD")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack"));
        let (map, _) = crate::read_world(&path).expect("a world");
        let game = Game::new(map, 0).unwrap();
        let world = &game.world;
        let mut costs: Vec<(Duration, usize, [usize; 3])> = Vec::new();
        for (index, &id) in world.map.building_ids.iter().enumerate() {
            let started = Instant::now();
            let Some(carving) = std::hint::black_box(world.cut_building(id)) else { continue };
            costs.push((started.elapsed(), index, carving.shell.dims));
        }
        costs.sort_by_key(|&(took, _, _)| std::cmp::Reverse(took));
        let total: Duration = costs.iter().map(|&(took, _, _)| took).sum();
        println!("{} buildings: avg {:?}, worst {:?}", costs.len(), total / costs.len().max(1) as u32, costs.first().map(|c| c.0));
        for (took, index, dims) in costs.iter().take(8) {
            // Its cells unpacked, then what stands in its rooms added, and then how it is held up.
            let started = Instant::now();
            let bare = Shell::from_cells(&world.map.buildings[*index].cells);
            let cells = started.elapsed();
            let inside = &world.map.looks[*index].inside;
            let started = Instant::now();
            let found = shared::carve::furnished(world.map.buildings[*index].cells.origin, *dims, inside).len();
            let rooms = started.elapsed();
            println!("    unpacked {cells:?} ({}); {} inside triangles found {found} times in {rooms:?}", bare.is_some(), inside.len());
            let started = Instant::now();
            let (shell, _) = sim::building_shell(&world.map, *index).expect("cut once already");
            let unpacked = started.elapsed();
            let started = Instant::now();
            std::hint::black_box(shell.hold(&shell.untouched()));
            let held = started.elapsed();
            println!("  building {index}: {took:?} (cells {unpacked:?}, held {held:?}), {dims:?} cells ({} in its grid, {} standing)", dims[0] * dims[1] * dims[2], shell.standing(&shell.untouched()));
        }
    }

    /// Not a regression test: what shelling one big house costs the server, shot by shot, to aim
    /// optimisation at. The house with the most look triangles takes 40 bites the size a tank's
    /// shell takes, 20 ticks of structure checks between them.
    /// `RAGNAROK_WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture demolition_cost`
    /// (the path whole: tests run in the crate). `DEMOLITION_OUT=<file>` writes the cuts out for the client's `destruction.bench`.
    #[test]
    #[ignore]
    fn demolition_cost() {
        use std::time::Instant;
        let path = std::env::var_os("RAGNAROK_WORLD")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack"));
        let (map, _) = crate::read_world(&path).expect("a world to shell");
        let mut game = Game::new(map, 0).unwrap();
        let world = &mut game.world;
        let size = |i: usize| world.map.looks.get(i).map_or(0, |l| l.walls.len() + l.roof.len() + l.inside.len());
        // `HOUSE=n`: the n-th biggest house with a look instead (0, the default, is the biggest).
        let rank: usize = std::env::var("HOUSE").ok().and_then(|n| n.parse().ok()).unwrap_or(0);
        let mut houses: Vec<usize> = (0..world.map.buildings.len()).filter(|&i| size(i) > 0).collect();
        houses.sort_by_key(|&i| std::cmp::Reverse(size(i)));
        let index = houses[rank.min(houses.len() - 1)];
        let id = world.map.building_ids[index];
        let look = world.map.looks[index].clone();
        let b = &world.map.buildings[index];
        println!("building {index}: {} m tall, {} wall, {} roof, {} inside triangles", b.height / 100, look.walls.len(), look.roof.len(), look.inside.len());
        let mut rand = 12345u64;
        let mut next = move || {
            rand ^= rand << 13;
            rand ^= rand >> 7;
            rand ^= rand << 17;
            rand
        };
        // The bite a tank's HE shell takes of the wall it lands on.
        let bite = shared::consts::EXPLOSION_RADIUS * crate::mechanics::CARVE_REACH;
        let (mut carve_total, mut carve_max, mut settle_total, mut settle_max, mut solid_total) = (Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO);
        let (mut standing_total, mut windows_total) = (Duration::ZERO, Duration::ZERO);
        // Cells the shells took out, cells left with nothing under them at once, cells the structure checks let go, and checks that let go of 100 or more.
        let (mut blasted, mut dropped, mut crumbled, mut big) = (0usize, 0usize, 0usize, 0usize);
        let whole = world.carving(id).map_or(0, |c| c.shell.standing(&c.removed));
        let shots = 60;
        // Every cut in order, for the client's bench to replay (`DEMOLITION_OUT`).
        let mut cuts = Vec::new();
        for shot in 0..shots {
            let t = look.walls[next() as usize % look.walls.len()];
            let at = [0, 1, 2].map(|a| (t[a] + t[a + 3] + t[a + 6]) as f32 / 300.0);
            let started = Instant::now();
            if let Some(cut) = world.carve(id, at, bite) {
                blasted += cut.blasted.len();
                dropped += cut.fell.len();
            }
            world.flush_buildings();
            let spent = started.elapsed();
            carve_total += spent;
            carve_max = carve_max.max(spent);
            if let Some(carving) = world.carves.get(&id) {
                let started = Instant::now();
                std::hint::black_box(sim::building_solid(&world.map, index, Some((&carving.shell, &carving.removed)), &carving.floor));
                solid_total += started.elapsed();
                // Of which the faces as they stand (the rest is the mesh made of them), and of those the windows cut out.
                let started = Instant::now();
                std::hint::black_box(shared::facade::standing(&look.walls, &look.roof, &look.windows, &look.inside, &look.shut, Some((&carving.shell, &carving.removed))));
                standing_total += started.elapsed();
                let started = Instant::now();
                std::hint::black_box(shared::facade::cut_windows(look.walls.iter().map(shared::facade::poly_of).collect(), &look.windows, !look.inside.is_empty(), &look.shut));
                windows_total += started.elapsed();
            }
            let mut worst = Duration::ZERO;
            // Five seconds between shots, about a tank's reload.
            for _ in 0..300 {
                world.tick = world.tick.wrapping_add(1);
                let before = world.fresh.carved.len();
                let started = Instant::now();
                world.settle();
                world.flush_buildings();
                worst = worst.max(started.elapsed());
                settle_total += started.elapsed();
                for (_, _, cut) in &world.fresh.carved[before..] {
                    crumbled += cut.fell.len();
                    if cut.fell.len() >= 100 {
                        big += 1;
                    }
                }
            }
            settle_max = settle_max.max(worst);
            for (cut_id, point, cut) in world.fresh.carved.drain(..) {
                cuts.push(serde_json::json!({ "col": cut_id.col, "row": cut_id.row, "index": cut_id.index, "point": point, "blasted": cut.blasted, "fell": cut.fell }));
            }
            world.fresh = Default::default();
            if shot % 10 == 9 {
                let standing = world.carves.get(&id).map_or(0, |c| c.shell.standing(&c.removed));
                println!("after {:2} shots: {:3}% standing; blasted {blasted}, dropped {dropped}, crumbled {crumbled} in checks ({big} of 100+); carve {:?}, settle worst tick {settle_max:?}", shot + 1, standing * 100 / whole.max(1), carve_total / (shot + 1));
            }
        }
        println!(
            "bite {bite:.1} m; carve avg {:?}, worst {carve_max:?}; collider alone {:?} (its faces {:?}, of which the windows cut out {:?}); settle worst tick {settle_max:?}",
            carve_total / shots,
            solid_total / shots,
            standing_total / shots,
            windows_total / shots
        );
        if let Some(out) = std::env::var_os("DEMOLITION_OUT") {
            std::fs::write(out, serde_json::to_string(&cuts).unwrap()).unwrap();
        }
    }

    /// Not a regression test: shells a stretch of real bridge (the viaduct by Backen in `home`) down
    /// the middle of its deck and draws what is left of each section from above, to see what hangs on.
    /// `RAGNAROK_WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture bridge_wreckage`
    /// `SINGLE=1` fires one shell at each section's middle instead and says what it took;
    /// `WRECKAGE_OUT=<file>` writes the cuts out for the client's `view.carved` to replay.
    #[test]
    #[ignore]
    fn bridge_wreckage() {
        let path = std::env::var_os("RAGNAROK_WORLD").map(std::path::PathBuf::from).expect("RAGNAROK_WORLD");
        let (map, _) = crate::read_world(&path).expect("a world");
        let mut game = Game::new(map, 0).unwrap();
        let world = &mut game.world;
        // The viaduct by Backen in `home`, over the river.
        let near = |c: [f32; 2]| (-1120.0..-1040.0).contains(&c[0]) && (170.0..380.0).contains(&c[1]);
        let sections: Vec<usize> = (0..world.map.buildings.len())
            .filter(|&i| world.map.buildings[i].kind == shared::world::BUILDING_BRIDGE)
            .filter(|&i| {
                let f = &world.map.buildings[i].footprint;
                let n = f.len() as f32;
                near([f.iter().map(|p| p[0] as f32).sum::<f32>() / n / 100.0, f.iter().map(|p| p[1] as f32).sum::<f32>() / n / 100.0])
            })
            .collect();
        let bite = shared::consts::EXPLOSION_RADIUS * crate::mechanics::CARVE_REACH;
        println!("{} sections, bite {bite:.1} m", sections.len());
        for &index in &sections {
            let id = world.map.building_ids[index];
            let b = world.map.buildings[index].clone();
            let pts: Vec<[f32; 2]> = b.footprint.iter().map(|p| [p[0] as f32 / 100.0, p[1] as f32 / 100.0]).collect();
            // The long way across the footprint: its two corners farthest apart.
            let (mut a, mut c, mut best) = (pts[0], pts[0], 0.0);
            for &p in &pts {
                for &q in &pts {
                    let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                    if d > best {
                        (a, c, best) = (p, q, d);
                    }
                }
            }
            let top = (b.base + b.height) as f32 / 100.0;
            if std::env::var_os("COLUMN").is_some() {
                // A Carl Gustaf round's bite (`blast_bridge`: a rocket's 5 m, a fifth of it, full strength)
                // into each column 3 m up its side, one then the other, the structure checks run after each.
                let bite = shared::consts::ROCKET_BLAST_RADIUS * 0.2;
                let Some(carving) = world.carving(id) else { continue };
                let shell = &carving.shell;
                let layer = 4usize;
                let low: Vec<[i32; 3]> = shell.solid_cells().into_iter().filter(|&c| shell.coords(c)[1] == layer).map(|c| shell.centre(c)).collect();
                let total = shell.solid_cells().len();
                let _ = &carving;
                // The two cells of that layer farthest apart: one on each column's outer side.
                let (mut p, mut q, mut best) = (low[0], low[0], -1i64);
                for &a in &low {
                    for &b in &low {
                        let d = ((a[0] - b[0]) as i64).pow(2) + ((a[2] - b[2]) as i64).pow(2);
                        if d > best {
                            (p, q, best) = (a, b, d);
                        }
                    }
                }
                let mut report = Vec::new();
                println!("section {index}: dims {:?}, columns hit at {p:?} and {q:?}", shell.dims);
                for at in [p, q] {
                    let cut = world.carve(id, at.map(|v| v as f32 / 100.0), bite);
                    println!("  hit: {} blasted, {} cut off at once", cut.as_ref().map_or(0, |c| c.blasted.len()), cut.as_ref().map_or(0, |c| c.fell.len()));
                    world.flush_buildings();
                    for _ in 0..600 {
                        world.tick = world.tick.wrapping_add(1);
                        world.settle();
                        world.flush_buildings();
                    }
                    let gone = world.carves.get(&id).map_or(0, |c| c.removed.iter().filter(|&&g| g).count());
                    report.push(gone);
                }
                println!("section {index}: {total} cells, {} low cells in layer {layer}; gone after one column hit {}, after both {}", low.len(), report[0], report[1]);
                continue;
            }
            if std::env::var_os("SINGLE").is_some() {
                let mid = [(a[0] + c[0]) / 2.0, top - 0.5, (a[1] + c[1]) / 2.0];
                let cut = world.carve(id, mid, bite);
                let (dims, cells) = world.carves.get(&id).map_or(([0; 3], 0), |c| (c.shell.dims, c.shell.solid_cells().len()));
                println!("section {index}: one shell at its middle: {:?} of {cells}, dims {dims:?}, {:.0} m long", cut.map(|cut| (cut.blasted.len(), cut.fell.len())), best);
                continue;
            }
            let steps = (best / 2.0) as i32;
            for k in 1..steps {
                let t = k as f32 / steps as f32;
                world.carve(id, [a[0] + (c[0] - a[0]) * t, top - 0.5, a[1] + (c[1] - a[1]) * t], bite);
                world.flush_buildings();
            }
            for _ in 0..600 {
                world.tick = world.tick.wrapping_add(1);
                world.settle();
                world.flush_buildings();
            }
        }
        // `WRECKAGE_OUT=<file>`: every cut in order, as the client's `carved` takes them.
        if let Some(out) = std::env::var_os("WRECKAGE_OUT") {
            let cuts: Vec<serde_json::Value> = world.fresh.carved.iter().map(|(id, point, cut)| serde_json::json!({ "col": id.col, "row": id.row, "index": id.index, "point": point, "blasted": cut.blasted, "fell": cut.fell })).collect();
            std::fs::write(out, serde_json::to_string(&cuts).unwrap()).unwrap();
        }
        for &index in &sections {
            let id = world.map.building_ids[index];
            let Some(carving) = world.carves.get(&id) else { continue };
            let shell = &carving.shell;
            let standing: Vec<u32> = shell.solid_cells().into_iter().filter(|&id| !carving.removed[id as usize]).collect();
            println!("section {index}: {} of {} cells standing, dims {:?}", standing.len(), shell.solid_cells().len(), shell.dims);
            // From above: the highest standing layer in each column, as a digit (relative to the top layer), or '.'.
            let [nx, ny, nz] = shell.dims;
            for z in 0..nz {
                let row: String = (0..nx)
                    .map(|x| match (0..ny).rev().find(|&y| standing.contains(&shell.index(x, y, z))) {
                        Some(y) => char::from_digit(((ny - 1 - y) % 10) as u32, 10).unwrap(),
                        None => '.',
                    })
                    .collect();
                println!("  {row}");
            }
        }
    }
}
