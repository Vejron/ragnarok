//! The wind over open water: the same function of server tick everywhere. It blows the way the long
//! swells run (`crate::waves`), veering slowly either side of that, and fills a ship's sail (the sim's
//! `vehicle::sail`). `client/src/sim/wind.ts` mirrors it, and a test holds the two together: the
//! client draws the sail and the wind's gauge from it, and predicts its own ship with the sim's.
//!
//! Like the waves it loops, over `LOOP_TICKS`, so the clock keeps full precision however long a
//! server runs.

use crate::consts::{TICK_DT, TICK_RATE};

/// Which way the wind blows towards, radians from +X towards +Z: with the main swell.
pub const HEADING: f32 = 0.3;
/// How far it veers either side of that, radians, and how long it takes to swing there and back, seconds.
pub const VEER: f32 = 0.6;
pub const LOOP_SECONDS: u32 = 512;
pub const LOOP_TICKS: u32 = LOOP_SECONDS * TICK_RATE;
/// Within this angle of where the wind comes from (radians, about 40°) a sail only flaps: a ship in
/// irons has `IRONS` of its drive. It has all of it from `BEAM`, a beam reach, and `RUN` of it dead
/// before the wind.
pub const CLOSE: f32 = 0.7;
pub const BEAM: f32 = 1.6;
pub const IRONS: f32 = 0.08;
pub const RUN: f32 = 0.7;
/// The furthest a boom swings out over the side, radians.
const BOOM_MAX: f32 = 1.4;

/// Which way the wind blows at `tick`, a unit `[x, z]`.
pub fn wind(tick: u32) -> [f32; 2] {
    let t = (tick % LOOP_TICKS) as f32 * TICK_DT;
    let heading = HEADING + VEER * (std::f32::consts::TAU * t / LOOP_SECONDS as f32).sin();
    let (sin, cos) = heading.sin_cos();
    [cos, sin]
}

/// How far off the wind a ship heading along `forward` (unit `[x, z]`) sails, radians: 0 straight into
/// it, π dead before it. Positive with the wind coming over the left (port) side,
/// as a yaw is (+ turns left).
pub fn off_wind(forward: [f32; 2], wind: [f32; 2]) -> f32 {
    // Where it comes from, against the heading; the cross product says which side it is on.
    let from = [-wind[0], -wind[1]];
    let cos = forward[0] * from[0] + forward[1] * from[1];
    let cross = forward[0] * from[1] - forward[1] * from[0];
    cross.atan2(cos)
}

/// The share of a full sail's drive a ship heading along `forward` gets from `wind`.
pub fn point_of_sail(forward: [f32; 2], wind: [f32; 2]) -> f32 {
    let off = off_wind(forward, wind).abs();
    if off <= CLOSE {
        IRONS
    } else if off < BEAM {
        let t = (off - CLOSE) / (BEAM - CLOSE);
        IRONS + (1.0 - IRONS) * t * t * (3.0 - 2.0 * t)
    } else {
        let t = (off - BEAM) / (std::f32::consts::PI - BEAM);
        1.0 - (1.0 - RUN) * t.powf(1.5)
    }
}

/// How far the boom swings off the centre line, radians, in the soldier convention (+ turns it
/// towards the left, port, side): let out on the side away from the wind, about half the angle off it.
pub fn boom(forward: [f32; 2], wind: [f32; 2]) -> f32 {
    let off = off_wind(forward, wind);
    // The wind over the left side blows it out to the right.
    (off * 0.5).clamp(-BOOM_MAX, BOOM_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heading(angle: f32) -> [f32; 2] {
        let (sin, cos) = angle.sin_cos();
        [cos, sin]
    }

    #[test]
    fn the_wind_loops_and_veers_about_the_swell() {
        assert_eq!(wind(LOOP_TICKS + 77), wind(77));
        let mut widest = 0.0f32;
        for tick in (0..LOOP_TICKS).step_by(60) {
            let [x, z] = wind(tick);
            assert!(((x * x + z * z) - 1.0).abs() < 1e-5);
            widest = widest.max((z.atan2(x) - HEADING).abs());
        }
        assert!((widest - VEER).abs() < 0.01, "it veers {widest}");
        assert_eq!(LOOP_TICKS % crate::waves::LOOP_TICKS, 0, "a whole number of the waves' loops");
    }

    #[test]
    fn a_beam_reach_is_fastest_and_into_the_wind_a_sail_only_flaps() {
        let wind = heading(0.3);
        let into = point_of_sail(heading(0.3 + std::f32::consts::PI), wind);
        let beam = point_of_sail(heading(0.3 + std::f32::consts::FRAC_PI_2), wind);
        let broad = point_of_sail(heading(0.3 + 0.8), wind);
        let run = point_of_sail(heading(0.3), wind);
        assert!((into - IRONS).abs() < 1e-5, "in irons {into}");
        assert!(beam > 0.97 && broad > 0.85 && broad < beam && (run - RUN).abs() < 1e-4, "beam {beam}, broad {broad}, run {run}");
        // Either tack is the same.
        for off in [0.5f32, 1.0, 1.6, 2.4, 3.0] {
            let left = point_of_sail(heading(0.3 + std::f32::consts::PI + off), wind);
            let right = point_of_sail(heading(0.3 + std::f32::consts::PI - off), wind);
            assert!((left - right).abs() < 1e-4, "{off}: {left} against {right}");
        }
    }

    #[test]
    fn the_boom_goes_out_away_from_the_wind() {
        let wind = [1.0, 0.0];
        // Heading -Z with the wind blowing towards +X: it comes from -X, the left, so the boom swings right.
        assert!(boom([0.0, -1.0], wind) < -0.5);
        assert!(boom([0.0, 1.0], wind) > 0.5);
        assert!(boom([1.0, 0.0], wind).abs() > 1.3, "dead before it, it is right out");
    }
}
