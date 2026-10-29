//! Ragdolls: soldiers knocked down by a blast (KNOCKDOWN.md), drawn as jointed bodies tumbling over the
//! scenery. Only the client has them, and only for show: where a soldier is stays the server's
//! (`Character`), and each ragdoll's pelvis is pulled towards it horizontally so the two agree.
//!
//! They live in a small Rapier world of their own, stepped once a frame at a steady `TICK_DT`, apart
//! from the client's `Physics`, which car prediction steps whenever it replays. The static colliders
//! near each ragdoll (ground, buildings, trees, placed cubes, vehicles) are copied in from that world
//! as they are needed: their shapes are shared, so a copy is cheap. Vehicles are left out: they move,
//! and a copy refreshed now and then would be a car that is not there, or one a ragdoll thrown by it
//! starts inside.

use crate::Physics;
use rapier3d::prelude::*;
use shared::consts::{GRAVITY, TICK_DT};
use shared::protocol::{LIMB_ARM_L, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_R};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The parts of a ragdoll, in this order: pelvis, torso, head, left upper arm and forearm, right
/// upper arm and forearm, left thigh and shin, right thigh and shin.
pub const PARTS: usize = 11;
/// Floats per part in a pose (`Ragdolls::pose`): position, then rotation as a quaternion (x, y, z, w).
pub const POSE_FLOATS: usize = 7;
/// Floats per part when spawning (`Ragdolls::spawn`): the segment it covers, from one joint to the next.
pub const SEGMENT_FLOATS: usize = 6;

const PELVIS: usize = 0;
/// Which part each hangs from.
const PARENT: [usize; PARTS] = [0, 0, 1, 1, 3, 1, 5, 0, 7, 0, 9];
/// Each part's thickness, metres.
const RADIUS: [f32; PARTS] = [0.12, 0.14, 0.11, 0.055, 0.05, 0.055, 0.05, 0.075, 0.06, 0.075, 0.06];
/// Each part's mass, kg: about 75 in all.
const MASS: [f32; PARTS] = [12.0, 22.0, 5.0, 2.5, 1.8, 2.5, 1.8, 9.0, 4.5, 9.0, 4.5];
/// The limb (`LIMB_*`) each part belongs to, 0 for the pelvis, torso and head.
const LIMB: [u8; PARTS] = [0, 0, 0, LIMB_ARM_L, LIMB_ARM_L, LIMB_ARM_R, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_L, LIMB_LEG_R, LIMB_LEG_R];

/// How each part is joined to its parent.
#[derive(Clone, Copy)]
enum Joint {
    /// Bends and twists about its own line, up to so far each way, radians: a waist, a neck.
    Spine(f32),
    /// Swings freely about where it was at the blast, up to so far each way, radians: a shoulder, a hip.
    Socket(f32),
    /// Bends only about the body's sideways axis, within this range: an elbow, a knee. Positive brings
    /// the far end forward.
    Hinge(f32, f32),
}

const JOINT: [Joint; PARTS] = [
    Joint::Spine(0.0),
    Joint::Spine(0.5),
    Joint::Spine(0.7),
    Joint::Socket(1.8),
    Joint::Hinge(0.0, 2.5),
    Joint::Socket(1.8),
    Joint::Hinge(0.0, 2.5),
    Joint::Socket(1.3),
    Joint::Hinge(-2.5, 0.0),
    Joint::Socket(1.3),
    Joint::Hinge(-2.5, 0.0),
];

/// Ragdoll parts never touch each other, of the same ragdoll or another: only the scenery.
const PART_GROUP: Group = Group::GROUP_4;
/// The pelvis is pulled towards where the server has the soldier at this stiffness (1/s²) and damping
/// (1/s), horizontally only: the ground decides the height.
const TETHER_STIFFNESS: f32 = 40.0;
const TETHER_DAMPING: f32 = 10.0;
/// Scenery within this many metres of a ragdoll's pelvis is copied in, and refreshed this often, seconds.
const SCENERY_REACH: f32 = 15.0;
const SCENERY_EVERY: f32 = 0.5;
/// The most substeps one call to `step` runs, so a long stall does not run away.
const MAX_SUBSTEPS: u32 = 4;

struct Doll {
    parts: [Option<RigidBodyHandle>; PARTS],
    /// Where the server has the soldier: the pelvis is pulled towards it.
    target: Option<[f32; 3]>,
    mass: f32,
}

pub struct Ragdolls {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    joints: ImpulseJointSet,
    multibody: MultibodyJointSet,
    soft: SoftBodySet,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pipeline: PhysicsPipeline,
    ccd: CCDSolver,
    parameters: IntegrationParameters,
    dolls: BTreeMap<u32, Doll>,
    /// The copy of each static collider of the main world brought in, by its handle there.
    scenery: HashMap<ColliderHandle, ColliderHandle>,
    /// Time not yet stepped, seconds.
    owed: f32,
    /// Seconds since the scenery was last brought up to date.
    since_scenery: f32,
}

impl Default for Ragdolls {
    fn default() -> Self {
        Self::new()
    }
}

impl Ragdolls {
    pub fn new() -> Self {
        Self {
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            joints: ImpulseJointSet::new(),
            multibody: MultibodyJointSet::new(),
            soft: SoftBodySet::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            pipeline: PhysicsPipeline::new(),
            ccd: CCDSolver::new(),
            parameters: IntegrationParameters { dt: TICK_DT, ..Default::default() },
            dolls: BTreeMap::new(),
            scenery: HashMap::new(),
            owed: 0.0,
            since_scenery: 0.0,
        }
    }

    /// How many ragdolls there are.
    pub fn len(&self) -> usize {
        self.dolls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dolls.is_empty()
    }

    /// Whether ragdoll `key` exists.
    pub fn has(&self, key: u32) -> bool {
        self.dolls.contains_key(&key)
    }

    /// Makes ragdoll `key` (replacing any it had) from `segments`: for each part, the world points it
    /// runs between (`SEGMENT_FLOATS` each), as the soldier stood when it was thrown. `side` is the
    /// soldier's right, the axis elbows and knees bend about. Every part sets off at `velocity` (m/s),
    /// turning at `spin` (rad/s, about world axes). The limbs in `lost` (`LIMB_*`) are left out.
    pub fn spawn(&mut self, physics: &Physics, key: u32, segments: &[f32], side: [f32; 3], velocity: [f32; 3], spin: [f32; 3], lost: u8) -> bool {
        if segments.len() < PARTS * SEGMENT_FLOATS {
            return false;
        }
        self.remove(key);
        let side = Vector::from(side).normalize_or(Vector::X);
        let segment = |part: usize| {
            let s = &segments[part * SEGMENT_FLOATS..(part + 1) * SEGMENT_FLOATS];
            (Vector::new(s[0], s[1], s[2]), Vector::new(s[3], s[4], s[5]))
        };
        let mut parts = [None; PARTS];
        let mut poses = [Pose::IDENTITY; PARTS];
        let mut mass = 0.0;
        for part in 0..PARTS {
            if LIMB[part] & lost != 0 {
                continue;
            }
            let (a, b) = segment(part);
            let along = b - a;
            let length = along.length().max(0.05);
            let direction = if along.length() > 1e-4 { along / along.length() } else { Vector::Y };
            let pose = Pose::from_parts((a + b) * 0.5, Rotation::from_rotation_arc(Vector::Y, direction));
            let body = RigidBodyBuilder::dynamic()
                .pose(pose)
                .linvel(Vector::from(velocity))
                .angvel(Vector::from(spin))
                .linear_damping(0.05)
                .angular_damping(0.8)
                .ccd_enabled(true)
                .build();
            let handle = self.bodies.insert(body);
            let half = (length * 0.5 - RADIUS[part]).max(0.01);
            let collider = ColliderBuilder::capsule_y(half, RADIUS[part])
                .mass(MASS[part])
                .friction(0.9)
                .restitution(0.05)
                .collision_groups(InteractionGroups::new(PART_GROUP, Group::ALL.difference(PART_GROUP), InteractionTestMode::And))
                .build();
            self.colliders.insert_with_parent(collider, handle, &mut self.bodies);
            parts[part] = Some(handle);
            poses[part] = pose;
            mass += MASS[part];
        }
        if parts[PELVIS].is_none() {
            for handle in parts.into_iter().flatten() {
                self.bodies.remove(handle, &mut self.islands, &mut self.colliders, &mut self.joints, &mut self.multibody, &mut self.soft, true);
            }
            return false;
        }
        for part in 1..PARTS {
            let parent = PARENT[part];
            let (Some(child), Some(above)) = (parts[part], parts[parent]) else { continue };
            let (anchor, _) = segment(part);
            let (from, to) = (poses[parent], poses[part]);
            // The frame each limit is measured in, as each body sees it.
            let frame = |pose: Pose, rotation: Rotation| Pose::from_parts(pose.inverse_transform_point(anchor), pose.rotation.inverse() * rotation);
            let along = |pose: Pose| pose.rotation * Vector::Y;
            let basis = |up: Vector| {
                let x = (side - up * side.dot(up)).normalize_or(Vector::X);
                Rotation::from_mat3(&rapier3d::glamx::Mat3::from_cols(x, up, x.cross(up)))
            };
            let joint = match JOINT[part] {
                // Measured from each part's own line: straight is no bend at all.
                Joint::Spine(limit) => GenericJointBuilder::new(JointAxesMask::LOCKED_SPHERICAL_AXES)
                    .local_frame1(frame(from, basis(along(from))))
                    .local_frame2(frame(to, basis(along(to))))
                    .limits(JointAxis::AngX, [-limit, limit])
                    .limits(JointAxis::AngY, [-limit * 0.6, limit * 0.6])
                    .limits(JointAxis::AngZ, [-limit, limit]),
                // Measured from the pose at the blast.
                Joint::Socket(limit) => {
                    let shared = basis(along(to));
                    GenericJointBuilder::new(JointAxesMask::LOCKED_SPHERICAL_AXES)
                        .local_frame1(frame(from, shared))
                        .local_frame2(frame(to, shared))
                        .limits(JointAxis::AngX, [-limit, limit])
                        .limits(JointAxis::AngY, [-0.6, 0.6])
                        .limits(JointAxis::AngZ, [-limit, limit])
                }
                Joint::Hinge(min, max) => GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES)
                    .local_frame1(frame(from, basis(along(from))))
                    .local_frame2(frame(to, basis(along(to))))
                    .limits(JointAxis::AngX, [min, max]),
            };
            self.joints.insert(above, child, joint.contacts_enabled(false).build(), true);
        }
        self.dolls.insert(key, Doll { parts, target: None, mass });
        self.bring_scenery(physics);
        true
    }

    /// Where the server has ragdoll `key`'s soldier now: its pelvis is pulled there.
    pub fn set_target(&mut self, key: u32, target: [f32; 3]) {
        if let Some(doll) = self.dolls.get_mut(&key) {
            doll.target = Some(target);
        }
    }

    /// Takes ragdoll `key` away.
    pub fn remove(&mut self, key: u32) {
        let Some(doll) = self.dolls.remove(&key) else { return };
        for handle in doll.parts.into_iter().flatten() {
            self.bodies.remove(handle, &mut self.islands, &mut self.colliders, &mut self.joints, &mut self.multibody, &mut self.soft, true);
        }
    }

    /// Writes ragdoll `key`'s parts into `out`, `POSE_FLOATS` each in `PARTS` order (a part left out,
    /// a lost limb, gets its parent's). Whether it exists.
    pub fn pose(&self, key: u32, out: &mut [f32]) -> bool {
        let Some(doll) = self.dolls.get(&key) else { return false };
        if out.len() < PARTS * POSE_FLOATS {
            return false;
        }
        for part in 0..PARTS {
            let mut from = part;
            while doll.parts[from].is_none() && from != PELVIS {
                from = PARENT[from];
            }
            let Some(body) = doll.parts[from].and_then(|handle| self.bodies.get(handle)) else { continue };
            let pose = body.position();
            let t = pose.translation;
            let r = pose.rotation;
            out[part * POSE_FLOATS..(part + 1) * POSE_FLOATS].copy_from_slice(&[t.x, t.y, t.z, r.x, r.y, r.z, r.w]);
        }
        true
    }

    /// Runs the ragdolls on by `dt` seconds, in steady `TICK_DT` steps, among the scenery of `physics`.
    pub fn step(&mut self, physics: &Physics, dt: f32) {
        if self.dolls.is_empty() {
            self.owed = 0.0;
            return;
        }
        self.since_scenery += dt;
        if self.since_scenery >= SCENERY_EVERY {
            self.bring_scenery(physics);
        }
        self.owed = (self.owed + dt).min(TICK_DT * MAX_SUBSTEPS as f32);
        while self.owed >= TICK_DT {
            self.owed -= TICK_DT;
            self.tether();
            self.pipeline.step(
                Vector::new(0.0, -GRAVITY, 0.0),
                &self.parameters,
                &mut self.islands,
                &mut self.broad_phase,
                &mut self.narrow_phase,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.joints,
                &mut self.multibody,
                &mut self.soft,
                &mut self.ccd,
                &(),
                &(),
            );
        }
    }

    /// Pulls each pelvis towards its soldier, horizontally: a damped spring on the whole body's mass.
    fn tether(&mut self) {
        for doll in self.dolls.values() {
            let (Some(target), Some(handle)) = (doll.target, doll.parts[PELVIS]) else { continue };
            let Some(pelvis) = self.bodies.get_mut(handle) else { continue };
            let at = pelvis.translation();
            let velocity = pelvis.linvel();
            let pull = |i: usize| doll.mass * (TETHER_STIFFNESS * (target[i] - at[i]) - TETHER_DAMPING * velocity[i]);
            pelvis.reset_forces(false);
            pelvis.add_force(Vector::new(pull(0), 0.0, pull(2)), true);
        }
    }

    /// Copies in the static colliders of `physics` near any ragdoll, moves the ones that have moved
    /// (vehicles), and drops the ones no ragdoll is near any more or that are gone.
    fn bring_scenery(&mut self, physics: &Physics) {
        self.since_scenery = 0.0;
        let mut near = HashSet::new();
        let centres: Vec<Vector> = self.dolls.values().filter_map(|doll| doll.parts[PELVIS]).filter_map(|handle| self.bodies.get(handle)).map(|body| body.translation()).collect();
        for centre in centres {
            let reach = Vector::splat(SCENERY_REACH);
            for (handle, shape, pose, groups) in physics.scenery_near(centre - reach, centre + reach) {
                near.insert(handle);
                match self.scenery.get(&handle).and_then(|copy| self.colliders.get_mut(*copy)) {
                    Some(copy) => {
                        if *copy.position() != pose {
                            copy.set_position(pose);
                        }
                    }
                    None => {
                        let copy = ColliderBuilder::new(shape).position(pose).collision_groups(groups).friction(0.9).build();
                        let copy = self.colliders.insert(copy);
                        self.scenery.insert(handle, copy);
                    }
                }
            }
        }
        let gone: Vec<ColliderHandle> = self.scenery.keys().filter(|handle| !near.contains(handle)).copied().collect();
        for handle in gone {
            if let Some(copy) = self.scenery.remove(&handle) {
                self.colliders.remove(copy, &mut self.islands, &mut self.bodies, &mut self.soft, true);
            }
        }
    }
}

impl Physics {
    /// The static scenery (ground, buildings, trees, placed cubes: everything not on a body, so no
    /// vehicle) whose bounds reach into the box from `min` to `max`: each collider's handle, shape,
    /// where it is and its collision groups.
    pub(crate) fn scenery_near(&self, min: Vector, max: Vector) -> Vec<(ColliderHandle, SharedShape, Pose, InteractionGroups)> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        queries
            .intersect_aabb_conservative(Aabb::new(min, max))
            .filter(|(_, collider)| collider.parent().is_none())
            .map(|(handle, collider)| (handle, collider.shared_shape().clone(), *collider.position(), collider.collision_groups()))
            .collect()
    }
}

#[cfg(test)]
mod tests;
