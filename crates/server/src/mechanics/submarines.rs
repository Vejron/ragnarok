use super::Mechanic;
use super::drones::launch_at;
use crate::entities::{Component, With};
use crate::game::{Player, Seats, Vehicle, World};
use shared::consts::*;
use shared::protocol::{KIND_MISSILE, KIND_SUBMARINE, ShotEvent, WEAPON_SPLASH};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Submarines' periscopes and launch tubes (`KIND_SUBMARINE`). Its captain drives it like any vehicle
/// (`Vehicles`); this is its weapons officer's seat:
///
/// - the periscope turns with where the officer looks, and is raised while they sit at it and its head
///   (`SUB_MAST_HEIGHT` over the hull's middle) is above the water;
/// - fire (a fresh press) launches a guided missile (`KIND_MISSILE`) from the first loaded tube, when the
///   hull is no deeper than `SUB_LAUNCH_DEPTH`: it goes up nose first out of the tube's mouth (out of
///   the water over it, with a splash, when the deck is under), and the officer flies it from its
///   nose as a drone (`Drones`), their body staying in their seat. Once it is gone they are back at the
///   periscope. The tube takes `SUB_TUBE_RELOAD` ticks to load again.
///
/// It runs before `Drones`, so the missile it launches is flown from this tick on.
pub struct Submarines;

/// What a submarine carries beyond being a vehicle: a component on its entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Sub {
    /// Ticks until each tube (`SUB_TUBE_AT`) is loaded again; 0 is loaded.
    pub reload: [u32; SUB_TUBES],
    /// The periscope's yaw from the hull, radians, + turns left.
    pub scope: f32,
    /// The periscope is raised.
    pub mast: bool,
}

impl Sub {
    /// The tubes loaded, as a mask: bit `n` for tube `n`.
    pub fn loaded(&self) -> u8 {
        self.reload.iter().enumerate().filter(|(_, left)| **left == 0).fold(0, |mask, (tube, _)| mask | 1 << tube)
    }
}

/// Fires a missile from submarine `vid` for its officer `id`, if a tube is loaded and the hull is
/// shallow enough. Returns whether one went up.
fn fire(world: &mut World, vid: u32, id: u32) -> bool {
    let Some(sub) = world.entities.get::<Sub>(vid).copied() else { return false };
    let Some(tube) = sub.reload.iter().position(|&left| left == 0) else { return false };
    let Some(pose) = world.physics.car_pose(vid) else { return false };
    let [x, y, z] = pose.position;
    let Some(level) = world.physics.water_level(x, z) else { return false };
    if level - y > SUB_LAUNCH_DEPTH || world.entities.dead(vid) {
        return false;
    }
    let mouth = pose.world_point(SUB_TUBE_AT[tube]);
    // Nose up, clear of the deck and of the water over it; it pitches over towards where its flier looks.
    let rise = MISSILE_HALF_EXTENTS[2] + 0.3;
    let start = [mouth[0], mouth[1].max(level) + rise, mouth[2]];
    let yaw = world.entities.get::<Player>(id).map_or(pose.yaw(), |player| from_mrad(player.input.yaw));
    let from = Some((vid, 1));
    if launch_at(world, id, KIND_MISSILE, start, yaw, LAUNCH_PITCH, SUB_LAUNCH_SPEED, from).is_none() {
        return false;
    }
    if mouth[1] < level {
        world.shots.push(ShotEvent { shooter: id, weapon: WEAPON_SPLASH, x: to_cm(mouth[0]), y: to_cm(level), z: to_cm(mouth[2]) });
    }
    if let Some(mut sub) = world.entities.get_mut::<Sub>(vid) {
        sub.reload[tube] = SUB_TUBE_RELOAD;
    }
    // The press is spent on the launch, so `Drones` does not take it for setting the missile off.
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        player.hands.trigger_held = true;
    }
    tracing::debug!(vid, id, tube, "missile launched");
    true
}

/// How far a missile's nose is up from level as it leaves its tube, radians: all but straight up (about
/// 89 degrees), tipped just enough towards where its flier looks that it pitches over that way.
const LAUNCH_PITCH: f32 = 1.55;

impl Mechanic for Submarines {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        let subs: Vec<u32> = world.entities.ids::<With<Vehicle>>().into_iter().filter(|&vid| world.entities.of::<Vehicle>(vid).kind == KIND_SUBMARINE).collect();
        for vid in subs {
            if !world.entities.has::<Sub>(vid) {
                world.entities.insert(vid, Sub::default());
            }
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let [x, y, z] = pose.position;
            let level = world.physics.water_level(x, z).unwrap_or(f32::NEG_INFINITY);
            // The officer at the periscope, not off flying a missile.
            let officer = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(1)).filter(|&id| world.entities.alive(id));
            let at_scope = officer.and_then(|id| world.entities.get::<Player>(id).map(|player| (id, player.seat, player.input, player.hands.trigger_held)));
            let looking = at_scope.filter(|(_, seat, ..)| seat.is_some_and(|seat| seat.vehicle == vid && !seat.remote));
            {
                let mut sub = world.entities.of_mut::<Sub>(vid);
                for left in &mut sub.reload {
                    *left = left.saturating_sub(1);
                }
                if let Some((_, _, input, _)) = looking {
                    sub.scope = wrap_angle(from_mrad(input.yaw) - pose.yaw());
                }
                sub.mast = officer.is_some() && y + SUB_MAST_HEIGHT > level;
            }
            if let Some((id, _, input, held)) = looking
                && input.buttons() & BUTTON_FIRE != 0
                && !held
            {
                fire(world, vid, id);
            }
        }
    }
}

#[cfg(test)]
mod tests;
