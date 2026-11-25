//! Regrowth (REGROWTH.md): felled trees grow back, a stage at a time, and broken pieces of fences and
//! walls are mended, each only where no player can see it happen and nothing stands in the way. The
//! physics gets the colliders back, and everyone is told (`Regrown`).

use super::*;
use shared::protocol::RegrownObject;

/// When `stage` may follow on something felled `since`, ticks after it fell, for what it is: a tree
/// grows through every stage, a piece of fence or wall is mended whole at once, a building never.
fn next_stage(object: Obj, stage: u8) -> Option<(u8, u32)> {
    let seconds = match (object, stage) {
        (Obj::Tree(_), GROWN_LOG) => (GROWN_SAPLING, REGROW_SAPLING_SECONDS),
        (Obj::Tree(_), GROWN_SAPLING) => (GROWN_YOUNG, REGROW_YOUNG_SECONDS),
        (Obj::Tree(_), GROWN_YOUNG) => (GROWN_WHOLE, REGROW_WHOLE_SECONDS),
        (Obj::Line(_), GROWN_LOG) => (GROWN_WHOLE, LINE_MEND_SECONDS),
        _ => return None,
    };
    Some((seconds.0, seconds.1 * TICK_RATE))
}

impl World {
    /// Whether anything felled could grow back or be mended, some time.
    pub fn regrowing(&self) -> bool {
        self.destroyed.iter().any(|felled| next_stage(felled.object, felled.stage).is_some())
    }

    /// Takes every felled tree and broken piece that is due a step further (REGROWTH.md): one stage
    /// each, and only where no eye in `eyes` (`(x, z)`, metres: the players) is within `HEAL_CLEAR`
    /// of it and, for a step that makes it solid, nothing (`things`, `(x, z)`, metres, and the placed
    /// cubes) is within `REGROW_CLEARANCE` of it. A tree also waits for dug ground under it to heal.
    /// Returns what grew.
    pub fn regrow(&mut self, eyes: &[[f32; 2]], things: &[[f32; 2]]) -> Vec<RegrownObject> {
        let mut grown = Vec::new();
        let mut cubes: Option<Vec<[f32; 2]>> = None;
        for at in 0..self.destroyed.len() {
            let felled = self.destroyed[at];
            let Some((stage, after)) = next_stage(felled.object, felled.stage) else { continue };
            if self.tick.wrapping_sub(felled.since) < after {
                continue;
            }
            let Some((foot, reach)) = self.footprint(felled.object) else { continue };
            let within = |at: &[f32; 2], margin: f32| (at[0] - foot[0]).hypot(at[1] - foot[1]) - reach < margin;
            if eyes.iter().any(|eye| within(eye, HEAL_CLEAR)) {
                continue;
            }
            if matches!(felled.object, Obj::Tree(_)) && self.on_dug_ground(foot) {
                continue;
            }
            if stage >= GROWN_YOUNG {
                if things.iter().any(|thing| within(thing, REGROW_CLEARANCE)) {
                    continue;
                }
                let cubes = cubes.get_or_insert_with(|| {
                    self.construct.cubes().map(|(pos, _)| shared::construct::centre(pos)).map(|[x, _, z]| [from_cm(x), from_cm(z)]).collect()
                });
                if cubes.iter().any(|cube| within(cube, REGROW_CLEARANCE)) {
                    continue;
                }
                let scale = if stage == GROWN_YOUNG { TREE_YOUNG_SCALE } else { 1.0 };
                let Some(collider) = self.collider_of(felled.object, scale) else { continue };
                self.physics.restore(felled.object, collider);
            }
            self.destroyed[at].stage = stage;
            let (kind, id) = match felled.object {
                Obj::Tree(id) => (OBJ_TREE, id),
                Obj::Line(id) => (OBJ_LINE, id),
                Obj::Building(id) => (OBJ_BUILDING, id),
            };
            grown.push(RegrownObject { kind, col: id.col, row: id.row, index: id.index, stage });
        }
        // What stands whole again is no longer felled.
        let whole: Vec<Obj> = self.destroyed.iter().filter(|felled| felled.stage == GROWN_WHOLE).map(|felled| felled.object).collect();
        for object in whole {
            self.felled.remove(&object);
            self.object_hp.remove(&object);
        }
        self.destroyed.retain(|felled| felled.stage != GROWN_WHOLE);
        grown
    }

    /// Where `object` stands, `(x, z)` metres, and how far it reaches from there, metres: a tree's
    /// trunk, a piece of fence or wall from its middle to its ends.
    fn footprint(&self, object: Obj) -> Option<([f32; 2], f32)> {
        match object {
            Obj::Tree(id) => self.tree(id).map(|tree| ([from_cm(tree.x), from_cm(tree.z)], from_cm(tree.trunk_radius()))),
            Obj::Line(id) => self.line_piece(id).map(|piece| {
                let [x, _, z] = piece.centre();
                ([x, z], (piece.b[0] - piece.a[0]).hypot(piece.b[1] - piece.a[1]) / 2.0)
            }),
            Obj::Building(_) => None,
        }
    }

    /// The collider `object` stands with again, `scale` of its size for a young tree.
    fn collider_of(&self, object: Obj, scale: f32) -> Option<sim::Collider> {
        match object {
            Obj::Tree(id) => self.tree(id).filter(|tree| tree.solid).map(|tree| sim::tree_collider(tree, scale)),
            Obj::Line(id) => self.line_piece(id).map(sim::fences::piece_collider),
            Obj::Building(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Tree, WORLD_VERSION};

    /// A world of one destructible pine at `(10, 0)` on flat ground, and a game around it.
    fn wood() -> (Game, ObjId) {
        let pine = Tree { x: 1000, z: 0, base: 0, height: 1500, crown: 300, kind: shared::world::TREE_PINE, solid: true };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], tile_size: 20_000, trees: vec![pine], tree_ids: vec![ObjId { col: 0, row: 0, index: 0 }], ..Default::default() };
        let game = Game::new(map, 0).unwrap();
        (game, ObjId { col: 0, row: 0, index: 0 })
    }

    /// Fells tree `id` outright.
    fn fell(game: &mut Game, id: ObjId) {
        assert!(crate::mechanics::damage_object(&mut game.world, Obj::Tree(id), u16::MAX, [0.0, 1.0, 0.0]));
        game.world.physics.step_cars(TICK_DT);
    }

    /// Whether a ray along +x through the trunk, a metre up, hits the tree.
    fn solid(game: &Game) -> bool {
        matches!(game.world.physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 30.0), Some((_, Some(Obj::Tree(_)))))
    }

    /// Somewhere well out of sight of the pine.
    const FAR: [f32; 2] = [2_000.0, 2_000.0];

    fn stage(game: &Game) -> Option<u8> {
        game.world.destroyed.first().map(|felled| felled.stage)
    }

    #[test]
    fn a_felled_tree_grows_back_through_sapling_and_young_while_nobody_is_near() {
        let (mut game, id) = wood();
        assert!(solid(&game));
        fell(&mut game, id);
        assert!(!solid(&game));
        game.world.tick += REGROW_SAPLING_SECONDS * TICK_RATE - 1;
        assert!(game.world.regrow(&[FAR], &[]).is_empty(), "not yet");
        game.world.tick += 1;
        assert_eq!(game.world.regrow(&[FAR], &[]), vec![RegrownObject { kind: OBJ_TREE, col: 0, row: 0, index: 0, stage: GROWN_SAPLING }]);
        game.world.physics.step_cars(TICK_DT);
        assert!(!solid(&game), "a sapling is not solid");
        game.world.tick = REGROW_YOUNG_SECONDS * TICK_RATE + 1;
        assert_eq!(stage(&game), Some(GROWN_SAPLING));
        game.world.regrow(&[FAR], &[]);
        assert_eq!(stage(&game), Some(GROWN_YOUNG));
        game.world.physics.step_cars(TICK_DT);
        assert!(solid(&game), "a young tree is");
        game.world.tick = REGROW_WHOLE_SECONDS * TICK_RATE + 1;
        game.world.regrow(&[FAR], &[]);
        game.world.physics.step_cars(TICK_DT);
        assert!(game.world.destroyed.is_empty() && game.world.felled.is_empty());
        assert!(solid(&game));
    }

    #[test]
    fn a_tree_grows_only_one_stage_a_check_however_long_it_waited() {
        let (mut game, id) = wood();
        fell(&mut game, id);
        game.world.tick += REGROW_WHOLE_SECONDS * TICK_RATE * 4;
        game.world.regrow(&[FAR], &[]);
        assert_eq!(stage(&game), Some(GROWN_SAPLING));
        game.world.regrow(&[FAR], &[]);
        assert_eq!(stage(&game), Some(GROWN_YOUNG));
    }

    #[test]
    fn a_tree_does_not_regrow_while_a_player_could_see_it() {
        let (mut game, id) = wood();
        fell(&mut game, id);
        game.world.tick += REGROW_SAPLING_SECONDS * TICK_RATE;
        assert!(game.world.regrow(&[[10.0, 350.0]], &[]).is_empty());
        assert_eq!(game.world.regrow(&[[10.0, 450.0]], &[]).len(), 1);
    }

    #[test]
    fn a_tree_does_not_grow_back_on_dug_ground_until_it_heals() {
        let ground = shared::terrain::Ground { origin: -10_000, cell: 500, side: 41, heights: vec![300; 41 * 41] };
        let pine = Tree { x: 1000, z: 0, base: 300, height: 1500, crown: 300, kind: shared::world::TREE_PINE, solid: true };
        let id = ObjId { col: 0, row: 0, index: 0 };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], tile_size: 20_000, ground, trees: vec![pine], tree_ids: vec![id], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        fell(&mut game, id);
        assert!(game.world.dig_crater([10.0, 3.0, 0.0], 5.6, 1.0, crate::game::Strike::DOWN));
        game.world.tick += REGROW_SAPLING_SECONDS * TICK_RATE;
        assert!(game.world.regrow(&[FAR], &[]).is_empty(), "it would stand in the air over the crater");
        game.world.tick += CRATER_HEAL_SECONDS * TICK_RATE;
        assert_eq!(game.world.heal_craters(&[FAR], &[]).len(), 1);
        assert_eq!(game.world.regrow(&[FAR], &[]).len(), 1);
    }

    #[test]
    fn a_broken_piece_of_fence_is_mended_unseen_and_is_solid_again() {
        use shared::world::{LINE_FENCE, Line};
        let line = Line { kind: LINE_FENCE, points: (-2..=2).map(|i| [1000, i * 300]).collect(), base: vec![0; 5], end: true };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], lines: vec![(2, 3, line)], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        let piece = Obj::Line(ObjId { col: 2, row: 3, index: 1 });
        let there = |game: &Game| game.world.physics.objects_near([10.0, 0.5, -1.5], 1.0).contains(&piece);
        assert!(there(&game));
        assert!(crate::mechanics::damage_object(&mut game.world, piece, u16::MAX, [0.0, 0.5, -1.5]));
        game.world.physics.step_cars(TICK_DT);
        assert!(!there(&game));
        game.world.tick += LINE_MEND_SECONDS * TICK_RATE;
        assert!(game.world.regrow(&[[10.0, 300.0]], &[]).is_empty(), "a player could see it");
        assert_eq!(game.world.regrow(&[FAR], &[]), vec![RegrownObject { kind: OBJ_LINE, col: 2, row: 3, index: 1, stage: GROWN_WHOLE }]);
        game.world.physics.step_cars(TICK_DT);
        assert!(there(&game), "solid again");
        assert!(game.world.destroyed.is_empty() && !game.world.felled.contains(&piece));
    }

    #[test]
    fn a_tree_does_not_become_solid_where_something_stands() {
        let (mut game, id) = wood();
        fell(&mut game, id);
        game.world.tick += REGROW_SAPLING_SECONDS * TICK_RATE;
        // A sapling is not solid: a vehicle on the stump does not stop it.
        assert_eq!(game.world.regrow(&[FAR], &[[10.0, 0.5]]).len(), 1);
        game.world.tick = REGROW_YOUNG_SECONDS * TICK_RATE + 1;
        assert!(game.world.regrow(&[FAR], &[[10.0, 0.5]]).is_empty(), "a vehicle stands on it");
        let cube = shared::construct::cell_at([10.0, 0.5, 0.0]);
        game.world.construct.place(cube, shared::construct::Cube { colour: 0, material: 0 });
        assert!(game.world.regrow(&[FAR], &[]).is_empty(), "a cube is built on it");
        game.world.construct.remove(cube);
        assert_eq!(game.world.regrow(&[FAR], &[[13.0, 0.0]]).len(), 1, "something a few metres off does not matter");
    }

    #[test]
    fn a_young_tree_can_be_felled_again_and_starts_over() {
        let (mut game, id) = wood();
        fell(&mut game, id);
        game.world.tick = REGROW_YOUNG_SECONDS * TICK_RATE + 1;
        game.world.regrow(&[FAR], &[]);
        game.world.regrow(&[FAR], &[]);
        assert_eq!(stage(&game), Some(GROWN_YOUNG));
        game.world.physics.step_cars(TICK_DT);
        fell(&mut game, id);
        assert_eq!(game.world.destroyed.len(), 1);
        assert_eq!(stage(&game), Some(GROWN_LOG));
        assert_eq!(game.world.destroyed[0].since, game.world.tick);
        assert!(!solid(&game));
    }

    #[test]
    fn a_joiner_hears_of_each_felled_tree_at_its_stage() {
        let (mut game, id) = wood();
        fell(&mut game, id);
        game.world.tick += REGROW_SAPLING_SECONDS * TICK_RATE;
        game.world.regrow(&[FAR], &[]);
        let caught_up: Vec<DestroyedObject> = game
            .catch_up()
            .into_iter()
            .filter(|bytes| peek_tag(bytes) == Some(S_DESTROYED))
            .filter_map(|bytes| decode::<Destroyed>(&bytes))
            .flat_map(|d| d.objects)
            .collect();
        assert_eq!(caught_up.len(), 1);
        assert_eq!(caught_up[0].stage, GROWN_SAPLING);
    }
}
