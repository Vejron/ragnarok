//! The fuel stations' drive-through washes (FUEL.md). The wash drives a car through itself, in the
//! shared physics (`sim`'s `drive_car`, `shared::station::pull`); this is what it does to what comes
//! out. A vehicle that goes in at one door and comes out of the other is made whole as if new (its
//! hull, wheels, doors, parts and turret, its fire put out, its tank full), everyone aboard is wanted
//! no more, and every client is told to wash its dents, holes and blood off (`S_WASHED`). One that
//! somehow leaves by the door it came in at is not healed.

use super::Mechanic;
use super::emergency::clear_wanted;
use crate::entities::{Health, With};
use crate::game::{Seats, Vehicle, World};
use shared::station::Bay;
use std::collections::HashMap;

/// The washes of the world, and the vehicles in them.
#[derive(Default)]
pub struct Carwash {
    bays: Vec<Bay>,
    /// Each vehicle between a bay's doors: which bay, and which end it came in at (the sign of how
    /// far along the lane it was when it came in).
    inside: HashMap<u32, (usize, f32)>,
}

impl Mechanic for Carwash {
    fn on_start(&mut self, world: &mut World) {
        self.bays = world.map.fuel_stations.iter().map(Bay::of).collect();
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        if self.bays.is_empty() {
            return;
        }
        let mut inside = HashMap::with_capacity(self.inside.len());
        for vid in world.entities.ids::<With<Vehicle>>() {
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let [x, y, z] = pose.position;
            let came = self.inside.get(&vid).copied();
            let now = self.bays.iter().position(|bay| (y - bay.floor).abs() < 3.0 && bay.inside([x, z]));
            match (came, now) {
                (Some((bay, from)), Some(now)) if now == bay => {
                    inside.insert(vid, (bay, from));
                }
                (Some((bay, from)), _) => {
                    // Out: through the far door, or back the way it came.
                    let [along, _] = self.bays[bay].local([x, z]);
                    if along.signum() != from && world.entities.alive(vid) {
                        wash(world, vid);
                    }
                }
                (None, Some(now)) => {
                    let [along, _] = self.bays[now].local([x, z]);
                    inside.insert(vid, (now, along.signum()));
                }
                (None, None) => {}
            }
        }
        self.inside = inside;
    }

    #[cfg(test)]
    fn inside(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// Vehicle `vid` made whole as it comes out of a wash, everyone aboard wanted no more, and every
/// client told.
fn wash(world: &mut World, vid: u32) {
    if world.entities.get::<Health>(vid).is_none_or(|health| !health.alive()) {
        return;
    }
    world.entities.of_mut::<Vehicle>(vid).renew();
    super::vehicles::restore(world, vid, false);
    world.physics.mend_car(vid);
    let riders: Vec<u32> = world.entities.get::<Seats>(vid).map(|seats| seats.riders().collect()).unwrap_or_default();
    for rider in riders {
        clear_wanted(world, rider);
    }
    world.fresh.washed.push(vid);
}

#[cfg(test)]
mod tests;
