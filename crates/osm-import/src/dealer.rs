//! The vehicle dealers of a world (DEALERS.md): about one or two stores (`BUILDING_SHOP`) to a square
//! kilometre, those with the biggest car parks first, each given a carport of the game's own
//! (`shared::dealer`) against a wall: the one facing its car park, or its front where it has none.
//!
//! The car parks come from OpenStreetMap before the world is assembled (`parking`); the stores and
//! what stands round them from the finished tiles (`lay`). A store with a wall long enough for the
//! carport may have one where the carport and the apron in front of it stand clear of every other
//! building, off the main roads, off the water and away from the fuel stations (`site`). A store's
//! score is the car park within `NEAR` of it, so where several are near one another the one with the
//! most parking is taken, and none is taken nearer than `DEALER_APART` to another (`choose`). The
//! ground under each carport is levelled and paved, and its trees taken out.
//!
//! Nothing is taken out of the tiles' buildings: a store whose carport would need a neighbour gone is
//! passed over, so no building is renumbered after the tiles are assembled.

use crate::assemble::{Built, clip_ring, distance_to_segment, join_rings, point_in_polygon, signed_area};
use crate::fuel::Lot;
use crate::osm::Osm;
use shared::consts::{DEALER_APART, DEALER_MIN_PARKING, DEALER_PORT};
use shared::world::{BUILDING_SHOP, Dealer, Frame, NO_WATER, ROAD_MAJOR, ROAD_MINOR};

type P = [f64; 2];

/// How far from a store a car park may be and still be its own, metres: across the service road
/// that often runs between them.
const NEAR: f64 = 30.0;
/// How far the carport's posts and roof reach past its clear width on each side, metres
/// (`shared::dealer`'s posts and eaves), with a little room.
const SIDE: f64 = 0.7;
/// How far in front of the mouth is kept clear of buildings and trees, metres: room to drive out.
const APRON: f64 = 8.0;
/// How much of another building the carport, or its apron, may cover and still stand, square metres.
const OVERLAP: f64 = 1.0;
/// How near a fuel station's middle a carport may not stand, metres.
const FUEL_CLEAR: f64 = 35.0;
/// How far a wall must face the car park, or the road in front, to take the carport: the cosine of the
/// angle between its outward normal and the way there.
const FACING: f64 = 0.5;
/// A road at least this wide keeps a carport off it, metres, as a main road always does: the narrower
/// service roads are a car park's own aisles and drives, which a carport may stand over.
const WIDE_ROAD: f64 = 6.0;

/// The car parks on the ground of `osm`, each its outer ring (open, local metres).
pub fn parking(osm: &Osm, frame: Frame) -> Vec<Vec<P>> {
    let line = |geometry: &[crate::osm::LatLon]| -> Vec<P> { geometry.iter().map(|at| frame.project(at.lat, at.lon)).collect() };
    let open = |mut ring: Vec<P>| {
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        ring
    };
    let ways = osm.ways.iter().filter(|way| crate::assemble::is_surface_parking(&way.tags) && way.geometry.len() >= 4 && way.geometry.first() == way.geometry.last()).map(|way| open(line(&way.geometry)));
    let areas = osm.areas.iter().filter(|area| crate::assemble::is_surface_parking(&area.tags)).flat_map(|area| join_rings(area.outer.iter().map(|ring| line(ring)).collect()).into_iter().map(open));
    ways.chain(areas).filter(|ring| ring.len() >= 3).collect()
}

/// A car park, ready to measure against: its ring, its area and its bounding box.
pub struct Park {
    ring: Vec<P>,
    area: f64,
    bounds: [f64; 4],
    middle: P,
}

impl Park {
    pub fn new(ring: Vec<P>) -> Self {
        let area = signed_area(&ring).abs();
        let bounds = bounds(&ring);
        let n = ring.len() as f64;
        let middle = ring.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n, sum[1] + p[1] / n]);
        Park { ring, area, bounds, middle }
    }
}

/// A building as the choosing sees it: its footprint (open, metres), whether it is a store, the way to
/// the road it stands by (`Building::front`, a unit vector, or zero by no road), and its bounding box.
pub struct Standing {
    pub footprint: Vec<P>,
    pub shop: bool,
    pub front: P,
    bounds: [f64; 4],
}

impl Standing {
    pub fn new(footprint: Vec<P>, shop: bool, front: P) -> Self {
        let bounds = bounds(&footprint);
        Standing { footprint, shop, front, bounds }
    }
}

/// A road the carport may not stand on: its centre line (metres) and half its width.
pub struct Way {
    pub line: Vec<P>,
    pub half: f64,
}

/// A dealer's carport as chosen: its frame (`Lot`, the middle of the carport, its +Z out of the mouth),
/// and how much car park it serves, square metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    pub lot: Lot,
    pub parking: f64,
}

impl Site {
    /// The middle of the carport's mouth, metres.
    pub fn mouth(&self) -> P {
        self.lot.world([0.0, f64::from(DEALER_PORT[1]) / 2.0])
    }

    /// Half the carport's footprint with its posts and roof, metres along its X and Z.
    pub fn half() -> P {
        [f64::from(DEALER_PORT[0]) / 2.0 + SIDE, f64::from(DEALER_PORT[1]) / 2.0]
    }

    /// The carport's footprint, an open ring of metres: paved.
    pub fn ring(&self) -> Vec<P> {
        let [hx, hz] = Self::half();
        [[-hx, -hz], [hx, -hz], [hx, hz], [-hx, hz]].into_iter().map(|corner| self.lot.world(corner)).collect()
    }

    /// Whether `at` (metres) is under the carport or on its apron, or within `margin` of them.
    fn covers(&self, at: P, margin: f64) -> bool {
        let [hx, hz] = Self::half();
        let [x, z] = self.lot.local(at);
        x.abs() <= hx + margin && z >= -hz - margin && z <= hz + APRON + margin
    }
}

/// Why a store gets no carport, as the import counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// No wall long enough facing its car park or its road (or, by neither, at all).
    Wall,
    /// Every carport it could have would stand on another building, or too far into its own.
    Building,
    /// On a main road.
    Road,
    /// By a fuel station.
    Fuel,
    /// In the water.
    Water,
}

/// What else a carport must keep clear of: the roads, the fuel stations' middles, and the water
/// (`wet`, true of a point in it).
pub struct Clear<'a> {
    pub roads: &'a [Way],
    pub avoid: &'a [P],
    pub wet: &'a dyn Fn(P) -> bool,
}

/// The carport for the store `shop` (an index into `buildings`), or why it gets none (at its best wall,
/// where it has walls to try).
pub fn site(shop: usize, buildings: &[Standing], parks: &[Park], clear: &Clear) -> Result<Site, Miss> {
    let store = &buildings[shop];
    let reach = grow(store.bounds, NEAR);
    let near: Vec<&Park> = parks.iter().filter(|park| meets(park.bounds, reach) && apart(&store.footprint, &park.ring) <= NEAR).collect();
    let parking: f64 = near.iter().map(|park| park.area).sum();
    let footprint = &store.footprint;
    let n = footprint.len() as f64;
    let centre = footprint.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n, sum[1] + p[1] / n]);
    // Where the carport should face: the biggest car park big enough to count, or else the road.
    let biggest = near.iter().max_by(|a, b| a.area.total_cmp(&b.area)).filter(|_| parking >= f64::from(DEALER_MIN_PARKING));
    let towards: Option<P> = match biggest {
        Some(park) => Some(park.middle),
        None if store.front != [0.0, 0.0] => Some([centre[0] + store.front[0] * 100.0, centre[1] + store.front[1] * 100.0]),
        None => None,
    };
    let [hx, hz] = Site::half();
    // Each wall long enough, with its outward normal, best facing the car park (or the road) first;
    // by neither, the longest first.
    let mut walls: Vec<(f64, P, P, P, f64)> = (0..footprint.len())
        .filter_map(|i| {
            let (a, b) = (footprint[i], footprint[(i + 1) % footprint.len()]);
            let length = (b[0] - a[0]).hypot(b[1] - a[1]);
            if length < 2.0 * hx + 0.6 {
                return None;
            }
            let along = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let mut out = [along[1], -along[0]];
            if (mid[0] - centre[0]) * out[0] + (mid[1] - centre[1]) * out[1] < 0.0 {
                out = [-out[0], -out[1]];
            }
            let rank = match towards {
                Some(goal) => {
                    let (dx, dz) = (goal[0] - mid[0], goal[1] - mid[1]);
                    (dx * out[0] + dz * out[1]) / dx.hypot(dz).max(1e-9)
                }
                None => FACING + length / 1000.0,
            };
            (rank >= FACING).then_some((rank, a, along, out, length))
        })
        .collect();
    walls.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut miss = Miss::Wall;
    for (k, (_, a, along, out, length)) in walls.into_iter().enumerate() {
        // Along the wall across from where it faces, kept back from the wall's ends.
        let goal = towards.unwrap_or([a[0] + along[0] * length / 2.0, a[1] + along[1] * length / 2.0]);
        let t = ((goal[0] - a[0]) * along[0] + (goal[1] - a[1]) * along[1]).clamp(hx + 0.3, length - hx - 0.3);
        let foot = [a[0] + along[0] * t, a[1] + along[1] * t];
        let middle = [foot[0] + out[0] * hz, foot[1] + out[1] * hz];
        let site = Site { lot: Lot { middle, yaw: out[0].atan2(out[1]) }, parking };
        match stands(&site, shop, buildings, clear) {
            Ok(()) => return Ok(site),
            Err(why) if k == 0 => miss = why,
            Err(_) => {}
        }
    }
    Err(miss)
}

/// Whether `site` stands clear: of every building but a sliver of its own store, with its apron clear
/// of every building too; off every road of `clear`, out of the water, and away from the fuel stations.
fn stands(site: &Site, shop: usize, buildings: &[Standing], clear: &Clear) -> Result<(), Miss> {
    if clear.avoid.iter().any(|at| (at[0] - site.lot.middle[0]).hypot(at[1] - site.lot.middle[1]) < FUEL_CLEAR) {
        return Err(Miss::Fuel);
    }
    let [hx, hz] = Site::half();
    // The carport's corners and middle, and the middle of its apron.
    let spots = [[-hx, -hz], [hx, -hz], [hx, hz], [-hx, hz], [0.0, 0.0], [0.0, hz + APRON / 2.0]];
    if spots.iter().any(|&spot| (clear.wet)(site.lot.world(spot))) {
        return Err(Miss::Water);
    }
    let reach = grow(bounds(&site.ring()), APRON + 1.0);
    for (i, building) in buildings.iter().enumerate() {
        if !meets(building.bounds, reach) {
            continue;
        }
        let local: Vec<P> = building.footprint.iter().map(|&at| site.lot.local(at)).collect();
        // Its own store only touches the back; any other stands clear of the apron too.
        let rect = if i == shop { [-hx, -hz + 0.3, hx, hz + APRON] } else { [-hx, -hz, hx, hz + APRON] };
        let inside = clip_ring(&local, rect);
        if inside.len() >= 3 && signed_area(&inside).abs() > OVERLAP {
            return Err(Miss::Building);
        }
    }
    for road in clear.roads {
        for pair in road.line.windows(2) {
            let (a, b) = (site.lot.local(pair[0]), site.lot.local(pair[1]));
            let steps = (b[0] - a[0]).hypot(b[1] - a[1]).ceil().max(1.0) as usize;
            for k in 0..=steps {
                let s = k as f64 / steps as f64;
                let [x, z] = [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s];
                if x.abs() <= hx + road.half && z.abs() <= hz + road.half {
                    return Err(Miss::Road);
                }
            }
        }
    }
    Ok(())
}

/// The carports of every store in `buildings` that gets one, the most parking first, none nearer than
/// `DEALER_APART` to one taken before it; and why each of the other stores got none.
pub fn choose(buildings: &[Standing], parks: &[Park], clear: &Clear) -> (Vec<Site>, Vec<Miss>) {
    let mut found = Vec::new();
    let mut missed = Vec::new();
    for i in (0..buildings.len()).filter(|&i| buildings[i].shop) {
        match site(i, buildings, parks, clear) {
            Ok(site) => found.push(site),
            Err(miss) => missed.push(miss),
        }
    }
    found.sort_by(|a, b| b.parking.total_cmp(&a.parking));
    let mut kept: Vec<Site> = Vec::new();
    for site in found {
        let [x, z] = site.lot.middle;
        if kept.iter().all(|other| (other.lot.middle[0] - x).hypot(other.lot.middle[1] - z) >= f64::from(DEALER_APART)) {
            kept.push(site);
        }
    }
    (kept, missed)
}

/// Chooses the dealers of `built` from its stores and the car parks `parking` (`parking`), levels and
/// paves the ground under each carport, takes the trees off it and its apron, and puts them in the manifest.
pub fn lay(built: &mut Built, parking: Vec<Vec<P>>) {
    let metres = |[x, z]: [i32; 2]| [f64::from(x) / 100.0, f64::from(z) / 100.0];
    let buildings: Vec<Standing> = built
        .tiles
        .values()
        .flat_map(|tile| tile.buildings.iter())
        .filter(|b| b.footprint.len() >= 3)
        .map(|b| Standing::new(b.footprint.iter().copied().map(metres).collect(), b.kind == BUILDING_SHOP, [f64::from(b.front[0]) / 127.0, f64::from(b.front[1]) / 127.0]))
        .collect();
    let roads: Vec<Way> = built
        .tiles
        .values()
        .flat_map(|tile| tile.roads.iter())
        .filter(|road| !road.carried && (road.kind == ROAD_MAJOR || (road.kind == ROAD_MINOR && f64::from(road.width) / 100.0 >= WIDE_ROAD)))
        .map(|road| Way { line: road.points.iter().copied().map(metres).collect(), half: f64::from(road.width) / 200.0 })
        .collect();
    let avoid: Vec<P> = built.manifest.fuel_stations.iter().map(|station| metres(station.at)).collect();
    let parks: Vec<Park> = parking.into_iter().map(Park::new).collect();
    let (half, tile) = (f64::from(built.manifest.half_extent) / 100.0, f64::from(built.manifest.tile_size) / 100.0);
    let inside = |[x, z]: P| {
        let (col, row) = (((x + half) / tile).floor(), ((z + half) / tile).floor());
        col >= 0.0 && row >= 0.0 && built.tiles.contains_key(&(col as u16, row as u16))
    };
    let (chosen, missed) = {
        let wet = |at: P| wet(built, at);
        choose(&buildings, &parks, &Clear { roads: &roads, avoid: &avoid, wet: &wet })
    };
    let count = |miss: Miss| missed.iter().filter(|&&m| m == miss).count();
    eprintln!(
        "dealers: {} stores, {} could have one; none for {} with no wall to take it, {} for a building in the way, {} on a main road, {} by a fuel station, {} in the water",
        buildings.iter().filter(|b| b.shop).count(),
        buildings.iter().filter(|b| b.shop).count() - missed.len(),
        count(Miss::Wall),
        count(Miss::Building),
        count(Miss::Road),
        count(Miss::Fuel),
        count(Miss::Water)
    );
    let sites: Vec<Site> = chosen.into_iter().filter(|site| inside(site.lot.middle)).collect();
    let [hx, hz] = Site::half();
    let mut dealers = Vec::new();
    for site in &sites {
        let mut heights = Vec::new();
        for i in 0..=((2.0 * hx) as i32) {
            for j in 0..=((2.0 * hz) as i32) {
                heights.extend(crate::fuel::ground(built, site.lot.world([-hx + f64::from(i), -hz + f64::from(j)])));
            }
        }
        heights.sort_by(f64::total_cmp);
        let base = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
        crate::fuel::level(built, &site.lot, [hx, hz], base);
        crate::fuel::pave(built, &site.ring());
        let mouth = site.mouth();
        dealers.push(Dealer { at: [cm(mouth[0]), cm(mouth[1])], yaw: (site.lot.yaw * 1000.0).round() as i16, base: cm(base) });
    }
    for tile in built.tiles.values_mut() {
        tile.trees.retain(|tree| {
            let at = [f64::from(tree.x) / 100.0, f64::from(tree.z) / 100.0];
            !sites.iter().any(|site| site.covers(at, f64::from(tree.crown) / 100.0))
        });
    }
    built.manifest.dealers = dealers;
}

/// Whether `at` (metres) is in the water of `built`: its tile's nearest sample has a surface over the ground.
fn wet(built: &Built, at: P) -> bool {
    let manifest = &built.manifest;
    if manifest.ground_cell <= 0 {
        return false;
    }
    let (col, row) = manifest.tile_at(cm(at[0]), cm(at[1]));
    let Some(tile) = built.tiles.get(&(col, row)) else { return false };
    let samples = (manifest.tile_size / manifest.ground_cell) as usize + 1;
    if tile.water.len() != samples * samples || tile.ground.len() != samples * samples {
        return false;
    }
    let local = |v: f64, index: u16| ((v * 100.0 + f64::from(manifest.half_extent) - f64::from(index) * f64::from(manifest.tile_size)) / f64::from(manifest.ground_cell)).round().clamp(0.0, samples as f64 - 1.0) as usize;
    let k = local(at[1], row) * samples + local(at[0], col);
    tile.water[k] != NO_WATER && tile.water[k] > tile.ground[k]
}

/// The bounding box `[x0, z0, x1, z1]` of `ring`, metres.
fn bounds(ring: &[P]) -> [f64; 4] {
    ring.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

/// `b` grown by `by` metres on every side.
fn grow(b: [f64; 4], by: f64) -> [f64; 4] {
    [b[0] - by, b[1] - by, b[2] + by, b[3] + by]
}

/// Whether boxes `a` and `b` meet.
fn meets(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// How far apart rings `a` and `b` are, metres: 0 where they overlap.
fn apart(a: &[P], b: &[P]) -> f64 {
    if a.iter().any(|&p| point_in_polygon(p, b)) || b.iter().any(|&p| point_in_polygon(p, a)) {
        return 0.0;
    }
    let edges = |ring: &[P]| -> Vec<(P, P)> { (0..ring.len()).map(|i| (ring[i], ring[(i + 1) % ring.len()])).collect() };
    let (ea, eb) = (edges(a), edges(b));
    let from_a = a.iter().flat_map(|&p| eb.iter().map(move |&(x, y)| distance_to_segment(p, x, y)));
    let from_b = b.iter().flat_map(|&p| ea.iter().map(move |&(x, y)| distance_to_segment(p, x, y)));
    from_a.chain(from_b).fold(f64::MAX, f64::min)
}

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store 40 × 20 m, its long south wall (+Z) along z = 10 from x = -20 to 20, with its road to the
    /// north (-Z).
    fn store(at: P) -> Standing {
        let [x, z] = at;
        Standing::new(vec![[x - 20.0, z - 10.0], [x + 20.0, z - 10.0], [x + 20.0, z + 10.0], [x - 20.0, z + 10.0]], true, [0.0, -1.0])
    }

    /// A car park 60 × 30 m south of the store at `at`, 2 m off its wall.
    fn lot_south(at: P) -> Park {
        let [x, z] = at;
        Park::new(vec![[x - 30.0, z + 12.0], [x + 30.0, z + 12.0], [x + 30.0, z + 42.0], [x - 30.0, z + 42.0]])
    }

    fn open() -> Clear<'static> {
        Clear { roads: &[], avoid: &[], wet: &|_| false }
    }

    #[test]
    fn a_store_by_a_big_car_park_gets_a_carport_against_the_wall_facing_it() {
        let buildings = [store([0.0, 0.0])];
        let site = site(0, &buildings, &[lot_south([0.0, 0.0])], &open()).expect("a dealer");
        // Its +Z, out of the mouth, is +Z of the world: a yaw of 0.
        assert!(site.lot.yaw.abs() < 1e-9, "{site:?}");
        let [_, hz] = Site::half();
        assert!((site.lot.middle[1] - (10.0 + hz)).abs() < 1e-6, "its back against the wall: {site:?}");
        assert!(site.lot.middle[0].abs() <= 20.0, "along the wall");
        assert!((site.mouth()[1] - (10.0 + 2.0 * hz)).abs() < 1e-6);
        assert_eq!(site.parking, 1800.0);
    }

    #[test]
    fn a_store_with_no_car_park_or_a_small_one_faces_its_road_and_counts_none() {
        let buildings = [store([0.0, 0.0])];
        let bare = site(0, &buildings, &[], &open()).expect("a dealer all the same");
        assert!((bare.lot.yaw.abs() - std::f64::consts::PI).abs() < 1e-9, "out to the road, north: {bare:?}");
        assert_eq!(bare.parking, 0.0);
        let small = Park::new(vec![[-10.0, 12.0], [10.0, 12.0], [10.0, 30.0], [-10.0, 30.0]]);
        let beside = site(0, &buildings, &[small], &open()).unwrap();
        assert!((beside.lot.yaw.abs() - std::f64::consts::PI).abs() < 1e-9, "360 m² is too little to turn it");
        assert_eq!(beside.parking, 360.0, "but it counts");
        let far = Park::new(vec![[-30.0, 50.0], [30.0, 50.0], [30.0, 80.0], [-30.0, 80.0]]);
        assert_eq!(site(0, &buildings, &[far], &open()).unwrap().parking, 0.0, "40 m off is not its car park");
    }

    #[test]
    fn a_carport_on_a_neighbour_a_main_road_a_fuel_station_or_the_water_is_not_built() {
        // A kiosk right in front of the whole south wall, and another on the north.
        let south = Standing::new(vec![[-20.0, 13.0], [20.0, 13.0], [20.0, 16.0], [-20.0, 16.0]], false, [0.0, 0.0]);
        let north = Standing::new(vec![[-20.0, -16.0], [20.0, -16.0], [20.0, -13.0], [-20.0, -13.0]], false, [0.0, 0.0]);
        let parks = [lot_south([0.0, 0.0])];
        assert_eq!(site(0, &[store([0.0, 0.0]), south, north], &parks, &open()), Err(Miss::Building));
        let road = [Way { line: vec![[-100.0, 15.0], [100.0, 15.0]], half: 4.0 }];
        assert_eq!(site(0, &[store([0.0, 0.0])], &parks, &Clear { roads: &road, ..open() }), Err(Miss::Road), "a road along the wall");
        assert_eq!(site(0, &[store([0.0, 0.0])], &parks, &Clear { avoid: &[[0.0, 30.0]], ..open() }), Err(Miss::Fuel));
        assert_eq!(site(0, &[store([0.0, 0.0])], &parks, &Clear { wet: &|[_, z]| z > 20.0, ..open() }), Err(Miss::Water));
    }

    #[test]
    fn a_store_too_small_for_the_carport_gets_none() {
        let kiosk = Standing::new(vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]], true, [0.0, 1.0]);
        assert_eq!(site(0, &[kiosk], &[], &open()), Err(Miss::Wall));
    }

    #[test]
    fn of_two_stores_closer_than_the_dealers_spacing_the_one_with_more_parking_is_chosen() {
        let bigger = Park::new(vec![[270.0, 12.0], [330.0, 12.0], [330.0, 62.0], [270.0, 62.0]]);
        let buildings = [store([0.0, 0.0]), store([300.0, 0.0]), store([2_000.0, 0.0])];
        let parks = [lot_south([0.0, 0.0]), bigger];
        let (chosen, _) = choose(&buildings, &parks, &open());
        assert_eq!(chosen.len(), 2, "{chosen:?}");
        assert!((chosen[0].lot.middle[0] - 300.0).abs() <= 20.0, "the bigger car park first: {chosen:?}");
        assert!((chosen[1].lot.middle[0] - 2_000.0).abs() <= 20.0, "and one with none, far off");
    }

    #[test]
    fn the_synthetic_city_has_one_dealer_facing_its_car_park() {
        let built = crate::assemble::synthetic_city(300.0, 100.0, Default::default(), Default::default());
        assert_eq!(built.manifest.dealers.len(), 1, "the offline world has one to buy from");
        let dealer = built.manifest.dealers[0];
        assert!(dealer.yaw.abs() <= 1, "facing south over its car park: {dealer:?}");
    }

    #[test]
    fn a_house_is_never_a_dealer() {
        let house = Standing::new(store([0.0, 0.0]).footprint, false, [0.0, -1.0]);
        assert!(choose(&[house], &[lot_south([0.0, 0.0])], &open()).0.is_empty());
    }
}
