//! Shelters (`BUILDING_SHELTER`): the lean-tos (vindskydd) that stand along trails, by ski slopes
//! and at fire places, mapped as `amenity=shelter` with `shelter_type=lean_to`. A lean-to has three
//! timber walls and no fourth: the side `direction` names is open, and its roof is high over that
//! side and falls towards the back, so the laser's few returns over a footprint 2.5 m across are not
//! asked for its shape.

use crate::osm::Tags;
use crate::roof::{Roof, Shape, Walls};

type P = [f64; 2];

/// How high the roof stands over the floor along the open side, metres.
pub const FRONT: f64 = 2.2;
/// How steeply the roof falls towards the back, metres of fall a metre (about 22°).
pub const PITCH: f64 = 0.4;
/// The lowest the roof comes over the floor at the back, metres: a deep shelter is pitched less.
pub const BACK: f64 = 1.0;
/// The shortest side that may be the open one, metres: not a sliver where the outline bends.
const SHORTEST_OPEN: f64 = 1.0;

/// Whether a building's tags make it a lean-to: `amenity=shelter` of the kind that stands in the
/// woods (`lean_to`, `weather_shelter`, or none said), not a bus stop's, a picnic table's roof, a
/// gazebo or a hut with a door.
pub fn is_lean_to(tags: &Tags) -> bool {
    tags.get("amenity").is_some_and(|amenity| amenity == "shelter") && tags.get("shelter_type").is_none_or(|kind| matches!(kind.as_str(), "lean_to" | "weather_shelter"))
}

/// The way a shelter's open side faces, a unit `[x, z]` (+x east, +z south), from its `direction`:
/// a compass point (`w`, `sse`, `west`) or degrees clockwise from north. `None` without one.
pub fn facing(tags: &Tags) -> Option<P> {
    let direction = tags.get("direction")?.trim().to_lowercase();
    let degrees = match direction.parse::<f64>() {
        Ok(degrees) => degrees,
        Err(_) => {
            let points = ["n", "nne", "ne", "ene", "e", "ese", "se", "sse", "s", "ssw", "sw", "wsw", "w", "wnw", "nw", "nnw"];
            let short = match direction.as_str() {
                "north" => "n",
                "east" => "e",
                "south" => "s",
                "west" => "w",
                other => other,
            };
            points.iter().position(|&point| point == short)? as f64 * 22.5
        }
    };
    let radians = degrees.to_radians();
    Some([radians.sin(), -radians.cos()])
}

/// A shelter's `[wall, roof]` colours, `0xRRGGBB`, from its `building:colour` and `roof:colour`:
/// red is falu red and brown tarred timber, as they are in the woods. `None` where neither is said;
/// where only one is, the other is a shelter's own: dark timber, black felt.
pub fn paint(tags: &Tags) -> Option<[u32; 2]> {
    let walls = tags.get("building:colour").and_then(|colour| colour_of(colour));
    let roof = tags.get("roof:colour").and_then(|colour| colour_of(colour));
    (walls.is_some() || roof.is_some()).then(|| [walls.unwrap_or(0x4a3828), roof.unwrap_or(0x2e2c2a)])
}

/// A mapped colour as a shelter is painted: a name or `#rrggbb`.
fn colour_of(colour: &str) -> Option<u32> {
    let colour = colour.trim().to_lowercase();
    if let Some(hex) = colour.strip_prefix('#') {
        return match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok(),
            3 => u32::from_str_radix(hex, 16).ok().map(|v| ((v >> 8) & 0xf) * 0x110000 + ((v >> 4) & 0xf) * 0x1100 + (v & 0xf) * 0x11),
            _ => None,
        };
    }
    Some(match colour.as_str() {
        "red" | "darkred" | "maroon" => 0x7c2b1e,
        "brown" | "saddlebrown" | "sienna" => 0x5a3d28,
        "black" => 0x2a2826,
        "grey" | "gray" | "darkgrey" | "darkgray" | "silver" => 0x77756f,
        "white" => 0xe6e2d8,
        "green" | "darkgreen" => 0x3e5a36,
        "yellow" | "ochre" => 0xc49a45,
        "beige" | "tan" => 0xb8a07c,
        _ => return None,
    })
}

/// A shelter on `footprint` (counter-clockwise or not, local metres) with its floor at `floor`,
/// open towards `facing` (or on its longest side where `facing` is zero): its roof, its walls with the open side left out, and the way the open
/// side really faces (the outward normal of the side that was opened), for `Building::front`.
///
/// The open side is the one of at least `SHORTEST_OPEN` that faces `facing` most squarely. The roof
/// is one plane, `FRONT` over the floor along it and falling at `PITCH` to the back, pitched less
/// where the back would come lower than `BACK`.
pub fn build(footprint: &[P], floor: f64, facing: P) -> (Roof, Walls, P) {
    let n = footprint.len();
    let centre = footprint.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n as f64, sum[1] + p[1] / n as f64]);
    let sides: Vec<(P, P, P)> = (0..n)
        .filter_map(|k| {
            let (a, b) = (footprint[k], footprint[(k + 1) % n]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dz);
            if length < 1e-6 {
                return None;
            }
            let mut normal = [dz / length, -dx / length];
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            if normal[0] * (mid[0] - centre[0]) + normal[1] * (mid[1] - centre[1]) < 0.0 {
                normal = [-normal[0], -normal[1]];
            }
            Some((a, b, normal))
        })
        .collect();
    // Facing nowhere (no `direction` and no road near), the longest side is open.
    let nowhere = facing[0] == 0.0 && facing[1] == 0.0;
    let facing_of = |&&(a, b, normal): &&(P, P, P)| if nowhere { (b[0] - a[0]).hypot(b[1] - a[1]) } else { normal[0] * facing[0] + normal[1] * facing[1] };
    let long = sides.iter().filter(|(a, b, _)| (b[0] - a[0]).hypot(b[1] - a[1]) >= SHORTEST_OPEN);
    let (a, b, normal) = *long.max_by(|x, y| facing_of(x).total_cmp(&facing_of(y))).or_else(|| sides.iter().max_by(|x, y| facing_of(x).total_cmp(&facing_of(y)))).expect("a footprint has sides");
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    // How far the back lies behind the open side, metres.
    let depth = footprint.iter().map(|p| (mid[0] - p[0]) * normal[0] + (mid[1] - p[1]) * normal[1]).fold(0.0, f64::max);
    let pitch = if depth > 0.0 { PITCH.min((FRONT - BACK) / depth) } else { 0.0 };
    let roof = Roof::Leaf(Shape::Shed { y: floor + FRONT, at: mid, slope: [normal[0] * pitch, normal[1] * pitch] });
    (roof, Walls { open: Some((a, b)), least: BACK * 0.8 }, normal)
}

/// `Building::front` from the way a side faces: a unit vector times 127.
pub fn front(normal: P) -> [i8; 2] {
    [(normal[0] * 127.0).round() as i8, (normal[1] * 127.0).round() as i8]
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::CollisionMesh;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn near(a: P, b: P) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn a_shelters_direction_is_read_from_compass_points_or_degrees() {
        let facing_of = |direction: &str| facing(&tags(&[("direction", direction)])).unwrap();
        assert!(near(facing_of("west"), [-1.0, 0.0]));
        assert!(near(facing_of("W"), [-1.0, 0.0]));
        assert!(near(facing_of("n"), [0.0, -1.0]));
        assert!(near(facing_of("90"), [1.0, 0.0]));
        let sse = facing_of("sse");
        assert!((sse[0] - 22.5f64.to_radians().sin()).abs() < 1e-9 && sse[1] > 0.9, "{sse:?}");
        assert_eq!(facing(&tags(&[("direction", "uphill")])), None);
        assert_eq!(facing(&tags(&[])), None);
    }

    #[test]
    fn a_lean_to_is_a_shelter_and_a_bus_shelter_is_not() {
        assert!(is_lean_to(&tags(&[("building", "yes"), ("amenity", "shelter"), ("shelter_type", "lean_to")])));
        assert!(is_lean_to(&tags(&[("building", "yes"), ("amenity", "shelter")])));
        assert!(!is_lean_to(&tags(&[("building", "yes"), ("amenity", "shelter"), ("shelter_type", "public_transport")])));
        assert!(!is_lean_to(&tags(&[("building", "yes"), ("amenity", "shelter"), ("shelter_type", "picnic_shelter")])));
        assert!(!is_lean_to(&tags(&[("building", "yes")])));
    }

    #[test]
    fn a_shelters_colours_come_from_its_tags() {
        assert_eq!(paint(&tags(&[("building:colour", "red"), ("roof:colour", "black")])), Some([0x7c2b1e, 0x2a2826]));
        assert_eq!(paint(&tags(&[("roof:colour", "#ABC")])), Some([0x4a3828, 0xaabbcc]));
        assert_eq!(paint(&tags(&[("building:colour", "mauve")])), None);
    }

    #[test]
    fn a_shelter_has_no_wall_on_its_open_side_and_a_low_back() {
        // 2.5 m square, open to the west (-x).
        let footprint = [[0.0, 0.0], [2.5, 0.0], [2.5, 2.5], [0.0, 2.5]];
        let (roof, walls, normal) = build(&footprint, 10.0, [-1.0, 0.0]);
        assert!(near(normal, [-1.0, 0.0]), "{normal:?}");
        let Roof::Leaf(shape) = roof else { panic!("one plane") };
        assert!((shape.height([0.0, 1.0]) - 12.2).abs() < 1e-9, "front eave {}", shape.height([0.0, 1.0]));
        assert!((shape.height([2.5, 1.0]) - 11.2).abs() < 1e-9, "back eave {}", shape.height([2.5, 1.0]));
        let mut mesh = CollisionMesh::default();
        let parts = crate::roof::build_with(&mut mesh, &footprint, 10.0, &roof, walls);
        let corner = |v: u32| mesh.vertices[v as usize].map(|c| c as f64 / 100.0);
        let wall_tris: Vec<[[f64; 3]; 3]> = mesh.triangles[..parts.walls as usize].iter().map(|t| t.map(corner)).collect();
        // Three walls of two triangles each, none along x = 0.
        assert_eq!(wall_tris.len(), 6);
        assert!(wall_tris.iter().all(|tri| tri.iter().any(|v| v[0] > 0.01)), "a wall on the open side");
        // The back wall reaches only to the back eave.
        let back_top = wall_tris.iter().flatten().filter(|v| (v[0] - 2.5).abs() < 0.01).map(|v| v[1]).fold(0.0, f64::max);
        assert!((back_top - 11.2).abs() < 0.02, "back wall up to {back_top}");
        assert!((parts.top - 12.2).abs() < 0.02, "top {}", parts.top);
        // It cuts into cells as a house does.
        let tri = |t: &[u32; 3]| -> shared::carve::Tri {
            let [a, b, c] = t.map(|v| mesh.vertices[v as usize]);
            [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]]
        };
        let walls_cm: Vec<_> = mesh.triangles[..parts.walls as usize].iter().map(tri).collect();
        let roof_cm: Vec<_> = mesh.triangles[parts.walls as usize..(parts.walls + parts.roof) as usize].iter().map(tri).collect();
        assert!(shared::carve::Shell::new(&walls_cm, &roof_cm).is_some_and(|shell| shell.solid_count() > 0));
    }

    #[test]
    fn a_deep_shelter_is_pitched_less_so_its_back_is_not_too_low() {
        let footprint = [[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [0.0, 3.0]];
        // Open to the south (+z), 3 m deep: 0.4 a metre would bring the back to 1.0 m exactly.
        let (Roof::Leaf(shape), ..) = build(&footprint, 0.0, [0.0, 1.0]) else { panic!() };
        assert!((shape.height([2.0, 0.0]) - 1.0).abs() < 1e-9);
        let deeper = [[0.0, 0.0], [5.0, 0.0], [5.0, 4.0], [0.0, 4.0]];
        let (Roof::Leaf(shape), ..) = build(&deeper, 0.0, [0.0, 1.0]) else { panic!() };
        assert!((shape.height([2.0, 0.0]) - BACK).abs() < 1e-9, "back {}", shape.height([2.0, 0.0]));
        assert!((shape.height([2.0, 4.0]) - FRONT).abs() < 1e-9);
    }
}
