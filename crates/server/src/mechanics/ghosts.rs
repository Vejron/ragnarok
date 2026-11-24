//! Ghosts (GHOSTS.md). A player killed comes back, after `RESPAWN_TICKS`, as a ghost: a shadowy cloud
//! that rises out of the body and flies anywhere, through walls and roofs, as fast as a drone. It
//! stays dead the while, so nothing can hurt it, and it can pick up, board or use nothing (every
//! mechanic that does those asks whether the player is alive). Everyone sees it drifting about.
//!
//! After `GHOST_MIN_TICKS` the player may take shape again with use (`BUTTON_USE`), but only where no
//! living enemy is within `GHOST_KEEPOUT` and the ground under it is somewhere to stand
//! (`spawn::usable`). Taking shape lasts `GHOST_FORM_TICKS`, a swirl on the ground everyone sees,
//! and then the soldier stands there whole, with everything they carried when they died. The
//! chute button (`BUTTON_CHUTE`) gives the ghost up for a parachute drop at their next entry point
//! instead, with their own loadout afresh. A ghost still one after `GHOST_TICKS` takes shape where it is if it may, and drops at
//! an entry point if not.
//!
//! Bots, zombies and bus drivers do not become ghosts: they come back at a spawn point as before.

use super::Mechanic;
use super::combat::same_team;
use super::spawn::{self, SPAWN_DROP};
use crate::entities::{Component, Health, With};
use crate::game::{Bot, Player, World};
use shared::consts::*;
use shared::quant::to_cm;
use sim::STANDING_Y;

pub struct Ghosts;

/// A dead player come back as a ghost. Their body is the `Player` with the same id, its character
/// flying (`Character::ghost`) until it takes shape.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ghost {
    /// Ticks it has been a ghost.
    pub age: u32,
    /// Ticks left taking shape, 0 while it is not.
    pub forming: u32,
    /// The buttons held last tick, so that a press is seen once.
    pub held: u16,
}

/// Whether player `id` comes back from death as a ghost: everyone with a mind of their own.
pub(super) fn haunts(world: &World, id: u32) -> bool {
    !world.entities.has::<Bot>(id) && !world.entities.has::<super::Zombie>(id) && !super::is_civilian(world, id)
}

/// Player `id`'s ghost, if they are one.
pub fn ghost(world: &World, id: u32) -> Option<Ghost> {
    world.entities.get::<Ghost>(id).copied()
}

/// What to show of player `id`'s ghost: its stance (`STANCE_GHOST` or `STANCE_FORM`), the whole
/// seconds it has left as a ghost and why it may not take shape (`GHOST_BLOCK_*`); `None` if they
/// are not one.
pub fn shown(world: &World, id: u32) -> Option<(u8, u8, u8)> {
    let ghost = ghost(world, id)?;
    let left = GHOST_TICKS.saturating_sub(ghost.age).div_ceil(TICK_RATE).min(255) as u8;
    if ghost.forming > 0 {
        return Some((STANCE_FORM, left, GHOST_BLOCK_NONE));
    }
    let block = if ghost.age < GHOST_MIN_TICKS { GHOST_BLOCK_SOON } else { spot(world, id) };
    Some((STANCE_GHOST, left, block))
}

/// Player `id`, dead and due back, rises from their body as a ghost.
pub(super) fn rise(world: &mut World, id: u32) {
    super::limbs::restore(world, id);
    let Some(mut player) = world.entities.get_mut::<Player>(id) else { return };
    player.interrupt();
    let held = player.input.buttons();
    let c = player.character;
    player.character = sim::Character { position: [c.position[0], c.position[1] + GHOST_RISE, c.position[2]], yaw: c.yaw, ghost: true, ..Default::default() };
    world.entities.insert(id, Ghost { age: 0, forming: 0, held });
}

/// Whether ghost `id` may take shape where it is, time aside: `GHOST_BLOCK_NONE`, or why not.
fn spot(world: &World, id: u32) -> u8 {
    let Some(player) = world.entities.get::<Player>(id) else { return GHOST_BLOCK_GROUND };
    let [x, _, z] = player.character.position;
    if !spawn::usable(&world.map, [to_cm(x), to_cm(z)]) {
        return GHOST_BLOCK_GROUND;
    }
    let enemy = world.entities.all::<Player>().any(|(other, body)| {
        let [ox, _, oz] = super::position_of(world, other).unwrap_or(body.character.position);
        other != id && world.entities.alive(other) && !super::is_civilian(world, other) && !same_team(world, id, other) && (ox - x).hypot(oz - z) < GHOST_KEEPOUT
    });
    if enemy { GHOST_BLOCK_ENEMY } else { GHOST_BLOCK_NONE }
}

/// Ghost `id` starts taking shape: it sinks to the ground where it is, and stays there, still dead,
/// for `GHOST_FORM_TICKS`.
fn form(world: &mut World, id: u32) {
    let Some(mut player) = world.entities.get_mut::<Player>(id) else { return };
    let [x, _, z] = player.character.position;
    let ground = world.map.ground.height_at(x, z);
    player.character = sim::Character { position: [x, ground + STANDING_Y + SPAWN_DROP, z], yaw: player.character.yaw, ..Default::default() };
    if let Some(mut ghost) = world.entities.get_mut::<Ghost>(id) {
        ghost.forming = GHOST_FORM_TICKS;
    }
}

/// Ghost `id` has taken shape: the soldier stands there, whole, with what they carried.
fn materialize(world: &mut World, id: u32) {
    world.entities.remove::<Ghost>(id);
    super::limbs::restore(world, id);
    if let Some(mut health) = world.entities.get_mut::<Health>(id) {
        health.hp = health.max;
    }
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        player.interrupt();
    }
}

/// Ghost `id` gives itself up and drops by parachute at the player's next entry point, with their
/// own loadout afresh in place of what they carried.
fn drop_in(world: &mut World, id: u32) {
    world.entities.remove::<Ghost>(id);
    if let Some(mut health) = world.entities.get_mut::<Health>(id) {
        health.hp = health.max;
    }
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        player.hands.hotbar = player.hands.loadout;
        player.interrupt();
        // The press that gave the ghost up must not close the canopy it drops under.
        player.input.buttons &= !BUTTON_CHUTE;
    }
    spawn::place(world, id);
}

impl Mechanic for Ghosts {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        for id in world.entities.ids::<With<Ghost>>() {
            let Some(buttons) = world.entities.get::<Player>(id).map(|player| player.input.buttons()) else { continue };
            let Some(mut ghost) = world.entities.get_mut::<Ghost>(id) else { continue };
            let pressed = buttons & !ghost.held;
            ghost.held = buttons;
            if ghost.forming > 0 {
                ghost.forming -= 1;
                if ghost.forming == 0 {
                    materialize(world, id);
                }
                continue;
            }
            ghost.age += 1;
            let (age, chute, wants) = (ghost.age, pressed & BUTTON_CHUTE != 0, pressed & BUTTON_USE != 0);
            if chute {
                drop_in(world, id);
                continue;
            }
            let timed_out = age >= GHOST_TICKS;
            if (wants && age >= GHOST_MIN_TICKS) || timed_out {
                if spot(world, id) == GHOST_BLOCK_NONE {
                    form(world, id);
                } else if timed_out {
                    drop_in(world, id);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "ghosts/tests.rs"]
mod tests;
