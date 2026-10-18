//! Per-player input jitter buffer.
//!
//! Input datagrams arrive unevenly, but the simulation consumes exactly one input per player per
//! tick. A short queue absorbs the jitter: the server waits for `TARGET` frames before consuming,
//! repeats the last input on underrun (minus one-shot buttons), and drops back to `TARGET` if the
//! queue grows past `MAX` (the client's clock is running ahead, or a burst arrived).

use shared::consts::{BUTTON_CHUTE, BUTTON_FIRE, BUTTON_JUMP, BUTTON_RELOAD, BUTTON_SLIDE, BUTTON_USE};
use shared::protocol::InputFrame;
use std::collections::VecDeque;

const TARGET: usize = 2;
const MAX: usize = 8;
/// After this many ticks without input (a backgrounded tab, a stalled connection) the player stops
/// instead of running on in the last direction forever.
const STARVE_LIMIT: u32 = 15;
/// A frame this far past the newest one seen (a minute of ticks) is not a client that kept counting;
/// accepting it would make every real frame after it look old.
const MAX_SEQ_JUMP: u32 = 3600;

#[derive(Default)]
pub struct JitterBuffer {
    frames: VecDeque<InputFrame>,
    newest: u32,
    primed: bool,
    last: InputFrame,
    starved: u32,
}

/// What the simulation should apply this tick.
#[derive(Debug, PartialEq)]
pub struct Consumed {
    pub input: InputFrame,
    /// True when `input` is a real frame from the client (and may be acknowledged).
    pub fresh: bool,
}

impl JitterBuffer {
    /// Accepts the redundant frames of one datagram, keeping only those not seen before.
    pub fn push(&mut self, frames: &[InputFrame]) {
        for frame in frames {
            if frame.seq() <= self.newest || (self.newest != 0 && frame.seq() - self.newest > MAX_SEQ_JUMP) {
                continue;
            }
            self.newest = frame.seq();
            self.frames.push_back(*frame);
        }
        if self.frames.len() > MAX {
            let excess = self.frames.len() - TARGET;
            self.frames.drain(..excess);
        }
    }

    pub fn pop(&mut self) -> Consumed {
        if !self.primed && self.frames.len() >= TARGET {
            self.primed = true;
        }
        if self.primed {
            if let Some(frame) = self.frames.pop_front() {
                self.last = frame;
                self.starved = 0;
                return Consumed { input: frame, fresh: true };
            }
            self.primed = false;
        }
        // Underrun: keep walking the way the player was, but never re-trigger a jump, a use or a shot. After a while,
        // stand still.
        self.starved = self.starved.saturating_add(1);
        let mut repeat = self.last;
        repeat.buttons &= !(BUTTON_JUMP | BUTTON_USE | BUTTON_FIRE | BUTTON_SLIDE | BUTTON_RELOAD | BUTTON_CHUTE);
        if self.starved > STARVE_LIMIT {
            repeat = InputFrame { seq: repeat.seq, strafe: 0, forward: 0, yaw: repeat.yaw, buttons: 0, pitch: repeat.pitch, view: repeat.view, slot: repeat.slot };
        }
        Consumed { input: repeat, fresh: false }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seq: u32) -> InputFrame {
        InputFrame { seq, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_JUMP, pitch: 0, view: 0, slot: 0 }
    }

    #[test]
    fn waits_for_target_depth_before_consuming() {
        let mut buffer = JitterBuffer::default();
        buffer.push(&[frame(1)]);
        assert!(!buffer.pop().fresh);
        buffer.push(&[frame(1), frame(2)]);
        assert_eq!(buffer.pop(), Consumed { input: frame(1), fresh: true });
        assert_eq!(buffer.pop(), Consumed { input: frame(2), fresh: true });
    }

    #[test]
    fn redundant_frames_are_deduplicated() {
        let mut buffer = JitterBuffer::default();
        buffer.push(&[frame(1), frame(2), frame(3)]);
        buffer.push(&[frame(2), frame(3), frame(4)]);
        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn underrun_repeats_last_input_without_jump() {
        let mut buffer = JitterBuffer::default();
        buffer.push(&[frame(1), frame(2)]);
        buffer.pop();
        buffer.pop();
        let repeat = buffer.pop();
        assert!(!repeat.fresh);
        assert_eq!(repeat.input.forward, 127);
        assert_eq!(repeat.input.buttons & BUTTON_JUMP, 0);
        // After an underrun it re-buffers to TARGET before consuming again.
        buffer.push(&[frame(3)]);
        assert!(!buffer.pop().fresh);
        buffer.push(&[frame(4)]);
        assert_eq!(buffer.pop().input.seq(), 3);
    }

    #[test]
    fn long_starvation_stops_the_player() {
        let mut buffer = JitterBuffer::default();
        buffer.push(&[frame(1), frame(2)]);
        buffer.pop();
        buffer.pop();
        for _ in 0..STARVE_LIMIT {
            assert_eq!(buffer.pop().input.forward, 127);
        }
        let stopped = buffer.pop().input;
        assert_eq!((stopped.strafe, stopped.forward), (0, 0));
    }

    #[test]
    fn a_forged_far_future_seq_does_not_lock_out_real_frames() {
        let mut buffer = JitterBuffer::default();
        buffer.push(&[frame(1), frame(2)]);
        buffer.push(&[frame(u32::MAX - 5)]);
        buffer.push(&[frame(3)]);
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn overflow_drops_back_to_target() {
        let mut buffer = JitterBuffer::default();
        let frames: Vec<_> = (1..=MAX as u32 + 1).map(frame).collect();
        buffer.push(&frames);
        assert_eq!(buffer.len(), TARGET);
        assert_eq!(buffer.pop().input.seq(), MAX as u32);
    }
}
