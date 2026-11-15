//! Thinning the woods before they are cut into tiles, so the client draws fewer trees that a
//! player would not miss. Every tree's crown overdraws its neighbours' (foliage is alpha-tested,
//! so overlapping crowns are paid for pixel by pixel); what goes is what stands in another
//! tree's crown: the understorey under a taller one, and one of two equals whose crowns are
//! nearly one. The canopy's outline, its gaps and its tallest trees stay as the laser saw them.
//! Trees pressed against a house go in `assemble`, which has the walls (`clear_of_walls`).

use shared::world::*;

type P = [f64; 2];

/// No trunk stands closer than this to a building's wall, metres.
pub const TRUNK_CLEARANCE: f64 = 1.5;
/// A tree no taller than this share of a neighbour whose crown its own crown touches is
/// suppressed: it stands in the shade of the taller one, and is dropped.
pub const SUPPRESSED: f64 = 0.75;
/// Two trees of any heights whose trunks stand closer than this share of their two crown radii
/// added are crowded: their crowns are nearly one, and the shorter is dropped.
pub const CROWDED: f64 = 0.6;

/// A tree in the importer: `(position, TREE_*, height, crown radius, planted)`, metres.
pub type Planted = (P, u8, f64, f64, bool);

/// Whether `kind` is a tree rather than a rock, a shrub or a flag.
pub fn is_tree(kind: u8) -> bool {
    kind <= TREE_LOWWIDE
}

/// Whether a tree of `crown` radius with its trunk `wall` metres from a building's wall stands far
/// enough from it: its trunk at least `TRUNK_CLEARANCE` away, and its crown not reaching the wall.
pub fn clear_of_walls(wall: f64, crown: f64) -> bool {
    wall >= TRUNK_CLEARANCE.max(crown)
}

/// For each of `trees`, whether it is kept: the tallest first, each tree is kept unless a kept one
/// is crowding it, or is so much taller and so near that it is suppressed (`SUPPRESSED`,
/// `CROWDED`). Planted trees, from a register, are always kept, and so are rocks, shrubs and flags,
/// which are not trees.
pub fn woods(trees: &[Planted]) -> Vec<bool> {
    thinned(trees, SUPPRESSED, CROWDED)
}

/// For each of `trees`, whether it is kept when trees no taller than `suppressed` of a kept
/// neighbour with touching crowns, and trees nearer a kept neighbour than `crowded` of their crowns
/// added, are dropped.
fn thinned(trees: &[Planted], suppressed: f64, crowded: f64) -> Vec<bool> {
    // Bins as wide as two of the widest crowns, so a tree is only measured against those of its
    // own bin and the eight round it.
    let bin_size = 2.0 * trees.iter().filter(|t| is_tree(t.1)).map(|t| t.3).fold(0.5, f64::max);
    let bin = |p: P| ((p[0] / bin_size).floor() as i64, (p[1] / bin_size).floor() as i64);
    let mut order: Vec<usize> = (0..trees.len()).filter(|&i| is_tree(trees[i].1)).collect();
    // Planted trees first, as they are kept whatever stands round them; then the tallest. The
    // index breaks ties, so the same trees always come out.
    order.sort_by(|&a, &b| trees[b].4.cmp(&trees[a].4).then(trees[b].2.total_cmp(&trees[a].2)).then(a.cmp(&b)));
    let mut keep = vec![true; trees.len()];
    let mut kept: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for i in order {
        let (at, _, height, crown, planted) = trees[i];
        let (bx, bz) = bin(at);
        let shaded = !planted
            && (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (bx + dx, bz + dz))).filter_map(|key| kept.get(&key)).flatten().any(|&j| {
                let (over, _, over_height, over_crown, _) = trees[j];
                let (d, reach) = ((over[0] - at[0]).hypot(over[1] - at[1]), over_crown + crown);
                d < crowded * reach || (height <= suppressed * over_height && d < reach)
            });
        if shaded {
            keep[i] = false;
        } else {
            kept.entry((bx, bz)).or_default().push(i);
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thin(trees: Vec<Planted>) -> Vec<Planted> {
        let keep = woods(&trees);
        trees.into_iter().zip(keep).filter_map(|(tree, keep)| keep.then_some(tree)).collect()
    }

    fn tree(x: f64, z: f64, height: f64, crown: f64) -> Planted {
        ([x, z], TREE_SPRUCE, height, crown, false)
    }

    #[test]
    fn a_small_tree_whose_crown_touches_a_much_taller_one_is_dropped() {
        let kept = thin(vec![tree(5.0, 0.0, 8.0, 2.0), tree(0.0, 0.0, 20.0, 4.0)]);
        assert_eq!(kept, vec![tree(0.0, 0.0, 20.0, 4.0)]);
    }

    #[test]
    fn a_small_tree_in_a_gap_or_nearly_as_tall_is_kept() {
        let gap = vec![tree(0.0, 0.0, 20.0, 4.0), tree(6.5, 0.0, 8.0, 2.0)];
        assert_eq!(thin(gap.clone()), gap);
        let nearly = vec![tree(0.0, 0.0, 20.0, 4.0), tree(5.0, 0.0, 16.0, 2.0)];
        assert_eq!(thin(nearly.clone()), nearly);
    }

    #[test]
    fn of_two_equals_with_crowns_nearly_one_the_shorter_goes() {
        let kept = thin(vec![tree(0.0, 0.0, 15.0, 3.0), tree(2.5, 0.0, 16.0, 3.0)]);
        assert_eq!(kept, vec![tree(2.5, 0.0, 16.0, 3.0)]);
    }

    #[test]
    fn a_dropped_tree_does_not_drop_others() {
        // The middle tree goes under the tall one; the far small one touched only the middle one.
        let woods = thin(vec![tree(0.0, 0.0, 20.0, 4.0), tree(5.0, 0.0, 10.0, 2.0), tree(8.0, 0.0, 6.0, 1.5)]);
        assert_eq!(woods, vec![tree(0.0, 0.0, 20.0, 4.0), tree(8.0, 0.0, 6.0, 1.5)]);
    }

    #[test]
    fn planted_trees_rocks_and_shrubs_are_kept() {
        let under = vec![
            tree(0.0, 0.0, 20.0, 4.0),
            ([4.0, 0.0], TREE_BIRCH, 5.0, 2.0, true),
            ([0.0, 1.0], TREE_ROCK_BOULDER, 1.0, 1.0, false),
            ([-1.0, 0.0], TREE_BUSH, 1.5, 1.0, false),
        ];
        assert_eq!(thin(under.clone()), under);
    }

    #[test]
    fn a_trunk_keeps_off_the_wall_and_its_crown_does_not_reach_it() {
        assert!(!clear_of_walls(1.4, 1.0), "the trunk is too near");
        assert!(!clear_of_walls(2.5, 3.0), "the crown reaches over the wall");
        assert!(clear_of_walls(1.5, 1.0));
        assert!(clear_of_walls(3.0, 3.0));
    }

    /// What the thinning would take out of a built world's trees:
    /// `WORLD=home cargo test --release -p osm-import thinning -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn thinning() {
        let name = std::env::var("WORLD").unwrap();
        let dir = std::path::PathBuf::from("../../worlds").join(&name);
        let manifest = Manifest::decode(&std::fs::read(dir.join("world.msgpack")).unwrap()).unwrap();
        let mut trees = Vec::new();
        for entry in &manifest.tiles {
            let tile = Tile::decode(&std::fs::read(dir.join(entry.path())).unwrap()).unwrap();
            let m = |v: i32| f64::from(v) / 100.0;
            trees.extend(tile.trees.iter().filter(|t| is_tree(t.kind)).map(|t| ([m(t.x), m(t.z)], t.kind, m(t.height), m(t.crown), false)));
        }
        let pixels = |keep: &[bool]| trees.iter().zip(keep).filter(|(_, k)| **k).map(|(t, _)| t.3 * t.3 * t.2).sum::<f64>();
        let all = pixels(&vec![true; trees.len()]);
        for (suppressed, crowded) in [(SUPPRESSED, CROWDED), (0.0, CROWDED), (SUPPRESSED, 0.0)] {
            let keep = thinned(&trees, suppressed, crowded);
            let dropped = keep.iter().filter(|&&k| !k).count();
            eprintln!("{name}: suppressed {suppressed}, crowded {crowded}: {dropped} of {} trees dropped, {:.0}% of the foliage", trees.len(), 100.0 * (1.0 - pixels(&keep) / all));
        }
    }
}
