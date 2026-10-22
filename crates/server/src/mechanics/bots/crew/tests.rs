use super::*;
use crate::game::{Command, Game, Profile, TEAM_BOTS};
use crate::mechanics::combat::damage_vehicle;
use shared::world::{CollisionMesh, Road, WORLD_VERSION};
use tokio::sync::{mpsc, oneshot};

/// A flat 600 m square. `wall` adds a 10 m tall wall across x = 20..22 m, z = -30..30 m.
fn game(wall: bool, roads: Vec<Road>) -> Game {
    let mut map = WorldFile { version: WORLD_VERSION, half_extent: 30_000, spawns: vec![[-25_000, -25_000]], roads, ..Default::default() };
    if wall {
        let v = |x: i32, y: i32, z: i32| [x * 100, y * 100, z * 100];
        let vertices = vec![
            v(20, 0, -30), v(22, 0, -30), v(22, 0, 30), v(20, 0, 30),
            v(20, 10, -30), v(22, 10, -30), v(22, 10, 30), v(20, 10, 30),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        map.collision = CollisionMesh { vertices, triangles };
    }
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn crew(game: &mut Game, at: [f32; 2], route: Vec<[f32; 2]>) -> (u32, [u32; 2]) {
    game.add_crew(["Driver".into(), "Gunner".into()], at, TEAM_BOTS, route).expect("room for a crew")
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    response.try_recv().unwrap().unwrap()
}

fn put(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let y = game.world.map.ground.height_at(x, z) + sim::STANDING_Y;
    game.world.players.get_mut(&id).unwrap().character.position = [x, y, z];
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

fn tank(game: &Game, vid: u32) -> CarPose {
    game.world.physics.car_pose(vid).unwrap()
}

fn seated(game: &Game, id: u32) -> Option<(u32, usize)> {
    game.world.players[&id].seat.map(|seat| (seat.vehicle, seat.index))
}

fn shells_fired(game: &Game, vid: u32) -> u8 {
    game.world.vehicles[&vid].gun.unwrap().shots
}

#[test]
fn a_crew_is_two_bots_in_their_seats_of_a_fresh_tank() {
    let mut game = game(false, vec![]);
    let (vid, [driver, gunner]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0], [0.0, -40.0]]);
    assert_eq!(game.world.vehicles[&vid].kind, shared::protocol::KIND_TANK);
    assert_eq!(seated(&game, driver), Some((vid, DRIVER)));
    assert_eq!(seated(&game, gunner), Some((vid, GUNNER)));
    assert_eq!(game.world.bots[&driver].crew, Some(Crew { vehicle: vid, seat: DRIVER }));
}

#[test]
fn the_driver_drives_its_route_round_and_round() {
    let mut game = game(false, vec![]);
    let route = vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [0.0, 50.0]];
    let (vid, [driver, _]) = crew(&mut game, [0.0, 0.0], route);
    let mut visited = [false; 4];
    for _ in 0..(90 * TICK_RATE) {
        game.step();
        visited[game.world.bots[&driver].next] = true;
        let [x, _, z] = tank(&game, vid).position;
        assert!(x > -10.0 && x < 60.0 && z > -10.0 && z < 60.0, "keeps to its route: {x}, {z}");
    }
    assert_eq!(visited, [true; 4], "drove to every waypoint");
    assert!(game.world.bots[&driver].tries == 0 || game.world.bots[&driver].tries < TRIES, "never stuck on open ground");
}

/// A road from (-200, -20) east to (200, -20), then north to (200, 100).
fn road() -> Road {
    Road { width: 600, kind: 0, points: vec![[-20_000, -2_000], [-10_000, -2_000], [0, -2_000], [10_000, -2_000], [20_000, -2_000], [20_000, 10_000]] }
}

/// How far `[x, z]` is from the polyline `points` (metres).
fn off_road(points: &[[f32; 2]], [x, z]: [f32; 2]) -> f32 {
    points
        .windows(2)
        .map(|pair| {
            let ([ax, az], [bx, bz]) = (pair[0], pair[1]);
            let (dx, dz) = (bx - ax, bz - az);
            let t = (((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
            (x - ax - dx * t).hypot(z - az - dz * t)
        })
        .fold(f32::MAX, f32::min)
}

#[test]
fn a_route_made_along_a_road_goes_out_both_ways_and_back() {
    let map = WorldFile { roads: vec![road()], ..Default::default() };
    let route = road_route(&map, [0.0, 0.0]);
    let points: Vec<[f32; 2]> = road().points.iter().map(|p| [p[0] as f32 / 100.0, p[1] as f32 / 100.0]).collect();
    assert!(route.iter().all(|&p| off_road(&points, p) < 0.01), "every waypoint is on the road: {route:?}");
    assert!(route.windows(2).all(|w| (w[0][0] - w[1][0]).hypot(w[0][1] - w[1][1]) >= ROAD_SPACING - 0.01));
    let xs: Vec<f32> = route.iter().map(|p| p[0]).collect();
    assert!(xs.iter().any(|&x| x <= -100.0) && xs.iter().any(|&x| x >= 100.0), "goes out both ways: {xs:?}");
    // There and back: the second half retraces the first.
    let n = route.len();
    assert!(n > 4);
    assert_eq!(route[n - 1], route[1], "comes back to just after where it started the loop");
    assert_eq!(road_route(&WorldFile::default(), [3.0, 4.0]), vec![[3.0, 4.0]], "with no road it stays put");
}

#[test]
fn a_road_route_goes_on_straight_through_junctions_of_roads_that_meet_end_to_end() {
    // An avenue along z = 0 in 100 m pieces, with a side street off it at x = 0.
    let piece = |a: [i32; 2], b: [i32; 2]| Road { width: 600, kind: 0, points: vec![a, b] };
    let roads = vec![
        piece([-30_000, 0], [-20_000, 0]),
        piece([-20_000, 0], [-10_000, 0]),
        piece([-10_000, 0], [0, 0]),
        piece([0, 0], [0, 10_000]),
        piece([10_000, 0], [0, 0]),
        piece([10_000, 0], [20_000, 0]),
        piece([20_000, 0], [30_000, 0]),
    ];
    let map = WorldFile { roads, ..Default::default() };
    let route = road_route(&map, [5.0, 1.0]);
    assert!(route.iter().all(|p| p[1].abs() < 0.01), "never down the side street: {route:?}");
    let xs: Vec<f32> = route.iter().map(|p| p[0]).collect();
    assert!(xs.contains(&-300.0) && xs.contains(&300.0), "the length of the avenue both ways: {xs:?}");
}

#[test]
fn with_no_route_it_drives_along_the_nearest_road() {
    let mut game = game(false, vec![road()]);
    let (vid, [driver, _]) = crew(&mut game, [0.0, -20.0], vec![]);
    let points: Vec<[f32; 2]> = road().points.iter().map(|p| [p[0] as f32 / 100.0, p[1] as f32 / 100.0]).collect();
    assert!(game.world.bots[&driver].route.len() > 4);
    let start = tank(&game, vid).position;
    let mut furthest: f32 = 0.0;
    for _ in 0..60 {
        run(&mut game, 1.0);
        let [x, _, z] = tank(&game, vid).position;
        assert!(off_road(&points, [x, z]) < 6.0, "keeps to the road: {x}, {z}");
        furthest = furthest.max((x - start[0]).hypot(z - start[2]));
    }
    assert!(furthest > 100.0, "drove along it, {furthest} m");
}

#[test]
fn the_shell_laid_for_a_mark_lands_on_it() {
    for (from, to) in [([0.0, 2.0, 0.0], [80.0, 0.0, 0.0]), ([0.0, 2.0, 0.0], [0.0, 10.0, -110.0]), ([0.0, 20.0, 0.0], [30.0, 0.0, 30.0])] {
        let pitch = lay(from, to);
        let d = distance_2d(from, to);
        let (vh, vy) = (SHELL_SPEED * pitch.cos(), SHELL_SPEED * pitch.sin());
        let t = d / vh;
        let height = from[1] + vy * t - 0.5 * GRAVITY * t * t;
        assert!((height - to[1]).abs() < 0.05, "{from:?} to {to:?}: at {height}");
    }
}

#[test]
fn the_gunner_shells_someone_it_sees_all_round_and_the_tank_holds_still() {
    let mut game = game(false, vec![]);
    let (vid, [_, gunner]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0], [0.0, -60.0]]);
    game.world.bots.get_mut(&gunner).unwrap().skill = 1.0;
    let person = join(&mut game);
    // Behind the tank, which first heads the other way.
    put(&mut game, person, [10.0, 70.0]);
    let mut halted = false;
    for _ in 0..(20 * TICK_RATE) {
        game.step();
        let fighting = game.world.bots[&gunner].sees;
        if fighting && tank(&game, vid).forward_speed.abs() < 0.3 {
            halted = true;
        }
        if !game.world.players[&person].alive() {
            break;
        }
        put(&mut game, person, [10.0, 70.0]);
    }
    assert!(shells_fired(&game, vid) > 0, "it fired");
    assert!(!game.world.players[&person].alive(), "and got them in the end");
    assert!(halted, "the driver held still while the gunner fought");
}

#[test]
fn it_leaves_its_own_team_alone() {
    let mut game = game(false, vec![]);
    let (vid, _) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0]]);
    let friend = game.add_bot([0.0, -40.0], TEAM_BOTS, vec![[0.0, -40.0]]).unwrap();
    run(&mut game, 8.0);
    assert_eq!(shells_fired(&game, vid), 0);
    assert!(game.world.players[&friend].alive());
}

#[test]
fn it_does_not_fire_on_someone_close_enough_for_the_burst_to_reach_it() {
    let mut game = game(false, vec![]);
    let (vid, [_, gunner]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0]]);
    game.world.bots.get_mut(&gunner).unwrap().skill = 1.0;
    let person = join(&mut game);
    for _ in 0..(6 * TICK_RATE) {
        put(&mut game, person, [6.0, -6.0]);
        game.step();
    }
    assert!(game.world.bots[&gunner].sees, "it sees them");
    assert_eq!(shells_fired(&game, vid), 0, "but holds its fire");
}

#[test]
fn a_stuck_tank_backs_out_and_gives_the_waypoint_up() {
    let mut game = game(true, vec![]);
    // The second waypoint is behind the wall, which a tank cannot push through.
    let (vid, [driver, _]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0], [40.0, 0.0], [0.0, -50.0]]);
    let mut reversed = false;
    let mut gave_up = false;
    for _ in 0..(80 * TICK_RATE) {
        game.step();
        reversed |= tank(&game, vid).forward_speed < -1.0;
        let [x, _, z] = tank(&game, vid).position;
        assert!(x < 21.0, "never through the wall: {x}");
        if game.world.bots[&driver].next == 2 && reversed {
            gave_up = true;
        }
        if gave_up && x.hypot(z + 50.0) < WAYPOINT_REACHED + 1.0 {
            return;
        }
    }
    panic!("reversed {reversed}, gave up {gave_up}, at {:?}", tank(&game, vid).position);
}

#[test]
fn a_crew_gets_back_in_when_their_wrecked_tank_comes_back() {
    let mut game = game(false, vec![]);
    let (vid, [driver, gunner]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0]]);
    run(&mut game, 1.0);
    damage_vehicle(&mut game.world, vid, 255, None);
    game.step();
    assert!(!game.world.players[&driver].alive() && !game.world.players[&gunner].alive(), "the crew die in the wreck");
    assert_eq!(seated(&game, driver), None);
    // They come back in 3 s, beside it; the tank is put back after `WRECK_TICKS`.
    run(&mut game, WRECK_TICKS as f32 / TICK_RATE as f32 + 6.0);
    assert_eq!(seated(&game, driver), Some((vid, DRIVER)));
    assert_eq!(seated(&game, gunner), Some((vid, GUNNER)));
}

#[test]
fn a_crewman_put_out_of_reach_of_its_tank_fights_on_foot() {
    let mut game = game(false, vec![]);
    let (vid, [driver, _]) = crew(&mut game, [0.0, 0.0], vec![[0.0, 0.0]]);
    crate::mechanics::remove_vehicle(&mut game.world, vid);
    put(&mut game, driver, [0.0, 10.0]);
    let person = join(&mut game);
    put(&mut game, person, [0.0, -10.0]);
    run(&mut game, 5.0);
    assert_eq!(seated(&game, driver), None);
    let hurt = &game.world.players[&person];
    assert!(hurt.hp < shared::consts::PLAYER_MAX_HP || hurt.deaths > 0, "it shot them on foot");
}

/// Not a regression test: tank crews let loose on the committed fixture city with no one about, to
/// see how they get on along its real roads. `RAGNAROK_WORLD` picks another world.
#[test]
#[ignore]
fn tanks_drive_the_fixture_city() {
    let fixture = std::env::var_os("RAGNAROK_WORLD")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack"));
    let (map, _) = crate::read_world(&fixture).expect("the fixture world");
    let mut game = Game::new(map, 0).unwrap();
    let spawns: Vec<[f32; 2]> = game.world.map.spawns.iter().map(|s| [s[0] as f32 / 100.0, s[1] as f32 / 100.0]).collect();
    let mut crews = Vec::new();
    for &at in spawns.iter().rev().take(40) {
        if crews.len() == 6 {
            break;
        }
        if let Some((vid, [driver, _])) = game.add_crew(["Driver".into(), "Gunner".into()], at, TEAM_BOTS, vec![]) {
            crews.push((vid, driver, tank(&game, vid).position, 0.0f32, 0u32));
        }
    }
    for _ in 0..120 {
        let before: Vec<[f32; 3]> = crews.iter().map(|c| tank(&game, c.0).position).collect();
        run(&mut game, 1.0);
        for (crew, was) in crews.iter_mut().zip(before) {
            let now = tank(&game, crew.0).position;
            crew.3 += (now[0] - was[0]).hypot(now[2] - was[2]);
            crew.4 = crew.4.max(game.world.bots[&crew.1].tries);
        }
    }
    for (vid, driver, start, driven, tries) in crews {
        let bot = &game.world.bots[&driver];
        let pose = tank(&game, vid);
        println!(
            "tank {vid}: {} waypoints, at {} now, drove {driven:.0} m, {:.0} m from start, most tries {tries}, upright {:.2}",
            bot.route.len(),
            bot.next,
            (pose.position[0] - start[0]).hypot(pose.position[2] - start[2]),
            pose.up_y
        );
    }
}
