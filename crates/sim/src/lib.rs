//! Static world collision, the stickman character controller and the dynamic vehicle bodies.
//!
//! The client builds the same scene in `client/src/sim/physics.ts` from the same world file and
//! constants, so its prediction hits the same walls. Native and WASM Rapier are not bit-exact,
//! so tiny differences are absorbed by reconciliation rather than assumed away.

use anyhow::{Context, Result};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::parry::utils::Array2;
use rapier3d::prelude::*;
use shared::carve::{Shell, Tri};
use shared::consts::*;
use shared::movement::{begin_slide, decay_knock, desired_translation, settle_vertical, steer_chute, toggle_chute};
use shared::protocol::InputFrame;
use shared::quant::from_cm;
use shared::world::{ObjId, WorldFile};
use std::collections::HashMap;

mod stream;
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
    /// Hanging under an open parachute. `knock` is then the glide.
    pub chute: bool,
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
    /// Buildings to be given a new shape (`solid_of`), or taken out when nothing is left of them,
    /// at the start of the next step.
    replacements: Vec<(Obj, Option<SharedShape>)>,
    /// The ground, `[min x, min z, max x, max z]` in metres, under every static collider taken out or
    /// reshaped since `take_changed` was last called: where the bots' walking grid is out of date.
    changed: Vec<[f32; 4]>,
    /// How far the playable square reaches from the centre to each side, metres. Planes turn back near it.
    bound: f32,
    /// The static colliders of each streamed tile (`add_tile`), by `(col, row)`. Empty on the server,
    /// which has every tile from the start.
    tiles: HashMap<(u16, u16), Vec<ColliderHandle>>,
    /// Vehicles as solid stand-ins at their snapshot pose (`set_obstacle`), by id: what the client's
    /// prediction walks into. Empty on the server, which simulates the vehicles themselves.
    obstacles: std::collections::BTreeMap<u32, stream::Obstacle>,
}

/// Height of the top of the invisible boundary walls around the playable square, metres above
/// the lowest ground. Mirrored by `BOUNDARY_HEIGHT` in `client/src/sim/physics.ts`.
const BOUNDARY_HEIGHT: f32 = 200.0;

/// The collision group of a tree that can be felled. A tank's hull leaves this group out of what it
/// touches, so it drives through such a trunk (felling it, see `plow_trees`) instead of stopping on it.
/// Mirrored by `TREE_GROUPS` and `HEAVY_GROUPS` in `client/src/sim/physics.ts`.
pub const TREE_GROUP: Group = Group::GROUP_2;

/// What a vehicle too heavy to be stopped by a tree touches: everything but felled-able trunks.
pub fn plows_trees() -> InteractionGroups {
    InteractionGroups::new(Group::ALL, Group::ALL.difference(TREE_GROUP), InteractionTestMode::And)
}

/// How far below the lowest dry ground the slab under a terrain sits, metres: the importer's
/// `MAX_BED_BELOW`, the deepest a water bed is cut. Mirrored by `BED_FLOOR` in `client/src/sim/physics.ts`.
const BED_FLOOR: f32 = 12.0;

/// The static colliders every simulation of `world` uses: the ground slab, the terrain, four
/// boundary walls, the building mesh and the tree trunks. Mirrored by `buildStaticColliders` in
/// `client/src/sim/physics.ts`.
///
/// A building or tree comes with the [`Obj`] it is, for worlds that went through `assemble`.
pub fn static_colliders(world: &WorldFile) -> Result<Vec<(Collider, Option<Obj>)>> {
    let terrain = !world.ground.is_flat();
    let mut colliders = vec![(slab(world, terrain), None)];
    let ground = &world.ground;
    if terrain {
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
    colliders.extend(walls(world).into_iter().map(|wall| (wall, None)));
    colliders.extend(object_colliders(world)?);
    Ok(colliders)
}

/// Ground: a 1 m thick slab. On flat worlds its top is y = 0 and it is the whole ground; under a
/// terrain it sits `BED_FLOOR` below, under the lowest water bed, and only catches what falls through
/// a gap.
fn slab(world: &WorldFile, terrain: bool) -> Collider {
    let half = from_cm(world.half_extent);
    let top = if terrain { -BED_FLOOR } else { 0.0 };
    ColliderBuilder::cuboid(half, 0.5, half).translation(Vector::new(0.0, top - 0.5, 0.0)).build()
}

/// The walls around the playable square (`boundary_walls`).
fn walls(world: &WorldFile) -> Vec<Collider> {
    let wall = BOUNDARY_HEIGHT / 2.0;
    boundary_walls(world).into_iter().map(|(x, z, hx, hz)| ColliderBuilder::cuboid(hx, wall, hz).translation(Vector::new(x, wall, z)).build()).collect()
}

/// The buildings, the rest of the collision mesh (bridges) and the tree trunks of `world`, each
/// building or tree with the [`Obj`] it is.
fn object_colliders(world: &WorldFile) -> Result<Vec<(Collider, Option<Obj>)>> {
    let mut colliders = Vec::new();
    let mesh = &world.collision;
    if !mesh.triangles.is_empty() {
        let vertex = |&[x, y, z]: &[i32; 3]| Vector::new(from_cm(x), from_cm(y), from_cm(z));
        // Each building is its own collider so it can be taken out alone: its boxes and its floor
        // (`building_shape`), or a trimesh of its own triangles when it has no cubes. Whatever no
        // building claims (bridges, hand-built worlds) stays together in one trimesh.
        let mut claimed = vec![false; mesh.triangles.len()];
        let mut buildings = Vec::new();
        for (index, &[start, end]) in world.building_triangles.iter().enumerate() {
            let (start, end) = (start as usize, (end as usize).min(mesh.triangles.len()));
            let Some(&id) = world.building_ids.get(index) else { break };
            if start >= end {
                continue;
            }
            claimed[start..end].iter_mut().for_each(|c| *c = true);
            buildings.push((index, id, start, end));
        }
        // Making every building's faces is most of the loading: spread over the cores.
        let built = in_parallel(&buildings, |&(index, _, start, end)| -> Result<Collider> {
            if let Some(shape) = building_shape(world, index) {
                return Ok(ColliderBuilder::new(shape).build());
            }
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
            Ok(ColliderBuilder::trimesh(vertices, triangles).context("a building's collision mesh is invalid")?.build())
        });
        for (&(_, id, _, _), collider) in buildings.iter().zip(built) {
            colliders.push((collider?, Some(Obj::Building(id))));
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
        let mut cylinder = ColliderBuilder::cylinder(half_height, from_cm(tree.trunk_radius()))
            .translation(Vector::new(from_cm(tree.x), from_cm(tree.base) + half_height, from_cm(tree.z)));
        if tree.destructible() {
            cylinder = cylinder.collision_groups(InteractionGroups::new(TREE_GROUP, Group::ALL, InteractionTestMode::And));
        }
        let cylinder = cylinder.build();
        // A rock is solid but is no `Obj`: nothing damages or fells it.
        let object = if tree.destructible() { world.tree_ids.get(index).map(|&id| Obj::Tree(id)) } else { None };
        colliders.push((cylinder, object));
    }
    Ok(colliders)
}

/// `f` of every item of `items`, in order, worked out on all the cores.
fn in_parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    // The browser has no threads to spawn: one tile at a time is little work anyway.
    if cfg!(target_arch = "wasm32") {
        return items.iter().map(f).collect();
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let f = &f;
        let workers: Vec<_> = items.chunks(chunk).map(|part| scope.spawn(move || part.iter().map(f).collect::<Vec<_>>())).collect();
        workers.into_iter().flat_map(|worker| worker.join().expect("a worker cutting buildings panicked")).collect()
    })
}

/// Building `index` of `world` as triangles in centimetres from the collision mesh: its walls, its
/// roof and its floor.
pub fn building_parts(world: &WorldFile, index: usize) -> Option<[Vec<Tri>; 3]> {
    let building = world.buildings.get(index)?;
    let [start, end] = *world.building_triangles.get(index)?;
    let mesh = &world.collision;
    let end = (end as usize).min(mesh.triangles.len());
    let start = (start as usize).min(end);
    let tri = |t: usize| -> Tri {
        let [a, b, c] = mesh.triangles[t].map(|v| mesh.vertices[v as usize]);
        [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]]
    };
    let split = |from: usize, to: usize| (from.min(end)..to.min(end)).map(tri).collect::<Vec<_>>();
    let roof = start + building.walls as usize;
    let floor = roof + building.roof as usize;
    Some([split(start, roof), split(roof, floor), split(floor, end)])
}

/// Building `index` of `world` cut into cubes, and its floor (centimetres): the cells the importer
/// baked, or for a building with none (a world built by hand) cut from its walls and roof. `None`
/// when it has no cubes (it is too big to cut, or has no walls or roof).
pub fn building_shell(world: &WorldFile, index: usize) -> Option<(Shell, Vec<Tri>)> {
    let [walls, roof, floor] = building_parts(world, index)?;
    let baked = &world.buildings.get(index)?.cells;
    let shell = Shell::from_cells(baked).or_else(|| Shell::new(&walls, &roof))?;
    Some((shell, floor))
}

/// What building `index` of `world` is collided with while nothing has been cut from it: its cubes
/// merged into boxes and its floor (`solid_of`). `None` when it has no cubes, and is collided with
/// as its own triangles. Mirrored by `rebuildBuildingCollider` in `client/src/sim/physics.ts`.
pub fn building_shape(world: &WorldFile, index: usize) -> Option<SharedShape> {
    let (shell, floor) = building_shell(world, index)?;
    solid_of(&shell.boxes(&shell.untouched()), &floor)
}

/// One shape of `boxes` (`Shell::boxes`) and `floor` triangles, centimetres: a cuboid a box and a
/// triangle a floor triangle, in a compound. A box is far less for Rapier to keep and test than the
/// triangles of its faces. `None` when both are empty.
pub fn solid_of(boxes: &[[i32; 6]], floor: &[Tri]) -> Option<SharedShape> {
    let corner = |t: &Tri, k: usize| Vector::new(from_cm(t[k * 3]), from_cm(t[k * 3 + 1]), from_cm(t[k * 3 + 2]));
    let mut parts: Vec<(Pose, SharedShape)> = Vec::with_capacity(boxes.len() + floor.len());
    for b in boxes {
        let centre = Vector::new(from_cm(b[0] + b[3]) / 2.0, from_cm(b[1] + b[4]) / 2.0, from_cm(b[2] + b[5]) / 2.0);
        let half = [from_cm(b[3] - b[0]) / 2.0, from_cm(b[4] - b[1]) / 2.0, from_cm(b[5] - b[2]) / 2.0];
        parts.push((Pose::from_translation(centre), SharedShape::cuboid(half[0], half[1], half[2])));
    }
    // A compound cannot hold a trimesh, so the floor goes in a triangle at a time.
    for t in floor {
        parts.push((Pose::from_translation(Vector::new(0.0, 0.0, 0.0)), SharedShape::triangle(corner(t, 0), corner(t, 1), corner(t, 2))));
    }
    (!parts.is_empty()).then(|| SharedShape::compound(parts))
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
        let mut physics = Self::empty(from_cm(world.half_extent));
        for (collider, object) in static_colliders(world)? {
            physics.insert_static(collider, object);
        }
        physics.step_pipeline();
        Ok(physics)
    }

    /// A world with nothing in it yet, `bound` metres from the centre to each side.
    fn empty(bound: f32) -> Self {
        Self {
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
            replacements: Vec::new(),
            changed: Vec::new(),
            bound,
            tiles: HashMap::new(),
            obstacles: std::collections::BTreeMap::new(),
        }
    }

    /// Adds a static collider, noting the building or tree it is.
    fn insert_static(&mut self, collider: Collider, object: Option<Obj>) -> ColliderHandle {
        let handle = self.colliders.insert(collider);
        if let Some(object) = object {
            self.objects.insert(handle, object);
            self.handles.insert(object, handle);
        }
        handle
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

    /// Gives building `object` the collider `shape` (`solid_of`) in place of the one it has: a
    /// bitten building is a different shape. `None` takes it out. Like `destroy`, it takes effect
    /// at the start of the next `step_cars`. Does nothing for an object that is not there.
    pub fn replace_building(&mut self, object: Obj, shape: Option<SharedShape>) {
        if !self.handles.contains_key(&object) {
            return;
        }
        self.replacements.retain(|(queued, _)| *queued != object);
        self.replacements.push((object, shape));
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

    /// How far a ray from `origin` along the unit `dir` goes before it meets the static world, within
    /// `max`. Vehicles are ignored.
    pub fn cast_ray_static(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<f32> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::exclude_dynamic());
        queries.cast_ray(&Ray::new(Vector::from(origin), Vector::from(dir)), max, true).map(|(_, distance)| distance)
    }

    /// Whether a box of half extents `half` centred at `centre`, unturned, touches the static world.
    pub fn box_touches_static(&self, centre: [f32; 3], half: [f32; 3]) -> bool {
        self.touches_static(&Cuboid::new(Vector::from(half)), Pose::from_translation(Vector::from(centre)))
    }

    /// The ground under the static colliders that have changed since the last call; see `changed`.
    pub fn take_changed(&mut self) -> Vec<[f32; 4]> {
        std::mem::take(&mut self.changed)
    }

    /// Notes the ground under collider `handle` as changed.
    fn mark_changed(&mut self, handle: ColliderHandle) {
        if let Some(collider) = self.colliders.get(handle) {
            let aabb = collider.compute_aabb();
            self.changed.push([aabb.mins.x, aabb.mins.z, aabb.maxs.x, aabb.maxs.z]);
        }
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
    #[cfg(any(test, feature = "testing"))]
    pub fn move_character(&self, character: &mut Character, input: &InputFrame, dt: f32) {
        self.move_character_scaled(character, input, dt, 1.0);
    }

    /// `move_character` with walking speed multiplied by `speed_scale` (aiming a gun slows you).
    pub fn move_character_scaled(&self, character: &mut Character, input: &InputFrame, dt: f32, speed_scale: f32) {
        character.chute = toggle_chute(character.chute, character.grounded, character.prone, input);
        self.change_stance(character, input.buttons() & BUTTON_PRONE != 0 && !character.chute);
        character.knock = if character.chute { steer_chute(character.knock, input) } else { begin_slide(character.knock, character.grounded, input) };
        let (vy, desired) = desired_translation(character.vy, character.grounded, character.knock, input, dt, speed_scale, character.chute);
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
        if grounded {
            character.chute = false;
        }
        if !character.chute {
            character.knock = decay_knock(character.knock, grounded, input.buttons() & BUTTON_CROUCH != 0);
        }
        character.yaw = shared::quant::from_mrad(input.yaw);
    }

    /// One tick of a stickman on foot, as both the server and the client's prediction run it:
    /// `move_character_scaled`, plus what standing on a vehicle moving at `ride_velocity` does. Jumping
    /// off it adds its speed to the knockback before the move; stepping off its edge adds it after.
    /// Which vehicle the character stands on next (`Character::ride`) is the caller's to say.
    pub fn step_character(&self, character: &mut Character, input: &InputFrame, dt: f32, speed_scale: f32, ride_velocity: Option<[f32; 3]>) {
        let jumped = character.ride != 0 && character.grounded && input.buttons() & BUTTON_JUMP != 0;
        if let (true, Some(velocity)) = (jumped, ride_velocity) {
            inherit(character, velocity);
        }
        self.move_character_scaled(character, input, dt, speed_scale);
        if let (false, false, Some(velocity)) = (jumped, character.grounded, ride_velocity) {
            // Stepped off the edge of the roof.
            inherit(character, velocity);
        }
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

/// Adds a vehicle's horizontal velocity to a character's knockback, within the wire's limit.
fn inherit(character: &mut Character, velocity: [f32; 3]) {
    let mut knock = [character.knock[0] + velocity[0], character.knock[1] + velocity[2]];
    let speed = knock[0].hypot(knock[1]);
    if speed > KNOCK_MAX_SPEED {
        knock = [knock[0] / speed * KNOCK_MAX_SPEED, knock[1] / speed * KNOCK_MAX_SPEED];
    }
    character.knock = knock;
}

/// Capsule-centre height for a character standing on flat ground at y = 0.
pub const STANDING_Y: f32 = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS + CONTROLLER_OFFSET;

#[cfg(test)]
mod tests;
