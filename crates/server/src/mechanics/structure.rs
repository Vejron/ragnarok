//! Damaged buildings that cannot hold themselves up come down: the carved ones are checked now and
//! then, a couple a tick (`World::settle`), after the blast that hurt them has had a moment.

use super::Mechanic;
use crate::game::World;

pub struct Structure;

impl Mechanic for Structure {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        world.settle();
    }
}
