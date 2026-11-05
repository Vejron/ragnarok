use super::*;
use crate::game::{Command, Game, Outgoing};
use shared::golf::{Course, Hole};
use shared::golf::{charge, line_frames};
use shared::items::{ITEM_DRIVER, ITEM_PUTTER};
use shared::protocol::{InputFrame, ItemSlot, NOTICE_SAY, Notice, S_NOTICE, decode, peek_tag};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Hole 1 is played from the tee at the origin to a cup 30 m up -Z, par 3.
const TEE: [f32; 2] = [0.0, 0.0];
const CUP: [f32; 2] = [0.0, -30.0];

/// A flat 400 m square with a one-hole course on it.
fn game() -> Game {
    let course = Course { holes: vec![Hole { number: 1, par: Some(3), tee: TEE, cup: CUP }], surfaces: Vec::new() };
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[5_000, 5_000]], golf: Some(course), ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game.world.rng = crate::rng::Rng::from_seed(5);
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// Puts player `id` standing at `[x, z]` with `item` in their first slot.
fn golfer(game: &mut Game, id: u32, [x, z]: [f32; 2], item: u8) {
    let mut player = game.world.entities.of_mut::<Player>(id);
    player.character.position = [x, sim::STANDING_Y, z];
    player.character.vy = 0.0;
    player.hands.hotbar[0] = ItemSlot { item, count: 1, ammo: 0 };
}

/// Steps `ticks` with player `id` facing -Z and `buttons` held.
fn hold(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, buttons, slot: 0, ..Default::default() }]));
        game.step();
    }
}

/// Swings on the meter: fire held `held` frames after the press, let go, and pressed again `error`
/// frames off the line (`None`: never), and then a few frames for the input to reach the server.
fn swing(game: &mut Game, id: u32, seq: &mut u32, held: u32, error: Option<i32>) {
    hold(game, id, seq, BUTTON_FIRE, held + 1);
    match error {
        Some(error) => {
            let after = line_frames(charge(held)) as i32 + error;
            hold(game, id, seq, 0, after as u32);
            hold(game, id, seq, BUTTON_FIRE, 1);
        }
        None => hold(game, id, seq, 0, line_frames(charge(held)) + GOLF_MISS_FRAMES + 1),
    }
    hold(game, id, seq, 0, 3);
}

/// Swings at the ball on the tee with the driver, `error` frames off the line, and lets it come to rest.
fn drive(error: Option<i32>) -> ([f32; 3], Card) {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let (ball, ..) = the_ball(&game);
    golfer(&mut game, id, [-0.6, 0.0], ITEM_DRIVER);
    let mut seq = 0;
    swing(&mut game, id, &mut seq, GOLF_CHARGE_TICKS * 3 / 4, error);
    hold(&mut game, id, &mut seq, 0, 20 * TICK_RATE);
    let (_, at, v) = the_ball_or_new(&game, ball);
    assert_eq!(v, [0.0; 3], "at rest");
    (at, *game.world.entities.get::<Card>(ball).unwrap())
}

fn the_ball(game: &Game) -> (u32, [f32; 3], [f32; 3]) {
    let balls = balls(&game.world);
    assert_eq!(balls.len(), 1, "one ball, on the tee");
    let (id, at) = balls[0];
    (id, at, game.world.entities.get::<Velocity>(id).unwrap().0)
}

fn notices(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<String> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_NOTICE) => decode::<Notice>(&bytes).filter(|notice| notice.kind == NOTICE_SAY).map(|notice| notice.text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_ball_waits_on_the_tee_and_a_released_swing_drives_it_up_the_hole() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let (ball, at, _) = the_ball(&game);
    assert!((at[0] - TEE[0]).abs() < 1e-3 && (at[2] - TEE[1]).abs() < 1e-3);
    // Stood over it, side-on as a golfer stands: it lies by their right foot, and they aim up the hole.
    golfer(&mut game, id, [-0.6, 0.0], ITEM_DRIVER);
    let mut seq = 0;
    hold(&mut game, id, &mut seq, BUTTON_FIRE, GOLF_CHARGE_TICKS / 2 + 1);
    hold(&mut game, id, &mut seq, 0, line_frames(0.5) - 1);
    assert_eq!(game.world.entities.get::<Velocity>(ball).unwrap().0, [0.0; 3], "charging and letting go do not hit it");
    // Pressed again on the line: it is struck straight, at the power let go at. The input reaches the
    // server a frame behind, so a few ticks after.
    hold(&mut game, id, &mut seq, BUTTON_FIRE, 1);
    hold(&mut game, id, &mut seq, 0, 3);
    let v = game.world.entities.get::<Velocity>(ball).unwrap().0;
    let speed = shared::vec3::length(v);
    let expected = GOLF_DRIVER_SPEED * 0.5;
    assert!((speed - expected).abs() < 2.0, "a half swing: {speed} m/s, not {expected}");
    assert!(v[2] < -0.9 * speed && v[1] > 0.0 && v[0].abs() < 1e-3, "straight up the hole and in the air: {v:?}");
    assert!(game.world.entities.get::<Spin>(ball).is_none(), "no spin on it");
    assert_eq!(game.world.entities.get::<Card>(ball), Some(&Card { hole: Some(1), strokes: 1 }));
    assert_eq!(game.world.entities.get::<Projectile>(ball).unwrap().owner, id);
    // It flies, lands and rolls out, and comes to rest far up the hole.
    hold(&mut game, id, &mut seq, 0, 20 * TICK_RATE);
    let (_, at, v) = the_ball_or_new(&game, ball);
    assert_eq!(v, [0.0; 3], "at rest");
    assert!(at[2] < -40.0, "carried and rolled: {at:?}");
    assert!((at[1] - GOLF_BALL_RADIUS).abs() < 0.05, "lying on the ground: {at:?}");
}

/// Ball `id` where it is, among the tee's new one.
fn the_ball_or_new(game: &Game, id: u32) -> (u32, [f32; 3], [f32; 3]) {
    let at = game.world.entities.get::<Transform>(id).unwrap().position;
    (id, at, game.world.entities.get::<Velocity>(id).unwrap().0)
}

#[test]
fn a_swing_with_no_ball_in_reach_moves_nothing() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let (ball, ..) = the_ball(&game);
    golfer(&mut game, id, [10.0, 10.0], ITEM_DRIVER);
    let mut seq = 0;
    swing(&mut game, id, &mut seq, GOLF_CHARGE_TICKS, Some(0));
    assert!(matches!(game.world.entities.of::<Player>(id).hands.golf.stage, shared::golf::Stage::Resting { .. }), "it swung");
    assert_eq!(game.world.entities.get::<Velocity>(ball).unwrap().0, [0.0; 3]);
    assert_eq!(game.world.entities.get::<Card>(ball).unwrap().strokes, 0);
}

#[test]
fn a_putt_rolls_into_the_cup_and_everyone_hears_the_score() {
    let mut game = game();
    let (id, mut inbox) = join(&mut game);
    let (ball, ..) = the_ball(&game);
    // Two strokes so far; the ball lies 3 m short of the cup.
    {
        let mut transform = game.world.entities.get_mut::<Transform>(ball).unwrap();
        transform.position = [CUP[0], GOLF_BALL_RADIUS, CUP[1] + 3.0];
    }
    game.world.entities.insert(ball, Card { hole: Some(1), strokes: 2 });
    game.world.entities.get_mut::<Projectile>(ball).unwrap().owner = id;
    golfer(&mut game, id, [CUP[0] - 0.6, CUP[1] + 3.0], ITEM_PUTTER);
    // A putt hard enough to get there and soft enough to drop: on the rough it rolls at
    // `GOLF_ROLL_ROUGH`, so it wants v² = 2 a d, a little over.
    let wanted = (2.0 * GOLF_ROLL_ROUGH * GRAVITY * GOLF_GRAVITY_SCALE * 3.3).sqrt() / GOLF_PUTTER_SPEED;
    let held = (0..GOLF_CHARGE_TICKS).min_by(|&a, &b| (charge(a) - wanted).abs().total_cmp(&(charge(b) - wanted).abs())).unwrap();
    let mut seq = 0;
    swing(&mut game, id, &mut seq, held, Some(0));
    hold(&mut game, id, &mut seq, 0, 4 * TICK_RATE);
    assert!(game.world.entities.get::<Rolls>(ball).is_none(), "holed");
    let said = notices(&mut inbox);
    assert!(said.iter().any(|text| text.contains("holed out on 1 in 3 strokes (par 3): par")), "{said:?}");
}

#[test]
fn a_rolling_ball_slows_to_a_stop_by_its_friction() {
    let mut game = game();
    let (ball, ..) = the_ball(&game);
    // Gently rolling across the flat: it slows to a stop.
    game.world.entities.get_mut::<Velocity>(ball).unwrap().0 = [2.0, 0.0, 0.0];
    for _ in 0..10 * TICK_RATE {
        game.step();
    }
    let (_, at, v) = the_ball_or_new(&game, ball);
    assert_eq!(v, [0.0; 3]);
    let rolled = at[0];
    let expected = 4.0 / (2.0 * GOLF_ROLL_ROUGH * GRAVITY * GOLF_GRAVITY_SCALE);
    assert!((rolled - expected).abs() < 0.3, "rolled {rolled} m, not {expected}");
}

#[test]
fn early_on_the_meter_hooks_it_left_and_late_slices_it_right() {
    let (straight, _) = drive(Some(0));
    let (hook, _) = drive(Some(-((GOLF_PURE_FRAMES + GOLF_CURVE_FRAMES) as i32)));
    let (slice, card) = drive(Some((GOLF_PURE_FRAMES + GOLF_CURVE_FRAMES) as i32));
    assert!(straight[0].abs() < 1.0, "straight down the middle: {straight:?}");
    // Facing -Z, left is -X.
    assert!(hook[0] < -8.0, "hooked left: {hook:?}");
    assert!(slice[0] > 8.0, "sliced right: {slice:?}");
    assert!(hook[2] < -40.0 && slice[2] < -40.0, "still up the hole: {hook:?} {slice:?}");
    assert_eq!(card.strokes, 1);
}

#[test]
fn a_swing_never_pressed_again_is_topped_short_and_off_line() {
    let (straight, _) = drive(Some(0));
    let (topped, card) = drive(None);
    assert_eq!(card.strokes, 1, "it struck");
    assert!(topped[2] > straight[2] / 2.0, "short: {topped:?} against {straight:?}");
    assert!(topped[0] > 0.5, "off to the right: {topped:?}");
}

#[test]
fn a_late_frame_repeated_without_fire_does_not_let_go_of_a_swing() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    golfer(&mut game, id, [-0.6, 0.0], ITEM_DRIVER);
    let mut seq = 0;
    hold(&mut game, id, &mut seq, BUTTON_FIRE, 20);
    // The connection stalls: the server repeats the last frame, with fire taken off it.
    for _ in 0..5 {
        game.step();
    }
    let stage = game.world.entities.of::<Player>(id).hands.golf.stage;
    assert!(matches!(stage, shared::golf::Stage::Charging { .. }), "still charging: {stage:?}");
}
