//! Turns OpenStreetMap's water (`osm_water.rs`) into what the world stores: a water surface height at
//! every ground sample, with lakes and wide rivers carved into the ground under it, and stream
//! centre lines.
//!
//! The data has no surface height or depth. The height model is flat over water, so the surface
//! of a body is the median elevation of the height model inside it, and its bed is cut below that
//! with a depth that grows with the distance from the shore and with the size of the body.

use shared::terrain::Ground;
use shared::world::{NO_WATER, REACH_STEP};

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
    /// Open sea: its bed follows the distance from land only (see `Kind::sea_depth`).
    Sea,
}

/// Depth of the sea at its farthest from land, metres (the most a bed may sit below y = 0).
const SEA_DEPTH: f64 = 12.0;
/// The sea reaches `SEA_DEPTH` no closer than this to land, metres, so a world that only touches
/// the sea does not get a cliff off its beach.
const SEA_SHELF: f64 = 600.0;

impl Kind {
    /// Depth of the deepest point of water whose widest part is `radius` metres from its bank.
    /// The data has no depths: bigger water is deeper (a pond about 3.5 m, a 100 m lake about 8.5 m,
    /// a kilometre-wide one about 24 m; a 20 m river about 3 m, a 200 m one about 7 m).
    fn max_depth(self, radius: f64) -> f64 {
        match self {
            Kind::Lake => (1.5 + 1.0 * radius.sqrt()).clamp(2.0, 24.0),
            Kind::River => (1.0 + 0.6 * radius.sqrt()).clamp(1.5, 12.0),
            Kind::Sea => SEA_DEPTH,
        }
    }

    /// Sea bed depth at `shore` metres from land when the farthest water is `farthest` from it. A
    /// lake's bowl is steep at the bank and flat in the middle; open sea instead shelves gently
    /// and keeps deepening out to the edge of the world, ending at `SEA_DEPTH` there (or at
    /// `SEA_SHELF`, if the world holds less sea). It does not follow the local width the way a
    /// river does: on a ragged coast that leaves the edges of overlapping circles in the bed.
    fn sea_depth(shore: f64, farthest: f64) -> f64 {
        let t = (shore / farthest.max(SEA_SHELF)).min(1.0);
        // Slope 1.5 at the shore and 0.5 at the end: steady, without a flat floor.
        SHORE_DEPTH + (SEA_DEPTH - SHORE_DEPTH) * t * (1.5 - 0.5 * t)
    }

    /// Bed depth at `shore` metres from the bank where the water reaches `radius` metres from it:
    /// a bowl that is `SHORE_DEPTH` at the bank and `max_depth` at `radius`, falling away steeply
    /// from the bank (three times its average slope) and flattening out towards the middle.
    fn depth(self, shore: f64, radius: f64) -> f64 {
        let max = self.max_depth(radius);
        let t = if radius > 0.0 { (shore / radius).min(1.0) } else { 1.0 };
        SHORE_DEPTH + (max - SHORE_DEPTH) * (1.0 - (1.0 - t).powi(3))
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
    /// How wide the open water is around each sample, in `REACH_STEP`s (`Tile::reach`); empty with `levels`.
    pub reach: Vec<u8>,
    pub streams: Vec<Vec<P>>,
}

impl Placed {
    /// The finished water of `map` over `ground`, whose y = 0 is `datum` in the height model.
    pub fn new(map: &Map, surface: &Surface, datum: f64, ground: &Ground) -> Self {
        let (levels, reach) = if surface.is_empty() { (Vec::new(), Vec::new()) } else { (surface.levels(datum, ground), surface.reaches()) };
        Self { levels, reach, streams: surface.trim_streams(&map.streams) }
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
    /// Surface elevation at each sample, metres above the height model's datum (NaN on dry
    /// samples). Flat over a lake; a river falls along its course (`river_levels`).
    level: Vec<f32>,
    /// Bed depth under each sample, metres (0 on dry samples).
    depth: Vec<f32>,
    /// How wide the open water is around each sample, metres (0 on dry samples): a lake's whole
    /// width, a river's where you are, and the open sea on the sea. The waves grow with it.
    reach: Vec<f32>,
}

impl Surface {
    /// Rasterises `map` onto `grid`. `elevation(x, z)` is the height model at a local position,
    /// metres, `None` where it has no data. Bodies it knows nothing about are dropped.
    pub fn new(map: &Map, grid: Grid, mut elevation: impl FnMut(f64, f64) -> Option<f64>) -> Self {
        let n = (grid.side * grid.side) as usize;
        let mut body = vec![NONE; n];
        // The height model at each wet sample of a body without a fixed level, NaN elsewhere.
        let mut model = vec![f32::NAN; n];
        // Each body's one surface: its fixed level, or the median of the model inside it.
        let mut flat = Vec::with_capacity(map.bodies.len());
        for (index, b) in map.bodies.iter().enumerate() {
            let mut cells = Vec::new();
            fill(&b.rings, grid, |cell| cells.push(cell));
            if let Some(fixed) = b.level {
                flat.push(fixed);
                for cell in cells {
                    body[cell] = index as u32;
                }
                continue;
            }
            let mut heights = Vec::with_capacity(cells.len());
            for &c in &cells {
                let height = elevation(grid.x(c as u32 % grid.side), grid.x(c as u32 / grid.side));
                model[c] = height.map_or(f32::NAN, |h| h as f32);
                heights.extend(height);
            }
            if heights.is_empty() {
                flat.push(f64::NAN);
                continue;
            }
            heights.sort_by(f64::total_cmp);
            let median = heights[heights.len() / 2];
            let spread = heights[heights.len() * 9 / 10] - heights[heights.len() / 10];
            if b.kind == Kind::Lake && spread > 1.5 && cells.len() > 200 {
                eprintln!("water: a {} m² lake has {spread:.1} m between its 10th and 90th percentile, drawn flat at {median:.1} m", cells.len() as f64 * (grid.cell as f64 / 100.0).powi(2));
            }
            flat.push(median);
            for cell in cells {
                body[cell] = index as u32;
            }
        }
        let mut level: Vec<f32> = body.iter().map(|&b| if b == NONE { f32::NAN } else { flat[b as usize] as f32 }).collect();
        river_levels(map, grid, &body, &model, &mut level);
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
        let mut farthest = vec![0.0f64; map.bodies.len()];
        for (at, &b) in body.iter().enumerate() {
            if b != NONE {
                farthest[b as usize] = farthest[b as usize].max(shore[at] as f64);
            }
        }
        let depth = (0..n)
            .map(|at| match body[at] {
                NONE => 0.0,
                b => (match map.bodies[b as usize].kind {
                    Kind::Sea => Kind::sea_depth(shore[at] as f64, farthest[b as usize]),
                    kind => kind.depth(shore[at] as f64, width[at] as f64),
                }) as f32,
            })
            .collect();
        let reach = (0..n)
            .map(|at| match body[at] {
                NONE => 0.0,
                b => match map.bodies[b as usize].kind {
                    Kind::Sea => shared::waves::OPEN_SEA,
                    Kind::Lake => 2.0 * farthest[b as usize] as f32,
                    Kind::River => 2.0 * width[at],
                },
            })
            .collect();
        Self { grid, body, level, depth, reach }
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
            _ => Some(self.level[at] as f64 - self.depth[at] as f64),
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
                out[at] = ((self.level[at] as f64 - datum) * 100.0).round() as i32;
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

    /// How wide the open water is at every ground sample, in `REACH_STEP`s, as `levels` lays out the
    /// surface: a bank sample beside the water takes the widest water next to it, a dry one 0.
    pub fn reaches(&self) -> Vec<u8> {
        let side = self.grid.side as usize;
        let step = |metres: f32| (metres / REACH_STEP).round().clamp(0.0, f32::from(u8::MAX)) as u8;
        let mut out: Vec<u8> = self.reach.iter().map(|&metres| step(metres)).collect();
        for j in 0..side {
            for i in 0..side {
                let at = j * side + i;
                if self.body[at] != NONE {
                    continue;
                }
                for (dj, di) in (-1i32..=1).flat_map(|dj| (-1i32..=1).map(move |di| (dj, di))) {
                    let (ni, nj) = (i as i32 + di, j as i32 + dj);
                    if ni >= 0 && nj >= 0 && ni < side as i32 && nj < side as i32 {
                        out[at] = out[at].max(step(self.reach[nj as usize * side + ni as usize]));
                    }
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

/// A river's profile is the height model's median in each stretch this long along its course, metres.
const RIVER_BIN: f64 = 20.0;
/// A connected stretch of river with fewer wet samples than this keeps its body's flat level.
const RIVER_MIN_SAMPLES: usize = 200;
/// A stretch at least this long, metres, has its fall reported.
const RIVER_REPORTED: f64 = 500.0;

/// Gives every river a surface that falls along its course instead of one flat level: a whole
/// river is often one polygon tens of kilometres long, and a level for all of it stands metres
/// above the banks at one end of the world and below them at the other.
///
/// Each connected stretch of a river body is measured along its length (the distance through the
/// water from one end, whose whole cross-section is the start where it leaves the grid), the
/// height model's median is taken every `RIVER_BIN` metres along it, and those medians are fitted
/// with the closest profile that only ever falls one way (isotonic regression, in whichever
/// direction fits better). The surface is level across the river and steps down at rapids.
fn river_levels(map: &Map, grid: Grid, body: &[u32], model: &[f32], level: &mut [f32]) {
    let side = grid.side as usize;
    let cell_m = grid.cell as f64 / 100.0;
    let n = body.len();
    let mut seen = vec![false; n];
    let mut dist = vec![u32::MAX; n];
    let mut stack = Vec::new();
    for start in 0..n {
        let b = body[start];
        if seen[start] || b == NONE || map.bodies[b as usize].kind != Kind::River {
            continue;
        }
        let mut members = Vec::new();
        seen[start] = true;
        stack.push(start);
        while let Some(at) = stack.pop() {
            members.push(at);
            for (next, _) in neighbours(at, side) {
                if !seen[next] && body[next] == b {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        if members.len() < RIVER_MIN_SAMPLES {
            continue;
        }
        // One end of the stretch is the sample farthest through the water from any other.
        chamfer(&[members[0]], body, side, &mut dist);
        let end = *members.iter().max_by_key(|&&at| dist[at]).expect("a stretch has samples");
        for &at in &members {
            dist[at] = u32::MAX;
        }
        chamfer(&cross_section(end, body, side), body, side, &mut dist);
        // Bin k holds the samples k to k + 1 bins along; its point of the profile is at k + 0.5.
        let along = |at: usize| dist[at] as f64 / 3.0 * cell_m / RIVER_BIN;
        let bins = members.iter().map(|&at| along(at) as usize).max().unwrap_or(0) + 1;
        let mut heights: Vec<Vec<f32>> = vec![Vec::new(); bins];
        for &at in &members {
            if model[at].is_finite() {
                heights[along(at) as usize].push(model[at]);
            }
        }
        let (mut centres, mut medians, mut weights) = (Vec::new(), Vec::new(), Vec::new());
        for (k, bin) in heights.iter_mut().enumerate().filter(|(_, bin)| !bin.is_empty()) {
            bin.sort_by(f32::total_cmp);
            centres.push(k as f64 + 0.5);
            medians.push(bin[bin.len() / 2] as f64);
            weights.push(bin.len() as f64);
        }
        if centres.len() < 3 {
            continue;
        }
        let rising = isotonic(&medians, &weights);
        let falling: Vec<f64> = isotonic(&medians.iter().map(|m| -m).collect::<Vec<_>>(), &weights).iter().map(|m| -m).collect();
        let error = |fit: &[f64]| fit.iter().zip(&medians).zip(&weights).map(|((f, m), w)| w * (f - m) * (f - m)).sum::<f64>();
        let profile = if error(&falling) <= error(&rising) { falling } else { rising };
        for &at in &members {
            level[at] = interpolate(&centres, &profile, along(at)) as f32;
        }
        let length = bins as f64 * RIVER_BIN;
        if length >= RIVER_REPORTED {
            let drop = (profile[0] - profile[profile.len() - 1]).abs();
            eprintln!("water: a river falls {drop:.1} m over {length:.0} m");
        }
        for &at in &members {
            dist[at] = u32::MAX;
        }
    }
}

/// The eight samples around `at` that are on the grid, with the chamfer cost of the step to each
/// (3 straight, 4 diagonal, in thirds of a cell).
fn neighbours(at: usize, side: usize) -> impl Iterator<Item = (usize, u32)> {
    let (i, j) = ((at % side) as i64, (at / side) as i64);
    [(-1, -1, 4), (0, -1, 3), (1, -1, 4), (-1, 0, 3), (1, 0, 3), (-1, 1, 4), (0, 1, 3), (1, 1, 4)].into_iter().filter_map(move |(di, dj, cost)| {
        let (ni, nj) = (i + di, j + dj);
        (ni >= 0 && nj >= 0 && ni < side as i64 && nj < side as i64).then(|| (nj as usize * side + ni as usize, cost))
    })
}

/// Distance through the water of the seeds' body from the nearest seed, in thirds of a cell,
/// written into `dist` (which must hold `u32::MAX` on the body's samples).
fn chamfer(seeds: &[usize], body: &[u32], side: usize, dist: &mut [u32]) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut queue = BinaryHeap::new();
    for &seed in seeds {
        dist[seed] = 0;
        queue.push(Reverse((0, seed)));
    }
    while let Some(Reverse((d, at))) = queue.pop() {
        if d > dist[at] {
            continue;
        }
        for (next, cost) in neighbours(at, side) {
            if body[next] == body[at] && d + cost < dist[next] {
                dist[next] = d + cost;
                queue.push(Reverse((d + cost, next)));
            }
        }
    }
}

/// Where a river's end `end` leaves the grid, the whole run of its samples along that edge of
/// the grid, so the distance along the river starts across its width; otherwise `end` alone.
fn cross_section(end: usize, body: &[u32], side: usize) -> Vec<usize> {
    let (i, j) = (end % side, end / side);
    let last = side - 1;
    let mut seeds = vec![end];
    // For each edge `end` is on, the step along it.
    let steps: Vec<(i64, i64)> = [(i == 0 || i == last, (0, 1)), (j == 0 || j == last, (1, 0))].into_iter().filter(|(on, _)| *on).map(|(_, step)| step).collect();
    for (si, sj) in steps {
        for sign in [-1i64, 1] {
            let (mut ci, mut cj) = (i as i64, j as i64);
            loop {
                (ci, cj) = (ci + sign * si, cj + sign * sj);
                if ci < 0 || cj < 0 || ci > last as i64 || cj > last as i64 {
                    break;
                }
                let at = cj as usize * side + ci as usize;
                if body[at] != body[end] {
                    break;
                }
                seeds.push(at);
            }
        }
    }
    seeds
}

/// The non-decreasing sequence closest to `values` in weighted least squares (pool adjacent violators).
fn isotonic(values: &[f64], weights: &[f64]) -> Vec<f64> {
    // Blocks of pooled values: mean, total weight, how many values.
    let mut blocks: Vec<(f64, f64, usize)> = Vec::with_capacity(values.len());
    for (&value, &weight) in values.iter().zip(weights) {
        blocks.push((value, weight, 1));
        while blocks.len() >= 2 && blocks[blocks.len() - 2].0 > blocks[blocks.len() - 1].0 {
            let (b, a) = (blocks.pop().unwrap(), blocks.pop().unwrap());
            blocks.push(((a.0 * a.1 + b.0 * b.1) / (a.1 + b.1), a.1 + b.1, a.2 + b.2));
        }
    }
    blocks.iter().flat_map(|&(mean, _, count)| std::iter::repeat_n(mean, count)).collect()
}

/// The piecewise linear function through `(xs[k], ys[k])` at `x`, held flat beyond the ends.
fn interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let k = xs.partition_point(|&p| p < x);
    if k == 0 {
        return ys[0];
    }
    if k == xs.len() {
        return ys[k - 1];
    }
    let t = (x - xs[k - 1]) / (xs[k] - xs[k - 1]);
    ys[k - 1] + (ys[k] - ys[k - 1]) * t
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

    /// 321 × 321 samples every 2 m, from -320 to 320 m.
    fn river_grid() -> Grid {
        Grid { origin: -32_000, cell: 200, side: 321 }
    }

    fn river(ring: Vec<P>) -> Map {
        Map { bodies: vec![Body { kind: Kind::River, rings: vec![ring], level: None }], streams: Vec::new() }
    }

    fn level(surface: &Surface, x: f64, z: f64) -> f64 {
        surface.level[surface.index(x, z).unwrap()] as f64
    }

    /// A few decimetres of unevenness, the same every run.
    fn noise(x: f64, z: f64) -> f64 {
        0.3 * (x * 0.73 + z * 1.31).sin()
    }

    #[test]
    fn a_river_falls_along_its_length_and_is_level_across() {
        // 20 m wide, running past both ends of the grid; the ground falls 1 m per 100 m in x.
        let map = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - x / 100.0 + noise(x, z)));
        let profile: Vec<f64> = (-30..=30).map(|k| level(&surface, k as f64 * 10.0, 0.0)).collect();
        assert!(profile.windows(2).all(|w| w[1] <= w[0] + 1e-4), "never rises downstream: {profile:?}");
        let drop = profile[0] - profile[profile.len() - 1];
        assert!((drop - 6.0).abs() < 0.5, "{drop}");
        for x in [-250.0, 0.0, 170.0] {
            assert!((level(&surface, x, -8.0) - level(&surface, x, 8.0)).abs() < 0.05, "level across at {x}");
            assert!((level(&surface, x, 0.0) - (10.0 - x / 100.0)).abs() < 0.3, "follows the ground at {x}");
        }
    }

    #[test]
    fn a_bending_river_follows_its_course() {
        // Runs east along z = 0 to x = 0, then turns south (+z); the ground falls 1 m per 100 m of course.
        let map = river(vec![[-400.0, -10.0], [10.0, -10.0], [10.0, 400.0], [-10.0, 400.0], [-10.0, 10.0], [-400.0, 10.0]]);
        let course = |x: f64, z: f64| x.min(0.0) + 400.0 + z.max(0.0);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - course(x, z) / 100.0 + noise(x, z)));
        let path: Vec<f64> = (-30..=-2).map(|k| level(&surface, k as f64 * 10.0, 0.0)).chain((2..=30).map(|k| level(&surface, 0.0, k as f64 * 10.0))).collect();
        assert!(path.windows(2).all(|w| w[1] <= w[0] + 1e-4), "falls round the bend: {path:?}");
        assert!(level(&surface, 0.0, 30.0) > level(&surface, 0.0, 300.0) + 2.0, "falls along the southern arm, where x does not change");
    }

    #[test]
    fn rapids_are_a_step_not_a_flood() {
        // The water drops 3 m at x = 0.
        let map = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(if x < 0.0 { 10.0 } else { 7.0 } + 0.05 * noise(x, z)));
        assert!((level(&surface, -200.0, 0.0) - 10.0).abs() < 0.1, "{}", level(&surface, -200.0, 0.0));
        assert!((level(&surface, 200.0, 0.0) - 7.0).abs() < 0.1, "{}", level(&surface, 200.0, 0.0));
    }

    #[test]
    fn isotonic_pools_what_rises_out_of_order() {
        assert_eq!(isotonic(&[1.0, 3.0, 2.0, 4.0], &[1.0; 4]), vec![1.0, 2.5, 2.5, 4.0]);
        assert_eq!(isotonic(&[3.0, 1.0], &[1.0, 3.0]), vec![1.5, 1.5]);
    }

    #[test]
    fn a_lakes_reach_is_its_whole_width_and_the_sea_is_open() {
        // The 80 m square lake: every sample of it has the same reach, the width across it.
        let surface = Surface::new(&lake(), grid(), |_, _| Some(5.0));
        let reach = surface.reaches();
        let middle = reach[10 * 21 + 10];
        assert!((f32::from(middle) * REACH_STEP - 80.0).abs() <= 2.0 * REACH_STEP, "{middle}");
        assert_eq!(reach[10 * 21 + 6], middle, "near the bank as in the middle");
        assert_eq!(reach[10 * 21 + 5], middle, "the bank sample beside it too");
        assert_eq!(reach[10 * 21 + 1], 0, "dry land has none");
        let ring = vec![[-100.0, -100.0], [100.0, -100.0], [100.0, 100.0], [-100.0, 100.0]];
        let sea = Map { bodies: vec![Body { kind: Kind::Sea, rings: vec![ring], level: Some(0.0) }], streams: Vec::new() };
        assert!(Surface::new(&sea, grid(), |_, _| Some(1.0)).reaches().iter().all(|&steps| steps == u8::MAX), "the sea carries every swell");
    }

    #[test]
    fn the_sea_shelves_steadily_out_to_its_farthest_point() {
        // A coast along x = -350 m, sea to the east as far as the grid goes (about 750 m).
        let ring = vec![[-350.0, -500.0], [500.0, -500.0], [500.0, 500.0], [-350.0, 500.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Sea, rings: vec![ring], level: Some(0.0) }], streams: Vec::new() };
        let grid = Grid { origin: -40_000, cell: 500, side: 161 };
        let surface = Surface::new(&map, grid, |_, _| Some(1.0));
        let depth = |x: f64| -surface.bed(x, 0.0, Some(1.0)).unwrap();
        let profile: Vec<f64> = (0..=149).map(|k| depth(-345.0 + k as f64 * 5.0)).collect();
        assert!(profile[0] < 0.6, "shallow at the beach: {}", profile[0]);
        let slopes: Vec<f64> = profile.windows(2).map(|w| (w[1] - w[0]) / 5.0).collect();
        assert!(slopes.iter().all(|&s| s > 0.0), "deepens all the way out");
        assert!(slopes.iter().all(|&s| s < 0.04), "no steep drop: {:?}", slopes.iter().cloned().fold(0.0, f64::max));
        assert!(slopes.last().unwrap() * 4.0 > slopes[0], "no flat floor offshore");
        assert!((profile.last().unwrap() - SEA_DEPTH).abs() < 0.2, "{}", profile.last().unwrap());
        // A sea the world only touches still shelves gently.
        assert!(Kind::sea_depth(50.0, 50.0) < 2.0);
    }

    #[test]
    fn bigger_water_is_deeper() {
        assert!(Kind::Lake.max_depth(100.0) > Kind::Lake.max_depth(10.0));
        assert!(Kind::River.max_depth(150.0) > Kind::River.max_depth(3.0));
        assert!(Kind::Lake.max_depth(1e6) <= 24.0 && Kind::River.max_depth(1e6) <= 12.0);
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
