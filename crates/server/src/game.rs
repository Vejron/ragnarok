//! The authoritative game loop: one task owns the whole world and steps it at a fixed rate.
//! Sessions talk to it only through `Command`s in and `Outgoing` messages out.

use crate::jitter::JitterBuffer;
use crate::mechanics::{Mechanic, default_mechanics};
use crate::physics::{Character, Physics};
use crate::rng::Rng;
use anyhow::Result;
use bytes::Bytes;
use shared::consts::*;
use shared::protocol::*;
use shared::quant::{to_cm, to_cm_i16, to_mrad};
use shared::world::WorldFile;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};

pub enum Command {
    /// Admit a player. Replies with its entity id, or `None` when the room is full.
    Join { outbox: mpsc::Sender<Outgoing>, reply: oneshot::Sender<Option<u32>> },
    Leave(u32),
    Input(u32, Vec<InputFrame>),
}

#[derive(Clone, Debug)]
pub enum Outgoing {
    /// Unreliable and unordered: snapshots.
    Datagram(Bytes),
    /// Reliable, each on its own unidirectional stream: welcome, leave.
    Reliable(Bytes),
}

pub struct Player {
    pub character: Character,
    /// The input applied this tick.
    pub input: InputFrame,
    pub inputs: JitterBuffer,
    /// Newest client input sequence already applied, echoed in snapshots for reconciliation.
    pub ack: u32,
}

/// Everything mechanics may read and change.
pub struct World {
    pub tick: u32,
    pub map: WorldFile,
    pub physics: Physics,
    /// Ordered by id so every pass over players is deterministic.
    pub players: BTreeMap<u32, Player>,
    pub rng: Rng,
}

pub struct Game {
    pub world: World,
    mechanics: Vec<Box<dyn Mechanic>>,
    outboxes: BTreeMap<u32, mpsc::Sender<Outgoing>>,
    next_id: u32,
    world_hash: u32,
}

impl Game {
    pub fn new(map: WorldFile, world_hash: u32) -> Result<Self> {
        Ok(Self {
            world: World { tick: 0, physics: Physics::new(&map)?, map, players: BTreeMap::new(), rng: Rng::from_time() },
            mechanics: default_mechanics(),
            outboxes: BTreeMap::new(),
            next_id: 1,
            world_hash,
        })
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Join { outbox, reply } => {
                let _ = reply.send(self.join(outbox));
            }
            Command::Leave(id) => self.leave(id),
            Command::Input(id, frames) => {
                if let Some(player) = self.world.players.get_mut(&id) {
                    player.inputs.push(&frames);
                }
            }
        }
    }

    fn join(&mut self, outbox: mpsc::Sender<Outgoing>) -> Option<u32> {
        if self.world.players.len() >= MAX_PLAYERS {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let welcome = encode(&Welcome(S_WELCOME, PROTOCOL_VERSION, id, TICK_RATE as u8, SNAPSHOT_EVERY as u8, self.world_hash));
        if outbox.try_send(Outgoing::Reliable(welcome.into())).is_err() {
            return None;
        }
        self.world.players.insert(
            id,
            Player { character: Character::default(), input: InputFrame::default(), inputs: JitterBuffer::default(), ack: 0 },
        );
        self.outboxes.insert(id, outbox);
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
        self.world.tick = self.world.tick.wrapping_add(1);
        if self.world.tick.is_multiple_of(SNAPSHOT_EVERY) {
            self.send_snapshots();
        }
    }

    pub fn entity_states(&self) -> Vec<EntityState> {
        self.world
            .players
            .iter()
            .map(|(&id, player)| {
                let c = &player.character;
                EntityState(
                    id,
                    KIND_STICKMAN,
                    to_cm(c.position[0]),
                    to_cm(c.position[1]),
                    to_cm(c.position[2]),
                    to_mrad(c.yaw),
                    to_cm_i16(c.vy),
                    if c.grounded { FLAG_GROUNDED } else { 0 },
                )
            })
            .collect()
    }

    fn send_snapshots(&mut self) {
        let entities = self.entity_states();
        for (id, outbox) in &self.outboxes {
            let ack = self.world.players.get(id).map_or(0, |player| player.ack);
            let snapshot = encode(&Snapshot(S_SNAPSHOT, self.world.tick, ack, entities.clone()));
            // A full outbox means the session is stalled; dropping a snapshot is harmless.
            let _ = outbox.try_send(Outgoing::Datagram(snapshot.into()));
        }
    }
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

        game.handle(Command::Input(id, vec![InputFrame(1, 0, 127, 0, 0), InputFrame(2, 0, 127, 0, 0)]));
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
        assert_eq!(last.2, 2, "both inputs acknowledged");
        assert_eq!(last.3.len(), 1);
        assert!(last.3[0].4 < 0, "walked towards -Z");
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
        assert_eq!(game.entity_states().len(), 1);
    }
}
