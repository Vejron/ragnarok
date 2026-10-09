//! The authoritative game loop: one task owns the whole world and steps it at a fixed rate.
//! Sessions talk to it only through `Command`s in and `Outgoing` messages out.

use crate::jitter::JitterBuffer;
use crate::mechanics::{Mechanic, default_mechanics};
use crate::physics::{CarControls, Character, Physics};
use crate::rng::Rng;
use anyhow::Result;
use bytes::Bytes;
use shared::consts::*;
use shared::protocol::*;
use shared::quant::{pack_quat, to_axis, to_cm, to_cm_i16, to_mrad};
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

/// Where a seated player sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub vehicle: u32,
    /// 0 is the driver.
    pub index: usize,
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
    /// The turret, for a vehicle that has one.
    pub gun: Option<Gun>,
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
    /// How far the last shell flew, metres.
    pub range: f32,
}

/// Everything mechanics may read and change.
pub struct World {
    pub tick: u32,
    pub map: WorldFile,
    pub physics: Physics,
    /// Ordered by id so every pass over players is deterministic.
    pub players: BTreeMap<u32, Player>,
    /// Vehicles share the id space with players.
    pub vehicles: BTreeMap<u32, Vehicle>,
    next_id: u32,
    pub rng: Rng,
}

impl World {
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

pub struct Game {
    pub world: World,
    mechanics: Vec<Box<dyn Mechanic>>,
    outboxes: BTreeMap<u32, mpsc::Sender<Outgoing>>,
    world_hash: u32,
}

impl Game {
    pub fn new(map: WorldFile, world_hash: u32) -> Result<Self> {
        let mut game = Self {
            world: World {
                tick: 0,
                physics: Physics::new(&map)?,
                map,
                players: BTreeMap::new(),
                vehicles: BTreeMap::new(),
                next_id: 1,
                rng: Rng::from_time(),
            },
            mechanics: default_mechanics(),
            outboxes: BTreeMap::new(),
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
        let id = self.world.alloc_id();
        let welcome = encode(&Welcome(S_WELCOME, PROTOCOL_VERSION, id, TICK_RATE as u8, SNAPSHOT_EVERY as u8, self.world_hash));
        if outbox.try_send(Outgoing::Reliable(welcome.into())).is_err() {
            return None;
        }
        self.world.players.insert(
            id,
            Player { character: Character::default(), seat: None, hit_cooldown: 0, input: InputFrame::default(), inputs: JitterBuffer::default(), ack: 0 },
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

    /// Standing players. Seated ones are shown by their vehicle instead.
    pub fn entity_states(&self) -> Vec<EntityState> {
        self.world
            .players
            .iter()
            .filter(|(_, player)| player.seat.is_none())
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
                    to_cm_i16(c.knock[0]),
                    to_cm_i16(c.knock[1]),
                )
            })
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
                Some(VehicleState(id, vehicle.kind, to_cm(x), to_cm(y), to_cm(z), qx, qy, qz, to_axis(pose.steer), seats, turret))
            })
            .collect()
    }

    fn send_snapshots(&mut self) {
        let entities = self.entity_states();
        let vehicles = self.vehicle_states();
        for (id, outbox) in &self.outboxes {
            let ack = self.world.players.get(id).map_or(0, |player| player.ack);
            let ride = self.world.players.get(id).map_or(0, |player| player.character.ride);
            let snapshot = encode(&Snapshot(S_SNAPSHOT, self.world.tick, ack, ride, entities.clone(), vehicles.clone()));
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

        game.handle(Command::Input(id, vec![InputFrame(1, 0, 127, 0, 0, 0), InputFrame(2, 0, 127, 0, 0, 0)]));
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
        assert_eq!(last.4.len(), 1);
        assert!(last.4[0].4 < 0, "walked towards -Z");
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
