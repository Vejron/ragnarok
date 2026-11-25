//! Getting in and out of a car by its doors, bailing out, and doors coming off (DOORS.md).

use super::*;
use shared::protocol::KIND_VOLVO_240;
use shared::vehicles::door_of;

/// A Volvo 240 standing off the road at `[x, z]`, facing -z, and a player who has settled in.
fn volvo(game: &mut Game, x: f32, z: f32) -> (u32, u32, u32) {
    let (id, _) = join(game);
    let mut seq = 0;
    idle(game, id, &mut seq, 30);
    let position = start_position(&game.world, KIND_VOLVO_240, x, z).unwrap();
    let vid = add_vehicle(&mut game.world, KIND_VOLVO_240, position, 0.0);
    idle(game, id, &mut seq, 20);
    (id, vid, seq)
}

/// Where the door of `seat` is got out of, in the world.
fn door_spot(game: &Game, vid: u32, seat: usize) -> [f32; 3] {
    let (_, door) = door_of(KIND_VOLVO_240, seat).unwrap();
    game.world.physics.car_pose(vid).unwrap().world_point(door.spot())
}

fn seated(game: &Game, id: u32) -> bool {
    game.world.entities.get::<Player>(id).unwrap().seat.is_some()
}

fn ticks_of(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32).round() as u32
}

#[test]
fn a_player_getting_out_of_a_car_opens_the_door_climbs_out_and_shuts_it_behind_them() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    assert!(seat(&mut game.world, id, vid, 0));
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, ticks_of(0.2));
    assert!(seated(&game, id), "still in the seat while the door opens");
    assert_eq!(game.world.entities.of::<Vehicle>(vid).doors.open, 0b01, "the driver's door is open");
    let seen = game.seen(vid).unwrap().doors.expect("its doors are told of");
    assert!(seen.left > 100 && seen.right == 0, "and is seen to swing open: {seen:?}");
    idle(&mut game, id, &mut seq, ticks_of(0.5));
    let step = game.seen(id).unwrap().seated.expect("still drawn in the car").step;
    assert!(step > 60 && step < 220, "part way out: {step}");
    idle(&mut game, id, &mut seq, ticks_of(0.5));
    assert!(!seated(&game, id), "out after a second");
    let at = game.world.entities.of::<Player>(id).character.position;
    let spot = door_spot(&game, vid, 0);
    assert!((at[0] - spot[0]).hypot(at[2] - spot[2]) < 0.2, "standing in the open door: {at:?}, not {spot:?}");
    idle(&mut game, id, &mut seq, ticks_of(0.6));
    assert_eq!(game.world.entities.of::<Vehicle>(vid).doors.open, 0, "and the door is shut behind them");
    assert!(game.seen(vid).unwrap().doors.is_none(), "nothing to tell of a car with its doors shut");
}

#[test]
fn a_player_getting_in_by_a_door_has_the_seat_at_once_and_drives_only_once_in() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    // At the passenger's door, not the one nearest the car's middle.
    let spot = door_spot(&game, vid, 1);
    game.world.entities.of_mut::<Player>(id).character.position = [spot[0], spot[1] + STANDING_Y, spot[2]];
    idle(&mut game, id, &mut seq, 3);
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 2);
    assert_eq!(game.world.entities.of::<Seats>(vid)[1], Some(id), "the seat by that door is theirs at once");
    assert!(game.seen(id).unwrap().seated.unwrap().step > 200, "drawn still outside it");
    // Switching to the wheel while climbing in does nothing.
    press(&mut game, id, &mut seq, (0, 0), BUTTON_RELOAD, ticks_of(0.5));
    assert_eq!(game.world.entities.of::<Seats>(vid)[1], Some(id), "still in their own seat");
    idle(&mut game, id, &mut seq, ticks_of(0.7));
    assert_eq!(game.seen(id).unwrap().seated.unwrap().step, 0, "in, and sitting");
    idle(&mut game, id, &mut seq, ticks_of(0.6));
    assert_eq!(game.world.entities.of::<Vehicle>(vid).doors.open, 0, "the door pulled shut");
}

#[test]
fn a_driver_climbing_in_cannot_drive_off_until_they_are_in() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    let spot = door_spot(&game, vid, 0);
    game.world.entities.of_mut::<Player>(id).character.position = [spot[0], spot[1] + STANDING_Y, spot[2]];
    idle(&mut game, id, &mut seq, 3);
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    let start = game.world.physics.car_pose(vid).unwrap().position;
    press(&mut game, id, &mut seq, (0, 127), 0, ticks_of(0.9));
    let now = game.world.physics.car_pose(vid).unwrap().position;
    assert!((now[0] - start[0]).hypot(now[2] - start[2]) < 0.1, "it stands while they climb in");
    press(&mut game, id, &mut seq, (0, 127), 0, ticks_of(1.5));
    let now = game.world.physics.car_pose(vid).unwrap().position;
    assert!((now[0] - start[0]).hypot(now[2] - start[2]) > 1.0, "and goes once they are at the wheel");
}

#[test]
fn leaving_a_car_going_fast_throws_you_out_knocked_down_at_its_speed() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    assert!(seat(&mut game.world, id, vid, 0));
    game.world.physics.set_car_velocity(vid, [0.0, 0.0, -20.0]);
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 2);
    assert!(!seated(&game, id), "out at once, with no door to climb out by");
    let player = game.world.entities.of::<Player>(id);
    assert!(player.character.down > 0, "knocked down");
    assert!(player.character.knock[1] < -15.0, "going on as fast as the car went: {:?}", player.character.knock);
    assert!(game.world.entities.of::<Health>(id).hp < PLAYER_MAX_HP, "and hurt by it");
}

#[test]
fn a_car_that_gets_going_while_someone_climbs_out_throws_them_out() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    assert!(seat(&mut game.world, id, vid, 1));
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, ticks_of(0.4));
    assert!(seated(&game, id), "climbing out");
    game.world.physics.set_car_velocity(vid, [0.0, 0.0, -12.0]);
    idle(&mut game, id, &mut seq, 1);
    assert!(!seated(&game, id), "thrown out");
    assert!(game.world.entities.of::<Player>(id).character.down > 0, "and knocked down");
}

#[test]
fn rear_seats_and_cars_without_doors_are_got_out_of_at_once() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    assert!(seat(&mut game.world, id, vid, 2));
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 2);
    assert!(!seated(&game, id), "a rear seat has no door that opens");
    let position = start_position(&game.world, KIND_JEEP, 20.0, 60.0).unwrap();
    let jeep = add_vehicle(&mut game.world, KIND_JEEP, position, 0.0);
    idle(&mut game, id, &mut seq, 10);
    assert!(seat(&mut game.world, id, jeep, 0));
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 2);
    assert!(!seated(&game, id), "nor has a jeep");
}

#[test]
fn rounds_into_a_door_take_it_off_and_an_open_door_is_torn_off_by_a_crash() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    let (_, door) = door_of(KIND_VOLVO_240, 0).unwrap();
    let middle = [door.hinge[0], door.hinge[1] + door.height * 0.3, door.hinge[2] + door.length * 0.5];
    let point = game.world.physics.car_pose(vid).unwrap().world_point(middle);
    let rifle = f32::from(shared::items::def(shared::items::ITEM_AK5).unwrap().damage);
    let mut hits = 0;
    while !game.world.entities.of::<Vehicle>(vid).doors.off(0) && hits < 100 {
        super::super::doors::shoot_doors(&mut game.world, vid, point, rifle);
        hits += 1;
    }
    assert!((5..=30).contains(&hits), "a magazine or so takes the door off: {hits} rounds");
    assert!(!game.world.entities.of::<Vehicle>(vid).doors.off(1), "the other one is still on");
    idle(&mut game, id, &mut seq, 1);
    assert_eq!(game.seen(vid).unwrap().doors.map(|doors| doors.gone), Some(0b01), "it is seen to be gone");
    // A blow on the right side that a shut door shrugs off tears it off standing open.
    let side = [-door.hinge[0] + 0.1, 0.0, door.hinge[2] + door.length * 0.6];
    super::super::doors::crash_doors(&mut game.world, vid, side, 2.0);
    assert!(!game.world.entities.of::<Vehicle>(vid).doors.off(1), "shut, it holds");
    game.world.entities.of_mut::<Vehicle>(vid).doors.open |= 0b10;
    super::super::doors::crash_doors(&mut game.world, vid, side, 2.0);
    assert!(game.world.entities.of::<Vehicle>(vid).doors.off(1), "open, it is gone");
}

#[test]
fn getting_out_by_a_door_that_has_come_off_is_quicker() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    game.world.entities.of_mut::<Vehicle>(vid).doors.gone = 0b01;
    assert!(seat(&mut game.world, id, vid, 0));
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, ticks_of(STEP_OUT_S) + 1);
    assert!(!seated(&game, id), "out without waiting for a door to open");
    assert_eq!(game.world.entities.of::<Vehicle>(vid).doors.open, 0, "and nothing is left open");
}

/// Door `n` of `vid` stands wide open, swinging free with nobody holding it.
fn fling_open(game: &mut Game, vid: u32, n: usize) {
    let doors = &mut game.world.entities.of_mut::<Vehicle>(vid).doors;
    doors.open |= 1 << n;
    doors.angle[n] = DOOR_SWING;
}

/// A Volvo 240 facing -z beside the wall of `game_with_a_house` (x = 55, from z = -5 to 5), its right
/// door standing open over the wall's line, which it passes at `speed` m/s, held there, for `seconds`:
/// the doors it ends with.
fn pass_the_wall_with_a_door_open(speed: f32, seconds: f32) -> crate::game::DoorState {
    let (mut game, _) = game_with_a_house();
    let (_, door) = door_of(KIND_VOLVO_240, 1).unwrap();
    // Its side half a metre off the wall, and its open door's edge a metre short of the wall's end.
    let x = 55.0 - door.hinge[0] - 0.5;
    let z = 5.0 + 1.0 - door.hinge[2] - door.length * DOOR_SWING.cos();
    let vid = add_vehicle(&mut game.world, KIND_VOLVO_240, [x, 1.5, z], 0.0);
    for _ in 0..60 {
        Vehicles.on_tick(&mut game.world, TICK_DT);
    }
    fling_open(&mut game, vid, 1);
    for _ in 0..ticks_of(seconds) {
        game.world.physics.set_car_velocity(vid, [0.0, 0.0, -speed]);
        Vehicles.on_tick(&mut game.world, TICK_DT);
        if game.world.entities.of::<Vehicle>(vid).doors.off(1) {
            break;
        }
    }
    let pose = game.world.physics.car_pose(vid).unwrap();
    assert!(pose.position[0] > x - 0.3, "the car itself goes on along the wall, untouched: {pose:?}");
    game.world.entities.of::<Vehicle>(vid).doors
}

#[test]
fn an_open_door_is_torn_off_driving_past_a_wall() {
    let doors = pass_the_wall_with_a_door_open(5.0, 2.0);
    assert!(doors.off(1), "the open door snagged on the wall and came off: {doors:?}");
    assert!(!doors.off(0), "the shut one on the other side is still on");
}

#[test]
fn an_open_door_that_brushes_a_wall_at_a_crawl_is_shoved_shut_and_dented_not_lost() {
    let doors = pass_the_wall_with_a_door_open(0.6, 4.0);
    assert!(!doors.off(1), "still on: {doors:?}");
    assert!(doors.damage[1] > 1.0, "but dented: {doors:?}");
    assert!(doors.angle[1] < DOOR_SWING - 0.2, "and pushed shut by the wall: {doors:?}");
}

#[test]
fn an_open_door_swings_shut_and_latches_as_the_car_drives_off() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    assert!(seat(&mut game.world, id, vid, 0));
    fling_open(&mut game, vid, 1);
    idle(&mut game, id, &mut seq, 3);
    assert!(game.world.entities.of::<Vehicle>(vid).doors.ajar(1), "it stands open while the car stands");
    press(&mut game, id, &mut seq, (0, 127), 0, ticks_of(2.0));
    let doors = game.world.entities.of::<Vehicle>(vid).doors;
    assert!(!doors.ajar(1) && doors.angle[1] == 0.0, "it swung shut and latched: {doors:?}");
    assert!(!doors.off(1), "without coming off");
    assert!(game.seen(vid).unwrap().doors.is_none(), "and is seen shut");
}

#[test]
fn a_parked_cars_open_door_stays_where_it_was_left() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 60.0);
    fling_open(&mut game, vid, 0);
    game.world.entities.of_mut::<Vehicle>(vid).doors.angle[0] = 0.7;
    idle(&mut game, id, &mut seq, ticks_of(3.0));
    let doors = game.world.entities.of::<Vehicle>(vid).doors;
    assert!(doors.ajar(0) && (doors.angle[0] - 0.7).abs() < 0.01, "{doors:?}");
    let seen = game.seen(vid).unwrap().doors.unwrap();
    assert_eq!(seen.left, (0.7 / DOOR_SWING * 255.0).round() as u8, "and is seen as far open");
}

#[test]
fn reversing_fast_with_a_door_open_bends_it_past_its_stop_and_off() {
    let mut game = game();
    let (id, vid, mut seq) = volvo(&mut game, 0.0, 0.0);
    assert!(seat(&mut game.world, id, vid, 0));
    fling_open(&mut game, vid, 1);
    game.world.entities.of_mut::<Vehicle>(vid).doors.angle[1] = 0.6;
    let mut fastest: f32 = 0.0;
    for _ in 0..ticks_of(4.0) {
        press(&mut game, id, &mut seq, (0, -127), 0, 1);
        fastest = fastest.max(game.world.physics.car_pose(vid).unwrap().forward_speed.abs());
        if game.world.entities.of::<Vehicle>(vid).doors.off(1) {
            break;
        }
    }
    assert!(game.world.entities.of::<Vehicle>(vid).doors.off(1), "the wind tore it off, reversing at up to {fastest} m/s: {:?}", game.world.entities.of::<Vehicle>(vid).doors);
}
