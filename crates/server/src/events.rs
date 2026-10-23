//! What happened this tick, as typed events: a mechanic says what it did (`World::send`) and whoever
//! cares reads it (`World::events`), so the code that hurts something does not also build the hit
//! marker, credit the kill or tell the bots.
//!
//! Each kind is a `bevy_ecs` `Messages` resource in the room's ECS world, in send order. `Game::step`
//! lets the mechanics react (`Mechanic::react`), turns what is left into wire messages
//! (`game::feedback`) and then moves every queue on a tick (`update`). A reader that reads once a
//! tick with its own `MessageCursor` (as the bots hear noises at the start of the next one) sees each
//! event exactly once; one that reads before `update` (the reactions, the feedback) reads this tick's
//! with `iter_current_update_messages`.
//!
//! A reaction that something later in the same tick must see (a bandage spoiled, a dead rider out of
//! their seat) cannot wait for the end of it: it is a hook (`Hooks`), which the damage path runs the
//! moment it sends the event.

use crate::game::World;
use bevy_ecs::message::{Message, Messages};

/// Something loud a bot may hear: where it was, who made it (none for a wreck going up, say), and
/// how far it carries, metres. Walls do not stop it.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    pub at: [f32; 3],
    pub by: Option<u32>,
    pub reach: f32,
}

/// Entity `victim` lost `lost` health. Sent only when it lost some.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct Damaged {
    pub victim: u32,
    pub lost: u8,
    /// Whoever did it, if anyone: the victim themself for a wound of their own.
    pub attacker: Option<u32>,
    /// `WEAPON_*`; 0 for harm a vehicle takes.
    pub weapon: u8,
    /// Where the harm came from (a blast's centre, or where the attacker was), when known.
    pub from: Option<[f32; 3]>,
    /// Where it was struck: a stickman's chest, a creature's crown, a vehicle's middle.
    pub at: [f32; 3],
    /// A round that hit the head.
    pub head: bool,
    /// This took the last of its health.
    pub killed: bool,
    /// Whoever rode in it, for a vehicle: they feel the hit as well.
    pub aboard: Vec<u32>,
}

/// Entity `victim` died, of `weapon`. `killer` is whoever is credited, if anyone other than the
/// victim; `style` (`DEATH_*`) is how it went down, and `from` where the killing blow came from.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Died {
    pub victim: u32,
    pub killer: Option<u32>,
    pub weapon: u8,
    pub style: u8,
    pub from: Option<[f32; 3]>,
}

/// The reactions run at once (see the module's doc), in the order the mechanics that register them
/// (`Mechanic::hooks`) run. Each guards itself: a hook for players looks for a `Player` first.
#[derive(Clone, Default)]
pub struct Hooks {
    pub damaged: Vec<fn(&mut World, &Damaged)>,
    pub died: Vec<fn(&mut World, &Died)>,
}

/// Sends `damaged`, and runs the hooks on it.
pub fn damaged(world: &mut World, damaged: Damaged) {
    for at in 0..world.hooks.damaged.len() {
        (world.hooks.damaged[at])(world, &damaged);
    }
    world.send(damaged);
}

/// Sends `died`, and runs the hooks on it.
pub fn died(world: &mut World, died: Died) {
    for at in 0..world.hooks.died.len() {
        (world.hooks.died[at])(world, &died);
    }
    world.send(died);
}

/// Adds a queue for every kind of event to the room's ECS world.
pub fn register(ecs: &mut bevy_ecs::world::World) {
    ecs.init_resource::<Messages<Noise>>();
    ecs.init_resource::<Messages<Damaged>>();
    ecs.init_resource::<Messages<Died>>();
}

/// Moves every queue on a tick: what was sent before the last `update` is dropped, what was sent since
/// becomes last tick's.
pub fn update(ecs: &mut bevy_ecs::world::World) {
    ecs.resource_mut::<Messages<Noise>>().update();
    ecs.resource_mut::<Messages<Damaged>>().update();
    ecs.resource_mut::<Messages<Died>>().update();
}
