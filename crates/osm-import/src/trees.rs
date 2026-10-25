//! Street and park trees from municipal open data: Umeå kommun's
//! (`trad-som-forvaltas-av-gator-och-parker`, ~32 000 trees with species and position) and the
//! Ville de Paris's (`les-arbres`, ~220 000 trees with species, height and position; Licence
//! ODbL). Which one a world uses follows from where it is (`Register::over`).
//!
//! Each species is mapped to one of a few archetypes (`TREE_*`). Umeå's data has no size, so the
//! archetype's typical size is varied a little per tree, deterministically from its id, so a
//! re-import gives the same forest; Paris's gives the height.

use anyhow::{Context, Result};
use serde::Deserialize;
use shared::world::*;

const ENDPOINT: &str = "https://opendata.umea.se/api/explore/v2.1/catalog/datasets/trad-som-forvaltas-av-gator-och-parker/exports/json";
/// The year ages are counted from, so an import does not depend on the date it runs.
const REFERENCE_YEAR: i32 = 2025;
/// A tree planted this many years ago or less is still smaller than a grown one.
const YEARS_TO_GROW: f64 = 15.0;

#[derive(Debug, Deserialize)]
struct Point {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct Record {
    id: Option<u64>,
    geo_point_2d: Option<Point>,
    #[serde(rename = "lov_eller_barrtrad_1_1_1")]
    broadleaf_or_conifer: Option<String>,
    #[serde(rename = "tradart_vetenskap_namn_1_1_2")]
    species: Option<String>,
    #[serde(rename = "planteringsdatum_6_1_1")]
    planted: Option<String>,
}

/// A tree as imported, before it is placed in the local frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planted {
    pub lat: f64,
    pub lon: f64,
    pub kind: u8,
    /// Metres.
    pub height: f64,
    /// Crown radius, metres.
    pub crown: f64,
    /// From the register of planted trees (as opposed to found in the laser data).
    pub registered: bool,
}

/// A municipal tree register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Umea,
    Paris,
}

/// Paris intra-muros, `[south, west, north, east]`, with the Bois de Boulogne and de Vincennes.
const PARIS: [f64; 4] = [48.81, 2.22, 48.91, 2.47];

impl Register {
    /// The register that covers `bbox` in `region`, if any.
    pub fn over(region: crate::region::Region, [south, west, north, east]: [f64; 4]) -> Option<Register> {
        match region {
            crate::region::Region::Sweden => Some(Register::Umea),
            crate::region::Region::France => {
                let [s, w, n, e] = PARIS;
                (south < n && s < north && west < e && w < east).then_some(Register::Paris)
            }
        }
    }

    /// The trees in each of `pieces`, merged: a tree on a shared border is kept once.
    pub fn fetch_pieces(self, pieces: &[[f64; 4]]) -> Result<Vec<Planted>> {
        match self {
            Register::Umea => fetch_pieces(pieces),
            Register::Paris => {
                let mut seen = std::collections::HashSet::new();
                let mut all = Vec::new();
                for &piece in pieces {
                    all.extend(fetch_paris(piece)?.into_iter().filter(|t| seen.insert((t.lat.to_bits(), t.lon.to_bits()))));
                }
                Ok(all)
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Register::Umea => "Umeå open data",
            Register::Paris => "Paris open data",
        }
    }
}

const PARIS_ENDPOINT: &str = "https://opendata.paris.fr/api/explore/v2.1/catalog/datasets/les-arbres/exports/json";

#[derive(Debug, Deserialize)]
struct ParisRecord {
    idbase: Option<u64>,
    geo_point_2d: Option<Point>,
    genre: Option<String>,
    espece: Option<String>,
    /// Metres; 0 where nobody measured.
    hauteurenm: Option<f64>,
}

/// Every tree of the Paris register in `[south, west, north, east]`.
pub fn fetch_paris(bbox: [f64; 4]) -> Result<Vec<Planted>> {
    let filter = filter(bbox);
    let text = crate::cache::text("trees-paris", &filter, || {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(client
            .get(PARIS_ENDPOINT)
            .query(&[("where", filter.as_str()), ("select", "idbase,geo_point_2d,genre,espece,hauteurenm")])
            .send()
            .context("tree request failed")?
            .error_for_status()
            .context("the Paris tree API returned an error")?
            .text()?)
    })?;
    let records: Vec<ParisRecord> = serde_json::from_str(&text).context("the Paris tree API returned invalid JSON")?;
    Ok(records.into_iter().filter_map(plant_paris).collect())
}

/// Tallest height in the Paris register taken as measured, metres; above it is a typing error.
const PARIS_MAX_HEIGHT: f64 = 45.0;

fn plant_paris(record: ParisRecord) -> Option<Planted> {
    let at = record.geo_point_2d?;
    let name = format!("{} {}", record.genre.as_deref().unwrap_or(""), record.espece.as_deref().unwrap_or(""));
    // Conifers are named by genus; anything else unknown is broadleaf, as most of Paris's trees are.
    let kind = classify(Some(name.trim()), Some("Lövträd"));
    let kind = if kind == TREE_BIRCH && !name.to_lowercase().starts_with("betula") { TREE_BROADLEAF } else { kind };
    let (typical, crown) = size(kind, record.idbase.unwrap_or(0), None);
    let height = record.hauteurenm.filter(|h| (1.0..=PARIS_MAX_HEIGHT).contains(h)).unwrap_or(typical);
    // The crown in proportion to the archetype's, by height.
    Some(Planted { lat: at.lat, lon: at.lon, kind, height, crown: crown * (height / typical).clamp(0.3, 2.0), registered: true })
}

/// The register's query for a box: also the cache key.
pub fn filter([south, west, north, east]: [f64; 4]) -> String {
    format!("in_bbox(geo_point_2d, {south}, {west}, {north}, {east})")
}

/// The trees in each of `pieces`, merged: a tree on a shared border is kept once.
pub fn fetch_pieces(pieces: &[[f64; 4]]) -> Result<Vec<Planted>> {
    let mut seen = std::collections::HashSet::new();
    let mut all = Vec::new();
    for &piece in pieces {
        all.extend(fetch(piece)?.into_iter().filter(|t| seen.insert((t.lat.to_bits(), t.lon.to_bits(), t.kind))));
    }
    Ok(all)
}

/// Every tree in `[south, west, north, east]`.
pub fn fetch(bbox: [f64; 4]) -> Result<Vec<Planted>> {
    let filter = filter(bbox);
    let text = crate::cache::text("trees", &filter, || {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(client
            .get(ENDPOINT)
            .query(&[
                ("where", filter.as_str()),
                ("select", "id,geo_point_2d,lov_eller_barrtrad_1_1_1,tradart_vetenskap_namn_1_1_2,planteringsdatum_6_1_1"),
            ])
            .send()
            .context("tree request failed")?
            .error_for_status()
            .context("the tree API returned an error")?
            .text()?)
    })?;
    let records: Vec<Record> = serde_json::from_str(&text).context("the tree API returned invalid JSON")?;
    Ok(records.into_iter().filter_map(plant).collect())
}

fn plant(record: Record) -> Option<Planted> {
    let at = record.geo_point_2d?;
    let kind = classify(record.species.as_deref(), record.broadleaf_or_conifer.as_deref());
    let (height, crown) = size(kind, record.id.unwrap_or(0), record.planted.as_deref());
    Some(Planted { lat: at.lat, lon: at.lon, kind, height, crown, registered: true })
}

/// The register's trees with the laser's: a detected tree within `max(2 m, its crown)` of a
/// register tree is that tree, keeping the register's species and the laser's measured size; the
/// rest of the detected trees are added, classified from the laser alone. Detections are in
/// `region`'s projected coordinates.
pub fn merge(region: crate::region::Region, register: Vec<Planted>, detected: &[crate::canopy::Detected]) -> Vec<Planted> {
    const BIN: f64 = 8.0;
    let bin = |e: f64, n: f64| ((e / BIN).floor() as i64, (n / BIN).floor() as i64);
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, tree) in detected.iter().enumerate() {
        grid.entry(bin(tree.e, tree.n)).or_default().push(i);
    }
    let mut taken = vec![false; detected.len()];
    let mut out = Vec::with_capacity(register.len() + detected.len());
    for mut tree in register {
        let (e, n) = region.source(tree.lat, tree.lon);
        let (bx, by) = bin(e, n);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in grid.get(&(bx + dx, by + dy)).map(Vec::as_slice).unwrap_or_default() {
                    let d = (detected[i].e - e).hypot(detected[i].n - n);
                    if d <= detected[i].crown.max(2.0) && best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, i));
                    }
                }
            }
        }
        if let Some((_, i)) = best {
            // A crown the laser sees merged with neighbours is not a better size than the species'.
            taken[i] = true;
            tree.height = detected[i].height;
            tree.crown = detected[i].crown;
        }
        out.push(tree);
    }
    for (found, taken) in detected.iter().zip(taken) {
        if !taken {
            let (lat, lon) = region.degrees(found.e, found.n);
            let kind = match region {
                crate::region::Region::Sweden => crate::canopy::classify(found),
                crate::region::Region::France => crate::canopy::classify_france(found),
            };
            out.push(Planted { lat, lon, kind, height: found.height, crown: found.crown, registered: false });
        }
    }
    out
}

/// The archetype for a scientific name (cultivar and clone suffixes included), falling back on
/// the data's broadleaf/conifer column when the species is missing ("Saknas") or unknown.
pub fn classify(species: Option<&str>, broadleaf_or_conifer: Option<&str>) -> u8 {
    let name = species.unwrap_or("").to_lowercase();
    // Narrow cultivars first: a columnar aspen or oak is a column whatever its genus.
    if ["erecta", "fastigiata", "pyramidalis", "columnar"].iter().any(|word| name.contains(word)) {
        return TREE_COLUMNAR;
    }
    let genus = name.split_whitespace().next().unwrap_or("");
    match genus {
        "betula" => TREE_BIRCH,
        "pinus" => TREE_PINE,
        "picea" | "abies" | "larix" | "pseudotsuga" | "juniperus" | "thuja" | "taxus" => TREE_SPRUCE,
        "sorbus" | "prunus" | "malus" | "syringa" | "crataegus" | "pyrus" | "cotoneaster" | "amelanchier" | "laburnum" | "cerasus" => TREE_ORNAMENTAL,
        "acer" | "tilia" | "quercus" | "aesculus" | "fraxinus" | "ulmus" | "fagus" | "carpinus" | "platanus" => TREE_BROADLEAF,
        "populus" => TREE_COLUMNAR,
        "salix" | "alnus" => TREE_LOWWIDE,
        _ => match broadleaf_or_conifer {
            Some(group) if group.to_lowercase().starts_with("barr") => TREE_SPRUCE,
            // Birch is by far the most common tree here, so it is the best guess.
            _ => TREE_BIRCH,
        },
    }
}

/// `(height, crown radius)` in metres: the archetype's grown size, varied ±20 % by `id`, and
/// scaled down while the tree is young.
pub fn size(kind: u8, id: u64, planted: Option<&str>) -> (f64, f64) {
    let (height, crown) = match kind {
        TREE_PINE => (16.0, 2.5),
        TREE_SPRUCE => (14.0, 2.2),
        TREE_ORNAMENTAL => (6.0, 2.0),
        TREE_BROADLEAF => (14.0, 5.0),
        TREE_COLUMNAR => (16.0, 1.5),
        TREE_LOWWIDE => (8.0, 3.5),
        _ => (12.0, 2.2),
    };
    let jitter = 0.8 + 0.4 * unit(id);
    let growth = planted
        .and_then(|date| date.get(..4)?.parse::<i32>().ok())
        .map(|year| (f64::from(REFERENCE_YEAR - year) / YEARS_TO_GROW).clamp(0.0, 1.0))
        .map_or(1.0, |age| 0.4 + 0.6 * age);
    (height * jitter * growth, crown * jitter * growth)
}

/// A stable pseudo-random value in `[0, 1)` from `id` (splitmix64).
fn unit(id: u64) -> f64 {
    let mut z = id.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_map_to_archetypes() {
        let kind = |name| classify(Some(name), Some("Lövträd"));
        assert_eq!(kind("Betula pendula"), TREE_BIRCH);
        assert_eq!(kind("Betula pendula fk SAVOLAKSIE"), TREE_BIRCH);
        assert_eq!(kind("Pinus sylvestris"), TREE_PINE);
        assert_eq!(kind("Picea pungens Glauca-Gruppen"), TREE_SPRUCE);
        assert_eq!(kind("Larix sibirica"), TREE_SPRUCE);
        assert_eq!(kind("Sorbus ulleungensis 'Dodong'"), TREE_ORNAMENTAL);
        assert_eq!(kind("Acer platanoides fk ULTUNA"), TREE_BROADLEAF);
        assert_eq!(kind("Populus tremula"), TREE_COLUMNAR);
        assert_eq!(kind("Populus tremula 'Erecta'"), TREE_COLUMNAR);
        assert_eq!(kind("Quercus robur Fastigiata-Gruppen"), TREE_COLUMNAR);
        assert_eq!(kind("Salix alba"), TREE_LOWWIDE);
        assert_eq!(kind("Alnus incana 'Laciniata'"), TREE_LOWWIDE);
    }

    #[test]
    fn missing_species_fall_back_on_the_group() {
        assert_eq!(classify(Some("Saknas"), Some("Barrträd")), TREE_SPRUCE);
        assert_eq!(classify(None, Some("Barrträd")), TREE_SPRUCE);
        assert_eq!(classify(Some("Saknas"), Some("Lövträd")), TREE_BIRCH);
        assert_eq!(classify(None, None), TREE_BIRCH);
    }

    #[test]
    fn sizes_are_stable_bounded_and_grow_with_age() {
        assert_eq!(size(TREE_BIRCH, 7, None), size(TREE_BIRCH, 7, None));
        for id in 0..200 {
            let (height, crown) = size(TREE_BIRCH, id, None);
            assert!((9.6..=14.4).contains(&height), "height {height}");
            assert!((1.76..=2.64).contains(&crown), "crown {crown}");
        }
        let young = size(TREE_BIRCH, 7, Some("2024-07-01")).0;
        let old = size(TREE_BIRCH, 7, Some("1990-05-01")).0;
        assert!(young < old * 0.6, "young {young}, old {old}");
        assert_eq!(old, size(TREE_BIRCH, 7, None).0);
    }

    #[test]
    fn records_decode() {
        let json = r#"[{"id":103017,"geo_point_2d":{"lon":20.23,"lat":63.83},"lov_eller_barrtrad_1_1_1":"Barrträd","tradart_vetenskap_namn_1_1_2":"Pinus sylvestris","planteringsdatum_6_1_1":null},
                       {"id":1,"geo_point_2d":null,"lov_eller_barrtrad_1_1_1":null,"tradart_vetenskap_namn_1_1_2":null,"planteringsdatum_6_1_1":null}]"#;
        let records: Vec<Record> = serde_json::from_str(json).unwrap();
        let trees: Vec<Planted> = records.into_iter().filter_map(plant).collect();
        assert_eq!(trees.len(), 1, "a record without a position is dropped");
        assert_eq!(trees[0].kind, TREE_PINE);
    }

    #[test]
    fn laser_trees_join_the_register_without_doubling() {
        use crate::canopy::Detected;
        let (lat, lon) = (63.83, 20.17);
        let (e, n) = crate::dem::sweref99tm(lat, lon);
        let found = |de: f64, dn: f64| Detected { e: e + de, n: n + dn, height: 17.0, crown: 3.0, multi: 0.8, intensity: 650.0, base: 0.45 };
        let register = vec![Planted { lat, lon, kind: TREE_PINE, height: 16.0, crown: 2.5, registered: true }];
        let merged = merge(crate::region::Region::Sweden, register, &[found(1.0, 0.5), found(30.0, 0.0)]);
        assert_eq!(merged.len(), 2);
        // The register's species with the laser's size.
        assert_eq!((merged[0].kind, merged[0].height, merged[0].crown), (TREE_PINE, 17.0, 3.0));
        assert!(merged[0].registered && !merged[1].registered);
        // The second is classified from the laser alone and lands where it was seen.
        let (back_e, _) = crate::dem::sweref99tm(merged[1].lat, merged[1].lon);
        assert!((back_e - (e + 30.0)).abs() < 0.01);
    }
}
