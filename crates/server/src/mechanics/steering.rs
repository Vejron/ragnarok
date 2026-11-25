//! Driving round what is in the way, for a vehicle steered by the server off the road: a police car
//! after someone across a field, a tank on its route (EMERGENCY.md, BOTS.md).
//!
//! It feels ahead with a fan of rays at the height of its bumper, each as wide as the vehicle, and
//! takes the way nearest the one it wants that is clear for as far as it needs to stop. Water deeper
//! than it can wade and a bank too steep to climb count as in the way too. A way round a building is
//! found by the soldiers' grid (`navigation`), which knows nothing of how wide the vehicle is: now
//! and then it scrapes a corner, and backs out (its driver's own rule) when it sticks.

use crate::game::World;
use shared::vehicles::vehicle_def;
use sim::CarPose;

/// The headings tried, radians off the one it wants, the nearest first.
const FAN: [f32; 7] = [0.0, 0.35, -0.35, 0.7, -0.7, 1.05, -1.05];
/// How far ahead it feels, metres: this, and this much more for every m/s it does, up to `REACH`.
const LOOK: f32 = 6.0;
const LOOK_PER_SPEED: f32 = 1.5;
const REACH: f32 = 40.0;
/// How far over the middle of the vehicle its rays go, metres, and how far outside its sides.
const BUMPER: f32 = 0.3;
const ROOM: f32 = 0.6;
/// How far apart the points it looks at water and banks are along a ray, metres; water deeper than
/// `WADE` over the ground there, and ground that rises more than `STEEP` between two of them, are in the way.
const STEP: f32 = 4.0;
const WADE: f32 = 0.6;
const STEEP: f32 = 1.6;
/// How much a metre of clear road is worth against a radian off the way it wants, and how much the
/// side it went round last time is worth: so that it does not dither between two.
const PER_RADIAN: f32 = 8.0;
const KEEP_SIDE: f32 = 3.0;
/// How hard it plans to brake for what is ahead, m/s², and how far short of it it means to stop, metres.
const BRAKE: f32 = 4.0;
const SHORT: f32 = 2.0;
/// The slowest it takes a turn of `SHARP` radians or more off the way it points, m/s.
const SHARP: f32 = 1.0;
const TURNING: f32 = 5.0;

/// The fastest it may go with `turn` radians to turn yet, m/s, if it goes `top` straight on.
pub(crate) fn turning(turn: f32, top: f32) -> f32 {
    let t = (turn.abs() / SHARP).min(1.0);
    top + (TURNING - top) * t
}

/// How far along the heading `yaw` from `pose` a vehicle of `kind` can go before it meets something,
/// water or a bank, up to `reach` metres; `own` is the vehicle itself.
pub(crate) fn clear(world: &World, own: u32, kind: u8, pose: &CarPose, yaw: f32, reach: f32) -> f32 {
    let [hx, _, hz] = vehicle_def(kind).spec.half_extents;
    let (sin, cos) = yaw.sin_cos();
    let dir = [-sin, 0.0, -cos];
    let side = [cos, 0.0, -sin];
    let p = pose.position;
    let mut free = reach;
    // Down its middle and along both its sides, with a little room.
    for offset in [0.0, hx + ROOM, -hx - ROOM] {
        let from = [p[0] + side[0] * offset, p[1] + BUMPER, p[2] + side[2] * offset];
        if let Some(hit) = world.physics.cast_ray(from, dir, reach, own) {
            free = free.min(hit);
        }
    }
    // Water too deep and banks too steep, every `STEP` along the middle.
    let ground = |x: f32, z: f32| world.map.ground.height_at(x, z);
    let mut last = ground(p[0], p[2]);
    let mut along = hz.max(STEP);
    while along < free {
        let (x, z) = (p[0] + dir[0] * along, p[2] + dir[2] * along);
        let here = ground(x, z);
        let deep = world.physics.water_level(x, z).is_some_and(|level| level - here > WADE);
        if deep || here - last > STEEP {
            return (along - STEP).max(0.0);
        }
        last = here;
        along += STEP;
    }
    free
}

/// Which way vehicle `vid` of `kind`, at `pose` doing `speed` m/s, should head when it wants to go
/// along `want` (radians), and the fastest it may go to stop short of what is ahead that way, m/s.
/// `lean` is the side it went round last (+1, -1 or 0), kept for next time.
pub(crate) fn avoid(world: &World, vid: u32, kind: u8, pose: &CarPose, want: f32, speed: f32, lean: &mut f32) -> (f32, f32) {
    let hz = vehicle_def(kind).spec.half_extents[2];
    let reach = (hz + LOOK + LOOK_PER_SPEED * speed.abs()).min(REACH);
    let stopping = |free: f32| (2.0 * BRAKE * (free - hz - SHORT).max(0.0)).sqrt();
    let straight = clear(world, vid, kind, pose, want, reach);
    if straight >= reach - 0.01 {
        *lean = 0.0;
        return (want, f32::MAX);
    }
    let (mut best, mut best_free, mut best_score) = (want, straight, f32::MIN);
    for off in FAN {
        let free = if off == 0.0 { straight } else { clear(world, vid, kind, pose, want + off, reach) };
        let side = off.signum();
        let score = free - PER_RADIAN * off.abs() + if off != 0.0 && side == *lean { KEEP_SIDE } else { 0.0 };
        if score > best_score {
            (best, best_free, best_score) = (want + off, free, score);
        }
    }
    let off = best - want;
    if off != 0.0 {
        *lean = off.signum();
    }
    (best, stopping(best_free))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use shared::consts::KIND_VOLVO_240;
    use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};

    /// A flat world with a house 12 m square at x = -6..6, z = -26..-14.
    fn game() -> Game {
        let v = |x: i32, y: i32, z: i32| [x * 100, y * 100, z * 100];
        let vertices = vec![v(-6, 0, -26), v(6, 0, -26), v(6, 0, -14), v(-6, 0, -14), v(-6, 6, -26), v(6, 6, -26), v(6, 6, -14), v(-6, 6, -14)];
        let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[-8_000, -8_000]], collision: CollisionMesh { vertices, triangles }, ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    #[test]
    fn a_car_heading_for_a_house_turns_off_to_one_side_and_slows_and_one_with_a_clear_way_does_neither() {
        let mut game = game();
        let vid = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_VOLVO_240, [0.0, 0.7, 0.0], 0.0);
        for _ in 0..30 {
            game.step();
        }
        let pose = game.world.physics.car_pose(vid).unwrap();
        let mut lean = 0.0;
        // Straight at the house, 14 m ahead, at 10 m/s.
        let (yaw, cap) = avoid(&game.world, vid, KIND_VOLVO_240, &pose, 0.0, 10.0, &mut lean);
        assert!(yaw.abs() > 0.3, "it turns off: {yaw}");
        assert!(cap < 15.0, "and slows for it: {cap}");
        assert_ne!(lean, 0.0);
        // The same side again next time, though the other is as good.
        let (again, _) = avoid(&game.world, vid, KIND_VOLVO_240, &pose, 0.0, 10.0, &mut lean);
        assert_eq!(again.signum(), yaw.signum());
        // Away from it, nothing is in the way.
        let (yaw, cap) = avoid(&game.world, vid, KIND_VOLVO_240, &pose, std::f32::consts::PI, 10.0, &mut lean);
        assert_eq!((yaw, cap), (std::f32::consts::PI, f32::MAX));
    }

    #[test]
    fn a_car_heading_for_a_lake_too_deep_to_wade_turns_off_and_one_beside_it_drives_on() {
        // Flat ground in 5 m cells, with a pond 10 m deep from 15 to 40 m ahead (-Z) and 20 m across.
        let side = 81u32;
        let wet = |i: u32, j: u32| {
            let (x, z) = (i as f32 * 5.0 - 200.0, j as f32 * 5.0 - 200.0);
            x.abs() <= 10.0 && (-40.0..=-15.0).contains(&z)
        };
        let (mut heights, mut water) = (Vec::new(), Vec::new());
        for j in 0..side {
            for i in 0..side {
                heights.push(if wet(i, j) { -1000 } else { 0 });
                water.push(if wet(i, j) { 0 } else { shared::world::NO_WATER });
            }
        }
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, ground: shared::terrain::Ground { origin: -20_000, cell: 500, side, heights }, water, spawns: vec![[-15_000, 0]], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        let vid = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_VOLVO_240, [0.0, 0.7, 0.0], 0.0);
        for _ in 0..30 {
            game.step();
        }
        let pose = game.world.physics.car_pose(vid).unwrap();
        let mut lean = 0.0;
        let (yaw, cap) = avoid(&game.world, vid, KIND_VOLVO_240, &pose, 0.0, 15.0, &mut lean);
        assert!(yaw.abs() > 0.3, "it turns off the water: {yaw}");
        assert!(cap < 20.0, "{cap}");
        // Along the shore, away from it, the way is clear.
        let (yaw, cap) = avoid(&game.world, vid, KIND_VOLVO_240, &pose, std::f32::consts::FRAC_PI_2, 15.0, &mut lean);
        assert_eq!((yaw, cap), (std::f32::consts::FRAC_PI_2, f32::MAX));
    }
}
