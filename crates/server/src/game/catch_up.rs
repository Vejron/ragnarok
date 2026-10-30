//! What a joiner is told of the room as it stands, so they see what everyone else sees: state, not news.

use super::*;

/// Most cubes in one `Built` of the catch-up: about 10 bytes each on the wire.
const BUILT_BATCH: usize = 32_768;
/// Numbers of `DugTile::runs` in one `Dug`: at most 5 bytes each, well under the client's 4 MiB a message.
const DUG_BATCH: usize = 400_000;
/// Cells in one `Carved`: at most 5 bytes each, likewise.
const CARVED_BATCH: usize = 400_000;

/// `items`, in order, in groups each stopping once their `weight` reaches `limit`.
fn batches<T>(items: impl IntoIterator<Item = T>, weight: impl Fn(&T) -> usize, limit: usize) -> Vec<Vec<T>> {
    let (mut groups, mut group, mut size) = (Vec::new(), Vec::new(), 0);
    for item in items {
        size += weight(&item);
        group.push(item);
        if size >= limit {
            groups.push(std::mem::take(&mut group));
            size = 0;
        }
    }
    if !group.is_empty() {
        groups.push(group);
    }
    groups
}

impl Game {
    /// The messages that bring a joiner up to date, in the order they are to be sent. Order matters:
    /// the ground as craters left it comes before the craters marked on it.
    pub(super) fn catch_up(&mut self) -> Vec<Vec<u8>> {
        let marks = self.world.crater_marks();
        let world = &self.world;
        let mut out = Vec::new();
        let profiles = world.entities.ids::<With<Player>>().into_iter().map(|id| profile_entry(world, id)).collect();
        out.push(encode(&Profiles { tag: S_PROFILES, players: profiles }));
        let scores = world.entities.all::<Player>().map(|(id, player)| Score { id, kills: player.score.kills, deaths: player.score.deaths, held: player.score.held }).collect();
        out.push(encode(&Scores { tag: S_SCORES, scores }));
        // Always sent, even empty: the client takes the first one after the welcome as the catch-up
        // (not news to animate), and snapshots on the datagram channel can overtake it.
        out.push(encode(&Destroyed { tag: S_DESTROYED, objects: world.destroyed.iter().map(|&(object, yaw)| destroyed_entry(object, yaw, 0)).collect() }));
        let hurt: Vec<_> = world
            .object_hp
            .keys()
            .filter_map(|&object| match (object, world.damage_stage(object)) {
                (Obj::Building(id), Some(stage)) if stage > 0 => Some(damaged_entry(id, stage, None)),
                _ => None,
            })
            .collect();
        if !hurt.is_empty() {
            out.push(encode(&shared::protocol::Damaged { tag: S_DAMAGED, buildings: hurt }));
        }
        // Cells already gone.
        let carved = world.carves.iter().filter_map(|(&id, carving)| {
            let gone: Vec<u32> = (0..carving.removed.len()).filter(|&i| carving.removed[i]).map(|i| i as u32).collect();
            (!gone.is_empty()).then(|| carved_entry(id, [0; 3], gone, Vec::new()))
        });
        for buildings in batches(carved, |building| building.blasted.len(), CARVED_BATCH) {
            out.push(encode(&Carved { tag: S_CARVED, live: false, buildings }));
        }
        for tiles in batches(world.dug_ground(), |tile| tile.runs.len(), DUG_BATCH) {
            out.push(encode(&Dug { tag: S_DUG, tiles }));
        }
        if !marks.is_empty() {
            out.push(encode(&Cratered { tag: S_CRATERED, live: false, craters: marks }));
        }
        let cubes: Vec<_> = world.construct.cubes().collect();
        for batch in cubes.chunks(BUILT_BATCH) {
            out.push(encode(&built_message(false, batch.to_vec(), Vec::new(), Vec::new())));
        }
        if let Some(objective) = self.scenario.as_ref().map(Scenario::objective).filter(|o| !o.is_empty()) {
            out.push(encode(&Notice { tag: S_NOTICE, kind: NOTICE_OBJECTIVE, text: objective.into() }));
        }
        if let Some(state) = &world.backpack {
            let held = state.carrier.and_then(|carrier| world.entities.get::<Player>(carrier)).map_or(0, |player| player.score.held);
            out.push(encode(&state.circle(held)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_stop_at_their_weight_and_keep_the_order() {
        assert_eq!(batches([1, 2, 3, 4, 1], |&n| n, 3), vec![vec![1, 2], vec![3], vec![4], vec![1]]);
        assert!(batches(Vec::<usize>::new(), |&n| n, 3).is_empty());
    }
}
