use super::*;
use crate::game::{Command, Game, Noise, Profile};
use shared::world::{ROAD_MINOR, Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A two-lane road along `points` (metres).
fn road(points: &[[f32; 2]]) -> Road {
    Road { width: 700, kind: ROAD_MINOR, points: points.iter().map(|p| [(p[0] * 100.0) as i32, (p[1] * 100.0) as i32]).collect(), lanes: 2, forward: 1, ..Default::default() }
}

/// A flat world 1200 m square with a block of streets 400 m by 300 m in its middle, a road across
/// the block, and a road out of the world from each of two corners, with `cars` of traffic.
fn game(cars: usize) -> Game {
    let roads = vec![
        road(&[[-200.0, -150.0], [0.0, -150.0], [200.0, -150.0]]),
        road(&[[200.0, -150.0], [200.0, 150.0]]),
        road(&[[200.0, 150.0], [0.0, 150.0], [-200.0, 150.0]]),
        road(&[[-200.0, 150.0], [-200.0, -150.0]]),
        road(&[[0.0, -150.0], [0.0, 150.0]]),
        road(&[[-600.0, -150.0], [-200.0, -150.0]]),
        road(&[[200.0, 150.0], [600.0, 150.0]]),
    ];
    let map = WorldFile { version: WORLD_VERSION, half_extent: 60_000, spawns: vec![[-55_000, 55_000]], roads, ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    busy(&mut game, cars);
    game
}

/// Puts `cars` of traffic on the roads of `game`: as many for each kilometre of its driven lane as
/// makes that number.
fn busy(game: &mut Game, cars: usize) {
    game.world.traffic = 0.0;
    if cars == 0 {
        return;
    }
    let driven = game.with_mechanic(|traffic: &mut Traffic, world| {
        traffic.lay_out(world);
        traffic.graph.driven()
    });
    game.world.traffic = cars as f32 * 1000.0 / driven;
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    let id = response.try_recv().unwrap().unwrap();
    // With nothing to hand: someone who stands by a road with a gun frightens whoever drives by
    // slowly. `threaten` gives the gun back.
    let gun = std::mem::take(&mut game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar[0]);
    GUN.with(|kept| kept.set(gun));
    id
}

/// Stands player `id` at `(x, z)`.
fn stand(game: &mut Game, id: u32, x: f32, z: f32) {
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [x, sim::STANDING_Y, z];
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

/// The traffic of `game`.
fn traffic(game: &Game) -> &Traffic {
    game.mechanic::<Traffic>().expect("the room runs traffic")
}

/// The vehicles of the traffic that are shown, with whether each is on rails.
fn shown(game: &Game) -> Vec<(u32, bool)> {
    game.world.entities.all::<TrafficCar>().map(|(vid, _)| (vid, game.world.physics.car_on_rails(vid))).collect()
}

#[test]
fn a_world_with_no_traffic_has_none_and_one_with_no_players_shows_none() {
    let mut empty = game(0);
    run(&mut empty, 1.0);
    assert_eq!(traffic(&empty).counts(), (0, 0, 0));
    let mut busy = game(40);
    run(&mut busy, 5.0);
    assert_eq!(traffic(&busy).counts(), (40, 0, 0), "cars, shown, physical");
    assert_eq!(busy.world.entities.count::<TrafficCar>(), 0);
}

#[test]
fn cars_keep_to_the_roads_apart_from_each_other_and_their_number_holds() {
    let mut game = game(40);
    let mut moved = 0.0f32;
    for second in 0..600 {
        run(&mut game, 1.0);
        let traffic = traffic(&game);
        assert_eq!(traffic.cars.len(), 40);
        moved += traffic.cars.iter().map(|car| car.speed).sum::<f32>();
        for lane in &traffic.lanes {
            for pair in lane.iter().collect::<Vec<_>>().windows(2) {
                let (ahead, behind) = (&traffic.cars[*pair[0] as usize], &traffic.cars[*pair[1] as usize]);
                // A car that is out passing is beside the one it passes, not behind it.
                if ahead.shift > 0.5 || behind.shift > 0.5 {
                    continue;
                }
                // One that has just pulled in may have to brake for what is ahead before the car
                // it passed has dropped back from it.
                if ahead.rest > 0 {
                    continue;
                }
                let gap = ahead.along - behind.along - half_length(ahead.kind) - half_length(behind.kind);
                assert!(gap > 0.5, "two cars {gap} m apart after {second} s");
            }
        }
        for car in &traffic.cars {
            assert!(traffic.graph.live[car.way.index()] && car.along >= 0.0 && car.along <= traffic.graph.length(car.way));
        }
    }
    // Ten minutes of forty cars: they have driven, on the whole, and not stood in a jam.
    let mean = moved / (600.0 * 40.0);
    assert!(mean > 4.0, "the cars averaged {mean} m/s");
}

#[test]
fn a_car_is_shown_near_a_player_let_go_beside_them_and_hidden_when_they_leave() {
    let mut game = game(40);
    let id = join(&mut game);
    // Beside the middle of the block's north street.
    stand(&mut game, id, -100.0, -160.0);
    run(&mut game, 20.0);
    let (cars, shown_count, physical) = traffic(&game).counts();
    assert_eq!(cars, 40);
    assert!(shown_count >= 6, "{shown_count} of the cars are shown");
    assert_eq!(shown(&game).len(), shown_count);
    assert_eq!(shown(&game).iter().filter(|&&(_, rails)| !rails).count(), physical);
    // Every shown car is where its row is, near enough, the physical ones too: they drive their lanes.
    let traffic_now = traffic(&game);
    let mut worst = 0.0f32;
    for (i, car) in traffic_now.cars.iter().enumerate() {
        let Some(vid) = car.vid else { continue };
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (p, _) = traffic_now.pose(i);
        worst = worst.max((pose.position[0] - p[0]).hypot(pose.position[2] - p[2]));
        assert!(pose.up_y > 0.9, "a car is on its wheels");
    }
    assert!(worst < 2.5, "a shown car is {worst} m from its lane");
    // Far from every road, nothing is shown.
    stand(&mut game, id, -550.0, 550.0);
    run(&mut game, 1.0);
    assert_eq!(traffic(&game).counts(), (40, 0, 0));
    assert_eq!(game.world.entities.count::<TrafficCar>(), 0);
}

#[test]
fn a_physical_car_drives_its_lane_and_goes_back_on_rails_behind_the_player() {
    let mut game = game(12);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 0.0);
    // Until a car has been physical and is on rails again, having passed the player.
    let mut was_physical: Vec<u32> = Vec::new();
    let mut back = false;
    for _ in 0..(900 * TICK_RATE) {
        game.step();
        for (vid, rails) in shown(&game) {
            if !rails && !was_physical.contains(&vid) {
                was_physical.push(vid);
            }
            back |= rails && was_physical.contains(&vid);
        }
        if back {
            break;
        }
    }
    assert!(!was_physical.is_empty(), "a car came near enough to be let go");
    assert!(back, "a car that was let go went back on rails");
    assert_eq!(game.world.entities.count::<Abandoned>(), 0, "none left the road");
}

#[test]
fn a_car_stops_for_someone_standing_in_the_road() {
    let mut game = game(30);
    let id = join(&mut game);
    // In the right-hand lane of the road across the block, both ways blocked by one person each
    // would take two: stand in the middle of the road, which both lanes' cars pass within reach of.
    stand(&mut game, id, 1.5, 0.0);
    let mut nearest = f32::MAX;
    let mut waited = false;
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        stand(&mut game, id, 1.5, 0.0);
        waited |= traffic(&game).cars.iter().any(|car| car.held > TICK_RATE);
        for (vid, _) in shown(&game) {
            let pose = game.world.physics.car_pose(vid).unwrap();
            let off = (pose.position[0] - 1.5).hypot(pose.position[2]);
            if pose.forward_speed.abs() > 1.0 {
                nearest = nearest.min(off);
            }
        }
    }
    assert!(waited, "a car came down the road and stood waiting");
    assert!(nearest > 2.0, "a moving car came within {nearest} m of the person in the road");
}

#[test]
fn a_car_a_player_takes_is_theirs_and_another_takes_its_place() {
    let mut game = game(20);
    let id = join(&mut game);
    stand(&mut game, id, -100.0, -160.0);
    // The cars start out of the player's sight: until the first drives into it.
    for _ in 0..(60 * TICK_RATE) {
        game.step();
        if !shown(&game).is_empty() {
            break;
        }
    }
    let (vid, _) = shown(&game)[0];
    crate::mechanics::seat(&mut game.world, id, vid, 0);
    run(&mut game, 1.0);
    assert!(!game.world.entities.has::<TrafficCar>(vid) && game.world.entities.has::<Abandoned>(vid));
    assert!(!game.world.physics.car_on_rails(vid), "it is a car like any other");
    assert_eq!(traffic(&game).counts().0, 20, "the roads are as busy as they were");
    assert!(traffic(&game).cars.iter().all(|car| car.vid != Some(vid)));
}

/// A flat world with one straight two-lane road 2.8 km long from edge to edge along z = 0, driven
/// east in the lane at z = 1.75, with `cars` of traffic.
fn highway(cars: usize) -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 140_000, spawns: vec![[-130_000, 130_000]], roads: vec![road(&[[-1400.0, 0.0], [0.0, 0.0], [1400.0, 0.0]])], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    busy(&mut game, cars);
    game
}

#[test]
fn a_slow_car_is_passed_on_the_open_road_and_not_into_the_face_of_another() {
    let mut game = highway(16);
    game.step();
    // One dawdler, and the rest in a hurry.
    game.with_mechanic(|traffic: &mut Traffic, _| {
        for (i, car) in traffic.cars.iter_mut().enumerate() {
            car.pace = if i % 4 == 0 { 0.45 } else { 1.15 };
        }
    });
    let mut nearest = f32::MAX;
    for _ in 0..(600 * TICK_RATE) {
        game.step();
        let traffic = traffic(&game);
        // A car on the wrong side of the road and one coming the other way never meet.
        for car in traffic.cars.iter().filter(|car| car.shift > 1.0) {
            for other in traffic.lanes[car.way.reversed().index()].iter().map(|&other| &traffic.cars[other as usize]) {
                // How far ahead of it the other is: one that has gone by is behind it.
                let ahead = traffic.graph.length(car.way) - other.along - car.along;
                if ahead > -2.0 {
                    nearest = nearest.min(ahead.abs());
                }
            }
        }
    }
    let traffic = traffic(&game);
    assert!(traffic.overtakes >= 5, "{} cars were passed", traffic.overtakes);
    assert!(nearest > 6.0, "a car that was out passing came within {nearest} m of one coming the other way");
    assert!(traffic.cars.iter().all(|car| car.shift < 4.0));
}

#[test]
fn cars_go_round_a_vehicle_standing_in_their_lane_without_touching_it() {
    let mut game = highway(14);
    let id = join(&mut game);
    // A jeep left in the eastbound lane, and someone on the verge to see it.
    stand(&mut game, id, 0.0, 30.0);
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    let parked = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [0.0, ride, 1.75], std::f32::consts::FRAC_PI_2);
    run(&mut game, 2.0);
    let at = game.world.physics.car_pose(parked).unwrap().position;
    // The traffic's cars seen west of it in the eastbound lane, and those of them seen east of it since.
    let (mut coming, mut past) = (Vec::new(), Vec::new());
    for _ in 0..(400 * TICK_RATE) {
        game.step();
        for (vid, _) in shown(&game) {
            let pose = game.world.physics.car_pose(vid).unwrap();
            let [x, _, z] = pose.position;
            if (-60.0..-20.0).contains(&x) && z > 0.5 && !coming.contains(&vid) {
                coming.push(vid);
            }
            if x > 25.0 && coming.contains(&vid) && !past.contains(&vid) {
                past.push(vid);
            }
        }
        // Enough have got by: the rest of the run would only repeat it.
        if past.len() >= 3 {
            break;
        }
    }
    let now = game.world.physics.car_pose(parked).unwrap().position;
    let moved = (now[0] - at[0]).hypot(now[2] - at[2]);
    assert!(past.len() >= 3, "{} of the {} cars that came up to it got past", past.len(), coming.len());
    assert!(moved < 0.3, "the standing jeep was pushed {moved} m");
    assert_eq!(game.world.entities.count::<Abandoned>(), 0, "nobody left the road over it");
}

#[test]
fn cars_go_round_a_vehicle_left_in_a_town_street_between_two_junctions() {
    // The block's north street, 200 m from junction to junction, with a jeep left in its eastbound lane.
    let mut game = game(30);
    let id = join(&mut game);
    stand(&mut game, id, -100.0, -175.0);
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    let parked = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [-100.0, ride, -148.25], -std::f32::consts::FRAC_PI_2);
    run(&mut game, 2.0);
    let at = game.world.physics.car_pose(parked).unwrap().position;
    let (mut coming, mut past) = (Vec::new(), Vec::new());
    for _ in 0..(400 * TICK_RATE) {
        game.step();
        for (vid, _) in shown(&game) {
            let [x, _, z] = game.world.physics.car_pose(vid).unwrap().position;
            if (-160.0..-120.0).contains(&x) && (-149.5..-147.0).contains(&z) && !coming.contains(&vid) {
                coming.push(vid);
            }
            if x > -75.0 && z < -140.0 && coming.contains(&vid) && !past.contains(&vid) {
                past.push(vid);
            }
        }
        // Enough have got by: the rest of the run would only repeat it.
        if past.len() >= 3 {
            break;
        }
    }
    let now = game.world.physics.car_pose(parked).unwrap().position;
    let moved = (now[0] - at[0]).hypot(now[2] - at[2]);
    assert!(past.len() >= 3, "{} of the {} cars that came up to it got past", past.len(), coming.len());
    assert!(moved < 0.3, "the standing jeep was pushed {moved} m");
}

#[test]
fn a_road_whose_bridge_is_down_is_shut_and_the_traffic_keeps_off_it() {
    // The block of streets, with the road across it on a bridge.
    let mut game = game(0);
    game.world.map.roads[4].carried = true;
    busy(&mut game, 40);
    let id = join(&mut game);
    stand(&mut game, id, 30.0, 0.0);
    let on_bridge = |traffic: &Traffic| traffic.cars.iter().filter(|car| traffic.graph.edge(car.way).carried).count();
    // The cars start out of the player's sight: until one of them is on the bridge, and shown.
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        let traffic = traffic(&game);
        if traffic.cars.iter().any(|car| traffic.graph.edge(car.way).carried && car.vid.is_some()) {
            break;
        }
    }
    assert!(on_bridge(traffic(&game)) > 0, "cars drive the bridge while it stands");
    let driven = traffic(&game).graph.driven();
    // Its deck drops out from under the road: the roadway is 3 m below where it was.
    game.with_mechanic(|traffic: &mut Traffic, _| {
        for (_, was) in &mut traffic.decks {
            was.iter_mut().for_each(|y| *y += 3.0);
        }
    });
    run(&mut game, 5.0);
    let traffic_now = traffic(&game);
    assert!(traffic_now.graph.edges.iter().filter(|edge| edge.carried).all(|edge| edge.shut), "the bridge's road is shut");
    assert!((driven - traffic_now.graph.driven() - 600.0).abs() < 2.0, "300 m less is driven each way");
    assert_eq!(on_bridge(traffic_now), 0, "no car of the traffic is on it");
    assert!(game.world.entities.count::<Abandoned>() > 0, "the cars that were on it are left to the physics");
    // And none drives onto it afterwards.
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        assert_eq!(on_bridge(traffic(&game)), 0);
    }
    // There are as many cars fewer as there is less road to drive.
    let traffic_now = traffic(&game);
    let left = (40.0 * traffic_now.graph.driven() / driven).round() as usize;
    assert!(left < 40);
    assert_eq!(traffic_now.counts().0, left);
}

/// Not a regression test: a real world's traffic with nobody connected, to see that it keeps moving
/// and what it costs. `WORLD` is the world, `PER_KM` how many cars a kilometre of driven lane (2) and `MINUTES` how long (20).
/// `WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture traffic_on_a_real_world`
#[test]
#[ignore]
fn traffic_on_a_real_world() {
    let path = std::env::var_os("WORLD").map(std::path::PathBuf::from).expect("WORLD");
    let (map, _) = crate::read_world(&path).unwrap();
    let mut game = Game::new(map, 0).unwrap();
    game.world.traffic = std::env::var("PER_KM").ok().and_then(|n| n.parse().ok()).unwrap_or(2.0);
    let minutes: u32 = std::env::var("MINUTES").ok().and_then(|n| n.parse().ok()).unwrap_or(20);
    for minute in 1..=minutes {
        let started = std::time::Instant::now();
        run(&mut game, 60.0);
        println!("after {minute} min ({:?} a tick for the whole room):\n{}", started.elapsed() / (60 * TICK_RATE), traffic(&game).report());
    }
}

/// Not a regression test: what it costs to find the way by road on a real world. Plans `ROUTES`
/// routes (200) between places picked at random on `WORLD`'s network, and prints how long one takes.
/// `WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture routes_on_a_real_world`
#[test]
#[ignore]
fn routes_on_a_real_world() {
    let path = std::env::var_os("WORLD").map(std::path::PathBuf::from).expect("WORLD");
    let (map, _) = crate::read_world(&path).unwrap();
    let routes: usize = std::env::var("ROUTES").ok().and_then(|n| n.parse().ok()).unwrap_or(200);
    let mut game = Game::new(map, 0).unwrap();
    game.world.traffic = 1.0;
    game.step();
    let graph = &traffic(&game).graph;
    let mut rng = crate::rng::Rng::from_seed(11);
    let mut place = || graph.place(rng.next_f64() as f32 * graph.driven()).expect("a road");
    let (mut found, mut longest, mut seconds, mut ways) = (0, std::time::Duration::ZERO, 0.0, 0);
    let started = std::time::Instant::now();
    for _ in 0..routes {
        let ((from, along), (to, stop)) = (place(), place());
        let (p, _) = graph.pose(None, to, None, stop);
        let one = std::time::Instant::now();
        let spot = graph.locate(p, 150.0).expect("the road it is on");
        if let Some(route) = graph.route(from, along, spot, 240.0) {
            (found, seconds, ways) = (found + 1, seconds + route.cost, ways + route.ways.len());
        }
        longest = longest.max(one.elapsed());
    }
    println!("{routes} routes over {} ways: {:?} each, {longest:?} the slowest; {found} are under 240 s of driving, {:.0} s and {} ways on average", graph.live.len(), started.elapsed() / routes as u32, seconds / found.max(1) as f32, ways / found.max(1));
}

/// Not a regression test: prints the network within `REACH` metres (150) of `AT` ("x,z") on `WORLD`.
#[test]
#[ignore]
fn network_around_a_place() {
    let path = std::env::var_os("WORLD").map(std::path::PathBuf::from).expect("WORLD");
    let (map, _) = crate::read_world(&path).unwrap();
    let at: Vec<f32> = std::env::var("AT").expect("AT").split(',').map(|v| v.parse().unwrap()).collect();
    let reach: f32 = std::env::var("REACH").ok().and_then(|n| n.parse().ok()).unwrap_or(150.0);
    let mut game = Game::new(map, 0).unwrap();
    game.world.traffic = 1.0;
    game.step();
    let graph = &traffic(&game).graph;
    let near = |p: [f32; 2]| (p[0] - at[0]).hypot(p[1] - at[1]) < reach;
    // How much of the world's driven road is of each sort, km: with lanes both ways, one-way with
    // two lanes or more, a single lane (one-way or not, which the world does not say), and with no
    // lanes written (a bridge's own road).
    let mut km = [0.0f32; 4];
    for road in game.world.map.roads.iter().filter(|road| road.kind <= 1 && road.width >= 350) {
        let length: f32 = road.points.windows(2).map(|pair| ((pair[1][0] - pair[0][0]) as f32).hypot((pair[1][1] - pair[0][1]) as f32) / 100.0).sum();
        let sort = if road.lanes == 0 { 3 } else if road.lanes == 1 { 2 } else if road.forward == road.lanes || road.forward == 0 { 1 } else { 0 };
        km[sort] += length / 1000.0;
    }
    println!("roads: {:.0} km two-way, {:.0} km one-way of several lanes, {:.0} km of a single lane, {:.0} km with no lanes written", km[0], km[1], km[2], km[3]);
    let dead: f32 = graph.edges.iter().enumerate().filter(|&(index, _)| !graph.live[index * 2] && !graph.live[index * 2 + 1]).map(|(_, edge)| edge.length()).sum();
    let dead_single: f32 = graph.edges.iter().enumerate().filter(|&(index, edge)| !graph.live[index * 2] && !graph.live[index * 2 + 1] && edge.lanes == [1, 0] && edge.speed < 8.0).map(|(_, edge)| edge.length()).sum();
    println!("not driven either way: {:.0} km, {:.0} km of it a single lane", dead / 1000.0, dead_single / 1000.0);
    for (index, edge) in graph.edges.iter().enumerate() {
        let [a, b] = edge.ends.map(|n| graph.nodes[n as usize].at);
        if !near(a) && !near(b) {
            continue;
        }
        let live = [graph.live[index * 2], graph.live[index * 2 + 1]];
        let (first, last) = (edge.points[0], edge.points[edge.points.len() - 1]);
        println!(
            "edge {index}: {}({:.0},{:.0} e{}) -> {}({:.0},{:.0} e{}) len {:.0} kind {} width {:.1} lanes {:?} live {:?} speed {:.0} y {:.1}..{:.1}",
            edge.ends[0], a[0], a[1], graph.nodes[edge.ends[0] as usize].edges, edge.ends[1], b[0], b[1], graph.nodes[edge.ends[1] as usize].edges, edge.length(), edge.kind, edge.width, edge.lanes, live, edge.speed, first[1], last[1]
        );
    }
    for road in game.world.map.roads.iter().filter(|road| road.kind <= 1 && road.points.iter().any(|p| near([p[0] as f32 / 100.0, p[1] as f32 / 100.0]))) {
        let (a, b) = (road.points[0], road.points[road.points.len() - 1]);
        println!("road kind {} width {} lanes {} forward {} carried {} points {} from ({},{}) to ({},{})", road.kind, road.width, road.lanes, road.forward, road.carried, road.points.len(), a[0] / 100, a[1] / 100, b[0] / 100, b[1] / 100);
    }
}

/// Not a regression test: cuts the bridge under the longest bridge road of `WORLD` with shells, as
/// `bridge_wreckage` does, and says whether the traffic shut its road.
/// `WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture a_real_bridge_cut`
#[test]
#[ignore]
fn a_real_bridge_cut_across_is_shut() {
    let path = std::env::var_os("WORLD").map(std::path::PathBuf::from).expect("WORLD");
    let (map, _) = crate::read_world(&path).unwrap();
    let mut game = Game::new(map, 0).unwrap();
    game.world.traffic = 2.0;
    run(&mut game, 20.0);
    let shut = |game: &Game| traffic(game).graph.edges.iter().filter(|edge| edge.shut).count();
    println!("before: {} shut\n{}", shut(&game), traffic(&game).report());
    // The middle of the longest road a bridge carries that is driven both ways, and the section of bridge nearest it.
    let (middle, length) = {
        let graph = &traffic(&game).graph;
        let edge = graph.edges.iter().enumerate().filter(|&(index, edge)| edge.carried && graph.live[index * 2] && graph.live[index * 2 + 1]).map(|(_, edge)| edge).max_by(|a, b| a.length().total_cmp(&b.length())).expect("a bridge");
        (edge.at(edge.length() / 2.0).0, edge.length())
    };
    println!("the longest bridge road is {length:.0} m, its middle at {middle:?}");
    let world = &mut game.world;
    let centre = |i: usize| {
        let f = &world.map.buildings[i].footprint;
        let n = f.len() as f32;
        [f.iter().map(|p| p[0] as f32).sum::<f32>() / n / 100.0, f.iter().map(|p| p[1] as f32).sum::<f32>() / n / 100.0]
    };
    let mut sections: Vec<usize> = (0..world.map.buildings.len()).filter(|&i| world.map.buildings[i].kind == shared::world::BUILDING_BRIDGE).collect();
    sections.sort_by(|&a, &b| (centre(a)[0] - middle[0]).hypot(centre(a)[1] - middle[2]).total_cmp(&(centre(b)[0] - middle[0]).hypot(centre(b)[1] - middle[2])));
    sections.truncate(1);
    let bite = EXPLOSION_RADIUS * crate::mechanics::CARVE_REACH;
    // One section, cut along its length down the middle of its deck.
    let index = sections[0];
    let id = world.map.building_ids[index];
    let b = world.map.buildings[index].clone();
    let pts: Vec<[f32; 2]> = b.footprint.iter().map(|p| [p[0] as f32 / 100.0, p[1] as f32 / 100.0]).collect();
    let (mut a, mut c, mut best) = (pts[0], pts[0], 0.0);
    for &p in &pts {
        for &q in &pts {
            let d = (p[0] - q[0]).hypot(p[1] - q[1]);
            if d > best {
                (a, c, best) = (p, q, d);
            }
        }
    }
    let top = (b.base + b.height) as f32 / 100.0;
    let steps = (best / 2.0) as i32;
    for k in 1..steps {
        let t = k as f32 / steps as f32;
        world.carve(id, [a[0] + (c[0] - a[0]) * t, top - 0.5, a[1] + (c[1] - a[1]) * t], bite);
        world.flush_buildings();
    }
    println!("the section nearest it cut from {a:?} to {c:?} at {top:.1} m");
    run(&mut game, 20.0);
    println!("after: {} shut\n{}", shut(&game), traffic(&game).report());
    let on_shut = traffic(&game).cars.iter().filter(|car| traffic(&game).graph.edge(car.way).shut).count();
    println!("{on_shut} cars on a shut road");
}

thread_local! {
    /// What a player joined with in their first slot: a gun, which `join` takes off them.
    static GUN: std::cell::Cell<shared::protocol::ItemSlot> = const { std::cell::Cell::new(shared::protocol::ItemSlot { item: 0, count: 0, ammo: 0 }) };
    /// The number of the last input a test's player sent: they go up by one, as a client's do.
    static SEQ: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Player `id` looks at the point `at`, with `buttons` held, as their next input.
fn look_at(game: &mut Game, id: u32, at: [f32; 3], buttons: u16) {
    let seq = SEQ.with(|seq| {
        seq.set(seq.get() + 1);
        seq.get()
    });
    let eye = game.world.entities.get::<Player>(id).unwrap().eye();
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.01);
    let frame = shared::protocol::InputFrame { seq, strafe: 0, forward: 0, yaw: shared::quant::to_mrad((-d[0]).atan2(-d[2])), buttons, pitch: shared::quant::to_mrad((d[1] / length).asin()), view: seq as u16, slot: 0, collective: 0 };
    game.handle(Command::Input(id, seq.saturating_sub(2), vec![frame]));
}

/// The drivers on foot, each with where they stand.
fn on_foot(game: &Game) -> Vec<(crate::mechanics::Civilian, [f32; 3])> {
    game.world.entities.all::<crate::mechanics::Civilian>().map(|(id, civilian)| (civilian.clone(), game.world.entities.of::<Player>(id).character.position)).collect()
}

/// Runs the highway with player `id` beside it at `(0, 9)`, a gun aimed down its sights at the nearest car
/// that is not an EPA and whose driver has `nerve`, until `done` says so or `seconds` are up.
/// Returns whether it said so.
fn threaten(game: &mut Game, id: u32, nerve: Nerve, seconds: u32, mut done: impl FnMut(&Game) -> bool) -> bool {
    game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar[0] = GUN.with(std::cell::Cell::get);
    let held = game.world.entities.get::<Player>(id).unwrap().held().item;
    assert!(shared::items::def(held).is_some_and(|item| item.kind == shared::items::ItemKind::Gun), "a player's first slot is a gun");
    for _ in 0..(seconds * TICK_RATE) {
        game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars.iter_mut().for_each(|car| car.nerve = nerve));
        let here = game.world.entities.get::<Player>(id).unwrap().character.position;
        let nearest = traffic(game)
            .cars
            .iter()
            .filter(|car| car.kind != KIND_EPA && car.driver.is_some())
            .filter_map(|car| game.world.physics.car_pose(car.vid?))
            .map(|pose| pose.position)
            .min_by(|a, b| (a[0] - here[0]).hypot(a[2] - here[2]).total_cmp(&(b[0] - here[0]).hypot(b[2] - here[2])));
        look_at(game, id, nearest.unwrap_or([here[0], here[1], here[2] - 10.0]), BUTTON_ALT);
        game.step();
        if done(game) {
            return true;
        }
    }
    false
}

#[test]
fn a_car_near_a_player_has_a_driver_at_its_wheel_and_a_far_one_has_none() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    run(&mut game, 60.0);
    let traffic = traffic(&game);
    let (mut manned, mut unmanned) = (0, 0);
    for car in &traffic.cars {
        match (car.vid, car.driver) {
            (Some(vid), Some(driver)) => {
                assert!(car.nearest < 190.0, "a driver {} m from anyone", car.nearest);
                assert_eq!(game.world.entities.of::<Driver>(driver).vehicle, vid, "the driver sits in their own car");
                manned += 1;
            }
            (_, None) => {
                assert!(car.nearest > 140.0, "a shown car {} m off with nobody in it", car.nearest);
                unmanned += 1;
            }
            (None, Some(_)) => panic!("a driver with no car"),
        }
    }
    assert!(manned > 0 && unmanned > 0, "{manned} cars with a driver and {unmanned} without");
    assert_eq!(game.world.entities.count::<Driver>(), manned, "one driver a car, and none on foot");
    // The car's seat is free for all the vehicles know: its driver is not a rider of it.
    assert!(shown(&game).iter().all(|&(vid, _)| game.world.entities.of::<Seats>(vid).vacant()));
}

#[test]
fn a_civilian_driver_who_climbs_out_is_the_same_person_in_the_same_colour() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let i = manned_car_near(&mut game, 60.0);
    let (vid, driver) = (traffic(&game).cars[i].vid.unwrap(), traffic(&game).cars[i].driver.unwrap());
    assert!(crate::mechanics::seat(&mut game.world, id, vid, 0), "the wheel is free to take");
    run(&mut game, 0.4);
    assert!(game.seen(driver).and_then(|seen| seen.seated).is_some_and(|seated| seated.step > 0), "the driver is seen climbing out");
    run(&mut game, 1.0);
    assert!(game.world.entities.has::<crate::mechanics::Civilian>(driver), "out, the same person");
    // Drawn at the wheel with no profile of their own, in the colour their id gives.
    assert_eq!(u32::from(game.world.entities.of::<Player>(driver).color), driver % u32::from(PLAYER_COLOR_COUNT), "in the same colour");
}

#[test]
fn a_player_who_takes_a_traffic_car_has_it_and_its_driver_is_out_on_the_road_running() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    // Until a car with its driver in it is near.
    let mut vid = None;
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        vid = traffic(&game).cars.iter().find(|car| car.driver.is_some() && car.nearest < 60.0).and_then(|car| car.vid);
        if vid.is_some() {
            break;
        }
    }
    let vid = vid.expect("a car with a driver came by");
    assert!(crate::mechanics::seat(&mut game.world, id, vid, 0), "the wheel is free to take");
    // The driver climbs out by their door (DOORS.md), which takes a second.
    run(&mut game, 0.5);
    assert!(on_foot(&game).is_empty(), "the driver is still getting out");
    run(&mut game, 1.0);
    assert!(game.world.entities.has::<Abandoned>(vid) && !game.world.entities.has::<TrafficCar>(vid), "it is the player's car now");
    let out = on_foot(&game);
    assert_eq!(out.len(), 1, "its driver is out of it");
    let car = game.world.physics.car_pose(vid).unwrap().position;
    let beside = (out[0].1[0] - car[0]).hypot(out[0].1[2] - car[2]);
    assert!(beside < 4.0, "the driver is put out beside the car, not {beside} m from it");
    run(&mut game, 4.0);
    let car = game.world.physics.car_pose(vid).unwrap().position;
    let out = on_foot(&game);
    let gone = (out[0].1[0] - car[0]).hypot(out[0].1[2] - car[2]);
    assert!(gone > beside + 10.0, "the driver runs: {gone} m from the car after four seconds");
}

#[test]
fn a_moving_traffic_car_is_not_boarded() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    // Until a car is driving by.
    let mut driving = None;
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        driving = traffic(&game).cars.iter().position(|car| car.vid.is_some() && car.physical && car.speed > 8.0);
        if driving.is_some() {
            break;
        }
    }
    let i = driving.expect("a car is driving by");
    // Beside it, pressing to board it, for as long as it drives.
    let mut pressed = 0;
    for _ in 0..30 {
        let car = traffic(&game).cars[i];
        let Some(pose) = car.vid.and_then(|vid| game.world.physics.car_pose(vid)) else { break };
        if pose.forward_speed < DRAG_SPEED + 1.0 {
            break;
        }
        let [x, _, z] = pose.world_point([-2.6, 0.0, 0.0]);
        stand(&mut game, id, x, z);
        look_at(&mut game, id, pose.position, BUTTON_USE);
        game.step();
        assert!(game.world.entities.get::<Player>(id).unwrap().seat.is_none(), "nobody boards a car going {} m/s", pose.forward_speed);
        pressed += 1;
    }
    assert!(pressed > 5, "the car drove on for the test to press beside it");
}

#[test]
fn a_driver_with_a_gun_on_them_who_gives_up_stops_gets_out_with_their_hands_up_and_runs() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let gave_up = threaten(&mut game, id, Nerve::GivesUp, 180, |game| !on_foot(game).is_empty());
    assert!(gave_up, "a driver got out");
    let (driver, at) = on_foot(&game).swap_remove(0);
    assert!(driver.hands_up > 0, "with their hands up");
    // Theirs is the car they got out of; another driver may have given up too while they climbed out.
    let left = game.world.entities.ids::<With<Abandoned>>().into_iter().min_by(|&a, &b| {
        let off = |vid: u32| game.world.physics.car_pose(vid).map_or(f32::MAX, |pose| (pose.position[0] - at[0]).hypot(pose.position[2] - at[2]));
        off(a).total_cmp(&off(b))
    });
    let left = left.expect("the car is left where it stopped");
    let pose = game.world.physics.car_pose(left).unwrap();
    assert!(pose.forward_speed.abs() < 1.0 && game.world.entities.of::<Seats>(left).vacant(), "standing, with nobody in it");
    let here = game.world.entities.get::<Player>(id).unwrap().character.position;
    let near = (at[0] - here[0]).hypot(at[2] - here[2]);
    run(&mut game, 6.0);
    let (driver, at) = on_foot(&game).into_iter().next().expect("the driver is still about");
    assert!(driver.hands_up == 0 && (at[0] - here[0]).hypot(at[2] - here[2]) > near + 10.0, "and then they run from the gun");
}

#[test]
fn a_driver_with_a_gun_on_them_who_flees_drives_off_faster_than_the_traffic() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let mut fastest = 0.0f32;
    let fled = threaten(&mut game, id, Nerve::Flees, 180, |game| {
        let traffic = traffic(game);
        for car in traffic.cars.iter().filter(|car| matches!(car.mood, Mood::Fleeing { .. })) {
            fastest = fastest.max(car.speed - traffic.graph.edge(car.way).speed * PACE[1]);
        }
        fastest > 2.0
    });
    assert!(fled, "a fleeing car got no more than {fastest} m/s over the fastest the traffic drives");
    assert!(on_foot(&game).is_empty() && game.world.entities.count::<Abandoned>() == 0, "its driver stays in it");
}

#[test]
fn a_driver_with_a_gun_on_them_who_goes_for_the_gunman_drives_at_them() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let mut nearest = f32::MAX;
    let rammed = threaten(&mut game, id, Nerve::Rams, 180, |game| {
        let here = game.world.entities.get::<Player>(id).unwrap().character.position;
        for car in traffic(game).cars.iter().filter(|car| matches!(car.mood, Mood::Ramming { .. })) {
            let pose = game.world.physics.car_pose(car.vid.unwrap()).unwrap();
            nearest = nearest.min((pose.position[0] - here[0]).hypot(pose.position[2] - here[2]));
        }
        nearest < 3.0
    });
    assert!(rammed, "the car that went for the gunman, who stands 7 m off its lane, came no nearer than {nearest} m");
}

/// Runs the highway until a car with its driver in it is within `within` metres of player `id`,
/// and says which car it is.
fn manned_car_near(game: &mut Game, within: f32) -> usize {
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        if let Some(i) = traffic(game).cars.iter().position(|car| car.driver.is_some() && car.vid.is_some() && car.nearest < within) {
            return i;
        }
    }
    panic!("no car with a driver came within {within} m");
}

#[test]
fn a_driver_at_the_wheel_is_hit_by_a_round_aimed_at_them_and_panics() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars.iter_mut().for_each(|car| car.nerve = Nerve::Flees));
    let i = manned_car_near(&mut game, 40.0);
    let car = traffic(&game).cars[i];
    let (vid, driver) = (car.vid.unwrap(), car.driver.unwrap());
    // Their body is a rider's, in the seat: a round from the player's eye to it meets them.
    let body = crate::mechanics::life::body(&game.world, driver, 0).expect("a driver at the wheel can be hit");
    assert_eq!(body.vehicle, Some(vid));
    let seat = game.world.physics.car_pose(vid).unwrap().world_point(shared::vehicles::seat_head(car.kind, 0, 0.0).unwrap());
    assert!((body.anchor[0] - seat[0]).hypot(body.anchor[2] - seat[2]) < 0.01, "they are where their seat is");
    let eye = game.world.entities.get::<Player>(id).unwrap().eye();
    let to = [body.centre[0] - eye[0], body.centre[1] - eye[1], body.centre[2] - eye[2]];
    let reach = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
    let hit = crate::mechanics::life::first_hurtbox_hit(&game.world, eye, to.map(|v| v / reach), reach + 2.0, Some(id), 0, None);
    assert_eq!(hit.map(|(who, _)| who), Some(driver), "the round meets the driver");
    // Wounded, they lose their head, whatever their nerve.
    crate::mechanics::damage_entity(&mut game.world, driver, crate::mechanics::Harm::new(30, Some(id), shared::protocol::WEAPON_BULLET));
    run(&mut game, 0.2);
    assert!(matches!(traffic(&game).cars[i].mood, Mood::Panicking { .. }), "{:?}", traffic(&game).cars[i].mood);
    assert!(traffic(&game).cars[i].physical, "and the car is theirs to have it out in");
    assert!(game.world.entities.alive(driver) && game.world.entities.of::<Driver>(driver).vehicle == vid, "and stay at the wheel");
}

#[test]
fn a_driver_hit_at_the_wheel_is_seen_struck_by_everyone() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let i = manned_car_near(&mut game, 40.0);
    let driver = traffic(&game).cars[i].driver.unwrap();
    crate::mechanics::damage_entity(&mut game.world, driver, crate::mechanics::Harm::new(10, Some(id), shared::protocol::WEAPON_BULLET));
    let scene = game.replicated();
    let (_, shown) = scene.iter().find(|(who, _)| *who == driver).expect("the driver is replicated");
    assert!(shown.seated.is_some(), "at the wheel");
    assert_eq!(shown.struck.as_ref().map(|struck| struck.count), Some(1), "and the hit is told, for the blood to be drawn");
}

#[test]
fn a_driver_shot_dead_at_speed_is_carried_on_by_the_car_until_it_stops_and_then_falls_out() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    // A car that is driving, with its driver in it.
    let mut found = None;
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        found = traffic(&game).cars.iter().position(|car| car.driver.is_some() && car.nearest < 40.0 && car.speed > 8.0);
        if found.is_some() {
            break;
        }
    }
    let i = found.expect("a car with a driver drove by");
    let car = traffic(&game).cars[i];
    let (vid, driver) = (car.vid.unwrap(), car.driver.unwrap());
    let from = game.world.physics.car_pose(vid).unwrap().position;
    crate::mechanics::damage_entity(&mut game.world, driver, crate::mechanics::Harm::new(PLAYER_MAX_HP, Some(id), shared::protocol::WEAPON_BULLET));
    run(&mut game, 0.3);
    assert!(matches!(traffic(&game).cars[i].mood, Mood::Runaway { .. }), "{:?}", traffic(&game).cars[i].mood);
    assert!(game.world.entities.dead(driver) && game.world.entities.of::<Driver>(driver).vehicle == vid, "dead, and still in the seat");
    assert!(on_foot(&game).is_empty());
    // It runs on until it is nobody's, with the player keeping up beside it to see.
    for _ in 0..(12 * TICK_RATE) {
        let at = game.world.physics.car_pose(vid).unwrap().position;
        stand(&mut game, id, at[0], at[2] + 12.0);
        game.step();
        if game.world.entities.has::<Abandoned>(vid) {
            break;
        }
    }
    assert!(game.world.entities.has::<Abandoned>(vid) && !game.world.entities.has::<TrafficCar>(vid), "nobody drives it");
    let to = game.world.physics.car_pose(vid).unwrap().position;
    let ran = (to[0] - from[0]).hypot(to[2] - from[2]);
    assert!(ran > 20.0, "it ran on {ran} m");
    let out = on_foot(&game);
    assert_eq!(out.len(), 1, "the body is out of the car");
    assert!(game.world.entities.dead(driver) && out[0].0.hands_up == 0);
    let at = out[0].1;
    run(&mut game, 5.0);
    assert!(game.world.physics.car_pose(vid).unwrap().forward_speed.abs() < 0.5, "the car has stopped");
    let lies = on_foot(&game)[0].1;
    assert!((lies[0] - at[0]).hypot(lies[2] - at[2]) < 0.01, "the dead do not run");
    run(&mut game, 6.0);
    assert!(on_foot(&game).is_empty(), "and the body is taken away");
}

#[test]
fn no_driver_is_run_down_by_the_car_they_sit_in() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let mut seen = 0;
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        for (driver, _) in game.world.entities.all::<Driver>() {
            seen += 1;
            assert_eq!(game.world.entities.of::<Health>(driver).hp, PLAYER_MAX_HP, "a driver at the wheel is unhurt");
        }
    }
    assert!(seen > 1000, "drivers drove by");
}

#[test]
fn a_badly_damaged_car_pulls_onto_the_verge_and_is_left_there_out_of_the_traffics_way() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 30.0);
    let i = manned_car_near(&mut game, 80.0);
    let vid = traffic(&game).cars[i].vid.unwrap();
    {
        let mut health = game.world.entities.of_mut::<Health>(vid);
        health.hp = health.max / 3;
    }
    for _ in 0..(40 * TICK_RATE) {
        game.step();
        if game.world.entities.has::<Abandoned>(vid) {
            break;
        }
    }
    assert!(game.world.entities.has::<Abandoned>(vid), "its driver has left it");
    let pose = game.world.physics.car_pose(vid).unwrap();
    let [hx, _, _] = vehicle_def(game.world.entities.of::<Vehicle>(vid).kind).spec.half_extents;
    assert!(pose.position[2].abs() - hx > 3.5 - 0.5, "it stands off the 7 m road: its middle is {} m from the centre line", pose.position[2].abs());
    // Out by their door, which takes a second.
    run(&mut game, 1.5);
    assert_eq!(on_foot(&game).len(), 1, "and the driver is on foot");
    // The traffic drives on past it as if it were not there.
    for _ in 0..(40 * TICK_RATE) {
        game.step();
        if !game.world.entities.has::<Abandoned>(vid) {
            break;
        }
        assert!(game.world.entities.of::<Abandoned>(vid).in_way.is_none(), "no car finds it in its path");
        assert!(traffic(&game).cars.iter().all(|car| !(car.aside > 0.0 && car.passing.is_none())), "and none goes round it");
    }
}

#[test]
fn a_scratched_car_goes_back_on_rails_as_traffic() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    // Until a car is let go beside the player.
    let mut found = None;
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        found = traffic(&game).cars.iter().position(|car| car.physical);
        if found.is_some() {
            break;
        }
    }
    let i = found.expect("a car came near enough to be let go");
    let vid = traffic(&game).cars[i].vid.unwrap();
    // A dent its driver makes nothing of.
    let hp = {
        let mut health = game.world.entities.of_mut::<Health>(vid);
        health.hp = health.max - health.max / 5;
        health.hp
    };
    game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars[i].hp = hp);
    let mut back = false;
    for _ in 0..(30 * TICK_RATE) {
        game.step();
        if traffic(&game).cars[i].vid != Some(vid) {
            break;
        }
        back |= game.world.physics.car_on_rails(vid);
    }
    assert!(back, "it went back on rails once it had driven on");
}

/// Makes a driver near player `id` on the highway give their car up, and says which vehicle it
/// was: it is left standing in its lane.
fn car_left_in_the_lane(game: &mut Game, id: u32) -> u32 {
    let gave_up = threaten(game, id, Nerve::GivesUp, 180, |game| game.world.entities.count::<Abandoned>() > 0);
    assert!(gave_up, "a driver gave their car up");
    game.world.entities.ids::<With<Abandoned>>()[0]
}

#[test]
fn a_car_left_in_the_lane_is_taken_away_soon_after_the_player_turns_their_back_on_it() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let vid = car_left_in_the_lane(&mut game, id);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    // Sixty metres off, looking the other way.
    stand(&mut game, id, at[0], 60.0);
    let mut gone = None;
    for seq in 1..=(60 * TICK_RATE) {
        look_at(&mut game, id, [at[0], sim::STANDING_Y, 200.0], 0);
        game.step();
        if game.world.physics.car_pose(vid).is_none() {
            gone = Some(seq as f32 / TICK_RATE as f32);
            break;
        }
    }
    let gone = gone.expect("it is taken away within a minute");
    assert!(gone > 15.0, "not at once: after {gone} s");
}

#[test]
fn a_car_left_in_the_lane_stays_a_while_under_a_players_eyes_and_for_good_with_one_in_it() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let vid = car_left_in_the_lane(&mut game, id);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    // Watched from 20 m, it outlasts the time it would have with nobody looking, and then goes.
    stand(&mut game, id, at[0], 20.0);
    let watch = |game: &mut Game, seconds: u32| {
        for _ in 0..(seconds * TICK_RATE) {
            look_at(game, id, at, 0);
            game.step();
        }
    };
    watch(&mut game, 60);
    assert!(game.world.physics.car_pose(vid).is_some(), "it is still there after a minute of being watched");
    watch(&mut game, 250);
    assert!(game.world.physics.car_pose(vid).is_none(), "and gone after five");
    // Another, with the player sat in it, is theirs for as long as they sit there.
    stand(&mut game, id, 0.0, 9.0);
    run(&mut game, 5.0);
    let vid = car_left_in_the_lane(&mut game, id);
    let free = game.world.entities.of::<Seats>(vid).free().unwrap();
    assert!(crate::mechanics::seat(&mut game.world, id, vid, free));
    run(&mut game, 330.0);
    assert!(game.world.physics.car_pose(vid).is_some() && game.world.entities.has::<Abandoned>(vid), "a car with a player in it is not taken away");
}

#[test]
fn a_jam_behind_a_blocked_road_clears_once_the_road_is_open_again() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 30.0);
    // Both lanes shut by a jeep each, which are nobody's to take away.
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    let blocks = [1.75, -1.75].map(|z| crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [0.0, ride, z], std::f32::consts::FRAC_PI_2));
    run(&mut game, 90.0);
    let standing = |game: &Game| traffic(game).cars.iter().filter(|car| car.speed < 0.5).count();
    assert!(standing(&game) >= 3, "cars queue at the jeeps: {} stand", standing(&game));
    for vid in blocks {
        remove_vehicle(&mut game.world, vid);
    }
    run(&mut game, 150.0);
    let traffic = traffic(&game);
    let longest = traffic.cars.iter().map(|car| car.stood).max().unwrap_or(0);
    let mean = traffic.cars.iter().map(|car| car.speed).sum::<f32>() / traffic.cars.len() as f32;
    assert!(longest < ticks(30.0), "no car has stood for more than {} s", longest / TICK_RATE);
    assert!(mean > 6.0, "the traffic is moving again: {mean} m/s");
}

#[test]
fn no_more_than_two_dozen_cars_that_were_traffic_are_left_standing() {
    let mut game = highway(4);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 30.0);
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    // Thirty of them beside the player, where each would be let stand as theirs.
    for n in 0..30 {
        let vid = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [(n % 10) as f32 * 3.0 - 13.5, ride, 22.0 + (n / 10) as f32 * 6.0], 0.0);
        game.world.entities.insert(vid, Abandoned::default());
    }
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.count::<Abandoned>(), ABANDONED_MAX);
}

/// A blast at `at`, heard as far as blasts are.
fn blast(game: &mut Game, at: [f32; 3]) {
    game.world.send(Noise { at, by: None, reach: crate::mechanics::bots::HEAR_BLAST, line: None });
}

/// Where shown car `i` is.
fn place(game: &Game, i: usize) -> [f32; 3] {
    game.world.physics.car_pose(traffic(game).cars[i].vid.unwrap()).unwrap().position
}

#[test]
fn a_blast_in_earshot_makes_drivers_hurry_and_one_out_of_earshot_does_not() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    run(&mut game, 90.0);
    // 110 m south of the road: heard along 100 m of it either way, and no further.
    blast(&mut game, [0.0, 0.0, 110.0]);
    game.step();
    let (mut near, mut far) = (0, 0);
    for (i, car) in traffic(&game).cars.iter().enumerate().filter(|(_, car)| car.vid.is_some()) {
        let x = place(&game, i)[0];
        if x.abs() < 90.0 {
            assert!(matches!(car.mood, Mood::Hurrying { .. }), "a car {x} m along heard it: {:?}", car.mood);
            near += 1;
        } else if x.abs() > 115.0 {
            assert_eq!(car.mood, Mood::Calm, "a car {x} m along did not");
            far += 1;
        }
    }
    assert!(near > 0 && far > 0, "{near} cars in earshot and {far} out of it");
    // And they drive faster than they would.
    let mut over = 0.0f32;
    for _ in 0..(8 * TICK_RATE) {
        game.step();
        let traffic = traffic(&game);
        for car in traffic.cars.iter().filter(|car| matches!(car.mood, Mood::Hurrying { .. })) {
            over = over.max(car.speed - traffic.graph.edge(car.way).speed * car.pace);
        }
    }
    assert!(over > 1.5, "a hurrying car got {over} m/s over its pace");
}

#[test]
fn a_blast_beside_a_car_has_its_driver_flee_panic_or_get_out_as_their_nerve_is() {
    for nerve in [Nerve::GivesUp, Nerve::Flees, Nerve::Rams] {
        let mut game = highway(24);
        let id = join(&mut game);
        stand(&mut game, id, 0.0, 9.0);
        let i = manned_car_near(&mut game, 60.0);
        game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars[i].nerve = nerve);
        let at = place(&game, i);
        blast(&mut game, [at[0], at[1], at[2] + 8.0]);
        game.step();
        let mood = traffic(&game).cars[i].mood;
        match nerve {
            Nerve::GivesUp => assert_eq!(mood, Mood::GivingUp),
            Nerve::Flees => assert!(matches!(mood, Mood::Fleeing { .. }), "{mood:?}"),
            Nerve::Rams => assert!(matches!(mood, Mood::Panicking { .. }), "{mood:?}"),
        }
    }
}

#[test]
fn gunfire_makes_drivers_hurry_and_a_round_past_the_car_makes_a_timid_one_duck_and_then_flee() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    let i = manned_car_near(&mut game, 60.0);
    game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars[i].nerve = Nerve::GivesUp);
    // A shot 50 m off, fired away from the road.
    let at = place(&game, i);
    game.world.send(Noise { at: [at[0], 1.0, at[2] + 50.0], by: None, reach: crate::mechanics::bots::HEAR_SHOT, line: Some([0.0, 0.0, 1.0]) });
    game.step();
    assert!(matches!(traffic(&game).cars[i].mood, Mood::Hurrying { .. }), "{:?}", traffic(&game).cars[i].mood);
    // And one whose round goes by the car.
    let at = place(&game, i);
    game.world.send(Noise { at: [at[0] + 1.0, 1.0, at[2] + 30.0], by: None, reach: crate::mechanics::bots::HEAR_SHOT, line: Some([0.0, 0.0, -1.0]) });
    game.step();
    assert!(matches!(traffic(&game).cars[i].mood, Mood::Ducking { .. }), "{:?}", traffic(&game).cars[i].mood);
    let mut stopped = false;
    for _ in 0..(5 * TICK_RATE) {
        game.step();
        let car = traffic(&game).cars[i];
        if !matches!(car.mood, Mood::Ducking { .. }) {
            break;
        }
        stopped |= car.speed.abs() < 0.5;
    }
    assert!(stopped, "it stood while its driver was down");
    assert!(matches!(traffic(&game).cars[i].mood, Mood::Fleeing { .. }), "{:?}", traffic(&game).cars[i].mood);
}

#[test]
fn a_rash_driver_whose_car_is_shot_leaves_the_lane_and_ends_against_what_stands_by_the_road() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 30.0);
    // A row of parked jeeps along each verge.
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    for n in 0..120 {
        for z in [-7.5, 7.5] {
            crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [n as f32 * 5.0 - 300.0, ride, z], std::f32::consts::FRAC_PI_2);
        }
    }
    // A car driving between them, whose driver is rash.
    let mut found = None;
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        found = traffic(&game).cars.iter().enumerate().position(|(i, car)| car.driver.is_some() && car.speed > 8.0 && car.mood == Mood::Calm && place(&game, i)[0].abs() < 100.0);
        if found.is_some() {
            break;
        }
    }
    let i = found.expect("a car drove between the rows");
    let vid = traffic(&game).cars[i].vid.unwrap();
    game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars[i].rash = true);
    crate::mechanics::damage_vehicle(&mut game.world, vid, 5, Some(id));
    let mut off = 0.0f32;
    let mut panicked = false;
    for _ in 0..(20 * TICK_RATE) {
        game.step();
        if game.world.entities.has::<Abandoned>(vid) || game.world.physics.car_pose(vid).is_none() {
            break;
        }
        let car = traffic(&game).cars[i];
        panicked |= matches!(car.mood, Mood::Panicking { .. });
        off = off.max(car.lateral.abs());
    }
    assert!(panicked, "its driver lost their head");
    assert!(off > 2.0, "it left its lane by {off} m");
    assert!(game.world.entities.has::<Abandoned>(vid) || game.world.physics.car_pose(vid).is_none() || game.world.entities.dead(vid), "and came to a stop against something");
}

#[test]
fn a_fleeing_driver_passes_a_slower_car_with_another_coming_the_other_way() {
    let mut game = highway(40);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    run(&mut game, 60.0);
    // Every car flees: none of them cares who is coming.
    let mut near = f32::MAX;
    for _ in 0..(60 * TICK_RATE) {
        game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars.iter_mut().for_each(|car| car.mood = Mood::Fleeing { left: 1000 }));
        game.step();
        let traffic = traffic(&game);
        for i in 0..traffic.cars.len() {
            let car = &traffic.cars[i];
            if car.shift > 1.0 && car.passing.is_some() && let Some((gap, ..)) = traffic.oncoming(i) {
                near = near.min(gap);
            }
        }
        if near < 100.0 {
            break;
        }
    }
    assert!(near < 100.0, "the nearest anyone came the other way while a car was out passing was {near} m");
}

#[test]
fn a_car_on_rails_with_a_vehicle_right_ahead_of_it_is_let_off_them() {
    let mut game = highway(24);
    let id = join(&mut game);
    // Far enough from the road that no car is let go for the player's being near.
    stand(&mut game, id, 0.0, 120.0);
    run(&mut game, 60.0);
    assert_eq!(traffic(&game).counts().2, 0, "every shown car is on rails");
    let i = traffic(&game).cars.iter().position(|car| car.vid.is_some() && car.speed > 5.0).expect("a car is shown");
    // A jeep set down across its lane a car's length ahead of its nose.
    let (ahead, _) = traffic(&game).pose_ahead(i, half_length(traffic(&game).cars[i].kind) + 2.2);
    let ride = sim::Physics::car_ride_height(KIND_JEEP);
    crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [ahead[0], ahead[1] + ride, ahead[2]], 0.0);
    run(&mut game, 0.2);
    assert!(traffic(&game).cars[i].physical, "it is let off its rails to be hit");
}

#[test]
fn a_gun_that_is_in_hand_but_not_aimed_frightens_nobody_however_long_it_points_at_a_car() {
    let mut game = highway(24);
    let id = join(&mut game);
    stand(&mut game, id, 0.0, 9.0);
    game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar[0] = GUN.with(std::cell::Cell::get);
    let mut pointed = 0;
    for _ in 0..(120 * TICK_RATE) {
        let here = game.world.entities.get::<Player>(id).unwrap().character.position;
        let nearest = traffic(&game)
            .cars
            .iter()
            .filter(|car| car.driver.is_some())
            .filter_map(|car| game.world.physics.car_pose(car.vid?))
            .map(|pose| pose.position)
            .filter(|p| (p[0] - here[0]).hypot(p[2] - here[2]) < 25.0)
            .min_by(|a, b| (a[0] - here[0]).hypot(a[2] - here[2]).total_cmp(&(b[0] - here[0]).hypot(b[2] - here[2])));
        if let Some(at) = nearest {
            look_at(&mut game, id, at, 0);
            pointed += 1;
        }
        game.step();
        assert!(traffic(&game).cars.iter().all(|car| car.mood == Mood::Calm), "a driver took fright");
    }
    assert!(pointed > 5 * TICK_RATE, "the gun was on a car for {pointed} ticks");
}

#[test]
fn the_traffic_is_a_mix_of_the_period_cars_each_riding_at_its_own_height() {
    let mut game = game(200);
    run(&mut game, 1.0);
    let mut kinds: Vec<u8> = traffic(&game).cars.iter().map(|car| car.kind).collect();
    // The everyday cars are listed twice to the Bronco's, the Manta's, the van's and the EPA's once.
    let everyday = kinds.iter().filter(|&&kind| ![KIND_JEEP, KIND_SPORTS, KIND_TRANSPORTER, KIND_EPA].contains(&kind)).count();
    assert!(everyday > kinds.len() * 2 / 3, "{everyday} of {} are everyday cars", kinds.len());
    kinds.sort_unstable();
    kinds.dedup();
    assert!(kinds.len() >= 10, "two hundred cars are of at least ten kinds: {kinds:?}");
    // Shown beside a player, each sits on the road at the height its own springs hold it.
    let id = join(&mut game);
    stand(&mut game, id, 0.0, -140.0);
    run(&mut game, 3.0);
    let on_rails: Vec<u32> = shown(&game).into_iter().filter(|&(_, rails)| rails).map(|(vid, _)| vid).collect();
    assert!(!on_rails.is_empty());
    for vid in on_rails {
        let kind = game.world.entities.of::<Vehicle>(vid).kind;
        let y = game.world.physics.car_pose(vid).unwrap().position[1];
        assert!((y - sim::Physics::car_ride_height(kind)).abs() < 0.05, "kind {kind} rides at {y}");
    }
}

/// Gives `game` emergency services with `patrols` police cars out on patrol.
fn services(game: &mut Game, patrols: usize) {
    (game.world.fleet.ambulance, game.world.fleet.police, game.world.fleet.patrols) = (true, true, patrols);
    game.step();
}

/// Sends a unit of `service` to `to` for `tag`, and gives which unit went. The tag is one that
/// `Emergency`, which commands the fleet in a real game, takes for none of its own suspects', so it
/// leaves the unit to the test.
fn send(game: &mut Game, service: Service, to: [f32; 3], tag: u32) -> usize {
    let tag = tag | 1 << 31;
    let kind = if service == Service::Police { KIND_POLICE_CAR } else { KIND_AMBULANCE };
    game.world.fleet.orders.push(FleetOrder::Send { service, kind, to, tag, ahead: None });
    game.step();
    game.world.fleet.units.iter().position(|unit| unit.tag == tag).expect("a unit was sent")
}

/// Steps `game` until unit `unit` is on `duty`, for `seconds` at most. Whether it got to be.
fn until(game: &mut Game, unit: usize, duty: Duty, seconds: f32) -> bool {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        if game.world.fleet.units[unit].duty == duty {
            return true;
        }
        game.step();
    }
    game.world.fleet.units[unit].duty == duty
}

#[test]
fn a_world_without_emergency_services_has_no_units_and_one_with_them_has_them_dormant() {
    let mut plain = game(10);
    run(&mut plain, 1.0);
    assert!(plain.world.fleet.units.is_empty());
    let mut game = game(10);
    services(&mut game, 0);
    run(&mut game, 5.0);
    assert_eq!(game.world.fleet.units.len(), FLEET);
    assert_eq!(game.world.fleet.units.iter().filter(|unit| unit.service == Service::Police).count(), POLICE);
    assert!(game.world.fleet.units.iter().all(|unit| unit.duty == Duty::Dormant && unit.vid.is_none()));
    assert_eq!(traffic(&game).counts().0, 10, "the traffic is as it was");
    // The traffic that was there before the fleet still drives, each car in the lane it was in.
    let traffic = traffic(&game);
    for (way, lane) in traffic.lanes.iter().enumerate() {
        assert!(lane.iter().all(|&car| car as usize >= FLEET && traffic.cars[car as usize].way.index() == way));
    }
}

#[test]
fn a_unit_sent_somewhere_drives_there_by_road_fast_parks_on_the_verge_and_is_gone_when_let_go() {
    let mut game = game(0);
    services(&mut game, 0);
    // Half way down the east side of the block.
    let to = [200.0, 0.0, 0.0];
    let unit = send(&mut game, Service::Ambulance, to, 7);
    assert_eq!(game.world.fleet.units[unit].service, Service::Ambulance);
    assert_eq!(game.world.fleet.units[unit].duty, Duty::Responding);
    let from = game.world.fleet.units[unit].at;
    assert!((from[0] - to[0]).hypot(from[2] - to[2]) > 250.0, "it starts well away: {from:?}");
    let mut fastest = 0.0f32;
    let mut turns = 0;
    let mut way = traffic(&game).cars[unit].way;
    for _ in 0..(180 * TICK_RATE) {
        game.step();
        let car = traffic(&game).cars[unit];
        fastest = fastest.max(car.speed);
        if car.way != way {
            (way, turns) = (car.way, turns + 1);
        }
        assert!(game.world.fleet.units[unit].siren == (car.duty == Duty::Responding));
        if car.duty == Duty::OnScene {
            break;
        }
    }
    let at = game.world.fleet.units[unit];
    assert_eq!(at.duty, Duty::OnScene, "it got there");
    assert!((at.at[0] - to[0]).hypot(at.at[2] - to[2]) < 8.0, "it stands by the place: {:?}", at.at);
    assert!(turns >= 1, "by more than one road");
    assert!(fastest > 12.0, "faster than the 10 m/s its roads are driven at: {fastest}");
    let car = traffic(&game).cars[unit];
    assert!(car.shift < -2.0 && car.speed == 0.0, "off the road on its right: {} m", car.shift);
    assert!(traffic(&game).lanes.iter().all(|lane| !lane.contains(&(unit as u32))), "and in nobody's lane");
    // It stands there as long as it is wanted.
    run(&mut game, 200.0);
    assert_eq!(game.world.fleet.units[unit].duty, Duty::OnScene);
    // Let go, with nobody to see it leave, it is gone.
    game.world.fleet.orders.push(FleetOrder::Release { unit });
    run(&mut game, 1.0);
    assert_eq!(game.world.fleet.units[unit].duty, Duty::Dormant);
    assert_eq!(game.world.fleet.units[unit].tag, 0);
}

#[test]
fn a_unit_someone_watches_arrive_is_a_car_that_parks_itself_and_drives_off_out_of_sight() {
    let mut game = game(0);
    let id = join(&mut game);
    stand(&mut game, id, 230.0, 20.0);
    services(&mut game, 0);
    let unit = send(&mut game, Service::Police, [200.0, 0.0, 0.0], 3);
    let from = game.world.fleet.units[unit].at;
    assert!((from[0] - 230.0).hypot(from[2] - 20.0) > HIDE_BEYOND, "it starts out of the player's sight");
    assert!(until(&mut game, unit, Duty::OnScene, 180.0), "it got there");
    run(&mut game, 3.0);
    let at = game.world.fleet.units[unit];
    let vid = at.vid.expect("a car to look at");
    assert!(!game.world.physics.car_on_rails(vid), "parked, it is a car like any other");
    let pose = game.world.physics.car_pose(vid).unwrap();
    assert!(pose.forward_speed.abs() < 0.2 && pose.up_y > 0.95, "standing on its wheels");
    assert!((pose.position[0] - 200.0).hypot(pose.position[2]) < 12.0, "by the place: {:?}", pose.position);
    assert!(game.world.entities.get::<Seats>(vid).is_some_and(|seats| seats.rider(0).is_none()), "with its wheel free");
    // Sent on round the corner, it drives there.
    game.world.fleet.orders.push(FleetOrder::Redirect { unit, to: [100.0, 0.0, 150.0] });
    game.step();
    assert_eq!(game.world.fleet.units[unit].duty, Duty::Responding);
    assert!(until(&mut game, unit, Duty::OnScene, 120.0), "it got there too");
    let at = game.world.fleet.units[unit].at;
    assert!((at[0] - 100.0).hypot(at[2] - 150.0) < 12.0, "{at:?}");
    // Let go, it drives off, and is gone once it is out of sight.
    game.world.fleet.orders.push(FleetOrder::Release { unit });
    game.step();
    assert_eq!(game.world.fleet.units[unit].duty, Duty::Leaving);
    assert!(!game.world.fleet.units[unit].siren);
    assert!(until(&mut game, unit, Duty::Dormant, 240.0), "it left");
    assert!(game.world.entities.get::<Vehicle>(vid).is_none(), "car and all");
}

#[test]
fn traffic_pulls_onto_the_verge_for_a_siren_and_drives_on_when_it_is_past() {
    // A long straight road with traffic on it, and a unit sent from one end to the other.
    let run_it = |sirens: bool| {
        let mut game = highway(40);
        services(&mut game, 0);
        run(&mut game, 20.0);
        let unit = send(&mut game, Service::Police, [1200.0, 0.0, 0.0], 1);
        let start = game.world.fleet.units[unit].at[0];
        if !sirens {
            game.with_mechanic(|traffic: &mut Traffic, _| traffic.cars[unit].mood = Mood::Calm);
        }
        let (mut yielded, mut seconds) = (0, 0.0);
        while game.world.fleet.units[unit].duty == Duty::Responding && seconds < 400.0 {
            run(&mut game, 0.5);
            seconds += 0.5;
            let traffic = traffic(&game);
            yielded = yielded.max(traffic.cars.iter().filter(|car| matches!(car.mood, Mood::Yielding { .. }) && car.shift < -YIELDED).count());
            // Nobody behind the siren is still out of its way.
            let siren = traffic.cars[unit];
            for car in traffic.cars.iter().filter(|car| car.way == siren.way && car.along < siren.along - 60.0) {
                assert!(!matches!(car.mood, Mood::Yielding { .. }), "a car 60 m behind the siren still waits");
            }
        }
        (game, unit, yielded, seconds / (1200.0 - start).max(1.0))
    };
    let (mut game, unit, yielded, pace) = run_it(true);
    assert_eq!(game.world.fleet.units[unit].duty, Duty::OnScene, "it got there");
    assert!(yielded >= 1, "cars pulled over for it");
    let (.., slow) = run_it(false);
    assert!(pace < slow * 0.8, "and it was there sooner for it: {pace} s/m against {slow}");
    // The traffic is back in its lanes and driving.
    run(&mut game, 20.0);
    let traffic = traffic(&game);
    assert!(traffic.cars[FLEET..].iter().all(|car| car.mood == Mood::Calm && car.shift.abs() < 0.5));
    assert!(traffic.cars[FLEET..].iter().filter(|car| car.speed > 5.0).count() > 30);
}

#[test]
fn a_unit_with_its_siren_on_is_given_a_junction_that_is_another_roads() {
    let mut game = game(60);
    services(&mut game, 0);
    run(&mut game, 30.0);
    let unit = send(&mut game, Service::Police, [0.0, 0.0, 100.0], 1);
    let (mut crossed, mut waited) = (0, 0);
    let mut held = None;
    while game.world.fleet.units[unit].duty == Duty::Responding {
        game.step();
        let traffic = traffic(&game);
        let car = traffic.cars[unit];
        if let Some(node) = car.holds.filter(|&node| Some(node) != held) {
            // The junction is its road's the moment it has it, whoever's it was.
            assert_eq!(traffic.junctions[node as usize].map(|(way, ..)| way), Some(car.way.index() as u32));
            crossed += 1;
        }
        held = car.holds;
        waited = waited.max(car.waiting);
        assert!(game.world.tick < 400 * TICK_RATE, "it never arrived");
    }
    assert!(crossed >= 1, "it crossed a junction on its way");
    assert_eq!(waited, 0, "and waited at none");
}

#[test]
fn traffic_goes_round_a_unit_parked_at_a_scene_and_does_not_queue_behind_it() {
    let mut game = game(40);
    let id = join(&mut game);
    stand(&mut game, id, 215.0, 0.0);
    services(&mut game, 0);
    let unit = send(&mut game, Service::Ambulance, [200.0, 0.0, 0.0], 1);
    assert!(until(&mut game, unit, Duty::OnScene, 240.0), "it got there");
    let parked = traffic(&game).cars[unit];
    // The cars of its lane keep coming by, and none stands behind it.
    let (mut passed, mut last): (usize, Vec<u32>) = (0, Vec::new());
    for _ in 0..(120 * TICK_RATE) {
        game.step();
        let traffic = traffic(&game);
        let by: Vec<u32> = traffic.lanes[parked.way.index()].iter().copied().filter(|&car| traffic.cars[car as usize].along > parked.along + 10.0).collect();
        passed += by.iter().filter(|car| !last.contains(car)).count();
        last = by;
        for &car in &traffic.lanes[parked.way.index()] {
            let car = traffic.cars[car as usize];
            assert!(!(car.along < parked.along && car.along > parked.along - 30.0 && car.stood > 20 * TICK_RATE), "a car has stood behind it for 20 s");
        }
    }
    assert!(passed >= 3, "{passed} cars came by");
    assert_eq!(game.world.fleet.units[unit].duty, Duty::OnScene);
}

#[test]
fn a_unit_whose_car_is_wrecked_or_taken_is_lost_and_the_traffic_keeps_its_number() {
    let mut game = game(10);
    let id = join(&mut game);
    stand(&mut game, id, 230.0, 20.0);
    services(&mut game, 0);
    let unit = send(&mut game, Service::Police, [200.0, 0.0, 0.0], 9);
    assert!(until(&mut game, unit, Duty::OnScene, 180.0), "it got there");
    run(&mut game, 1.0);
    let vid = game.world.fleet.units[unit].vid.expect("a car to take");
    // A player takes its wheel.
    crate::mechanics::seat(&mut game.world, id, vid, 0);
    run(&mut game, 1.0);
    // Whoever sent it sees that it is gone: the unit no longer has that car, nor that errand.
    let lost = game.world.fleet.units[unit];
    assert!(lost.vid != Some(vid) && lost.tag != 9 | 1 << 31, "{lost:?}");
    assert!(game.world.entities.has::<Abandoned>(vid) && !game.world.entities.has::<TrafficCar>(vid), "the car is the player's");
    assert!(game.world.fleet.units.iter().all(|unit| unit.vid != Some(vid)));
    assert_eq!(traffic(&game).counts().0, 10);
}

#[test]
fn a_unit_plans_another_way_when_the_road_it_meant_to_take_is_shut_and_is_lost_when_there_is_none() {
    let mut game = game(0);
    services(&mut game, 0);
    // To the road across the middle of the block, which is reached from either end of it.
    let unit = send(&mut game, Service::Police, [0.0, 0.0, 50.0], 1);
    let planned: Vec<Way> = traffic(&game).plans[unit].ways.iter().copied().collect();
    let last = *planned.last().expect("it has more than one road to drive");
    let shut = planned.iter().copied().find(|way| way.edge != last.edge).expect("and a road between where it is and the last");
    game.with_mechanic(|traffic: &mut Traffic, world| traffic.shut(shut.edge, world));
    assert!(traffic(&game).plans[unit].ways.iter().all(|way| way.edge != shut.edge), "its plan no longer has the road that is shut");
    assert!(until(&mut game, unit, Duty::OnScene, 240.0), "and it got there another way");
    let at = game.world.fleet.units[unit].at;
    assert!(at[0].hypot(at[2] - 50.0) < 8.0, "{at:?}");
    // One whose way to where it was sent is gone altogether is lost to whoever sent it.
    let other = send(&mut game, Service::Ambulance, [200.0, 0.0, 0.0], 2);
    let end = traffic(&game).plans[other].last;
    game.with_mechanic(|traffic: &mut Traffic, world| traffic.shut(end.edge, world));
    game.step();
    let lost = game.world.fleet.units[other];
    assert_eq!((lost.duty, lost.tag), (Duty::Dormant, 0));
}

#[test]
fn a_world_with_patrols_keeps_that_many_police_cars_on_the_roads_and_sends_the_nearest() {
    let mut game = game(10);
    services(&mut game, 2);
    run(&mut game, 30.0);
    let patrols = |game: &Game| game.world.fleet.units.iter().filter(|unit| unit.duty == Duty::Patrol).count();
    assert_eq!(patrols(&game), 2);
    assert!(game.world.fleet.units.iter().all(|unit| unit.duty != Duty::Patrol || unit.service == Service::Police));
    assert!(traffic(&game).cars[..FLEET].iter().filter(|car| car.duty == Duty::Patrol).all(|car| car.speed > 1.0 || car.waiting > 0 || car.along > 0.0));
    // A patrol car that is near by road is the one that goes, with no siren until it is sent.
    assert!(game.world.fleet.units.iter().all(|unit| !unit.siren));
    let unit = send(&mut game, Service::Police, [0.0, 0.0, 0.0], 5);
    assert_eq!(patrols(&game), 1, "one of the two went");
    // And another takes its place on patrol.
    run(&mut game, 3.0);
    assert_eq!(patrols(&game), 2);
    assert!(until(&mut game, unit, Duty::OnScene, 180.0));
    // Let go with nobody near, it is gone: the patrols are already what they should be.
    game.world.fleet.orders.push(FleetOrder::Release { unit });
    run(&mut game, 2.0);
    assert_eq!(game.world.fleet.units[unit].duty, Duty::Dormant);
    assert_eq!(patrols(&game), 2);
    // No more than there are police cars, whatever is asked for: the vans do not patrol.
    game.world.fleet.patrols = 50;
    run(&mut game, 20.0);
    assert_eq!(patrols(&game), fleet::CARS);
    assert!(game.world.fleet.units.iter().all(|unit| unit.duty != Duty::Patrol || unit.kind == KIND_POLICE_CAR));
    assert_eq!(traffic(&game).counts().0, 10);
}

#[test]
fn nothing_is_sent_to_a_place_no_road_leads_to_or_when_every_unit_is_out() {
    let mut game = game(0);
    services(&mut game, 0);
    game.world.fleet.orders.push(FleetOrder::Send { service: Service::Ambulance, kind: KIND_AMBULANCE, to: [-500.0, 0.0, 500.0], tag: 1, ahead: None });
    run(&mut game, 1.0);
    assert!(game.world.fleet.units.iter().all(|unit| unit.duty == Duty::Dormant), "a field far from any road");
    for tag in 1..=3 {
        game.world.fleet.orders.push(FleetOrder::Send { service: Service::Ambulance, kind: KIND_AMBULANCE, to: [200.0, 0.0, 0.0], tag, ahead: None });
        game.step();
    }
    let out: Vec<u32> = game.world.fleet.units.iter().filter(|unit| unit.duty != Duty::Dormant).map(|unit| unit.tag).collect();
    assert_eq!(out, vec![1, 2], "there are two ambulances, and no police car goes in their place");
}
