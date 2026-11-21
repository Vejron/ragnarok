use super::*;
use crate::game::{Command, Game, Outgoing};
use crate::mechanics::for_tests::{Harm, damage_entity};
use shared::protocol::{C_PHONE, WEAPON_RIFLE, decode, peek_tag};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// The one shop's door in these tests, and the one house's, 300 m apart.
const SHOP: [f32; 3] = [40.0, 0.0, 40.0];
const HOUSE: [f32; 3] = [40.0, 0.0, -260.0];
/// A second house, nearer the shop than any job may be, so every job goes to `HOUSE`.
const NEXT_DOOR: [f32; 3] = [50.0, 0.0, 40.0];

/// A flat 800 m square with one shop and a house to deliver to, and a board of `jobs`.
fn game(jobs: usize) -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game.world.rng = crate::rng::Rng::from_seed(3);
    game.world.deliveries = Some(DeliveryState::new(vec![SHOP], vec![HOUSE, NEXT_DOOR], jobs));
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

fn state(game: &Game) -> &DeliveryState {
    game.world.deliveries.as_ref().unwrap()
}

/// The jobs on the board.
fn offers(game: &Game) -> Vec<u32> {
    state(game).jobs.iter().filter(|job| job.offered()).map(|job| job.id).collect()
}

/// Puts player `id` standing at `[x, _, z]`.
fn stand(game: &mut Game, id: u32, [x, _, z]: [f32; 3]) {
    let mut player = game.world.entities.of_mut::<Player>(id);
    player.character.position = [x, sim::STANDING_Y, z];
    player.character.vy = 0.0;
}

fn tap(game: &mut Game, id: u32, action: u8, job: u32) {
    game.handle(Command::Phone(id, Phone { tag: C_PHONE, action, job }));
}

fn steps(game: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        game.step();
    }
}

/// What player's phone was last told, of what has come since the last look.
fn phone(inbox: &mut mpsc::Receiver<Outgoing>) -> Option<Jobs> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_JOBS) => decode::<Jobs>(&bytes),
            _ => None,
        })
        .last()
}

/// The deliveries told of since the last look.
fn delivered(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Delivered> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_DELIVERED) => decode::<Delivered>(&bytes),
            _ => None,
        })
        .collect()
}

/// A courier who took the board's first job and fetched its parcel at the shop.
fn fetched() -> (Game, u32, u32, mpsc::Receiver<Outgoing>) {
    let mut game = game(3);
    let (id, inbox) = join(&mut game);
    game.step();
    let job = offers(&game)[0];
    tap(&mut game, id, PHONE_ACCEPT, job);
    stand(&mut game, id, SHOP);
    game.step();
    assert!(carries_parcel(&game.world, id));
    (game, id, job, inbox)
}

fn kill(game: &mut Game, id: u32) {
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE));
    game.step();
    assert!(!game.world.entities.alive(id));
}

#[test]
fn a_joiner_is_shown_a_full_board_of_jobs_to_the_house() {
    let mut game = game(3);
    let (_, mut inbox) = join(&mut game);
    game.step();
    let jobs = phone(&mut inbox).expect("the board");
    assert_eq!(jobs.offers.len(), 3);
    assert!(jobs.mine.is_empty());
    let (pay, par) = terms(300.0);
    for offer in &jobs.offers {
        assert_eq!([offer.house_x, offer.house_z], [4_000, -26_000], "only the far house is far enough");
        assert_eq!((offer.pay, offer.par), (pay, par.round() as u16));
    }
}

#[test]
fn accepting_an_offer_takes_it_off_the_board_and_a_new_one_appears() {
    let mut game = game(3);
    let (id, mut inbox) = join(&mut game);
    game.step();
    let job = offers(&game)[0];
    tap(&mut game, id, PHONE_ACCEPT, job);
    game.step();
    assert!(!offers(&game).contains(&job));
    assert_eq!(offers(&game).len(), 3, "the board is filled again");
    let jobs = phone(&mut inbox).unwrap();
    assert_eq!(jobs.mine.len(), 1);
    assert_eq!((jobs.mine[0].id, jobs.mine[0].stage), (job, JOB_FETCH));
    assert_eq!([jobs.mine[0].x, jobs.mine[0].z], [4_000, 4_000], "make for the shop");
}

#[test]
fn a_courier_takes_one_job_at_a_time() {
    let mut game = game(3);
    let (id, _inbox) = join(&mut game);
    game.step();
    let board = offers(&game);
    tap(&mut game, id, PHONE_ACCEPT, board[0]);
    tap(&mut game, id, PHONE_ACCEPT, board[1]);
    game.step();
    assert!(offers(&game).contains(&board[1]));
}

#[test]
fn cancelling_before_pickup_returns_the_offer() {
    let mut game = game(3);
    let (id, _inbox) = join(&mut game);
    game.step();
    let job = offers(&game)[0];
    tap(&mut game, id, PHONE_ACCEPT, job);
    game.step();
    tap(&mut game, id, PHONE_CANCEL, job);
    game.step();
    assert!(offers(&game).contains(&job));
}

#[test]
fn walking_to_the_shop_door_picks_up_the_parcel_and_it_cannot_be_given_back() {
    let (mut game, id, job, mut inbox) = fetched();
    game.step();
    let jobs = phone(&mut inbox).unwrap();
    assert_eq!(jobs.mine[0].stage, JOB_CARRY);
    assert_eq!([jobs.mine[0].x, jobs.mine[0].z], [4_000, -26_000], "make for the house");
    tap(&mut game, id, PHONE_CANCEL, job);
    game.step();
    assert!(carries_parcel(&game.world, id));
}

#[test]
fn nobody_fetches_a_parcel_without_taking_its_job() {
    let mut game = game(3);
    let (id, _inbox) = join(&mut game);
    game.step();
    stand(&mut game, id, SHOP);
    steps(&mut game, 2);
    assert!(!carries_parcel(&game.world, id));
}

#[test]
fn delivering_pays_full_before_par_and_less_after() {
    let (mut game, id, job, mut inbox) = fetched();
    let pay = state(&game).job(job).unwrap().pay;
    stand(&mut game, id, HOUSE);
    game.step();
    assert!(!carries_parcel(&game.world, id));
    assert!(state(&game).job(job).is_none());
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, pay);
    let told = delivered(&mut inbox);
    assert_eq!(told.len(), 1);
    assert_eq!((told[0].by, told[0].courier, told[0].job, told[0].pay, told[0].cash), (id, id, job, pay, pay));

    let (mut game, id, job, _inbox) = fetched();
    let entry = state(&game).job(job).unwrap().clone();
    // Halfway from the par time to as long again: halfway from the whole pay to the floor.
    steps(&mut game, (entry.par * 1.5 * TICK_RATE as f32) as u32);
    stand(&mut game, id, HOUSE);
    game.step();
    let expected = (entry.pay as f32 * (1.0 + DELIVERY_FLOOR) / 2.0).round() as u32;
    assert!(game.world.entities.of::<Player>(id).score.cash.abs_diff(expected) <= 1, "{} paid, about {expected} expected", game.world.entities.of::<Player>(id).score.cash);
}

#[test]
fn a_late_delivery_still_pays_the_floor() {
    let mut job = Job { id: 1, shop: SHOP, house: HOUSE, pay: 100, par: 60.0, courier: None, fetched: Some(0), parcel: Whereabouts::Shop };
    assert_eq!(job.pay_at(60 * TICK_RATE), 100);
    assert_eq!(job.pay_at(10_000 * TICK_RATE), (100.0 * DELIVERY_FLOOR) as u32);
    job.fetched = None;
    assert_eq!(job.pay_at(10_000 * TICK_RATE), 100, "the clock starts at the shop");
}

#[test]
fn a_couriers_parcel_drops_where_they_die_and_a_robber_who_delivers_it_is_paid() {
    let (mut game, courier, job, mut courier_inbox) = fetched();
    let (robber, mut robber_inbox) = join(&mut game);
    stand(&mut game, courier, [0.0, 0.0, -100.0]);
    stand(&mut game, robber, [30.0, 0.0, -100.0]);
    game.step();
    kill(&mut game, courier);
    assert!(!carries_parcel(&game.world, courier));
    let [(_, at)] = lying(&game.world)[..] else { panic!("one parcel on the ground") };
    assert!(at[0].abs() < 0.5 && (at[2] + 100.0).abs() < 0.5, "{at:?}");
    game.step();
    let lost = phone(&mut courier_inbox).unwrap();
    assert_eq!(lost.mine[0].stage, JOB_LOST);

    stand(&mut game, robber, [0.5, 0.0, -100.0]);
    game.step();
    assert!(carries_parcel(&game.world, robber));
    assert!(lying(&game.world).is_empty());
    game.step();
    let robbed = phone(&mut courier_inbox).unwrap();
    assert_eq!((robbed.mine[0].stage, robbed.mine[0].x, robbed.mine[0].z), (JOB_LOST, 4_000, -26_000), "the courier knows where the robber is making for");
    assert_eq!(phone(&mut robber_inbox).unwrap().mine[0].stage, JOB_CARRY);

    stand(&mut game, robber, HOUSE);
    game.step();
    let told = delivered(&mut robber_inbox);
    assert_eq!((told[0].by, told[0].courier, told[0].job), (robber, courier, job));
    assert!(game.world.entities.of::<Player>(robber).score.cash > 0);
    assert_eq!(game.world.entities.of::<Player>(courier).score.cash, 0);
}

#[test]
fn a_soldier_carrying_a_parcel_cannot_take_a_second() {
    let (mut game, carrier, _, _inbox) = fetched();
    let (other, _other_inbox) = join(&mut game);
    game.step();
    let second = offers(&game)[0];
    tap(&mut game, other, PHONE_ACCEPT, second);
    stand(&mut game, other, SHOP);
    game.step();
    stand(&mut game, other, [0.0, 0.0, -100.0]);
    game.step();
    kill(&mut game, other);
    stand(&mut game, carrier, [0.0, 0.0, -100.0]);
    steps(&mut game, 2);
    assert_eq!(game.world.entities.get::<Parcel>(carrier).map(|parcel| parcel.job), Some(state(&game).jobs.iter().find(|job| job.parcel == Whereabouts::Carried(carrier)).unwrap().id));
    assert_eq!(lying(&game.world).len(), 1, "the second stays on the ground");
}

#[test]
fn a_parcel_left_lying_spoils_and_ends_the_job() {
    let (mut game, id, job, _inbox) = fetched();
    stand(&mut game, id, [0.0, 0.0, -100.0]);
    game.step();
    kill(&mut game, id);
    // Kept well away once they are back on their feet.
    for _ in 1..(DELIVERY_SPOIL_SECONDS * TICK_RATE as f32) as u32 {
        stand(&mut game, id, [200.0, 0.0, 200.0]);
        game.step();
    }
    assert_eq!(lying(&game.world).len(), 1, "not yet");
    game.step();
    assert!(lying(&game.world).is_empty());
    assert!(state(&game).job(job).is_none());
}

#[test]
fn leaving_frees_the_job() {
    let mut game = game(3);
    let (id, _inbox) = join(&mut game);
    game.step();
    let job = offers(&game)[0];
    tap(&mut game, id, PHONE_ACCEPT, job);
    game.step();
    game.handle(Command::Leave(id));
    game.step();
    assert!(offers(&game).contains(&job));
}

#[test]
fn a_courier_who_leaves_drops_the_parcel() {
    let (mut game, id, job, _inbox) = fetched();
    stand(&mut game, id, [0.0, 0.0, -100.0]);
    game.step();
    game.handle(Command::Leave(id));
    game.step();
    assert_eq!(lying(&game.world).len(), 1);
    assert!(matches!(state(&game).job(job).unwrap().parcel, Whereabouts::Lying { .. }));
    assert!(!offers(&game).contains(&job), "a fetched parcel is not put back on the board");
}

#[test]
fn a_world_without_shops_has_no_deliveries() {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, ..Default::default() };
    assert!(DeliveryState::from_map(&map, 3).is_err());
}
