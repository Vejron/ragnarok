//! Sending each player's snapshot: the world as components (`replicate.rs`), with what only they own.

use super::*;

/// The world as a client is told it (`Game::replicated`), by what each thing is, for tests that ask
/// what a client is shown.
#[cfg(test)]
impl Game {
    /// What a client other than its owner is told of entity `id`, if anything.
    pub fn seen(&self, id: u32) -> Option<shared::replicated::Replicated> {
        self.seen_where(|_| true).into_iter().find(|(seen, _)| *seen == id).map(|(_, entity)| entity)
    }

    /// What player `receiver` is told of entity `id`, if anything: all of it if they own it.
    pub fn told(&self, receiver: u32, id: u32) -> Option<shared::replicated::Replicated> {
        let entity = self.replicated().into_iter().find(|(seen, _)| *seen == id)?.1;
        if self.owned(receiver).contains(&Some(id)) { Some(entity) } else { Some(entity.public()).filter(told_of) }
    }

    /// The replicated entities `which` picks, by id, as anyone but their owner is told them.
    pub fn seen_where(&self, which: impl Fn(&shared::replicated::Replicated) -> bool) -> Vec<(u32, shared::replicated::Replicated)> {
        self.replicated().into_iter().map(|(id, entity)| (id, entity.public())).filter(|(_, entity)| told_of(entity) && which(entity)).collect()
    }

    /// Stickmen and creatures of `kind` on foot.
    pub fn seen_people(&self, kind: u8) -> Vec<(u32, shared::replicated::Replicated)> {
        self.seen_where(|e| e.place.is_some() && e.orientation.is_none() && e.projectile.is_none() && e.pickup.is_none() && e.kind.as_ref().is_some_and(|k| k.kind == kind))
    }

    /// Players sitting in vehicle `vehicle`, with no place of their own.
    pub fn seen_riders(&self, vehicle: u32) -> Vec<(u32, shared::replicated::Replicated)> {
        self.seen_where(|e| e.place.is_none() && e.seated.as_ref().is_some_and(|seated| seated.vehicle == vehicle))
    }

    pub fn seen_vehicles(&self) -> Vec<(u32, shared::replicated::Replicated)> {
        self.seen_where(|e| e.orientation.is_some())
    }

    pub fn seen_projectiles(&self) -> Vec<(u32, shared::replicated::Replicated)> {
        self.seen_where(|e| e.projectile.is_some())
    }

    pub fn seen_pickups(&self) -> Vec<(u32, shared::replicated::Replicated)> {
        self.seen_where(|e| e.pickup.is_some())
    }
}

/// Whether anyone but its owner is told of `entity` at all: it is somewhere, or sits somewhere.
#[cfg(test)]
fn told_of(entity: &shared::replicated::Replicated) -> bool {
    entity.place.is_some() || entity.seated.is_some()
}

impl Game {
    pub(super) fn send_snapshots(&mut self) {
        let scene = crate::replication::ready(self.replicated());
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
            let own = Own { tick: self.world.tick, ack: player.ack, ride: player.character.ride, owned: self.owned(id) };
            // Seated players ride with their vehicle, so the vehicle's position is theirs. A drone's
            // pilot sees what it sees; what they own (their body among it) is sent wherever it is.
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
