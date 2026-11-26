//! A fuel station's buildings and its drive-through wash (FUEL.md). Every station has the same
//! layout in its own frame (`FuelStation`): X runs along the road it faces, Z back from that road. A
//! paved lot, a tall round sign on the corner nearest the road, a canopy over a pump island, and
//! behind it a wash bay whose lane runs along X, open at both ends, with the lot clear past either end.
//!
//! The server and the client's physics stand colliders of `structure`, and the client draws them
//! (through the WASM's `stationParts`), so what is seen is what is hit. A car whose middle comes into
//! the lane (`Bay::lane`), facing along it, is driven through by the wash (`pull`), in the shared
//! physics so the driver's prediction agrees; the server heals what comes out at the far end.

use crate::consts::{
    STATION_CANOPY_AT, STATION_CANOPY_DEPTH, STATION_CANOPY_HEIGHT, STATION_CANOPY_SIZE, STATION_LOT, STATION_SIGN_AT, STATION_SIGN_HEIGHT, STATION_SIGN_RADIUS, WASH_BAY_HEIGHT, WASH_BAY_WIDTH, WASH_BAY_X,
    WASH_BAY_Z, WASH_LEAD, WASH_SPEED, WASH_STEER_AHEAD, WASH_TAKE_ANGLE, WASH_TAKE_ASIDE,
};
use crate::lifts::{Part, Shape};
use crate::world::FuelStation;

/// `Part::look` of a station's parts: white paint.
pub const PAINT_WHITE: u8 = 0;
/// Deep navy paint.
pub const PAINT_NAVY: u8 = 1;
/// Orange paint.
pub const PAINT_ORANGE: u8 = 2;
/// The lot's concrete.
pub const PAINT_CONCRETE: u8 = 3;
/// Bare steel.
pub const PAINT_STEEL: u8 = 4;

/// How thick the walls of the wash bay are, metres.
const BAY_WALL: f32 = 0.3;
/// How thick the bay's roof is, metres.
const BAY_ROOF: f32 = 0.4;
/// How deep the paving of the lot reaches under the ground, metres: enough that a lot on a slope
/// shows a kerb of concrete, not a gap.
const PAVING_DEPTH: f32 = 1.2;
/// How far the paving stands proud of the lot's level, metres.
const PAVING_PROUD: f32 = 0.05;
/// The pump island along the canopy's middle: its half length along X and half width, metres, and
/// how high its kerb stands.
const ISLAND: [f32; 3] = [6.2, 0.7, 0.18];
/// The pumps on the island, metres along X from the canopy's middle.
const PUMPS: [f32; 2] = [-2.5, 2.5];
/// A pump's half extents, metres.
const PUMP: [f32; 3] = [0.35, 0.8, 0.3];
/// The canopy's two pillars, metres along X from its middle, and their radius.
const PILLARS: [f32; 2] = [-5.0, 5.0];
const PILLAR_RADIUS: f32 = 0.22;
/// The radius of the sign's post, metres.
const SIGN_POST: f32 = 0.22;

/// A station's frame: where its own `(x, z)` lies in the world, metres, and its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub middle: [f32; 2],
    /// The turn about the vertical, radians (`FuelStation::yaw`).
    pub yaw: f32,
    pub base: f32,
}

impl Frame {
    pub fn of(station: &FuelStation) -> Self {
        Frame { middle: [station.at[0] as f32 / 100.0, station.at[1] as f32 / 100.0], yaw: f32::from(station.yaw) / 1000.0, base: station.base as f32 / 100.0 }
    }

    /// The station's +X (along the road) in the world, a unit vector across the ground.
    pub fn along(&self) -> [f32; 2] {
        [self.yaw.cos(), -self.yaw.sin()]
    }

    /// The station's +Z (back from the road) in the world.
    pub fn back(&self) -> [f32; 2] {
        [self.yaw.sin(), self.yaw.cos()]
    }

    /// The point `(x, z)` of the station's frame in the world, metres.
    pub fn world(&self, [x, z]: [f32; 2]) -> [f32; 2] {
        let (a, b) = (self.along(), self.back());
        [self.middle[0] + a[0] * x + b[0] * z, self.middle[1] + a[1] * x + b[1] * z]
    }

    /// The world's point `(x, z)` in the station's frame, metres.
    pub fn local(&self, [x, z]: [f32; 2]) -> [f32; 2] {
        let (a, b) = (self.along(), self.back());
        let (dx, dz) = (x - self.middle[0], z - self.middle[1]);
        [dx * a[0] + dz * a[1], dx * b[0] + dz * b[1]]
    }
}

/// The solid parts of `station`: its paving, the wash bay's walls and roof, the canopy, its
/// pillars, the pump island and pumps, and the sign's post. The sign itself, the doors and what is
/// only painted on are drawn by the client and not solid.
pub fn structure(station: &FuelStation) -> Vec<Part> {
    let frame = Frame::of(station);
    let b = frame.base;
    let boxed = |[x, z]: [f32; 2], y: f32, half: [f32; 3], look: u8| {
        let [wx, wz] = frame.world([x, z]);
        Part { shape: Shape::Box(half), centre: [wx, y, wz], turn: frame.yaw, look }
    };
    let post = |[x, z]: [f32; 2], from: f32, to: f32, radius: f32, look: u8| {
        let [wx, wz] = frame.world([x, z]);
        Part { shape: Shape::Post { radius, half: (to - from) / 2.0 }, centre: [wx, (from + to) / 2.0, wz], turn: frame.yaw, look }
    };
    let mut parts = Vec::new();
    // The paving, its top just proud of the lot's level.
    let paving = (PAVING_DEPTH + PAVING_PROUD) / 2.0;
    parts.push(boxed([0.0, 0.0], b + PAVING_PROUD - paving, [STATION_LOT[0] / 2.0, paving, STATION_LOT[1] / 2.0], PAINT_CONCRETE));
    let floor = b + PAVING_PROUD;
    // The wash bay: two long walls and a roof over the clear height, open at both ends.
    let [from, to] = WASH_BAY_X;
    let (mid, half) = ((from + to) / 2.0, (to - from) / 2.0);
    let top = WASH_BAY_HEIGHT + BAY_ROOF;
    for side in [-1.0, 1.0] {
        let z = WASH_BAY_Z + side * (WASH_BAY_WIDTH + BAY_WALL) / 2.0;
        parts.push(boxed([mid, z], floor + top / 2.0, [half, top / 2.0, BAY_WALL / 2.0], PAINT_WHITE));
    }
    parts.push(boxed([mid, WASH_BAY_Z], floor + WASH_BAY_HEIGHT + BAY_ROOF / 2.0, [half, BAY_ROOF / 2.0, WASH_BAY_WIDTH / 2.0 + BAY_WALL], PAINT_WHITE));
    // The canopy's roof on its two pillars, which stand on the pump island under its middle.
    let [cx, cz] = STATION_CANOPY_AT;
    parts.push(boxed(STATION_CANOPY_AT, floor + STATION_CANOPY_HEIGHT + STATION_CANOPY_DEPTH / 2.0, [STATION_CANOPY_SIZE[0] / 2.0, STATION_CANOPY_DEPTH / 2.0, STATION_CANOPY_SIZE[1] / 2.0], PAINT_WHITE));
    for x in PILLARS {
        parts.push(post([cx + x, cz], floor, floor + STATION_CANOPY_HEIGHT, PILLAR_RADIUS, PAINT_WHITE));
    }
    parts.push(boxed(STATION_CANOPY_AT, floor + ISLAND[2] / 2.0, [ISLAND[0], ISLAND[2] / 2.0, ISLAND[1]], PAINT_CONCRETE));
    for x in PUMPS {
        parts.push(boxed([cx + x, cz], floor + ISLAND[2] + PUMP[1], PUMP, PAINT_ORANGE));
    }
    // The sign's post, up to the bottom of the round sign.
    parts.push(post(STATION_SIGN_AT, floor, floor + STATION_SIGN_HEIGHT - STATION_SIGN_RADIUS, SIGN_POST, PAINT_STEEL));
    parts
}

/// A station's wash lane in the world: the middle of the bay, the way the lane runs (either way
/// along it is through), and its size, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bay {
    pub middle: [f32; 2],
    /// A unit vector along the lane, the station's +X.
    pub along: [f32; 2],
    /// Half the length between the doors, metres.
    pub half_length: f32,
    /// The height of the bay's floor, metres.
    pub floor: f32,
}

impl Bay {
    pub fn of(station: &FuelStation) -> Self {
        let frame = Frame::of(station);
        let [from, to] = WASH_BAY_X;
        Bay { middle: frame.world([(from + to) / 2.0, WASH_BAY_Z]), along: frame.along(), half_length: (to - from) / 2.0, floor: frame.base + PAVING_PROUD }
    }

    /// The world's point `(x, z)` as `[along, aside]` of the bay's middle, metres: along the lane
    /// (+ towards the station's +X door) and to the side of it.
    pub fn local(&self, [x, z]: [f32; 2]) -> [f32; 2] {
        let (dx, dz) = (x - self.middle[0], z - self.middle[1]);
        [dx * self.along[0] + dz * self.along[1], dz * self.along[0] - dx * self.along[1]]
    }

    /// `[along, aside]` of `(x, z)` while it is in the lane, from `WASH_LEAD` before one door to as far
    /// past the other, within `WASH_TAKE_ASIDE` of its middle: where the wash takes a car.
    pub fn lane(&self, at: [f32; 2]) -> Option<[f32; 2]> {
        let [along, aside] = self.local(at);
        (along.abs() <= self.half_length + WASH_LEAD && aside.abs() <= WASH_TAKE_ASIDE).then_some([along, aside])
    }

    /// Whether `(x, z)` is between the doors, within the bay's walls.
    pub fn inside(&self, at: [f32; 2]) -> bool {
        let [along, aside] = self.local(at);
        along.abs() <= self.half_length && aside.abs() <= WASH_BAY_WIDTH / 2.0
    }
}

/// What the wash asks of a car in its lane, `[throttle, steer]` as `CarControls` has them, or `None`
/// when the car is not the wash's: its middle `at` out of the lane (`Bay::lane`), or it heads more than
/// `WASH_TAKE_ANGLE` off the lane's line. It drives the car on the way it faces, along the middle of
/// the lane at `WASH_SPEED`. `yaw` is the car's (0 faces -Z), `speed` how fast it goes ahead (m/s),
/// `wheelbase` and `steer_lock` its spec's.
pub fn pull(bay: &Bay, at: [f32; 2], yaw: f32, speed: f32, wheelbase: f32, steer_lock: f32) -> Option<[f32; 2]> {
    let [along, _] = bay.lane(at)?;
    let ahead = [-yaw.sin(), -yaw.cos()];
    let facing = ahead[0] * bay.along[0] + ahead[1] * bay.along[1];
    if facing.abs() < WASH_TAKE_ANGLE.cos() {
        return None;
    }
    // A point on the lane's middle a little ahead, steered for as traffic steers (`Traffic::drive`).
    let goal = along + facing.signum() * WASH_STEER_AHEAD;
    let goal = [bay.middle[0] + bay.along[0] * goal, bay.middle[1] + bay.along[1] * goal];
    let (dx, dz) = (goal[0] - at[0], goal[1] - at[1]);
    let reach = dx.hypot(dz).max(1.0);
    let bearing = wrap((-dx).atan2(-dz) - yaw);
    let lock = (2.0 * wheelbase.max(0.5) * bearing.sin() / reach).atan();
    let steer = (-lock / steer_lock.max(0.1)).clamp(-1.0, 1.0);
    // On for more speed, against the way it rolls for less.
    let over = speed - WASH_SPEED;
    let throttle = if over < 0.0 { (-over * 0.6).min(1.0) } else { (-over * 0.5).max(-1.0) };
    Some([throttle, steer])
}

/// `angle` wrapped into -π..π.
fn wrap(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A station at `(100, 50)` turned a quarter round, so its +X runs along -Z of the world.
    fn station() -> FuelStation {
        FuelStation { at: [10_000, 5_000], yaw: 1_571, base: 300 }
    }

    #[test]
    fn a_stations_frame_turns_its_own_axes_into_the_world_and_back() {
        let frame = Frame::of(&station());
        let [x, z] = frame.world([10.0, 0.0]);
        assert!((x - 100.0).abs() < 0.01 && (z - 40.0).abs() < 0.01, "+X runs along -Z: {x}, {z}");
        let [x, z] = frame.world([0.0, 10.0]);
        assert!((x - 110.0).abs() < 0.01 && (z - 50.0).abs() < 0.01, "+Z runs along +X: {x}, {z}");
        let back = frame.local(frame.world([3.0, -7.0]));
        assert!((back[0] - 3.0).abs() < 1e-3 && (back[1] + 7.0).abs() < 1e-3, "{back:?}");
    }

    #[test]
    fn the_lane_reaches_out_in_front_of_both_doors_and_the_bay_is_between_them() {
        let bay = Bay::of(&station());
        let frame = Frame::of(&station());
        let door = |x: f32, aside: f32| frame.world([x, WASH_BAY_Z + aside]);
        assert!(bay.inside(door((WASH_BAY_X[0] + WASH_BAY_X[1]) / 2.0, 0.0)));
        assert!(!bay.inside(door(WASH_BAY_X[0] - 1.0, 0.0)), "out of the door");
        assert!(bay.lane(door(WASH_BAY_X[0] - 1.0, 0.0)).is_some(), "but in the lead-in");
        assert!(bay.lane(door(WASH_BAY_X[1] + WASH_LEAD - 0.5, 1.5)).is_some());
        assert!(bay.lane(door(WASH_BAY_X[1] + WASH_LEAD + 0.5, 0.0)).is_none(), "past the lead-out");
        assert!(bay.lane(door(0.0, WASH_TAKE_ASIDE + 0.5)).is_none(), "beside the lane");
        assert!((bay.floor - 3.05).abs() < 1e-4);
    }

    #[test]
    fn the_wash_drives_a_car_facing_along_the_lane_at_a_walking_pace_and_leaves_one_across_it() {
        let bay = Bay::of(&station());
        let frame = Frame::of(&station());
        let at = frame.world([WASH_BAY_X[0] - 2.0, WASH_BAY_Z]);
        // The lane runs along -Z of the world, which a yaw of 0 faces.
        let [throttle, steer] = pull(&bay, at, 0.0, 0.0, 2.6, 0.6).expect("taken");
        assert!(throttle > 0.9 && steer.abs() < 0.05, "standing, it is driven on straight: {throttle} {steer}");
        let [throttle, _] = pull(&bay, at, 0.0, 15.0, 2.6, 0.6).unwrap();
        assert!(throttle <= -1.0 + 1e-6, "fast, it is braked: {throttle}");
        let [throttle, _] = pull(&bay, at, 0.0, WASH_SPEED, 2.6, 0.6).unwrap();
        assert!(throttle.abs() < 1e-6, "at the pace, held there");
        // Facing the other way it is driven through the other way, still ahead.
        let [throttle, _] = pull(&bay, at, std::f32::consts::PI, 0.0, 2.6, 0.6).unwrap();
        assert!(throttle > 0.9);
        assert!(pull(&bay, at, std::f32::consts::FRAC_PI_2, 0.0, 2.6, 0.6).is_none(), "across the lane");
        // Left of the middle (a +X aside of the world, facing -Z, is to the right), it steers back.
        let right = frame.world([WASH_BAY_X[0] - 2.0, WASH_BAY_Z + 1.0]);
        let [_, steer] = pull(&bay, right, 0.0, 0.0, 2.6, 0.6).unwrap();
        assert!(steer < -0.1, "a car right of the middle steers left: {steer}");
    }

    #[test]
    fn a_station_stands_its_paving_bay_canopy_pumps_and_sign_on_its_base() {
        let parts = structure(&station());
        assert_eq!(parts.len(), 1 + 3 + 1 + PILLARS.len() + 1 + PUMPS.len() + 1);
        let paving = parts[0];
        let Shape::Box(half) = paving.shape else { panic!("paving is a box") };
        assert!((paving.centre[1] + half[1] - 3.05).abs() < 1e-4, "its top just proud of the base");
        assert!(parts.iter().all(|part| (part.turn - 1.571).abs() < 1e-6));
        // The bay's roof leaves the clear height under it.
        let roof = parts[3];
        let Shape::Box(half) = roof.shape else { panic!() };
        assert!((roof.centre[1] - half[1] - (3.05 + WASH_BAY_HEIGHT)).abs() < 1e-4);
    }
}
