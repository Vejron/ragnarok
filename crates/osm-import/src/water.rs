//! Turns OpenStreetMap's water (`osm_water.rs`) into what the world stores: a water surface height at
//! every ground sample, with lakes and wide rivers carved into the ground under it, and stream
//! centre lines.
//!
//! The data has no surface height or depth. The height model is flat over water, so the surface
//! of a body is the median elevation of the height model inside it, and its bed is cut below that
//! with a depth that grows with the distance from the shore and with the size of the body.

use shared::terrain::Ground;
use shared::world::{FLOW_STEP, NO_WATER, REACH_STEP};

type P = [f64; 2];

/// Streams are drawn this wide, metres: the data has centre lines only.
pub const STREAM_WIDTH: f64 = 2.5;
/// Depth of the water at the shore, metres.
const SHORE_DEPTH: f64 = 0.3;
/// How steeply the ground falls into the water at the waterline, and rises out of it on the bank,
/// metres a metre. The bed is no deeper than this times the distance in from the outline, and the
/// depth the client interpolates between a wet and a dry sample then crosses zero on the outline.
const SHORE_SLOPE: f64 = 0.8;
/// The most a bank sample's ground is raised to meet `SHORE_SLOPE`, metres: enough to lift a bank
/// level with the water clear of it, never a dyke.
const BANK_RAISE: f64 = 0.2;
/// The surface carried onto a bank sample sits at least this far under its ground, centimetres...
const BANK_DROP_CM: i32 = 10;
/// ...and dips under the water beside it by no more than this, centimetres, where its ground is too
/// low to rise out of the water at `SHORE_SLOPE`.
const BANK_DIP_CM: i32 = 50;
/// The distance from the bank of water whose outline lies wholly outside the grid, metres (the open sea).
const OPEN_SHORE: f32 = 1.0e6;

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
    /// Which way and how fast the water runs at each sample, `x` then `z` in `FLOW_STEP`s (`Tile::flow`);
    /// empty when nothing flows.
    pub flow: Vec<i8>,
    pub streams: Vec<Vec<P>>,
}

impl Placed {
    /// The finished water of `map` over `ground`, whose y = 0 is `datum` in the height model.
    pub fn new(map: &Map, surface: &Surface, datum: f64, ground: &Ground) -> Self {
        let (levels, reach, flow) = if surface.is_empty() { (Vec::new(), Vec::new(), Vec::new()) } else { (surface.levels(datum, ground), surface.reaches(), surface.flows()) };
        Self { levels, reach, flow, streams: surface.trim_streams(&map.streams) }
    }

    /// How deep the water is at a local position over `ground` (the grid `levels` lies on), metres,
    /// blended from the samples round it as the client draws the shoreline: negative on dry ground,
    /// -1 at a sample without water.
    pub fn depth_at(&self, ground: &Ground, x: f64, z: f64) -> f64 {
        if self.levels.len() != ground.heights.len() || ground.cell <= 0 || ground.side < 2 {
            return -1.0;
        }
        let cell = f64::from(ground.cell) / 100.0;
        let last = f64::from(ground.side - 1);
        let gx = ((x - f64::from(ground.origin) / 100.0) / cell).clamp(0.0, last);
        let gz = ((z - f64::from(ground.origin) / 100.0) / cell).clamp(0.0, last);
        let side = ground.side as usize;
        let (i, j) = ((gx.floor() as usize).min(side - 2), (gz.floor() as usize).min(side - 2));
        let (fx, fz) = (gx - i as f64, gz - j as f64);
        let depth = |i: usize, j: usize| {
            let at = j * side + i;
            if self.levels[at] == NO_WATER { -1.0 } else { f64::from(self.levels[at] - ground.heights[at]) / 100.0 }
        };
        let near = depth(i, j) + (depth(i + 1, j) - depth(i, j)) * fx;
        let far = depth(i, j + 1) + (depth(i + 1, j + 1) - depth(i, j + 1)) * fx;
        near + (far - near) * fz
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
    /// The water's velocity at each sample, `[x, z]` m/s: down a river's course, fastest where it falls
    /// steeply and in the middle, still at its banks; still on lakes, the sea and dry samples.
    flow: Vec<[f32; 2]>,
    /// On each dry sample next to the water, the lowest surface beside it, metres above the datum,
    /// and how far outside the outline it is, metres; NaN elsewhere. `bed` shapes the bank with it.
    bank: Vec<[f32; 2]>,
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
        let mut flow = vec![[0.0f32; 2]; n];
        river_levels(map, grid, &body, &model, &mut level, &mut flow);
        let side = grid.side as usize;
        let cell_m = grid.cell as f64 / 100.0;
        // Metres from the bank of every wet sample, measured to the drawn outline itself rather
        // than to the nearest dry sample, so the bed (and the shoreline the client draws where the
        // depth crosses zero) follows the outline between samples. Also filled in on the dry
        // samples next to the water, for `bed` to shape the bank.
        let measured = shore_distance(map, grid, &body);
        // The dry samples next to the water: the lowest level beside each and how far outside
        // the outline they are, NaN elsewhere.
        let mut bank = vec![[f32::NAN; 2]; n];
        for (at, b) in bank.iter_mut().enumerate() {
            if body[at] != NONE || !measured[at].is_finite() {
                continue;
            }
            for (nb, _) in neighbours(at, side) {
                if body[nb] != NONE && !level[nb].is_nan() {
                    *b = [if b[0].is_nan() { level[nb] } else { b[0].min(level[nb]) }, measured[at]];
                }
            }
        }
        let shore: Vec<f32> = measured.iter().zip(&body).map(|(&d, &b)| if b == NONE { 0.0 } else if d.is_finite() { d } else { OPEN_SHORE }).collect();
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
                b => {
                    let bowl = match map.bodies[b as usize].kind {
                        Kind::Sea => Kind::sea_depth(shore[at] as f64, farthest[b as usize]),
                        kind => kind.depth(shore[at] as f64, width[at] as f64),
                    };
                    // The bank shelves in from the waterline at `SHORE_SLOPE` rather than dropping
                    // `SHORE_DEPTH` at once, so the first sample in is not at the foot of a wall.
                    bowl.min(SHORE_SLOPE * shore[at] as f64) as f32
                }
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
        // The current is smoothed over the stretch, then slowed towards the banks.
        blur_flow(&mut flow, side, (FLOW_BLUR / cell_m).round().max(1.0) as usize);
        for at in 0..n {
            let bank = if body[at] == NONE { 0.0 } else { (shore[at] / width[at].max(cell_m as f32)).clamp(0.0, 1.0).sqrt() };
            flow[at] = flow[at].map(|v| v * bank);
        }
        Self { grid, body, level, depth, reach, flow, bank }
    }

    fn index(&self, x: f64, z: f64) -> Option<usize> {
        let at = |v: f64| ((v * 100.0 - self.grid.origin as f64) / self.grid.cell as f64).round();
        let (i, j) = (at(x), at(z));
        let last = self.grid.side as f64 - 1.0;
        (i >= 0.0 && j >= 0.0 && i <= last && j <= last).then(|| j as usize * self.grid.side as usize + i as usize)
    }

    /// The ground elevation at a grid position given the height model's there: under water, the
    /// bed, cut `depth` below the surface whatever the model says (it is flat over open water, and
    /// on a bank sample inside the polygon it is higher than the surface). A dry sample next to the
    /// water is raised, by no more than `BANK_RAISE`, to rise from the waterline at `SHORE_SLOPE`,
    /// so a bank level with the water still meets it on the outline.
    pub fn bed(&self, x: f64, z: f64, elevation: Option<f64>) -> Option<f64> {
        let Some(at) = self.index(x, z) else { return elevation };
        match self.body[at] {
            NONE => {
                let [level, outside] = self.bank[at];
                match elevation {
                    Some(model) if !level.is_nan() => Some(model.max((level as f64 + SHORE_SLOPE * outside as f64).min(model + BANK_RAISE))),
                    _ => elevation,
                }
            }
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
    /// its y = 0, which is `datum` in the height model. Samples next to water carry its level on,
    /// so the drawn shoreline falls where the depth crosses zero between them and the water. One
    /// whose ground does not rise out of the water at `SHORE_SLOPE` takes a surface that far under
    /// its ground instead (`BANK_DROP_CM` to `BANK_DIP_CM`), so the crossing is still on the outline.
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
                    let outside = self.bank[at][1];
                    let under = if outside.is_nan() { BANK_DROP_CM } else { ((SHORE_SLOPE * outside as f64 * 100.0).round() as i32).clamp(BANK_DROP_CM, BANK_DIP_CM) };
                    out[at] = level.min(ground.heights[at] - under);
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

    /// The water's velocity at every ground sample, `x` then `z` in `FLOW_STEP`s, as `Tile::flow` lays it
    /// out; empty when none of it flows.
    pub fn flows(&self) -> Vec<i8> {
        let step = |v: f32| (v / FLOW_STEP).round().clamp(-127.0, 127.0) as i8;
        let out: Vec<i8> = self.flow.iter().flat_map(|&[x, z]| [step(x), step(z)]).collect();
        if out.iter().all(|&v| v == 0) { Vec::new() } else { out }
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

/// The distance of every wet sample from the bank, metres, measured to the outlines themselves:
/// infinite where no bank is found (water whose outline lies wholly outside the grid). Dry samples
/// within `SHORE_BAND` cells of an outline carry their distance outside it too, infinite elsewhere.
///
/// Samples near an outline measure to its nearest edge, but a wet one only to an edge with dry
/// ground beyond it, so where a lake's polygon meets a river's the line between them is no shore.
/// The rest of the water takes the nearest of its neighbours' bank points, swept over the grid both
/// ways (as a vector distance transform does), which is within a few centimetres of exact.
fn shore_distance(map: &Map, grid: Grid, body: &[u32]) -> Vec<f32> {
    let side = grid.side as usize;
    let cell_m = grid.cell as f64 / 100.0;
    let band = SHORE_BAND * cell_m;
    let mut dist = vec![f32::INFINITY; side * side];
    let mut nearest = vec![[f64::NAN; 2]; side * side];
    let to_index = |v: f64| (v * 100.0 - grid.origin as f64) / grid.cell as f64;
    let last = side as i64 - 1;
    let dry = |x: f64, z: f64| {
        let (i, j) = (to_index(x).round(), to_index(z).round());
        i < 0.0 || j < 0.0 || i > last as f64 || j > last as f64 || body[j as usize * side + i as usize] == NONE
    };
    for ring in map.bodies.iter().flat_map(|b| &b.rings) {
        let mut a = *ring.last().unwrap();
        for &b in ring {
            let span = |lo: f64, hi: f64| ((to_index(lo - band).floor() as i64).max(0), (to_index(hi + band).ceil() as i64).min(last));
            let (i0, i1) = span(a[0].min(b[0]), a[0].max(b[0]));
            let (j0, j1) = span(a[1].min(b[1]), a[1].max(b[1]));
            let along = [b[0] - a[0], b[1] - a[1]];
            let length2 = along[0] * along[0] + along[1] * along[1];
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let s = [grid.x(i as u32), grid.x(j as u32)];
                    let t = if length2 > 0.0 { (((s[0] - a[0]) * along[0] + (s[1] - a[1]) * along[1]) / length2).clamp(0.0, 1.0) } else { 0.0 };
                    let p = [a[0] + along[0] * t, a[1] + along[1] * t];
                    let d = (s[0] - p[0]).hypot(s[1] - p[1]);
                    let at = j as usize * side + i as usize;
                    if d > band || d as f32 >= dist[at] {
                        continue;
                    }
                    if body[at] != NONE {
                        // A cell past the edge, away from the sample (or either side of it, for a
                        // sample on the edge), must be dry.
                        let shore = if d > 1e-6 {
                            let k = (d + cell_m) / d;
                            dry(s[0] + (p[0] - s[0]) * k, s[1] + (p[1] - s[1]) * k)
                        } else {
                            let norm = length2.sqrt().max(1e-9);
                            let n = [-along[1] / norm * cell_m, along[0] / norm * cell_m];
                            dry(p[0] + n[0], p[1] + n[1]) || dry(p[0] - n[0], p[1] - n[1])
                        };
                        if !shore {
                            continue;
                        }
                    }
                    dist[at] = d as f32;
                    nearest[at] = p;
                }
            }
            a = b;
        }
    }
    // Sweep the bank points through the water: down the rows, then back up.
    let take = |at: usize, from: usize, dist: &mut [f32], nearest: &mut [[f64; 2]]| {
        let p = nearest[from];
        if body[at] == NONE || p[0].is_nan() {
            return;
        }
        let s = [grid.x((at % side) as u32), grid.x((at / side) as u32)];
        let d = (s[0] - p[0]).hypot(s[1] - p[1]) as f32;
        if d < dist[at] {
            dist[at] = d;
            nearest[at] = p;
        }
    };
    for j in 0..side {
        for i in 0..side {
            let at = j * side + i;
            if i > 0 {
                take(at, at - 1, &mut dist, &mut nearest);
            }
            if j > 0 {
                take(at, at - side, &mut dist, &mut nearest);
                if i > 0 {
                    take(at, at - side - 1, &mut dist, &mut nearest);
                }
                if i + 1 < side {
                    take(at, at - side + 1, &mut dist, &mut nearest);
                }
            }
        }
        for i in (0..side - 1).rev() {
            take(j * side + i, j * side + i + 1, &mut dist, &mut nearest);
        }
    }
    for j in (0..side).rev() {
        for i in (0..side).rev() {
            let at = j * side + i;
            if i + 1 < side {
                take(at, at + 1, &mut dist, &mut nearest);
            }
            if j + 1 < side {
                take(at, at + side, &mut dist, &mut nearest);
                if i + 1 < side {
                    take(at, at + side + 1, &mut dist, &mut nearest);
                }
                if i > 0 {
                    take(at, at + side - 1, &mut dist, &mut nearest);
                }
            }
        }
        for i in 1..side {
            take(j * side + i, j * side + i - 1, &mut dist, &mut nearest);
        }
    }
    dist
}

/// How far from an outline, in cells, `shore_distance` measures to it directly.
const SHORE_BAND: f64 = 2.0;

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
/// A river's current is measured on its fall over this many `RIVER_BIN`s up and down its course.
const FLOW_SLOPE_BINS: f64 = 3.0;
/// The current's speed in the middle of a river is this many m/s times the square root of how
/// steeply it falls there (Manning's law with the depth and roughness folded in), and never slower
/// than `FLOW_MIN` nor faster than `FLOW_MAX`, m/s: a calm pool still drifts, a rapid runs at a run.
const FLOW_K: f64 = 25.0;
const FLOW_MIN: f64 = 0.3;
const FLOW_MAX: f64 = 4.0;
/// A stretch without centre lines to tell its direction flows only if it falls at least this far, metres.
const FLOW_MIN_DROP: f64 = 0.3;
/// The current is smoothed over this far, metres, so it bends round with the river.
const FLOW_BLUR: f64 = 8.0;

/// Gives every river a surface that falls along its course instead of one flat level: a whole
/// river is often one polygon tens of kilometres long, and a level for all of it stands metres
/// above the banks at one end of the world and below them at the other.
///
/// Each connected stretch of a river body is measured along its length (the distance through the
/// water from one end, whose whole cross-section is the start where it leaves the grid), the
/// height model's median is taken every `RIVER_BIN` metres along it, and those medians are fitted
/// with the closest profile that only ever falls one way (isotonic regression, in whichever
/// direction fits better). The surface is level across the river and steps down at rapids.
///
/// It also writes the river's current into `flow`, m/s: along its course, downstream as its centre
/// lines are drawn (or downhill where it has none), as fast as `FLOW_K` makes its fall over
/// `FLOW_SLOPE_BINS` either way. `Surface::new` smooths it and slows it at the banks.
fn river_levels(map: &Map, grid: Grid, body: &[u32], model: &[f32], level: &mut [f32], flow: &mut [[f32; 2]]) {
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
        // Downstream is further along (+1) or back (-1): as its centre lines run, where they cross
        // it, and the surface then falls that way; otherwise the way it fits the ground better.
        let drawn = downstream(map, grid, b, body, &dist);
        let sense = match drawn {
            0 => if error(&falling) <= error(&rising) { 1.0 } else { -1.0 },
            vote => vote.signum() as f64,
        };
        let profile = if sense > 0.0 { falling } else { rising };
        for &at in &members {
            level[at] = interpolate(&centres, &profile, along(at)) as f32;
        }
        let drop = (profile[0] - profile[profile.len() - 1]).abs();
        if drawn != 0 || drop >= FLOW_MIN_DROP {
            // The chamfer's distance leans towards the eight grid directions; the current's direction
            // comes from a true one, marched out from the same end.
            let course = march(&cross_section(end, body, side), side, |next| body[next] == b);
            for &at in &members {
                let a = along(at);
                let fall = (interpolate(&centres, &profile, a - FLOW_SLOPE_BINS) - interpolate(&centres, &profile, a + FLOW_SLOPE_BINS)) * sense;
                let slope = (fall / (2.0 * FLOW_SLOPE_BINS * RIVER_BIN)).max(0.0);
                let speed = (FLOW_K * slope.sqrt()).clamp(FLOW_MIN, FLOW_MAX);
                let [gx, gz] = gradient(at, side, &course, |next| body[next] == b);
                let length = gx.hypot(gz);
                if length > 0.0 {
                    flow[at] = [(gx / length * speed * sense) as f32, (gz / length * speed * sense) as f32];
                }
            }
        }
        let length = bins as f64 * RIVER_BIN;
        if length >= RIVER_REPORTED {
            let drop = (profile[0] - profile[profile.len() - 1]).abs();
            let fastest = members.iter().map(|&at| flow[at][0].hypot(flow[at][1])).fold(0.0f32, f32::max);
            let rapid = members.iter().filter(|&&at| flow[at][0].hypot(flow[at][1]) >= 2.5).count() as f64 * cell_m * cell_m;
            eprintln!("water: a river falls {drop:.1} m over {length:.0} m, running at up to {fastest:.1} m/s, {rapid:.0} m² of it at 2.5 m/s or more");
        }
        for &at in &members {
            dist[at] = u32::MAX;
        }
    }
}

/// How a river stretch's centre lines (`Map::streams`, drawn downstream) run against `dist`, the
/// distance along stretch `b`: positive where they run the way it grows, negative the other way, 0
/// where none crosses it.
fn downstream(map: &Map, grid: Grid, b: u32, body: &[u32], dist: &[u32]) -> i64 {
    let side = grid.side as usize;
    let index = |p: P| {
        let at = |v: f64| ((v * 100.0 - grid.origin as f64) / grid.cell as f64).round();
        let (i, j) = (at(p[0]), at(p[1]));
        let last = grid.side as f64 - 1.0;
        (i >= 0.0 && j >= 0.0 && i <= last && j <= last).then(|| j as usize * side + i as usize).filter(|&at| body[at] == b && dist[at] != u32::MAX)
    };
    let mut vote = 0i64;
    for line in &map.streams {
        for pair in line.windows(2) {
            // Every few metres along the line, so a long straight segment counts as much as its length.
            let length = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            let steps = (length / (grid.cell as f64 / 100.0 * 4.0)).ceil().max(1.0) as usize;
            let point = |k: usize| {
                let t = k as f64 / steps as f64;
                [pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t]
            };
            for k in 0..steps {
                if let (Some(from), Some(to)) = (index(point(k)), index(point(k + 1))) {
                    vote += (i64::from(dist[to]) - i64::from(dist[from])).signum();
                }
            }
        }
    }
    vote
}

/// Which way `dist` grows at `at`, in its units a cell `[x, z]`, from the samples either side that
/// `inside` keeps (one side alone where the other is not).
fn gradient(at: usize, side: usize, dist: &[f64], inside: impl Fn(usize) -> bool) -> [f64; 2] {
    let (i, j) = (at % side, at / side);
    let value = |next: Option<usize>| next.filter(|&next| inside(next) && dist[next].is_finite()).map(|next| dist[next]);
    let here = dist[at];
    let along = |back: Option<usize>, ahead: Option<usize>| match (value(back), value(ahead)) {
        (Some(b), Some(a)) => (a - b) / 2.0,
        (Some(b), None) => here - b,
        (None, Some(a)) => a - here,
        (None, None) => 0.0,
    };
    let x = along((i > 0).then(|| at - 1), (i + 1 < side).then(|| at + 1));
    let z = along((j > 0).then(|| at - side), (j + 1 < side).then(|| at + side));
    [x, z]
}

/// The distance from the nearest seed through the samples `inside` keeps, in cells, as the crow
/// flies where it can (fast marching, solving |∇d| = 1 a sample at a time); infinite where it cannot
/// get to.
fn march(seeds: &[usize], side: usize, inside: impl Fn(usize) -> bool) -> Vec<f64> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    /// A distance the heap can order.
    #[derive(PartialEq)]
    struct D(f64);
    impl Eq for D {}
    impl PartialOrd for D {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for D {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&other.0)
        }
    }
    let mut dist = vec![f64::INFINITY; side * side];
    let mut done = vec![false; side * side];
    let mut queue = BinaryHeap::new();
    for &seed in seeds {
        dist[seed] = 0.0;
        queue.push(Reverse((D(0.0), seed)));
    }
    let known = |dist: &[f64], done: &[bool], next: Option<usize>| next.filter(|&next| done[next]).map_or(f64::INFINITY, |next| dist[next]);
    while let Some(Reverse((D(d), at))) = queue.pop() {
        if done[at] || d > dist[at] {
            continue;
        }
        done[at] = true;
        let (i, j) = (at % side, at / side);
        let around = [(i > 0).then(|| at - 1), (i + 1 < side).then(|| at + 1), (j > 0).then(|| at - side), (j + 1 < side).then(|| at + side)];
        for next in around.into_iter().flatten() {
            if done[next] || !inside(next) {
                continue;
            }
            let (ni, nj) = (next % side, next / side);
            let a = known(&dist, &done, (ni > 0).then(|| next - 1)).min(known(&dist, &done, (ni + 1 < side).then(|| next + 1)));
            let b = known(&dist, &done, (nj > 0).then(|| next - side)).min(known(&dist, &done, (nj + 1 < side).then(|| next + side)));
            let (lo, hi) = (a.min(b), a.max(b));
            let solved = if hi - lo < 1.0 { (lo + hi + (2.0 - (hi - lo) * (hi - lo)).sqrt()) / 2.0 } else { lo + 1.0 };
            if solved < dist[next] {
                dist[next] = solved;
                queue.push(Reverse((D(solved), next)));
            }
        }
    }
    dist
}

/// Averages the current over a square `radius` samples either way, among the samples that flow, so
/// it turns smoothly round a bend; still samples stay still.
fn blur_flow(flow: &mut [[f32; 2]], side: usize, radius: usize) {
    let moving: Vec<bool> = flow.iter().map(|v| v[0] != 0.0 || v[1] != 0.0).collect();
    if !moving.iter().any(|&m| m) {
        return;
    }
    // Two passes, along rows and then down columns, each a running sum of the moving samples.
    let mut pass = |stride: usize, step: usize, lines: usize| {
        let mut sums = vec![[0.0f64; 3]; side + 1];
        for line in 0..lines {
            let start = line * stride;
            for k in 0..side {
                let at = start + k * step;
                let v = if moving[at] { [f64::from(flow[at][0]), f64::from(flow[at][1]), 1.0] } else { [0.0; 3] };
                sums[k + 1] = [sums[k][0] + v[0], sums[k][1] + v[1], sums[k][2] + v[2]];
            }
            for k in 0..side {
                let at = start + k * step;
                if !moving[at] {
                    continue;
                }
                let (lo, hi) = (k.saturating_sub(radius), (k + radius + 1).min(side));
                let count = sums[hi][2] - sums[lo][2];
                flow[at] = [((sums[hi][0] - sums[lo][0]) / count) as f32, ((sums[hi][1] - sums[lo][1]) / count) as f32];
            }
        }
    };
    pass(side, 1, side);
    pass(1, side, side);
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
pub(crate) fn isotonic(values: &[f64], weights: &[f64]) -> Vec<f64> {
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
pub(crate) fn interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
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
        // Distances are to the outline itself: the sample on it is at the waterline, the next one
        // in 10 m from it, and the middle 40 m from every side, the body's radius.
        assert!(depth(40.0).abs() < 1e-4, "{}", depth(40.0));
        assert!((depth(30.0) - Kind::Lake.depth(10.0, 40.0)).abs() < 1e-4, "{}", depth(30.0));
        assert!((depth(0.0) - Kind::Lake.max_depth(40.0)).abs() < 1e-4, "{}", depth(0.0));
        assert!(depth(0.0) > depth(20.0) && depth(20.0) > depth(30.0));
        assert_eq!(surface.bed(80.0, 0.0, Some(7.0)), Some(7.0), "dry ground is untouched");
    }

    /// A round pond 9.3 m across on a 1 m grid, its outline falling anywhere between samples, in
    /// ground flat at 5 m.
    fn pond() -> (Surface, Ground, Vec<i32>) {
        let ring: Vec<P> = (0..48).map(|k| {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            [0.3 + 9.3 * a.cos(), -0.2 + 9.3 * a.sin()]
        }).collect();
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![ring], level: None }], streams: Vec::new() };
        let grid = Grid { origin: -1_500, cell: 100, side: 31 };
        let surface = Surface::new(&map, grid, |_, _| Some(5.0));
        let heights = (0..31 * 31)
            .map(|at| {
                let (x, z) = (grid.x(at % 31), grid.x(at / 31));
                (surface.bed(x, z, Some(5.0)).unwrap() * 100.0).round() as i32
            })
            .collect();
        let ground = Ground { origin: -1_500, cell: 100, side: 31, heights };
        let levels = surface.levels(0.0, &ground);
        (surface, ground, levels)
    }

    #[test]
    fn a_ponds_shoreline_follows_its_outline_not_the_samples() {
        let (_, ground, levels) = pond();
        let depth = |at: usize| if levels[at] == NO_WATER { -1.0 } else { f64::from(levels[at] - ground.heights[at]) / 100.0 };
        let x = |i: usize| -15.0 + i as f64;
        // Where the depth crosses zero along each edge between samples, as the client draws it, the
        // shoreline is on the circle.
        let mut crossings = 0;
        for j in 0..31 {
            for i in 0..31 {
                for (di, dj) in [(1, 0), (0, 1)] {
                    if i + di > 30 || j + dj > 30 {
                        continue;
                    }
                    let (a, b) = (j * 31 + i, (j + dj) * 31 + i + di);
                    let (da, db) = (depth(a), depth(b));
                    if (da > 0.0) == (db > 0.0) {
                        continue;
                    }
                    let t = da / (da - db);
                    let p = [x(i) + t * di as f64, x(j) + t * dj as f64];
                    let off = (p[0] - 0.3).hypot(p[1] + 0.2) - 9.3;
                    assert!(off.abs() < 0.2, "the shoreline is {off:.2} m off the outline at {p:?}");
                    crossings += 1;
                }
            }
        }
        assert!(crossings > 60, "{crossings}");
    }

    #[test]
    fn a_ponds_bank_shelves_in_without_a_wall() {
        let (surface, ground, levels) = pond();
        for (at, (&level, &height)) in levels.iter().zip(&ground.heights).enumerate() {
            if level == NO_WATER || level <= height {
                continue;
            }
            let (x, z) = (-15.0 + (at % 31) as f64, -15.0 + (at / 31) as f64);
            let inside = 9.3 - (x - 0.3).hypot(z + 0.2);
            if inside > 1.5 {
                continue;
            }
            let depth = f64::from(level - height) / 100.0;
            assert!(depth <= SHORE_SLOPE * inside + 0.03, "{depth:.2} m deep {inside:.2} m in");
        }
        // A bank level with the water is lifted to meet it, by no more than `BANK_RAISE`.
        let ring = surface.bed(9.6 + 0.3, -0.2, Some(5.0)).unwrap();
        assert!(ring > 5.0 && ring <= 5.0 + BANK_RAISE + 1e-9, "{ring}");
        assert_eq!(surface.bed(14.0, 0.0, Some(5.0)), Some(5.0), "further out the ground is untouched");
    }

    #[test]
    fn where_a_lake_meets_a_river_the_water_is_not_shallow() {
        let lake = vec![[-40.0, -40.0], [0.0, -40.0], [0.0, 40.0], [-40.0, 40.0]];
        let river = vec![[0.0, -40.0], [40.0, -40.0], [40.0, 40.0], [0.0, 40.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![lake], level: None }, Body { kind: Kind::River, rings: vec![river], level: None }], streams: Vec::new() };
        let surface = Surface::new(&map, grid(), |_, _| Some(0.0));
        assert!(-surface.bed(0.0, 0.0, Some(0.0)).unwrap() > 2.0, "the line between them is open water");
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
        let (east, south) = (current(&surface, -200.0, 0.0), current(&surface, 0.0, 200.0));
        assert!(east[0] > 1.0 && east[1].abs() < 0.1 * east[0], "runs east before the bend: {east:?}");
        assert!(south[1] > 1.0 && south[0].abs() < 0.1 * south[1], "and south after it: {south:?}");
    }

    #[test]
    fn rapids_are_a_step_not_a_flood() {
        // The water drops 3 m at x = 0.
        let map = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(if x < 0.0 { 10.0 } else { 7.0 } + 0.05 * noise(x, z)));
        assert!((level(&surface, -200.0, 0.0) - 10.0).abs() < 0.1, "{}", level(&surface, -200.0, 0.0));
        assert!((level(&surface, 200.0, 0.0) - 7.0).abs() < 0.1, "{}", level(&surface, 200.0, 0.0));
    }

    fn current(surface: &Surface, x: f64, z: f64) -> [f32; 2] {
        surface.flow[surface.index(x, z).unwrap()]
    }

    #[test]
    fn a_falling_river_flows_downhill_and_fastest_at_rapids() {
        // 20 m wide, the ground falling 1 m per km in x, with a 3 m rapid at x = 0.
        let map = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - x / 1000.0 - if x < 0.0 { 0.0 } else { 3.0 } + 0.02 * noise(x, z)));
        let calm = current(&surface, -200.0, 0.0);
        assert!(calm[0] > 0.5 && calm[0] < 1.2 && calm[1].abs() < 0.05, "a gentle stretch drifts downstream: {calm:?}");
        let rapid = current(&surface, 0.0, 0.0);
        assert!(rapid[0] > 3.5 && rapid[0] <= FLOW_MAX as f32 + 1e-3, "the rapid runs: {rapid:?}");
        let bank = current(&surface, -200.0, 8.0);
        assert!(bank[0] > 0.0 && bank[0] < 0.7 * calm[0], "slower by the bank: {bank:?} against {calm:?}");
        let flow = surface.flows();
        let at = surface.index(0.0, 0.0).unwrap();
        assert_eq!(flow.len(), 2 * surface.level.len());
        assert_eq!(flow[2 * at], (rapid[0] / FLOW_STEP).round() as i8, "written in FLOW_STEPs");
    }

    #[test]
    fn a_level_river_runs_the_way_its_centre_line_is_drawn() {
        let ring = vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]];
        let level = |_: f64, _: f64| Some(5.0);
        assert!(Surface::new(&river(ring.clone()), river_grid(), level).flows().is_empty(), "nothing tells which way it runs");
        let map = Map { streams: vec![vec![[300.0, 0.0], [-300.0, 0.0]]], ..river(ring) };
        let surface = Surface::new(&map, river_grid(), level);
        let middle = current(&surface, 0.0, 0.0);
        assert!((middle[0] + FLOW_MIN as f32).abs() < 0.05 && middle[1].abs() < 0.02, "drifts west, as drawn: {middle:?}");
        assert!(Surface::new(&lake(), grid(), |_, _| Some(1.0)).flows().is_empty(), "a lake is still");
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
        assert_eq!(at(5, 0), 1000 - BANK_DIP_CM, "10 m out from the outline, as far under as it dips");
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
