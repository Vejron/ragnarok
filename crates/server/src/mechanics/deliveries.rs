//! The delivery mode (DELIVERY.md): a board of jobs on everyone's phone, each a parcel to fetch at a
//! shop's door and carry to a house's. A player takes a job off the board (`Phone`), walks to the shop
//! to fetch the parcel, and carries it on their back to the house. Whoever hands it in there is paid,
//! in full within the job's par time and less after it, and that need not be who took the job: a
//! courier who dies drops the parcel where they fall, and anyone may pick it up and deliver it. Left
//! lying for `DELIVERY_SPOIL_SECONDS`, it spoils and the job is over.
//!
//! Each player is told the board and their own jobs (`Jobs`), and everyone is told who delivered what
//! (`Delivered`), which carries the new cash for the scoreboard.

use super::Mechanic;
use super::carry::{drop_spot, within};
use crate::entities::{Component, Transform, With};
use crate::game::{Died, Player, World};
use shared::consts::*;
use shared::protocol::{Delivered, JobOffer, JobState, Jobs, Phone, S_DELIVERED, S_JOBS};
use shared::quant::to_cm;
use shared::world::{BUILDING_CABIN, BUILDING_FARMHOUSE, BUILDING_FLATS, BUILDING_PLAIN, BUILDING_SHOP, BUILDING_VILLA, WorldFile};
use std::collections::BTreeSet;

pub struct Deliveries;

/// A parcel carried on its back by the soldier who has it, for job `job`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parcel {
    pub job: u32,
}

/// A parcel lying on the ground, at its entity's `Transform`, for job `job`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LyingParcel {
    pub job: u32,
}

/// How many shops and houses the board picks among before it gives up for this tick.
const TRIES: u32 = 32;

/// Where a job's parcel is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Whereabouts {
    /// Still at the shop, waiting to be fetched.
    Shop,
    /// On player `id`'s back.
    Carried(u32),
    /// On the ground as entity `entity` at `at`, since tick `since`.
    Lying { entity: u32, at: [f32; 3], since: u32 },
}

/// One job: a parcel from the shop's door to the house's.
#[derive(Clone, Debug)]
pub struct Job {
    pub id: u32,
    /// Where the parcel is fetched and where it is handed in: the middle of each door's sill, metres.
    pub shop: [f32; 3],
    pub house: [f32; 3],
    /// What handing it in pays within the par time.
    pub pay: u32,
    /// Seconds from fetching it to hand it in for the full pay.
    pub par: f32,
    /// Who took it off the board; `None` while it is on the board, or once they left.
    pub courier: Option<u32>,
    /// The tick its parcel was fetched.
    pub fetched: Option<u32>,
    pub parcel: Whereabouts,
}

impl Job {
    /// On the board, for anyone to take.
    fn offered(&self) -> bool {
        self.courier.is_none() && self.fetched.is_none()
    }

    /// What handing it in pays at `tick`: in full within its par time, then less, down to
    /// `DELIVERY_FLOOR` of it once as long again has gone by.
    pub fn pay_at(&self, tick: u32) -> u32 {
        let late = self.fetched.map_or(0.0, |fetched| tick.wrapping_sub(fetched) as f32 / TICK_RATE as f32 - self.par);
        let share = if late <= 0.0 { 1.0 } else { (1.0 - (1.0 - DELIVERY_FLOOR) * late / self.par).max(DELIVERY_FLOOR) };
        (self.pay as f32 * share).round() as u32
    }

    /// Seconds of its par time left at `tick`: negative once over it, the whole of it before it is fetched.
    fn left_at(&self, tick: u32) -> f32 {
        self.fetched.map_or(self.par, |fetched| self.par - tick.wrapping_sub(fetched) as f32 / TICK_RATE as f32)
    }
}

/// What a job between doors `distance` metres apart pays, and its par time in seconds.
pub fn terms(distance: f32) -> (u32, f32) {
    let pay = (DELIVERY_BASE + DELIVERY_PER_KM * distance / 1000.0).round() as u32;
    (pay, DELIVERY_PAR_GRACE + distance / DELIVERY_PAR_SPEED)
}

/// The board, the jobs taken off it and what the players are to be told. `World::deliveries` is
/// `None` in a world without the mode.
#[derive(Clone, Debug)]
pub struct DeliveryState {
    /// The doors parcels are fetched at, and handed in at, metres.
    pub shops: Vec<[f32; 3]>,
    pub houses: Vec<[f32; 3]>,
    /// How many jobs the board holds at once.
    pub wanted: usize,
    /// Every job, on the board or taken, oldest first.
    pub jobs: Vec<Job>,
    next: u32,
    /// Whether the board changed, so everyone is to be told it.
    board_changed: bool,
    /// Players whose own jobs changed.
    dirty: BTreeSet<u32>,
    /// To be sent at the end of the tick (`Game::step`): each player's phone, and the parcels handed in.
    pub phones: Vec<(u32, Jobs)>,
    pub delivered: Vec<Delivered>,
}

impl DeliveryState {
    pub fn new(shops: Vec<[f32; 3]>, houses: Vec<[f32; 3]>, wanted: usize) -> Self {
        DeliveryState { shops, houses, wanted, jobs: Vec::new(), next: 1, board_changed: true, dirty: BTreeSet::new(), phones: Vec::new(), delivered: Vec::new() }
    }

    /// The shops' and the houses' doors of `map`: an error when it has no shop, or fewer than two houses.
    pub fn from_map(map: &WorldFile, wanted: usize) -> Result<Self, String> {
        let (mut shops, mut houses) = (Vec::new(), Vec::new());
        for (index, building) in map.buildings.iter().enumerate() {
            let list = match building.kind {
                BUILDING_SHOP => &mut shops,
                BUILDING_PLAIN | BUILDING_VILLA | BUILDING_FLATS | BUILDING_FARMHOUSE | BUILDING_CABIN => &mut houses,
                _ => continue,
            };
            if let Some(door) = map.looks.get(index).and_then(|look| look.windows.iter().find(|window| window.door && !window.is_gate())) {
                let n = door.corners.len() as f32;
                let [x, z] = door.corners.iter().fold([0.0, 0.0], |[x, z], c| [x + c[0] / n, z + c[2] / n]);
                let floor = door.corners.iter().map(|c| c[1]).fold(f32::INFINITY, f32::min);
                list.push([x, floor, z]);
            }
        }
        if shops.is_empty() || houses.len() < 2 {
            return Err(format!("deliveries need a shop and two houses with doors; the world has {} shops and {} houses", shops.len(), houses.len()));
        }
        Ok(Self::new(shops, houses, wanted))
    }

    /// The job `id`, if it is still on.
    pub fn job(&self, id: u32) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }

    fn job_mut(&mut self, id: u32) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|job| job.id == id)
    }

    /// Ends job `id`, telling whoever had a hand in it.
    fn end(&mut self, id: u32) {
        let Some(at) = self.jobs.iter().position(|job| job.id == id) else { return };
        let job = self.jobs.remove(at);
        self.touch(&job);
    }

    /// Marks for telling the players job `job` concerns: its courier and whoever carries its parcel.
    fn touch(&mut self, job: &Job) {
        self.dirty.extend(job.courier);
        if let Whereabouts::Carried(by) = job.parcel {
            self.dirty.insert(by);
        }
    }

    /// What player `id`'s phone shows at `tick`.
    fn phone(&self, id: u32, tick: u32) -> Jobs {
        let offers = self
            .jobs
            .iter()
            .filter(|job| job.offered())
            .map(|job| JobOffer {
                id: job.id,
                shop_x: to_cm(job.shop[0]),
                shop_z: to_cm(job.shop[2]),
                house_x: to_cm(job.house[0]),
                house_z: to_cm(job.house[2]),
                pay: job.pay,
                par: job.par.round() as u16,
            })
            .collect();
        let mine = self
            .jobs
            .iter()
            .filter_map(|job| {
                let (stage, [x, _, z]) = match (job.parcel, job.courier == Some(id)) {
                    (Whereabouts::Carried(by), _) if by == id => (JOB_CARRY, job.house),
                    (Whereabouts::Shop, true) => (JOB_FETCH, job.shop),
                    (Whereabouts::Lying { at, .. }, true) => (JOB_LOST, at),
                    // In someone else's hands: they are making for the house.
                    (Whereabouts::Carried(_), true) => (JOB_LOST, job.house),
                    _ => return None,
                };
                let left = job.left_at(tick).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                Some(JobState { id: job.id, stage, x: to_cm(x), z: to_cm(z), pay: job.pay_at(tick), left })
            })
            .collect();
        Jobs { tag: S_JOBS, offers, mine }
    }
}

/// Whether `a` and `b` are within `DELIVERY_REACH` across the ground and `DELIVERY_REACH_Y` up or down.
fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    (a[0] - b[0]).hypot(a[2] - b[2]) <= DELIVERY_REACH && (a[1] - b[1]).abs() <= DELIVERY_REACH_Y
}

/// Where player `id` stands, if they are alive and on foot.
fn on_foot(world: &World, id: u32) -> Option<[f32; 3]> {
    let player = world.entities.get::<Player>(id).filter(|player| player.seat.is_none())?;
    world.entities.alive(id).then_some(player.character.position)
}

/// Whether player `id` carries a parcel.
pub fn carries_parcel(world: &World, id: u32) -> bool {
    world.entities.has::<Parcel>(id)
}

/// The parcels lying on the ground, for replication: their ids and where they lie.
pub fn lying(world: &World) -> Vec<(u32, [f32; 3])> {
    world.entities.ids::<With<LyingParcel>>().into_iter().filter_map(|id| Some((id, world.entities.get::<Transform>(id)?.position))).collect()
}

/// Puts new jobs on the board until it holds `wanted`, each from a shop to a house
/// `DELIVERY_MIN_DISTANCE..=DELIVERY_MAX_DISTANCE` away, picked at random.
fn fill_board(world: &mut World) {
    let Some(state) = &world.deliveries else { return };
    let mut missing = state.wanted.saturating_sub(state.jobs.iter().filter(|job| job.offered()).count());
    let mut tries = 0;
    while missing > 0 && tries < TRIES {
        tries += 1;
        let (a, b) = (world.rng.next_u64(), world.rng.next_u64());
        let state = world.deliveries.as_mut().unwrap();
        let shop = state.shops[(a % state.shops.len() as u64) as usize];
        let house = state.houses[(b % state.houses.len() as u64) as usize];
        let distance = (house[0] - shop[0]).hypot(house[2] - shop[2]);
        if !(DELIVERY_MIN_DISTANCE..=DELIVERY_MAX_DISTANCE).contains(&distance) {
            continue;
        }
        let (pay, par) = terms(distance);
        let id = state.next;
        state.next += 1;
        state.jobs.push(Job { id, shop, house, pay, par, courier: None, fetched: None, parcel: Whereabouts::Shop });
        state.board_changed = true;
        missing -= 1;
    }
}

/// What a tap on player `id`'s phone asks: to take a job off the board, if they have no other parcel
/// to fetch or carry, or to give back one they have not fetched.
pub fn act(world: &mut World, id: u32, phone: Phone) {
    if !world.entities.has::<Player>(id) {
        return;
    }
    let carrying = carries_parcel(world, id);
    let Some(state) = world.deliveries.as_mut() else { return };
    match phone.action {
        PHONE_ACCEPT => {
            let busy = carrying || state.jobs.iter().any(|job| job.courier == Some(id) && job.fetched.is_none());
            let Some(job) = state.job_mut(phone.job).filter(|job| job.offered()) else { return };
            if busy {
                return;
            }
            job.courier = Some(id);
        }
        PHONE_CANCEL => {
            let Some(job) = state.job_mut(phone.job).filter(|job| job.courier == Some(id) && job.fetched.is_none()) else { return };
            job.courier = None;
        }
        _ => return,
    }
    state.board_changed = true;
    state.dirty.insert(id);
}

/// Lays job `job`'s parcel on the ground at `at`.
fn lay(world: &mut World, job: u32, at: [f32; 3]) {
    let entity = world.alloc_id();
    world.entities.spawn(entity, (Transform { position: at, yaw: 0.0 }, LyingParcel { job }));
    let tick = world.tick;
    let state = world.deliveries.as_mut().unwrap();
    if let Some(job) = state.job_mut(job) {
        job.parcel = Whereabouts::Lying { entity, at, since: tick };
        let job = job.clone();
        state.touch(&job);
    }
}

/// Takes the parcel off player `id`, if they carry one, and lays it where they are; with nowhere to
/// put it (off the world), its job is over.
fn drop_from(world: &mut World, id: u32) {
    let Some(Parcel { job }) = world.entities.get::<Parcel>(id).copied() else { return };
    world.entities.remove::<Parcel>(id);
    let Some(state) = world.deliveries.as_mut() else { return };
    state.dirty.insert(id);
    match drop_spot(world, id) {
        Some(at) => lay(world, job, at),
        None => world.deliveries.as_mut().unwrap().end(job),
    }
}

/// Puts job `job`'s parcel on player `id`'s back.
fn give(world: &mut World, id: u32, job: u32) {
    world.entities.insert(id, Parcel { job });
    let tick = world.tick;
    let state = world.deliveries.as_mut().unwrap();
    let Some(entry) = state.job_mut(job) else { return };
    entry.parcel = Whereabouts::Carried(id);
    entry.fetched.get_or_insert(tick);
    let entry = entry.clone();
    state.touch(&entry);
}

/// Pays player `id` for handing in job `job`'s parcel, and ends the job.
fn deliver(world: &mut World, id: u32, job: u32) {
    world.entities.remove::<Parcel>(id);
    let tick = world.tick;
    let state = world.deliveries.as_mut().unwrap();
    let Some(entry) = state.job(job).cloned() else { return };
    let pay = entry.pay_at(tick);
    state.end(job);
    state.dirty.insert(id);
    let cash = {
        let mut player = world.entities.of_mut::<Player>(id);
        player.score.cash = player.score.cash.saturating_add(pay);
        player.score.cash
    };
    let courier = entry.courier.unwrap_or(0);
    world.deliveries.as_mut().unwrap().delivered.push(Delivered { tag: S_DELIVERED, by: id, courier, job, pay, cash });
}

impl Mechanic for Deliveries {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        if world.deliveries.is_none() {
            return;
        }
        fill_board(world);
        let tick = world.tick;
        let jobs: Vec<Job> = world.deliveries.as_ref().unwrap().jobs.clone();
        for job in jobs {
            match job.parcel {
                Whereabouts::Shop => {
                    let Some(courier) = job.courier else { continue };
                    if on_foot(world, courier).is_some_and(|at| near(at, job.shop)) && !carries_parcel(world, courier) {
                        give(world, courier, job.id);
                    }
                }
                Whereabouts::Carried(by) => {
                    if on_foot(world, by).is_some_and(|at| near(at, job.house)) {
                        deliver(world, by, job.id);
                    }
                }
                Whereabouts::Lying { entity, at, since } => {
                    if tick.wrapping_sub(since) as f32 >= DELIVERY_SPOIL_SECONDS * TICK_RATE as f32 {
                        world.entities.despawn(entity);
                        world.deliveries.as_mut().unwrap().end(job.id);
                    } else if let Some(taker) = within(world, at, DELIVERY_REACH, DELIVERY_REACH_Y).into_iter().find(|&id| !carries_parcel(world, id)) {
                        world.entities.despawn(entity);
                        give(world, taker, job.id);
                    }
                }
            }
        }

        let state = world.deliveries.as_mut().unwrap();
        // A running job's pay and par time change by the second.
        if tick.is_multiple_of(TICK_RATE) {
            let running: Vec<Job> = state.jobs.iter().filter(|job| job.courier.is_some() || job.fetched.is_some()).cloned().collect();
            for job in &running {
                state.touch(job);
            }
        }
        let to: Vec<u32> = if std::mem::take(&mut state.board_changed) {
            world.entities.ids::<With<Player>>()
        } else {
            std::mem::take(&mut state.dirty).into_iter().filter(|&id| world.entities.has::<Player>(id)).collect()
        };
        let state = world.deliveries.as_mut().unwrap();
        state.dirty.clear();
        let phones: Vec<(u32, Jobs)> = to.into_iter().map(|id| (id, state.phone(id, tick))).collect();
        state.phones.extend(phones);
    }

    /// A joiner is shown the board.
    fn on_join(&mut self, world: &mut World, id: u32) {
        if let Some(state) = world.deliveries.as_mut() {
            state.dirty.insert(id);
        }
    }

    /// A courier who leaves drops what they carry, and gives back the job they had not fetched.
    fn on_leave(&mut self, world: &mut World, id: u32) {
        drop_from(world, id);
        let Some(state) = world.deliveries.as_mut() else { return };
        for job in state.jobs.iter_mut().filter(|job| job.courier == Some(id)) {
            job.courier = None;
            if job.fetched.is_none() {
                state.board_changed = true;
            }
        }
        state.dirty.remove(&id);
    }

    /// A courier who died lets their parcel fall where they lie.
    fn react(&mut self, world: &mut World) {
        if world.deliveries.is_none() {
            return;
        }
        let dead: Vec<u32> = world.happened::<Died>().map(|death| death.victim).collect();
        for id in dead {
            drop_from(world, id);
        }
    }
}

#[cfg(test)]
mod tests;
