//! What the importer is doing right now, for the map editor: the stage, how far through it, and how
//! fast bytes are arriving. Every download and every cache hit is counted here (`cache` reports
//! them), and the importers name their stage and count their steps.
//!
//! The editor reads `snapshot` for a download it runs itself. A generate runs `osm-import` as a
//! child process, which prints a `@progress {json}` line to stderr whenever the state changes (with
//! `events`, the `--progress` flag). Lines come at most every quarter second, except stage changes.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Prefix of the stderr lines that carry progress, so a reader can tell them from log text.
pub const PREFIX: &str = "@progress ";

/// How far back the rate looks: long enough to smooth over one 512 KiB block, short enough that a
/// stall shows within seconds.
const WINDOW: Duration = Duration::from_secs(5);

/// The shortest time between two printed lines.
const EVERY: Duration = Duration::from_millis(250);

/// The state at one moment. `bytes` counts what came over the network, `cached` what came from
/// the cache, both since the last `reset` (or since start).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub stage: String,
    /// Steps of the stage finished, and how many it has (0 when the stage does not count).
    pub step: usize,
    pub steps: usize,
    pub bytes: u64,
    pub bytes_per_second: f64,
    pub cached: u64,
    pub cached_per_second: f64,
}

struct State {
    stage: String,
    done: usize,
    total: usize,
    /// `(when, bytes)` for each rate, oldest first. Two are always kept, so a rate can be read.
    samples: VecDeque<(Instant, u64)>,
    cached_samples: VecDeque<(Instant, u64)>,
    last_print: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    stage: String::new(),
    done: 0,
    total: 0,
    samples: VecDeque::new(),
    cached_samples: VecDeque::new(),
    last_print: None,
});
static EVENTS: AtomicBool = AtomicBool::new(false);
static BYTES: AtomicU64 = AtomicU64::new(0);
static CACHED: AtomicU64 = AtomicU64::new(0);

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Print a `@progress` line whenever the state changes, and every half second while it does not,
/// so the rate falls to zero in a stall. Off by default, so a plain run is quiet.
pub fn events() {
    EVENTS.store(true, Ordering::Relaxed);
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_millis(500));
            emit(&mut state(), false);
        }
    });
}

/// Starts counting from zero, for a new job in the same process.
pub fn reset() {
    BYTES.store(0, Ordering::Relaxed);
    CACHED.store(0, Ordering::Relaxed);
    let mut state = state();
    state.samples.clear();
    state.cached_samples.clear();
    state.done = 0;
    state.total = 0;
}

/// Names the stage now running (`water`, `laser`, ...) and clears its step count.
pub fn stage(name: &str) {
    let mut state = state();
    state.done = 0;
    state.total = 0;
    name.clone_into(&mut state.stage);
    emit(&mut state, true);
}

/// Waits `seconds` before a retry, with the stage saying so meanwhile. The stage and its steps
/// are kept as they were.
pub fn retry(seconds: u64) {
    let current = {
        let mut state = state();
        let current = state.stage.clone();
        state.stage = format!("{current}, retrying in {seconds} s");
        emit(&mut state, true);
        current
    };
    std::thread::sleep(Duration::from_secs(seconds));
    let mut state = state();
    state.stage = current;
    emit(&mut state, true);
}

/// `done` of `total` steps of the current stage are finished.
pub fn steps(done: usize, total: usize) {
    let mut state = state();
    state.done = done;
    state.total = total;
    emit(&mut state, done == total);
}

/// `bytes` came over the network.
pub fn received(bytes: usize) {
    BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    emit(&mut state(), false);
}

/// `bytes` were read from the cache instead.
pub fn reused(bytes: usize) {
    CACHED.fetch_add(bytes as u64, Ordering::Relaxed);
    emit(&mut state(), false);
}

/// The state now, for the editor to show. Reading it also moves the rate's window forward.
pub fn snapshot() -> Snapshot {
    view(&mut state(), Instant::now())
}

fn view(state: &mut State, now: Instant) -> Snapshot {
    let (bytes, cached) = (BYTES.load(Ordering::Relaxed), CACHED.load(Ordering::Relaxed));
    record(&mut state.samples, now, bytes);
    record(&mut state.cached_samples, now, cached);
    Snapshot {
        stage: state.stage.clone(),
        step: state.done,
        steps: state.total,
        bytes,
        bytes_per_second: rate(&state.samples),
        cached,
        cached_per_second: rate(&state.cached_samples),
    }
}

/// Adds a sample and drops the ones older than the window, keeping two.
fn record(samples: &mut VecDeque<(Instant, u64)>, now: Instant, bytes: u64) {
    samples.push_back((now, bytes));
    while samples.len() > 2 && samples.front().is_some_and(|(when, _)| now.duration_since(*when) > WINDOW) {
        samples.pop_front();
    }
}

/// Bytes per second between the oldest and the newest sample.
fn rate(samples: &VecDeque<(Instant, u64)>) -> f64 {
    let (Some(&(first_when, first_bytes)), Some(&(last_when, last_bytes))) = (samples.front(), samples.back()) else {
        return 0.0;
    };
    let seconds = last_when.duration_since(first_when).as_secs_f64();
    if seconds <= 0.0 {
        return 0.0;
    }
    (last_bytes - first_bytes) as f64 / seconds
}

/// Prints the state, unless one was printed less than `EVERY` ago (and `force` is not set).
fn emit(state: &mut State, force: bool) {
    if !EVENTS.load(Ordering::Relaxed) {
        return;
    }
    let now = Instant::now();
    if !force && state.last_print.is_some_and(|printed| now.duration_since(printed) < EVERY) {
        return;
    }
    state.last_print = Some(now);
    let snapshot = view(state, now);
    if let Ok(line) = serde_json::to_string(&snapshot) {
        eprintln!("{PREFIX}{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples at the given seconds after `origin`, for the rate alone (the counters are not involved).
    fn with_samples(samples: &[(f64, u64)], origin: Instant) -> VecDeque<(Instant, u64)> {
        samples.iter().map(|&(s, b)| (origin + Duration::from_secs_f64(s), b)).collect()
    }

    #[test]
    fn rate_is_bytes_over_the_seconds_between_the_ends() {
        let origin = Instant::now();
        let samples = with_samples(&[(0.0, 0), (2.0, 4_000_000)], origin);
        assert!((rate(&samples) - 2_000_000.0).abs() < 1.0);
    }

    #[test]
    fn a_stall_reads_as_zero() {
        let origin = Instant::now();
        assert_eq!(rate(&with_samples(&[(0.0, 1_000), (3.0, 1_000)], origin)), 0.0);
        assert_eq!(rate(&with_samples(&[], origin)), 0.0);
    }

    #[test]
    fn the_newest_sample_is_kept_even_when_old() {
        let origin = Instant::now();
        let mut samples = with_samples(&[(0.0, 0), (1.0, 500)], origin);
        // Ten seconds on with no traffic: both old samples are past the window, two stay.
        record(&mut samples, origin + Duration::from_secs(10), 500);
        assert_eq!(samples.len(), 2);
        assert_eq!(rate(&samples), 0.0);
    }
}
