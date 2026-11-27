//! A vehicle dealer's carport (DEALERS.md): a flat roof on steel posts against an OSM store's wall,
//! open on the side facing the store's parking. Every carport has the same layout in its own frame
//! (`Dealer`): X runs across its mouth, Z out of the mouth onto the lot, and the carport fills
//! `-DEALER_PORT[1]..0` along Z, its back against the store's wall.
//!
//! A soldier on foot within `DEALER_REACH` of the mouth orders a vehicle on their phone; the server
//! stands it under the roof facing out (`port`), or sets a helicopter down on the lot in front
//! (`heli_spot`). The server and the client's physics stand colliders of `structure`, and the client
//! draws them (through the WASM's `dealerParts`), so what is seen is what is hit.

use crate::consts::{DEALER_HELI_OUT, DEALER_PORT, DEALER_PORT_HEIGHT, DEALER_REACH, DEALER_RESALE_DIVISOR, DEALER_SELL_REACH};
use crate::lifts::{Part, Shape};
use crate::station::{Frame, PAINT_CONCRETE, PAINT_STEEL, PAINT_WHITE};
use crate::world::Dealer;

/// The radius of the carport's posts, metres.
const POST_RADIUS: f32 = 0.16;
/// How thick the carport's roof is, metres.
const ROOF: f32 = 0.35;
/// How far the roof reaches past the posts at the sides and over the mouth, metres.
const EAVES: f32 = 0.4;
/// How deep the carport's floor reaches under the ground, metres, so one on a slope shows a kerb.
const FLOOR_DEPTH: f32 = 1.2;
/// How far the floor stands proud of the carport's level, metres.
const FLOOR_PROUD: f32 = 0.05;
/// How far in from the wall the back posts stand, metres.
const BACK_POSTS: f32 = 0.3;

/// The carport's frame (`shared::station::Frame`): its +X across the mouth, +Z out onto the lot.
pub fn frame(dealer: &Dealer) -> Frame {
    Frame { middle: [dealer.at[0] as f32 / 100.0, dealer.at[1] as f32 / 100.0], yaw: f32::from(dealer.yaw) / 1000.0, base: dealer.base as f32 / 100.0 }
}

/// The height of the carport's floor, metres.
pub fn floor(dealer: &Dealer) -> f32 {
    dealer.base as f32 / 100.0 + FLOOR_PROUD
}

/// The solid parts of `dealer`'s carport: its floor, the posts down both sides and the roof. The sign
/// on the roof's edge and the roller door in the store's wall are only drawn.
pub fn structure(dealer: &Dealer) -> Vec<Part> {
    let frame = frame(dealer);
    let [width, depth] = DEALER_PORT;
    let boxed = |[x, z]: [f32; 2], y: f32, half: [f32; 3], look: u8| {
        let [wx, wz] = frame.world([x, z]);
        Part { shape: Shape::Box(half), centre: [wx, y, wz], turn: frame.yaw, look }
    };
    let floor = floor(dealer);
    let mut parts = Vec::new();
    let slab = (FLOOR_DEPTH + FLOOR_PROUD) / 2.0;
    parts.push(boxed([0.0, -depth / 2.0], floor - slab, [width / 2.0 + EAVES, slab, depth / 2.0], PAINT_CONCRETE));
    // Three posts down each side, outside the clear width: at the mouth, halfway, and by the wall.
    let side = width / 2.0 + POST_RADIUS;
    for x in [-side, side] {
        for z in [0.0, -depth / 2.0, -depth + BACK_POSTS] {
            let [wx, wz] = frame.world([x, z]);
            parts.push(Part { shape: Shape::Post { radius: POST_RADIUS, half: DEALER_PORT_HEIGHT / 2.0 }, centre: [wx, floor + DEALER_PORT_HEIGHT / 2.0, wz], turn: frame.yaw, look: PAINT_STEEL });
        }
    }
    let reach = (depth + EAVES) / 2.0;
    parts.push(boxed([0.0, -depth + reach], floor + DEALER_PORT_HEIGHT + ROOF / 2.0, [side + EAVES, ROOF / 2.0, reach], PAINT_WHITE));
    parts
}

/// Where an ordered vehicle stands: the middle of the carport's floor across the ground, metres, and
/// the yaw that faces it out of the mouth (0 faces -Z, as a vehicle's yaw does).
pub fn port(dealer: &Dealer) -> ([f32; 2], f32) {
    let frame = frame(dealer);
    (frame.world([0.0, -DEALER_PORT[1] / 2.0]), facing_out(&frame))
}

/// Where an ordered helicopter is set down: on the lot `DEALER_HELI_OUT` out from the mouth, facing
/// away from the carport, metres and radians as `port` has them.
pub fn heli_spot(dealer: &Dealer) -> ([f32; 2], f32) {
    let frame = frame(dealer);
    (frame.world([0.0, DEALER_HELI_OUT]), facing_out(&frame))
}

/// The yaw of a vehicle facing along the frame's +Z: a vehicle's ahead is `(-sin, -cos)` of its yaw,
/// the frame's +Z `(sin, cos)` of its own.
fn facing_out(frame: &Frame) -> f32 {
    wrap(frame.yaw + std::f32::consts::PI)
}

/// The index of the dealer in `dealers` whose carport's mouth `(x, z)` is within `DEALER_REACH` of,
/// the nearest when there are two.
pub fn at_dealer(dealers: &[Dealer], [x, z]: [f32; 2]) -> Option<usize> {
    dealers
        .iter()
        .enumerate()
        .map(|(i, dealer)| (i, (dealer.at[0] as f32 / 100.0 - x).hypot(dealer.at[1] as f32 / 100.0 - z)))
        .filter(|&(_, distance)| distance <= DEALER_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Whether a vehicle whose middle is at `at` (metres), turned `yaw` (0 faces -Z), `half_length` metres
/// from its middle to its nose, has been driven in through `dealer`'s door: it is under the carport,
/// heading for the store's wall within 45°, with its nose within `DEALER_SELL_REACH` of the door.
pub fn driven_in(dealer: &Dealer, at: [f32; 2], yaw: f32, half_length: f32) -> bool {
    let frame = frame(dealer);
    let [x, z] = frame.local(at);
    let ahead = [-yaw.sin(), -yaw.cos()];
    let back = frame.back();
    // How far the vehicle heads into the carport, towards the wall (its -Z).
    let inwards = -(ahead[0] * back[0] + ahead[1] * back[1]);
    x.abs() <= DEALER_PORT[0] / 2.0 && z >= -DEALER_PORT[1] && z <= -DEALER_PORT[1] + half_length + DEALER_SELL_REACH && inwards >= std::f32::consts::FRAC_1_SQRT_2
}

/// What a dealer pays for a vehicle of `kind` with `health` of its health left (0..1): its price over
/// `DEALER_RESALE_DIVISOR`, by its health. `None` for a kind a dealer does not sell, nor buy.
pub fn resale(kind: u8, health: f32) -> Option<u32> {
    price(kind).map(|price| (price as f32 / DEALER_RESALE_DIVISOR as f32 * health.clamp(0.0, 1.0)).round() as u32)
}

/// What dealer kind `kind` costs (`DEALER_PRICES`), or `None` where a dealer does not sell it.
pub fn price(kind: u8) -> Option<u32> {
    use crate::consts::{DEALER_KINDS, DEALER_PRICES};
    DEALER_KINDS.iter().position(|&k| k == kind).map(|i| DEALER_PRICES[i])
}

/// `angle` wrapped into -π..π.
fn wrap(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{DEALER_KINDS, DEALER_PRICES, KIND_PLANE, KIND_SEAPLANE, KIND_TITAN, KIND_VOLVO_240};
    use crate::vehicles::{vehicle_def, Lift};

    /// A carport at `(100, 50)` turned a quarter round, so its mouth faces +X of the world.
    fn dealer() -> Dealer {
        Dealer { at: [10_000, 5_000], yaw: 1_571, base: 300 }
    }

    #[test]
    fn an_ordered_vehicle_stands_under_the_roof_facing_out_of_the_mouth() {
        let ([x, z], yaw) = port(&dealer());
        assert!((x - (100.0 - DEALER_PORT[1] / 2.0)).abs() < 0.01 && (z - 50.0).abs() < 0.01, "back from the mouth: {x}, {z}");
        // A vehicle's ahead is (-sin, -cos) of its yaw: out along +X.
        assert!((-yaw.sin() - 1.0).abs() < 1e-3 && yaw.cos().abs() < 1e-3, "{yaw}");
        let ([x, _], heli) = heli_spot(&dealer());
        assert!((x - (100.0 + DEALER_HELI_OUT)).abs() < 0.01, "a helicopter out on the lot: {x}");
        assert!((heli - yaw).abs() < 1e-6);
    }

    #[test]
    fn a_dealer_is_reached_from_near_its_mouth_and_the_nearest_wins() {
        let near = Dealer { at: [10_000 + 3_000, 5_000], ..dealer() };
        let dealers = [dealer(), near];
        assert_eq!(at_dealer(&dealers, [105.0, 50.0]), Some(0));
        assert_eq!(at_dealer(&dealers, [128.0, 50.0]), Some(1));
        assert_eq!(at_dealer(&dealers, [100.0, 50.0 + DEALER_REACH + 1.0]), None, "too far");
    }

    #[test]
    fn a_vehicle_is_driven_in_when_its_nose_reaches_the_door_heading_for_it() {
        let dealer = Dealer { at: [0, 0], yaw: 0, base: 0 };
        // The door is at z = -14; a car facing -Z (yaw 0) heads for it.
        let half = 2.4;
        let at_door = [0.0, -DEALER_PORT[1] + half + 0.5];
        assert!(driven_in(&dealer, at_door, 0.0, half));
        assert!(!driven_in(&dealer, [0.0, -DEALER_PORT[1] + half + 2.0], 0.0, half), "short of the door");
        assert!(!driven_in(&dealer, at_door, std::f32::consts::PI, half), "backed in, its nose out");
        assert!(!driven_in(&dealer, [DEALER_PORT[0], at_door[1]], 0.0, half), "beside the carport");
        let ([x, z], yaw) = port(&dealer);
        assert!(!driven_in(&dealer, [x, z], yaw, 6.0), "a bus just bought, facing out, is not sold back");
    }

    #[test]
    fn a_dealer_buys_back_at_a_sixth_of_the_price_less_for_damage() {
        let new = price(KIND_VOLVO_240).unwrap();
        assert_eq!(resale(KIND_VOLVO_240, 1.0), Some((new as f32 / 6.0).round() as u32));
        assert_eq!(resale(KIND_VOLVO_240, 0.5), Some((new as f32 / 12.0).round() as u32));
        assert_eq!(resale(KIND_TITAN, 1.0), None);
    }

    #[test]
    fn the_carports_roof_leaves_its_clear_height_over_the_floor() {
        let parts = structure(&dealer());
        assert_eq!(parts.len(), 1 + 6 + 1);
        let roof = parts[7];
        let Shape::Box(half) = roof.shape else { panic!("the roof is a box") };
        assert!((roof.centre[1] - half[1] - (3.05 + DEALER_PORT_HEIGHT)).abs() < 1e-4);
        let Shape::Box(half) = parts[0].shape else { panic!() };
        assert!((parts[0].centre[1] + half[1] - 3.05).abs() < 1e-4, "the floor's top at the floor");
    }

    #[test]
    fn a_dealer_sells_no_planes_walkers_boats_or_the_titan_and_prices_everything_it_sells() {
        assert_eq!(DEALER_KINDS.len(), DEALER_PRICES.len());
        for &kind in &DEALER_KINDS {
            let def = vehicle_def(kind);
            assert_eq!(def.kind, kind, "kind {kind} is a vehicle");
            assert!(!matches!(def.spec.lift, Lift::Wing | Lift::Walker | Lift::Drone | Lift::Loiter | Lift::Missile | Lift::Mounted) && !def.spec.lift.afloat(), "{} is not sold", def.name);
            assert!(def.drone.is_none() && def.spec.airframe.is_none(), "{}", def.name);
            assert!(def.spec.half_extents[0] * 2.0 < DEALER_PORT[0] && def.spec.half_extents[2] * 2.0 < DEALER_PORT[1], "{} fits the carport", def.name);
        }
        for kind in [KIND_PLANE, KIND_SEAPLANE, KIND_TITAN] {
            assert_eq!(price(kind), None);
        }
        assert!(price(KIND_VOLVO_240).is_some_and(|price| price > 0));
    }
}
