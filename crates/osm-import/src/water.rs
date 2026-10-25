//! Turns Hydrografi's water (`hydro.rs`) into what the world stores: a water surface height at
//! every ground sample, with lakes and wide rivers carved into the ground under it, and stream
//! centre lines.
//!
//! The data has no surface height or depth. The height model is flat over water, so the surface
//! of a body is the median elevation of the height model inside it, and its bed is cut below that
//! with a depth that grows with the distance from the shore and with the size of the body.

use shared::terrain::Ground;
use shared::world::NO_WATER;

type P = [f64; 2];

/// Streams are drawn this wide, metres: the data has centre lines only.
pub const STREAM_WIDTH: f64 = 2.5;
/// Depth of the water at the shore, metres.
const SHORE_DEPTH: f64 = 0.3;
/// A bank sample next to water is left this far below the surface, centimetres, so the surface
/// mesh's interpolation crosses zero between the wet and the dry sample.
const BANK_DROP_CM: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Lake,
    River,
}

impl Kind {
    /// Depth of the deepest point of water whose widest part is `radius` metres from its bank.
    /// The data has no depths: bigger water is deeper (a pond about 2 m, a 100 m lake about 7 m).
    fn max_depth(self, radius: f64) -> f64 {
        match self {
            Kind::Lake => (1.0 + 0.6 * radius.sqrt()).clamp(1.5, 12.0),
            Kind::River => (0.8 + 0.35 * radius.sqrt()).clamp(1.2, 6.0),
        }
    }

    /// Bed depth at `shore` metres from the bank where the water reaches `radius` metres from it:
    /// a bowl that is `SHORE_DEPTH` at the bank and `max_depth` at `radius`.
    fn depth(self, shore: f64, radius: f64) -> f64 {
        let max = self.max_depth(radius);
        let t = if radius > 0.0 { (shore / radius).min(1.0) } else { 1.0 };
        SHORE_DEPTH + (max - SHORE_DEPTH) * (1.0 - (1.0 - t) * (1.0 - t))
    }
}

/// A body of water: one polygon, outer ring first and then holes, in local metres.
#[derive(Clone, Debug)]
pub struct Body {
    pub kind: Kind,
    pub rings: Vec<Vec<P>>,
    /// Surface elevation, metres, when the height model cannot tell (the sea, which it leaves out).
    pub level: Option<f64>,
}

/// All the water in the world, in local metres.
#[derive(Clone, Debug, Default)]
pub struct Map {
    pub bodies: Vec<Body>,
    /// Stream centre lines in the direction the water flows.
    pub streams: Vec<Vec<P>>,
}

/// What the tiles are cut from: the water surface at every ground sample of the whole grid
/// (empty when there is no water) and the stream centre lines.
#[derive(Clone, Debug, Default)]
pub struct Placed {
    pub levels: Vec<i32>,
    pub streams: Vec<Vec<P>>,
}

impl Placed {
    /// The finished water of `map` over `ground`, whose y = 0 is `datum` in the height model.
    pub fn new(map: &Map, surface: &Surface, datum: f64, ground: &Ground) -> Self {
        let levels = if surface.is_empty() { Vec::new() } else { surface.levels(datum, ground) };
        Self { levels, streams: surface.trim_streams(&map.streams) }
    }
}

/// The sample grid of the ground: position of sample `(0, 0)` and spacing in centimetres, samples a side.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub origin: i32,
    pub cell: i32,
    pub side: u32,
}

const NONE: u32 = u32::MAX;

/// Which body covers each ground sample, its surface and its depth.
pub struct Surface {
    grid: Grid,
    body: Vec<u32>,
    /// Surface elevation of each body, metres above the height model's datum.
    level: Vec<f64>,
    /// Bed depth under each sample, metres (0 on dry samples).
    depth: Vec<f32>,
}

impl Surface {
    /// Rasterises `map` onto `grid`. `elevation(x, z)` is the height model at a local position,
    /// metres, `None` where it has no data. Bodies it knows nothing about are dropped.
    pub fn new(map: &Map, grid: Grid, mut elevation: impl FnMut(f64, f64) -> Option<f64>) -> Self {
        let n = (grid.side * grid.side) as usize;
        let mut body = vec![NONE; n];
        let mut level = Vec::with_capacity(map.bodies.len());
        for (index, b) in map.bodies.iter().enumerate() {
            let mut cells = Vec::new();
            fill(&b.rings, grid, |cell| cells.push(cell));
            if let Some(fixed) = b.level {
                level.push(fixed);
                for cell in cells {
                    body[cell] = index as u32;
                }
                continue;
            }
            let mut heights: Vec<f64> = cells.iter().filter_map(|&c| elevation(grid.x(c as u32 % grid.side), grid.x(c as u32 / grid.side))).collect();
            if heights.is_empty() {
                level.push(f64::NAN);
                continue;
            }
            heights.sort_by(f64::total_cmp);
            let median = heights[heights.len() / 2];
            let spread = heights[heights.len() * 9 / 10] - heights[heights.len() / 10];
            if spread > 1.5 && cells.len() > 200 {
                eprintln!("water: a {} m² body has {spread:.1} m between its 10th and 90th percentile, drawn flat at {median:.1} m", cells.len() as f64 * (grid.cell as f64 / 100.0).powi(2));
            }
            level.push(median);
            for cell in cells {
                body[cell] = index as u32;
            }
        }
        // Distance to the nearest dry sample, in thirds of a cell: a chamfer transform.
        const FAR: u32 = u32::MAX / 2;
        let side = grid.side as usize;
        let mut dist: Vec<u32> = body.iter().map(|&b| if b == NONE { 0 } else { FAR }).collect();
        for j in 0..side {
            for i in 0..side {
                let at = j * side + i;
                let mut d = dist[at];
                if i > 0 {
                    d = d.min(dist[at - 1] + 3);
                }
                if j > 0 {
                    d = d.min(dist[at - side] + 3);
                    if i > 0 {
                        d = d.min(dist[at - side - 1] + 4);
                    }
                    if i + 1 < side {
                        d = d.min(dist[at - side + 1] + 4);
                    }
                }
                dist[at] = d;
            }
        }
        for j in (0..side).rev() {
            for i in (0..side).rev() {
                let at = j * side + i;
                let mut d = dist[at];
                if i + 1 < side {
                    d = d.min(dist[at + 1] + 3);
                }
                if j + 1 < side {
                    d = d.min(dist[at + side] + 3);
                    if i + 1 < side {
                        d = d.min(dist[at + side + 1] + 4);
                    }
                    if i > 0 {
                        d = d.min(dist[at + side - 1] + 4);
                    }
                }
                dist[at] = d;
            }
        }
        let cell_m = grid.cell as f64 / 100.0;
        // Metres from the bank of every wet sample (the first one is half a cell from it).
        let shore: Vec<f32> = (0..n)
            .map(|at| if body[at] == NONE { 0.0 } else { ((dist[at].min(FAR) as f64 / 3.0 - 0.5).max(0.0) * cell_m) as f32 })
            .collect();
        let width = local_radius(&body, &shore, side, cell_m);
        let depth = (0..n)
            .map(|at| match body[at] {
                NONE => 0.0,
                b => map.bodies[b as usize].kind.depth(shore[at] as f64, width[at] as f64) as f32,
            })
            .collect();
        Self { grid, body, level, depth }
    }

    fn index(&self, x: f64, z: f64) -> Option<usize> {
        let at = |v: f64| ((v * 100.0 - self.grid.origin as f64) / self.grid.cell as f64).round();
        let (i, j) = (at(x), at(z));
        let last = self.grid.side as f64 - 1.0;
        (i >= 0.0 && j >= 0.0 && i <= last && j <= last).then(|| j as usize * self.grid.side as usize + i as usize)
    }

    /// The ground elevation at a grid position given the height model's there: under water, the
    /// bed, cut `depth` below the surface whatever the model says (it is flat over open water, and
    /// on a bank sample inside the polygon it is higher than the surface).
    pub fn bed(&self, x: f64, z: f64, elevation: Option<f64>) -> Option<f64> {
        let Some(at) = self.index(x, z) else { return elevation };
        match self.body[at] {
            NONE => elevation,
            b => Some(self.level[b as usize] - self.depth[at] as f64),
        }
    }

    /// Whether the sample nearest `(x, z)` is under water.
    pub fn is_wet(&self, x: f64, z: f64) -> bool {
        self.index(x, z).is_some_and(|at| self.body[at] != NONE)
    }

    pub fn is_empty(&self) -> bool {
        self.body.iter().all(|&b| b == NONE)
    }

    /// The water surface at every ground sample of `ground` (built from `bed`), centimetres above
    /// its y = 0, which is `datum` in the height model. Samples next to water get a surface just
    /// under the bank, so the drawn shoreline falls between samples.
    pub fn levels(&self, datum: f64, ground: &Ground) -> Vec<i32> {
        let side = self.grid.side as usize;
        let mut out = vec![NO_WATER; side * side];
        for (at, &b) in self.body.iter().enumerate() {
            if b != NONE {
                out[at] = ((self.level[b as usize] - datum) * 100.0).round() as i32;
            }
        }
        let wet = out.clone();
        for j in 0..side {
            for i in 0..side {
                let at = j * side + i;
                if wet[at] != NO_WATER {
                    continue;
                }
                let mut near: Option<i32> = None;
                for dj in -1i32..=1 {
                    for di in -1i32..=1 {
                        let (ni, nj) = (i as i32 + di, j as i32 + dj);
                        if ni < 0 || nj < 0 || ni >= side as i32 || nj >= side as i32 {
                            continue;
                        }
                        let level = wet[nj as usize * side + ni as usize];
                        if level != NO_WATER {
                            near = Some(near.map_or(level, |n| n.min(level)));
                        }
                    }
                }
                if let Some(level) = near {
                    out[at] = level.min(ground.heights[at] - BANK_DROP_CM);
                }
            }
        }
        out
    }

    /// Stream centre lines without the vertices that lie in the middle of a lake or river (a
    /// vertex stays when a neighbour is dry, so a stream still reaches the shore).
    pub fn trim_streams(&self, streams: &[Vec<P>]) -> Vec<Vec<P>> {
        let mut out = Vec::new();
        for line in streams {
            let wet: Vec<bool> = line.iter().map(|p| self.is_wet(p[0], p[1])).collect();
            let mut run: Vec<P> = Vec::new();
            for (k, &p) in line.iter().enumerate() {
                let inside = wet[k] && k > 0 && wet[k - 1] && k + 1 < line.len() && wet[k + 1];
                if inside {
                    if run.len() >= 2 {
                        out.push(std::mem::take(&mut run));
                    }
                    run.clear();
                } else {
                    run.push(p);
                }
            }
            if run.len() >= 2 {
                out.push(run);
            }
        }
        out
    }
}

impl Grid {
    /// Local metres of sample index `i` along either axis.
    fn x(&self, i: u32) -> f64 {
        (self.origin + i as i32 * self.cell) as f64 / 100.0
    }
}

/// How far the water reaches from the bank in the neighbourhood of each wet sample, metres: the
/// radius of the widest circle that is inside the body and covers the sample, or nearly (a
/// bit more, so a bank sample at a corner is covered). A whole river is one polygon, narrow
/// upstream and wide at its mouth, so its depth must follow the width where you are, not the
/// body's widest part.
fn local_radius(body: &[u32], shore: &[f32], side: usize, cell_m: f64) -> Vec<f32> {
    let mut radius = shore.to_vec();
    let mut covered = vec![0.0f32; shore.len()];
    // The centres of the inscribed circles are the samples no neighbour is further from the bank than.
    let mut centres: Vec<usize> = (0..body.len())
        .filter(|&at| {
            if body[at] == NONE {
                return false;
            }
            let (i, j) = ((at % side) as i64, (at / side) as i64);
            (-1i64..=1).all(|dj| {
                (-1i64..=1).all(|di| {
                    let (ni, nj) = (i + di, j + dj);
                    ni < 0 || nj < 0 || ni >= side as i64 || nj >= side as i64 || shore[nj as usize * side + ni as usize] <= shore[at]
                })
            })
        })
        .collect();
    // Widest first, so a centre inside a bigger circle is skipped.
    centres.sort_by(|&a, &b| shore[b].total_cmp(&shore[a]));
    for c in centres {
        let r = shore[c];
        if covered[c] >= r {
            continue;
        }
        let reach = (r as f64 * 1.25 / cell_m).ceil() as i64 + 3;
        let limit = (reach * reach) as f64;
        let (ci, cj) = ((c % side) as i64, (c / side) as i64);
        for j in (cj - reach).max(0)..=(cj + reach).min(side as i64 - 1) {
            for i in (ci - reach).max(0)..=(ci + reach).min(side as i64 - 1) {
                let (di, dj) = ((i - ci) as f64, (j - cj) as f64);
                let at = j as usize * side + i as usize;
                if di * di + dj * dj <= limit && body[at] == body[c] {
                    radius[at] = radius[at].max(r);
                    covered[at] = covered[at].max(r);
                }
            }
        }
    }
    radius
}

/// Calls `set` with the index of every grid sample inside the polygon (even-odd over its rings).
fn fill(rings: &[Vec<P>], grid: Grid, mut set: impl FnMut(usize)) {
    let (mut z_min, mut z_max) = (f64::MAX, f64::MIN);
    for p in rings.iter().flatten() {
        z_min = z_min.min(p[1]);
        z_max = z_max.max(p[1]);
    }
    if z_min > z_max {
        return;
    }
    let to_index = |v: f64| (v * 100.0 - grid.origin as f64) / grid.cell as f64;
    let last = grid.side as i64 - 1;
    let (j0, j1) = ((to_index(z_min).ceil() as i64).max(0), (to_index(z_max).floor() as i64).min(last));
    let mut crossings = Vec::new();
    for j in j0..=j1 {
        let z = grid.x(j as u32);
        crossings.clear();
        for ring in rings {
            let mut previous = *ring.last().unwrap();
            for &current in ring {
                if (current[1] > z) != (previous[1] > z) {
                    crossings.push(previous[0] + (z - previous[1]) * (current[0] - previous[0]) / (current[1] - previous[1]));
                }
                previous = current;
            }
        }
        crossings.sort_by(f64::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let (i0, i1) = ((to_index(pair[0]).ceil() as i64).max(0), (to_index(pair[1]).floor() as i64).min(last));
            for i in i0..=i1 {
                set((j * grid.side as i64 + i) as usize);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        // 21 × 21 samples every 10 m, from -100 to 100 m.
        Grid { origin: -10_000, cell: 1_000, side: 21 }
    }

    fn lake() -> Map {
        let ring = vec![[-40.0, -40.0], [40.0, -40.0], [40.0, 40.0], [-40.0, 40.0]];
        Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![ring], level: None }], streams: Vec::new() }
    }

    #[test]
    fn the_surface_is_the_median_and_the_bed_is_deeper_inland() {
        // Ground rises 1 m per 10 m in x, except inside the lake where the model is flat at 5 m.
        let dem = |x: f64, _z: f64| Some(if x.abs() <= 40.0 { 5.0 } else { 5.0 + (x.abs() - 40.0) / 10.0 });
        let surface = Surface::new(&lake(), grid(), dem);
        assert!(surface.is_wet(0.0, 0.0) && !surface.is_wet(60.0, 0.0));
        let depth = |x: f64| 5.0 - surface.bed(x, 0.0, dem(x, 0.0)).unwrap();
        // The first wet sample is 5 m from the bank, the middle 35 m (the fill is half-open in z, so
        // the lake's last row is z = 30 m): the body's radius.
        assert!((depth(40.0) - Kind::Lake.depth(5.0, 35.0)).abs() < 1e-4, "{}", depth(40.0));
        assert!((depth(0.0) - Kind::Lake.max_depth(35.0)).abs() < 1e-4, "{}", depth(0.0));
        assert!(depth(0.0) > depth(20.0) && depth(20.0) > depth(40.0));
        assert_eq!(surface.bed(80.0, 0.0, Some(7.0)), Some(7.0), "dry ground is untouched");
    }

    #[test]
    fn a_river_is_as_deep_as_it_is_wide_where_you_are() {
        // One polygon: 20 m wide for the first 300 m, then a 300 m wide basin.
        let ring = vec![[-300.0, -10.0], [0.0, -10.0], [0.0, -150.0], [300.0, -150.0], [300.0, 150.0], [0.0, 150.0], [0.0, 10.0], [-300.0, 10.0]];
        let map = Map { bodies: vec![Body { kind: Kind::River, rings: vec![ring], level: None }], streams: Vec::new() };
        let grid = Grid { origin: -40_000, cell: 1_000, side: 81 };
        let surface = Surface::new(&map, grid, |_, _| Some(0.0));
        let depth = |x: f64, z: f64| -surface.bed(x, z, Some(0.0)).unwrap();
        // Mid-channel upstream is as deep as a 10 m wide river is, not a basin's depth.
        assert!(depth(-200.0, 0.0) < Kind::River.max_depth(10.0) + 1e-3, "{}", depth(-200.0, 0.0));
        assert!(depth(150.0, 0.0) > depth(-200.0, 0.0) + 1.0, "{} {}", depth(150.0, 0.0), depth(-200.0, 0.0));
    }

    #[test]
    fn bigger_water_is_deeper() {
        assert!(Kind::Lake.max_depth(100.0) > Kind::Lake.max_depth(10.0));
        assert!(Kind::River.max_depth(150.0) > Kind::River.max_depth(3.0));
        assert!(Kind::Lake.max_depth(1e6) <= 12.0 && Kind::River.max_depth(1e6) <= 6.0);
        assert!((Kind::Lake.depth(0.0, 50.0) - SHORE_DEPTH).abs() < 1e-9);
        assert!((Kind::Lake.depth(50.0, 50.0) - Kind::Lake.max_depth(50.0)).abs() < 1e-9);
        assert_eq!(Kind::Lake.depth(3.0, 0.0), Kind::Lake.max_depth(0.0), "no division by a zero radius");
    }

    #[test]
    fn levels_cover_the_bank_one_sample_out_and_sit_below_it() {
        let dem = |_: f64, _: f64| Some(10.0);
        let surface = Surface::new(&lake(), grid(), dem);
        let side = 21;
        let heights: Vec<i32> = (0..side * side).map(|_| 1000).collect();
        let ground = Ground { origin: -10_000, cell: 1_000, side: side as u32, heights };
        let levels = surface.levels(0.0, &ground);
        let at = |x: i32, z: i32| levels[((z + 10) * side + x + 10) as usize];
        assert_eq!(at(0, 0), 1000);
        // Sample x = 40 m (index 4) is wet, x = 50 m (5) is bank, x = 60 m (6) has nothing.
        assert_eq!(at(4, 0), 1000);
        assert_eq!(at(5, 0), 1000 - BANK_DROP_CM);
        assert_eq!(at(6, 0), NO_WATER);
    }

    #[test]
    fn a_body_the_height_model_does_not_cover_is_dropped() {
        let surface = Surface::new(&lake(), grid(), |_, _| None);
        assert!(surface.is_empty());
    }

    #[test]
    fn holes_stay_dry() {
        let outer = vec![[-80.0, -80.0], [80.0, -80.0], [80.0, 80.0], [-80.0, 80.0]];
        let island = vec![[-20.0, -20.0], [20.0, -20.0], [20.0, 20.0], [-20.0, 20.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![outer, island], level: None }], streams: Vec::new() };
        let surface = Surface::new(&map, grid(), |_, _| Some(0.0));
        assert!(surface.is_wet(50.0, 0.0));
        assert!(!surface.is_wet(0.0, 0.0));
    }

    #[test]
    fn streams_lose_only_their_middle_in_a_lake() {
        let surface = Surface::new(&lake(), grid(), |_, _| Some(0.0));
        // Runs from x = -90 to 90 through the lake, vertices every 20 m.
        let line: Vec<P> = (0..10).map(|k| [-90.0 + k as f64 * 20.0, 0.0]).collect();
        let trimmed = surface.trim_streams(&[line]);
        assert_eq!(trimmed.len(), 2, "{trimmed:?}");
        assert!(trimmed[0].last().unwrap()[0] <= -30.0 && trimmed[1][0][0] >= 30.0);
    }
}
