//! Turns Hydrografi's water (`hydro.rs`) into what the world stores: a water surface height at
//! every ground sample, with lakes and wide rivers carved into the ground under it, and stream
//! centre lines.
//!
//! The data has no surface height or depth. The height model is flat over water, so the surface
//! of a body is the median elevation of the height model inside it, and its bed is cut below that
//! with a depth that grows with the distance from the shore.

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
    /// Bed depth at `shore` metres from the bank.
    fn depth(self, shore: f64) -> f64 {
        match self {
            Kind::Lake => (SHORE_DEPTH + 0.2 * shore).min(2.5),
            Kind::River => (SHORE_DEPTH + 0.15 * shore).min(1.2),
        }
    }
}

/// A body of water: one polygon, outer ring first and then holes, in local metres.
#[derive(Clone, Debug)]
pub struct Body {
    pub kind: Kind,
    pub rings: Vec<Vec<P>>,
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
    pub levels: Vec<i16>,
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
        let depth = (0..n)
            .map(|at| match body.get(at).copied().filter(|&b| b != NONE) {
                // The first wet sample is half a cell from the bank.
                Some(b) => map.bodies[b as usize].kind.depth((dist[at].min(FAR) as f64 / 3.0 - 0.5).max(0.0) * cell_m) as f32,
                None => 0.0,
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
    pub fn levels(&self, datum: f64, ground: &Ground) -> Vec<i16> {
        let side = self.grid.side as usize;
        let mut out = vec![NO_WATER; side * side];
        for (at, &b) in self.body.iter().enumerate() {
            if b != NONE {
                out[at] = (((self.level[b as usize] - datum) * 100.0).round() as i32).clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16;
            }
        }
        let wet = out.clone();
        for j in 0..side {
            for i in 0..side {
                let at = j * side + i;
                if wet[at] != NO_WATER {
                    continue;
                }
                let mut near: Option<i16> = None;
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
                    let ground_cm = ground.heights[at] as i32;
                    out[at] = (level as i32).min(ground_cm - BANK_DROP_CM).clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16;
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
        Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![ring] }], streams: Vec::new() }
    }

    #[test]
    fn the_surface_is_the_median_and_the_bed_is_deeper_inland() {
        // Ground rises 1 m per 10 m in x, except inside the lake where the model is flat at 5 m.
        let dem = |x: f64, _z: f64| Some(if x.abs() <= 40.0 { 5.0 } else { 5.0 + (x.abs() - 40.0) / 10.0 });
        let surface = Surface::new(&lake(), grid(), dem);
        assert!(surface.is_wet(0.0, 0.0) && !surface.is_wet(60.0, 0.0));
        let shore = surface.bed(40.0, 0.0, dem(40.0, 0.0)).unwrap();
        let middle = surface.bed(0.0, 0.0, dem(0.0, 0.0)).unwrap();
        assert!((5.0 - shore - SHORE_DEPTH - 0.2 * 5.0 + 0.0).abs() < 1e-6, "first wet sample is half a cell from the bank: {shore}");
        assert!(middle < shore, "{middle} {shore}");
        assert_eq!(surface.bed(80.0, 0.0, Some(7.0)), Some(7.0), "dry ground is untouched");
    }

    #[test]
    fn levels_cover_the_bank_one_sample_out_and_sit_below_it() {
        let dem = |_: f64, _: f64| Some(10.0);
        let surface = Surface::new(&lake(), grid(), dem);
        let side = 21;
        let heights: Vec<i16> = (0..side * side).map(|_| 1000).collect();
        let ground = Ground { origin: -10_000, cell: 1_000, side: side as u32, heights };
        let levels = surface.levels(0.0, &ground);
        let at = |x: i32, z: i32| levels[((z + 10) * side + x + 10) as usize];
        assert_eq!(at(0, 0), 1000);
        // Sample x = 40 m (index 4) is wet, x = 50 m (5) is bank, x = 60 m (6) has nothing.
        assert_eq!(at(4, 0), 1000);
        assert_eq!(at(5, 0), 1000 - BANK_DROP_CM as i16);
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
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![outer, island] }], streams: Vec::new() };
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
