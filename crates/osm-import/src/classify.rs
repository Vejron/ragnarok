//! What each building is (`BUILDING_*`): a house, a church, a barn, an outbuilding, a hall, a shop,
//! a block of flats, a public building, a fire station or a cabin.
//!
//! OpenStreetMap says so for few of them: nine in ten of Sweden's are `building=yes` and no more.
//! So a building is known, in this order, by its own tags; by a word in its name; by the land use it
//! stands in (a big one on industrial land is a hall); and by its shape and what stands round it (a
//! small one beside a bigger one is an outbuilding, a long plain one that is the biggest of a farm's
//! yard is a barn, a small one alone in a field is a hay barn and one alone in the woods a cabin).
//! What none of that names is a house, and a house is an old wooden one, a post-war villa or
//! neither by where it stands: old on a farm, and in a village by its quarter, since a street is
//! built at once.
//!
//! The numbers were set against the tagged buildings of northern Sweden (62° to 66° N, 337 000
//! buildings, 734 of them tagged as barns and 50 000 as houses): the yard rule finds a fifth of the
//! tagged barns and takes 0.8% of the tagged houses for one, and the field rule another tenth and
//! 0.1%; the cabin rule finds a tenth of the 836 tagged cabins and takes 0.5% of the houses. A barn
//! or a cabin it misses is drawn as a house, as all were before.

use crate::osm::{Lookup, Tags};
use shared::world::*;
use std::collections::HashMap;

type P = [f64; 2];

/// The land use a building stands in or near, as far as it says what the building is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Land {
    /// Fields and meadows: `landuse=farmland`, `meadow`.
    Farm,
    /// A farm's yard: `landuse=farmyard`.
    Farmyard,
    /// Where people live: `landuse=residential`.
    Residential,
    /// Factories, workshops and depots: `landuse=industrial`, and the railway's land.
    Industrial,
    /// Shops and offices: `landuse=retail`, `commercial`.
    Trade,
    /// Garden plots with their sheds: `landuse=allotments`.
    Allotments,
}

impl Land {
    /// The land use `tags` map, where it is one of these.
    pub fn of(tags: &Tags) -> Option<Land> {
        Some(match tags.get("landuse")?.as_str() {
            "farmland" | "meadow" => Land::Farm,
            "farmyard" => Land::Farmyard,
            "residential" => Land::Residential,
            "industrial" | "railway" => Land::Industrial,
            "retail" | "commercial" => Land::Trade,
            "allotments" => Land::Allotments,
            _ => return None,
        })
    }
}

/// Under this footprint, square metres, a building beside a bigger one is an outbuilding.
const SMALL: f64 = 45.0;
/// How far round a building its yard reaches, and its neighbourhood, metres.
const YARD: f64 = 50.0;
const NEIGHBOURHOOD: f64 = 150.0;
/// More buildings than this in its neighbourhood and a building stands in a village, where a long
/// plain one is a block of flats or a school sooner than a barn.
const VILLAGE: usize = 15;
/// A house, for a barn to have one beside it: its footprint from and to, square metres.
const HOUSE: [f64; 2] = [60.0, 260.0];
/// A barn is plain: at most this many corners, filling at least this share of the box round it.
const PLAIN_CORNERS: usize = 8;
const PLAIN_FILL: f64 = 0.85;
/// A long barn: at least this long and this big, at least this many times as long as it is wide, and no wider than this, metres.
const LONG_BARN: [f64; 4] = [16.0, 130.0, 1.6, 14.0];
/// A big barn: at least this long and this big, whatever its width, metres.
const BIG_BARN: [f64; 2] = [20.0, 180.0];
/// A barn with no house beside it has fields within this, metres.
const BY_FIELDS: f64 = 120.0;
/// A hay barn: a footprint from and to, square metres, nothing within `YARD`, and fields within this, metres.
const HAY_BARN: [f64; 2] = [15.0, 130.0];
const IN_FIELDS: f64 = 30.0;
/// On industrial land an untagged building from this footprint is a hall, and on shops' land from
/// this one a shop, square metres.
const HALL: f64 = 150.0;
const SHOP: f64 = 200.0;
/// A cabin: a footprint under this, square metres, on no mapped land, with at most this many
/// buildings in its neighbourhood.
const CABIN: f64 = 70.0;
const CABIN_NEIGHBOURS: usize = 3;
/// An untagged building with this many `building:levels` or more is a block of flats.
const FLATS_LEVELS: f64 = 3.0;
/// A `building=farm_auxiliary` from this footprint, square metres, is a barn; under it an outbuilding.
const AUXILIARY_BARN: f64 = 80.0;

/// How far from a building's middle a node inside it is looked for, metres.
const NODE_REACH: f64 = 150.0;

/// A house that gets a style of its own: a footprint from and to, square metres.
const STYLED_HOUSE: [f64; 2] = [50.0, 260.0];
/// A quarter of a village, metres a side: its houses are of one age.
const QUARTER: f64 = 200.0;
/// A villa is squat: at least this wide across its short side, metres, and this big, square metres.
const SQUAT: [f64; 2] = [9.0, 110.0];

/// Whether the tags are a dwelling's, or no more than a building's: what `house_style` may style.
fn is_dwelling(tags: &Tags) -> bool {
    matches!(tags.get("building").map(String::as_str), Some("yes" | "house" | "detached" | "residential" | "semidetached_house" | "bungalow" | "farm"))
}

/// A number from 0 to 2 for the quarter `at` lies in, the same for every house of it.
fn quarter_of(at: P) -> u32 {
    let (x, z) = ((at[0] / QUARTER).floor() as i32, (at[1] / QUARTER).floor() as i32);
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (z as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h % 3
}

/// The `amenity` values of a place of trade: where people eat, drink, bank and fill up, in a building like a shop's.
pub const TRADES: [&str; 10] = ["restaurant", "cafe", "fast_food", "pub", "bar", "bank", "pharmacy", "post_office", "fuel", "car_wash"];
/// The `amenity` values of a public building.
pub const PUBLIC: [&str; 10] = ["school", "kindergarten", "library", "police", "townhall", "hospital", "clinic", "community_centre", "social_centre", "courthouse"];

/// What the use mapped on a building, or on a node inside one, says the building is: a shop, a
/// place of trade, a public building or a fire station. A node of this kind is read with the
/// buildings (`osm::is_poi`): most shops and police stations are mapped as one, not on the building.
pub fn by_use<T: Lookup + ?Sized>(tags: &T) -> Option<u8> {
    if tags.tag("shop").is_some_and(|value| value != "no" && value != "vacant") {
        return Some(BUILDING_SHOP);
    }
    match tags.tag("amenity")? {
        "fire_station" => Some(BUILDING_FIRE_STATION),
        value if PUBLIC.contains(&value) => Some(BUILDING_PUBLIC),
        value if TRADES.contains(&value) => Some(BUILDING_SHOP),
        _ => None,
    }
}

/// A church: mapped as a church or a chapel, or as a Christian place of worship.
pub fn is_church(tags: &Tags) -> bool {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    matches!(tag("building"), Some("church" | "chapel" | "cathedral")) || (tag("amenity") == Some("place_of_worship") && tag("religion") == Some("christian"))
}

/// What the tags of a building with a footprint of `area` square metres say it is, or `None` where
/// they say no more than that it is a building (`building=yes`).
pub fn by_tags(tags: &Tags, area: f64) -> Option<u8> {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    if is_church(tags) {
        return Some(BUILDING_CHURCH);
    }
    // `building=school;library`: the first of several values.
    let building = tag("building").map(|value| value.split(';').next().unwrap_or(value));
    let named = match building {
        Some("barn" | "stable" | "cowshed" | "sty") => Some(BUILDING_BARN),
        Some("farm_auxiliary") => Some(if area >= AUXILIARY_BARN { BUILDING_BARN } else { BUILDING_OUTBUILDING }),
        Some("garage" | "garages" | "shed" | "carport" | "service") => Some(BUILDING_OUTBUILDING),
        Some("industrial" | "warehouse" | "hangar" | "manufacture") => Some(BUILDING_HALL),
        Some("retail" | "supermarket" | "commercial" | "kiosk") => Some(BUILDING_SHOP),
        Some("apartments" | "dormitory" | "hotel") => Some(BUILDING_FLATS),
        Some("fire_station") => Some(BUILDING_FIRE_STATION),
        Some("school" | "kindergarten" | "university" | "college" | "hospital" | "civic" | "public" | "government" | "train_station") => Some(BUILDING_PUBLIC),
        Some("cabin" | "hut") => Some(BUILDING_CABIN),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if tag("man_made") == Some("works") {
        return Some(BUILDING_HALL);
    }
    if let Some(kind) = by_use(tags) {
        return Some(kind);
    }
    if matches!(tag("tourism"), Some("wilderness_hut" | "chalet" | "alpine_hut")) {
        return Some(BUILDING_CABIN);
    }
    // Any other value is what the mapper saw: a house, a school, a block of flats.
    if building != Some("yes") {
        return Some(BUILDING_PLAIN);
    }
    if tag("building:levels").and_then(|levels| levels.parse::<f64>().ok()).is_some_and(|levels| levels >= FLATS_LEVELS) {
        return Some(BUILDING_FLATS);
    }
    // A name that says it: "Lada", "Gamla ladugården".
    let name = tag("name").unwrap_or_default().to_lowercase();
    let mut words = name.split(|c: char| !c.is_alphanumeric());
    if words.clone().any(|word| matches!(word, "lada" | "ladan" | "ladugård" | "ladugården" | "loge" | "logen" | "stall" | "stallet")) {
        return Some(BUILDING_BARN);
    }
    if words.clone().any(|word| matches!(word, "brandstation" | "brandstationen" | "räddningstjänst" | "räddningstjänsten" | "brandförsvar" | "brandförsvaret")) {
        return Some(BUILDING_FIRE_STATION);
    }
    // A village's meeting house, a chapel of the free churches, a school: "Tavelsjö bygdegård", "EFS Bodbyn".
    let meeting = words.any(|word| matches!(word, "bygdegård" | "bygdegården" | "byastuga" | "byastugan" | "bystuga" | "bystugan" | "efs" | "missionshus" | "missionshuset" | "bönhus" | "bönhuset" | "skola" | "skolan" | "förskola" | "förskolan" | "hembygdsgård" | "hembygdsgården")) || name.contains("folkets hus");
    meeting.then_some(BUILDING_PUBLIC)
}

/// A building as the rules see it.
struct Seen {
    mid: P,
    area: f64,
    /// The long and the short side of the smallest box round it that lies along one of its walls, metres.
    long: f64,
    short: f64,
    corners: usize,
}

fn area_of(ring: &[P]) -> f64 {
    (0..ring.len()).map(|i| { let (p, q) = (ring[i], ring[(i + 1) % ring.len()]); p[0] * q[1] - q[0] * p[1] }).sum::<f64>().abs() / 2.0
}

fn see(ring: &[P]) -> Seen {
    let n = ring.len().max(1) as f64;
    let mid = ring.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
    let mut best: Option<(f64, f64)> = None;
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dz) = (q[0] - p[0], q[1] - p[1]);
        let length = dx.hypot(dz);
        if length < 0.5 {
            continue;
        }
        let (ux, uz) = (dx / length, dz / length);
        let span = |f: &dyn Fn(&P) -> f64| { let (lo, hi) = ring.iter().map(f).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v))); hi - lo };
        let (a, b) = (span(&|r| r[0] * ux + r[1] * uz), span(&|r| -r[0] * uz + r[1] * ux));
        if best.is_none_or(|(l, s)| a * b < l * s) {
            best = Some((a.max(b), a.min(b)));
        }
    }
    let (long, short) = best.unwrap_or((1.0, 1.0));
    Seen { mid, area: area_of(ring), long, short: short.max(0.01), corners: ring.len() }
}

fn holds(ring: &[P], [x, z]: P) -> bool {
    let mut inside = false;
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        if (p[1] > z) != (q[1] > z) && x < (q[0] - p[0]) * (z - p[1]) / (q[1] - p[1]) + p[0] {
            inside = !inside;
        }
    }
    inside
}

/// How far `at` is from `ring`: 0 inside it, else to its nearest side, metres.
fn distance(ring: &[P], at: P) -> f64 {
    if holds(ring, at) {
        return 0.0;
    }
    let mut nearest = f64::MAX;
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dz) = (q[0] - p[0], q[1] - p[1]);
        let t = if dx == 0.0 && dz == 0.0 { 0.0 } else { (((at[0] - p[0]) * dx + (at[1] - p[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0) };
        nearest = nearest.min((p[0] + dx * t - at[0]).hypot(p[1] + dz * t - at[1]));
    }
    nearest
}

/// How many buildings were known by what, for the importer's log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub by_tags: usize,
    /// By a shop, a police station or the like mapped as a node inside it.
    pub by_node: usize,
    pub by_land: usize,
    pub by_yard: usize,
    /// How many of each `BUILDING_*`, by its number.
    pub kinds: [usize; BUILDING_KINDS],
}

/// What each of `sites` is (`BUILDING_*`): a building's tags and its outline (local metres, not
/// closed), among `lands`, the land use areas round them, and `nodes`, the mapped uses (`by_use`)
/// and where each is.
pub fn classify(sites: &[(&Tags, &[P])], lands: &[(Land, Vec<P>)], nodes: &[(u8, P)]) -> (Vec<u8>, Tally) {
    let seen: Vec<Seen> = sites.iter().map(|(_, ring)| see(ring)).collect();
    // The buildings by the `YARD` square their middle is in.
    let cell = |at: P| ((at[0] / YARD).floor() as i32, (at[1] / YARD).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (index, s) in seen.iter().enumerate() {
        grid.entry(cell(s.mid)).or_default().push(index);
    }
    let near = |index: usize, reach: f64| -> Vec<usize> {
        let s = &seen[index];
        let (cx, cz) = cell(s.mid);
        let span = (reach / YARD).ceil() as i32;
        let mut found = Vec::new();
        for x in cx - span..=cx + span {
            for z in cz - span..=cz + span {
                for &other in grid.get(&(x, z)).into_iter().flatten() {
                    let o = &seen[other];
                    if other != index && (o.mid[0] - s.mid[0]).hypot(o.mid[1] - s.mid[1]) < reach {
                        found.push(other);
                    }
                }
            }
        }
        found
    };
    // The lands, each with the box round it.
    let boxed: Vec<(Land, &[P], [f64; 4], f64)> = lands
        .iter()
        .filter(|(_, ring)| ring.len() >= 3)
        .map(|(land, ring)| {
            let bounds = ring.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
            (*land, ring.as_slice(), bounds, area_of(ring))
        })
        .collect();
    let within = |b: &[f64; 4], at: P, by: f64| at[0] >= b[0] - by && at[0] <= b[2] + by && at[1] >= b[1] - by && at[1] <= b[3] + by;
    // The land a building stands in: the smallest area round its middle that is no field.
    let land_at = |at: P| -> Option<Land> {
        boxed.iter().filter(|(land, ring, bounds, _)| *land != Land::Farm && within(bounds, at, 0.0) && holds(ring, at)).min_by(|a, b| a.3.total_cmp(&b.3)).map(|found| found.0)
    };
    let fields_within = |at: P, reach: f64| boxed.iter().any(|(land, ring, bounds, _)| matches!(land, Land::Farm | Land::Farmyard) && within(bounds, at, reach) && distance(ring, at) < reach);

    // What the nodes inside a building say it is: a fire station before a public building before a shop.
    let weight = |kind: u8| match kind {
        BUILDING_FIRE_STATION => 3,
        BUILDING_PUBLIC => 2,
        _ => 1,
    };
    let mut said: HashMap<usize, u8> = HashMap::new();
    for &(kind, at) in nodes {
        let (cx, cz) = cell(at);
        // A building's middle is within `NODE_REACH` of a node inside it, but for the very biggest.
        let span = (NODE_REACH / YARD).ceil() as i32;
        let holder = (cx - span..=cx + span).flat_map(|x| (cz - span..=cz + span).map(move |z| (x, z))).flat_map(|key| grid.get(&key).into_iter().flatten()).find(|&&index| holds(sites[index].1, at));
        if let Some(&index) = holder {
            let held = said.entry(index).or_insert(kind);
            if weight(kind) > weight(*held) {
                *held = kind;
            }
        }
    }

    let mut tally = Tally::default();
    let kinds: Vec<u8> = (0..sites.len())
        .map(|index| {
            let s = &seen[index];
            if let Some(kind) = by_tags(sites[index].0, s.area) {
                tally.by_tags += usize::from(kind != BUILDING_PLAIN);
                return kind;
            }
            if let Some(&kind) = said.get(&index) {
                tally.by_node += 1;
                return kind;
            }
            let land = land_at(s.mid);
            let by_land = match land {
                Some(Land::Industrial) if s.area >= HALL => Some(BUILDING_HALL),
                Some(Land::Industrial) if s.area < SMALL => Some(BUILDING_OUTBUILDING),
                Some(Land::Trade) if s.area >= SHOP => Some(BUILDING_SHOP),
                Some(Land::Allotments) => Some(BUILDING_OUTBUILDING),
                _ => None,
            };
            if let Some(kind) = by_land {
                tally.by_land += 1;
                return kind;
            }
            let yard = near(index, YARD);
            let kind = if s.area < SMALL && yard.iter().any(|&other| seen[other].area > s.area) {
                BUILDING_OUTBUILDING
            } else if matches!(land, None | Some(Land::Farmyard)) {
                let plain = s.corners <= PLAIN_CORNERS && s.area / (s.long * s.short) > PLAIN_FILL;
                let long = s.long >= LONG_BARN[0] && s.area >= LONG_BARN[1] && s.long / s.short >= LONG_BARN[2] && s.short <= LONG_BARN[3];
                let big = s.long >= BIG_BARN[0] && s.area >= BIG_BARN[1];
                let house_beside = || yard.iter().any(|&other| { let o = &seen[other]; o.area >= HOUSE[0] && o.area <= HOUSE[1] && o.area < s.area });
                // A farm's barn, or a hay barn alone in its field.
                let farm = plain && (long || big) && near(index, NEIGHBOURHOOD).len() <= VILLAGE && (house_beside() || fields_within(s.mid, BY_FIELDS));
                let hay = land.is_none() && s.area >= HAY_BARN[0] && s.area < HAY_BARN[1] && yard.is_empty() && fields_within(s.mid, IN_FIELDS);
                // A small house on its own, far from any village.
                let cabin = land.is_none() && s.area < CABIN && near(index, NEIGHBOURHOOD).len() <= CABIN_NEIGHBOURS;
                if farm || hay {
                    BUILDING_BARN
                } else if cabin {
                    BUILDING_CABIN
                } else {
                    BUILDING_PLAIN
                }
            } else {
                BUILDING_PLAIN
            };
            tally.by_yard += usize::from(kind != BUILDING_PLAIN);
            kind
        })
        .collect();
    // A house's style: old on a farm; in a village as its quarter is, old, post-war or mixed.
    let kinds: Vec<u8> = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let s = &seen[index];
            if kind != BUILDING_PLAIN || !is_dwelling(sites[index].0) || s.area < STYLED_HOUSE[0] || s.area > STYLED_HOUSE[1] {
                return kind;
            }
            let rural = matches!(land_at(s.mid), None | Some(Land::Farmyard)) && near(index, NEIGHBOURHOOD).len() <= VILLAGE;
            let squat = s.short >= SQUAT[0] && s.area >= SQUAT[1];
            match (rural, quarter_of(s.mid)) {
                (true, _) | (false, 0) => BUILDING_FARMHOUSE,
                (false, 1) => BUILDING_VILLA,
                _ if squat => BUILDING_VILLA,
                _ => BUILDING_PLAIN,
            }
        })
        .collect();
    for &kind in &kinds {
        tally.kinds[usize::from(kind)] += 1;
    }
    (kinds, tally)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// A rectangle `w` × `d` metres with its corner at `(x, z)`.
    fn rect(x: f64, z: f64, w: f64, d: f64) -> Vec<P> {
        vec![[x, z], [x + w, z], [x + w, z + d], [x, z + d]]
    }

    /// The kinds of untagged buildings on `rings`, among `lands`, a house of any style as `BUILDING_PLAIN`.
    fn kinds(rings: &[Vec<P>], lands: &[(Land, Vec<P>)]) -> Vec<u8> {
        styles(rings, lands).into_iter().map(|kind| if matches!(kind, BUILDING_FARMHOUSE | BUILDING_VILLA) { BUILDING_PLAIN } else { kind }).collect()
    }

    /// The kinds of untagged buildings on `rings`, among `lands`.
    fn styles(rings: &[Vec<P>], lands: &[(Land, Vec<P>)]) -> Vec<u8> {
        let yes = tags(&[("building", "yes")]);
        let sites: Vec<(&Tags, &[P])> = rings.iter().map(|ring| (&yes, ring.as_slice())).collect();
        classify(&sites, lands, &[]).0
    }

    #[test]
    fn a_house_on_a_farm_is_an_old_one_and_the_houses_of_a_village_quarter_are_of_one_age() {
        assert_eq!(styles(&[rect(0.0, 0.0, 12.0, 9.0)], &[]), [BUILDING_FARMHOUSE]);
        // Narrow houses in rows 30 m apart on residential land, over many quarters.
        let rings: Vec<Vec<P>> = (0..400).map(|i| rect(30.0 * f64::from(i % 20), 30.0 * f64::from(i / 20), 12.0, 8.0)).collect();
        let town = styles(&rings, &[(Land::Residential, rect(-50.0, -50.0, 700.0, 700.0))]);
        for style in [BUILDING_PLAIN, BUILDING_FARMHOUSE, BUILDING_VILLA] {
            assert!(town.iter().filter(|&&kind| kind == style).count() > 20, "{style}: some quarters of each");
        }
        // Next-door neighbours within a quarter are the same.
        assert_eq!(town[0], town[1]);
        assert_eq!(town[1], town[21]);
        // A squat house in a mixed quarter is a villa, a narrow one beside it neither.
        let mixed = (0..400).find(|&i| town[i] == BUILDING_PLAIN).unwrap();
        let mut squat = rings.clone();
        squat[mixed] = rect(30.0 * (mixed % 20) as f64, 30.0 * (mixed / 20) as f64, 14.0, 10.0);
        assert_eq!(styles(&squat, &[(Land::Residential, rect(-50.0, -50.0, 700.0, 700.0))])[mixed], BUILDING_VILLA);
        // A school is not restyled, nor a block too big to be a house.
        let school = tags(&[("building", "terrace")]);
        let ring = rect(0.0, 0.0, 12.0, 9.0);
        assert_eq!(classify(&[(&school, ring.as_slice())], &[], &[]).0, [BUILDING_PLAIN]);
        assert_eq!(styles(&[rect(0.0, 0.0, 30.0, 12.0)], &[(Land::Residential, rect(-50.0, -50.0, 200.0, 200.0))]), [BUILDING_PLAIN]);
    }

    #[test]
    fn a_church_is_known_by_its_building_tag_or_as_a_christian_place_of_worship() {
        for pairs in [&[("building", "church")][..], &[("building", "chapel")], &[("building", "yes"), ("amenity", "place_of_worship"), ("religion", "christian")]] {
            assert_eq!(by_tags(&tags(pairs), 300.0), Some(BUILDING_CHURCH), "{pairs:?}");
        }
        for pairs in [&[("building", "yes")][..], &[("building", "yes"), ("amenity", "place_of_worship"), ("religion", "muslim")], &[("building", "yes"), ("amenity", "place_of_worship")]] {
            assert_eq!(by_tags(&tags(pairs), 300.0), None, "{pairs:?}");
        }
    }

    #[test]
    fn a_tagged_building_is_what_its_tags_say() {
        for (pairs, area, kind) in [
            (&[("building", "barn")][..], 200.0, BUILDING_BARN),
            (&[("building", "farm_auxiliary")], 200.0, BUILDING_BARN),
            (&[("building", "farm_auxiliary")], 40.0, BUILDING_OUTBUILDING),
            (&[("building", "garage")], 60.0, BUILDING_OUTBUILDING),
            (&[("building", "shed")], 20.0, BUILDING_OUTBUILDING),
            (&[("building", "industrial")], 1500.0, BUILDING_HALL),
            (&[("building", "warehouse")], 600.0, BUILDING_HALL),
            (&[("building", "supermarket")], 1700.0, BUILDING_SHOP),
            (&[("building", "yes"), ("shop", "supermarket")], 1700.0, BUILDING_SHOP),
            (&[("building", "yes"), ("name", "Gamla ladugården")], 200.0, BUILDING_BARN),
            (&[("building", "yes"), ("name", "Tavelsjö bygdegård")], 300.0, BUILDING_PUBLIC),
            (&[("building", "yes"), ("name", "Folkets Hus")], 300.0, BUILDING_PUBLIC),
            // What a mapper called a house is one, however much it looks like a barn.
            (&[("building", "house")], 300.0, BUILDING_PLAIN),
            (&[("building", "terrace")], 300.0, BUILDING_PLAIN),
            (&[("building", "apartments")], 500.0, BUILDING_FLATS),
            (&[("building", "yes"), ("building:levels", "4")], 500.0, BUILDING_FLATS),
            (&[("building", "school")], 500.0, BUILDING_PUBLIC),
            (&[("building", "school;library")], 500.0, BUILDING_PUBLIC),
            (&[("building", "yes"), ("amenity", "police")], 300.0, BUILDING_PUBLIC),
            (&[("building", "fire_station")], 400.0, BUILDING_FIRE_STATION),
            (&[("building", "yes"), ("amenity", "fire_station")], 400.0, BUILDING_FIRE_STATION),
            (&[("building", "yes"), ("name", "Vindelns brandstation")], 400.0, BUILDING_FIRE_STATION),
            (&[("building", "yes"), ("amenity", "library")], 300.0, BUILDING_PUBLIC),
            (&[("building", "yes"), ("amenity", "restaurant")], 300.0, BUILDING_SHOP),
            (&[("building", "yes"), ("amenity", "fuel")], 120.0, BUILDING_SHOP),
            (&[("building", "cabin")], 30.0, BUILDING_CABIN),
            (&[("building", "yes"), ("tourism", "wilderness_hut")], 20.0, BUILDING_CABIN),
        ] {
            assert_eq!(by_tags(&tags(pairs), area), Some(kind), "{pairs:?}");
        }
        assert_eq!(by_tags(&tags(&[("building", "yes"), ("name", "Installationsbolaget")]), 200.0), None, "a word within a word is no barn");
    }

    #[test]
    fn a_shop_or_a_police_station_mapped_as_a_node_names_the_untagged_building_it_lies_in() {
        let yes = tags(&[("building", "yes")]);
        let house = tags(&[("building", "house")]);
        let rings = [rect(0.0, 0.0, 20.0, 12.0), rect(40.0, 0.0, 20.0, 12.0), rect(80.0, 0.0, 20.0, 12.0), rect(120.0, 0.0, 12.0, 9.0)];
        let sites: Vec<(&Tags, &[P])> = vec![(&yes, &rings[0]), (&yes, &rings[1]), (&yes, &rings[2]), (&house, &rings[3])];
        let use_of = |pairs: &[(&str, &str)]| by_use(&tags(pairs)).unwrap();
        let nodes = [
            (use_of(&[("shop", "bakery")]), [10.0, 6.0]),
            // A café and the police in one building: it is the police station.
            (use_of(&[("amenity", "cafe")]), [45.0, 3.0]),
            (use_of(&[("amenity", "police")]), [55.0, 9.0]),
            (use_of(&[("amenity", "fire_station")]), [90.0, 6.0]),
            // A hairdresser's in what is mapped as a house, and a kiosk out in the street.
            (use_of(&[("shop", "hairdresser")]), [126.0, 4.0]),
            (use_of(&[("shop", "kiosk")]), [30.0, 6.0]),
        ];
        let (kinds, tally) = classify(&sites, &[], &nodes);
        assert_eq!(kinds, [BUILDING_SHOP, BUILDING_PUBLIC, BUILDING_FIRE_STATION, BUILDING_FARMHOUSE], "and the house stays a house");
        assert_eq!(tally.by_node, 3);
        assert_eq!(by_use(&tags(&[("amenity", "bench")])), None);
        assert_eq!(by_use(&tags(&[("shop", "vacant")])), None);
    }

    #[test]
    fn a_long_plain_building_that_is_the_biggest_of_a_farms_yard_is_its_barn() {
        // A farmhouse 12 × 9 m, a barn 24 × 10 m across the yard and a shed 5 × 4 m.
        let farm = [rect(0.0, 0.0, 12.0, 9.0), rect(-5.0, 25.0, 24.0, 10.0), rect(20.0, 2.0, 5.0, 4.0)];
        assert_eq!(kinds(&farm, &[]), [BUILDING_PLAIN, BUILDING_BARN, BUILDING_OUTBUILDING]);
        // With no house beside it, it is a barn by the fields it stands at.
        let alone = [rect(-5.0, 25.0, 24.0, 10.0)];
        assert_eq!(kinds(&alone, &[]), [BUILDING_PLAIN]);
        assert_eq!(kinds(&alone, &[(Land::Farm, rect(30.0, 0.0, 200.0, 200.0))]), [BUILDING_BARN]);
    }

    #[test]
    fn a_long_building_in_a_village_or_on_residential_land_is_no_barn() {
        // The same farm, on land mapped as residential.
        let farm = [rect(0.0, 0.0, 12.0, 9.0), rect(-5.0, 25.0, 24.0, 10.0)];
        assert_eq!(kinds(&farm, &[(Land::Residential, rect(-100.0, -100.0, 300.0, 300.0))]), [BUILDING_PLAIN, BUILDING_PLAIN]);
        // And with twenty houses round it, 30 m apart.
        let mut village = farm.to_vec();
        village.extend((0..20).map(|i| rect(-130.0 + 30.0 * f64::from(i % 10), -40.0 - 30.0 * f64::from(i / 10), 12.0, 9.0)));
        assert_eq!(kinds(&village, &[])[1], BUILDING_PLAIN);
        // An L-shaped one that long is a house with a wing.
        let wing = vec![[0.0, 30.0], [24.0, 30.0], [24.0, 50.0], [14.0, 50.0], [14.0, 40.0], [0.0, 40.0]];
        assert_eq!(kinds(&[rect(0.0, 0.0, 12.0, 9.0), wing], &[])[1], BUILDING_PLAIN);
    }

    #[test]
    fn a_small_building_alone_in_a_field_is_a_hay_barn_and_one_alone_in_the_woods_a_cabin() {
        let lada = [rect(0.0, 0.0, 8.0, 6.0)];
        assert_eq!(kinds(&lada, &[(Land::Farm, rect(-100.0, -100.0, 200.0, 200.0))]), [BUILDING_BARN]);
        assert_eq!(kinds(&lada, &[]), [BUILDING_CABIN]);
        // In a village the same small house is a house.
        let mut village = lada.to_vec();
        village.extend((0..6).map(|i| rect(-90.0 + 30.0 * f64::from(i), 60.0, 12.0, 9.0)));
        assert_eq!(kinds(&village, &[])[0], BUILDING_PLAIN);
    }

    #[test]
    fn a_small_building_is_an_outbuilding_only_beside_a_bigger_one() {
        assert_eq!(kinds(&[rect(0.0, 0.0, 6.0, 5.0)], &[]), [BUILDING_CABIN], "alone it is a cabin");
        assert_eq!(kinds(&[rect(0.0, 0.0, 6.0, 5.0), rect(15.0, 0.0, 12.0, 9.0)], &[]), [BUILDING_OUTBUILDING, BUILDING_PLAIN]);
        assert_eq!(kinds(&[rect(0.0, 0.0, 6.0, 5.0), rect(15.0, 0.0, 5.0, 4.0)], &[]), [BUILDING_CABIN, BUILDING_OUTBUILDING], "of two the smaller");
    }

    #[test]
    fn the_land_a_building_stands_on_says_what_an_untagged_one_is() {
        let industry = [(Land::Industrial, rect(-50.0, -50.0, 200.0, 200.0))];
        assert_eq!(kinds(&[rect(0.0, 0.0, 40.0, 20.0), rect(60.0, 0.0, 10.0, 9.0), rect(60.0, 30.0, 5.0, 4.0)], &industry), [BUILDING_HALL, BUILDING_PLAIN, BUILDING_OUTBUILDING]);
        let trade = [(Land::Trade, rect(-50.0, -50.0, 200.0, 200.0))];
        assert_eq!(kinds(&[rect(0.0, 0.0, 30.0, 20.0), rect(60.0, 0.0, 12.0, 9.0)], &trade), [BUILDING_SHOP, BUILDING_PLAIN]);
        assert_eq!(kinds(&[rect(0.0, 0.0, 4.0, 3.0)], &[(Land::Allotments, rect(-50.0, -50.0, 200.0, 200.0))]), [BUILDING_OUTBUILDING]);
        // The smallest area round it counts: a works' yard inside a residential district.
        let nested = [(Land::Residential, rect(-500.0, -500.0, 1000.0, 1000.0)), industry[0].clone()];
        assert_eq!(kinds(&[rect(0.0, 0.0, 40.0, 20.0)], &nested), [BUILDING_HALL]);
    }

    #[test]
    fn the_tally_counts_each_kind_and_what_it_was_known_by() {
        let (barn, yes) = (tags(&[("building", "barn")]), tags(&[("building", "yes")]));
        let rings = [rect(0.0, 0.0, 12.0, 9.0), rect(20.0, 0.0, 5.0, 4.0), rect(200.0, 0.0, 20.0, 10.0)];
        let sites: Vec<(&Tags, &[P])> = vec![(&yes, &rings[0]), (&yes, &rings[1]), (&barn, &rings[2])];
        let (kinds, tally) = classify(&sites, &[], &[]);
        assert_eq!(kinds, [BUILDING_FARMHOUSE, BUILDING_OUTBUILDING, BUILDING_BARN]);
        assert_eq!(tally, Tally { by_tags: 1, by_node: 0, by_land: 0, by_yard: 1, kinds: [0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0] });
    }
}
