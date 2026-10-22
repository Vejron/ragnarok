//! Sending each player's snapshot: the world as components (`replicate.rs`) and the per-player part
//! (own state, driver motion) that goes with it.

use super::*;

/// The world as v1's rows (`shared::replicated::v1_rows` of `Game::replicated`), for tests that ask
/// what a client is shown.
#[cfg(test)]
impl Game {
    pub fn entity_states(&self) -> Vec<EntityState> {
        shared::replicated::v1_rows(&self.replicated()).0
    }

    pub fn vehicle_states(&self) -> Vec<VehicleState> {
        shared::replicated::v1_rows(&self.replicated()).1
    }

    pub fn projectile_states(&self) -> Vec<ProjectileState> {
        shared::replicated::v1_rows(&self.replicated()).2
    }

    pub fn pickup_states(&self) -> Vec<PickupState> {
        shared::replicated::v1_rows(&self.replicated()).3
    }
}

impl Game {
    /// The motion of the vehicle `id` drives, for their own prediction; `None` for anyone else.
    pub(crate) fn drive_state(&self, id: u32) -> Option<DriveState> {
        let seat = self.world.entities.get::<Player>(id)?.seat?;
        if seat.index != 0 {
            return None;
        }
        let motion = self.world.physics.car_motion(seat.vehicle)?;
        let [vx, vy, vz] = motion.linvel.map(to_cm_i16);
        let [wx, wy, wz] = motion.angvel.map(to_milli_i16);
        Some(DriveState { vx, vy, vz, wx, wy, wz, steer: to_milli_i16(motion.steer), spin: to_milli_i16(motion.spin) })
    }

    pub(super) fn send_snapshots(&mut self) {
        let scene = self.replicated();
        let mut shots = std::mem::take(&mut self.world.shots);
        shots.truncate(MAX_SHOT_EVENTS);
        // Each bullet goes out in a couple of snapshots, so a lost datagram does not lose it.
        let now = self.world.tick;
        self.world.bullets.retain(|b| now.wrapping_sub(b.tick) <= 2 * SNAPSHOT_EVERY);
        let bullets = self.world.bullets.clone();
        let everything = Everything { scene: &scene, shots: &shots, bullets: &bullets };
        let ids: Vec<u32> = self.outboxes.keys().copied().collect();
        for id in ids {
            let Some(player) = self.world.entities.get::<Player>(id) else { continue };
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, drive: self.drive_state(id),
                me: SelfState { hp: player.hp, progress: progress(player), slots: player.hotbar.to_vec() },
            };
            // Seated players ride with their vehicle, so the vehicle's position is theirs. A drone's
            // pilot sees what it sees. An FPV drone's range keeps their body in the picture too; a
            // loitering drone can take it out, which their client, drawing the body itself, does not mind.
            let remote = player.seat.filter(|seat| seat.remote).and_then(|seat| self.world.physics.car_pose(seat.vehicle));
            let here = remote.map_or(player.character.position, |pose| pose.position);
            let at = [here[0], here[2]];
            let (Some(outbox), Some(replica)) = (self.outboxes.get(&id), self.replicas.get_mut(&id)) else { continue };
            let snapshot = replica.snapshot(at, &everything, own);
            // A full outbox means the session is stalled; dropping a snapshot is harmless.
            let _ = outbox.try_send(Outgoing::Datagram(snapshot.into()));
        }
    }
}

/// How far the player's reload or bandage has got, 0..=255, or 0 when neither is going on.
fn progress(player: &Player) -> u8 {
    use shared::items::{BANDAGE_TICKS, def};
    let (left, total) = if player.channel_left > 0 {
        (player.channel_left, BANDAGE_TICKS)
    } else if player.reload_left > 0 {
        (player.reload_left, def(player.held().item).map_or(1, |d| d.reload_ticks.max(1)))
    } else {
        return 0;
    };
    let done = 1.0 - left as f32 / total as f32;
    ((done * 255.0).round() as u8).max(1)
}
