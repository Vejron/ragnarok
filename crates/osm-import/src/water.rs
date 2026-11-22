//! Turns OpenStreetMap's water (`osm_water.rs`) into what the world stores: a water surface height at
//! every ground sample, with lakes and wide rivers carved into the ground under it, and stream
//! centre lines.
//!
//! The data has no surface height or depth. The height model is flat over water, so the surface
//! of a body is the median elevation of the height model inside it, and its bed is cut below that
//! with a depth that grows with the distance from the shore and with the size of the body.
//!
//! A river that OpenStreetMap draws as a line is given banks first (`channel`), and is then a river
//! like one drawn as a polygon.
//!
//! A river is different in three ways. Its surface falls along its course (`river_levels`). It
//! carries one discharge from end to end, and its bed is cut shallower where that water must run
//! faster to get through: in its rapids (`Stretch::shallows`), where it is also rough, with bars,
//! boulders and a chute left deep for boats (`Stretch::relief`). And its current is solved over that
//! bed as a flow (`riverflow`), so it speeds up through a narrows, parts round an island and turns
//! back in the bay below a point.

use rayon::prelude::*;
use std::collections::BTreeMap;
use shared::terrain::Ground;
use shared::world::{FLOW_STEP, LANE_FLUX, LANE_STEPS, NO_WATER, REACH_STEP, STIR_STEP};

use crate::{channel, riverflow};

type P = [f64; 2];

/// Streams are drawn this wide, metres: the data has centre lines only.
pub const STREAM_WIDTH: f64 = 2.5;
/// Steps a stream is walked in to find where it goes into water, metres.
const SHORELINE_STEP: f64 = 0.5;
/// A piece of stream cut shorter than this by the water is left out, metres: the tips the client
/// adds at both its ends would be most of it, a patch of ribbon lying on the shore.
const SHORTEST_ASHORE: f64 = 2.0;
/// Depth of the water at the shore, metres.
const SHORE_DEPTH: f64 = 0.3;
/// How steeply the ground falls into the water at the waterline, and rises out of it on the bank,
/// metres a metre. The bed is no deeper than this times the distance in from the outline, and the
/// depth the client interpolates between a wet and a dry sample then crosses zero on the outline.
const SHORE_SLOPE: f64 = 0.8;
/// The most a bank sample's ground is raised to meet `SHORE_SLOPE`, metres: enough to lift a bank
/// level with the water clear of it, never a dyke.
const BANK_RAISE: f64 = 0.2;
/// How steeply a bank higher than the water is cut down to it, metres a metre, past its first
/// sample spacing out from the outline (which rises at `SHORE_SLOPE`, as the bed falls). A high
/// bank left as the height model has it would meet the water at the nearest wet sample instead,
/// so the drawn shoreline would step along the sample grid rather than follow the outline.
const BANK_STEEP: f64 = 1.5;
/// A bank is cut down to `BANK_STEEP` out to this many sample spacings from the outline...
const BANK_CUT_CELLS: f64 = 2.0;
/// ...and the cut eases back into the height model over this much further out, metres.
const BANK_FADE: f64 = 3.0;
/// A bank cut down to a small river's channel rises no more steeply than this, metres a metre, all
/// the way out to where the cut ends. A channel lies down in a valley and is cut along all its
/// length, so at `BANK_STEEP` whoever fell in could not climb out (`MAX_SLOPE_CLIMB_DEG`, 50°).
const CHANNEL_BANK: f64 = 0.95;
/// The surface carried onto a bank sample whose distance from the outline is not known sits this
/// far under its ground, centimetres...
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
/// The share of a river's width that counts as open water for the wind's swell (`Tile::reach`). A
/// river is sheltered by its banks and its water is moving, so the wind raises only a small chop
/// on it: its waves come from the current instead.
const RIVER_REACH: f32 = 0.25;
/// The most open water a river counts as having however wide it is, metres: enough for the two
/// shortest swells and no more.
const RIVER_REACH_MAX: f32 = 30.0;
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
    /// Rivers' centre lines in the direction the water flows. Where they are outside wide water
    /// `channel::lay` makes channels of them.
    pub river_lines: Vec<Vec<P>>,
    /// The channels along `river_lines`, longest first.
    pub channels: Vec<channel::Channel>,
    /// The bodies that are the channels' water: their own, and the pools along them.
    pub narrow: Vec<usize>,
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
    /// How much of the surface is under foam at each sample, in 255ths (`Tile::foam`); empty when none is.
    pub foam: Vec<u8>,
    /// How the current stirs the surface at each sample, two entries a sample (`Tile::stir`), and its
    /// stream function (`Tile::lane`); both empty when nothing flows.
    pub stir: Vec<i8>,
    pub lane: Vec<i16>,
    /// The samples of a river's channel and of the bank cut down to it, which a bridge crosses from
    /// bank to bank (`bridge`); empty when there are no channels.
    pub channel: Vec<bool>,
    pub streams: Vec<Vec<P>>,
}

impl Placed {
    /// The finished water of `map` over `ground`, whose y = 0 is `datum` in the height model.
    pub fn new(map: &Map, surface: &Surface, datum: f64, ground: &Ground) -> Self {
        if surface.is_empty() {
            return Self { streams: map.streams.clone(), ..Self::default() };
        }
        let (stir, lane) = surface.stirs();
        let mut placed = Self { levels: surface.levels(datum, ground), reach: surface.reaches(), flow: surface.flows(), foam: surface.foams(), stir, lane, channel: surface.channel.clone(), streams: Vec::new() };
        placed.streams = surface.trim_streams(&map.streams).iter().flat_map(|line| placed.out_of_water(ground, line)).collect();
        placed
    }

    /// A stream's centre line without what of it is in a lake, a river or the sea, cut where it
    /// crosses the shoreline as drawn (`depth_at`): a ribbon further in would lie on the bed. The
    /// client runs it `STREAM_TIP` on from there, so it still reaches into the water.
    fn out_of_water(&self, ground: &Ground, line: &[P]) -> Vec<Vec<P>> {
        let mut runs = uncovered(line, SHORELINE_STEP, |p| self.depth_at(ground, p[0], p[1]) > 0.0);
        // A line no water cuts is kept however short it is.
        if runs.len() != 1 || runs[0] != line {
            runs.retain(|run| run.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>() >= SHORTEST_ASHORE);
        }
        runs
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
/// What `river_levels` calls every river sample's body, whichever polygon it came from.
const RIVER: u32 = u32::MAX - 1;

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
    /// How much of the surface is under foam at each sample, 0 to 1: what a river's rapids make,
    /// carried downstream; none on lakes, the sea and dry samples.
    foam: Vec<f32>,
    /// How a river's current stirs its surface at each sample, 1/s (`Tile::stir`): how it wells
    /// up or gathers, and how fast it spins; 0 on lakes, the sea and dry samples.
    stir: Vec<[f32; 2]>,
    /// A river's stream function at each sample, m³/s (`Tile::lane`); 0 off a river.
    lane: Vec<f64>,
    /// On each dry sample next to the water, the lowest surface beside it, metres above the datum,
    /// and how far outside the outline it is, metres; NaN elsewhere. `bed` shapes the bank with it.
    bank: Vec<[f32; 2]>,
    /// `Placed::channel`.
    channel: Vec<bool>,
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
        // The channels' water is measured along its lines, and the wide rivers' by itself, so that a
        // small river keeps its own fall and discharge where it runs out into a big one.
        let mut is_narrow = vec![false; map.bodies.len()];
        for &index in &map.narrow {
            is_narrow[index] = true;
        }
        let along_channels = channel::courses_at(&map.channels, grid, |at| body[at] != NONE && is_narrow[body[at] as usize]);
        let mut stretches = river_levels(map, grid, &body, &model, &along_channels, &mut level);
        let mut narrows = channel_levels(map, grid, &along_channels, &mut level);
        let arms = arm_levels(grid, &along_channels, &model, &mut level);
        stretches.extend(arms);
        stretches.append(&mut narrows.stretches);
        let side = grid.side as usize;
        let cell_m = grid.cell as f64 / 100.0;
        // Metres from the bank of every wet sample, measured to the drawn outline itself rather
        // than to the nearest dry sample, so the bed (and the shoreline the client draws where the
        // depth crosses zero) follows the outline between samples. Also filled in on the dry
        // samples next to the water, for `bed` to shape the bank.
        let measured = shore_distance(map, grid, &body);
        let shore: Vec<f32> = measured.iter().zip(&body).map(|(&d, &b)| if b == NONE { 0.0 } else if d.is_finite() { d } else { OPEN_SHORE }).collect();
        let width = local_radius(&body, &shore, side, cell_m);
        let mut farthest = vec![0.0f64; map.bodies.len()];
        for (at, &b) in body.iter().enumerate() {
            if b != NONE {
                farthest[b as usize] = farthest[b as usize].max(shore[at] as f64);
            }
        }
        // The bowl under each sample: deepest where the water reaches furthest from its banks.
        // The bank shelves in from the waterline at `SHORE_SLOPE` rather than dropping
        // `SHORE_DEPTH` at once, so the first sample in is not at the foot of a wall.
        let shelf = |at: usize| SHORE_SLOPE * shore[at] as f64;
        let bowl: Vec<f32> = (0..n)
            .map(|at| match body[at] {
                NONE => 0.0,
                b => match map.bodies[b as usize].kind {
                    Kind::Sea => Kind::sea_depth(shore[at] as f64, farthest[b as usize]),
                    kind => kind.depth(shore[at] as f64, width[at] as f64),
                }
                .min(shelf(at)) as f32,
            })
            .collect();
        // The share of its bowl each sample keeps: less in a river's rapids, where the same water
        // runs faster and so shallower.
        let mut shallow = vec![1.0f32; n];
        let cut: Vec<(f64, f64)> = stretches.iter().map(|stretch| stretch.shallows(cell_m, &bowl, &mut shallow)).collect();
        let carried: Vec<f64> = cut.iter().map(|&(carried, _)| carried).collect();
        // A river's bed is then roughened (bars, pools and boulders) round a chute left open for
        // boats, so the current has something to gather in and break over.
        let mut depth: Vec<f32> = (0..n).map(|at| bowl[at] * shallow[at]).collect();
        let banks = Banks { bowl: &bowl, shore: &shore, width: &width };
        let chutes: Vec<Vec<usize>> = stretches.iter().map(|stretch| stretch.relief(grid, &banks, &mut depth)).collect();
        let reach = (0..n)
            .map(|at| match body[at] {
                NONE => 0.0,
                b => match map.bodies[b as usize].kind {
                    Kind::Sea => shared::waves::OPEN_SEA,
                    Kind::Lake => 2.0 * farthest[b as usize] as f32,
                    Kind::River => (RIVER_REACH * 2.0 * width[at]).min(RIVER_REACH_MAX),
                },
            })
            .collect();
        // Each flowing stretch's current is solved over the bed just cut: a stretch at a time on
        // its own thread, each one the same every run.
        let mut flow = vec![[0.0f32; 2]; n];
        let mut foam = vec![0.0f32; n];
        let solved: Vec<(Run, f64)> = stretches
            .par_iter()
            .zip(&carried)
            .map(|(stretch, &carried)| {
                let started = std::time::Instant::now();
                (stretch.current(grid, &depth, &banks, carried), started.elapsed().as_secs_f64())
            })
            .collect();
        // A bed too rough to pass its river's water at the depth it was cut to was cut deeper.
        for (stretch, (run, _)) in stretches.iter().zip(&solved) {
            for (&at, &bed) in stretch.members.iter().zip(&run.bed) {
                depth[at] = bed;
            }
        }
        let marks: Vec<riverflow::Marks> = stretches.par_iter().zip(&solved).map(|(stretch, (run, _))| stretch.marks(grid, &depth, &run.current.flow)).collect();
        let mut stir = vec![[0.0f32; 2]; n];
        let mut lane = vec![0.0f64; n];
        for ((((stretch, (Run { current, deeper, .. }, seconds)), marked), &(_, step)), chute) in stretches.iter().zip(&solved).zip(&marks).zip(&cut).zip(&chutes) {
            let cover = &marked.foam;
            for (k, (&at, &velocity)) in stretch.members.iter().zip(&current.flow).enumerate() {
                flow[at] = velocity;
                foam[at] = cover[k];
                stir[at] = [marked.boil[k], marked.spin[k]];
                lane[at] = marked.lane[k];
            }
            if stretch.length >= RIVER_REPORTED {
                let speeds = || current.flow.iter().map(|v| v[0].hypot(v[1]));
                let fastest = speeds().fold(0.0f32, f32::max);
                let rapid = speeds().filter(|&speed| speed >= 2.5).count() as f64 * cell_m * cell_m;
                let shallowest = stretch.members.iter().map(|&at| shallow[at]).fold(1.0f32, f32::min);
                let chute = chute.iter().map(|&at| depth[at]).fold(f32::MAX, f32::min);
                let foamy = cover.iter().filter(|&&cover| cover >= 0.1).count() as f64 * cell_m * cell_m;
                eprintln!(
                    "water: a river falls {:.1} m over {:.0} m, carrying {:.0} m³/s at up to {fastest:.1} m/s, {rapid:.0} m² of it at 2.5 m/s or more and {foamy:.0} m² under foam; its steepest rapid keeps {:.0}% of its depth, its rapids' rough bed was cut {deeper:.2} times deeper to carry it, the chute down it is nowhere shallower than {chute:.1} m, and its bed's mean depth steps {step:.2} m at most from one {RIVER_BIN:.0} m to the next ({} in {} steps, {seconds:.1} s)",
                    stretch.drop,
                    stretch.length,
                    stretch.discharge(cell_m, &depth, &current.flow),
                    shallowest * 100.0,
                    if current.settled { "settled" } else { "still shifting" },
                    current.steps,
                );
            }
        }
        // The current then shapes the surface it ran under: the water stands higher where it is
        // braked and lower where it runs fast, and a rapid stands in waves. The bed stays where it
        // was cut, so the water over it is deeper or shallower by as much.
        for (stretch, (run, _)) in stretches.iter().zip(&solved) {
            for (&at, [lift, wave]) in stretch.members.iter().zip(stretch.shape(grid, &depth, run)) {
                // Water a boat can pass (the chute's) stays deep enough for one, and shallower
                // water is not lowered: a boulder's top stays as far under as it was put. The
                // waves stand on whatever the lift leaves.
                let deep = (depth[at] + lift).max(depth[at].min(RAPID_DEPTH)) + wave;
                // The same water passes, so it runs slower where it stands deeper.
                if deep > 0.0 {
                    flow[at] = flow[at].map(|v| v * depth[at] / deep);
                }
                level[at] += deep - depth[at];
                depth[at] = deep;
            }
        }
        // The current lifted or lowered the water the channels come from and run out into: they follow
        // it at their ends, so there is no step at a mouth.
        for mouth in &narrows.mouths {
            let now = level[mouth.beside];
            let moved = now - mouth.level;
            // Water risen where the channel runs out backs up into it as far as it stands higher, and
            // water sunk where it starts is as high as any of it: eased in, either would run uphill.
            let backs = if mouth.out { moved > 0.0 } else { moved < 0.0 };
            for &(at, share) in &mouth.members {
                let wanted = match (backs, mouth.out) {
                    (true, true) => (now - level[at]).max(0.0),
                    (true, false) => (now - level[at]).min(0.0),
                    (false, _) => moved * share,
                };
                // Never so far down that the bed is dry: the river keeps its outline.
                let moved = wanted.max(MOUTH_SHALLOWEST - depth[at]).min(SURFACE_LIFT_MAX as f32);
                level[at] += moved;
                depth[at] += moved;
            }
        }
        // A channel too short to solve runs at a river's usual speed down its line.
        for &(at, course) in &narrows.short {
            flow[at] = course.map(|part| (part * USUAL_SPEED) as f32);
        }
        // A river is never quite still: where its current is too slow to write down (in the slack
        // water along a bank) it is written as one step of `Tile::flow` down the course. The water
        // there is then a river's to the eye, not a lake's, and what floats on it creeps on.
        for (stretch, (run, _)) in stretches.iter().zip(&solved) {
            if run.current.flow.iter().all(|v| v[0] == 0.0 && v[1] == 0.0) {
                continue;
            }
            for (&at, course) in stretch.members.iter().zip(&stretch.course) {
                if flow[at][0].abs() < 0.5 * FLOW_STEP && flow[at][1].abs() < 0.5 * FLOW_STEP {
                    flow[at] = course.map(|part| part as f32 * FLOW_STEP);
                }
            }
        }
        // The dry samples near the water: the lowest level beside each and how far outside the
        // outline they are, NaN elsewhere. Those next to the water take its level; those further
        // out take the lowest a bank sample beside them took, a ring at a time.
        let mut bank = vec![[f32::NAN; 2]; n];
        let mut ring: Vec<usize> = Vec::new();
        for (at, b) in bank.iter_mut().enumerate() {
            if body[at] != NONE || !measured[at].is_finite() {
                continue;
            }
            for (nb, _) in neighbours(at, side) {
                if body[nb] != NONE && !level[nb].is_nan() {
                    *b = [if b[0].is_nan() { level[nb] } else { b[0].min(level[nb]) }, measured[at]];
                }
            }
            if !b[0].is_nan() {
                ring.push(at);
            }
        }
        while !ring.is_empty() {
            let mut next = Vec::new();
            for &at in &ring {
                for (nb, _) in neighbours(at, side) {
                    if body[nb] != NONE || !measured[nb].is_finite() || !bank[nb][0].is_nan() && bank[nb][0] <= bank[at][0] {
                        continue;
                    }
                    if bank[nb][0].is_nan() {
                        next.push(nb);
                    }
                    bank[nb] = [bank[at][0], measured[nb]];
                }
            }
            ring = next;
        }
        // The channels, and as much of the ground beside them as was cut down to them.
        let mut channel = if along_channels.is_empty() { Vec::new() } else { vec![false; n] };
        let cut_band = (bank_band(cell_m) / cell_m).ceil() as i64;
        for &at in along_channels.keys() {
            for (di, dj) in (-cut_band..=cut_band).flat_map(|dj| (-cut_band..=cut_band).map(move |di| (di, dj))) {
                let (i, j) = ((at % side) as i64 + di, (at / side) as i64 + dj);
                if i < 0 || j < 0 || i >= side as i64 || j >= side as i64 {
                    continue;
                }
                let beside = j as usize * side + i as usize;
                if (di, dj) == (0, 0) || body[beside] == NONE && measured[beside].is_finite() {
                    channel[beside] = true;
                }
            }
        }
        Self { grid, body, level, depth, reach, flow, foam, stir, lane, bank, channel }
    }

    fn index(&self, x: f64, z: f64) -> Option<usize> {
        self.grid.index(x, z)
    }

    /// The ground elevation at a grid position given the height model's there: under water, the
    /// bed, cut `depth` below the surface whatever the model says (it is flat over open water, and
    /// on a bank sample inside the polygon it is higher than the surface). A dry sample near the
    /// water rises from the waterline in proportion to its distance outside the outline, as the bed
    /// falls inside it, so the ground crosses the surface on the outline whatever the height model
    /// does between samples: one too low is raised, by no more than `BANK_RAISE`, to `SHORE_SLOPE`;
    /// one too high is cut down to `BANK_STEEP`, the cut easing out `BANK_FADE` further on.
    pub fn bed(&self, x: f64, z: f64, elevation: Option<f64>) -> Option<f64> {
        let Some(at) = self.index(x, z) else { return elevation };
        match self.body[at] {
            NONE => {
                let [level, outside] = self.bank[at];
                match elevation {
                    Some(model) if !level.is_nan() => {
                        let (level, outside, cell) = (f64::from(level), f64::from(outside), self.grid.cell as f64 / 100.0);
                        Some(if self.channel.get(at) == Some(&true) { channel_bank_height(model, level, outside, cell) } else { bank_height(model, level, outside, cell) })
                    }
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
                    let under = if outside.is_nan() { BANK_DROP_CM } else { ((SHORE_SLOPE * outside as f64 * 100.0).round() as i32).min(BANK_DIP_CM) };
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

    /// The foam on the water at every ground sample, in 255ths, as `Tile::foam` lays it out; empty
    /// when there is none.
    pub fn foams(&self) -> Vec<u8> {
        let out: Vec<u8> = self.foam.iter().map(|&cover| (cover * 255.0).round().clamp(0.0, 255.0) as u8).collect();
        if out.iter().all(|&v| v == 0) { Vec::new() } else { out }
    }

    /// How the current stirs the surface at every ground sample, two entries a sample in steps of
    /// `STIR_STEP`, and its stream function, in `1 / LANE_STEPS` of `LANE_FLUX` and wrapping round,
    /// as `Tile::stir` and `Tile::lane` lay them out; both empty when nothing flows.
    pub fn stirs(&self) -> (Vec<i8>, Vec<i16>) {
        if self.flow.iter().all(|v| v[0] == 0.0 && v[1] == 0.0) {
            return (Vec::new(), Vec::new());
        }
        let step = |v: f32| (v / STIR_STEP).round().clamp(-127.0, 127.0) as i8;
        let stir = self.stir.iter().flat_map(|&[boil, spin]| [step(boil), step(spin)]).collect();
        // Wrapping round on purpose: the client only reads where it is among the 64 streaks.
        let lane = self.lane.iter().map(|&psi| (psi / f64::from(LANE_FLUX) * f64::from(LANE_STEPS)).round() as i64 as i16).collect();
        (stir, lane)
    }


    /// Stream centre lines without the vertices that lie in the middle of a lake or river (a
    /// vertex stays when a neighbour is dry, so a stream still reaches the shore). What is left is
    /// then cut at the waterline (`Placed::out_of_water`), which cannot do this by itself: a line
    /// OSM draws through a pool has long straight stretches that stray over the bank between points
    /// that are in the water, and those are no stream.
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
    /// The sample nearest local `(x, z)`, `None` off the grid.
    fn index(&self, x: f64, z: f64) -> Option<usize> {
        let at = |v: f64| ((v * 100.0 - self.origin as f64) / self.cell as f64).round();
        let (i, j) = (at(x), at(z));
        let last = self.side as f64 - 1.0;
        (i >= 0.0 && j >= 0.0 && i <= last && j <= last).then(|| j as usize * self.side as usize + i as usize)
    }

    /// Local metres of sample index `i` along either axis.
    fn x(&self, i: u32) -> f64 {
        (self.origin + i as i32 * self.cell) as f64 / 100.0
    }
}

/// The ground of a dry sample `outside` metres out from the outline of water at `level`, metres,
/// where the height model has `model` and the samples are `cell` metres apart (see `Surface::bed`).
fn bank_height(model: f64, level: f64, outside: f64, cell: f64) -> f64 {
    let raised = model.max((level + SHORE_SLOPE * outside).min(model + BANK_RAISE));
    let ceiling = level + SHORE_SLOPE * outside.min(cell) + BANK_STEEP * (outside - cell).max(0.0);
    let shaped = raised.min(ceiling);
    // How much of the height model comes back, from none at `BANK_CUT_CELLS` out to all of it
    // `BANK_FADE` further on.
    let t = ((outside - BANK_CUT_CELLS * cell) / BANK_FADE).clamp(0.0, 1.0);
    let keep = t * t * (3.0 - 2.0 * t);
    shaped + (model - shaped) * keep
}

/// The same beside a small river's channel: a slope that can be climbed (`CHANNEL_BANK`). The cut is
/// not eased back into the height model, which would steepen it. Only ground all but level with the
/// water is raised: the channel keeps under its banks, and ground raised higher up stands through a
/// road or a deck's end that lies on it.
fn channel_bank_height(model: f64, level: f64, outside: f64, cell: f64) -> f64 {
    let lift = ((level + (SHORE_SLOPE * outside).min(BANK_RAISE)).min(model + BANK_RAISE) - model).max(0.0);
    let keep = smoothstep(cell, BANK_CUT_CELLS * cell + BANK_FADE, outside);
    (model + lift * (1.0 - keep)).min(level + SHORE_SLOPE * outside.min(cell) + CHANNEL_BANK * (outside - cell).max(0.0))
}

/// How far out from an outline dry samples measure their distance to it, metres, for `bank_height`.
fn bank_band(cell: f64) -> f64 {
    BANK_CUT_CELLS * cell + BANK_FADE
}

/// The distance of every wet sample from the bank, metres, measured to the outlines themselves:
/// infinite where no bank is found (water whose outline lies wholly outside the grid). Dry samples
/// within `bank_band` of an outline carry their distance outside it too, infinite elsewhere.
///
/// Samples near an outline measure to its nearest edge, but a wet one only to an edge with dry
/// ground beyond it, so where a lake's polygon meets a river's the line between them is no shore.
/// The rest of the water takes the nearest of its neighbours' bank points, swept over the grid both
/// ways (as a vector distance transform does), which is within a few centimetres of exact.
fn shore_distance(map: &Map, grid: Grid, body: &[u32]) -> Vec<f32> {
    let side = grid.side as usize;
    let cell_m = grid.cell as f64 / 100.0;
    let band = SHORE_BAND * cell_m;
    // Dry samples measure further out, for the bank to be shaped (`bank_height`).
    let dry_band = band.max(bank_band(cell_m));
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
            let span = |lo: f64, hi: f64| ((to_index(lo - dry_band).floor() as i64).max(0), (to_index(hi + dry_band).ceil() as i64).min(last));
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
                    if d > if body[at] == NONE { dry_band } else { band } || d as f32 >= dist[at] {
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
/// A channel shorter than this, metres, has too few cross-sections to solve a current over.
const CHANNEL_SOLVED: f64 = 3.0 * RIVER_BIN;
/// A pool is its channel's, level across, out to this far from the channel, metres through the
/// water. Further out it is an arm, with a fall of its own (`arm_levels`).
const ARM_OUT: f64 = 24.0;
/// A stretch at least this long, metres, has its fall reported.
const RIVER_REPORTED: f64 = 500.0;
/// A river's fall is measured over this many `RIVER_BIN`s up and down its course.
const FLOW_SLOPE_BINS: f64 = 3.0;
/// A river that flows counts as falling at least this steeply, metres a metre (15 cm a kilometre),
/// so a level stretch drawn with a centre line still drifts at a few decimetres a second.
const FLOW_MIN_SLOPE: f64 = 1.5e-4;
/// A stretch without centre lines to tell its direction flows only if it falls at least this far, metres.
const FLOW_MIN_DROP: f64 = 0.3;
/// How fast a river's water moves through its usual cross-section, m/s. With the width and depth
/// there it sets the water the river carries, and its rapids are cut to carry the same.
const USUAL_SPEED: f64 = 0.4;
/// A rapid keeps at least this share of its bowl's depth, so a small river's rapid is shallow enough
/// to run but never dry...
const RAPID_SHARE: f32 = 0.25;
/// ...and no more of it than this many metres needs keeping: enough for a boat's draft (the
/// deepest, the AMV's, is 1.2 m) down the middle of a wide river.
const RAPID_DEPTH: f32 = 1.2;
/// No current is written faster than this, m/s. A rapid runs at up to about 4 m/s; only a gap a
/// few samples wide, where the outline pinches a channel, squeezes the water harder.
const FLOW_MAX: f32 = 5.0;
/// A rapid's bed is made no more than this many times deeper to carry the river's water past its
/// bars and boulders.
const DEEPENED: f32 = 1.4;
/// Water comes into a stretch and leaves it over this many `RIVER_BIN`s at each end of its course.
const FLOW_END_BINS: f64 = 1.0;
/// The current lifts or lowers a river's surface by no more than this, metres: a pinch the solver
/// squeezes the water through would otherwise stand it a storey high.
const SURFACE_LIFT_MAX: f64 = 0.4;
/// ...and a small river's by no more than this, metres. Between banks 10 m apart what lifts a big
/// river's surface a little on one side would stand across all of this one, as a hump before
/// every narrows.
const CHANNEL_LIFT_MAX: f64 = 0.06;
/// The surface takes its shape from the current over this many `RIVER_BIN`s in from where the
/// water comes in and leaves, so it meets the even surface at the stretch's ends without a step.
const SURFACE_END_BINS: f64 = 3.0;
/// A rapid's standing waves rise this high from the even surface to a crest, metres, where the
/// water runs fastest and thinnest...
const TRAIN_HEIGHT: f64 = 0.25;
/// ...times something between these, by the patch of the rapid they stand in: most of a rapid's
/// waves are low and a few stand twice as high as the rest...
const TRAIN_UNEVEN: [f64; 2] = [0.2, 2.2];
/// ...in patches about this far across, metres.
const TRAIN_PATCH: f64 = 9.0;
/// A crest lies this far up or down the river from an even line across it, metres, wandering
/// along its length about once in `TRAIN_WANDERS` metres.
const TRAIN_WANDER: f64 = 1.6;
const TRAIN_WANDERS: f64 = 7.0;
/// ...and never more than this share of the water's depth.
const TRAIN_DEPTH_SHARE: f64 = 0.4;
/// The water stands in waves where it runs fast (`shared::waves::CHURN_SPEEDS`, whose surge the sim
/// adds to them) and is also thin for its speed: from the first of these Froude numbers (its speed over
/// `√(g · depth)`), fully from the second. A deep run at the same speed stays smooth.
const TRAIN_FROUDES: [f64; 2] = [0.5, 0.9];
/// The lengths the standing waves come in, metres, crest to crest. A wave stands on water that
/// runs as fast as the wave would travel, which is `2π · speed² / g` long; the water takes the two
/// lengths nearest that. A length under `TRAIN_SAMPLES` samples of the grid is left out.
const TRAIN_LENGTHS: [f64; 3] = [4.0, 5.2, 6.8];
/// A standing wave needs this many samples of the grid to its length to show.
const TRAIN_SAMPLES: f64 = 4.0;

/// A rapid's bed swings between its mean depth divided by this and times it, from bar to pool...
const RAPID_SWING: f64 = 2.2;
/// ...and a pool's by this, which leaves it all but the smooth bowl it was.
const POOL_SWING: f64 = 1.15;
/// A river that falls less steeply than the first of these, metres a metre, is a pool for
/// `POOL_SWING`, and one that falls more steeply than the second is all rapid for `RAPID_SWING`.
const RAPID_SLOPES: [f64; 2] = [0.0005, 0.004];
/// How wide a river's bars and pools are, metres: the big ones, and the small ones on them. Each
/// is drawn out along the current to about `BAR_DRAWN` times as long.
const BAR_WIDTHS: [f64; 2] = [40.0, 10.0];
/// How far up and down the course a bar's shape is averaged, in its widths each way.
const BAR_DRAWN: f64 = 1.5;
/// A river strews boulders where it falls more steeply than the first of these, metres a metre,
/// and as thickly as it ever does from the second.
const BOULDER_SLOPES: [f64; 2] = [0.003, 0.008];
/// One boulder may lie in each square this wide, metres...
const BOULDER_SPACING: f64 = 10.0;
/// ...and in the steepest water this share of the squares hold one.
const BOULDER_COVER: f64 = 0.7;
/// A boulder (or a few lying together) is a lump this far across from its middle, metres: no
/// smaller, or the current's 2 m cells would miss it.
const BOULDER_RADII: [f64; 2] = [1.2, 2.6];
/// A boulder's top lies this far under the surface, metres.
const BOULDER_TOPS: [f64; 2] = [0.15, 0.4];
/// The chute down a river reaches this far to each side of its line, metres, and fades out over
/// half as much again.
const CHUTE_HALF: f64 = 4.0;
/// How strongly the chute's line keeps to deep water: a step over a bed this deep, metres, costs it
/// about twice a step over a bottomless one, and one over a bed a third as deep nine times as much
/// again, so it will not cut a bend along the shallow bank inside it.
const CHUTE_KEEPS_DEEP: f64 = 3.0;

/// What `Stretch::relief` reads of the water round a river, each under every sample of the grid.
struct Banks<'a> {
    /// The bowl cut under the sample, metres deep.
    bowl: &'a [f32],
    /// How far the sample is from the bank, metres.
    shore: &'a [f32],
    /// How far the water round the sample reaches from its banks, metres.
    width: &'a [f32],
}

/// 0 below `from`, 1 above `to`, and eased between them.
fn smoothstep(from: f64, to: f64, x: f64) -> f64 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The stretches of `line` outside what `covered` covers: it is walked in `step`s, and each crossing
/// halved down to the spot. A line nothing covers comes back as it was.
pub(crate) fn uncovered(line: &[P], step: f64, covered: impl Fn(P) -> bool) -> Vec<Vec<P>> {
    let Some(&first) = line.first() else { return Vec::new() };
    let mut runs = Vec::new();
    let mut was = covered(first);
    let mut run: Vec<P> = if was { Vec::new() } else { vec![first] };
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / step).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            if covered(at(t)) == was {
                continue;
            }
            // Halve the step down to the edge: `lo` stays on the side the line came from.
            let (mut lo, mut hi) = ((k - 1) as f64 / steps as f64, t);
            for _ in 0..12 {
                let middle = (lo + hi) / 2.0;
                if covered(at(middle)) == was { lo = middle } else { hi = middle }
            }
            if was {
                run.push(at(hi));
            } else {
                run.push(at(lo));
                runs.push(std::mem::take(&mut run));
            }
            was = !was;
        }
        if !was {
            run.push(b);
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

/// A number from 0 to 1 for the grid square `(i, j)`, the same every run and unlike its
/// neighbours' or another `seed`'s.
fn chance(i: i64, j: i64, seed: u64) -> f64 {
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
    for _ in 0..2 {
        h ^= h >> 32;
        h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    }
    ((h ^ (h >> 32)) >> 11) as f64 / (1u64 << 53) as f64
}

/// A smooth noise of the place, -1 to 1, that turns about once over each unit of `x` and `z`; the
/// same every run.
pub(crate) fn noise(x: f64, z: f64, seed: u64) -> f64 {
    let (i, j) = (x.floor(), z.floor());
    let (fx, fz) = (smoothstep(0.0, 1.0, x - i), smoothstep(0.0, 1.0, z - j));
    let corner = |di: i64, dj: i64| 2.0 * chance(i as i64 + di, j as i64 + dj, seed) - 1.0;
    let near = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * fx;
    let far = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * fx;
    near + (far - near) * fz
}

/// The noise that shapes a river's bars at `place` (local metres `[x, z]`), -1 to 1: `noise` with
/// features `width` m across, averaged along the line of the `course` there (a unit direction), so
/// a bar or a pool lies drawn out along the current as the water leaves it. Each `seed`'s noise is
/// turned a different way, or the grid it is made on would show in the bed.
fn drawn_out(place: [f64; 2], course: [f64; 2], width: f64, seed: u64) -> f64 {
    const TAPS: i32 = 3;
    let (sin, cos) = (seed as f64 * 1.1).sin_cos();
    let mut sum = 0.0;
    for tap in -TAPS..=TAPS {
        let along = f64::from(tap) / f64::from(TAPS) * BAR_DRAWN * width;
        let (x, z) = (place[0] + course[0] * along, place[1] + course[1] * along);
        sum += noise((x * cos - z * sin) / width, (x * sin + z * cos) / width, seed);
    }
    sum / f64::from(2 * TAPS + 1)
}

/// The bed at `place` (local metres `[x, z]`), `depth` m under the surface, with whatever boulder
/// lies there: a lump that rises to its top (`BOULDER_TOPS`) at its middle. A bed already
/// shallower than a boulder's top is left alone. Where the river falls at `slope` there are none
/// below `BOULDER_SLOPES`.
fn boulders(place: [f64; 2], slope: f64, depth: f32) -> f32 {
    let cover = BOULDER_COVER * smoothstep(BOULDER_SLOPES[0], BOULDER_SLOPES[1], slope);
    if cover <= 0.0 {
        return depth;
    }
    let pick = |range: [f64; 2], t: f64| range[0] + (range[1] - range[0]) * t;
    let square = place.map(|v| (v / BOULDER_SPACING).floor() as i64);
    let mut bed = f64::from(depth);
    for (di, dj) in (-1..=1).flat_map(|dj| (-1..=1).map(move |di| (di, dj))) {
        let (i, j) = (square[0] + di, square[1] + dj);
        if chance(i, j, 3) >= cover {
            continue;
        }
        // Its middle keeps clear of the square's edges, so the lump stays within the next square.
        let middle = [(i as f64 + 0.2 + 0.6 * chance(i, j, 4)) * BOULDER_SPACING, (j as f64 + 0.2 + 0.6 * chance(i, j, 5)) * BOULDER_SPACING];
        let (radius, top) = (pick(BOULDER_RADII, chance(i, j, 6)), pick(BOULDER_TOPS, chance(i, j, 7)));
        let out = (place[0] - middle[0]).hypot(place[1] - middle[1]) / radius;
        if out < 1.0 && f64::from(depth) > top {
            bed = bed.min(top + (f64::from(depth) - top) * out * out);
        }
    }
    bed as f32
}

/// A connected stretch of a river that flows, measured along its course: what its bed's depth and
/// its current (`riverflow`) are worked out from.
struct Stretch {
    /// Its samples, as indices into the grid.
    members: Vec<usize>,
    /// How far along the course each member is, in `RIVER_BIN`s from the end it was measured from.
    along: Vec<f64>,
    /// How steeply the surface falls at each member, metres a metre, never under `FLOW_MIN_SLOPE`.
    slope: Vec<f64>,
    /// The unit direction downstream at each member, `[x, z]` (zero where the course has none).
    course: Vec<[f64; 2]>,
    /// The stretch's length along its course and its surface's fall over it, metres.
    length: f64,
    drop: f64,
    /// The most its current lifts or lowers its surface, metres.
    lift_max: f64,
}

/// What `Stretch::current` found.
struct Run {
    /// The water's velocity at each member (`riverflow::solve`), scaled to carry the river's water.
    current: riverflow::Current,
    /// The bed's depth at each member, metres: deeper than it was cut where it passed too little.
    bed: Vec<f32>,
    /// How many times deeper a rapid's bed was made.
    deeper: f32,
}

/// A cross-section of a river, over one `RIVER_BIN` of its course.
struct Section {
    /// How wide the water is across all its channels, metres.
    width: f64,
    /// Its bowl's mean depth, metres.
    depth: f64,
    /// How steeply the surface falls there, metres a metre.
    slope: f64,
}

impl Stretch {
    /// The stretch's cross-sections, one for each `RIVER_BIN` along its course, over the bowl
    /// `bowl` cut under every sample of the grid: each one's wetted width, mean depth and slope.
    /// The width and depth are taken over the bin and the one either side of it, since one bin
    /// alone cuts a bend or a fork raggedly.
    fn sections(&self, cell_m: f64, bowl: &[f32]) -> Vec<Section> {
        let bins = self.along.iter().map(|&a| a as usize).max().unwrap_or(0) + 1;
        // Each bin's wet area, m², the volume over it, m³, and its samples' slopes summed.
        let mut sums = vec![[0.0f64; 3]; bins];
        for (k, &at) in self.members.iter().enumerate() {
            let bin = &mut sums[self.along[k] as usize];
            *bin = [bin[0] + cell_m * cell_m, bin[1] + f64::from(bowl[at]) * cell_m * cell_m, bin[2] + self.slope[k]];
        }
        (0..bins)
            .map(|k| {
                let near = &sums[k.saturating_sub(1)..(k + 2).min(bins)];
                let (area, volume) = near.iter().fold((0.0, 0.0), |(a, v), bin| (a + bin[0], v + bin[1]));
                let own = (sums[k][0] / (cell_m * cell_m)).max(1.0);
                Section { width: area / (near.len() as f64 * RIVER_BIN), depth: if area > 0.0 { volume / area } else { 0.0 }, slope: (sums[k][2] / own).max(FLOW_MIN_SLOPE) }
            })
            .collect()
    }

    /// The water the stretch is made to carry, m³/s: what runs at `USUAL_SPEED` through its usual
    /// cross-section, the median of `sections` by area.
    fn carries(sections: &[Section]) -> f64 {
        let mut areas: Vec<f64> = sections.iter().map(|section| section.width * section.depth).collect();
        areas.sort_by(f64::total_cmp);
        USUAL_SPEED * areas[areas.len() / 2]
    }

    /// Writes into `shallow` the share of its bowl's depth (`bowl`, under every sample of the
    /// grid) each sample keeps, and returns the water the stretch carries, m³/s. By Manning's law
    /// that water passes a cross-section `width` wide that falls at `slope` at the mean depth
    /// `(carried · n / (width · √slope))^(3/5)`: little in a wide, steep rapid, more where the
    /// river is squeezed, and more than the bowl holds in a pool, which keeps its bowl. Every
    /// channel of a cross-section keeps the same share, so the water has no deep way round a rapid.
    ///
    /// With the water carried comes the biggest step in the bed's mean depth from one cross-section
    /// to the next, metres: what a depth taken from each place's own slope might leave as a ledge.
    fn shallows(&self, cell_m: f64, bowl: &[f32], shallow: &mut [f32]) -> (f64, f64) {
        let sections = self.sections(cell_m, bowl);
        let carried = Self::carries(&sections);
        let centres: Vec<f64> = (0..sections.len()).map(|k| k as f64 + 0.5).collect();
        let kept: Vec<f64> = sections
            .iter()
            .map(|section| {
                if section.width <= 0.0 || section.depth <= 0.0 {
                    return 1.0;
                }
                let needed = (carried * riverflow::MANNING_N / (section.width * section.slope.sqrt())).powf(0.6);
                (needed / section.depth).min(1.0)
            })
            .collect();
        for (&at, &along) in self.members.iter().zip(&self.along) {
            shallow[at] = interpolate(&centres, &kept, along) as f32;
        }
        let means: Vec<f64> = sections.iter().zip(&kept).map(|(section, kept)| section.depth * kept).collect();
        let step = means.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0, f64::max);
        (carried, step)
    }

    /// Roughens the stretch's bed. `depth` holds, under every sample of the grid, the smooth bed
    /// `shallows` asked for (its share of the bowl), and leaves with:
    ///
    /// - bars and pools: the depth swung up and down by a smooth noise of the place, widely where
    ///   the river falls as a rapid (`RAPID_SWING`) and hardly at all in a pool (`POOL_SWING`);
    /// - boulders where the river falls steeply (`BOULDER_SLOPES`): lumps that rise to just under
    ///   the surface, never through it, so the river's outline is what was drawn;
    /// - each cross-section still passing the river's water: its depths are scaled so that the
    ///   mean of `depth^(5/3)`, which is what Manning's law lets through, is the smooth bed's;
    /// - a chute: a lane down the deepest way from end to end (`chute`), with no boulder in it and
    ///   never shallower than `RAPID_DEPTH` where the bowl is deep enough, so a boat still gets
    ///   up a wide river's rapid.
    ///
    /// The noise is of the place alone, so the bed is the same every run. It returns the
    /// samples along the chute's line, as indices into the grid.
    fn relief(&self, grid: Grid, banks: &Banks, depth: &mut [f32]) -> Vec<usize> {
        let side = grid.side as usize;
        let cell_m = grid.cell as f64 / 100.0;
        let place = |at: usize| [grid.x((at % side) as u32), grid.x((at / side) as u32)];
        let smooth: Vec<f32> = self.members.iter().map(|&at| depth[at]).collect();
        let mut bars: Vec<f32> = self
            .members
            .iter()
            .zip(&smooth)
            .zip(&self.course)
            .zip(&self.slope)
            .map(|(((&at, &smooth), &course), &slope)| {
                let rapid = smoothstep(RAPID_SLOPES[0], RAPID_SLOPES[1], slope);
                let swing = POOL_SWING.ln() + (RAPID_SWING.ln() - POOL_SWING.ln()) * rapid;
                let wave = 2.2 * (0.7 * drawn_out(place(at), course, BAR_WIDTHS[0], 1) + 0.5 * drawn_out(place(at), course, BAR_WIDTHS[1], 2));
                smooth * (swing * wave.clamp(-1.0, 1.0)).exp() as f32
            })
            .collect();
        let mut rough: Vec<f32> = self.members.iter().zip(&bars).zip(&self.slope).map(|((&at, &bar), &slope)| boulders(place(at), slope, bar)).collect();
        // What each `RIVER_BIN` lets through, before and after: the sums of `depth^(5/3)`.
        let bins = self.along.iter().map(|&a| a as usize).max().unwrap_or(0) + 1;
        let mut through = vec![[0.0f64; 2]; bins];
        for ((&along, &smooth), &rough) in self.along.iter().zip(&smooth).zip(&rough) {
            let bin = &mut through[along as usize];
            *bin = [bin[0] + f64::from(smooth).powf(5.0 / 3.0), bin[1] + f64::from(rough).powf(5.0 / 3.0)];
        }
        let centres: Vec<f64> = (0..bins).map(|k| k as f64 + 0.5).collect();
        let scales: Vec<f64> = through.iter().map(|&[smooth, rough]| if rough > 0.0 { (smooth / rough).powf(0.6).clamp(0.6, 1.6) } else { 1.0 }).collect();
        for ((bar, rough), &along) in bars.iter_mut().zip(&mut rough).zip(&self.along) {
            let scale = interpolate(&centres, &scales, along) as f32;
            *bar *= scale;
            *rough *= scale;
        }
        let (lane, line) = self.chute(side, cell_m, &bars, banks.width);
        for (k, &at) in self.members.iter().enumerate() {
            let open = bars[k].max((banks.bowl[at] * RAPID_SHARE).min(RAPID_DEPTH));
            let bed = rough[k] + (open.max(rough[k]) - rough[k]) * lane[k];
            depth[at] = bed.min((SHORE_SLOPE * f64::from(banks.shore[at])) as f32);
        }
        line.iter().map(|&k| self.members[k]).collect()
    }

    /// How far inside the chute each member is, 1 in it and 0 clear of it, and the members along
    /// its line. The chute follows the
    /// cheapest way through the water from one end of the course to the other over the bed `bars`
    /// (at each member), where a step costs its length and far more the shallower it is
    /// (`CHUTE_KEEPS_DEEP`): so it keeps to the deep water and wanders from bank to bank with
    /// it, as a river's deepest line does. It is `CHUTE_HALF` to each side, or half the water's
    /// own reach from its banks (`width`, under every sample of the grid) where that is less.
    fn chute(&self, side: usize, cell_m: f64, bars: &[f32], width: &[f32]) -> (Vec<f32>, Vec<usize>) {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let mut lane = vec![0.0f32; self.members.len()];
        let last = self.along.iter().cloned().fold(0.0, f64::max);
        // Which member each sample of the grid is.
        let mut member = vec![NONE; side * side];
        for (k, &at) in self.members.iter().enumerate() {
            member[at] = k as u32;
        }
        let mut cost = vec![u32::MAX; self.members.len()];
        let mut from = vec![NONE; self.members.len()];
        let mut queue = BinaryHeap::new();
        for (k, _) in self.along.iter().enumerate().filter(|&(_, &along)| along < 0.5) {
            cost[k] = 0;
            queue.push(Reverse((0u32, k as u32)));
        }
        let mut end = None;
        while let Some(Reverse((so_far, k))) = queue.pop() {
            let k = k as usize;
            if so_far > cost[k] {
                continue;
            }
            if self.along[k] > last - 0.5 {
                end = Some(k);
                break;
            }
            for (next, step) in neighbours(self.members[k], side) {
                let next = member[next];
                if next == NONE {
                    continue;
                }
                let shallow = 1.0 + (CHUTE_KEEPS_DEEP / (f64::from(bars[next as usize]) + 0.2)).powi(2);
                let reached = so_far.saturating_add((f64::from(step) * 100.0 * shallow) as u32);
                if reached < cost[next as usize] {
                    cost[next as usize] = reached;
                    from[next as usize] = k as u32;
                    queue.push(Reverse((reached, next)));
                }
            }
        }
        let mut line = Vec::new();
        while let Some(k) = end {
            line.push(k);
            let at = self.members[k];
            let half = CHUTE_HALF.min(0.5 * f64::from(width[at])).max(cell_m);
            let reach = (1.5 * half / cell_m).ceil() as i64;
            let (i, j) = ((at % side) as i64, (at / side) as i64);
            for (di, dj) in (-reach..=reach).flat_map(|dj| (-reach..=reach).map(move |di| (di, dj))) {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= side as i64 || nj >= side as i64 {
                    continue;
                }
                let near = member[nj as usize * side + ni as usize];
                if near != NONE {
                    let inside = 1.0 - smoothstep(half, 1.5 * half, (di as f64).hypot(dj as f64) * cell_m);
                    lane[near as usize] = lane[near as usize].max(inside as f32);
                }
            }
            end = (from[k] != NONE).then(|| from[k] as usize);
        }
        (lane, line)
    }

    /// Where the stretch's water comes in and leaves (the two ends of its course, and where the
    /// river runs off the grid), and the surface's fall at each member, as `riverflow::Channel`
    /// wants them.
    fn ends(&self, side: usize) -> (Vec<bool>, Vec<[f64; 2]>) {
        let last = self.along.iter().cloned().fold(0.0, f64::max);
        let open = self
            .members
            .iter()
            .zip(&self.along)
            .map(|(&at, &along)| {
                let (i, j) = (at % side, at / side);
                along < FLOW_END_BINS || along > last - FLOW_END_BINS || i == 0 || j == 0 || i == side - 1 || j == side - 1
            })
            .collect();
        let fall = self.course.iter().zip(&self.slope).map(|(course, slope)| [course[0] * slope, course[1] * slope]).collect();
        (open, fall)
    }

    /// The stretch's current over the bed `depth`, solved (`riverflow::solve`), carrying `carried`
    /// m³/s, with the bed it then runs over, which is `depth` cut deeper where it must be.
    ///
    /// The solve finds how much water the fall drives over this bed, which is not quite what the
    /// bed was cut for: its bars and boulders hold the water back, as a pool's bowl may, and the
    /// chute lets more through. So what passes each sample is scaled to make the river's
    /// `carried`. Where the bed passes too little and was cut shallower than its bowl, it is too
    /// shallow: by Manning's law a bed `a` times deeper runs `a^(2/3)` times faster, so `a^(5/3)`
    /// makes up the shortfall (never more than `DEEPENED`, nor deeper than the bowl), and the
    /// current there is scaled by the rest. A pool keeps its bowl, and a bed that passes too much
    /// its depth: there the current alone is scaled. Scaling the current is the solution for a
    /// fall scaled by the square of the same factor, since the bed's hold and the water's momentum
    /// both go by the speed squared.
    fn current(&self, grid: Grid, depth: &[f32], banks: &Banks, carried: f64) -> Run {
        let side = grid.side as usize;
        let cell_m = grid.cell as f64 / 100.0;
        let (open, fall) = self.ends(side);
        let mut current = riverflow::solve(&riverflow::Channel { side, cell: cell_m, members: &self.members, depth, fall: &fall, open: &open });
        let mut bed: Vec<f32> = self.members.iter().map(|&at| depth[at]).collect();
        let driven = self.discharge(cell_m, depth, &current.flow);
        let mut deeper = 1.0;
        if driven > 0.0 {
            let short = (carried / driven) as f32;
            deeper = short.powf(0.6).clamp(1.0, DEEPENED);
            for ((velocity, bed), &at) in current.flow.iter_mut().zip(&mut bed).zip(&self.members) {
                let was = *bed;
                *bed = (was * deeper).min(was.max(banks.bowl[at])).min((SHORE_SLOPE * f64::from(banks.shore[at])) as f32).max(was);
                let scale = if was > 0.0 { short * was / *bed } else { short };
                let speed = velocity[0].hypot(velocity[1]) * scale;
                let capped = if speed > FLOW_MAX { FLOW_MAX / speed } else { 1.0 };
                *velocity = velocity.map(|v| v * scale * capped);
            }
            // The surface's shape goes by the speed squared, as the fall that drives it does.
            for lift in &mut current.lift {
                *lift *= short * short;
            }
        }
        Run { current, bed, deeper }
    }

    /// How the current `run` shapes the surface at each member, over the bed `depth`: how far it
    /// lifts it above the even one it was solved under, metres (negative where it lowers it), and
    /// how high a standing wave rises there on top of that.
    ///
    /// The two are these. The first is the pressure the solve found under its lid
    /// (`riverflow::Current::lift`): the water piles up before a boulder, backs up above a narrows,
    /// leans to the outside of a bend, and dips where it runs fast. The second is the waves that
    /// stand in a rapid, where the water runs fast and thin: their crests lie across the current
    /// (they are lines of equal distance along the course) and they stay where they are.
    fn shape(&self, grid: Grid, depth: &[f32], run: &Run) -> Vec<[f32; 2]> {
        let side = grid.side as usize;
        let cell_m = grid.cell as f64 / 100.0;
        let last = self.along.iter().cloned().fold(0.0, f64::max);
        let lengths: Vec<f64> = TRAIN_LENGTHS.into_iter().filter(|&length| length >= TRAIN_SAMPLES * cell_m).collect();
        // What the whole of a cross-section is lifted by is left out: how the surface falls along
        // the course is the fitted profile's to say, and the solve's lid only knows it roughly.
        let bins = last as usize + 1;
        let (mut sum, mut count) = (vec![0.0f64; bins], vec![0.0f64; bins]);
        for (&along, &lift) in self.along.iter().zip(&run.current.lift) {
            sum[along as usize] += f64::from(lift);
            count[along as usize] += 1.0;
        }
        let centres: Vec<f64> = (0..bins).filter(|&bin| count[bin] > 0.0).map(|bin| bin as f64 + 0.5).collect();
        let means: Vec<f64> = (0..bins).filter(|&bin| count[bin] > 0.0).map(|bin| sum[bin] / count[bin]).collect();
        self.members
            .iter()
            .enumerate()
            .map(|(k, &at)| {
                let along = self.along[k];
                let inside = smoothstep(FLOW_END_BINS, FLOW_END_BINS + SURFACE_END_BINS, along.min(last - along));
                let lift = f64::from(run.current.lift[k]) - interpolate(&centres, &means, along);
                let lift = self.lift_max * (lift / self.lift_max).tanh();
                let speed = f64::from(run.current.flow[k][0]).hypot(f64::from(run.current.flow[k][1]));
                let deep = f64::from(depth[at]).max(0.05);
                let stands = smoothstep(f64::from(shared::waves::CHURN_SPEEDS[0]), f64::from(shared::waves::CHURN_SPEEDS[1]), speed) * smoothstep(TRAIN_FROUDES[0], TRAIN_FROUDES[1], speed / (riverflow::G * deep).sqrt());
                let mut wave = 0.0;
                if let (true, Some(&longest)) = (stands > 0.0, lengths.last()) {
                    // No two waves of a rapid are alike: they stand high in one patch of it and
                    // barely show in the next, and a crest wanders up and down the river along
                    // its length, so the rapid is not ruled with even bars.
                    let (x, z) = (grid.x((at % side) as u32), grid.x((at / side) as u32));
                    let patch = 0.5 + 0.5 * noise(x / TRAIN_PATCH, z / TRAIN_PATCH, 31);
                    let uneven = TRAIN_UNEVEN[0] + (TRAIN_UNEVEN[1] - TRAIN_UNEVEN[0]) * patch * patch;
                    // A crest is peaked and the trough between two is flat, and at the even
                    // surface, so the waves take no water off a bed.
                    let metres = along * RIVER_BIN + TRAIN_WANDER * noise(x / TRAIN_WANDERS, z / TRAIN_WANDERS, 32);
                    let crest = |length: f64| (0.5 + 0.5 * (std::f64::consts::TAU * metres / length).cos()).powi(2);
                    let fits = (std::f64::consts::TAU * speed * speed / riverflow::G).clamp(lengths[0], longest);
                    let longer = lengths.iter().position(|&length| length >= fits).unwrap_or(lengths.len() - 1);
                    let shorter = longer.saturating_sub(1);
                    let share = if longer == shorter { 1.0 } else { (fits - lengths[shorter]) / (lengths[longer] - lengths[shorter]) };
                    wave = (TRAIN_HEIGHT * stands * uneven).min(TRAIN_DEPTH_SHARE * deep) * (crest(lengths[shorter]) * (1.0 - share) + crest(lengths[longer]) * share);
                }
                [(inside * lift) as f32, (inside * wave) as f32]
            })
            .collect()
    }

    /// The foam the current `flow` (at each member) makes and carries over the bed `depth`, and
    /// how it stirs the surface (`riverflow::marks`).
    fn marks(&self, grid: Grid, depth: &[f32], flow: &[[f32; 2]]) -> riverflow::Marks {
        let side = grid.side as usize;
        let (open, fall) = self.ends(side);
        riverflow::marks(&riverflow::Channel { side, cell: grid.cell as f64 / 100.0, members: &self.members, depth, fall: &fall, open: &open }, flow)
    }

    /// The water the stretch carries, m³/s, given the current at each member: the median, over
    /// the `RIVER_BIN`s along its course, of what passes down each.
    fn discharge(&self, cell_m: f64, depth: &[f32], flow: &[[f32; 2]]) -> f64 {
        let bins = self.along.iter().map(|&a| a as usize).max().unwrap_or(0) + 1;
        let mut through = vec![0.0f64; bins];
        for (k, &at) in self.members.iter().enumerate() {
            let down = f64::from(flow[k][0]) * self.course[k][0] + f64::from(flow[k][1]) * self.course[k][1];
            through[self.along[k] as usize] += down * f64::from(depth[at]) * cell_m * cell_m / RIVER_BIN;
        }
        through.sort_by(f64::total_cmp);
        through[bins / 2]
    }
}

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
/// It returns the stretches that flow, each with its course and how steeply it falls along it: downstream
/// as its centre lines are drawn (or downhill where it has none), with the fall measured over
/// `FLOW_SLOPE_BINS` either way. `Surface::new` cuts their beds and solves their currents from these.
fn river_levels(map: &Map, grid: Grid, body: &[u32], model: &[f32], along_channels: &BTreeMap<usize, channel::At>, level: &mut [f32]) -> Vec<Stretch> {
    let mut stretches = Vec::new();
    // A long river is drawn as several polygons end to end. They are one water: every river
    // sample counts as the same body here, so a stretch runs on through the seams between them.
    let mut river: Vec<u32> = body.iter().map(|&b| if b != NONE && map.bodies[b as usize].kind == Kind::River { RIVER } else { NONE }).collect();
    // A channel and its pools are a water of their own (`channel_levels`).
    for &at in along_channels.keys() {
        river[at] = NONE;
    }
    let body = &river[..];
    let side = grid.side as usize;
    let cell_m = grid.cell as f64 / 100.0;
    let n = body.len();
    let mut seen = vec![false; n];
    let mut dist = vec![u32::MAX; n];
    let mut stack = Vec::new();
    for start in 0..n {
        if seen[start] || body[start] == NONE {
            continue;
        }
        let mut members = Vec::new();
        seen[start] = true;
        stack.push(start);
        while let Some(at) = stack.pop() {
            members.push(at);
            for (next, _) in neighbours(at, side) {
                if !seen[next] && body[next] != NONE {
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
        let drawn = downstream(map, grid, body, &dist);
        let sense = match drawn {
            0 => if error(&falling) <= error(&rising) { 1.0 } else { -1.0 },
            vote => vote.signum() as f64,
        };
        let profile = if sense > 0.0 { falling } else { rising };
        for &at in &members {
            level[at] = interpolate(&centres, &profile, along(at)) as f32;
        }
        let drop = (profile[0] - profile[profile.len() - 1]).abs();
        let length = bins as f64 * RIVER_BIN;
        if drawn != 0 || drop >= FLOW_MIN_DROP {
            // The chamfer's distance leans towards the eight grid directions; the current's direction
            // comes from a true one, marched out from the same end.
            let marched = march(&cross_section(end, body, side), side, |next| body[next] != NONE);
            let (mut slope, mut course) = (Vec::with_capacity(members.len()), Vec::with_capacity(members.len()));
            for &at in &members {
                let a = along(at);
                let fall = (interpolate(&centres, &profile, a - FLOW_SLOPE_BINS) - interpolate(&centres, &profile, a + FLOW_SLOPE_BINS)) * sense;
                slope.push((fall / (2.0 * FLOW_SLOPE_BINS * RIVER_BIN)).max(FLOW_MIN_SLOPE));
                let [gx, gz] = gradient(at, side, &marched, |next| body[next] != NONE);
                let size = gx.hypot(gz);
                course.push(if size > 0.0 { [gx / size * sense, gz / size * sense] } else { [0.0; 2] });
            }
            let along = members.iter().map(|&at| along(at)).collect();
            stretches.push(Stretch { members: members.clone(), along, slope, course, length, drop, lift_max: SURFACE_LIFT_MAX });
        } else if length >= RIVER_REPORTED {
            eprintln!("water: a river falls {drop:.1} m over {length:.0} m, and nothing tells which way it runs: it stands still");
        }
        for &at in &members {
            dist[at] = u32::MAX;
        }
    }
    stretches
}

/// What `channel_levels` found.
#[derive(Default)]
struct Narrows {
    /// The channels long enough to solve, each a stretch of its own.
    stretches: Vec<Stretch>,
    /// The samples of those too short to, each with its direction downstream.
    short: Vec<(usize, [f64; 2])>,
    mouths: Vec<Mouth>,
}

/// Where a channel meets other water.
struct Mouth {
    /// A sample of that water, and its level when the channel was held to it, metres.
    beside: usize,
    level: f32,
    /// Whether the channel runs out into that water rather than starting from it.
    out: bool,
    /// The channel's samples, each with how much of what that water moves it follows: all of it at
    /// this end, none from `channel::MOUTH` in.
    members: Vec<(usize, f32)>,
}

/// Water in a channel's mouth is never drawn down shallower than this, metres.
const MOUTH_SHALLOWEST: f32 = 0.05;

/// Gives each channel, with the pools along it (`along`: which channel each of their samples is and
/// how far along it, metres), its surface: its valley's profile held to the water at its two ends
/// (`Channel::held`), the longest channel first so a tributary is held to the river it joins. A pool's
/// far arm starts from the level where it leaves the channel (`arm_levels`). Each channel is
/// a stretch of one course, measured along its line rather than by the distance through all the
/// water it touches, which would give it and the river it runs into one profile and one discharge.
fn channel_levels(map: &Map, grid: Grid, along: &BTreeMap<usize, channel::At>, level: &mut [f32]) -> Narrows {
    let mut found = Narrows::default();
    let mut members: Vec<Vec<(usize, f64)>> = vec![Vec::new(); map.channels.len()];
    let mut arms: Vec<Vec<(usize, f64)>> = vec![Vec::new(); map.channels.len()];
    for (&at, place) in along {
        (if place.out > ARM_OUT { &mut arms } else { &mut members })[place.channel].push((at, place.along));
    }
    for (index, (line, members)) in map.channels.iter().zip(members).enumerate() {
        // The water at an end: wide water, or a channel already levelled.
        let meets = |p: P, level: &[f32]| {
            grid.index(p[0], p[1]).filter(|&at| !level[at].is_nan() && along.get(&at).is_none_or(|place| place.channel < index)).map(|at| (at, level[at]))
        };
        let last = line.points.len() - 1;
        let (from, to) = (meets(line.points[0], level), meets(line.points[last], level));
        let profile = line.held(from.map(|(_, height)| f64::from(height)), to.map(|(_, height)| f64::from(height)));
        for &(at, metres) in members.iter().chain(&arms[index]) {
            level[at] = interpolate(&line.arc, &profile, metres) as f32;
        }
        let length = line.length();
        let over = channel::MOUTH.min(length / 2.0).max(1e-6);
        for (end, from_end) in [(from, false), (to, true)] {
            let Some((beside, height)) = end else { continue };
            let members = members
                .iter()
                .map(|&(at, metres)| {
                    let share = 1.0 - if from_end { length - metres } else { metres } / over;
                    (at, share.max(0.0) as f32)
                })
                .collect();
            found.mouths.push(Mouth { beside, level: height, out: from_end, members });
        }
        let (low, high) = members.iter().fold((f64::MAX, f64::MIN), |(low, high), &(_, metres)| (low.min(metres), high.max(metres)));
        if high - low < CHANNEL_SOLVED {
            found.short.extend(members.iter().map(|&(at, metres)| (at, line.course(metres))));
            continue;
        }
        let reach = FLOW_SLOPE_BINS * RIVER_BIN;
        found.stretches.push(Stretch {
            along: members.iter().map(|&(_, metres)| (metres - low) / RIVER_BIN).collect(),
            slope: members
                .iter()
                .map(|&(_, metres)| {
                    let fall = interpolate(&line.arc, &profile, metres - reach) - interpolate(&line.arc, &profile, metres + reach);
                    (fall / (2.0 * reach)).max(FLOW_MIN_SLOPE)
                })
                .collect(),
            course: members.iter().map(|&(_, metres)| line.course(metres)).collect(),
            members: members.iter().map(|&(at, _)| at).collect(),
            length: high - low,
            drop: profile[0] - profile[last],
            lift_max: CHANNEL_LIFT_MAX,
        });
    }
    found
}

/// Gives each arm of a pool that reaches away from its channel's line (more than `ARM_OUT`, as a mill's
/// race does up to its dam) a fall of its own, where it would lie level with the place it leaves the
/// river, cut metres into the valley's side. The height model's median each `RIVER_BIN` out along
/// the arm is fitted with a profile that only rises or only falls, eased from the level at the
/// channel, which each of its samples holds on the way in. A long arm is a stretch that flows.
fn arm_levels(grid: Grid, along: &BTreeMap<usize, channel::At>, model: &[f32], level: &mut [f32]) -> Vec<Stretch> {
    let side = grid.side as usize;
    let out = |at: usize| along.get(&at).map(|place| place.out);
    let far = |at: usize| out(at).filter(|&out| out > ARM_OUT);
    let mut seen = std::collections::BTreeSet::new();
    let mut found = Vec::new();
    for &start in along.keys() {
        if far(start).is_none() || !seen.insert(start) {
            continue;
        }
        let mut members = vec![(start, along[&start].out - ARM_OUT)];
        let mut next = 0;
        while next < members.len() {
            for (beside, _) in neighbours(members[next].0, side) {
                if let Some(out) = far(beside)
                    && seen.insert(beside)
                {
                    members.push((beside, out - ARM_OUT));
                }
            }
            next += 1;
        }
        let length = members.iter().map(|&(_, out)| out).fold(0.0, f64::max);
        if length < CHANNEL_SOLVED {
            continue;
        }
        let mut bins: Vec<Vec<f64>> = vec![Vec::new(); (length / RIVER_BIN) as usize + 1];
        for &(at, out) in members.iter().filter(|&&(at, _)| model[at].is_finite()) {
            bins[(out / RIVER_BIN) as usize].push(f64::from(model[at]));
        }
        let (mut centres, mut medians, mut weights) = (Vec::new(), Vec::new(), Vec::new());
        for (k, bin) in bins.iter_mut().enumerate().filter(|(_, bin)| !bin.is_empty()) {
            bin.sort_by(f64::total_cmp);
            centres.push((k as f64 + 0.5) * RIVER_BIN);
            medians.push(bin[bin.len() / 2]);
            weights.push(bin.len() as f64);
        }
        if centres.len() < 3 {
            continue;
        }
        // Out along the arm is upstream where the ground rises that way.
        let upstream = medians[medians.len() - 1] >= medians[0];
        let sign = if upstream { 1.0 } else { -1.0 };
        let profile: Vec<f64> = isotonic(&medians.iter().map(|median| median * sign).collect::<Vec<_>>(), &weights).iter().map(|fitted| fitted * sign).collect();
        for &(at, out) in &members {
            let root = f64::from(level[at]);
            let fitted = (interpolate(&centres, &profile, out) * sign).max(root * sign) * sign;
            level[at] = (root + (fitted - root) * (out / channel::MOUTH).min(1.0)) as f32;
        }
        let reach = FLOW_SLOPE_BINS * RIVER_BIN;
        found.push(Stretch {
            along: members.iter().map(|&(_, out)| if upstream { length - out } else { out } / RIVER_BIN).collect(),
            slope: members.iter().map(|&(_, out)| (((interpolate(&centres, &profile, out + reach) - interpolate(&centres, &profile, out - reach)) * sign) / (2.0 * reach)).max(FLOW_MIN_SLOPE)).collect(),
            course: members
                .iter()
                .map(|&(at, here)| {
                    let mut way = [0.0; 2];
                    for (beside, _) in neighbours(at, side) {
                        if let Some(there) = out(beside) {
                            let nearer = (here + ARM_OUT - there) * sign;
                            way[0] += ((beside % side) as f64 - (at % side) as f64) * nearer;
                            way[1] += ((beside / side) as f64 - (at / side) as f64) * nearer;
                        }
                    }
                    let length = way[0].hypot(way[1]);
                    if length > 0.0 { way.map(|part| part / length) } else { way }
                })
                .collect(),
            members: members.iter().map(|&(at, _)| at).collect(),
            length,
            drop: (profile[0] - profile[profile.len() - 1]).abs(),
            lift_max: CHANNEL_LIFT_MAX,
        });
    }
    found
}

/// How a river stretch's centre lines (`Map::river_lines` and `Map::streams`, drawn downstream) run against `dist`, the
/// distance along the stretch (`u32::MAX` on the rest of `body`, the river samples): positive where they run the way it grows, negative the other way, 0
/// where none crosses it.
fn downstream(map: &Map, grid: Grid, body: &[u32], dist: &[u32]) -> i64 {
    let side = grid.side as usize;
    let index = |p: P| {
        let at = |v: f64| ((v * 100.0 - grid.origin as f64) / grid.cell as f64).round();
        let (i, j) = (at(p[0]), at(p[1]));
        let last = grid.side as f64 - 1.0;
        (i >= 0.0 && j >= 0.0 && i <= last && j <= last).then(|| j as usize * side + i as usize).filter(|&at| body[at] != NONE && dist[at] != u32::MAX)
    };
    let mut vote = 0i64;
    for line in map.river_lines.iter().chain(&map.streams) {
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
        Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![ring], level: None }], ..Map::default() }
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
        pond_in(|_, _| 5.0)
    }

    /// The same pond in ground where the height model is `model(x, z)`, metres; 5 m inside it.
    fn pond_in(model: impl Fn(f64, f64) -> f64) -> (Surface, Ground, Vec<i32>) {
        let ring: Vec<P> = (0..48).map(|k| {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            [0.3 + 9.3 * a.cos(), -0.2 + 9.3 * a.sin()]
        }).collect();
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![ring], level: None }], ..Map::default() };
        let grid = Grid { origin: -1_500, cell: 100, side: 31 };
        let surface = Surface::new(&map, grid, |x, z| Some(model(x, z)));
        let heights = (0..31 * 31)
            .map(|at| {
                let (x, z) = (grid.x(at % 31), grid.x(at / 31));
                (surface.bed(x, z, Some(model(x, z))).unwrap() * 100.0).round() as i32
            })
            .collect();
        let ground = Ground { origin: -1_500, cell: 100, side: 31, heights };
        let levels = surface.levels(0.0, &ground);
        (surface, ground, levels)
    }

    /// How far from the pond's outline the drawn shoreline falls, metres, at worst, and how many
    /// times it crosses an edge between samples: where the water's depth crosses zero (where the
    /// client fades the water out), and where the ground rises through the water's level (where it
    /// hides the water) if that is inside the outline. Ground that rises past the outline hides
    /// nothing the fade has not already taken.
    fn shoreline_off_the_pond(ground: &Ground, levels: &[i32]) -> (f64, usize) {
        let depth = |at: usize| if levels[at] == NO_WATER { -1.0 } else { f64::from(levels[at] - ground.heights[at]) / 100.0 };
        let above = |at: usize| f64::from(ground.heights[at]) / 100.0 - 5.0;
        let x = |i: usize| -15.0 + i as f64;
        let (mut worst, mut crossings) = (0.0f64, 0);
        for j in 0..31 {
            for i in 0..31 {
                for (di, dj) in [(1, 0), (0, 1)] {
                    if i + di > 30 || j + dj > 30 {
                        continue;
                    }
                    let (a, b) = (j * 31 + i, (j + dj) * 31 + i + di);
                    for (hiding, da, db) in [(false, depth(a), depth(b)), (true, -above(a), -above(b))] {
                        if (da > 0.0) == (db > 0.0) {
                            continue;
                        }
                        let t = da / (da - db);
                        let p = [x(i) + t * di as f64, x(j) + t * dj as f64];
                        let off = (p[0] - 0.3).hypot(p[1] + 0.2) - 9.3;
                        worst = worst.max(if hiding { -off } else { off.abs() });
                        crossings += 1;
                    }
                }
            }
        }
        (worst, crossings)
    }

    #[test]
    fn a_ponds_shoreline_follows_its_outline_not_the_samples() {
        let (_, ground, levels) = pond();
        let (off, crossings) = shoreline_off_the_pond(&ground, &levels);
        assert!(off < 0.2, "the shoreline is {off:.2} m off the outline");
        assert!(crossings > 120, "{crossings}");
    }

    #[test]
    fn a_high_banked_ponds_shoreline_follows_its_outline() {
        let r = |x: f64, z: f64| (x - 0.3).hypot(z + 0.2);
        // A bank that stands 3 m over the water at the outline, as a river's cut bank does...
        let (_, ground, levels) = pond_in(|x, z| if r(x, z) < 9.3 { 5.0 } else { 8.0 });
        let (off, crossings) = shoreline_off_the_pond(&ground, &levels);
        assert!(off < 0.2 && crossings > 120, "the shoreline is {off:.2} m off the cliff's outline ({crossings} crossings)");
        // Further out the bank is as the height model has it.
        assert_eq!(ground.heights[15 * 31 + 30], 800, "the cliff's top 14.7 m out");
        // ...and one that climbs from the waterline at 2 m a metre.
        let (_, ground, levels) = pond_in(|x, z| 5.0 + 2.0 * (r(x, z) - 9.3).max(0.0));
        let (off, crossings) = shoreline_off_the_pond(&ground, &levels);
        assert!(off < 0.2 && crossings > 120, "the shoreline is {off:.2} m off the slope's outline ({crossings} crossings)");
    }

    #[test]
    fn a_bank_is_cut_down_to_the_water_and_eased_back_into_the_ground() {
        let heights: Vec<f64> = (0..=80).map(|k| bank_height(9.0, 5.0, k as f64 * 0.1, 1.0)).collect();
        assert!((heights[0] - 5.0).abs() < 1e-9, "it meets the water on the outline");
        assert!((heights[10] - 5.0 - SHORE_SLOPE).abs() < 1e-9, "one sample out it rises as the bed falls");
        assert!(heights.windows(2).all(|w| w[1] >= w[0]), "always rising");
        assert!(heights.windows(2).all(|w| w[1] - w[0] <= 0.1 * BANK_STEEP + 0.15), "no step: {heights:?}");
        assert_eq!(*heights.last().unwrap(), 9.0, "back to the height model past the band");
        assert_eq!(bank_height(6.7, 5.0, 2.0, 1.0), 6.7, "a bank under the cut is left as it is");
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
        assert_eq!(surface.bed(15.0, 0.0, Some(5.0)), Some(5.0), "further out the ground is untouched");
    }

    #[test]
    fn where_a_lake_meets_a_river_the_water_is_not_shallow() {
        let lake = vec![[-40.0, -40.0], [0.0, -40.0], [0.0, 40.0], [-40.0, 40.0]];
        let river = vec![[0.0, -40.0], [40.0, -40.0], [40.0, 40.0], [0.0, 40.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![lake], level: None }, Body { kind: Kind::River, rings: vec![river], level: None }], ..Map::default() };
        let surface = Surface::new(&map, grid(), |_, _| Some(0.0));
        assert!(-surface.bed(0.0, 0.0, Some(0.0)).unwrap() > 2.0, "the line between them is open water");
    }

    #[test]
    fn a_river_is_as_deep_as_it_is_wide_where_you_are() {
        // One polygon: 20 m wide for the first 300 m, then a 300 m wide basin.
        let ring = vec![[-300.0, -10.0], [0.0, -10.0], [0.0, -150.0], [300.0, -150.0], [300.0, 150.0], [0.0, 150.0], [0.0, 10.0], [-300.0, 10.0]];
        let map = Map { bodies: vec![Body { kind: Kind::River, rings: vec![ring], level: None }], ..Map::default() };
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
        Map { bodies: vec![Body { kind: Kind::River, rings: vec![ring], level: None }], ..Map::default() }
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

    /// How deep the water is at a sample.
    fn deep(surface: &Surface, x: f64, z: f64) -> f32 {
        surface.depth[surface.index(x, z).unwrap()]
    }

    /// The water passing the cross-section at `x` of a river that runs along x, m³/s.
    fn passing(surface: &Surface, x: f64) -> f64 {
        let cell = surface.grid.cell as f64 / 100.0;
        (-160..=160).map(|k| k as f64 * cell).map(|z| f64::from(current(surface, x, z)[0]) * f64::from(deep(surface, x, z)) * cell).sum()
    }

    #[test]
    fn a_falling_river_flows_downhill_and_fastest_at_rapids() {
        // 20 m wide, the ground falling 1 m per km in x, with a 3 m rapid at x = 0.
        let map = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - x / 1000.0 - if x < 0.0 { 0.0 } else { 3.0 } + 0.02 * noise(x, z)));
        let calm = current(&surface, -200.0, 0.0);
        assert!(calm[0] > 0.5 && calm[0] < 1.2 && calm[1].abs() < 0.05, "a gentle stretch drifts downstream: {calm:?}");
        let rapid = current(&surface, 0.0, 0.0);
        assert!(rapid[0] > 2.0 * calm[0] && rapid[0] <= FLOW_MAX, "the rapid runs: {rapid:?}");
        let bank = current(&surface, -200.0, 8.0);
        assert!(bank[0] > 0.0 && bank[0] < 0.7 * calm[0], "slower by the bank: {bank:?} against {calm:?}");
        let flow = surface.flows();
        let at = surface.index(0.0, 0.0).unwrap();
        assert_eq!(flow.len(), 2 * surface.level.len());
        assert_eq!(flow[2 * at], (rapid[0] / FLOW_STEP).round() as i8, "written in FLOW_STEPs");
    }

    #[test]
    fn a_rapid_is_shallow_and_fast_and_a_pool_is_deep_and_slow() {
        // 40 m wide and nearly level, with a 3 m rapid at x = 0.
        let map = river(vec![[-400.0, -20.0], [400.0, -20.0], [400.0, 20.0], [-400.0, 20.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - if x < 0.0 { 0.0 } else { 3.0 } + 0.02 * noise(x, z)));
        let (pool, rapid) = (deep(&surface, -250.0, 0.0), deep(&surface, 0.0, 0.0));
        assert!(f64::from(pool) > 0.85 * Kind::River.max_depth(20.0), "the pool all but keeps its bowl: {pool}");
        assert!(rapid < 0.4 * pool && rapid >= RAPID_SHARE * pool - 0.01, "the rapid is cut shallow, but not dry: {rapid} against {pool}");
        let (slow, fast) = (current(&surface, -250.0, 0.0)[0], current(&surface, 0.0, 0.0)[0]);
        assert!(fast > 2.5 * slow, "{fast} m/s over the rapid, {slow} m/s in the pool");
        // The rapid makes foam, which the water carries on below it and never up.
        let foam = surface.foams();
        let cover = |x: f64| foam[surface.index(x, 0.0).unwrap()];
        assert!(cover(-100.0) == 0 && cover(60.0) > 50 && cover(300.0) < cover(60.0) / 2, "{} above it, {} just below, {} far below", cover(-100.0), cover(60.0), cover(300.0));
        // The same water passes both.
        let through: Vec<f64> = [-250.0, -100.0, 0.0, 100.0, 250.0].map(|x| passing(&surface, x)).to_vec();
        let mean = through.iter().sum::<f64>() / through.len() as f64;
        assert!(through.iter().all(|q| (q - mean).abs() < 0.05 * mean), "{through:?}");
        // Through the pool's cross-section it runs at about `USUAL_SPEED` (a little over: the
        // discharge is set over the bowl, and the pool is trimmed a little under it).
        let section: f64 = (-10..=10).map(|k| f64::from(deep(&surface, -250.0, k as f64 * 2.0)) * 2.0).sum();
        assert!((mean / section - USUAL_SPEED).abs() < 0.2 * USUAL_SPEED, "{mean} m³/s through {section} m²");
    }

    /// A river 160 m wide and nearly level, with a 3 m rapid at x = 0.
    fn wide_rapid() -> Surface {
        let map = river(vec![[-400.0, -80.0], [400.0, -80.0], [400.0, 80.0], [-400.0, 80.0]]);
        Surface::new(&map, river_grid(), |x, z| Some(10.0 - if x < 0.0 { 0.0 } else { 3.0 } + 0.02 * noise(x, z)))
    }

    #[test]
    fn a_rapids_bed_has_bars_and_boulders_and_a_pools_is_smooth() {
        let surface = wide_rapid();
        // Along the middle of the river, where its bowl is as deep all the way.
        let middle = |from: i32, to: i32| (from..=to).map(|k| deep(&surface, k as f64 * 2.0, 0.0)).collect::<Vec<f32>>();
        let spread = |depths: &[f32]| depths.iter().cloned().fold(0.0, f32::max) / depths.iter().cloned().fold(f32::MAX, f32::min);
        let pool = middle(-150, -100);
        assert!(spread(&pool) < 1.35, "the pool's bed is all but smooth: {pool:?}");
        // Across the rapid, clear of the banks.
        let rapid: Vec<f32> = (-20..=20).flat_map(|i| (-25..=25).map(move |j| (i, j))).map(|(i, j)| deep(&surface, i as f64 * 2.0, j as f64 * 2.0)).collect();
        let (shallowest, deepest) = (rapid.iter().cloned().fold(f32::MAX, f32::min), rapid.iter().cloned().fold(0.0, f32::max));
        assert!(deepest > 2.5 * shallowest.max(0.5), "the rapid's bed has bars and pools: {shallowest} to {deepest}");
        assert!(shallowest > 0.1 && shallowest < 0.6, "and boulders that all but break the surface: {shallowest}");
    }

    #[test]
    fn a_deep_chute_runs_unbroken_down_a_big_rivers_rapid() {
        let surface = wide_rapid();
        let side = surface.grid.side as usize;
        // Whatever a boat drawing `RAPID_DEPTH` can reach from the river's upper end.
        let floats = |at: usize| surface.depth[at] >= RAPID_DEPTH - 0.01;
        let mut reached = vec![false; side * side];
        let mut stack: Vec<usize> = (0..side).map(|j| j * side + 10).filter(|&at| floats(at)).collect();
        assert!(!stack.is_empty());
        while let Some(at) = stack.pop() {
            if std::mem::replace(&mut reached[at], true) {
                continue;
            }
            stack.extend([at - 1, at + 1, at - side, at + side].into_iter().filter(|&next| !reached[next] && floats(next)));
        }
        assert!((0..side).any(|j| reached[j * side + side - 11]), "it gets down the rapid to the lower end");
        // The rapid is not that deep all over: the chute is a lane through it.
        let across: Vec<f32> = (-35..=35).map(|j| deep(&surface, 0.0, j as f64 * 2.0)).collect();
        let open = across.iter().filter(|&&depth| depth >= RAPID_DEPTH - 0.01).count();
        assert!(open >= 3 && open < across.len() / 2, "{open} of {} samples across the rapid float a boat: {across:?}", across.len());
    }

    #[test]
    fn a_rough_bed_still_carries_the_rivers_discharge() {
        let surface = wide_rapid();
        let through: Vec<f64> = [-250.0, -100.0, -20.0, 0.0, 20.0, 100.0, 250.0].map(|x| passing(&surface, x)).to_vec();
        let mean = through.iter().sum::<f64>() / through.len() as f64;
        assert!(mean > 100.0 && through.iter().all(|q| (q - mean).abs() < 0.06 * mean), "{through:?}");
    }

    #[test]
    fn a_river_runs_faster_through_a_narrows() {
        // 60 m wide, pinched to 20 m for 60 m at x = 0; nearly level, drawn running east.
        let ring = vec![[-400.0, -30.0], [-30.0, -30.0], [-30.0, -10.0], [30.0, -10.0], [30.0, -30.0], [400.0, -30.0], [400.0, 30.0], [30.0, 30.0], [30.0, 10.0], [-30.0, 10.0], [-30.0, 30.0], [-400.0, 30.0]];
        let map = Map { streams: vec![vec![[-300.0, 0.0], [300.0, 0.0]]], ..river(ring) };
        let surface = Surface::new(&map, river_grid(), |_, _| Some(5.0));
        let (wide, narrow) = (current(&surface, -200.0, 0.0)[0], current(&surface, 0.0, 0.0)[0]);
        assert!(wide > 0.2 && narrow > 2.0 * wide, "{narrow} m/s in the narrows, {wide} m/s above it");
        let (above, within) = (passing(&surface, -200.0), passing(&surface, 0.0));
        assert!((above - within).abs() < 0.05 * above, "{above} m³/s above it, {within} m³/s through it");
    }

    #[test]
    fn the_current_parts_round_an_island_and_dies_at_the_bank() {
        // 80 m wide with an island 30 m square in the middle of it, drawn running east.
        let outer = vec![[-400.0, -40.0], [400.0, -40.0], [400.0, 40.0], [-400.0, 40.0]];
        let island = vec![[-15.0, -15.0], [15.0, -15.0], [15.0, 15.0], [-15.0, 15.0]];
        let mut map = Map { streams: vec![vec![[-300.0, 28.0], [300.0, 28.0]]], ..river(outer) };
        map.bodies[0].rings.push(island);
        let surface = Surface::new(&map, river_grid(), |_, _| Some(5.0));
        let above = current(&surface, -200.0, 0.0);
        let (north, south) = (current(&surface, -24.0, -14.0), current(&surface, -24.0, 14.0));
        assert!(north[1] < -0.03 && south[1] > 0.03, "it parts before the island: {north:?} {south:?}");
        let beside = current(&surface, 0.0, 28.0);
        assert!(beside[0] > 1.2 * above[0], "and runs faster past it: {beside:?} against {above:?}");
        let lee = current(&surface, 22.0, 0.0);
        assert!(lee[0] < 0.3 * above[0], "the water behind it is all but still: {lee:?}");
        let bank = current(&surface, -200.0, 38.0);
        assert!(bank[0] > 0.0 && bank[0] < 0.5 * above[0], "as it is by the bank: {bank:?}");
    }

    #[test]
    fn a_river_drawn_as_two_polygons_is_one_water() {
        // The same 20 m river, falling 1 m per 100 m, as one polygon and cut in two at x = 0.
        let whole = river(vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]]);
        let mut halves = river(vec![[-400.0, -10.0], [0.0, -10.0], [0.0, 10.0], [-400.0, 10.0]]);
        halves.bodies.push(Body { kind: Kind::River, rings: vec![vec![[0.0, -10.0], [400.0, -10.0], [400.0, 10.0], [0.0, 10.0]]], level: None });
        let dem = |x: f64, z: f64| Some(10.0 - x / 100.0 + noise(x, z));
        let (one, two) = (Surface::new(&whole, river_grid(), dem), Surface::new(&halves, river_grid(), dem));
        for x in [-200.0, -2.0, 0.0, 2.0, 200.0] {
            assert!((level(&one, x, 0.0) - level(&two, x, 0.0)).abs() < 0.02, "the same level at {x}");
            let (a, b) = (current(&one, x, 0.0), current(&two, x, 0.0));
            assert!(a[0] > 0.3 && (a[0] - b[0]).abs() < 0.05 * a[0], "the same current at {x}: {a:?} {b:?}");
        }
    }

    #[test]
    fn importing_a_river_twice_gives_the_same_current() {
        let outer = vec![[-400.0, -40.0], [400.0, -40.0], [400.0, 40.0], [-400.0, 40.0]];
        let island = vec![[-15.0, -15.0], [15.0, -15.0], [15.0, 15.0], [-15.0, 15.0]];
        let mut map = river(outer);
        map.bodies[0].rings.push(island);
        let dem = |x: f64, z: f64| Some(10.0 - x / 200.0 + noise(x, z));
        let twice = [0, 1].map(|_| Surface::new(&map, river_grid(), dem));
        let flows = twice.each_ref().map(Surface::flows);
        assert!(!flows[0].is_empty() && flows[0] == flows[1]);
        assert_eq!(twice[0].foams(), twice[1].foams());
    }

    #[test]
    fn a_level_river_runs_the_way_its_centre_line_is_drawn() {
        let ring = vec![[-400.0, -10.0], [400.0, -10.0], [400.0, 10.0], [-400.0, 10.0]];
        let level = |_: f64, _: f64| Some(5.0);
        assert!(Surface::new(&river(ring.clone()), river_grid(), level).flows().is_empty(), "nothing tells which way it runs");
        let map = Map { streams: vec![vec![[300.0, 0.0], [-300.0, 0.0]]], ..river(ring) };
        let surface = Surface::new(&map, river_grid(), level);
        let middle = current(&surface, 0.0, 0.0);
        assert!(middle[0] < -USUAL_SPEED as f32 && middle[0] > -2.0 * USUAL_SPEED as f32 && middle[1].abs() < 0.02, "drifts west, as drawn: {middle:?}");
        assert!(deep(&surface, 0.0, 0.0) > 2.5, "and keeps its bowl: {}", deep(&surface, 0.0, 0.0));
        assert!(surface.foams().is_empty(), "with no rapid, it has no foam");
        assert!(Surface::new(&lake(), grid(), |_, _| Some(1.0)).flows().is_empty(), "a lake is still");
    }

    #[test]
    fn a_river_line_and_the_pools_painted_over_it_are_one_flowing_water() {
        let pool = vec![[-20.0, -11.0], [20.0, -11.0], [20.0, 11.0], [-20.0, 11.0]];
        let mut map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![pool], level: None }], river_lines: vec![vec![[-300.0, 0.0], [300.0, 0.0]]], ..Map::default() };
        let ground = |x: f64, z: f64| Some(10.0 - x / 200.0 + 0.1 * z.abs().min(20.0));
        channel::lay(&mut map, &Default::default(), |_| true, ground);
        let surface = Surface::new(&map, river_grid(), ground);
        let profile: Vec<f64> = (-28..=28).map(|k| level(&surface, f64::from(k) * 10.0, 0.0)).collect();
        assert!(profile.iter().all(|level| level.is_finite()), "no dry gap: {profile:?}");
        assert!(profile.windows(2).all(|pair| pair[1] < pair[0] + 0.1), "no step at the pool: {profile:?}");
        for x in [-200.0, 0.0, 200.0] {
            assert!(current(&surface, x, 0.0)[0] > 0.05, "runs east at {x}: {:?}", current(&surface, x, 0.0));
        }
    }

    #[test]
    fn a_pools_arm_away_from_the_river_climbs_the_valleys_side() {
        // A pool on the river with an arm running 220 m north, the ground rising 8 m along it.
        let pool = vec![[-8.0, -240.0], [8.0, -240.0], [8.0, 10.0], [-8.0, 10.0]];
        let mut map = Map { bodies: vec![Body { kind: Kind::River, rings: vec![pool], level: None }], river_lines: vec![vec![[-300.0, 0.0], [300.0, 0.0]]], ..Map::default() };
        let ground = |x: f64, z: f64| Some(10.0 - x / 500.0 + (-z - 20.0).max(0.0) / 25.0);
        channel::lay(&mut map, &Default::default(), |_| true, ground);
        let surface = Surface::new(&map, river_grid(), ground);
        let (top, middle, foot) = (level(&surface, 0.0, -230.0), level(&surface, 0.0, -130.0), level(&surface, 0.0, -2.0));
        assert!(top > 16.0 && top > middle + 2.0 && middle > foot + 1.0, "{top}, {middle}, {foot}");
    }

    #[test]
    fn a_soldier_can_climb_out_of_a_carved_river() {
        // A line down a ravine whose sides rise at 60 degrees for 3 m.
        let mut map = Map { river_lines: vec![vec![[-150.0, 0.0], [150.0, 0.0]]], ..Map::default() };
        let model = |_: f64, z: f64| Some(10.0 + ((z.abs() - 3.0).max(0.0) * 1.7).min(3.0));
        channel::lay(&mut map, &Default::default(), |_| true, model);
        let grid = Grid { origin: -16_000, cell: 100, side: 321 };
        let surface = Surface::new(&map, grid, model);
        let ground = |x: f64, z: f64| surface.bed(x, z, model(x, z)).unwrap();
        let most = f64::from(shared::consts::MAX_SLOPE_CLIMB_DEG).to_radians().tan();
        for (x, z) in (-60..=60).step_by(4).flat_map(|x| (2..12).map(move |z| (f64::from(x), f64::from(z)))) {
            if !surface.is_wet(x, z) {
                assert!(ground(x, z + 1.0) - ground(x, z) < most, "too steep to climb at {x}, {z}");
            }
        }
    }

    #[test]
    fn a_tributary_keeps_its_own_fall_and_meets_the_big_river_without_a_step() {
        // A level river 100 m wide, and a small one that falls 6 m down to it from the north.
        let big = vec![[-400.0, 60.0], [400.0, 60.0], [400.0, 160.0], [-400.0, 160.0]];
        let alone = river(big.clone());
        let mut map = Map { river_lines: vec![vec![[0.0, -310.0], [0.0, 110.0]]], ..river(big) };
        let ground = |x: f64, z: f64| Some(if z >= 60.0 { 4.0 } else { 4.0 + (60.0 - z) / 60.0 + 0.1 * x.abs().min(20.0) });
        channel::lay(&mut map, &Default::default(), |_| true, ground);
        let (surface, before) = (Surface::new(&map, river_grid(), ground), Surface::new(&alone, river_grid(), ground));
        assert!(level(&surface, 0.0, -280.0) > 8.5, "its own fall: {}", level(&surface, 0.0, -280.0));
        assert!((level(&surface, 0.0, 56.0) - level(&surface, 0.0, 64.0)).abs() < 0.08, "a step at the mouth");
        assert!((level(&surface, -250.0, 110.0) - level(&before, -250.0, 110.0)).abs() < 0.02, "the big river is as it was");
        assert!(current(&surface, 0.0, -100.0)[1] > 0.1, "and runs down to it");
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
        let sea = Map { bodies: vec![Body { kind: Kind::Sea, rings: vec![ring], level: Some(0.0) }], ..Map::default() };
        assert!(Surface::new(&sea, grid(), |_, _| Some(1.0)).reaches().iter().all(|&steps| steps == u8::MAX), "the sea carries every swell");
    }

    #[test]
    fn a_rivers_reach_is_a_small_share_of_its_width_so_the_wind_raises_only_a_chop() {
        // 120 m wide, running past both ends of the grid: a lake that wide would carry the 25 m swell.
        let map = river(vec![[-400.0, -60.0], [400.0, -60.0], [400.0, 60.0], [-400.0, 60.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - x / 100.0 + noise(x, z)));
        let reach = f32::from(surface.reaches()[surface.index(0.0, 0.0).unwrap()]) * REACH_STEP;
        assert!((reach - RIVER_REACH_MAX).abs() <= REACH_STEP, "the middle of a wide river has the most a river gets: {reach}");
        let swell = shared::waves::wave(0.0, 0.0, 0.0, reach, 1.0);
        let lake = shared::waves::wave(0.0, 0.0, 0.0, 120.0, 1.0);
        assert!(swell.height.abs() < 0.05 && swell.height.abs() < lake.height.abs(), "{} {}", swell.height, lake.height);
    }

    #[test]
    fn the_sea_shelves_steadily_out_to_its_farthest_point() {
        // A coast along x = -350 m, sea to the east as far as the grid goes (about 750 m).
        let ring = vec![[-350.0, -500.0], [500.0, -500.0], [500.0, 500.0], [-350.0, 500.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Sea, rings: vec![ring], level: Some(0.0) }], ..Map::default() };
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
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![outer, island], level: None }], ..Map::default() };
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

    #[test]
    fn a_stream_drawn_through_water_does_not_show_where_its_straight_stretch_strays_onto_the_bank() {
        // A pool that bends, and a line through it that cuts the bend's corner over dry ground.
        let pool = vec![[-60.0, -8.0], [8.0, -8.0], [8.0, 60.0], [-8.0, 60.0], [-8.0, 8.0], [-60.0, 8.0]];
        let line = vec![[-54.0, 0.0], [-40.0, 0.0], [0.0, 40.0], [0.0, 54.0]];
        let map = Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![pool], level: None }], streams: vec![line], ..Map::default() };
        let grid = Grid { origin: -7_000, cell: 100, side: 141 };
        let surface = Surface::new(&map, grid, |_, _| Some(5.0));
        let heights = (0..141 * 141).map(|at| (surface.bed(grid.x(at % 141), grid.x(at / 141), Some(5.0)).unwrap() * 100.0).round() as i32).collect();
        let ground = Ground { origin: -7_000, cell: 100, side: 141, heights };
        assert!(!surface.is_wet(-20.0, 20.0), "the corner it cuts is dry");
        let placed = Placed::new(&map, &surface, 0.0, &ground);
        assert!(placed.streams.is_empty(), "{:?}", placed.streams);
    }

    /// What is drawn of `line` by the pond (`pond`: 9.3 m in radius round about the origin, on samples
    /// 1 m apart), and the water it was trimmed by.
    fn stream_by_the_pond(line: Vec<P>) -> (Vec<Vec<P>>, Placed, Ground) {
        let (surface, ground, _) = pond();
        let placed = Placed::new(&Map { streams: vec![line], ..Map::default() }, &surface, 0.0, &ground);
        (placed.streams.clone(), placed, ground)
    }

    #[test]
    fn a_stream_is_cut_at_the_waterline_wherever_its_points_lie() {
        // One segment right across the pond, with no point in it.
        let (runs, placed, ground) = stream_by_the_pond(vec![[-14.0, 0.0], [14.0, 0.0]]);
        assert_eq!(runs.len(), 2, "{runs:?}");
        for end in [runs[0][1], runs[1][0]] {
            assert!(placed.depth_at(&ground, end[0], end[1]).abs() < 0.02, "not on the waterline: {end:?}");
        }
    }

    #[test]
    fn a_stream_that_ends_in_a_pond_stops_at_its_shore() {
        let (runs, placed, ground) = stream_by_the_pond(vec![[-14.0, 0.0], [0.0, 0.0]]);
        assert_eq!(runs.len(), 1, "{runs:?}");
        let end = runs[0][runs[0].len() - 1];
        assert!(end[0] < -8.0 && placed.depth_at(&ground, end[0], end[1]).abs() < 0.02, "{end:?}");
    }

    #[test]
    fn a_stream_wholly_under_water_is_not_drawn() {
        assert!(stream_by_the_pond(vec![[-5.0, 0.0], [5.0, 1.0]]).0.is_empty());
    }

    #[test]
    fn a_stream_clear_of_the_water_is_left_as_it_was() {
        // Shorter than a piece the water cuts may be: nothing cut it.
        let line = vec![[-14.0, 13.0], [-12.8, 13.2]];
        assert_eq!(stream_by_the_pond(line.clone()).0, vec![line]);
    }

    /// A river 40 m wide on a grid of 321 × 321 samples every metre, with a 2 m rapid at x = 0.
    fn fine_rapid() -> Surface {
        let map = river(vec![[-200.0, -20.0], [200.0, -20.0], [200.0, 20.0], [-200.0, 20.0]]);
        Surface::new(&map, Grid { origin: -16_000, cell: 100, side: 321 }, |x, z| Some(10.0 - x / 1000.0 - if x < 0.0 { 0.0 } else { 2.0 } + 0.02 * noise(x, z)))
    }

    /// How many crests the surface has along `z` from `from` to `to` metres of x: samples that
    /// stand at least 5 cm over the mean of the water 3 m up and down the river from them.
    fn crests(surface: &Surface, z: f64, from: i32, to: i32) -> usize {
        let over = |x: i32| level(surface, f64::from(x), z) - 0.5 * (level(surface, f64::from(x - 3), z) + level(surface, f64::from(x + 3), z));
        (from..=to).filter(|&x| over(x) > 0.05 && over(x) >= over(x - 1) && over(x) > over(x + 1)).count()
    }

    #[test]
    fn a_rapid_stands_in_waves_across_the_current_and_a_pool_lies_flat() {
        let surface = fine_rapid();
        let fast = current(&surface, 20.0, 0.0);
        assert!(fast[0] > 1.5, "the rapid runs: {fast:?}");
        let standing = crests(&surface, 0.0, 5, 45);
        assert!((4..=12).contains(&standing), "a crest every few metres down the rapid: {standing}");
        // A crest lies across the current: the water beside it stands with it.
        let beside = crests(&surface, 3.0, 5, 45);
        assert!(beside.abs_diff(standing) <= 2, "{beside} beside {standing}");
        assert_eq!(crests(&surface, 0.0, -140, -100), 0, "the slow water above lies flat");
    }

    #[test]
    fn a_river_is_never_written_as_quite_still_even_in_the_slack_water_at_its_banks() {
        let surface = fine_rapid();
        let flows = surface.flows();
        let still = (0..surface.body.len()).filter(|&at| surface.body[at] != NONE && flows[2 * at] == 0 && flows[2 * at + 1] == 0).count();
        assert_eq!(still, 0, "every sample of it runs at a step of the flow at least");
        assert!(Surface::new(&lake(), grid(), |_, _| Some(1.0)).flows().is_empty(), "and a lake is still");
    }

    #[test]
    fn a_coarse_grid_cannot_hold_a_standing_wave_and_gets_none() {
        // The same rapid sampled every 2 m: a 6.8 m wave would be three samples long.
        let map = river(vec![[-400.0, -20.0], [400.0, -20.0], [400.0, 20.0], [-400.0, 20.0]]);
        let surface = Surface::new(&map, river_grid(), |x, z| Some(10.0 - x / 1000.0 - if x < 0.0 { 0.0 } else { 2.0 } + 0.02 * noise(x, z)));
        let over = |x: f64| level(&surface, x, 0.0) - 0.5 * (level(&surface, x - 4.0, 0.0) + level(&surface, x + 4.0, 0.0));
        assert!((20..=50).all(|k| over(f64::from(k) * 2.0).abs() < 0.05), "no crest stands out below the rapid");
    }

    #[test]
    fn the_water_stands_higher_against_an_island_than_in_the_fast_water_beside_it() {
        // 60 m wide with an island 16 m square in the middle, the ground falling 2 m per km.
        let outer = vec![[-400.0, -30.0], [400.0, -30.0], [400.0, 30.0], [-400.0, 30.0]];
        let island = vec![[-8.0, -8.0], [8.0, -8.0], [8.0, 8.0], [-8.0, 8.0]];
        let map = Map { bodies: vec![Body { kind: Kind::River, rings: vec![outer, island], level: None }], ..Map::default() };
        let surface = Surface::new(&map, river_grid(), |x, _| Some(10.0 - x / 500.0));
        assert!(!surface.is_wet(0.0, 0.0), "the island is dry");
        let even = |x: f64| level(&surface, -100.0, 0.0) - (x + 100.0) / 500.0;
        let (before, beside) = (level(&surface, -12.0, 0.0) - even(-12.0), level(&surface, 0.0, 20.0) - even(0.0));
        assert!(before > beside + 0.03, "{before} against the island, {beside} beside it");
        // Across the river above the island the surface is level again.
        assert!((level(&surface, -200.0, -20.0) - level(&surface, -200.0, 20.0)).abs() < 0.03);
    }
}
