//! Waves on open water: a few long crossing swells, the same function of place and server tick
//! everywhere. The sim rides vehicles on it (`sim::vehicle::water`), the client's water shader moves
//! its surface with it and its effects splash on it (`client/src/sim/waves.ts` mirrors these numbers,
//! and a test holds the two together).
//!
//! They are Gerstner waves (GERSTNER-SEA.md): the water itself goes round as a wave passes, forwards on
//! a crest and back in a trough, so it gathers under the crests, which stand narrow and sharp, and
//! spreads over the troughs, which lie broad and flat. A point of the water at rest at `p0` stands at
//! `p0 - Σ Q·A·d·sin θ` across and `Σ A·cos θ` up, with `θ = k·d·p0 - ω·t + phase`.
//!
//! The waves loop every `LOOP_TICKS`: each swell runs a whole number of cycles in that time, so the
//! clock can be the tick count modulo the loop and keep full precision however long a server runs.

use crate::consts::{TICK_DT, TICK_RATE};

/// One swell: which way it runs (radians from +X towards +Z), its wavelength (metres), its height
/// (metres, half crest to trough), how far its water swings across against how high it rises (`steep`:
/// 0 for none, 1 for the round orbit of a real wave, more for sharper crests), and how many times it
/// rises and falls in a loop, with its phase.
#[derive(Clone, Copy, Debug)]
pub struct Swell {
    pub heading: f32,
    pub length: f32,
    pub height: f32,
    pub steep: f32,
    pub cycles: u32,
    pub phase: f32,
}

/// A rough sea: two long swells from the south-west, crossing a little and heaving the water a couple
/// of metres crest to trough, and three shorter, quicker ones across them (each running about as fast
/// as a real wave of its length) so the surface is never smooth between the crests. The long ones are
/// about as steep as a wind sea gets before it breaks (`k·A` 0.1), the short ones half that, so a lake
/// has a chop and the sea a heavy swell; their water swings twice as far as a real wave's so the crests
/// stand pointed, and together they swing less than would fold the surface over (`Σ Q·k·A` 0.6).
pub const SWELLS: [Swell; 5] = [
    Swell { heading: 0.3, length: 70.0, height: 1.1, steep: 2.0, cycles: 8, phase: 0.0 },
    Swell { heading: -0.2, length: 45.0, height: 0.75, steep: 2.0, cycles: 10, phase: 1.7 },
    Swell { heading: 0.7, length: 20.0, height: 0.16, steep: 1.8, cycles: 17, phase: 4.1 },
    Swell { heading: -0.9, length: 10.0, height: 0.07, steep: 1.5, cycles: 24, phase: 2.6 },
    Swell { heading: 1.6, length: 6.0, height: 0.035, steep: 1.2, cycles: 31, phase: 0.9 },
];

/// The waves die away towards the shore: none shallower than the first depth, metres, all of them deeper than the second.
pub const SHORE_DEPTHS: [f32; 2] = [0.3, 2.5];
/// How wide open water must be for a swell to rise on it, in its wavelengths: none on water narrower
/// than the first, the whole swell on water wider than the second. Waves grow with the open water the
/// wind has to raise them over, so a pond has only the chop, a lake the shorter swells and the sea all of them.
pub const FETCH: [f32; 2] = [2.0, 8.0];
/// Open water this wide, metres, carries every swell: the sea, and what a world stores for it.
pub const OPEN_SEA: f32 = 1020.0;
/// The loop, seconds and ticks.
pub const LOOP_SECONDS: u32 = 64;
pub const LOOP_TICKS: u32 = LOOP_SECONDS * TICK_RATE;
/// How many Newton steps find the water that has come to stand over a point; three put it within a
/// micrometre on the roughest sea.
const SOLVE_STEPS: usize = 3;

/// The surface over a point: how far above the water's level it is (metres), its slope `[dh/dx, dh/dz]`,
/// how fast it is rising there (m/s), and which way and how fast the water there is swinging across
/// (`[x, z]` m/s: forwards under a crest, back in a trough).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    pub height: f32,
    pub slope: [f32; 2],
    pub rise: f32,
    pub orbit: [f32; 2],
}

/// Where the water at rest at a point has gone: across (`[x, z]` metres) and up (metres above the
/// level), and how much the surface about it is squeezed (the area it covers against its area at rest:
/// 1 on still water, under 1 gathered into a crest, over 1 spread over a trough). The renderer moves
/// its vertices so, and lays foam where the water is most squeezed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    pub shift: [f32; 2],
    pub height: f32,
    pub squeeze: f32,
}

/// The wave clock at `tick`, seconds into the loop.
pub fn time(tick: u32) -> f32 {
    (tick % LOOP_TICKS) as f32 * TICK_DT
}

/// How much of the waves water `depth` metres deep has.
pub fn shore(depth: f32) -> f32 {
    let t = ((depth - SHORE_DEPTHS[0]) / (SHORE_DEPTHS[1] - SHORE_DEPTHS[0])).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of a swell `length` metres long rises on open water `reach` metres across.
pub fn fetch(reach: f32, length: f32) -> f32 {
    let t = ((reach - FETCH[0] * length) / ((FETCH[1] - FETCH[0]) * length)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One swell as it stands on a stretch of water: its heading `[x, z]`, wavenumber (radians a metre),
/// rate (radians a second), phase, height and how far its water swings across (metres).
struct Term {
    d: [f32; 2],
    k: f32,
    omega: f32,
    phase: f32,
    height: f32,
    swing: f32,
}

/// The swells that rise on open water `reach` metres across with `share` of their height.
fn terms(reach: f32, share: f32) -> impl Iterator<Item = Term> {
    SWELLS.iter().filter_map(move |swell| {
        let height = swell.height * fetch(reach, swell.length) * share;
        if height <= 0.0 {
            return None;
        }
        let (dz, dx) = swell.heading.sin_cos();
        Some(Term {
            d: [dx, dz],
            k: std::f32::consts::TAU / swell.length,
            omega: std::f32::consts::TAU * swell.cycles as f32 / LOOP_SECONDS as f32,
            phase: swell.phase,
            height,
            swing: swell.steep * height,
        })
    })
}

/// How far the water's level stands above the middle of its waves, metres. A Gerstner surface spends
/// more of its width in its broad troughs than on its narrow crests, so over a point its height falls
/// by `Q·k·A²/2` a swell on average; this lifts it back so the water keeps its level.
fn level(reach: f32, share: f32) -> f32 {
    terms(reach, share).map(|term| 0.5 * term.swing * term.k * term.height).sum()
}

/// Everything the waves do to the water at rest at `p0`, `time` seconds into the loop.
#[derive(Default)]
struct Sums {
    /// How far it has gone across, metres.
    shift: [f32; 2],
    /// How the place it has gone to changes with where it rests: `∂p/∂p0`, symmetric, as `[xx, xz, zz]`.
    stretch: [f32; 3],
    /// How high it stands, metres, without the level.
    height: f32,
    /// How its height changes with where it rests, `∂y/∂p0`.
    grad: [f32; 2],
    /// How fast it rises, m/s.
    lift: f32,
    /// How fast it swings across, `[x, z]` m/s.
    orbit: [f32; 2],
}

fn sums(p0: [f32; 2], time: f32, reach: f32, share: f32) -> Sums {
    let mut out = Sums { stretch: [1.0, 0.0, 1.0], ..Sums::default() };
    for term in terms(reach, share) {
        let theta = term.k * (term.d[0] * p0[0] + term.d[1] * p0[1]) - term.omega * time + term.phase;
        let (sin, cos) = theta.sin_cos();
        let [dx, dz] = term.d;
        out.shift[0] -= term.swing * sin * dx;
        out.shift[1] -= term.swing * sin * dz;
        let gather = term.swing * term.k * cos;
        out.stretch[0] -= gather * dx * dx;
        out.stretch[1] -= gather * dx * dz;
        out.stretch[2] -= gather * dz * dz;
        out.height += term.height * cos;
        out.grad[0] -= term.height * term.k * sin * dx;
        out.grad[1] -= term.height * term.k * sin * dz;
        out.lift += term.height * term.omega * sin;
        out.orbit[0] += term.swing * term.omega * cos * dx;
        out.orbit[1] += term.swing * term.omega * cos * dz;
    }
    out
}

/// `stretch⁻¹ · v` for the symmetric `stretch`.
fn unstretch(stretch: [f32; 3], v: [f32; 2]) -> [f32; 2] {
    let [xx, xz, zz] = stretch;
    let det = xx * zz - xz * xz;
    [(zz * v[0] - xz * v[1]) / det, (xx * v[1] - xz * v[0]) / det]
}

/// Where the water at rest at `(x0, z0)`, metres, has gone `time` seconds into the loop, on open water
/// `reach` metres across with `share` of the waves (see [`shore`]): how the renderer moves the surface.
pub fn particle(x0: f32, z0: f32, time: f32, reach: f32, share: f32) -> Particle {
    let at = sums([x0, z0], time, reach, share);
    let [xx, xz, zz] = at.stretch;
    Particle { shift: at.shift, height: at.height + level(reach, share), squeeze: xx * zz - xz * xz }
}

/// The waves over `(x, z)`, metres, `time` seconds into the loop, on open water `reach` metres across
/// (each swell as much as [`fetch`] lets rise) with `share` of them (see [`shore`]). The water standing
/// over the point has come there from elsewhere, so it is found first, by Newton's method.
pub fn wave(x: f32, z: f32, time: f32, reach: f32, share: f32) -> Wave {
    if share <= 0.0 {
        return Wave::default();
    }
    let mut p0 = [x, z];
    let mut at = sums(p0, time, reach, share);
    for _ in 0..SOLVE_STEPS {
        let miss = [p0[0] + at.shift[0] - x, p0[1] + at.shift[1] - z];
        let step = unstretch(at.stretch, miss);
        p0 = [p0[0] - step[0], p0[1] - step[1]];
        at = sums(p0, time, reach, share);
    }
    // Over the fixed point: the slope is the height's change with where the water rests, carried
    // through how that place moves; the surface rises as the water there rises, less what the water
    // swinging across carries away under the slope.
    let slope = unstretch(at.stretch, at.grad);
    Wave {
        height: at.height + level(reach, share),
        slope,
        rise: at.lift - slope[0] * at.orbit[0] - slope[1] * at.orbit[1],
        orbit: at.orbit,
    }
}

/// White water: where a river runs fast its surface stands in waves, crests across the current. The
/// waves themselves are part of the river's level (the importer shapes it, RIVER-SURFACE.md) and stay
/// where they are; this is how they surge, rising and falling in place. The surge is this high
/// (metres, half crest to trough) where the current runs at the second speed, m/s, and there is none
/// below the first, which is also where the standing waves start.
pub const CHURN_HEIGHT: f32 = 0.1;
pub const CHURN_SPEEDS: [f32; 2] = [1.2, 2.6];
/// The surge's waves: four that lie a quarter of a half-turn apart, each rising and falling `cycles`
/// times a loop. Water takes each by how nearly it runs along that one's heading (`churn`), so the
/// crests lie across the current wherever it turns, without a seam. `height` and `steep` are not used.
const CHURN: [Swell; 4] = [
    Swell { heading: 0.2, length: 4.6, height: 1.0, steep: 0.0, cycles: 41, phase: 0.0 },
    Swell { heading: 0.2 + std::f32::consts::FRAC_PI_4, length: 5.3, height: 1.0, steep: 0.0, cycles: 37, phase: 1.3 },
    Swell { heading: 0.2 + std::f32::consts::FRAC_PI_2, length: 4.9, height: 1.0, steep: 0.0, cycles: 43, phase: 2.9 },
    Swell { heading: 0.2 + 3.0 * std::f32::consts::FRAC_PI_4, length: 5.6, height: 1.0, steep: 0.0, cycles: 35, phase: 4.4 },
];
/// What the four waves' shares (the fourth power of the cosine of the angle to each) add up to,
/// whichever way the water runs.
const CHURN_SHARES: f32 = 1.5;

/// The surge of a rapid's standing waves at `(x, z)`, `time` seconds into the loop, on water running
/// at `flow` (`[x, z]` m/s): nothing on a calm river, a heave across the current in a rapid. A hull
/// or a swimmer rides it, and it lights the water the client draws. The slope takes the current as
/// the same all round the point. Standing waves heave in place, so their water does not swing across.
pub fn churn(x: f32, z: f32, time: f32, flow: [f32; 2]) -> Wave {
    let speed = flow[0].hypot(flow[1]);
    let t = ((speed - CHURN_SPEEDS[0]) / (CHURN_SPEEDS[1] - CHURN_SPEEDS[0])).clamp(0.0, 1.0);
    let mut out = Wave::default();
    if t <= 0.0 {
        return out;
    }
    let size = CHURN_HEIGHT * t * t * (3.0 - 2.0 * t) / CHURN_SHARES;
    for swell in &CHURN {
        let (dz, dx) = swell.heading.sin_cos();
        let along = (dx * flow[0] + dz * flow[1]) / speed;
        let height = size * along * along * along * along;
        let k = std::f32::consts::TAU / swell.length;
        let omega = std::f32::consts::TAU * swell.cycles as f32 / LOOP_SECONDS as f32;
        let (sin_a, cos_a) = (k * (dx * x + dz * z) + swell.phase).sin_cos();
        let (sin_b, cos_b) = (omega * time).sin_cos();
        out.height += height * sin_a * cos_b;
        out.slope[0] += height * k * dx * cos_a * cos_b;
        out.slope[1] += height * k * dz * cos_a * cos_b;
        out.rise -= height * omega * sin_a * sin_b;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rapids_waves_surge_in_place_across_the_current_only_where_the_river_runs_fast() {
        assert_eq!(churn(3.0, 4.0, 10.0, [1.0, 0.0]), Wave::default(), "a calm river is smooth");
        let (x, z, t, e, fast) = (3.7, -12.2, 21.4, 1e-3, [2.4, 3.2]);
        let at = churn(x, z, t, fast);
        let dx = (churn(x + e, z, t, fast).height - churn(x - e, z, t, fast).height) / (2.0 * e);
        let dz = (churn(x, z + e, t, fast).height - churn(x, z - e, t, fast).height) / (2.0 * e);
        let dt = (churn(x, z, t + e, fast).height - churn(x, z, t - e, fast).height) / (2.0 * e);
        assert!((at.slope[0] - dx).abs() < 0.02 && (at.slope[1] - dz).abs() < 0.02 && (at.rise - dt).abs() < 0.02, "{at:?} against {dx} {dz} {dt}");
        assert_eq!(churn(x, z, time(LOOP_TICKS + 30), fast), churn(x, z, time(30), fast), "it loops with the swell");
        let highest = (0..LOOP_TICKS).step_by(3).map(|tick| churn(x, z, time(tick), fast).height.abs()).fold(0.0, f32::max);
        assert!(highest > 0.02 && highest <= CHURN_HEIGHT + 1e-4, "{highest}");
        // The surface slopes along the current far more than across it, whichever way the river runs:
        // the crests lie across.
        for heading in [0.0f32, 0.6, 1.3, 2.2, 2.9] {
            let (dz, dx) = heading.sin_cos();
            let (mut along, mut across) = (0.0, 0.0);
            for k in 0..400 {
                let wave = churn(k as f32 * 1.3, k as f32 * -0.7, k as f32 * 0.37, [4.0 * dx, 4.0 * dz]);
                along += (wave.slope[0] * dx + wave.slope[1] * dz).powi(2);
                across += (wave.slope[1] * dx - wave.slope[0] * dz).powi(2);
            }
            assert!(along > 3.0 * across, "running at {heading} rad: {along} along, {across} across");
        }
    }

    #[test]
    fn the_slope_and_rise_are_the_heights_derivatives_over_a_fixed_point() {
        let e = 2e-3;
        let sea = |x: f32, z: f32, t: f32| wave(x, z, t, OPEN_SEA, 1.0);
        for (x, z, t) in [(3.7, -12.2, 21.4), (-41.0, 77.5, 3.3), (250.0, 18.0, 50.9)] {
            let at = sea(x, z, t);
            let dx = (sea(x + e, z, t).height - sea(x - e, z, t).height) / (2.0 * e);
            let dz = (sea(x, z + e, t).height - sea(x, z - e, t).height) / (2.0 * e);
            let dt = (sea(x, z, t + e).height - sea(x, z, t - e).height) / (2.0 * e);
            assert!((at.slope[0] - dx).abs() < 0.01 && (at.slope[1] - dz).abs() < 0.01 && (at.rise - dt).abs() < 0.02, "{at:?} against {dx} {dz} {dt}");
        }
    }

    #[test]
    fn the_water_standing_over_a_point_is_found_and_its_height_is_what_the_renderer_draws() {
        for (x, z, t) in [(3.7, -12.2, 21.4), (-41.0, 77.5, 3.3), (250.0, 18.0, 50.9), (0.0, 0.0, 0.0)] {
            let at = wave(x, z, t, OPEN_SEA, 1.0);
            // Move the water back from where it is to where it rested, by the same Newton steps run by
            // hand, then forwards as the renderer does: it lands back on the point at the same height.
            let mut p0 = [x, z];
            for _ in 0..8 {
                let moved = particle(p0[0], p0[1], t, OPEN_SEA, 1.0);
                p0 = [x - moved.shift[0], z - moved.shift[1]];
            }
            let drawn = particle(p0[0], p0[1], t, OPEN_SEA, 1.0);
            assert!((p0[0] + drawn.shift[0] - x).abs() < 1e-3 && (p0[1] + drawn.shift[1] - z).abs() < 1e-3, "{p0:?} {drawn:?}");
            assert!((drawn.height - at.height).abs() < 1e-3, "drawn at {} m, ridden at {} m", drawn.height, at.height);
        }
    }

    #[test]
    fn the_crests_stand_sharp_over_flat_troughs_and_the_surface_never_folds() {
        let (mut least, mut highest, mut lowest) = (f32::MAX, f32::MIN, f32::MAX);
        for k in 0..2000 {
            let (x, z, t) = (k as f32 * 1.3, k as f32 * 0.7, k as f32 * 0.037);
            least = least.min(particle(x, z, t, OPEN_SEA, 1.0).squeeze);
            let at = wave(x, z, t, OPEN_SEA, 1.0);
            highest = highest.max(at.height);
            lowest = lowest.min(at.height);
        }
        assert!(least > 0.2, "the surface squeezes to {least} of itself at a crest");
        assert!(least < 0.7, "the crests gather the water: at most {least}");
        assert!(highest > 1.5 && highest < 3.0, "crests stand {highest} m over the level");
        assert!(highest > -1.1 * lowest, "the crests stand higher than the troughs sink: {highest} over {lowest}");
    }

    #[test]
    fn the_water_rolls_forward_under_a_crest_and_back_in_a_trough() {
        // One swell alone, on water just wide enough for it and no wider ones: along x at the 90 m swell's heading.
        let swell = SWELLS[0];
        let (dz, dx) = swell.heading.sin_cos();
        let (mut crest, mut trough) = ((f32::MIN, [0.0; 2]), (f32::MAX, [0.0; 2]));
        for k in 0..180 {
            let s = k as f32 * 0.5;
            let at = wave(s * dx, s * dz, 10.0, OPEN_SEA, 1.0);
            if at.height > crest.0 {
                crest = (at.height, at.orbit);
            }
            if at.height < trough.0 {
                trough = (at.height, at.orbit);
            }
        }
        let along = |v: [f32; 2]| v[0] * dx + v[1] * dz;
        assert!(along(crest.1) > 0.5, "forwards on the crest: {:?}", crest.1);
        assert!(along(trough.1) < -0.3, "back in the trough: {:?}", trough.1);
    }

    #[test]
    fn the_waves_loop_and_keep_the_waters_level() {
        let sea = |t: f32| wave(5.0, 9.0, t, OPEN_SEA, 1.0);
        assert_eq!(sea(time(LOOP_TICKS + 30)), sea(time(30)));
        assert!((sea(0.0).height - sea(LOOP_SECONDS as f32).height).abs() < 1e-3, "every swell runs whole cycles in a loop");
        let mean = (0..LOOP_TICKS).map(|tick| sea(time(tick)).height).sum::<f32>() / LOOP_TICKS as f32;
        assert!(mean.abs() < 0.02, "the water keeps its level, not {mean}");
        assert_eq!(shore(0.1), 0.0);
        assert_eq!(shore(3.0), 1.0);
        assert_eq!(wave(5.0, 9.0, 3.0, OPEN_SEA, 0.0), Wave::default(), "none at the shore");
    }

    #[test]
    fn small_water_has_only_the_chop() {
        let crest = |reach: f32| (0..LOOP_TICKS).step_by(7).map(|tick| wave(5.0, 9.0, time(tick), reach, 1.0).height).fold(0.0, f32::max);
        let (pond, lake, big, sea) = (crest(20.0), crest(200.0), crest(800.0), crest(OPEN_SEA));
        assert!(pond < 0.05, "a pond has ripples, not {pond} m crests");
        assert!(pond < lake && lake < big, "crests grow with the water: {pond} {lake} {big}");
        assert!(lake > 0.2 && lake < 0.8, "a 200 m lake has a chop of {lake} m");
        assert_eq!(big, sea, "most of a kilometre of water carries the whole swell");
        assert_eq!(fetch(5.0, 7.5), 0.0);
        assert_eq!(fetch(OPEN_SEA, SWELLS[0].length), 1.0);
    }
}
