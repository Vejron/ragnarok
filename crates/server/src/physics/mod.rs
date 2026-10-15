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
pub use vehicle::{CarCondition, CarControls, CarImpact, CarPose, Strike, half_extents};

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
    /// Lying down. The capsule is then the prone ball, with the feet where they were standing.
    pub prone: bool,
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
    /// The shape of a character lying down. Same controller; only the collider differs.
    prone_shape: Collider,
    /// Which building or tree each destructible static collider is.
    objects: HashMap<ColliderHandle, Obj>,
    handles: HashMap<Obj, ColliderHandle>,
    /// Objects destroyed since the last step. Their colliders go at the start of the next
    /// `step_cars`, which is also what makes the broad-phase forget them.
    removals: Vec<Obj>,
    /// How far the playable square reaches from the centre to each side, metres. Planes turn back near it.
    bound: f32,
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
    for (x, z, hx, hz) in boundary_walls(world) {
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
        // A rock is solid but is no `Obj`: nothing damages or fells it.
        let object = if tree.destructible() { world.tree_ids.get(index).map(|&id| Obj::Tree(id)) } else { None };
        colliders.push((cylinder, object));
    }
    Ok(colliders)
}

/// The boundary walls as `(x, z, half width, half depth)`, metres: the four sides of the square
/// for a world that fills it, else a wall along every edge between a tile of the world and one that
/// is not (or the edge of the square), merged into runs. Mirrored by `boundaryWalls` in
/// `client/src/sim/physics.ts`.
fn boundary_walls(world: &WorldFile) -> Vec<(f32, f32, f32, f32)> {
    let half = from_cm(world.half_extent);
    let grid = world.grid() as i64;
    if world.inside.is_empty() || world.tile_size <= 0 {
        return vec![(half, 0.0, 0.5, half), (-half, 0.0, 0.5, half), (0.0, half, half, 0.5), (0.0, -half, half, 0.5)];
    }
    let side = from_cm(world.tile_size);
    let inside = |col: i64, row: i64| col >= 0 && row >= 0 && col < grid && row < grid && world.inside[(row * grid + col) as usize];
    let at = |i: i64| -half + i as f32 * side;
    let mut walls = Vec::new();
    // Edges along X sit between rows `line - 1` and `line`; edges along Z between columns.
    for along_x in [true, false] {
        for line in 0..=grid {
            let mut start: Option<i64> = None;
            for i in 0..=grid {
                let edge = i < grid && {
                    let (before, after) = if along_x { (inside(i, line - 1), inside(i, line)) } else { (inside(line - 1, i), inside(line, i)) };
                    before != after
                };
                match (edge, start) {
                    (true, None) => start = Some(i),
                    (false, Some(first)) => {
                        let (mid, reach) = ((at(first) + at(i)) / 2.0, (at(i) - at(first)) / 2.0);
                        walls.push(if along_x { (mid, at(line), reach, 0.5) } else { (at(line), mid, 0.5, reach) });
                        start = None;
                    }
                    _ => {}
                }
            }
        }
    }
    walls
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
            prone_shape: ColliderBuilder::ball(PRONE_RADIUS).build(),
            objects: HashMap::new(),
            handles: HashMap::new(),
            removals: Vec::new(),
            bound: from_cm(world.half_extent),
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
        self.fits(position, false)
    }

    /// Whether the character's body, standing or prone, centred at `position` fits there.
    fn fits(&self, position: [f32; 3], prone: bool) -> bool {
        !self.touches(self.shape(prone).shape(), Pose::from_translation(Vector::from(position)), QueryFilter::default())
    }

    fn shape(&self, prone: bool) -> &Collider {
        if prone { &self.prone_shape } else { &self.character_shape }
    }

    /// Advances one character by one tick of `input`. Characters do not collide with each other,
    /// which keeps the local player's prediction independent of where remote players really are.
    #[cfg(test)]
    pub fn move_character(&self, character: &mut Character, input: &InputFrame, dt: f32) {
        self.move_character_scaled(character, input, dt, 1.0);
    }

    /// `move_character` with walking speed multiplied by `speed_scale` (aiming a gun slows you).
    pub fn move_character_scaled(&self, character: &mut Character, input: &InputFrame, dt: f32, speed_scale: f32) {
        self.change_stance(character, input.buttons() & BUTTON_PRONE != 0);
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
        let shape = self.shape(character.prone).shape();
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
        character.yaw = shared::quant::from_mrad(input.yaw);
    }

    /// Lies down or gets up, keeping the feet where they are. Getting up is refused while the
    /// standing body would not fit, so the player stays low until there is room.
    fn change_stance(&self, character: &mut Character, wants_prone: bool) {
        if wants_prone == character.prone {
            return;
        }
        let drop = STAND_CENTRE - PRONE_CENTRE;
        let mut raised = character.position;
        raised[1] += if wants_prone { -drop } else { drop };
        if wants_prone || self.fits(raised, false) {
            character.position = raised;
            character.prone = wants_prone;
        }
    }
}

/// Capsule-centre height for a character standing on flat ground at y = 0.
pub const STANDING_Y: f32 = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET;

#[cfg(test)]
mod tests;
