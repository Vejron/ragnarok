//! Static world collision, the stickman character controller and the dynamic vehicle bodies.
//!
//! The client builds the same scene in `client/src/sim/physics.ts` from the same world file and
//! constants, so its prediction hits the same walls. Native and WASM Rapier are not bit-exact,
//! so tiny differences are absorbed by reconciliation rather than assumed away.

use anyhow::{Context, Result};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::parry::utils::Array2;
use rapier3d::prelude::*;
use shared::consts::*;
use shared::movement::{begin_slide, decay_knock, desired_translation, settle_vertical};
use shared::protocol::InputFrame;
use shared::quant::from_cm;
use shared::world::{ObjId, WorldFile};
use std::collections::HashMap;

mod vehicle;
pub use vehicle::{CarControls, CarPose, half_extents};

/// A destructible piece of the static world: a building or a tree, by tile and index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Obj {
    Building(ObjId),
    Tree(ObjId),
}

/// Authoritative state of one stickman. Positions are the capsule centre, in metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Character {
    pub position: [f32; 3],
    pub yaw: f32,
    pub vy: f32,
    pub grounded: bool,
    /// Leftover knockback velocity `[x, z]`, m/s. Added to walking and decaying every tick.
    pub knock: [f32; 2],
    /// Id of the vehicle this character stands on, or 0. Riders are carried along with it.
    pub ride: u32,
}

pub struct Physics {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd: CCDSolver,
    parameters: IntegrationParameters,
    /// Vehicles by id.
    cars: std::collections::BTreeMap<u32, vehicle::Car>,
    controller: KinematicCharacterController,
    /// The same controller with snap-to-ground on, for the horizontal pass of a character that was
    /// standing: it keeps them on a slope they walk down instead of stepping off it.
    snapping: KinematicCharacterController,
    character_shape: Collider,
    /// Which building or tree each destructible static collider is.
    objects: HashMap<ColliderHandle, Obj>,
    handles: HashMap<Obj, ColliderHandle>,
    /// Objects destroyed since the last step. Their colliders go at the start of the next
    /// `step_cars`, which is also what makes the broad-phase forget them.
    removals: Vec<Obj>,
}

/// Height of the top of the invisible boundary walls around the playable square, metres above
/// the lowest ground. Mirrored by `BOUNDARY_HEIGHT` in `client/src/sim/physics.ts`.
const BOUNDARY_HEIGHT: f32 = 200.0;

/// How far below the lowest dry ground the slab under a terrain sits, metres: the importer's
/// `MAX_BED_BELOW`, the deepest a water bed is cut. Mirrored by `BED_FLOOR` in `client/src/sim/physics.ts`.
const BED_FLOOR: f32 = 12.0;

/// The static colliders every simulation of `world` uses: the ground slab, the terrain, four
/// boundary walls, the building mesh and the tree trunks. Mirrored by `buildStaticColliders` in
/// `client/src/sim/physics.ts`.
///
/// A building or tree comes with the [`Obj`] it is, for worlds that went through `assemble`.
pub fn static_colliders(world: &WorldFile) -> Result<Vec<(Collider, Option<Obj>)>> {
    let half = from_cm(world.half_extent);
    // Ground: a 1 m thick slab. On flat worlds its top is y = 0 and it is the whole ground;
    // elsewhere it sits `BED_FLOOR` below, under the lowest water bed, and only catches what
    // falls through a gap.
    let top = if world.ground.is_flat() { 0.0 } else { -BED_FLOOR };
    let mut colliders = vec![(ColliderBuilder::cuboid(half, 0.5, half).translation(Vector::new(0.0, top - 0.5, 0.0)).build(), None)];
    let ground = &world.ground;
    if !ground.is_flat() {
        let side = ground.side as usize;
        // Parry's heightfield is column-major with rows along Z: heights[(row, col)].
        let mut heights = Vec::with_capacity(side * side);
        for column in 0..side {
            for row in 0..side {
                heights.push(ground.sample(column as i64, row as i64));
            }
        }
        let width = (side - 1) as f32 * from_cm(ground.cell);
        let center = from_cm(ground.origin) + width / 2.0;
        colliders.push((
            ColliderBuilder::heightfield(Array2::new(side, side, heights), Vector::new(width, 1.0, width))
                .translation(Vector::new(center, 0.0, center))
                .build(),
            None,
        ));
    }
    let wall = BOUNDARY_HEIGHT / 2.0;
    for (x, z, hx, hz) in [(half, 0.0, 0.5, half), (-half, 0.0, 0.5, half), (0.0, half, half, 0.5), (0.0, -half, half, 0.5)] {
        colliders.push((ColliderBuilder::cuboid(hx, wall, hz).translation(Vector::new(x, wall, z)).build(), None));
    }
    let mesh = &world.collision;
    if !mesh.triangles.is_empty() {
        let vertex = |&[x, y, z]: &[i32; 3]| Vector::new(from_cm(x), from_cm(y), from_cm(z));
        // Each building is its own trimesh so it can be taken out alone. Whatever no building
        // claims (bridges, hand-built worlds) stays together in one.
        let mut claimed = vec![false; mesh.triangles.len()];
        for (index, &[start, end]) in world.building_triangles.iter().enumerate() {
            let (start, end) = (start as usize, (end as usize).min(mesh.triangles.len()));
            let Some(&id) = world.building_ids.get(index) else { break };
            if start >= end {
                continue;
            }
            claimed[start..end].iter_mut().for_each(|c| *c = true);
            // Only this building's vertices, renumbered from zero.
            let mut renumber: HashMap<u32, u32> = HashMap::new();
            let mut vertices = Vec::new();
            let triangles: Vec<[u32; 3]> = mesh.triangles[start..end]
                .iter()
                .map(|triangle| {
                    triangle.map(|v| {
                        *renumber.entry(v).or_insert_with(|| {
                            vertices.push(vertex(&mesh.vertices[v as usize]));
                            vertices.len() as u32 - 1
                        })
                    })
                })
                .collect();
            let trimesh = ColliderBuilder::trimesh(vertices, triangles).context("a building's collision mesh is invalid")?;
            colliders.push((trimesh.build(), Some(Obj::Building(id))));
        }
        let rest: Vec<[u32; 3]> = mesh.triangles.iter().zip(&claimed).filter(|&(_, &taken)| !taken).map(|(&t, _)| t).collect();
        if !rest.is_empty() {
            let vertices = mesh.vertices.iter().map(vertex).collect();
            let trimesh = ColliderBuilder::trimesh(vertices, rest).context("world collision mesh is invalid")?;
            colliders.push((trimesh.build(), None));
        }
    }
    // A tree is solid up to the crown: an upright cylinder on the ground at its foot.
    for (index, tree) in world.trees.iter().enumerate().filter(|(_, tree)| tree.solid) {
        let half_height = from_cm(tree.trunk_height()) / 2.0;
        let cylinder = ColliderBuilder::cylinder(half_height, from_cm(tree.trunk_radius()))
            .translation(Vector::new(from_cm(tree.x), from_cm(tree.base) + half_height, from_cm(tree.z)))
            .build();
        colliders.push((cylinder, world.tree_ids.get(index).map(|&id| Obj::Tree(id))));
    }
    Ok(colliders)
}

/// The stickman's character controller, without snap-to-ground.
fn controller() -> KinematicCharacterController {
    KinematicCharacterController {
        offset: CharacterLength::Absolute(CONTROLLER_OFFSET),
        slide: true,
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(AUTOSTEP_HEIGHT),
            min_width: CharacterLength::Absolute(AUTOSTEP_MIN_WIDTH),
            include_dynamic_bodies: false,
        }),
        max_slope_climb_angle: MAX_SLOPE_CLIMB_DEG.to_radians(),
        min_slope_slide_angle: MAX_SLOPE_CLIMB_DEG.to_radians(),
        snap_to_ground: None,
        ..Default::default()
    }
}

impl Physics {
    pub fn new(world: &WorldFile) -> Result<Self> {
        let mut physics = Self {
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd: CCDSolver::new(),
            parameters: IntegrationParameters { dt: TICK_DT, ..Default::default() },
            cars: Default::default(),
            snapping: KinematicCharacterController { snap_to_ground: Some(CharacterLength::Absolute(SNAP_DISTANCE)), ..controller() },
            controller: controller(),
            character_shape: ColliderBuilder::capsule_y(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS).build(),
            objects: HashMap::new(),
            handles: HashMap::new(),
            removals: Vec::new(),
        };
        for (collider, object) in static_colliders(world)? {
            let handle = physics.colliders.insert(collider);
            if let Some(object) = object {
                physics.objects.insert(handle, object);
                physics.handles.insert(object, handle);
            }
        }
        physics.step_pipeline();
        Ok(physics)
    }

    /// Runs the rigid-body pipeline for one tick. Before any vehicle exists this only builds the
    /// broad-phase over the static scene, which scene queries need.
    fn step_pipeline(&mut self) {
        self.pipeline.step(
            Vector::new(0.0, -vehicle::CAR_GRAVITY, 0.0),
            &self.parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd,
            &(),
            &(),
        );
    }

    /// Takes a building or tree out of the world. It stops colliding at the start of the next
    /// `step_cars`. Returns whether it was there to destroy.
    pub fn destroy(&mut self, object: Obj) -> bool {
        let there = self.handles.contains_key(&object) && !self.removals.contains(&object);
        if there {
            self.removals.push(object);
        }
        there
    }

    /// Every building and tree whose collider reaches within `radius` of `point`.
    pub fn objects_near(&self, point: [f32; 3], radius: f32) -> Vec<Obj> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::exclude_dynamic());
        let ball = Ball::new(radius);
        let mut found: Vec<Obj> = queries
            .intersect_shape(Pose::from_translation(Vector::from(point)), &ball)
            .filter_map(|(handle, _)| self.objects.get(&handle).copied())
            .collect();
        found.sort_by_key(|object| match *object {
            Obj::Building(id) => (0, id),
            Obj::Tree(id) => (1, id),
        });
        found.dedup();
        found
    }

    /// Which building or tree a collider is, if it is one.
    pub fn object_of(&self, collider: ColliderHandle) -> Option<Obj> {
        self.objects.get(&collider).copied()
    }

    /// Whether `shape` at `pose` touches the static world. Vehicles are ignored.
    pub fn touches_static(&self, shape: &dyn Shape, pose: Pose) -> bool {
        self.touches(shape, pose, QueryFilter::exclude_dynamic())
    }

    fn touches(&self, shape: &dyn Shape, pose: Pose, filter: QueryFilter) -> bool {
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            filter,
        );
        queries.intersect_shape(pose, shape).next().is_some()
    }

    /// Whether a standing character capsule centred at `position` fits there, among the static
    /// world and the vehicles.
    pub fn character_fits(&self, position: [f32; 3]) -> bool {
        !self.touches(self.character_shape.shape(), Pose::from_translation(Vector::from(position)), QueryFilter::default())
    }

    /// Advances one character by one tick of `input`. Characters do not collide with each other,
    /// which keeps the local player's prediction independent of where remote players really are.
    pub fn move_character(&self, character: &mut Character, input: &InputFrame, dt: f32) {
        self.move_character_scaled(character, input, dt, 1.0);
    }

    /// `move_character` with walking speed multiplied by `speed_scale` (aiming a gun slows you).
    pub fn move_character_scaled(&self, character: &mut Character, input: &InputFrame, dt: f32, speed_scale: f32) {
        character.knock = begin_slide(character.knock, character.grounded, input);
        let (vy, desired) = desired_translation(character.vy, character.grounded, character.knock, input, dt, speed_scale);
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            // Vehicles are solid. The client mirrors them as colliders at their last snapshot pose.
            QueryFilter::default(),
        );
        // Horizontal and vertical movement are separate passes. A single combined pass makes the
        // controller stall for a tick every few seconds while walking on flat ground (both native
        // and WASM Rapier), and every stall would be a misprediction.
        let shape = self.character_shape.shape();
        let start = Vector::from(character.position);
        let walker = if character.grounded && vy <= 0.0 { &self.snapping } else { &self.controller };
        let horizontal = walker.move_shape(dt, &queries, shape, &Pose::from_translation(start), Vector::new(desired[0], 0.0, desired[2]), |_| {});
        let middle = start + horizontal.translation;
        let vertical = self.controller.move_shape(dt, &queries, shape, &Pose::from_translation(middle), Vector::new(0.0, desired[1], 0.0), |_| {});
        let end = middle + vertical.translation;
        character.position = end.into();
        // The tick a jump starts the capsule is still touching the floor; it is not grounded.
        let grounded = vertical.grounded && vy <= 0.0;
        character.vy = settle_vertical(vy, desired[1], end.y - start.y, grounded);
        character.grounded = grounded;
        character.knock = decay_knock(character.knock, grounded, input.buttons() & BUTTON_CROUCH != 0);
        character.yaw = shared::quant::from_mrad(input.3);
    }
}

/// Capsule-centre height for a character standing on flat ground at y = 0.
pub const STANDING_Y: f32 = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET;

#[cfg(test)]
mod tests {
    use super::*;
    use shared::terrain::Ground;
    use shared::world::{CollisionMesh, WORLD_VERSION};

    fn flat_world() -> WorldFile {
        WorldFile { version: WORLD_VERSION, half_extent: 10_000, ..Default::default() }
    }

    /// A 2 m tall box from (5, 0, -1) to (6, 2, 1), as a closed triangle mesh.
    fn world_with_wall() -> WorldFile {
        let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
        let vertices = vec![
            v(5.0, 0.0, -1.0), v(6.0, 0.0, -1.0), v(6.0, 0.0, 1.0), v(5.0, 0.0, 1.0),
            v(5.0, 2.0, -1.0), v(6.0, 2.0, -1.0), v(6.0, 2.0, 1.0), v(5.0, 2.0, 1.0),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat_world() }
    }

    /// 200 m square of 5 m cells, height `f(x, z)` metres.
    fn terrain_world(f: impl Fn(f32, f32) -> f32) -> WorldFile {
        let side = 41u32;
        let heights = (0..side)
            .flat_map(|j| (0..side).map(move |i| (i, j)))
            .map(|(i, j)| (f(i as f32 * 5.0 - 100.0, j as f32 * 5.0 - 100.0) * 100.0).round() as i16)
            .collect();
        WorldFile { ground: Ground { origin: -10_000, cell: 500, side, heights }, ..flat_world() }
    }

    fn standing() -> Character {
        Character { position: [0.0, STANDING_Y, 0.0], grounded: true, ..Default::default() }
    }

    #[test]
    fn falls_and_lands_on_the_ground() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = Character { position: [0.0, 3.0, 0.0], ..Default::default() };
        for _ in 0..120 {
            physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
        }
        assert!(character.grounded);
        assert_eq!(character.vy, 0.0);
        assert!((character.position[1] - STANDING_Y).abs() < 0.05, "{:?}", character.position);
    }

    #[test]
    fn walks_forward_at_walk_speed() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = standing();
        let forward = InputFrame(1, 0, 127, 0, 0, 0, 0, 0);
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &forward, TICK_DT);
        }
        assert!((character.position[2] + WALK_SPEED).abs() < 0.1, "{:?}", character.position);
        assert!(character.grounded);
    }

    #[test]
    fn sliding_carries_a_runner_much_further_than_crouch_walking() {
        let physics = Physics::new(&flat_world()).unwrap();
        let held = InputFrame(1, 0, 127, 0, BUTTON_RUN | BUTTON_CROUCH, 0, 0, 0);
        let press = InputFrame(1, 0, 127, 0, BUTTON_RUN | BUTTON_CROUCH | BUTTON_SLIDE, 0, 0, 0);
        let distance = |first: &InputFrame| {
            let mut character = standing();
            physics.move_character(&mut character, first, TICK_DT);
            for _ in 0..TICK_RATE {
                physics.move_character(&mut character, &held, TICK_DT);
            }
            -character.position[2]
        };
        assert!(distance(&press) > distance(&held) + 3.0, "{} vs {}", distance(&press), distance(&held));
    }

    #[test]
    fn jumps_and_comes_back_down() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = standing();
        physics.move_character(&mut character, &InputFrame(1, 0, 0, 0, BUTTON_JUMP, 0, 0, 0), TICK_DT);
        assert!(!character.grounded);
        let mut peak: f32 = 0.0;
        for _ in 0..120 {
            physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
            peak = peak.max(character.position[1]);
        }
        assert!(peak > STANDING_Y + 0.8, "peak {peak}");
        assert!(character.grounded);
    }

    #[test]
    fn walls_block_movement() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        let mut character = standing();
        // Yaw -π/2 faces +X, towards the wall.
        let toward_wall = InputFrame(1, 0, 127, -1571, BUTTON_RUN, 0, 0, 0);
        for _ in 0..TICK_RATE * 2 {
            physics.move_character(&mut character, &toward_wall, TICK_DT);
        }
        let x = character.position[0];
        assert!(x < 5.0 - CAPSULE_RADIUS + 0.05 && x > 4.0, "x = {x}");
    }

    #[test]
    fn a_ray_names_the_building_and_destroying_it_opens_the_wall() {
        let id = ObjId { col: 3, row: 4, index: 7 };
        let mut world = world_with_wall();
        world.building_ids = vec![id];
        world.building_triangles = vec![[0, 12]];
        let mut physics = Physics::new(&world).unwrap();
        let (distance, hit) = physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).expect("the wall is there");
        assert!((distance - 5.0).abs() < 1e-3, "{distance}");
        assert_eq!(hit, Some(Obj::Building(id)));

        assert!(physics.destroy(Obj::Building(id)));
        assert!(!physics.destroy(Obj::Building(id)), "already going");
        // Still there until the step that removes it.
        assert!(physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).is_some());
        physics.step_cars(TICK_DT);
        assert!(physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).is_none());

        let mut character = standing();
        let toward_wall = InputFrame(1, 0, 127, -1571, BUTTON_RUN, 0, 0, 0);
        for _ in 0..TICK_RATE * 2 {
            physics.move_character(&mut character, &toward_wall, TICK_DT);
        }
        assert!(character.position[0] > 6.0, "walked through: x = {}", character.position[0]);
    }

    #[test]
    fn world_triangles_no_building_claims_stay_one_collider() {
        // Same wall, but a building claims only half of it: the other half must still block.
        let id = ObjId { col: 0, row: 0, index: 0 };
        let mut world = world_with_wall();
        world.building_ids = vec![id];
        world.building_triangles = vec![[0, 4]];
        let mut physics = Physics::new(&world).unwrap();
        physics.destroy(Obj::Building(id));
        physics.step_cars(TICK_DT);
        let still_solid = (0..20).any(|k| physics.cast_ray_object([0.0, 1.0, -0.9 + k as f32 * 0.09], [1.0, 0.0, 0.0], 20.0).is_some());
        assert!(still_solid, "the unclaimed triangles vanished with the building");
    }

    #[test]
    fn boundary_walls_keep_players_in() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = standing();
        let forward = InputFrame(1, 0, 127, 0, BUTTON_RUN, 0, 0, 0);
        for _ in 0..TICK_RATE * 20 {
            physics.move_character(&mut character, &forward, TICK_DT);
        }
        assert!(character.position[2] > -100.0, "{:?}", character.position);
    }

    /// A scripted run against the wall box: walking, wall sliding and jumps. `predict.spec.ts` replays
    /// the same script in WASM Rapier and checks it against `SCRIPTED_CHECKPOINTS`.
    pub fn scripted_input(seq: u32) -> InputFrame {
        let yaw = -1571 + ((seq as i32 * 13) % 1200 - 600) as i16;
        let buttons = if seq % 90 == 30 { BUTTON_JUMP } else if seq > 150 { BUTTON_RUN } else { 0 };
        InputFrame(seq, (seq % 7) as i8 * 10 - 30, 127, yaw, buttons, 0, 0, 0)
    }

    /// `(tick, position)` after `scripted_input`, mirrored in `predict.spec.ts`.
    const SCRIPTED_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (30, [1.8138, 0.9722, 0.7480]),
        (60, [3.7758, 1.4722, 0.7701]),
        (90, [4.6299, 0.8649, 0.0827]),
        (300, [4.6295, 0.9728, 0.4872]),
    ];

    #[test]
    fn scripted_run_matches_fixture() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        let mut character = standing();
        for seq in 1..=300 {
            physics.move_character(&mut character, &scripted_input(seq), TICK_DT);
            if let Some((_, expected)) = SCRIPTED_CHECKPOINTS.iter().find(|(tick, _)| *tick == seq) {
                let close = character.position.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 0.001);
                assert!(close, "tick {seq}: {:?}", character.position);
            }
        }
        assert!(!character.grounded);
    }

    #[test]
    fn height_at_is_the_surface_the_collider_has() {
        let hills = |x: f32, z: f32| 8.0 + 6.0 * (x * 0.07).sin() * (z * 0.05).cos();
        let world = terrain_world(hills);
        let physics = Physics::new(&world).unwrap();
        for k in 0..200 {
            let (x, z) = (-95.0 + (k * 37 % 190) as f32 + 0.37, -95.0 + (k * 53 % 190) as f32 + 0.61);
            let hit = physics.cast_ray([x, 100.0, z], [0.0, -1.0, 0.0], 200.0, 0).expect("terrain below");
            let surface = 100.0 - hit;
            let sampled = world.ground.height_at(x, z);
            assert!((surface - sampled).abs() < 0.01, "({x}, {z}): ray {surface}, sampler {sampled}");
        }
    }

    #[test]
    fn walking_down_and_up_a_slope_stays_grounded() {
        // A 20° ramp rising towards +Z.
        let slope = 20.0f32.to_radians().tan();
        let world = terrain_world(|_, z| (z + 100.0) * slope);
        let physics = Physics::new(&world).unwrap();
        for (start_z, input_z) in [(40.0, 127), (-40.0, -127)] {
            let ground = world.ground.height_at(0.0, start_z);
            let mut character = Character { position: [0.0, ground + STANDING_Y, start_z], grounded: true, ..Default::default() };
            let mut airborne = 0;
            for seq in 0..TICK_RATE * 4 {
                let input = InputFrame(seq, 0, input_z, 0, BUTTON_RUN, 0, 0, 0);
                physics.move_character(&mut character, &input, TICK_DT);
                airborne += !character.grounded as u32;
            }
            let ground = world.ground.height_at(character.position[0], character.position[2]);
            assert!((character.position[1] - ground - STANDING_Y).abs() < 0.1, "{:?} on {ground}", character.position);
            assert!(airborne <= TICK_RATE / 10, "airborne {airborne} of {} ticks from z = {start_z}", TICK_RATE * 4);
        }
    }

    /// The slope the cross-language fixture runs on: a 20° ramp rising towards +Z, in 5 m cells.
    /// `predict.spec.ts` builds the same heights.
    fn ramp_world() -> WorldFile {
        let slope = 20.0f32.to_radians().tan();
        terrain_world(|_, z| (z + 100.0) * slope)
    }

    /// Running downhill with strafing, a jump and a stop: `ramp_world`'s fixture input.
    pub fn ramp_input(seq: u32) -> InputFrame {
        let buttons = if seq == 50 { BUTTON_RUN | BUTTON_JUMP } else if seq < 150 { BUTTON_RUN } else { 0 };
        InputFrame(seq, (seq % 5) as i8 * 10 - 20, if seq < 150 { 127 } else { 0 }, (seq as i16 / 3) % 200, buttons, 0, 0, 0)
    }

    /// `(tick, position)` after `ramp_input` from `RAMP_START`, mirrored in `predict.spec.ts`.
    const RAMP_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (30, [-0.0180, 50.4969, 36.2721]),
        (60, [-0.0733, 50.4621, 32.5448]),
        (120, [-0.2956, 46.4279, 25.0934]),
        (240, [-0.4475, 45.1151, 21.4869]),
    ];
    const RAMP_START: [f32; 2] = [0.0, 40.0];

    #[test]
    fn ramp_run_matches_fixture() {
        let world = ramp_world();
        let physics = Physics::new(&world).unwrap();
        let ground = world.ground.height_at(RAMP_START[0], RAMP_START[1]);
        let mut character = Character { position: [RAMP_START[0], ground + STANDING_Y, RAMP_START[1]], grounded: true, ..Default::default() };
        for seq in 1..=240 {
            physics.move_character(&mut character, &ramp_input(seq), TICK_DT);
            if let Some((_, expected)) = RAMP_CHECKPOINTS.iter().find(|(tick, _)| *tick == seq) {
                let close = character.position.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 0.001);
                assert!(close, "tick {seq}: {:?}", character.position);
            }
        }
    }

    #[test]
    fn simulation_is_deterministic() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        let run = || {
            let mut character = standing();
            for seq in 0..300u32 {
                let input = InputFrame(seq, 90, 127, (seq as i16 * 7) % 3000, if seq % 50 == 0 { BUTTON_JUMP } else { 0 }, 0, 0, 0);
                physics.move_character(&mut character, &input, TICK_DT);
            }
            character
        };
        assert_eq!(run(), run());
    }
}
