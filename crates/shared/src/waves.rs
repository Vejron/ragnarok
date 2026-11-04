//! Waves on open water: a few long crossing swells with peaked crests, the same function of place and
//! server tick everywhere. The sim rides vehicles on it (`sim::vehicle::water`), the client's water
//! shader moves its surface with it and its effects splash on it (`client/src/sim/waves.ts` mirrors
//! these numbers, and a test holds the two together).
//!
//! The waves loop every `LOOP_TICKS`: each swell runs a whole number of cycles in that time, so the
//! clock can be the tick count modulo the loop and keep full precision however long a server runs.

use crate::consts::{TICK_DT, TICK_RATE};

/// One swell: which way it runs (radians from +X towards +Z), its wavelength (metres), its height
/// (metres, half crest to trough) and how many times it rises and falls in a loop, with its phase.
#[derive(Clone, Copy, Debug)]
pub struct Swell {
    pub heading: f32,
    pub length: f32,
    pub height: f32,
    pub cycles: u32,
    pub phase: f32,
}

/// A big, lazy swell: two long ones from the south-west, crossing a little and heaving the water
/// slowly, with a faint shorter pair on them so it is never quite glassy.
pub const SWELLS: [Swell; 4] = [
    Swell { heading: 0.3, length: 90.0, height: 0.4, cycles: 7, phase: 0.0 },
    Swell { heading: -0.2, length: 60.0, height: 0.25, cycles: 8, phase: 1.7 },
    Swell { heading: 0.7, length: 25.0, height: 0.04, cycles: 12, phase: 4.1 },
    Swell { heading: -0.9, length: 13.0, height: 0.012, cycles: 17, phase: 2.6 },
];

/// How peaked the crests are: a swell's shape is `((1 + cos θ) / 2)^PEAK`, narrow crests over broad,
/// flat troughs. The crests fall away fast enough that something quick over them leaves the water.
pub const PEAK: i32 = 8;
/// That shape's mean over a cycle, `C(2·PEAK, PEAK) / 4^PEAK`, taken off so the water keeps its level.
const PEAK_MEAN: f32 = 12870.0 / 65536.0;
/// The waves die away towards the shore: none shallower than the first depth, metres, all of them deeper than the second.
pub const SHORE_DEPTHS: [f32; 2] = [0.3, 2.5];
/// The loop, seconds and ticks.
pub const LOOP_SECONDS: u32 = 64;
pub const LOOP_TICKS: u32 = LOOP_SECONDS * TICK_RATE;

/// The surface at a point: how far above the water's level it is (metres), its slope `[dh/dx, dh/dz]`,
/// and how fast it is rising there (m/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    pub height: f32,
    pub slope: [f32; 2],
    pub rise: f32,
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

/// The waves at `(x, z)`, metres, `time` seconds into the loop, at full height (scale by [`shore`]).
pub fn wave(x: f32, z: f32, time: f32) -> Wave {
    let mut out = Wave::default();
    for swell in &SWELLS {
        let k = std::f32::consts::TAU / swell.length;
        let omega = std::f32::consts::TAU * swell.cycles as f32 / LOOP_SECONDS as f32;
        let (dz, dx) = swell.heading.sin_cos();
        let theta = k * (dx * x + dz * z) - omega * time + swell.phase;
        let (sin, cos) = theta.sin_cos();
        let lift = (0.5 * (1.0 + cos)).max(0.0);
        let lift_less = lift.powi(PEAK - 1);
        out.height += 2.0 * swell.height * (lift_less * lift - PEAK_MEAN);
        // d/dθ of the shape; θ grows with place by k along the heading and falls with time by ω.
        let turn = -swell.height * PEAK as f32 * lift_less * sin;
        out.slope[0] += turn * k * dx;
        out.slope[1] += turn * k * dz;
        out.rise -= turn * omega;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slope_and_rise_are_the_heights_derivatives() {
        let (x, z, t, e) = (3.7, -12.2, 21.4, 1e-3);
        let at = wave(x, z, t);
        let dx = (wave(x + e, z, t).height - wave(x - e, z, t).height) / (2.0 * e);
        let dz = (wave(x, z + e, t).height - wave(x, z - e, t).height) / (2.0 * e);
        let dt = (wave(x, z, t + e).height - wave(x, z, t - e).height) / (2.0 * e);
        assert!((at.slope[0] - dx).abs() < 0.02 && (at.slope[1] - dz).abs() < 0.02 && (at.rise - dt).abs() < 0.02, "{at:?} against {dx} {dz} {dt}");
    }

    #[test]
    fn the_waves_loop_and_are_a_gentle_swell() {
        assert_eq!(wave(5.0, 9.0, time(LOOP_TICKS + 30)), wave(5.0, 9.0, time(30)));
        let (start, end) = (wave(5.0, 9.0, 0.0), wave(5.0, 9.0, LOOP_SECONDS as f32));
        assert!((start.height - end.height).abs() < 1e-3, "every swell runs whole cycles in a loop");
        let crest = SWELLS.iter().map(|swell| swell.height).sum::<f32>() * 2.0 * (1.0 - PEAK_MEAN);
        assert!(crest > 0.8 && crest < 1.3, "crests stand {crest} m over the level");
        let mean = (0..LOOP_TICKS).map(|tick| wave(5.0, 9.0, time(tick)).height).sum::<f32>() / LOOP_TICKS as f32;
        assert!(mean.abs() < 1e-3, "the water keeps its level, not {mean}");
        // Big but calm: nowhere does the surface slope more than about 1 in 7.
        let steepest = (0..400).map(|k| wave(k as f32 * 1.3, k as f32 * 0.7, k as f32 * 0.37)).map(|w| w.slope[0].hypot(w.slope[1])).fold(0.0, f32::max);
        assert!(steepest < 0.15, "slopes reach {steepest}");
        assert_eq!(shore(0.1), 0.0);
        assert_eq!(shore(3.0), 1.0);
    }
}
