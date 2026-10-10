//! Static world collision, the stickman character controller and the dynamic vehicle bodies.
//!
//! The client builds the same scene in `client/src/sim/physics.ts` from the same world file and
//! constants, so its prediction hits the same walls. Native and WASM Rapier are not bit-exact,
//! so tiny differences are absorbed by reconciliation rather than assumed away.

use anyhow::{Context, Result};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude::*;
use shared::consts::*;
use shared::movement::{decay_knock, desired_translation, settle_vertical};
use shared::protocol::InputFrame;
use shared::quant::from_cm;
use shared::world::WorldFile;

mod vehicle;
pub use vehicle::{CarControls, CarPose, half_extents};

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
    character_shape: Collider,
}

/// Height of the invisible boundary walls around the playable square, metres.
const BOUNDARY_HEIGHT: f32 = 50.0;

/// The static colliders every simulation of `world` uses: the ground slab, four boundary walls,
/// and the building mesh. Mirrored by `buildStaticColliders` in `client/src/sim/physics.ts`.
pub fn static_colliders(world: &WorldFile) -> Result<Vec<Collider>> {
    let half = from_cm(world.half_extent);
    let mut colliders = vec![
        // Ground: a 1 m thick slab whose top face is y = 0.
        ColliderBuilder::cuboid(half, 0.5, half).translation(Vector::new(0.0, -0.5, 0.0)).build(),
    ];
    let wall = BOUNDARY_HEIGHT / 2.0;
    for (x, z, hx, hz) in [(half, 0.0, 0.5, half), (-half, 0.0, 0.5, half), (0.0, half, half, 0.5), (0.0, -half, half, 0.5)] {
        colliders.push(ColliderBuilder::cuboid(hx, wall, hz).translation(Vector::new(x, wall, z)).build());
    }
    let mesh = &world.collision;
    if !mesh.triangles.is_empty() {
        let vertices = mesh
            .vertices
            .iter()
            .map(|&[x, y, z]| Vector::new(from_cm(x), from_cm(y), from_cm(z)))
            .collect();
        let trimesh = ColliderBuilder::trimesh(vertices, mesh.triangles.clone())
            .context("world collision mesh is invalid")?;
        colliders.push(trimesh.build());
    }
    Ok(colliders)
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
            controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(CONTROLLER_OFFSET),
                slide: true,
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(AUTOSTEP_HEIGHT),
                    min_width: CharacterLength::Absolute(AUTOSTEP_MIN_WIDTH),
                    include_dynamic_bodies: false,
                }),
                max_slope_climb_angle: MAX_SLOPE_CLIMB_DEG.to_radians(),
                min_slope_slide_angle: MAX_SLOPE_CLIMB_DEG.to_radians(),
                // Snapping is off: with the two-pass move below it slowly sinks the capsule into the floor.
                snap_to_ground: None,
                ..Default::default()
            },
            character_shape: ColliderBuilder::capsule_y(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS).build(),
        };
        for collider in static_colliders(world)? {
            physics.colliders.insert(collider);
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
        let (vy, desired) = desired_translation(character.vy, character.grounded, character.knock, input, dt);
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
        let horizontal = self.controller.move_shape(dt, &queries, shape, &Pose::from_translation(start), Vector::new(desired[0], 0.0, desired[2]), |_| {});
        let middle = start + horizontal.translation;
        let vertical = self.controller.move_shape(dt, &queries, shape, &Pose::from_translation(middle), Vector::new(0.0, desired[1], 0.0), |_| {});
        let end = middle + vertical.translation;
        character.position = end.into();
        // The tick a jump starts the capsule is still touching the floor; it is not grounded.
        let grounded = vertical.grounded && vy <= 0.0;
        character.vy = settle_vertical(vy, desired[1], end.y - start.y, grounded);
        character.grounded = grounded;
        character.knock = decay_knock(character.knock, grounded);
        character.yaw = shared::quant::from_mrad(input.3);
    }
}

/// Capsule-centre height for a character standing on flat ground at y = 0.
pub const STANDING_Y: f32 = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET;

#[cfg(test)]
mod tests {
    use super::*;
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
        let forward = InputFrame(1, 0, 127, 0, 0, 0, 0);
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &forward, TICK_DT);
        }
        assert!((character.position[2] + WALK_SPEED).abs() < 0.1, "{:?}", character.position);
        assert!(character.grounded);
    }

    #[test]
    fn jumps_and_comes_back_down() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = standing();
        physics.move_character(&mut character, &InputFrame(1, 0, 0, 0, BUTTON_JUMP, 0, 0), TICK_DT);
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
        let toward_wall = InputFrame(1, 0, 127, -1571, BUTTON_RUN, 0, 0);
        for _ in 0..TICK_RATE * 2 {
            physics.move_character(&mut character, &toward_wall, TICK_DT);
        }
        let x = character.position[0];
        assert!(x < 5.0 - CAPSULE_RADIUS + 0.05 && x > 4.0, "x = {x}");
    }

    #[test]
    fn boundary_walls_keep_players_in() {
        let physics = Physics::new(&flat_world()).unwrap();
        let mut character = standing();
        let forward = InputFrame(1, 0, 127, 0, BUTTON_RUN, 0, 0);
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
        InputFrame(seq, (seq % 7) as i8 * 10 - 30, 127, yaw, buttons, 0, 0)
    }

    /// `(tick, position)` after `scripted_input`, mirrored in `predict.spec.ts`.
    const SCRIPTED_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (30, [1.8128, 0.9720, 0.7471]),
        (60, [3.7747, 1.4720, 0.7692]),
        (90, [4.6299, 0.8701, 0.0616]),
        (300, [4.6299, 0.9729, 0.5537]),
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
    fn simulation_is_deterministic() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        let run = || {
            let mut character = standing();
            for seq in 0..300u32 {
                let input = InputFrame(seq, 90, 127, (seq as i16 * 7) % 3000, if seq % 50 == 0 { BUTTON_JUMP } else { 0 }, 0, 0);
                physics.move_character(&mut character, &input, TICK_DT);
            }
            character
        };
        assert_eq!(run(), run());
    }
}
