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
use shared::items::{HOTBAR_SLOTS, ITEM_AK5, ITEM_DRONE, ITEM_FLARE, ITEM_LOITER, ITEM_SNIPER, def};
use shared::protocol::*;
use shared::quant::{from_cm, pack_quat, to_axis, to_cm, to_cm_i16, to_milli_i16, to_mrad};
use shared::world::{Building, ObjId, Tree, WorldFile};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

mod replicate;
mod snapshot;
mod state;

pub use crate::events::Noise;
pub use state::*;

/// `object` as a `Destroyed` entry.
fn destroyed_entry(object: Obj, yaw: i16, force: u8) -> (u8, u16, u16, u32, i16, u8) {
    match object {
        Obj::Tree(id) => (OBJ_TREE, id.col, id.row, id.index, yaw, force),
        Obj::Building(id) => (OBJ_BUILDING, id.col, id.row, id.index, yaw, force),
    }
}

/// Building `id`'s damage `stage`, and where a blast marked it, as a `Damaged` entry.
/// Player `id`'s `Profiles` entry: who they are, and which side.
fn profile_entry(world: &World, id: u32) -> ProfileEntry {
    let player = &world.entities.of::<Player>(id);
    (id, player.name.clone(), player.color, player.team, world.entities.has::<Bot>(id) as u8)
}

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
    /// Players whose outbox rejected a reliable message this tick. Reliable messages carry state
    /// that is never repeated (kills, destroyed, damaged), so a client that missed one is out of
    /// sync for good; it is disconnected and rejoins with the catch-up instead.
    stalled: Vec<u32>,
    world_hash: u32,
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
                buildings_by_id: map.building_ids.iter().enumerate().map(|(index, &id)| (id, index)).collect(),
                object_hp: HashMap::new(),
                destroyed: Vec::new(),
                fresh_destroyed: Vec::new(),
                fresh_damaged: Vec::new(),
                carves: HashMap::new(),
                fresh_carved: Vec::new(),
                map,
                entities: Default::default(),
                nav: Default::default(),
                next_id: 1,
                rng: Rng::from_time(),
                shots: Vec::new(),
                bullets: Vec::new(),
                kills: Vec::new(),
                hits: Vec::new(),
                history: VecDeque::new(),
            },
            mechanics: default_mechanics(),
            outboxes: BTreeMap::new(),
            stalled: Vec::new(),
            replicas: BTreeMap::new(),
            world_hash,
            scenario: None,
        };
        let names: Vec<&str> = game.mechanics.iter().map(|mechanic| mechanic.name()).collect();
        tracing::debug!(mechanics = ?names, "room created");
        for mechanic in &mut game.mechanics {
            mechanic.on_start(&mut game.world);
        }
        for bot in std::mem::take(&mut game.world.map.placed_bots) {
            let route = bot.route.iter().map(|point| point.map(from_cm)).collect();
            match game.add_bot(bot.at.map(from_cm), bot.team, route) {
                Some(id) => game.world.entities.get_mut::<Bot>(id).expect("just added").skill = bot.skill.clamp(0.0, 1.0),
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
                    let _ = outbox.try_send(Outgoing::Reliable(encode(&Rtc(S_RTC, from, kind, json)).into()));
                }
            }
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
        let welcome = encode(&Welcome { tag: S_WELCOME, version: PROTOCOL_VERSION, id, tick_rate: TICK_RATE as u8, snapshot_every: SNAPSHOT_EVERY as u8, world_hash: self.world_hash });
        if outbox.try_send(Outgoing::Reliable(welcome.into())).is_err() {
            return None;
        }
        self.world.entities.spawn(
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
                name: if profile.name.is_empty() { format!("Player {id}") } else { profile.name },
                color: profile.color.unwrap_or((id % PLAYER_COLOR_COUNT as u32) as u8),
                spawns: profile.spawns.into_iter().filter(|&point| crate::mechanics::spawn_usable(&self.world.map, point)).collect(),
                next_spawn: 0,
                team: TEAM_NONE,
            },
        );
        let profiles = self.world.entities.ids::<With<Player>>().into_iter().map(|id| profile_entry(&self.world, id)).collect();
        let _ = outbox.try_send(Outgoing::Reliable(encode(&Profiles(S_PROFILES, profiles)).into()));
        let joiner = Bytes::from(encode(&Profiles(S_PROFILES, vec![profile_entry(&self.world, id)])));
        for (&to, other) in &self.outboxes {
            send_reliable(other, to, &joiner, &mut self.stalled);
        }
        let scores = self.world.entities.all::<Player>().map(|(id, player)| (id, player.kills, player.deaths)).collect();
        let _ = outbox.try_send(Outgoing::Reliable(encode(&Scores(S_SCORES, scores)).into()));
        // Always sent, even empty: the client takes the first one after the welcome as the catch-up
        // (not news to animate), and snapshots on the datagram channel can overtake it.
        let message = Destroyed(S_DESTROYED, self.world.destroyed.iter().map(|&(object, yaw)| destroyed_entry(object, yaw, 0)).collect());
        let _ = outbox.try_send(Outgoing::Reliable(encode(&message).into()));
        let hurt: Vec<_> = self.world.object_hp.keys().filter_map(|&object| self.world.damage_stage(object).map(|stage| (object, stage))).collect();
        let hurt: Vec<_> = hurt.into_iter().filter_map(|(object, stage)| match object {
            Obj::Building(object_id) if stage > 0 => Some(damaged_entry(object_id, stage, None)),
            _ => None,
        }).collect();
        if !hurt.is_empty() {
            let _ = outbox.try_send(Outgoing::Reliable(encode(&Damaged(S_DAMAGED, hurt)).into()));
        }
        // Cells already gone, as state: the building they were taken from is still standing.
        let carved: Vec<_> = self
            .world
            .carves
            .iter()
            .filter(|(object_id, _)| !self.world.destroyed.iter().any(|&(done, _)| done == Obj::Building(**object_id)))
            .filter_map(|(object_id, carving)| {
                let gone: Vec<u32> = (0..carving.removed.len()).filter(|&i| carving.removed[i]).map(|i| i as u32).collect();
                (!gone.is_empty()).then(|| (object_id.col, object_id.row, object_id.index, 0, 0, 0, gone, Vec::new()))
            })
            .collect();
        if !carved.is_empty() {
            let _ = outbox.try_send(Outgoing::Reliable(encode(&Carved(S_CARVED, 0, carved)).into()));
        }
        if let Some(objective) = self.scenario.as_ref().map(Scenario::objective).filter(|o| !o.is_empty()) {
            let _ = outbox.try_send(Outgoing::Reliable(encode(&Notice(S_NOTICE, NOTICE_OBJECTIVE, objective.into())).into()));
        }
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
        self.world.entities.despawn(id);
        self.outboxes.remove(&id);
        self.replicas.remove(&id);
        let leave = Bytes::from(encode(&Leave(S_LEAVE, id)));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &leave, &mut self.stalled);
        }
        tracing::info!(id, players = self.world.entities.count::<Player>(), "player left");
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
        self.world.entities.spawn(
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
                name,
                // One outfit per team, so a squad reads as one.
                color: (team.wrapping_mul(5)) % PLAYER_COLOR_COUNT,
                spawns: vec![home.map(to_cm)],
                next_spawn: 0,
                team,
            },
        );
        self.world.entities.insert(id, Bot::new(home, route));
        for mechanic in &mut self.mechanics {
            mechanic.on_join(&mut self.world, id);
        }
        // Scores are only ever sent whole, to joiners; a bot's starts at zero on every client anyway.
        let profile = Bytes::from(encode(&Profiles(S_PROFILES, vec![profile_entry(&self.world, id)])));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &profile, &mut self.stalled);
        }
        tracing::info!(id, team, x = home[0], z = home[1], "bot added");
        Some(id)
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
        let bytes = Bytes::from(encode(&Notice(S_NOTICE, kind, text.into())));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &bytes, &mut self.stalled);
        }
    }

    /// One fixed simulation step.
    pub fn step(&mut self) {
        for id in self.world.entities.ids::<With<Player>>() {
            let mut player = self.world.entities.of_mut::<Player>(id);
            let consumed = player.inputs.pop();
            player.input = consumed.input;
            if consumed.fresh {
                player.ack = consumed.input.seq();
            }
        }
        for mechanic in &mut self.mechanics {
            mechanic.on_tick(&mut self.world, TICK_DT);
        }
        if let Some(mut scenario) = self.scenario.take() {
            scenario.tick(self);
            self.scenario = Some(scenario);
        }
        self.broadcast_kills();
        self.send_hits();
        self.broadcast_destroyed();
        self.broadcast_damaged();
        self.broadcast_carved();
        self.drop_stalled();
        crate::events::update(&mut self.world.entities.ecs);
        self.world.tick = self.world.tick.wrapping_add(1);
        self.world.record_history();
        if self.world.tick.is_multiple_of(SNAPSHOT_EVERY) {
            self.send_snapshots();
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
        if self.world.fresh_destroyed.is_empty() {
            return;
        }
        let entries = self.world.fresh_destroyed.drain(..).map(|(object, yaw, force)| destroyed_entry(object, yaw, force)).collect();
        let bytes = Bytes::from(encode(&Destroyed(S_DESTROYED, entries)));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &bytes, &mut self.stalled);
        }
    }

    /// Tells everyone which cells blasts took out of buildings this tick, as one message.
    fn broadcast_carved(&mut self) {
        if self.world.fresh_carved.is_empty() {
            return;
        }
        let entries = self.world.fresh_carved.drain(..).map(|(id, at, cut)| (id.col, id.row, id.index, at[0], at[1], at[2], cut.blasted, cut.fell)).collect();
        let bytes = Bytes::from(encode(&Carved(S_CARVED, 1, entries)));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &bytes, &mut self.stalled);
        }
    }

    /// Tells everyone about the buildings hurt this tick, as one message.
    fn broadcast_damaged(&mut self) {
        if self.world.fresh_damaged.is_empty() {
            return;
        }
        let entries = self.world.fresh_damaged.drain(..).map(|(id, stage, mark)| damaged_entry(id, stage, mark)).collect();
        let bytes = Bytes::from(encode(&Damaged(S_DAMAGED, entries)));
        for (&to, outbox) in &self.outboxes {
            send_reliable(outbox, to, &bytes, &mut self.stalled);
        }
    }

    /// Tells each player about the hits they dealt or took this tick, amounts summed per kind.
    fn send_hits(&mut self) {
        let mut merged: Vec<(u32, Hit)> = Vec::new();
        for (to, hit) in self.world.hits.drain(..) {
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

    /// Tells everyone about the kills mechanics recorded this tick.
    fn broadcast_kills(&mut self) {
        for kill in self.world.kills.drain(..) {
            let bytes = Bytes::from(encode(&kill));
            for (&to, outbox) in &self.outboxes {
                send_reliable(outbox, to, &bytes, &mut self.stalled);
            }
        }
    }

}


/// Runs the game loop until every command sender is gone.
pub async fn run(mut game: Game, mut commands: mpsc::Receiver<Command>) {
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
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
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
        assert_eq!(decode::<Welcome>(&welcome).unwrap().id, id);

        game.handle(Command::Input(id, 0, vec![InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }, InputFrame { seq: 2, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
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
        // No snapshot was acknowledged, so each brings everything whole: the stickman among it.
        let scene = shared::replicated::apply(&[], &last.updates, &last.removed);
        let stickmen: Vec<_> = scene.iter().filter(|(_, e)| e.kind.as_ref().is_some_and(|kind| kind.kind == KIND_STICKMAN)).collect();
        assert_eq!(stickmen.len(), 1);
        assert!(stickmen[0].1.place.unwrap().z < 0, "walked towards -Z");
    }

    fn profiles(outgoing: Vec<Outgoing>) -> Vec<Vec<ProfileEntry>> {
        outgoing
            .into_iter()
            .filter_map(|out| match out {
                Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_PROFILES) => decode::<Profiles>(&bytes).map(|Profiles(_, list)| list),
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
        game.handle(Command::Join { outbox, reply, profile: Profile { name: "Åsa".into(), color: Some(5), spawns: vec![] } });
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
        game.world.fresh_destroyed.push((Obj::Tree(Default::default()), 0, 0));
        game.step();
        assert!(!game.world.entities.has::<Player>(id));
    }

    #[test]
    fn rtc_is_relayed_with_the_sender_filled_in() {
        let mut game = game();
        let (a, _a) = join(&mut game);
        let (b, mut inbox_b) = join(&mut game);
        drain(&mut inbox_b);
        game.handle(Command::Rtc { from: a, to: b, kind: RTC_OFFER, json: "{}".into() });
        let Some(Outgoing::Reliable(bytes)) = drain(&mut inbox_b).into_iter().next() else { panic!("no rtc") };
        assert_eq!(decode::<Rtc>(&bytes), Some(Rtc(S_RTC, a, RTC_OFFER, "{}".into())));
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
        assert_eq!(decode::<Leave>(&leave), Some(Leave(S_LEAVE, first)));
        assert_eq!(game.seen_people(KIND_STICKMAN).len(), 1);
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
        let (mut step_total, mut step_max, mut snap_total) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
        let ticks = 600u32;
        let mut per_mechanic = vec![Duration::ZERO; game.mechanics.len()];
        for t in 1..=ticks {
            for (n, id) in ids.iter().enumerate() {
                let fire = if (t as usize + n).is_multiple_of(30) { BUTTON_FIRE } else { 0 };
                let frame = InputFrame { seq: t, strafe: if n % 2 == 0 { 100 } else { -100 }, forward: 127, yaw: (t as i16 * 7 + n as i16 * 50) % 3000, buttons: BUTTON_RUN | fire, pitch: 0, view: t as u16, slot: 0 };
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
                per_mechanic[index] += at.elapsed();
            }
            game.world.tick = game.world.tick.wrapping_add(1);
            let took = started.elapsed();
            step_total += took;
            step_max = step_max.max(took);
            if t % SNAPSHOT_EVERY == 0 {
                let started = Instant::now();
                game.send_snapshots();
                snap_total += started.elapsed();
            }
            for inbox in &mut inboxes {
                while inbox.try_recv().is_ok() {}
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
        println!("mechanics: avg {:?}, worst {:?}; snapshots for the room: avg {:?} per call (every {SNAPSHOT_EVERY} ticks); budget {budget:?}", step_total / ticks, step_max, snap_total / (ticks / SNAPSHOT_EVERY));
        // What reading the world for snapshots costs, once per snapshot.
        let runs = 200u32;
        let started = Instant::now();
        for _ in 0..runs {
            std::hint::black_box(game.replicated());
        }
        println!("reading the world as components: {:?}", started.elapsed() / runs);
    }
}
