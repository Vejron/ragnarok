//! A giant's limbs: the two fists its pilot moves, and its two solid feet.
//!
//! The hull (its torso) is a dynamic body on spring legs, as a walker's is. Its limbs are kinematic
//! bodies of their own, which the hull does not feel: a fist goes where its pilot asks, within the arm's
//! reach and no faster than the arm swings. Its feet are what it stands on (`Planted`, which `walk`
//! reads): a planted foot does not move, and they step in turn, the weight going onto one while the
//! other swings ahead to where the hull is going. They shove whatever dynamic body they meet (a car,
//! a tank), and a vehicle a fist holds is joined to it. What they do to soldiers, buildings and trees is
//! the server's `Titans` mechanic's business, which reads them with [`Physics::limbs`].

use super::*;
use crate::TREE_GROUP;
use shared::vehicles::titan::bend;
use shared::vehicles::{Feet, Step};

/// The collision group of a giant's limbs. A giant's own hull leaves this group out of what it touches,
/// so its fists and feet never push it about.
pub const LIMB_GROUP: Group = Group::GROUP_5;

/// What a limb touches: everything but other limbs.
fn limb_groups() -> InteractionGroups {
    InteractionGroups::new(LIMB_GROUP, Group::ALL.difference(LIMB_GROUP), InteractionTestMode::And)
}

/// What the hull of a kind with limbs touches: neither a limb nor a tree it goes through.
pub(super) fn giant_groups() -> InteractionGroups {
    InteractionGroups::new(Group::ALL.difference(LIMB_GROUP), Group::ALL.difference(TREE_GROUP).difference(LIMB_GROUP), InteractionTestMode::And)
}

/// Whether `collider` is some giant's limb.
pub(super) fn is_limb(collider: &Collider) -> bool {
    collider.collision_groups().memberships == LIMB_GROUP
}

/// A foot a hull at rest has left this far from under its hip steps back there, metres.
const SETTLE_REACH: f32 = 2.0;
/// The hull is at rest below this speed, m/s.
const SETTLE_SPEED: f32 = 1.0;
/// A foot this many strides from its hip has been left behind (the giant was put somewhere else): it is
/// put back under it at once.
const LOST_STRIDES: f32 = 3.0;
/// Off the ground, its feet hang this share of a leg's length under its hips, and are drawn up to
/// there at this speed, m/s.
const HANG: f32 = 0.93;
const TUCK_SPEED: f32 = 10.0;
/// A foot lifts once it carries less than this share of the weight.
const LIGHT: f32 = 0.02;
/// Going faster than this with the stick let go, m/s, it is being carried along and steps to keep up.
const CARRIED_SPEED: f32 = 3.0;
/// The stick is held past this.
const WALKED: f32 = 0.1;
/// How far above the hip, and below the leg's length, a foot looks for the ground, metres.
const GROUND_PROBE: f32 = 8.0;

/// What a pilot asks of one hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hand {
    /// Where the fist should be, in the hull frame. `None` lets it hang.
    pub target: Option<[f32; 3]>,
    /// The hand is closed: it holds what it closes on.
    pub closed: bool,
}

/// Where a giant's limbs are after the last step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimbsPose {
    /// Each fist's middle (left, right) in the hull frame and in the world, and its velocity in the
    /// world, m/s.
    pub fists: [[f32; 3]; 2],
    pub fists_world: [[f32; 3]; 2],
    pub fist_velocity: [[f32; 3]; 2],
    /// Which hands are closed.
    pub closed: [bool; 2],
    /// The underside of each foot (left, right) in the world.
    pub feet: [[f32; 3]; 2],
    /// A foot came down in the last step.
    pub landed: [bool; 2],
    /// Where each foot stands, or the ground it is over while it steps.
    pub planted: [[f32; 3]; 2],
    /// Where it is in its walk.
    pub step: Step,
    /// The share of the weight the left foot carries.
    pub weight: f32,
    /// How fast it was coming down when its feet met the ground in the last step, m/s; 0 in any other.
    pub impact: f32,
}

struct Fist {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// Its middle and its velocity, in the hull frame.
    at: Vector,
    velocity: Vector,
    hand: Hand,
    /// Its velocity in the world after the last step, m/s.
    world_velocity: Vector,
    /// The vehicle it holds, and the joint that holds it.
    holds: Option<(u32, ImpulseJointHandle)>,
}

struct Foot {
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// Where it stands, or the ground it is over while it steps.
    at: Vector,
    /// The way it points, radians about +Y: as it was put down.
    yaw: f32,
    landed: bool,
}

/// How a giant stands on its feet.
struct Stance {
    /// Where it is in its walk.
    step: Step,
    /// The share of the weight the left foot carries; the right has the rest.
    weight: f32,
    /// How fast it was coming down when its feet met the ground in the last step, m/s.
    impact: f32,
}

impl Stance {
    const LEVEL: Stance = Stance { step: Step::Planted, weight: 0.5, impact: 0.0 };
}

/// The feet a giant stands on, for `walk`: where each is, and the share of the weight it carries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Planted {
    pub at: [Vector; 2],
    pub weight: [f32; 2],
}

pub(super) struct Limbs {
    fists: [Fist; 2],
    feet: Option<[Foot; 2]>,
    stance: Stance,
}

impl Limbs {
    /// Whether `collider` is one of these limbs.
    pub(super) fn owns(&self, collider: ColliderHandle) -> bool {
        self.fists.iter().any(|fist| fist.collider == collider) || self.feet.as_ref().is_some_and(|feet| feet.iter().any(|foot| foot.collider == collider))
    }

    /// Its fists' and feet's colliders.
    pub(super) fn colliders(&self) -> impl Iterator<Item = ColliderHandle> + '_ {
        self.fists.iter().map(|fist| fist.collider).chain(self.feet.iter().flatten().map(|foot| foot.collider))
    }

    /// The feet it stands on, once they have been found under it.
    pub(super) fn planted(&self) -> Option<Planted> {
        let feet = self.feet.as_ref()?;
        if feet.iter().any(|foot| foot.at.x == f32::MAX) {
            return None;
        }
        let weight = match self.stance.step {
            Step::Airborne => [0.0, 0.0],
            Step::Swinging(0, _) => [0.0, 1.0],
            Step::Swinging(..) => [1.0, 0.0],
            _ => [self.stance.weight, 1.0 - self.stance.weight],
        };
        Some(Planted { at: [feet[0].at, feet[1].at], weight })
    }
}

/// How high a foot `progress` through its step is over the ground, metres.
fn lifted(shape: &Feet, progress: f32) -> f32 {
    shape.lift * (std::f32::consts::PI * progress).sin()
}

/// -1 for the left limb (index 0), 1 for the right.
fn side(index: usize) -> f32 {
    if index == 0 { -1.0 } else { 1.0 }
}

/// `point`, given for the right side, on the side of limb `index`.
fn mirrored(point: [f32; 3], index: usize) -> Vector {
    Vector::new(point[0] * side(index), point[1], point[2])
}

/// `at`, a fist's middle in the hull frame, kept within its arm's reach of `shoulder` and out of the
/// torso's box.
fn within_reach(spec: &Spec, arms: &Arms, shoulder: Vector, at: Vector) -> Vector {
    let mut at = at;
    // Out of the torso: through the nearest face.
    let half = Vector::from(spec.half_extents) + Vector::splat(arms.fist);
    let inside = half - at.abs();
    if inside.x > 0.0 && inside.y > 0.0 && inside.z > 0.0 {
        if inside.x <= inside.y && inside.x <= inside.z {
            at.x = half.x * if at.x == 0.0 { shoulder.x.signum() } else { at.x.signum() };
        } else if inside.z <= inside.y {
            at.z = half.z * if at.z > 0.0 { 1.0 } else { -1.0 };
        } else {
            at.y = half.y * at.y.signum();
        }
    }
    let arm = at - shoulder;
    let length = arm.length();
    if length < 1e-3 {
        return shoulder + Vector::new(0.0, -arms.min_reach, 0.0);
    }
    shoulder + arm * (length.clamp(arms.min_reach, arms.reach) / length)
}

impl Physics {
    /// Gives vehicle `id` the limbs its kind has, where it stands. Called as it is added.
    pub(super) fn add_limbs(&mut self, id: u32) {
        let Some(car) = self.cars.get(&id) else { return };
        let (spec, hull) = (car.spec, car.body);
        let Some(arms) = spec.arms else { return };
        let Some(pose) = self.bodies.get(hull).map(|body| *body.position()) else { return };
        let fists = [0, 1].map(|index| {
            let at = mirrored(arms.rest, index);
            let body = self.bodies.insert(RigidBodyBuilder::kinematic_position_based().pose(Pose::from_parts(pose * at, pose.rotation)).build());
            let collider = self.colliders.insert_with_parent(ColliderBuilder::ball(arms.fist).collision_groups(limb_groups()).friction(0.8).build(), body, &mut self.bodies);
            Fist { body, collider, at, velocity: Vector::ZERO, hand: Hand::default(), world_velocity: Vector::ZERO, holds: None }
        });
        let feet = spec.gait.and_then(|gait| Some((gait, gait.feet?))).map(|(gait, feet)| {
            [0, 1].map(|index| {
                let [hx, hy, hz] = feet.half;
                let hip = pose * Vector::new(side(index) * gait.foot_half, -spec.half_extents[1], 0.0);
                let at = Vector::new(hip.x, hip.y - gait.leg_length, hip.z);
                let body = self.bodies.insert(RigidBodyBuilder::kinematic_position_based().pose(Pose::from_parts(at + Vector::new(0.0, hy, 0.0), pose.rotation)).build());
                let collider = self.colliders.insert_with_parent(ColliderBuilder::cuboid(hx, hy, hz).collision_groups(limb_groups()).friction(0.8).build(), body, &mut self.bodies);
                Foot { body, collider, at, yaw: 0.0, landed: false }
            })
        });
        if let Some(car) = self.cars.get_mut(&id) {
            car.limbs = Some(Limbs { fists, feet, stance: Stance::LEVEL });
        }
    }

    /// Takes a vehicle's limbs out of the world, letting go of what they held.
    pub(super) fn remove_limbs(&mut self, limbs: Limbs) {
        let feet = limbs.feet.into_iter().flatten().map(|foot| foot.body);
        for body in limbs.fists.iter().map(|fist| fist.body).chain(feet) {
            self.bodies.remove(body, &mut self.islands, &mut self.colliders, &mut self.impulse_joints, &mut self.multibody_joints, &mut self.soft_bodies, true);
        }
    }

    /// Puts a vehicle's limbs back where they hang and stand, at rest, with its hands empty.
    pub(super) fn reset_limbs(&mut self, id: u32) {
        self.release_car(id, 0);
        self.release_car(id, 1);
        let Some(car) = self.cars.get_mut(&id) else { return };
        let (Some(limbs), Some(arms)) = (&mut car.limbs, car.spec.arms) else { return };
        for (index, fist) in limbs.fists.iter_mut().enumerate() {
            fist.at = mirrored(arms.rest, index);
            fist.velocity = Vector::ZERO;
            fist.hand = Hand::default();
        }
        // The feet are found again under the hips by the next step.
        for foot in limbs.feet.iter_mut().flatten() {
            foot.at = Vector::splat(f32::MAX);
        }
        limbs.stance = Stance::LEVEL;
    }

    /// Puts vehicle `id`'s feet where a snapshot has them (`LimbsPose::planted`, `step` and `weight`),
    /// for its pilot's client to walk it on from there.
    pub fn set_feet(&mut self, id: u32, planted: [[f32; 3]; 2], step: Step, weight: f32) {
        let Some(limbs) = self.cars.get_mut(&id).and_then(|car| car.limbs.as_mut()) else { return };
        let Some(feet) = limbs.feet.as_mut() else { return };
        for (foot, at) in feet.iter_mut().zip(planted) {
            foot.at = Vector::from(at);
        }
        limbs.stance = Stance { step, weight: weight.clamp(0.0, 1.0), impact: 0.0 };
    }

    /// What the pilot of vehicle `id` asks of its hands (left, right) from now on.
    pub fn set_hands(&mut self, id: u32, hands: [Hand; 2]) {
        let Some(limbs) = self.cars.get_mut(&id).and_then(|car| car.limbs.as_mut()) else { return };
        for (fist, hand) in limbs.fists.iter_mut().zip(hands) {
            fist.hand = hand;
        }
    }

    /// Where vehicle `id`'s limbs are, if it has any.
    pub fn limbs(&self, id: u32) -> Option<LimbsPose> {
        let car = self.cars.get(&id)?;
        let limbs = car.limbs.as_ref()?;
        let pose = *self.bodies.get(car.body)?.position();
        let fists = [0, 1].map(|index| &limbs.fists[index]);
        let feet = limbs.feet.as_ref();
        let shape = car.spec.gait.and_then(|gait| gait.feet);
        let planted = [0, 1].map(|index| feet.map_or(Vector::ZERO, |feet| feet[index].at));
        let up = |index: usize| match (limbs.stance.step, shape) {
            (Step::Swinging(foot, progress), Some(shape)) if foot == index => lifted(&shape, progress),
            _ => 0.0,
        };
        Some(LimbsPose {
            fists: fists.map(|fist| fist.at.into()),
            fists_world: fists.map(|fist| (pose * fist.at).into()),
            fist_velocity: fists.map(|fist| fist.world_velocity.into()),
            closed: fists.map(|fist| fist.hand.closed),
            feet: [0, 1].map(|index| (planted[index] + Vector::new(0.0, up(index), 0.0)).into()),
            landed: [0, 1].map(|index| feet.is_some_and(|feet| feet[index].landed)),
            planted: planted.map(Into::into),
            step: limbs.stance.step,
            weight: limbs.stance.weight,
            impact: limbs.stance.impact,
        })
    }

    /// The bones of vehicle `id`'s limbs as rounds meet them, in the world: each a capsule from one end
    /// to the other, of a radius, metres. Empty for a vehicle without limbs.
    pub fn hit_bones(&self, id: u32) -> Vec<([f32; 3], [f32; 3], f32)> {
        self.cars.get(&id).map_or_else(Vec::new, |car| self.bones(car).into_iter().map(|(from, to, radius)| (from.into(), to.into(), radius)).collect())
    }

    /// Where the forearm of vehicle `id`'s arm `hand` (0 the left, 1 the right) is, in the world, bent as
    /// the client draws it: its elbow and the middle of its fist. A gun held in that fist fires along it.
    pub fn forearm(&self, id: u32, hand: usize) -> Option<([f32; 3], [f32; 3])> {
        let car = self.cars.get(&id)?;
        let pose = *self.bodies.get(car.body)?.position();
        let (_, elbow, end) = arm(car, hand)?;
        Some(((pose * Vector::from(elbow)).into(), (pose * Vector::from(end)).into()))
    }

    /// The bones of `car`'s limbs, in the world, bent as the client draws them (`TitanModel.update` in
    /// `client/src/render/titan.ts`): each upper arm from its shoulder to its elbow and each forearm on
    /// to its fist, and each thigh from its hip to its knee and each shin on to the ankle over where its
    /// foot is (lifted, while it steps). A foot not yet found under it has no leg.
    pub(super) fn bones(&self, car: &Car) -> Vec<(Vector, Vector, f32)> {
        let (Some(limbs), Some(body)) = (&car.limbs, self.bodies.get(car.body)) else { return Vec::new() };
        let pose = *body.position();
        let world = |point: [f32; 3]| pose * Vector::from(point);
        let mut bones = Vec::with_capacity(8);
        if let Some(arms) = car.spec.arms {
            for index in 0..limbs.fists.len() {
                let Some((shoulder, elbow, end)) = arm(car, index) else { continue };
                bones.push((world(shoulder), world(elbow), arms.upper_radius));
                bones.push((world(elbow), world(end), arms.fore_radius));
            }
        }
        if let (Some(gait), Some(feet)) = (car.spec.gait, &limbs.feet)
            && let Some(shape) = gait.feet
        {
            for (index, foot) in feet.iter().enumerate().filter(|(_, foot)| foot.at.x != f32::MAX) {
                let hip = [side(index) * gait.foot_half, -car.spec.half_extents[1], 0.0];
                let up = match limbs.stance.step {
                    Step::Swinging(stepping, progress) if stepping == index => lifted(&shape, progress),
                    _ => 0.0,
                };
                let ankle = pose.inverse() * (foot.at + Vector::new(0.0, up + shape.ankle, 0.0));
                let (knee, end) = bend(hip, ankle.into(), shape.thigh, shape.shin, shape.knee_pole);
                bones.push((world(hip), world(knee), shape.thigh_radius));
                bones.push((world(knee), world(end), shape.shin_radius));
            }
        }
        bones
    }

    /// The vehicles the ball of `radius` at `centre` overlaps, but for `but`: what a fist or a foot is on.
    pub fn cars_touching_ball(&self, centre: [f32; 3], radius: f32, but: u32) -> Vec<u32> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::only_dynamic());
        let mut found: Vec<u32> = queries
            .intersect_shape(Pose::from_translation(Vector::from(centre)), &Ball::new(radius))
            .filter_map(|(hit, _)| self.cars.iter().find(|(_, car)| car.owns(hit)).map(|(&id, _)| id))
            .filter(|&id| id != but)
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// The buildings and trees the ball of `radius` at `centre` overlaps.
    pub fn objects_touching_ball(&self, centre: [f32; 3], radius: f32) -> Vec<Obj> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::only_fixed());
        let mut found: Vec<Obj> = queries.intersect_shape(Pose::from_translation(Vector::from(centre)), &Ball::new(radius)).filter_map(|(hit, _)| self.object_of(hit)).collect();
        found.dedup();
        found
    }

    /// A vehicle's mass, kilograms.
    pub fn car_mass(&self, id: u32) -> Option<f32> {
        Some(self.bodies.get(self.cars.get(&id)?.body)?.mass())
    }

    /// Gives vehicle `id` a knock of `dv` m/s at `point` (world), as a blow there would.
    pub fn knock_car(&mut self, id: u32, dv: [f32; 3], point: [f32; 3]) {
        let Some(body) = self.cars.get(&id).and_then(|car| self.bodies.get_mut(car.body)) else { return };
        let impulse = Vector::from(dv) * body.mass();
        body.apply_impulse_at_point(impulse, Vector::from(point), true);
    }

    /// Hand `hand` (0 left, 1 right) of giant `id` takes hold of vehicle `car`, as it lies in the fist
    /// now. Returns whether it did: not a vehicle heavier than the arm lifts, nor with a full hand.
    pub fn grab_car(&mut self, id: u32, hand: usize, car: u32) -> bool {
        let Some(giant) = self.cars.get(&id) else { return false };
        let (Some(limbs), Some(arms)) = (&giant.limbs, giant.spec.arms) else { return false };
        let Some(held) = self.cars.get(&car) else { return false };
        let fist = &limbs.fists[hand];
        if fist.holds.is_some() || car == id || held.limbs.is_some() {
            return false;
        }
        let (Some(fist_body), Some(held_body)) = (self.bodies.get(fist.body), self.bodies.get(held.body)) else { return false };
        if held_body.mass() > arms.lift {
            return false;
        }
        // The vehicle stays where it is in the fist's frame.
        let frame = fist_body.position().inverse() * *held_body.position();
        let joint = FixedJointBuilder::new().local_frame1(frame).contacts_enabled(false).build();
        let handle = self.impulse_joints.insert(fist.body, held.body, joint, true);
        if let Some(limbs) = self.cars.get_mut(&id).and_then(|giant| giant.limbs.as_mut()) {
            limbs.fists[hand].holds = Some((car, handle));
        }
        true
    }

    /// Hand `hand` of giant `id` lets go of the vehicle it holds, which flies on as fast as the fist was
    /// going. Returns the vehicle.
    pub fn release_car(&mut self, id: u32, hand: usize) -> Option<u32> {
        let limbs = self.cars.get_mut(&id)?.limbs.as_mut()?;
        let (car, handle) = limbs.fists[hand].holds.take()?;
        let velocity = limbs.fists[hand].world_velocity;
        self.impulse_joints.remove(handle, true);
        if let Some(body) = self.cars.get(&car).and_then(|held| self.bodies.get_mut(held.body)) {
            body.set_linvel(velocity, true);
        }
        Some(car)
    }

    /// The vehicle hand `hand` of giant `id` holds.
    pub fn held_car(&self, id: u32, hand: usize) -> Option<u32> {
        self.cars.get(&id)?.limbs.as_ref()?.fists[hand].holds.map(|(car, _)| car)
    }

    /// Lets go of vehicle `car` wherever a fist holds it: it is leaving the world.
    pub(super) fn drop_from_fists(&mut self, car: u32) {
        let holders: Vec<(u32, usize)> = self
            .cars
            .iter()
            .filter_map(|(&id, giant)| Some((id, giant.limbs.as_ref()?)))
            .flat_map(|(id, limbs)| limbs.fists.iter().enumerate().filter(|(_, fist)| fist.holds.is_some_and(|(held, _)| held == car)).map(move |(hand, _)| (id, hand)))
            .collect();
        for (id, hand) in holders {
            self.release_car(id, hand);
        }
    }

    /// One tick of every giant's limbs, before the world steps: each fist moves towards where it is asked
    /// to be, and each foot stands or steps.
    pub(super) fn step_limbs(&mut self, dt: f32) {
        let ids: Vec<u32> = self.cars.iter().filter(|(_, car)| car.limbs.is_some()).map(|(&id, _)| id).collect();
        for id in ids {
            let Some(car) = self.cars.get(&id) else { continue };
            let spec = car.spec;
            let Some(hull) = self.bodies.get(car.body) else { continue };
            let (pose, linvel, angvel) = (*hull.position(), hull.linvel(), hull.angvel());
            let up = pose.rotation * Vector::Y;
            let gait = spec.gait;
            // Fallen, wrecked or with nobody aboard, its arms hang and its feet stay where they are.
            let able = car.flight.powered && car.condition.power > 0.5 && gait.is_none_or(|gait| up.y >= gait.fallen_up);
            // Where the ground is under each foot's hip and under where it would step to.
            let ground = |x: f32, z: f32, top: f32, reach: f32| {
                let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::only_fixed());
                queries.cast_ray(&Ray::new(Vector::new(x, top, z), Vector::new(0.0, -1.0, 0.0)), reach, true).map(|(_, distance)| top - distance)
            };
            let nose = pose.rotation * Vector::new(0.0, 0.0, -1.0);
            let heading = Vector::new(nose.x, 0.0, nose.z).normalize_or(Vector::new(0.0, 0.0, -1.0));
            let right = heading.cross(Vector::Y);
            let flat = Vector::new(linvel.x, 0.0, linvel.z);
            let facing = (-heading.x).atan2(-heading.z);
            let (throttle, crouch) = if able { (car.flight.throttle.clamp(-1.0, 1.0), (-car.flight.lift).clamp(0.0, 1.0)) } else { (0.0, 0.0) };
            // What a tick makes of its feet: where each is, the step, the weight on the left, and which came down.
            let mut stepped: Option<([Vector; 2], Stance, [bool; 2])> = None;
            if let (Some(gait), Some(limbs)) = (gait, &car.limbs)
                && let (Some(shape), Some(feet)) = (gait.feet, &limbs.feet)
            {
                let hip_y = pose.translation.y - spec.half_extents[1];
                let top = hip_y + GROUND_PROBE;
                let reach = gait.leg_length + 2.0 * GROUND_PROBE;
                let centre = Vector::new(pose.translation.x, 0.0, pose.translation.z);
                // Where the hip of foot `index` will be over in `ahead` seconds, on the ground plane.
                let home = |index: usize, ahead: f32, turned: f32| centre + flat * ahead + Rotation::from_rotation_y(angvel.y * turned) * (right * (side(index) * gait.foot_half));
                let mut at = [feet[0].at, feet[1].at];
                let mut stance = Stance { step: limbs.stance.step, weight: limbs.stance.weight, impact: 0.0 };
                let mut landed = [false; 2];
                let away = |index: usize, at: &[Vector; 2]| {
                    let off = home(index, 0.0, 0.0) - at[index];
                    Vector::new(off.x, 0.0, off.z)
                };
                // The way it walks: the way it is going, when its pilot walks it or it is carried along (a
                // blow, or coming to a stop), or the way its pilot has just asked for. Slower than that it
                // only brings its feet back under it, or it would follow them about.
                let speed = flat.length();
                let way = if speed >= if throttle.abs() > WALKED { SETTLE_SPEED } else { CARRIED_SPEED } {
                    flat / speed
                } else if throttle.abs() > WALKED {
                    heading * throttle.signum()
                } else {
                    Vector::ZERO
                };
                let walking = way != Vector::ZERO;
                // It steps as far as it goes in the time a step takes, shorter crouched, and no shorter
                // than its walk's when it is walked.
                let cycle = shape.step_time + shape.shift_time;
                let least = if throttle.abs() > WALKED { shape.stride_min } else { 2.0 * SETTLE_REACH };
                let stride = (speed * cycle).clamp(least, shape.stride) * (1.0 - crouch * gait.crouch / gait.leg_length);
                if (0..2).any(|index| at[index].x == f32::MAX || away(index, &at).length() > LOST_STRIDES * shape.stride) {
                    at = [0, 1].map(|index| {
                        let under = home(index, 0.0, 0.0);
                        let y = ground(under.x, under.z, top, reach).unwrap_or(hip_y - gait.leg_length);
                        Vector::new(under.x, y, under.z)
                    });
                    stance = Stance::LEVEL;
                } else if stance.step == Step::Airborne {
                    // Off the ground its feet hang under its hips, a little drawn up. When one meets the
                    // ground on the way down, both are planted where they are.
                    let hang = HANG * (gait.leg_length - crouch * gait.crouch);
                    let mut met = false;
                    for (index, foot) in at.iter_mut().enumerate() {
                        let under = home(index, 0.0, 0.0);
                        let below = (foot.y - hip_y + linvel.y * dt).max(-gait.leg_length);
                        let y = hip_y + below + (-hang - below).clamp(-TUCK_SPEED * dt, TUCK_SPEED * dt);
                        let floor = ground(under.x, under.z, top, reach);
                        met |= floor.is_some_and(|floor| y <= floor);
                        *foot = Vector::new(under.x, floor.map_or(y, |floor| y.max(floor)), under.z);
                    }
                    if met && linvel.y <= 0.0 {
                        stance = Stance { step: Step::Planted, weight: 0.5, impact: -linvel.y };
                        landed = [true; 2];
                    }
                } else if (0..2).all(|index| stance.step.swinging().is_some_and(|(foot, _)| foot == index) || hip_y - at[index].y >= gait.leg_length) {
                    // Lifted clear of every foot that was carrying it (a leap, a blast): it is off the ground.
                    stance = Stance { step: Step::Airborne, weight: 0.5, impact: 0.0 };
                } else if let Step::Swinging(index, progress) = stance.step {
                    // In the air, the foot goes to where its hip will be when it lands, and half a stride
                    // beyond: the hull is then carried over it.
                    let left = (1.0 - progress) * shape.step_time;
                    let to = home(index, left, left + 0.5 * cycle) + way * (0.5 * stride);
                    let share = (dt / left.max(dt)).min(1.0);
                    let y = ground(to.x, to.z, top, reach).unwrap_or(at[index].y);
                    at[index] += (Vector::new(to.x, y, to.z) - at[index]) * share;
                    let progress = progress + dt / shape.step_time;
                    stance.step = if progress < 1.0 { Step::Swinging(index, progress) } else { Step::Planted };
                    stance.weight = if index == 0 { 0.0 } else { 1.0 };
                    landed[index] = progress >= 1.0;
                } else {
                    // On both feet. Walking, a foot is due to step once the hull has gone half a stride past
                    // it, or has no foot ahead to go onto; at rest, once it has been left out of place.
                    let behind = |index: usize| away(index, &at).dot(way);
                    let on = |index: usize, weight: f32| if index == 0 { weight } else { 1.0 - weight };
                    let due = |index: usize, slack: f32| {
                        able && if walking {
                            behind(index) + speed * shape.shift_time >= 0.5 * stride * slack || behind(1 - index) > -0.1 * stride
                        } else {
                            away(index, &at).length() > SETTLE_REACH * slack
                        }
                    };
                    let pace = dt / shape.shift_time;
                    match stance.step {
                        // The weight comes off the foot that is to step, and it lifts when it is light.
                        Step::Shifting(next) if due(next, 0.5) => {
                            let wanted = if next == 0 { 0.0 } else { 1.0 };
                            stance.weight += (wanted - stance.weight).clamp(-pace, pace);
                            if on(next, stance.weight) <= LIGHT {
                                stance = Stance { step: Step::Swinging(next, 0.0), weight: wanted, impact: 0.0 };
                            }
                        }
                        _ => {
                            // The next to step is the one furthest behind; at rest, the one furthest out of
                            // place, and of two much alike the one that did not step last, which has the weight.
                            let lag = |index: usize| if walking { behind(index) } else { away(index, &at).length() + on(index, stance.weight) * 2.0 * SETTLE_REACH };
                            let next = if lag(0) >= lag(1) { 0 } else { 1 };
                            if due(next, 1.0) {
                                stance.step = Step::Shifting(next);
                            } else {
                                stance.step = Step::Planted;
                                stance.weight += (0.5 - stance.weight).clamp(-pace, pace);
                            }
                        }
                    }
                }
                stepped = Some((at, stance, landed));
            }
            let Some(car) = self.cars.get_mut(&id) else { continue };
            let Some(limbs) = car.limbs.as_mut() else { continue };
            if let (Some(feet), Some(shape), Some((at, stance, landed))) = (limbs.feet.as_mut(), gait.and_then(|gait| gait.feet), stepped) {
                for (index, foot) in feet.iter_mut().enumerate() {
                    foot.at = at[index];
                    foot.landed = landed[index];
                    // A planted foot points the way it was put down; in the air it comes round to the hull's.
                    let up = match stance.step {
                        Step::Swinging(stepping, progress) if stepping == index => {
                            let turn = (facing - foot.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                            foot.yaw += turn * (dt / ((1.0 - progress) * shape.step_time).max(dt)).min(1.0);
                            lifted(&shape, progress)
                        }
                        _ => 0.0,
                    };
                    if landed[index] {
                        foot.yaw = facing;
                    }
                    if let Some(body) = self.bodies.get_mut(foot.body) {
                        body.set_next_kinematic_position(Pose::from_parts(foot.at + Vector::new(0.0, shape.half[1] + up, 0.0), Rotation::from_rotation_y(foot.yaw)));
                    }
                }
                limbs.stance = stance;
            }
            let Some(arms) = spec.arms else { continue };
            for (index, fist) in limbs.fists.iter_mut().enumerate() {
                let shoulder = mirrored(arms.shoulder, index);
                let wanted = fist.hand.target.filter(|_| able).map_or(mirrored(arms.rest, index), Vector::from);
                let wanted = within_reach(spec, &arms, shoulder, wanted);
                // As fast as it can still stop from: it slows into where it is going rather than past it.
                let away = wanted - fist.at;
                let distance = away.length();
                let speed = (2.0 * arms.accel * distance).sqrt().min(arms.speed).min(distance / dt);
                let aim = if distance > 1e-4 { away * (speed / distance) } else { Vector::ZERO };
                fist.velocity += (aim - fist.velocity).clamp_length_max(arms.accel * dt);
                let before = fist.at;
                fist.at = within_reach(spec, &arms, shoulder, fist.at + fist.velocity * dt);
                // What the reach took off its travel is taken off its speed.
                fist.velocity = (fist.at - before) / dt;
                fist.world_velocity = linvel + angvel.cross(pose.rotation * fist.at) + pose.rotation * fist.velocity;
                if let Some(body) = self.bodies.get_mut(fist.body) {
                    body.set_next_kinematic_position(Pose::from_parts(pose * fist.at, pose.rotation));
                }
            }
        }
    }
}

/// Arm `index` of `car` as the client bends it, in the hull frame: its shoulder, its elbow and the end
/// of its forearm, at the fist's middle when that is within reach. `None` for a car without arms.
fn arm(car: &Car, index: usize) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let (arms, limbs) = (car.spec.arms?, car.limbs.as_ref()?);
    let fist = limbs.fists.get(index)?;
    let shoulder: [f32; 3] = mirrored(arms.shoulder, index).into();
    let (elbow, end) = bend(shoulder, fist.at.into(), arms.upper, arms.fore, mirrored(arms.elbow_pole, index).into());
    Some((shoulder, elbow, end))
}
