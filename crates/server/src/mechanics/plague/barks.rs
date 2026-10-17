//! What the Orange Plague says, and when. The server only picks the moment and a line number; the
//! clients hold the lines (`client/src/plague-lines.ts`) and fill in whom they are about.
//!
//! He talks a lot, but not over himself: a line waits for `GAP_TICKS` after the last one, and a kind
//! of line for `REPEAT_TICKS` after it was last said. The moments that matter (arriving, falling,
//! leaving, a building bought) are announced whatever he said lately.

use crate::game::World;
use shared::consts::TICK_RATE;
use shared::protocol::{BARK_IDLE, BARK_LAST, Bark, S_BARK};

/// Quiet between two lines, ticks.
const GAP_TICKS: u32 = 5 * TICK_RATE;
/// Quiet between two lines of the same kind, ticks.
const REPEAT_TICKS: u32 = 20 * TICK_RATE;
/// After this long with nothing said, he rants about nothing in particular.
const IDLE_TICKS: u32 = 40 * TICK_RATE;
const TRIGGERS: usize = BARK_LAST as usize + 1;

#[derive(Debug, Default)]
pub struct Barker {
    /// When he last said anything, and each kind of line.
    last: Option<u32>,
    each: [Option<u32>; TRIGGERS],
}

impl Barker {
    /// Says a `trigger` line about player `subject` (0 for nobody), unless he spoke too recently.
    /// Returns whether he said it.
    pub fn say(&mut self, world: &mut World, trigger: u8, subject: u32) -> bool {
        self.say_if_quiet(world, trigger, subject, GAP_TICKS)
    }

    /// As `say`, for a line that cuts in on whatever he was saying (someone fired, a new Enemy of the
    /// Week): only its own kind has to have been quiet a while.
    pub fn cut_in(&mut self, world: &mut World, trigger: u8, subject: u32) -> bool {
        self.say_if_quiet(world, trigger, subject, 0)
    }

    fn say_if_quiet(&mut self, world: &mut World, trigger: u8, subject: u32, gap: u32) -> bool {
        let now = world.tick;
        let since = |at: Option<u32>| at.map_or(u32::MAX, |at| now.wrapping_sub(at));
        if since(self.last) < gap || since(self.each.get(trigger as usize).copied().flatten()) < REPEAT_TICKS {
            return false;
        }
        self.announce(world, trigger, subject);
        true
    }

    /// Says a `trigger` line about `subject` whatever he said lately.
    pub fn announce(&mut self, world: &mut World, trigger: u8, subject: u32) {
        let variant = (world.rng.next_u64() % 256) as u8;
        world.barks.push(Bark(S_BARK, trigger, variant, subject));
        self.last = Some(world.tick);
        if let Some(at) = self.each.get_mut(trigger as usize) {
            *at = Some(world.tick);
        }
    }

    /// Rants if he has said nothing for a while.
    pub fn idle(&mut self, world: &mut World) {
        let quiet = self.last.map_or(u32::MAX, |at| world.tick.wrapping_sub(at));
        if quiet >= IDLE_TICKS {
            self.announce(world, BARK_IDLE, 0);
        }
    }
}
