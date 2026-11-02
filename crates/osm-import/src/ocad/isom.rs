//! What the symbols of an orienteering map are in the world, by the numbers of ISOM 2017.
//!
//! The numbers belong to the standard, but older maps use other ones (a boulder is 206 in ISOM 2000),
//! so `check` reads the map's own symbol names before the table is trusted. The decimals are the
//! mapper's own variants of a symbol; only a few are told apart.

use super::file::Map;
use anyhow::{Result, bail};
use shared::world::*;

/// One rock or shrub. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stone {
    pub kind: u8,
    pub height: f64,
    pub crown: f64,
    /// How many stand around the mapped point, and how far from it.
    pub count: u32,
    pub spread: f64,
    /// Whether it stops a soldier: stony ground is something to walk over.
    pub solid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feature {
    Stone(Stone),
    Road(Track),
    Cover(u8),
    /// Green: a cover, filled with shrubs.
    Thicket(u8, Shrubs),
    Building,
    /// A small knoll or pit, for the ground to show.
    Landform(Form),
    Cliff(Ledge),
    /// A fence or a power line.
    Line(Wire),
}

/// A fence or a power line. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wire {
    /// `LINE_*`.
    pub kind: u8,
    /// Width of the strip kept clear of trees under it; 0 for none.
    pub cleared: f64,
}

const fn wire(kind: u8, cleared: f64) -> Feature {
    Feature::Line(Wire { kind, cleared })
}

/// A cliff. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ledge {
    pub height: f64,
    /// Over this width the face climbs: 1 m is too steep for anyone, 2 m is a scramble.
    pub face: f64,
    /// Width of the bare rock along it.
    pub rock: f64,
}

const fn ledge(height: f64, face: f64, rock: f64) -> Feature {
    Feature::Cliff(Ledge { height, face, rock })
}

/// A road or path. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub width: f64,
    /// `ROAD_*`.
    pub kind: u8,
    /// Width of the line cut through the trees for it, past the road itself; 0 for none.
    pub cleared: f64,
}

const fn track(width: f64, kind: u8, cleared: f64) -> Feature {
    Feature::Road(Track { width, kind, cleared })
}

/// A knoll or pit. Metres. A little over the map's minimums, so that they show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Form {
    pub along: f64,
    pub across: f64,
    /// Height, or depth when negative.
    pub target: f64,
    pub power: f64,
}

const fn form(along: f64, across: f64, target: f64, power: f64) -> Feature {
    Feature::Landform(Form { along, across, target, power })
}

/// The shrubs of a thicket. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shrubs {
    /// One to this many square metres.
    pub per_area: f64,
    pub height: f64,
    pub crown: f64,
}

const fn thicket(cover: u8, per_area: f64, height: f64, crown: f64) -> Feature {
    Feature::Thicket(cover, Shrubs { per_area, height, crown })
}

const fn stone(kind: u8, height: f64, crown: f64, count: u32, spread: f64, solid: bool) -> Feature {
    Feature::Stone(Stone { kind, height, crown, count, spread, solid })
}

/// The feature of symbol `number` (times 1000), or `None` for what the map draws but the world takes
/// from elsewhere: contours, water, trees, or nothing it has yet.
pub fn feature(number: i32) -> Option<Feature> {
    Some(match number {
        204_001 => stone(TREE_ROCK_BOULDER, 1.7, 1.1, 1, 0.0, true),
        _ => match number / 1000 {
            204 => stone(TREE_ROCK_BOULDER, 1.3, 0.8, 1, 0.0, true),
            205 => stone(TREE_ROCK_BOULDER, 2.3, 1.7, 1, 0.0, true),
            206 => stone(TREE_ROCK_SLAB, 3.5, 2.9, 1, 0.0, true),
            207 => stone(TREE_ROCK_BOULDER, 1.0, 0.7, 4, 4.0, true),
            208 => stone(TREE_ROCK_SLAB, 0.6, 1.1, 3, 5.0, true),
            210 => stone(TREE_ROCK_BOULDER, 0.4, 0.4, 3, 3.0, false),
            417 => stone(TREE_BROADLEAF, 14.0, 5.0, 1, 0.0, true),
            418 => stone(TREE_BUSH, 1.4, 0.9, 1, 0.0, false),
            201 => ledge(3.0, 1.0, 3.0),
            202 => ledge(1.8, 2.0, 3.0),
            109 => form(3.0, 3.0, 1.5, 1.0),
            110 => form(4.0, 2.0, 1.3, 1.0),
            111 => form(2.5, 2.5, -1.5, 0.8),
            112 => form(2.0, 2.0, -1.8, 0.5),
            // ISOM's roads by what they are to whoever crosses them, not OSM's classes.
            502 => track(6.0, ROAD_MINOR, 0.0),
            503 => track(4.0, ROAD_GRAVEL, 0.0),
            504 => track(3.0, ROAD_TRACK, 0.0),
            505 => track(1.6, ROAD_PATH, 0.0),
            506 => track(1.0, ROAD_PATH, 0.0),
            507 => track(0.7, ROAD_FAINT, 0.0),
            // A ride or a line: a gap in the forest with a trace along it.
            508 => track(0.6, ROAD_FAINT, 3.0),
            // A power line runs down a cleared lane, wider for the big ones. A ruined fence is left out.
            510 => wire(LINE_POWER, 14.0),
            511 => wire(LINE_POWER_MAJOR, 40.0),
            516 => wire(LINE_FENCE, 0.0),
            518 => wire(LINE_FENCE_HIGH, 0.0),
            213 => Feature::Cover(COVER_SAND),
            // Marsh is drawn over whatever grows on it.
            307 => Feature::Cover(COVER_MARSH_DEEP),
            308 | 309 => Feature::Cover(COVER_MARSH),
            310 => Feature::Cover(COVER_MARSH_FAINT),
            401..=404 => Feature::Cover(COVER_PARK),
            // The greens by what they do to a runner. Tall shrubs hide you; the good-visibility
            // symbols are the same going but knee-high: brambles, heather, cut branches.
            406 => thicket(COVER_THICKET_SLOW, 25.0, 1.7, 1.0),
            407 => thicket(COVER_THICKET_SLOW, 18.0, 0.7, 0.8),
            408 => thicket(COVER_THICKET_WALK, 9.0, 1.9, 1.1),
            409 => thicket(COVER_THICKET_WALK, 7.0, 0.8, 0.9),
            410 | 411 => thicket(COVER_THICKET_FIGHT, 3.0, 2.3, 1.2),
            412 => Feature::Cover(COVER_FARM),
            501 | 520 => Feature::Cover(COVER_URBAN),
            521 => Feature::Building,
            _ => return None,
        },
    })
}

/// The share of the trees that an area symbol leaves standing, or `None` where it does not clear them.
/// The laser finds what stood when it flew, or a stray sapling; the map says the land is open.
pub fn trees_kept(number: i32) -> Option<f64> {
    Some(match number {
        404_001 => 0.3,
        _ => match number / 1000 {
            213 | 401 | 403 | 412 => 0.0,
            402 | 404 => 0.15,
            _ => return None,
        },
    })
}

/// Refuses a map whose symbol 204 is not a boulder: it was drawn to another standard, and the table
/// would turn its pits into rocks.
pub fn check(map: &Map) -> Result<()> {
    let Some(symbol) = map.symbols.get(&204_000) else { return Ok(()) };
    let name = symbol.description.to_lowercase();
    let boulder = ["sten", "boulder", "block", "stein", "kivi"].iter().any(|word| name.contains(word));
    if !boulder || name.contains("grop") || name.contains("pit") {
        bail!("symbol 204 is {:?}, not a boulder: the map is not ISOM 2017", symbol.description);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::file::Symbol;
    use super::*;

    #[test]
    fn variants_fall_back_on_their_symbol() {
        assert_eq!(feature(506_000), feature(506_002));
        assert_eq!(feature(508_010), feature(508_000));
        assert_eq!(feature(521_003), Some(Feature::Building));
        assert_ne!(feature(204_001), feature(204_000));
    }

    #[test]
    fn open_land_is_cleared_and_scattered_trees_thinned() {
        assert_eq!(trees_kept(401_000), Some(0.0));
        assert_eq!(trees_kept(403_000), Some(0.0));
        assert_eq!(trees_kept(402_000), Some(0.15));
        assert_eq!(trees_kept(404_001), Some(0.3));
        assert_eq!(trees_kept(408_000), None, "a thicket keeps its trees");
    }

    #[test]
    fn only_a_cliff_marked_impassable_stops_a_soldier() {
        let Some(Feature::Cliff(sheer)) = feature(201_002) else { panic!() };
        let Some(Feature::Cliff(broken)) = feature(202_001) else { panic!() };
        // `MAX_SLOPE_CLIMB_DEG` is 50°, a rise of 1.19; `crates/sim` tests these two shapes.
        assert!(sheer.height / sheer.face > 1.19 && broken.height / broken.face < 1.19);
    }

    #[test]
    fn roads_narrow_down_the_standard() {
        let widths: Vec<f64> = (502..=507).map(|number| match feature(number * 1000) {
            Some(Feature::Road(track)) => track.width,
            other => panic!("{other:?}"),
        }).collect();
        assert!(widths.windows(2).all(|pair| pair[0] > pair[1]), "{widths:?}");
        let Some(Feature::Road(ride)) = feature(508_000) else { panic!() };
        assert!(ride.cleared > ride.width);
    }

    #[test]
    fn the_darker_the_green_the_denser() {
        let shrubs = |number| match feature(number) {
            Some(Feature::Thicket(_, shrubs)) => shrubs,
            other => panic!("{other:?}"),
        };
        assert!(shrubs(406_000).per_area > shrubs(408_000).per_area && shrubs(408_000).per_area > shrubs(410_000).per_area);
        // Good visibility: as slow, but it hides nobody.
        assert!(shrubs(409_000).height < 1.0 && shrubs(408_000).height > 1.5);
    }

    #[test]
    fn marsh_comes_in_three_grades() {
        for number in [308_000, 308_001, 309_000] {
            assert_eq!(feature(number), Some(Feature::Cover(COVER_MARSH)), "{number}");
        }
        assert_eq!(feature(307_001), Some(Feature::Cover(COVER_MARSH_DEEP)));
        assert_eq!(feature(310_000), Some(Feature::Cover(COVER_MARSH_FAINT)));
    }

    #[test]
    fn fences_and_power_lines_are_lines_and_only_power_clears_the_forest() {
        assert_eq!(feature(516_000), Some(wire(LINE_FENCE, 0.0)));
        assert_eq!(feature(518_000), Some(wire(LINE_FENCE_HIGH, 0.0)));
        assert_eq!(feature(517_000), None, "a ruined fence");
        let Some(Feature::Line(minor)) = feature(510_000) else { panic!() };
        let Some(Feature::Line(major)) = feature(511_000) else { panic!() };
        assert!(major.cleared > minor.cleared && minor.cleared > 0.0);
    }

    #[test]
    fn contours_and_water_are_not_features() {
        for number in [101_000, 103_000, 301_001, 305_000, 405_000, 601_000] {
            assert_eq!(feature(number), None, "{number}");
        }
    }

    fn named(description: &str) -> Map {
        let mut map = Map::default();
        map.symbols.insert(204_000, Symbol { number: 204_000, kind: 1, description: description.into() });
        map
    }

    #[test]
    fn another_standard_is_refused() {
        assert!(check(&named("Sten")).is_ok());
        assert!(check(&named("Boulder")).is_ok());
        assert!(check(&named("Rocky pit")).is_err());
        assert!(check(&named("Klippgrop")).is_err());
        assert!(check(&Map::default()).is_ok());
    }
}
