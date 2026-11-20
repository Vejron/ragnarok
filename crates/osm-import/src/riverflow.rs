//! A river's current, solved once at import.
//!
//! The water is taken as one layer under a rigid lid: its surface is already fixed (`water.rs` fits
//! a level that falls along the course), so what is left of the Navier-Stokes equations is the
//! velocity averaged over the depth, `u`, with
//!
//! - the same volume through every cross-section, `∇·(h·u) = 0` over the bed's depth `h`;
//! - the surface's fall pushing the water downstream, `g·S`;
//! - the bed holding it back by Manning's law, `g·n²·|u|·u / h^(4/3)`;
//! - the water carrying its own momentum along, which is what makes it shoot past a point and
//!   turn back in an eddy behind it;
//! - an eddy viscosity that spreads the momentum sideways and stills the water at the banks.
//!
//! `foam` then works out where the water breaks into foam, and where that foam is carried to.
//!
//! `solve` marches these in pseudo-time from the speed each sample would run at alone until
//! nothing changes any more, on a staggered grid (the velocity through each cell's faces, the
//! pressure in its middle). The result is the same bytes every run: nothing in here is parallel or
//! random.

/// Gravity, m/s².
pub const G: f64 = 9.81;
/// Manning's roughness of a river's bed, s/m^(1/3): a stony bed with boulders. With the depths
/// `water.rs` cuts, a gently falling river (1 m a km) then runs at a little under 1 m/s.
pub const MANNING_N: f64 = 0.05;
/// How fast momentum spreads sideways through the water, m²/s. It sets how wide the slow water
/// along a bank is (a few metres) and how readily the current sheds an eddy.
const EDDY_VISCOSITY: f64 = 0.3;
/// The solver's cells are at least this wide, metres: a world sampled finer is solved on every
/// second (or fourth) sample and read back between them.
const CELL_MIN: f64 = 2.0;
/// A step of pseudo-time, seconds for each metre of a cell's width, so the fastest water crosses a
/// few cells a step.
const STEP_PER_METRE: f64 = 1.0;
/// The current has settled when no face's velocity changed by more than this in a step, m/s (a
/// twentieth of the step `Tile::flow` stores).
const SETTLED: f64 = 0.0025;
/// The most steps taken: water that keeps shedding eddies never settles, and is taken as it is then.
const STEPS_MAX: usize = 400;
/// No face counts as shallower than this, metres, so the bed's hold on the water stays finite.
const DEPTH_MIN: f64 = 0.05;
/// The pressure is solved until what is left of the divergence is this share of what there was.
const PRESSURE_EXACT: f64 = 1.0e-4;
/// The most conjugate gradient steps one pressure solve takes.
const PRESSURE_STEPS: usize = 200;

const NONE: u32 = u32::MAX;

/// A stretch of river to solve, on the world's sample grid.
pub struct Channel<'a> {
    /// Samples a side of the world's grid.
    pub side: usize,
    /// The grid's spacing, metres.
    pub cell: f64,
    /// The stretch's samples, as indices into the grid.
    pub members: &'a [usize],
    /// The bed's depth under every sample of the grid, metres.
    pub depth: &'a [f32],
    /// At each member, the surface's fall along the course: its slope, metres a metre, times the
    /// unit direction downstream, `[x, z]`.
    pub fall: &'a [[f64; 2]],
    /// Whether each member is at an end of the stretch, where water comes in or leaves.
    pub open: &'a [bool],
}

/// What `solve` found.
pub struct Current {
    /// The water's velocity at each member, `[x, z]` m/s.
    pub flow: Vec<[f32; 2]>,
    /// How far the water stands above (or below) the even surface it was solved under at each
    /// member, metres: the pressure the lid had to bear, as a head of water. It is up where the
    /// current is braked (before a boulder, above a narrows, on the outside of a bend) and down
    /// where it runs fast.
    pub lift: Vec<f32>,
    /// How many steps of pseudo-time it took.
    pub steps: usize,
    /// Whether it settled before `STEPS_MAX`.
    pub settled: bool,
}

/// The solver's cells: the wet ones only, each knowing its four neighbours.
struct Lattice {
    /// Cells across and down the stretch's bounding box, with a dry ring round it.
    nx: usize,
    nz: usize,
    /// A cell's width, metres.
    dx: f64,
    /// The box's first place, in cells from the world grid's first sample.
    origin: [f64; 2],
    /// Which cell is at each place of the box, `NONE` where it is dry.
    map: Vec<u32>,
    /// Each cell's place in the box, `[i, j]`.
    place: Vec<[u32; 2]>,
    /// Which of the channel's members each cell was made from.
    member: Vec<u32>,
    /// Each cell's neighbours to the west (-x), east, north (-z) and south, `NONE` where dry.
    around: Vec<[u32; 4]>,
    /// Whether the cell is at an open end: its pressure is held at zero.
    open: Vec<bool>,
    /// The surface's fall at the cell (see `Channel::fall`).
    fall: Vec<[f64; 2]>,
    /// The depth at the cell, metres.
    deep: Vec<f64>,
    /// The depth at each cell's west and north face, metres; 0 where the neighbour is dry, which
    /// is a bank no water crosses.
    deep_w: Vec<f64>,
    deep_n: Vec<f64>,
}

const WEST: usize = 0;
const EAST: usize = 1;
const NORTH: usize = 2;
const SOUTH: usize = 3;

impl Lattice {
    /// The cell at `(i, j)` of the box, if it is wet.
    fn at(&self, i: i64, j: i64) -> Option<usize> {
        if i < 0 || j < 0 || i >= self.nx as i64 || j >= self.nz as i64 {
            return None;
        }
        let cell = self.map[j as usize * self.nx + i as usize];
        (cell != NONE).then_some(cell as usize)
    }

    /// `field` (a value at each cell's lattice point, 0 where dry) at `(x, z)` in lattice units,
    /// blended from the four points round it.
    fn blend(&self, field: &[f64], x: f64, z: f64) -> f64 {
        let x = x.clamp(0.0, self.nx as f64 - 1.0);
        let z = z.clamp(0.0, self.nz as f64 - 1.0);
        let (i, j) = (x.floor() as i64, z.floor() as i64);
        let (fx, fz) = (x - i as f64, z - j as f64);
        let value = |i: i64, j: i64| self.at(i, j).map_or(0.0, |cell| field[cell]);
        let near = value(i, j) + (value(i + 1, j) - value(i, j)) * fx;
        let far = value(i, j + 1) + (value(i + 1, j + 1) - value(i, j + 1)) * fx;
        near + (far - near) * fz
    }

    /// `field` at `cell`'s neighbour on `side`, 0 where that is dry.
    fn beside(&self, field: &[f64], cell: usize, side: usize) -> f64 {
        match self.around[cell][side] {
            NONE => 0.0,
            next => field[next as usize],
        }
    }

    /// The velocity along z at `cell`'s west face, from the four north and south faces round it.
    fn v_at_west(&self, v: &[f64], cell: usize) -> f64 {
        let west = self.around[cell][WEST];
        let mut sum = v[cell] + self.beside(v, cell, SOUTH);
        if west != NONE {
            sum += v[west as usize] + self.beside(v, west as usize, SOUTH);
        }
        0.25 * sum
    }

    /// The velocity along x at `cell`'s north face, from the four west and east faces round it.
    fn u_at_north(&self, u: &[f64], cell: usize) -> f64 {
        let north = self.around[cell][NORTH];
        let mut sum = u[cell] + self.beside(u, cell, EAST);
        if north != NONE {
            sum += u[north as usize] + self.beside(u, north as usize, EAST);
        }
        0.25 * sum
    }

    /// How much more water leaves each cell than enters it, m²/s a metre of face.
    fn divergence(&self, u: &[f64], v: &[f64], out: &mut [f64]) {
        for cell in 0..u.len() {
            let [_, east, _, south] = self.around[cell];
            let mut sum = -self.deep_w[cell] * u[cell] - self.deep_n[cell] * v[cell];
            if east != NONE {
                sum += self.deep_w[east as usize] * u[east as usize];
            }
            if south != NONE {
                sum += self.deep_n[south as usize] * v[south as usize];
            }
            out[cell] = sum;
        }
    }
}

/// One grid of the pressure solve: the finest is the lattice's cells, each coarser one a cell for
/// every 2 × 2 of the one before. A level holds the pressure's operator there, which for every
/// cell is the sum over its faces of the face's weight (its depth) times the pressure's drop
/// across it.
struct Level {
    /// Each cell's place on this level's grid, `[i, j]`.
    place: Vec<[u32; 2]>,
    /// Each cell's neighbours (west, east, north, south) that the operator ties it to, `NONE`
    /// where there is none.
    around: Vec<[u32; 4]>,
    /// The weight of each cell's tie to its west and north neighbours.
    west: Vec<f64>,
    north: Vec<f64>,
    /// The operator's diagonal: the weights of all of a cell's faces, tied or held at zero beyond.
    diagonal: Vec<f64>,
    /// Whether the cell's pressure is solved for (an open cell's is held at zero).
    solved: Vec<bool>,
    /// The cell of the next coarser level each cell belongs to.
    parent: Vec<u32>,
    /// Working arrays of a cycle: the guess, what it must match, what is left, and a spare.
    x: Vec<f64>,
    b: Vec<f64>,
    r: Vec<f64>,
    spare: Vec<f64>,
}

/// A level no bigger than this is the coarsest.
const COARSEST: usize = 24;
/// Smoothing passes before and after a level's coarser correction, and on the coarsest level.
const SMOOTHINGS: usize = 2;
const COARSEST_SMOOTHINGS: usize = 40;
/// How far a smoothing pass moves towards each cell's own solution.
const SMOOTHING: f64 = 0.8;
/// A coarser level's correction is taken this many times over: a cell standing for 2 × 2 with one
/// value between them underestimates how far the pressure must move.
const OVERCORRECTION: f64 = 1.6;

impl Level {
    /// The lattice's own level.
    fn finest(lattice: &Lattice) -> Self {
        let n = lattice.open.len();
        let tied = |cell: usize, side: usize| match lattice.around[cell][side] {
            next if next != NONE && !lattice.open[cell] && !lattice.open[next as usize] => next,
            _ => NONE,
        };
        let around = (0..n).map(|cell| [WEST, EAST, NORTH, SOUTH].map(|side| tied(cell, side))).collect();
        let diagonal = (0..n)
            .map(|cell| {
                let [_, east, _, south] = lattice.around[cell];
                let mut sum = lattice.deep_w[cell] + lattice.deep_n[cell];
                if east != NONE {
                    sum += lattice.deep_w[east as usize];
                }
                if south != NONE {
                    sum += lattice.deep_n[south as usize];
                }
                sum
            })
            .collect::<Vec<f64>>();
        let solved = (0..n).map(|cell| !lattice.open[cell] && diagonal[cell] > 0.0).collect();
        Self::new(lattice.place.clone(), around, lattice.deep_w.clone(), lattice.deep_n.clone(), diagonal, solved)
    }

    fn new(place: Vec<[u32; 2]>, around: Vec<[u32; 4]>, west: Vec<f64>, north: Vec<f64>, diagonal: Vec<f64>, solved: Vec<bool>) -> Self {
        let n = place.len();
        Self { place, around, west, north, diagonal, solved, parent: Vec::new(), x: vec![0.0; n], b: vec![0.0; n], r: vec![0.0; n], spare: vec![0.0; n] }
    }

    /// The next coarser level, and each of this one's solved cells' `parent` in it.
    fn coarser(&mut self) -> Self {
        let n = self.place.len();
        let mut found = std::collections::HashMap::new();
        let mut place = Vec::new();
        self.parent = vec![NONE; n];
        for cell in (0..n).filter(|&cell| self.solved[cell]) {
            let key = [self.place[cell][0] / 2, self.place[cell][1] / 2];
            self.parent[cell] = *found.entry(key).or_insert_with(|| {
                place.push(key);
                place.len() as u32 - 1
            });
        }
        let m = place.len();
        let (mut around, mut west, mut north, mut diagonal) = (vec![[NONE; 4]; m], vec![0.0; m], vec![0.0; m], vec![0.0; m]);
        for cell in (0..n).filter(|&cell| self.solved[cell]) {
            let parent = self.parent[cell] as usize;
            diagonal[parent] += self.diagonal[cell];
            for (side, back, weight, sum) in [(WEST, EAST, self.west[cell], &mut west), (NORTH, SOUTH, self.north[cell], &mut north)] {
                let next = self.around[cell][side];
                if next == NONE {
                    continue;
                }
                let other = self.parent[next as usize] as usize;
                if other == parent {
                    // A tie inside the coarse cell: both its ends move together, so it holds nothing.
                    diagonal[parent] -= 2.0 * weight;
                } else {
                    around[parent][side] = other as u32;
                    around[other][back] = parent as u32;
                    sum[parent] += weight;
                }
            }
        }
        Self::new(place, around, west, north, diagonal, vec![true; m])
    }

    /// The operator on `x`, into `out`; 0 at cells that are not solved.
    fn apply(&self, x: &[f64], out: &mut [f64]) {
        for cell in 0..x.len() {
            if !self.solved[cell] {
                out[cell] = 0.0;
                continue;
            }
            let [west, east, north, south] = self.around[cell];
            let mut sum = self.diagonal[cell] * x[cell];
            if west != NONE {
                sum -= self.west[cell] * x[west as usize];
            }
            if east != NONE {
                sum -= self.west[east as usize] * x[east as usize];
            }
            if north != NONE {
                sum -= self.north[cell] * x[north as usize];
            }
            if south != NONE {
                sum -= self.north[south as usize] * x[south as usize];
            }
            out[cell] = sum;
        }
    }

    /// `passes` smoothing passes on `x` towards `b` (damped Jacobi).
    fn smooth(&mut self, passes: usize) {
        for _ in 0..passes {
            let mut spare = std::mem::take(&mut self.spare);
            self.apply(&self.x, &mut spare);
            for (cell, applied) in spare.iter().enumerate() {
                if self.solved[cell] {
                    self.x[cell] += SMOOTHING * (self.b[cell] - applied) / self.diagonal[cell];
                }
            }
            self.spare = spare;
        }
    }
}

/// One multigrid cycle from `levels[0]` down and back: leaves in its `x` roughly the pressure
/// whose operator gives its `b`. The same passes going down as coming up, so the cycle is a
/// symmetric operator and conjugate gradients may use it as their preconditioner.
fn cycle(levels: &mut [Level]) {
    let (level, below) = levels.split_first_mut().expect("there is a finest level");
    level.x.fill(0.0);
    let Some(coarse) = below.first_mut() else {
        level.smooth(COARSEST_SMOOTHINGS);
        return;
    };
    level.smooth(SMOOTHINGS);
    let mut r = std::mem::take(&mut level.r);
    level.apply(&level.x, &mut r);
    coarse.b.fill(0.0);
    for (cell, applied) in r.iter().enumerate() {
        if level.solved[cell] {
            coarse.b[level.parent[cell] as usize] += level.b[cell] - applied;
        }
    }
    level.r = r;
    cycle(below);
    let coarse = &below[0];
    for cell in 0..level.x.len() {
        if level.solved[cell] {
            level.x[cell] += OVERCORRECTION * coarse.x[level.parent[cell] as usize];
        }
    }
    level.smooth(SMOOTHINGS);
}

/// The pressure solve, kept between steps: the last pressure is the next one's first guess.
struct Pressure {
    levels: Vec<Level>,
    p: Vec<f64>,
    residual: Vec<f64>,
    direction: Vec<f64>,
    spread: Vec<f64>,
}

impl Pressure {
    fn new(lattice: &Lattice) -> Self {
        let n = lattice.open.len();
        let mut levels = vec![Level::finest(lattice)];
        loop {
            let last = levels.last_mut().expect("there is a finest level");
            let solved = last.solved.iter().filter(|&&solved| solved).count();
            if solved <= COARSEST {
                break;
            }
            let next = last.coarser();
            // Cells strung out singly do not pair up; what is left is then the coarsest.
            if next.place.len() * 10 > solved * 9 {
                last.parent.clear();
                break;
            }
            levels.push(next);
        }
        Self { levels, p: vec![0.0; n], residual: vec![0.0; n], direction: vec![0.0; n], spread: vec![0.0; n] }
    }

    /// Takes out of `u`, `v` whatever piles water up: solves for the pressure whose gradient
    /// leaves as much water entering each cell as leaving it (conjugate gradients, with a
    /// multigrid `cycle` as the preconditioner), and subtracts that gradient.
    fn project(&mut self, lattice: &Lattice, u: &mut [f64], v: &mut [f64]) {
        let n = self.p.len();
        let mut rhs = std::mem::take(&mut self.spread);
        lattice.divergence(u, v, &mut rhs);
        let mut size = 0.0;
        for (wanted, &solved) in rhs.iter_mut().zip(&self.levels[0].solved) {
            *wanted = if solved { -*wanted } else { 0.0 };
            size += *wanted * *wanted;
        }
        self.levels[0].apply(&self.p, &mut self.residual);
        for (left, wanted) in self.residual.iter_mut().zip(&rhs) {
            *left = wanted - *left;
        }
        self.spread = rhs;
        let enough = PRESSURE_EXACT * PRESSURE_EXACT * size;
        let mut along = 0.0;
        for step in 0..PRESSURE_STEPS {
            let left: f64 = self.residual.iter().map(|r| r * r).sum();
            if left <= enough {
                break;
            }
            self.levels[0].b.copy_from_slice(&self.residual);
            cycle(&mut self.levels);
            let scaled = &self.levels[0].x;
            let next: f64 = self.residual.iter().zip(scaled).map(|(r, z)| r * z).sum();
            if next <= 0.0 {
                break;
            }
            let carry = if step == 0 { 0.0 } else { next / along };
            for (direction, scaled) in self.direction.iter_mut().zip(scaled) {
                *direction = scaled + carry * *direction;
            }
            along = next;
            self.levels[0].apply(&self.direction, &mut self.spread);
            let curve: f64 = self.direction.iter().zip(&self.spread).map(|(d, s)| d * s).sum();
            if curve <= 0.0 {
                break;
            }
            let stride = along / curve;
            for cell in 0..n {
                self.p[cell] += stride * self.direction[cell];
                self.residual[cell] -= stride * self.spread[cell];
            }
        }
        for cell in 0..n {
            let [west, _, north, _] = lattice.around[cell];
            if west != NONE {
                u[cell] -= self.p[cell] - self.p[west as usize];
            }
            if north != NONE {
                v[cell] -= self.p[cell] - self.p[north as usize];
            }
        }
    }
}

/// The steady current of `channel`: see the module's notes.
pub fn solve(channel: &Channel) -> Current {
    let still = |steps| Current { flow: vec![[0.0; 2]; channel.members.len()], lift: vec![0.0; channel.members.len()], steps, settled: true };
    let every = (CELL_MIN / channel.cell).ceil().max(1.0) as usize;
    let Some(lattice) = lattice(channel, every) else { return still(0) };
    let n = lattice.open.len();
    let dt = STEP_PER_METRE * lattice.dx;
    // The fall's push on each west and north face, m/s², and the bed's hold there, 1/m: the water
    // is slowed at `hold · speed` a second.
    let push = |a: [f64; 2], b: [f64; 2], axis: usize| G * 0.5 * (a[axis] + b[axis]);
    let hold = |depth: f64| G * MANNING_N * MANNING_N / depth.max(DEPTH_MIN).powf(4.0 / 3.0);
    let mut push_w = vec![0.0; n];
    let mut push_n = vec![0.0; n];
    let mut hold_w = vec![0.0; n];
    let mut hold_n = vec![0.0; n];
    // Velocity through each cell's west face (along x) and north face (along z), m/s; 0 at a bank.
    let mut u = vec![0.0; n];
    let mut v = vec![0.0; n];
    for cell in 0..n {
        let [west, _, north, _] = lattice.around[cell];
        // Where the water starts: at the speed the fall and the bed alone would give it.
        let alone = |fall: [f64; 2], depth: f64, axis: usize| {
            let slope = fall[0].hypot(fall[1]);
            if slope > 0.0 { depth.max(DEPTH_MIN).powf(2.0 / 3.0) * slope.sqrt() / MANNING_N * fall[axis] / slope } else { 0.0 }
        };
        if west != NONE {
            let fall = [0, 1].map(|axis| 0.5 * (lattice.fall[cell][axis] + lattice.fall[west as usize][axis]));
            push_w[cell] = push(lattice.fall[cell], lattice.fall[west as usize], 0);
            hold_w[cell] = hold(lattice.deep_w[cell]);
            u[cell] = alone(fall, lattice.deep_w[cell], 0);
        }
        if north != NONE {
            let fall = [0, 1].map(|axis| 0.5 * (lattice.fall[cell][axis] + lattice.fall[north as usize][axis]));
            push_n[cell] = push(lattice.fall[cell], lattice.fall[north as usize], 1);
            hold_n[cell] = hold(lattice.deep_n[cell]);
            v[cell] = alone(fall, lattice.deep_n[cell], 1);
        }
    }
    let mut pressure = Pressure::new(&lattice);
    pressure.project(&lattice, &mut u, &mut v);
    // The viscosity is spread in as many passes as keep each one stable.
    let passes = (dt * EDDY_VISCOSITY / (lattice.dx * lattice.dx) / 0.2).ceil().max(1.0) as usize;
    let spread = dt * EDDY_VISCOSITY / (lattice.dx * lattice.dx) / passes as f64;
    let (mut next_u, mut next_v) = (vec![0.0; n], vec![0.0; n]);
    let (mut steps, mut settled) = (0, false);
    while steps < STEPS_MAX && !settled {
        steps += 1;
        // The water carries its velocity along: each face takes the velocity of the water that
        // arrives at it, looked up where that water was a step ago.
        for cell in 0..n {
            let [west, _, north, _] = lattice.around[cell];
            let [i, j] = lattice.place[cell].map(f64::from);
            next_u[cell] = if west == NONE {
                0.0
            } else {
                let across = lattice.v_at_west(&v, cell);
                lattice.blend(&u, i - u[cell] * dt / lattice.dx, j - across * dt / lattice.dx)
            };
            next_v[cell] = if north == NONE {
                0.0
            } else {
                let across = lattice.u_at_north(&u, cell);
                lattice.blend(&v, i - across * dt / lattice.dx, j - v[cell] * dt / lattice.dx)
            };
        }
        // The fall pushes and the bed holds (the hold taken at the end of the step, so a shallow
        // face cannot overshoot).
        for cell in 0..n {
            let [west, _, north, _] = lattice.around[cell];
            if west != NONE {
                let speed = next_u[cell].hypot(lattice.v_at_west(&v, cell));
                next_u[cell] = (next_u[cell] + dt * push_w[cell]) / (1.0 + dt * hold_w[cell] * speed);
            }
            if north != NONE {
                let speed = next_v[cell].hypot(lattice.u_at_north(&u, cell));
                next_v[cell] = (next_v[cell] + dt * push_n[cell]) / (1.0 + dt * hold_n[cell] * speed);
            }
        }
        // Momentum spreads sideways. A dry neighbour counts as still: the water sticks to the bank.
        for _ in 0..passes {
            for (field, face) in [(&mut next_u, WEST), (&mut next_v, NORTH)] {
                let before = field.clone();
                for cell in 0..n {
                    if lattice.around[cell][face] == NONE {
                        continue;
                    }
                    let round: f64 = (0..4).map(|side| lattice.beside(&before, cell, side)).sum();
                    field[cell] += spread * (round - 4.0 * before[cell]);
                }
            }
        }
        pressure.project(&lattice, &mut next_u, &mut next_v);
        let change = u.iter().zip(&next_u).chain(v.iter().zip(&next_v)).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        std::mem::swap(&mut u, &mut next_u);
        std::mem::swap(&mut v, &mut next_v);
        settled = change <= SETTLED;
    }
    // Each cell's velocity is the mean of its faces'.
    let middle_u: Vec<f64> = (0..n).map(|cell| 0.5 * (u[cell] + lattice.beside(&u, cell, EAST))).collect();
    let middle_v: Vec<f64> = (0..n).map(|cell| 0.5 * (v[cell] + lattice.beside(&v, cell, SOUTH))).collect();
    let mut out = still(steps);
    out.settled = settled;
    for (flow, &at) in out.flow.iter_mut().zip(channel.members) {
        let (x, z) = lattice.position(at, channel.side, every);
        *flow = [lattice.among(&middle_u, x, z) as f32, lattice.among(&middle_v, x, z) as f32];
    }
    // The projection takes `p[cell] - p[west]` off a face's velocity each step, which is what a
    // surface standing `p · dx / (g · dt)` higher would do in that time.
    let head: Vec<f64> = pressure.p.iter().map(|p| p * lattice.dx / (G * dt)).collect();
    for (lift, &at) in out.lift.iter_mut().zip(channel.members) {
        let (x, z) = lattice.position(at, channel.side, every);
        *lift = lattice.among(&head, x, z) as f32;
    }
    out
}

/// How long foam lasts on the water, seconds: it thins to a third in this time, so the trail below
/// a rapid is about this many metres long for each m/s the water carries it at.
const FOAM_LIFE: f64 = 45.0;
/// How long water that runs all white takes to cover itself with foam, seconds.
const FOAM_RISE: f64 = 4.0;
/// Fast water starts to break at the first of these Froude numbers (its speed over `√(g · depth)`,
/// the speed of a wave on it) and all of it breaks by the second: thin water does, a deep run at
/// the same speed does not.
const FOAM_FROUDES: [f64; 2] = [0.6, 1.1];
/// Fast water that loses speed starts to break when it loses the first of these, m/s each second,
/// and all of it breaks by the second.
const FOAM_SLOWING: [f64; 2] = [0.2, 1.0];
/// The foam has settled when no cell's changed by more than this in a step...
const FOAM_SETTLED: f64 = 0.001;
/// ...and is taken as it is after this many steps.
const FOAM_STEPS: usize = 600;

/// How much of the water's surface is under foam at each member of `channel`, 0 to 1, given the
/// current `flow` there (`[x, z]` m/s, as `solve` gives it).
///
/// Foam is made where the water breaks: where it runs white (`WHITE_WATER_SPEEDS`) and thin for
/// its speed (`FOAM_FROUDES`), as over a bar or a boulder, and where fast water is braked
/// (`FOAM_SLOWING`), as below one. A deep run at the same speed makes none. It is carried along
/// by the current and thins with age (`FOAM_LIFE`). So it trails away below a rapid, and gathers
/// where the water goes round: an eddy is fed its own foam back. This is marched in pseudo-time to a steady state
/// on the cells `solve` uses.
pub fn foam(channel: &Channel, flow: &[[f32; 2]]) -> Vec<f32> {
    let every = (CELL_MIN / channel.cell).ceil().max(1.0) as usize;
    let mut out = vec![0.0; channel.members.len()];
    let Some(lattice) = lattice(channel, every) else { return out };
    let n = lattice.open.len();
    let dt = STEP_PER_METRE * lattice.dx;
    let [from, to] = shared::consts::WHITE_WATER_SPEEDS.map(f64::from);
    // Each cell's current, in cells a step, and how fast its water makes foam, a second.
    let carried: Vec<[f64; 2]> = lattice.member.iter().map(|&member| flow[member as usize].map(|v| f64::from(v) * dt / lattice.dx)).collect();
    let ease = |from: f64, to: f64, x: f64| {
        let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let speed: Vec<f64> = lattice.member.iter().map(|&member| f64::from(flow[member as usize][0]).hypot(f64::from(flow[member as usize][1]))).collect();
    let made: Vec<f64> = (0..n)
        .map(|cell| {
            let here = speed[cell];
            if here <= 0.0 {
                return 0.0;
            }
            // The speed of the water a cell upstream and a cell down.
            let [i, j] = lattice.place[cell].map(f64::from);
            let step = carried[cell].map(|v| v / (here * dt / lattice.dx));
            let above = lattice.among(&speed, i - step[0], j - step[1]);
            let below = lattice.among(&speed, i + step[0], j + step[1]);
            // Thin water running fast breaks; deep water running as fast is a dark tongue.
            let thin = ease(from, to, here) * ease(FOAM_FROUDES[0], FOAM_FROUDES[1], here / (G * lattice.deep[cell]).sqrt());
            // Fast water that is being slowed piles up and breaks: below a boulder or a ledge.
            let slowing = here * (above - below) / (2.0 * lattice.dx);
            let braked = ease(0.5 * from, from, above) * ease(FOAM_SLOWING[0], FOAM_SLOWING[1], slowing);
            thin.max(braked) / FOAM_RISE
        })
        .collect();
    if made.iter().all(|&rate| rate == 0.0) {
        return out;
    }
    // How much of the difference to each neighbour a cell takes up in a step.
    let mixing = (dt * EDDY_VISCOSITY / (lattice.dx * lattice.dx)).min(0.2);
    let mut cover = vec![0.0; n];
    let mut arrived = vec![0.0; n];
    let mut next = vec![0.0; n];
    for _ in 0..FOAM_STEPS {
        let mut change = 0.0f64;
        // Carried: each cell takes the foam of the water that arrives at it.
        for cell in 0..n {
            let [i, j] = lattice.place[cell].map(f64::from);
            arrived[cell] = lattice.among(&cover, i - carried[cell][0], j - carried[cell][1]);
        }
        for cell in 0..n {
            // The water's churning also mixes it sideways, into the slack water beside a fast run
            // (none goes into a bank).
            let mixed: f64 = lattice.around[cell].iter().filter(|&&beside| beside != NONE).map(|&beside| arrived[beside as usize] - arrived[cell]).sum();
            // Made towards full cover and thinning, both taken at the end of the step.
            next[cell] = (arrived[cell] + mixing * mixed + dt * made[cell]) / (1.0 + dt * (made[cell] + 1.0 / FOAM_LIFE));
            change = change.max((next[cell] - cover[cell]).abs());
        }
        std::mem::swap(&mut cover, &mut next);
        if change <= FOAM_SETTLED {
            break;
        }
    }
    for (foam, &at) in out.iter_mut().zip(channel.members) {
        let (x, z) = lattice.position(at, channel.side, every);
        *foam = lattice.among(&cover, x, z) as f32;
    }
    out
}

impl Lattice {
    /// `field` (a value a cell) at `(x, z)` in lattice units, blended from the wet cells among the
    /// four round it; 0 where all four are dry.
    fn among(&self, field: &[f64], x: f64, z: f64) -> f64 {
        let (i, j) = (x.floor() as i64, z.floor() as i64);
        let (fx, fz) = (x - i as f64, z - j as f64);
        let (mut sum, mut weight) = (0.0, 0.0);
        for (di, dj, share) in [(0, 0, (1.0 - fx) * (1.0 - fz)), (1, 0, fx * (1.0 - fz)), (0, 1, (1.0 - fx) * fz), (1, 1, fx * fz)] {
            if let (true, Some(cell)) = (share > 0.0, self.at(i + di, j + dj)) {
                sum += share * field[cell];
                weight += share;
            }
        }
        if weight > 0.0 { sum / weight } else { 0.0 }
    }

    /// Where the grid sample `at` lies among the cells, in lattice units (a cell's lattice point
    /// is the sample it was made from).
    fn position(&self, at: usize, side: usize, every: usize) -> (f64, f64) {
        let (i, j) = ((at % side) as f64 / every as f64, (at / side) as f64 / every as f64);
        (i - self.origin[0], j - self.origin[1])
    }
}

/// The cells of `channel`: one for every `every`th of its samples each way. `None` when it has none.
fn lattice(channel: &Channel, every: usize) -> Option<Lattice> {
    let side = channel.side;
    let aligned = |at: usize| (at % side).is_multiple_of(every) && (at / side).is_multiple_of(every);
    let coarse = |at: usize| [(at % side) / every, (at / side) / every];
    let mut low = [usize::MAX; 2];
    let mut high = [0usize; 2];
    for &at in channel.members.iter().filter(|&&at| aligned(at)) {
        let c = coarse(at);
        low = [low[0].min(c[0]), low[1].min(c[1])];
        high = [high[0].max(c[0]), high[1].max(c[1])];
    }
    if low[0] == usize::MAX {
        return None;
    }
    // A dry ring round the box, so every wet cell has four places to look at.
    let (nx, nz) = (high[0] - low[0] + 3, high[1] - low[1] + 3);
    let mut map = vec![NONE; nx * nz];
    let (mut place, mut from, mut open, mut fall, mut depth) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (member, &at) in channel.members.iter().enumerate().filter(|&(_, &at)| aligned(at)) {
        let c = coarse(at);
        let (i, j) = (c[0] - low[0] + 1, c[1] - low[1] + 1);
        map[j * nx + i] = place.len() as u32;
        place.push([i as u32, j as u32]);
        from.push(member as u32);
        open.push(channel.open[member]);
        fall.push(channel.fall[member]);
        depth.push(f64::from(channel.depth[at]).max(DEPTH_MIN));
    }
    let around: Vec<[u32; 4]> = place
        .iter()
        .map(|&[i, j]| {
            let (i, j) = (i as usize, j as usize);
            [map[j * nx + i - 1], map[j * nx + i + 1], map[(j - 1) * nx + i], map[(j + 1) * nx + i]]
        })
        .collect();
    let face = |cell: usize, side: usize| match around[cell][side] {
        NONE => 0.0,
        next => 0.5 * (depth[cell] + depth[next as usize]),
    };
    let deep_w = (0..place.len()).map(|cell| face(cell, WEST)).collect();
    let deep_n = (0..place.len()).map(|cell| face(cell, NORTH)).collect();
    let origin = [low[0] as f64 - 1.0, low[1] as f64 - 1.0];
    Some(Lattice { nx, nz, dx: channel.cell * every as f64, origin, map, place, member: from, around, open, fall, deep: depth, deep_w, deep_n })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stretch of river for `solve`, with what `Channel` borrows.
    struct Bed {
        side: usize,
        members: Vec<usize>,
        depth: Vec<f32>,
        fall: Vec<[f64; 2]>,
        open: Vec<bool>,
    }

    impl Bed {
        /// A straight channel along x, `long` by `wide` samples and `depth` m deep, in a grid with
        /// dry ground round it; open at both ends and falling `slope` towards +x. `dry` takes
        /// samples out of it.
        fn new(long: usize, wide: usize, depth: f32, slope: f64, dry: impl Fn(usize, usize) -> bool) -> Self {
            let side = long + 2;
            let (mut members, mut open) = (Vec::new(), Vec::new());
            for j in 1..=wide {
                for i in (1..=long).filter(|&i| !dry(i, j)) {
                    members.push(j * side + i);
                    open.push(i <= 2 || i + 2 > long);
                }
            }
            let fall = vec![[slope, 0.0]; members.len()];
            Self { side, members, depth: vec![depth; side * side], fall, open }
        }

        /// Its current, with samples `cell` m apart.
        fn solve(&self, cell: f64) -> Solved<'_> {
            let current = solve(&Channel { side: self.side, cell, members: &self.members, depth: &self.depth, fall: &self.fall, open: &self.open });
            Solved { bed: self, current }
        }
    }

    struct Solved<'a> {
        bed: &'a Bed,
        current: Current,
    }

    impl Solved<'_> {
        /// The velocity at sample `(i, j)`, m/s.
        fn at(&self, i: usize, j: usize) -> [f32; 2] {
            self.current.flow[self.bed.members.iter().position(|&m| m == j * self.bed.side + i).unwrap()]
        }

        /// How far the water stands above the even surface at sample `(i, j)`, metres.
        fn lift(&self, i: usize, j: usize) -> f32 {
            self.current.lift[self.bed.members.iter().position(|&m| m == j * self.bed.side + i).unwrap()]
        }

        /// The water through the cross-section at column `i` of a 2 m grid, m³/s.
        fn discharge(&self, i: usize) -> f64 {
            let bed = self.bed;
            bed.members.iter().zip(&self.current.flow).filter(|&(&at, _)| at % bed.side == i).map(|(&at, flow)| f64::from(flow[0]) * f64::from(bed.depth[at]) * 2.0).sum()
        }
    }

    #[test]
    fn a_straight_even_channel_runs_at_mannings_speed_in_the_middle_and_slower_at_the_banks() {
        let bed = Bed::new(120, 20, 2.0, 0.001, |_, _| false);
        let solved = bed.solve(2.0);
        assert!(solved.current.settled, "{} steps", solved.current.steps);
        let manning = 2.0f64.powf(2.0 / 3.0) * 0.001f64.sqrt() / MANNING_N;
        let middle = solved.at(60, 10);
        // The banks' hold reaches the middle of a channel this narrow, and takes a tenth off.
        assert!(f64::from(middle[0]) > 0.85 * manning && f64::from(middle[0]) < manning && middle[1].abs() < 0.01, "{middle:?} against {manning}");
        let bank = solved.at(60, 1);
        assert!(bank[0] > 0.0 && bank[0] < 0.6 * middle[0], "{bank:?}");
    }

    #[test]
    fn the_same_water_passes_every_cross_section_and_runs_faster_through_a_narrows() {
        // 40 m wide, pinched to 16 m for 40 m in the middle.
        let bed = Bed::new(120, 20, 2.0, 0.001, |i, j| (50..70).contains(&i) && !(7..15).contains(&j));
        let solved = bed.solve(2.0);
        let through = [20, 40, 60, 80, 100].map(|i| solved.discharge(i));
        let mean = through.iter().sum::<f64>() / through.len() as f64;
        assert!(mean > 5.0 && through.iter().all(|q| (q - mean).abs() < 0.03 * mean), "{through:?}");
        let speed = |i: usize| solved.at(i, 10)[0];
        assert!(speed(60) > 1.8 * speed(20), "{} in the narrows against {} above it", speed(60), speed(20));
    }

    /// 60 m wide, with an island 20 m square in the middle of it.
    fn round_an_island(long: usize) -> Bed {
        Bed::new(long, 30, 2.5, 0.002, |i, j| (40..50).contains(&i) && (11..21).contains(&j))
    }

    #[test]
    fn the_current_parts_round_an_island_and_turns_back_behind_it() {
        let bed = round_an_island(140);
        let solved = bed.solve(2.0);
        let (above, beside) = (solved.at(20, 15), solved.at(45, 5));
        assert!(beside[0] > 1.2 * above[0], "faster past the island: {beside:?} against {above:?}");
        let (left, right) = (solved.at(38, 8), solved.at(38, 23));
        assert!(left[1] < -0.05 && right[1] > 0.05, "parting before it: {left:?} {right:?}");
        let back = (50..62).flat_map(|i| (11..21).map(move |j| (i, j))).map(|(i, j)| solved.at(i, j)[0]).fold(f32::MAX, f32::min);
        assert!(back < -0.02, "the water behind it runs back upstream: {back}");
        let below = solved.at(130, 15);
        assert!(below[0] > 0.5 * above[0], "and the current closes again below: {below:?}");
    }

    #[test]
    fn deep_water_runs_faster_than_the_shallows_beside_it() {
        let mut bed = Bed::new(120, 20, 1.0, 0.001, |_, _| false);
        // A channel 3 m deep along one side.
        for &at in bed.members.iter().filter(|&&at| at / bed.side <= 8) {
            bed.depth[at] = 3.0;
        }
        let solved = bed.solve(2.0);
        let (channel, shallows) = (solved.at(60, 5)[0], solved.at(60, 15)[0]);
        assert!(channel > 1.5 * shallows, "{channel} in the channel, {shallows} on the shallows");
    }

    #[test]
    fn a_finer_grid_is_solved_on_every_second_sample_and_read_back_between_them() {
        let bed = Bed::new(120, 20, 2.0, 0.001, |_, _| false);
        let solved = bed.solve(1.0);
        let (on, between) = (solved.at(60, 10)[0], solved.at(61, 11)[0]);
        assert!(on > 0.5 && (between - on).abs() < 0.05, "{on} {between}");
    }

    #[test]
    fn foam_is_made_in_a_rapid_and_trails_away_downstream_not_up() {
        // 40 m wide and 1 m deep, pinched to 12 m for 40 m: only the narrows runs fast enough to whiten.
        let bed = Bed::new(200, 20, 1.0, 0.005, |i, j| (50..70).contains(&i) && !(8..14).contains(&j));
        let solved = bed.solve(2.0);
        let foam = foam(&Channel { side: bed.side, cell: 2.0, members: &bed.members, depth: &bed.depth, fall: &bed.fall, open: &bed.open }, &solved.current.flow);
        let at = |i: usize, j: usize| foam[bed.members.iter().position(|&m| m == j * bed.side + i).unwrap()];
        assert!(solved.at(60, 10)[0] > 1.5 && solved.at(20, 10)[0] < 1.5, "{:?} {:?}", solved.at(60, 10), solved.at(20, 10));
        assert!(at(68, 10) > 0.5, "the narrows is white: {}", at(68, 10));
        assert!(at(40, 10) < 0.01, "the water above it is clean: {}", at(40, 10));
        let trail: Vec<f32> = [80, 100, 130, 180].map(|i| at(i, 10)).to_vec();
        assert!(trail.windows(2).all(|w| w[1] < w[0]) && trail[0] > 0.3 && trail[2] > 0.02 && trail[3] < 0.01, "it thins as it is carried off, and is gone 200 m on: {trail:?}");
    }

    #[test]
    fn foam_spreads_from_the_fast_water_into_the_slack_water_behind_an_island() {
        // 60 m wide and 1 m deep, an island leaving 12 m either side: only the water squeezed past it whitens.
        let bed = Bed::new(140, 30, 1.0, 0.005, |i, j| (40..50).contains(&i) && (7..25).contains(&j));
        let solved = bed.solve(2.0);
        let foam = foam(&Channel { side: bed.side, cell: 2.0, members: &bed.members, depth: &bed.depth, fall: &bed.fall, open: &bed.open }, &solved.current.flow);
        let at = |i: usize, j: usize| foam[bed.members.iter().position(|&m| m == j * bed.side + i).unwrap()];
        assert!(at(20, 15) == 0.0 && at(45, 3) > 0.15, "none above the island, and it is made beside it: {} {}", at(20, 15), at(45, 3));
        // Two trails leave the island's sides; the slack water between them has none at first...
        let (trail, lee) = (at(60, 5), at(53, 15));
        assert!(trail > 0.15 && lee < 0.03, "{trail} in a trail, {lee} between them");
        // ...and takes it up from them further down, as they thin.
        let (later, between) = (at(75, 7), at(75, 15));
        assert!(between > 0.03 && between > 2.0 * lee && later < trail, "{between} between them 50 m down, {later} in the trail");
    }

    /// The foam on `bed` under its own current, and that current.
    fn foamed(bed: &Bed) -> (Vec<f32>, Solved<'_>) {
        let solved = bed.solve(2.0);
        let foam = foam(&Channel { side: bed.side, cell: 2.0, members: &bed.members, depth: &bed.depth, fall: &bed.fall, open: &bed.open }, &solved.current.flow);
        (foam, solved)
    }

    #[test]
    fn the_current_slackens_behind_a_boulder_and_foam_trails_from_it() {
        // 40 m wide and 1.2 m deep: it runs too deep to whiten by itself. A ledge of boulders 8 m
        // across lies in the middle of it, 0.15 m under the surface.
        let mut bed = Bed::new(120, 20, 1.2, 0.008, |_, _| false);
        for (i, j) in (59..=62).flat_map(|i| (9..=12).map(move |j| (i, j))) {
            bed.depth[j * bed.side + i] = 0.15;
        }
        let (foam, solved) = foamed(&bed);
        let at = |i: usize, j: usize| foam[bed.members.iter().position(|&m| m == j * bed.side + i).unwrap()];
        let (beside, over, behind) = (solved.at(60, 4)[0], solved.at(60, 10)[0], solved.at(64, 10)[0]);
        assert!(beside > 1.8 && over > beside && behind < 0.3 * beside, "{beside} m/s beside it, {over} m/s over it, {behind} m/s behind it");
        assert!(at(50, 10) == 0.0 && at(64, 3) < 0.01, "no foam above it or well to the side: {} {}", at(50, 10), at(64, 3));
        let trail = [60, 64, 70, 90].map(|i| at(i, 10));
        assert!(trail[0] > 0.15 && trail.windows(2).all(|w| w[1] < w[0]) && trail[3] > 0.01, "the thin water over it breaks, and the foam trails away below: {trail:?}");
    }

    #[test]
    fn foam_is_made_over_a_shallow_bar_and_not_in_the_deep_tongue_beside_it() {
        // 40 m wide and 2 m deep, with a bar 0.5 m deep across most of it for 10 m, and a gap
        // left at the full depth along one bank.
        let mut bed = Bed::new(140, 20, 2.0, 0.002, |_, _| false);
        for (i, j) in (60..65).flat_map(|i| (1..=14).map(move |j| (i, j))) {
            bed.depth[j * bed.side + i] = 0.5;
        }
        let (foam, solved) = foamed(&bed);
        let at = |i: usize, j: usize| foam[bed.members.iter().position(|&m| m == j * bed.side + i).unwrap()];
        let (over, tongue) = (solved.at(62, 7)[0], solved.at(62, 18)[0]);
        assert!(over > 1.5 && tongue > 1.5, "both run fast: {over} m/s over the bar, {tongue} m/s in the tongue");
        assert!(at(64, 7) > 0.1 && at(68, 7) > 0.05, "the bar is white, and the water below it: {} {}", at(64, 7), at(68, 7));
        assert!(at(62, 18) < 0.01 && at(40, 7) == 0.0, "the tongue is dark, and the water above: {} {}", at(62, 18), at(40, 7));
    }

    #[test]
    fn a_river_without_a_rapid_has_no_foam() {
        let bed = Bed::new(120, 20, 2.0, 0.001, |_, _| false);
        let solved = bed.solve(2.0);
        let foam = foam(&Channel { side: bed.side, cell: 2.0, members: &bed.members, depth: &bed.depth, fall: &bed.fall, open: &bed.open }, &solved.current.flow);
        assert!(foam.iter().all(|&cover| cover == 0.0));
    }

    #[test]
    fn solving_a_river_twice_gives_the_same_current() {
        let bed = round_an_island(100);
        assert_eq!(bed.solve(2.0).current.flow, bed.solve(2.0).current.flow);
    }

    #[test]
    fn the_water_piles_up_before_an_island_and_dips_in_the_fast_water_beside_it() {
        let bed = round_an_island(120);
        let solved = bed.solve(2.0);
        let (before, beside, above) = (solved.lift(38, 16), solved.lift(44, 8), solved.lift(20, 16));
        assert!(before > above + 0.05, "it stands higher against the island than upstream of it: {before} {above}");
        assert!(beside < above - 0.05 && beside > -0.4, "and lower where it runs fast past it: {beside} {above}");
    }

    #[test]
    fn the_water_backs_up_above_a_narrows_and_runs_down_through_it() {
        let bed = Bed::new(120, 20, 2.0, 0.001, |i, j| (50..70).contains(&i) && !(7..15).contains(&j));
        let solved = bed.solve(2.0);
        let (above, within, even) = (solved.lift(44, 10), solved.lift(66, 10), solved.lift(10, 10));
        assert!(above > even + 0.02, "it stands higher above the narrows: {above} {even}");
        assert!(within < above - 0.08, "and falls through it: {within} {above}");
    }

    #[test]
    fn an_even_channels_surface_stays_even() {
        let bed = Bed::new(120, 20, 2.0, 0.001, |_, _| false);
        let solved = bed.solve(2.0);
        // Clear of the ends, where the water gathers speed as it comes in.
        let across: Vec<f32> = (1..=20).map(|j| solved.lift(60, j)).collect();
        let (low, high) = (across.iter().cloned().fold(f32::MAX, f32::min), across.iter().cloned().fold(f32::MIN, f32::max));
        assert!(high - low < 0.01, "level from bank to bank: {across:?}");
        assert!((solved.lift(40, 10) - solved.lift(80, 10)).abs() < 0.03, "and along it");
    }
}
