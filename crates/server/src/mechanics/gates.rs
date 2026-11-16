//! Gates: the wide doors of barns, halls and garages (`shared::windows::Window::is_gate`), which a
//! soldier on foot opens and shuts with USE, and which someone in a vehicle drawn up to a shut one
//! opens with USE from their seat (a second press gets out, as ever). A gate is shut until someone opens it. Shut, it is wall
//! in its building's collider (`World::set_gate`), so it stops rounds and vehicles, and a blast takes
//! it with the cells it stands in; open, it is the hole a car drives in by. See BUILDING-STYLES.md.

use super::Mechanic;
use crate::game::{Player, Vehicle, World};
use shared::consts::BUTTON_USE;
use shared::world::ObjId;
use sim::Obj;
use std::collections::HashMap;

/// How far from a gate, measured over the ground to the nearest point of its sill, a soldier works it, metres.
pub const GATE_REACH: f32 = 2.5;
/// How far from a shut gate a vehicle's middle may be for USE from a seat in it to open the gate, metres.
pub const GATE_REACH_SEATED: f32 = 8.0;
/// How far above or below the sill the soldier's middle may be, metres.
const GATE_HEIGHT: f32 = 3.0;
/// A gate does not shut on a soldier this near its sill, or a vehicle this near past its half width, metres.
const GATE_CLEAR_SOLDIER: f32 = 0.9;
const GATE_CLEAR_VEHICLE: f32 = 3.5;
/// The squares the gates are filed in, metres a side.
const CELL: f32 = 32.0;

/// One gate: its building, its number among the building's openings, and the two ends of its sill, metres.
#[derive(Clone, Copy, Debug)]
struct Site {
    id: ObjId,
    opening: u32,
    a: [f32; 3],
    b: [f32; 3],
}

impl Site {
    /// How far `at` is from the sill over the ground, metres.
    fn distance(&self, at: [f32; 3]) -> f32 {
        let (dx, dz) = (self.b[0] - self.a[0], self.b[2] - self.a[2]);
        let t = (((at[0] - self.a[0]) * dx + (at[2] - self.a[2]) * dz) / (dx * dx + dz * dz).max(1e-6)).clamp(0.0, 1.0);
        (self.a[0] + dx * t - at[0]).hypot(self.a[2] + dz * t - at[2])
    }
}

/// Every gate of the world, filed by the square its middle is in. The mechanic runs twice a tick:
/// before `Vehicles` for those in a seat (`Gates::seated`), whose press would otherwise get them
/// out, and after it for those on foot, whose press boards a vehicle first if one is at hand.
#[derive(Default)]
pub struct Gates {
    sites: Vec<Site>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    /// Whether this is the pass for those in a seat.
    seated: bool,
}

fn cell(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

impl Gates {
    /// The pass for those in a seat.
    pub fn seated() -> Self {
        Gates { seated: true, ..Default::default() }
    }

    /// The gate nearest `at` within `reach`, if any.
    fn nearest(&self, at: [f32; 3], reach: f32) -> Option<Site> {
        let (cx, cz) = cell(at[0], at[2]);
        let mut best: Option<(f32, Site)> = None;
        for x in cx - 1..=cx + 1 {
            for z in cz - 1..=cz + 1 {
                for &index in self.grid.get(&(x, z)).into_iter().flatten() {
                    let site = self.sites[index];
                    let distance = site.distance(at);
                    if distance <= reach && (at[1] - site.a[1]).abs() <= GATE_HEIGHT && best.is_none_or(|(nearest, _)| distance < nearest) {
                        best = Some((distance, site));
                    }
                }
            }
        }
        best.map(|(_, site)| site)
    }
}

/// Whether a soldier or a vehicle is in the way of `site` shutting.
fn blocked(world: &World, site: &Site) -> bool {
    let half = (site.b[0] - site.a[0]).hypot(site.b[2] - site.a[2]) / 2.0;
    let mid = [(site.a[0] + site.b[0]) / 2.0, site.a[1], (site.a[2] + site.b[2]) / 2.0];
    let soldier = world.entities.all::<Player>().any(|(id, player)| world.entities.alive(id) && player.seat.is_none() && site.distance(player.character.position) < GATE_CLEAR_SOLDIER);
    let vehicle = || world.entities.all::<Vehicle>().filter_map(|(id, _)| world.physics.car_pose(id)).any(|pose| (pose.position[0] - mid[0]).hypot(pose.position[2] - mid[2]) < half + GATE_CLEAR_VEHICLE && (pose.position[1] - mid[1]).abs() < 2.0 * GATE_HEIGHT);
    soldier || vehicle()
}

impl Mechanic for Gates {
    fn on_start(&mut self, world: &mut World) {
        for (look, &id) in world.map.looks.iter().zip(&world.map.building_ids) {
            if look.inside.is_empty() {
                continue;
            }
            for opening in shared::windows::gates(&look.windows) {
                let [a, b, ..] = look.windows[opening as usize].corners;
                self.grid.entry(cell((a[0] + b[0]) / 2.0, (a[2] + b[2]) / 2.0)).or_default().push(self.sites.len());
                self.sites.push(Site { id, opening, a, b });
            }
        }
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        if self.sites.is_empty() {
            return;
        }
        if self.seated {
            // From a seat: a shut gate ahead opens, and the press is spent, so nobody gets out.
            let pressed = world.entities.ids_where::<Player>(|id, player| world.entities.alive(id) && player.seat.is_some_and(|seat| !seat.remote) && player.input.buttons() & BUTTON_USE != 0);
            for id in pressed {
                let Some(pose) = world.entities.of::<Player>(id).seat.and_then(|seat| world.physics.car_pose(seat.vehicle)) else { continue };
                let Some(site) = self.nearest(pose.position, GATE_REACH_SEATED) else { continue };
                if world.gate_shut(site.id, site.opening) == Some(true) && world.set_gate(site.id, site.opening, true) {
                    world.entities.of_mut::<Player>(id).input.buttons &= !BUTTON_USE;
                    tracing::debug!(id, ?site.id, gate = site.opening, "gate opened from a seat");
                }
            }
            return;
        }
        // Whoever boarded a vehicle or took an item with this press has spent it already.
        let pressed = world.entities.ids_where::<Player>(|id, player| world.entities.alive(id) && player.seat.is_none() && player.lift.is_none() && player.input.buttons() & BUTTON_USE != 0);
        for id in pressed {
            let at = world.entities.of::<Player>(id).character.position;
            let Some(site) = self.nearest(at, GATE_REACH) else { continue };
            if world.destroyed.iter().any(|&(object, _)| object == Obj::Building(site.id)) {
                continue;
            }
            let Some(shut) = world.gate_shut(site.id, site.opening) else { continue };
            // A gate does not come down on whoever stands under it.
            if !shut && blocked(world, &site) {
                continue;
            }
            if world.set_gate(site.id, site.opening, shut) {
                world.entities.of_mut::<Player>(id).input.buttons &= !BUTTON_USE;
                tracing::debug!(id, ?site.id, gate = site.opening, open = shut, "gate");
            }
        }
    }
}

#[cfg(test)]
mod tests;
