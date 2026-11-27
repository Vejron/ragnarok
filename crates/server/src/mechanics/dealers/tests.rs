use super::*;
use crate::game::{Command, Game, Outgoing, Player};
use shared::consts::*;
use shared::protocol::{C_PHONE, InputFrame, KIND_HELI, KIND_PLANE, KIND_TANK, KIND_TITAN, KIND_VOLVO_240, Phone, decode, peek_tag};
use shared::dealer::resale;
use shared::world::{Dealer, WORLD_VERSION, WorldFile};
use sim::STANDING_Y;
use tokio::sync::{mpsc, oneshot};

/// A flat 600 m square with a dealer in the middle, unturned: its carport runs from the wall at
/// z = -`DEALER_PORT[1]` to its mouth at z = 0, open towards +Z.
fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 30_000, spawns: vec![[0, -25_000]], dealers: vec![Dealer { at: [0, 0], yaw: 0, base: 0 }], ..Default::default() };
    Game::new(map, 0).unwrap()
}

/// Joins a player with `cash` and stands them on foot at `(x, z)`.
fn buyer(game: &mut Game, cash: u32, [x, z]: [f32; 2]) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    let id = response.try_recv().unwrap().unwrap();
    for seq in 1..=30 {
        game.handle(Command::Input(id, 0, vec![InputFrame { seq, ..Default::default() }]));
        game.step();
    }
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [x, STANDING_Y, z];
    player.score.cash = cash;
    (id, inbox)
}

/// Taps `kind` on player `id`'s Dealer app and runs a tick.
fn tap(game: &mut Game, id: u32, kind: u8) {
    game.handle(Command::Phone(id, Phone { tag: C_PHONE, action: PHONE_ORDER, job: u32::from(kind) }));
    game.step();
}

/// Every `Ordered` in `inbox`.
fn ordered(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Ordered> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_ORDERED) => decode::<Ordered>(&bytes),
            _ => None,
        })
        .collect()
}

fn vehicles(game: &Game) -> usize {
    game.world.entities.count::<Vehicle>()
}

#[test]
fn an_order_at_a_dealer_takes_the_price_and_puts_the_vehicle_in_the_carport() {
    let mut game = game();
    let (id, mut inbox) = buyer(&mut game, 1_000, [0.0, 6.0]);
    let cost = price(KIND_VOLVO_240).unwrap();
    tap(&mut game, id, KIND_VOLVO_240);
    let news = ordered(&mut inbox);
    assert_eq!(news.len(), 1, "everyone is told");
    let Ordered { by, kind, vehicle, cash, dealer, .. } = news[0];
    assert_eq!((by, kind, cash, dealer), (id, KIND_VOLVO_240, 1_000 - cost, 0));
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, 1_000 - cost);
    assert_eq!(game.world.entities.of::<Vehicle>(vehicle).kind, KIND_VOLVO_240);
    let pose = game.world.physics.car_pose(vehicle).unwrap();
    assert!(pose.position[0].abs() < 0.5 && (pose.position[2] + DEALER_PORT[1] / 2.0).abs() < 0.5, "under the roof: {:?}", pose.position);
    // It faces out of the mouth, +Z: a vehicle's ahead is (-sin, -cos) of its yaw.
    let yaw = game.world.entities.of::<Vehicle>(vehicle).home.1;
    assert!((-yaw.cos() - 1.0).abs() < 1e-3, "{yaw}");
}

#[test]
fn an_order_away_from_a_dealer_or_without_the_cash_does_nothing() {
    let mut game = game();
    let before = vehicles(&game);
    let (far, mut far_inbox) = buyer(&mut game, 10_000, [0.0, DEALER_REACH + 10.0]);
    tap(&mut game, far, KIND_VOLVO_240);
    assert!(ordered(&mut far_inbox).is_empty());
    assert_eq!(game.world.entities.of::<Player>(far).score.cash, 10_000, "nothing paid");
    let (poor, _) = buyer(&mut game, price(KIND_TANK).unwrap() - 1, [0.0, 5.0]);
    tap(&mut game, poor, KIND_TANK);
    assert_eq!(game.world.entities.of::<Player>(poor).score.cash, price(KIND_TANK).unwrap() - 1);
    assert_eq!(vehicles(&game), before);
}

#[test]
fn a_plane_or_the_titan_cannot_be_ordered() {
    let mut game = game();
    let before = vehicles(&game);
    let (id, _) = buyer(&mut game, 100_000, [0.0, 5.0]);
    tap(&mut game, id, KIND_PLANE);
    tap(&mut game, id, KIND_TITAN);
    assert_eq!(vehicles(&game), before);
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, 100_000);
}

#[test]
fn a_second_order_waits_while_the_carport_is_taken() {
    let mut game = game();
    let (id, _) = buyer(&mut game, 10_000, [0.0, 6.0]);
    tap(&mut game, id, KIND_VOLVO_240);
    let paid = game.world.entities.of::<Player>(id).score.cash;
    let count = vehicles(&game);
    tap(&mut game, id, KIND_VOLVO_240);
    assert_eq!((vehicles(&game), game.world.entities.of::<Player>(id).score.cash), (count, paid), "the first still stands there");
}

#[test]
fn a_helicopter_is_set_down_on_the_lot() {
    let mut game = game();
    let (id, mut inbox) = buyer(&mut game, 10_000, [6.0, 4.0]);
    tap(&mut game, id, KIND_HELI);
    let news = ordered(&mut inbox);
    assert_eq!(news.len(), 1);
    let pose = game.world.physics.car_pose(news[0].vehicle).unwrap();
    assert!(pose.position[0].abs() < 0.5 && (pose.position[2] - DEALER_HELI_OUT).abs() < 0.5, "out in front: {:?}", pose.position);
}

#[test]
fn a_bought_vehicle_that_burns_out_is_gone_not_put_back_in_the_carport() {
    let mut game = game();
    let (id, mut inbox) = buyer(&mut game, 1_000, [0.0, 6.0]);
    tap(&mut game, id, KIND_VOLVO_240);
    let vehicle = ordered(&mut inbox)[0].vehicle;
    crate::mechanics::for_tests::damage_entity(&mut game.world, vehicle, crate::mechanics::for_tests::Harm::new(u8::MAX, None, 0).unsparing());
    for _ in 0..(TICK_RATE * 30) {
        game.step();
        if !game.world.entities.has::<Vehicle>(vehicle) {
            return;
        }
    }
    panic!("the wreck is still there");
}

#[test]
fn a_player_joins_with_starting_cash_to_buy_with_before_any_delivery() {
    let mut game = game();
    let (outbox, _inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    let id = response.try_recv().unwrap().unwrap();
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, STARTING_CASH);
    assert!(price(KIND_VOLVO_240).unwrap() <= STARTING_CASH, "enough for a car");
}

/// A Volvo 240 placed at `(x, z)`, its driver `id` at the wheel, then put with its nose at dealer 0's
/// door facing in (`yaw` 0 faces -Z, the door's way), or turned `yaw`.
fn drive_in(game: &mut Game, id: u32, yaw: f32) -> u32 {
    let vid = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_VOLVO_240, [40.0, 1.2, 40.0], 0.0);
    assert!(crate::mechanics::vehicles::seat(&mut game.world, id, vid, 0));
    let half = vehicle_def(KIND_VOLVO_240).spec.half_extents[2];
    game.world.physics.reset_car(vid, [0.0, 0.8, -DEALER_PORT[1] + half + 0.4], yaw);
    game.step();
    game.step();
    vid
}

/// Every `Sold` in `inbox`.
fn sold(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Sold> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_SOLD) => decode::<Sold>(&bytes),
            _ => None,
        })
        .collect()
}

#[test]
fn a_placed_car_driven_in_through_the_door_is_sold_for_a_sixth_and_goes_home_as_new() {
    let mut game = game();
    let (id, mut inbox) = buyer(&mut game, 0, [0.0, 6.0]);
    let vid = drive_in(&mut game, id, 0.0);
    let news = sold(&mut inbox);
    assert_eq!(news.len(), 1, "sold once");
    let pay = resale(KIND_VOLVO_240, 1.0).unwrap();
    assert_eq!((news[0].by, news[0].vehicle, news[0].pay, news[0].cash), (id, vid, pay, pay));
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, pay);
    assert!(game.world.entities.of::<Player>(id).seat.is_none(), "set down");
    // Placed in the world, it is back at its home, not gone.
    let at = game.world.physics.car_pose(vid).unwrap().position;
    assert!((at[0] - 40.0).abs() < 1.0 && (at[2] - 40.0).abs() < 1.0, "home: {at:?}");
}

#[test]
fn a_bought_car_sold_back_is_gone_and_a_hurt_one_fetches_less() {
    let mut game = game();
    let (id, mut inbox) = buyer(&mut game, 0, [0.0, 6.0]);
    let vid = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_VOLVO_240, [40.0, 1.2, 40.0], 0.0);
    game.world.entities.insert(vid, Bought);
    game.world.entities.of_mut::<crate::entities::Health>(vid).hp = vehicle_def(KIND_VOLVO_240).hp / 2;
    assert!(crate::mechanics::vehicles::seat(&mut game.world, id, vid, 0));
    let half = vehicle_def(KIND_VOLVO_240).spec.half_extents[2];
    game.world.physics.reset_car(vid, [0.0, 0.8, -DEALER_PORT[1] + half + 0.4], 0.0);
    game.step();
    game.step();
    let news = sold(&mut inbox);
    assert_eq!(news.len(), 1);
    assert!(news[0].pay < resale(KIND_VOLVO_240, 1.0).unwrap(), "{} for a car at half its health", news[0].pay);
    assert!(!game.world.entities.has::<Vehicle>(vid), "gone for good");
}

#[test]
fn a_wanted_driver_or_a_car_backed_in_is_not_bought() {
    let mut game = game();
    game.world.fleet.police = true;
    let (id, mut inbox) = buyer(&mut game, 0, [0.0, 6.0]);
    crate::mechanics::emergency::charge_wanted(&mut game.world, id, 60.0, None);
    assert!(crate::mechanics::emergency::wanted_stars(&game.world, id) > 0);
    drive_in(&mut game, id, 0.0);
    assert!(sold(&mut inbox).is_empty(), "not while the police are after them");
    assert_eq!(game.world.entities.of::<Player>(id).score.cash, 0);

    let mut game = self::game();
    let (id, mut inbox) = buyer(&mut game, 0, [0.0, 6.0]);
    drive_in(&mut game, id, std::f32::consts::PI);
    assert!(sold(&mut inbox).is_empty(), "its nose must go in through the door");
}
