//! The authoritative game loop: one task owns the whole world and steps it at a fixed rate.
//! Sessions talk to it only through `Command`s in and `Outgoing` messages out.

use crate::jitter::JitterBuffer;
use crate::mechanics::{Mechanic, default_mechanics};
use crate::physics::{CarControls, Character, Physics};
use crate::replication::{Everything, Own, Replica};
use crate::rng::Rng;
use anyhow::Result;
use bytes::Bytes;
use shared::consts::*;
use shared::protocol::*;
use shared::quant::{pack_quat, to_axis, to_cm, to_cm_i16, to_milli_i16, to_mrad};
use shared::world::WorldFile;
use std::collections::{BTreeMap, VecDeque};
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
    /// Ticks until the rifle can fire again.
    pub fire_cooldown: u32,
    pub rocket_cooldown: u32,
    pub kills: u16,
    pub deaths: u16,
}

impl Player {
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
    /// The turret, for a vehicle that has one.
    pub gun: Option<Gun>,
    /// Health. At zero it is a wreck for `WRECK_TICKS`, then it is put back at `home`.
    pub hp: u8,
    pub wreck_in: u32,
    /// Whoever last hurt it, who is credited with the deaths when it blows up.
    pub last_attacker: Option<u32>,
}

impl Vehicle {
    pub fn wrecked(&self) -> bool {
        self.hp == 0
    }
}

/// A rocket in flight. The `Combat` mechanic steps it and bursts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rocket {
    pub owner: u32,
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
    /// How far the last shell flew, metres.
    pub range: f32,
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
    next_id: u32,
    pub rng: Rng,
    /// Rifle shots since the last snapshot, for drawing tracers. Drained when snapshots go out.
    pub shots: Vec<ShotEvent>,
    /// Kills since the last tick. Drained and broadcast reliably by `Game::step`.
    pub kills: Vec<Kill>,
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
                map,
                players: BTreeMap::new(),
                vehicles: BTreeMap::new(),
                rockets: BTreeMap::new(),
                next_id: 1,
                rng: Rng::from_time(),
                shots: Vec::new(),
                kills: Vec::new(),
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
                fire_cooldown: 0,
                rocket_cooldown: 0,
                kills: 0,
                deaths: 0,
            },
        );
        let scores = self.world.players.iter().map(|(&id, player)| (id, player.kills, player.deaths)).collect();
        let _ = outbox.try_send(Outgoing::Reliable(encode(&Scores(S_SCORES, scores)).into()));
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
        self.world.tick = self.world.tick.wrapping_add(1);
        self.world.record_history();
        if self.world.tick.is_multiple_of(SNAPSHOT_EVERY) {
            self.send_snapshots();
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

    /// Standing players. Seated ones are shown by their vehicle instead.
    pub fn entity_states(&self) -> Vec<EntityState> {
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
                Some(VehicleState(id, vehicle.kind, to_cm(x), to_cm(y), to_cm(z), qx, qy, qz, to_axis(pose.steer), seats, turret, vehicle.hp))
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
                ProjectileState(id, x, y, z, vx, vy, vz)
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
        let mut shots = std::mem::take(&mut self.world.shots);
        shots.truncate(MAX_SHOT_EVENTS);
        let everything = Everything { entities: &entities, vehicles: &vehicles, shots: &shots, rockets: &rockets };
        let ids: Vec<u32> = self.outboxes.keys().copied().collect();
        for id in ids {
            let Some(player) = self.world.players.get(&id) else { continue };
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, drive: self.drive_state(id), hp: player.hp };
            // Seated players ride with their vehicle, so the vehicle's position is theirs.
            let at = [player.character.position[0], player.character.position[2]];
            let (Some(outbox), Some(replica)) = (self.outboxes.get(&id), self.replicas.get_mut(&id)) else { continue };
            let snapshot = replica.snapshot(at, &everything, own);
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

        game.handle(Command::Input(id, 0, vec![InputFrame(1, 0, 127, 0, 0, 0, 0), InputFrame(2, 0, 127, 0, 0, 0, 0)]));
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
        assert_eq!(last.5.len(), 1);
        assert!(last.5[0].4 < 0, "walked towards -Z");
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
