//! Riding the ski lifts (LIFTS.md). A chair is nothing in the world: its place is a function of its
//! lift and the tick (`shared::lifts`), worked out here for the riders and on the client for drawing.
//!
//! USE beside a chair at a station sits the soldier on it; the chair then carries them, seated and
//! able to look about and shoot, round and round the loop for as long as they like. A jump lets go,
//! with the cable's speed: at a station they step off, between the pylons they fall.

use super::Mechanic;
use crate::game::{LiftRide, Player, World};
use shared::consts::{BUTTON_JUMP, BUTTON_USE, LIFT_BOARD_HEIGHT, LIFT_BOARD_RADIUS, LIFT_RIDER_CENTRE};
use shared::lifts::Path;

/// Carries the soldiers on the ski lifts' chairs, and seats and puts them off.
#[derive(Default)]
pub struct Lifts {
    /// Each of the world's lifts, laid out.
    paths: Vec<Path>,
}

impl Lifts {
    /// The lifts laid out, again if the world's lifts changed (a test sets them after the start).
    fn lay_out(&mut self, world: &World) {
        if self.paths.len() != world.map.lifts.len() {
            self.paths = world.map.lifts.iter().map(Path::new).collect();
        }
    }
}

/// The free seat nearest `at` (capsule centre, metres) on a chair a soldier there can sit on: one low
/// over the ground beside them, which is at a station.
fn chair_for(world: &World, paths: &[Path], at: [f32; 3]) -> Option<LiftRide> {
    let feet = at[1] - shared::consts::STAND_CENTRE;
    let mut best: Option<(f32, LiftRide)> = None;
    for (lift, path) in paths.iter().enumerate() {
        for chair in 0..path.chairs {
            let pose = path.chair(chair, world.tick);
            if (pose.seat[0] - at[0]).hypot(pose.seat[2] - at[2]) > LIFT_BOARD_RADIUS + 1.0 {
                continue;
            }
            let ground = world.map.ground.height_at(pose.seat[0], pose.seat[2]);
            if pose.seat[1] - ground > LIFT_BOARD_HEIGHT || (pose.seat[1] - feet).abs() > LIFT_BOARD_HEIGHT {
                continue;
            }
            for seat in 0..path.seats {
                let taken = !world.entities.ids_where::<Player>(|_, player| player.lift == Some(LiftRide { lift: lift as u16, chair: chair as u16, seat })).is_empty();
                let place = path.place(&pose, seat);
                let far = (place[0] - at[0]).hypot(place[2] - at[2]);
                if !taken && far <= LIFT_BOARD_RADIUS && best.is_none_or(|(d, _)| far < d) {
                    best = Some((far, LiftRide { lift: lift as u16, chair: chair as u16, seat }));
                }
            }
        }
    }
    best.map(|(_, ride)| ride)
}

/// Lets a rider go where they are, with the cable's speed.
fn let_go(world: &mut World, paths: &[Path], id: u32) {
    let tick = world.tick;
    let Some(mut player) = world.entities.get_mut::<Player>(id) else { return };
    let Some(ride) = player.lift.take() else { return };
    let Some(path) = paths.get(ride.lift as usize) else { return };
    let chair = path.chair(u32::from(ride.chair), tick);
    let velocity = path.velocity(&chair);
    let character = &mut player.character;
    character.knock = [velocity[0], velocity[2]];
    character.vy = velocity[1];
    character.grounded = false;
    tracing::debug!(id, lift = ride.lift, "off the lift");
}

impl Mechanic for Lifts {
    fn on_start(&mut self, world: &mut World) {
        self.lay_out(world);
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        self.lay_out(world);
        if self.paths.is_empty() {
            return;
        }
        let paths = std::mem::take(&mut self.paths);
        // Getting on. A rider's USE does nothing: only a jump lets go.
        let pressed = world.entities.ids_where::<Player>(|id, player| world.entities.alive(id) && player.seat.is_none() && player.lift.is_none() && player.input.buttons() & BUTTON_USE != 0);
        for id in pressed {
            let at = world.entities.of::<Player>(id).character.position;
            if let Some(ride) = chair_for(world, &paths, at) {
                let mut player = world.entities.of_mut::<Player>(id);
                player.lift = Some(ride);
                player.character.prone = false;
                player.character.skis = false;
                tracing::debug!(id, lift = ride.lift, chair = ride.chair, "on the lift");
            } else {
                continue;
            }
            // The press is spent: the car beside the station stays unboarded, the item there untaken.
            world.entities.of_mut::<Player>(id).input.buttons &= !BUTTON_USE;
        }
        // A rider's USE is nobody's: no car beside the line is boarded from the chair, no item taken.
        for id in world.entities.ids_where::<Player>(|_, player| player.lift.is_some()) {
            world.entities.of_mut::<Player>(id).input.buttons &= !BUTTON_USE;
        }
        // Carrying the riders, round and round.
        let riders = world.entities.ids_where::<Player>(|_, player| player.lift.is_some());
        for id in riders {
            let (alive, jumped, down, ride) = {
                let player = world.entities.of::<Player>(id);
                (world.entities.alive(id), player.input.buttons() & BUTTON_JUMP != 0, player.character.down > 0, player.lift)
            };
            let Some(ride) = ride else { continue };
            let Some(path) = paths.get(ride.lift as usize) else {
                world.entities.of_mut::<Player>(id).lift = None;
                continue;
            };
            let chair = path.chair(u32::from(ride.chair), world.tick);
            // Jumping off, thrown off by a blast, or dead.
            if !alive || jumped || down {
                let_go(world, &paths, id);
                continue;
            }
            let seat = path.place(&chair, ride.seat);
            let mut player = world.entities.of_mut::<Player>(id);
            let player = &mut *player;
            let character = &mut player.character;
            character.position = [seat[0], seat[1] + LIFT_RIDER_CENTRE, seat[2]];
            // They look about as they like; the client turns the body to the chair.
            character.yaw = shared::quant::from_mrad(player.input.yaw);
            character.vy = 0.0;
            character.knock = [0.0, 0.0];
            character.grounded = true;
            character.ride = 0;
            character.chute = false;
            character.swimming = false;
        }
        self.paths = paths;
    }

    fn on_leave(&mut self, world: &mut World, id: u32) {
        if let Some(mut player) = world.entities.get_mut::<Player>(id) {
            player.lift = None;
        }
    }
}

#[cfg(test)]
mod tests;
