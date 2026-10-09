//! Debug network emulation: delays, jitters and drops datagrams in both directions so netcode can
//! be exercised on localhost. Configured by `RAGNAROK_SIM_LATENCY_MS`, `RAGNAROK_SIM_JITTER_MS`
//! and `RAGNAROK_SIM_LOSS` (0..1); each applies one way, so round-trip latency doubles.

use crate::rng::Rng;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct NetemConfig {
    pub latency: Duration,
    pub jitter: Duration,
    pub loss: f64,
}

impl NetemConfig {
    pub fn from_env() -> Self {
        let millis = |name: &str| {
            std::env::var(name).ok().and_then(|value| value.parse::<u64>().ok()).map(Duration::from_millis).unwrap_or_default()
        };
        let loss = std::env::var("RAGNAROK_SIM_LOSS").ok().and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0);
        Self { latency: millis("RAGNAROK_SIM_LATENCY_MS"), jitter: millis("RAGNAROK_SIM_JITTER_MS"), loss: loss.clamp(0.0, 1.0) }
    }

    pub fn enabled(&self) -> bool {
        !self.latency.is_zero() || !self.jitter.is_zero() || self.loss > 0.0
    }
}

/// Holds items until their simulated arrival time. Jitter can reorder datagrams, as on a real network.
pub struct DelayQueue<T> {
    config: NetemConfig,
    rng: Rng,
    order: u64,
    heap: BinaryHeap<Reverse<(Instant, u64)>>,
    items: std::collections::HashMap<u64, T>,
}

impl<T> DelayQueue<T> {
    pub fn new(config: NetemConfig, seed: u64) -> Self {
        Self { config, rng: Rng::from_seed(seed), order: 0, heap: BinaryHeap::new(), items: Default::default() }
    }

    /// Queues `item`, or drops it to simulate loss.
    pub fn push(&mut self, item: T, now: Instant) {
        if self.rng.next_f64() < self.config.loss {
            return;
        }
        let jitter = self.config.jitter.mul_f64(self.rng.next_f64());
        self.order += 1;
        self.heap.push(Reverse((now + self.config.latency + jitter, self.order)));
        self.items.insert(self.order, item);
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.heap.peek().map(|Reverse((due, _))| *due)
    }

    pub fn pop_due(&mut self, now: Instant) -> Option<T> {
        match self.heap.peek() {
            Some(Reverse((due, _))) if *due <= now => {
                let Reverse((_, order)) = self.heap.pop()?;
                self.items.remove(&order)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_items_for_the_latency() {
        let config = NetemConfig { latency: Duration::from_millis(100), ..Default::default() };
        let mut queue = DelayQueue::new(config, 1);
        let now = Instant::now();
        queue.push("a", now);
        assert_eq!(queue.pop_due(now), None);
        assert_eq!(queue.pop_due(now + Duration::from_millis(100)), Some("a"));
    }

    #[test]
    fn total_loss_drops_everything() {
        let config = NetemConfig { loss: 1.0, ..Default::default() };
        let mut queue = DelayQueue::new(config, 1);
        queue.push(1, Instant::now());
        assert_eq!(queue.next_due(), None);
    }
}
