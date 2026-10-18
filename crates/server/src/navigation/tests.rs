use super::*;
use shared::world::{CollisionMesh, WORLD_VERSION};

/// A flat 200 m square with 10 m tall boxes on it, each `[x0, z0, x1, z1]` in whole metres.
fn world(boxes: &[[i32; 4]]) -> (WorldFile, Physics) {
    blocks(&boxes.iter().map(|&[x0, z0, x1, z1]| [x0, z0, x1, z1, 1000]).collect::<Vec<_>>())
}

/// A flat 200 m square with boxes on it, each `[x0, z0, x1, z1]` in whole metres and its height in centimetres.
fn blocks(boxes: &[[i32; 5]]) -> (WorldFile, Physics) {
    let mut map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, ..Default::default() };
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    for &[x0, z0, x1, z1, height] in boxes {
        let base = vertices.len() as u32;
        for y in [0, height] {
            for (x, z) in [(x0, z0), (x1, z0), (x1, z1), (x0, z1)] {
                vertices.push([x * 100, y, z * 100]);
            }
        }
        for t in [[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]] {
            triangles.push(t.map(|v: u32| base + v));
        }
    }
    map.collision = CollisionMesh { vertices, triangles };
    let physics = Physics::new(&map).unwrap();
    (map, physics)
}

/// Feet on the flat ground at `[x, z]`.
fn feet([x, z]: [f32; 2]) -> [f32; 3] {
    [x, 0.0, z]
}

fn plenty() -> usize {
    MAX_SEARCH
}

fn length(from: [f32; 2], path: &[[f32; 2]]) -> f32 {
    let mut at = from;
    path.iter()
        .map(|&p| {
            let d = (p[0] - at[0]).hypot(p[1] - at[1]);
            at = p;
            d
        })
        .sum()
}

#[test]
fn open_ground_is_one_straight_line() {
    let (map, physics) = world(&[]);
    let mut grid = NavGrid::default();
    assert_eq!(grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [40.0, 0.0, 25.0], &mut plenty()), Some(vec![[40.0, 25.0]]));
}

#[test]
fn a_wall_and_its_surroundings_are_blocked_and_the_rest_is_open() {
    let (map, physics) = world(&[[10, -30, 12, 30]]);
    let mut grid = NavGrid::default();
    // The top of the wall is a floor of its own, which the ground beside it does not join.
    assert!(grid.floor(&physics, &map, cell_of([11.0, 0.0])).is_none_or(|top| top > 9.0), "inside the wall");
    assert!(!grid.open(&physics, &map, cell_of([9.6, 0.0])), "against it");
    assert!(grid.open(&physics, &map, cell_of([8.5, 0.0])), "a metre and a half off");
    assert!(!grid.open(&physics, &map, cell_of([99.5, 0.0])), "the edge of the world");
}

#[test]
fn it_goes_round_the_end_of_a_wall_without_cutting_through() {
    let (map, physics) = world(&[[10, -30, 12, 30]]);
    let mut grid = NavGrid::default();
    let (from, to) = ([0.0, 0.0], [25.0, 0.0]);
    let path = grid.find_path(&physics, &map, feet(from), feet(to), &mut plenty()).expect("a way round");
    assert_eq!(path.last(), Some(&to));
    let mut at = from;
    for &corner in &path {
        assert!(grid.clear_line(&physics, &map, at, corner), "{at:?} to {corner:?} goes through the wall");
        at = corner;
    }
    // Round one end: up to z = ±30 and back, a little over 2 * hypot(30, 12.5).
    let walked = length(from, &path);
    assert!(walked > 64.0 && walked < 75.0, "walked {walked} m: {path:?}");
    assert!(path.len() <= 4, "smoothed down to its corners: {path:?}");
}

#[test]
fn it_finds_the_gap_in_a_wall() {
    // Two walls with a 4 m gap between them at z = 0.
    let (map, physics) = world(&[[10, -40, 12, -2], [10, 2, 12, 40]]);
    let mut grid = NavGrid::default();
    let path = grid.find_path(&physics, &map, [0.0, 0.0, 10.0], [25.0, 0.0, 10.0], &mut plenty()).expect("through the gap");
    let walked = length([0.0, 10.0], &path);
    assert!(walked < 35.0, "through the gap, not round: {walked} m, {path:?}");
}

#[test]
fn a_closed_room_cannot_be_reached_and_the_search_gives_up() {
    // A 20 m room with 1 m walls and no door.
    let (map, physics) = world(&[[30, -10, 50, -9], [30, 9, 50, 10], [30, -10, 31, 10], [49, -10, 50, 10]]);
    let mut grid = NavGrid::default();
    assert!(grid.open(&physics, &map, cell_of([40.0, 0.0])), "the floor inside is open");
    // Once the grid there is built, which costs as well.
    grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [40.0, 0.0, 0.0], &mut plenty());
    let mut budget = MAX_SEARCH;
    assert_eq!(grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [40.0, 0.0, 0.0], &mut budget), None);
    assert!(MAX_SEARCH - budget <= 2 * ISLAND, "found out from the room being small: {} cells", MAX_SEARCH - budget);
    // From inside, out.
    assert_eq!(grid.find_path(&physics, &map, [40.0, 0.0, 0.0], [0.0, 0.0, 0.0], &mut plenty()), None);
}

#[test]
fn someone_against_a_wall_is_still_reached() {
    let (map, physics) = world(&[[10, -30, 12, 30]]);
    let mut grid = NavGrid::default();
    // Both ends in cells the wall touches.
    let path = grid.find_path(&physics, &map, [9.7, 0.0, 5.0], [12.3, 0.0, 5.0], &mut plenty()).expect("a way round");
    assert_eq!(path.last(), Some(&[12.3, 5.0]));
}

#[test]
fn forgetting_an_area_builds_its_chunks_again() {
    let (map, physics) = world(&[]);
    let mut grid = NavGrid::default();
    grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [40.0, 0.0, 0.0], &mut plenty());
    let built = grid.built();
    assert!(built > 1);
    grid.forget([0.0, -1.0, 5.0, 1.0]);
    assert!(grid.built() < built);
    assert!(grid.open(&physics, &map, cell_of([2.0, 0.0])));
}

#[test]
fn it_climbs_steps_onto_a_deck_but_not_onto_a_roof() {
    // Five 30 cm steps a metre deep up to a 1.5 m deck from x = 20 to 40; and a 3 m block beside it.
    let mut boxes: Vec<[i32; 5]> = (0..5).map(|n| [15 + n, -3, 16 + n, 3, 30 * (n + 1)]).collect();
    boxes.push([20, -3, 40, 3, 150]);
    boxes.push([20, 10, 30, 20, 300]);
    let (map, physics) = blocks(&boxes);
    let mut grid = NavGrid::default();
    let deck = grid.floor(&physics, &map, cell_of([30.0, 0.0])).expect("the deck is open");
    assert!((deck - 1.5).abs() < 0.05, "deck floor at {deck} m");
    assert!(grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [30.0, 1.5, 0.0], &mut plenty()).is_some(), "up the steps");
    // From the far side of the deck, the only way on is round to the steps.
    let path = grid.find_path(&physics, &map, [50.0, 0.0, 0.0], [30.0, 1.5, 0.0], &mut plenty()).expect("round to the steps");
    assert!(length([50.0, 0.0], &path) > 50.0, "{path:?}");
    grid.find_path(&physics, &map, [0.0, 0.0, 15.0], [25.0, 3.0, 15.0], &mut plenty());
    let mut budget = MAX_SEARCH;
    assert_eq!(grid.find_path(&physics, &map, [0.0, 0.0, 15.0], [25.0, 3.0, 15.0], &mut budget), None, "a roof is not walked onto");
    assert!(MAX_SEARCH - budget <= 2 * ISLAND, "{} cells", MAX_SEARCH - budget);
}

#[test]
fn a_search_takes_no_more_than_its_budget() {
    // A wall round three sides of the goal, open on the far side: a long way round.
    let (map, physics) = world(&[[10, -15, 12, 15], [10, -15, 40, -13], [10, 13, 40, 15]]);
    let mut grid = NavGrid::default();
    let mut budget = 50;
    assert_eq!(grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [30.0, 0.0, 0.0], &mut budget), None);
    assert_eq!(budget, 0);
    assert!(grid.find_path(&physics, &map, [0.0, 0.0, 0.0], [30.0, 0.0, 0.0], &mut plenty()).is_some());
}
