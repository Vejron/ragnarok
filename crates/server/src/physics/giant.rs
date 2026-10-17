//! The Orange Plague as solid matter: a kinematic capsule for each solid part of him, posed every
//! tick. Characters walk into him and cars bounce off him, but rays go through: rounds, shells and
//! sight lines test his own hit capsules (`mechanics/plague.rs`), which also know which part was hit.

use super::Physics;
use rapier3d::prelude::*;

/// The collision group his capsules are in. Ray queries leave it out (`Physics::rays`).
pub const GIANT: Group = Group::GROUP_2;

/// A query filter that ignores him: for anything but walking.
pub fn without_giant(filter: QueryFilter<'_>) -> QueryFilter<'_> {
    filter.groups(InteractionGroups::new(Group::ALL, Group::ALL.difference(GIANT), InteractionTestMode::And))
}

/// The pose that puts a Y-up capsule (or a ball) on the segment `a`..`b`.
fn segment_pose(a: [f32; 3], b: [f32; 3]) -> Pose {
    let mid = Vector::new((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0);
    let along = Vector::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    let rotation = if along.length() < 1e-6 { Rotation::IDENTITY } else { Rotation::from_rotation_arc(Vector::Y, along.normalize()) };
    Pose::from_parts(mid, rotation)
}

fn half_length(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt() / 2.0
}

impl Physics {
    /// Puts his solid capsules (`a`, `b`, radius, in the world) where they are this tick. The first
    /// call makes them; a call with a different count makes them anew. They move with the next step.
    pub fn set_giant(&mut self, capsules: &[([f32; 3], [f32; 3], f32)]) {
        if self.giant.len() != capsules.len() {
            self.clear_giant();
            for &(a, b, radius) in capsules {
                let body = self.bodies.insert(RigidBodyBuilder::kinematic_position_based().pose(segment_pose(a, b)).build());
                let half = half_length(a, b);
                let shape = if half < 1e-3 { ColliderBuilder::ball(radius) } else { ColliderBuilder::capsule_y(half, radius) };
                let collider = shape.collision_groups(InteractionGroups::new(GIANT, Group::ALL, InteractionTestMode::And)).friction(0.6).build();
                self.colliders.insert_with_parent(collider, body, &mut self.bodies);
                self.giant.push(body);
            }
            // Characters query the broad-phase before the next step: put them in it now.
            self.step_pipeline();
            return;
        }
        for (&handle, &(a, b, _)) in self.giant.iter().zip(capsules) {
            if let Some(body) = self.bodies.get_mut(handle) {
                body.set_next_kinematic_position(segment_pose(a, b));
            }
        }
    }

    /// Takes him out of the world.
    pub fn clear_giant(&mut self) {
        for handle in std::mem::take(&mut self.giant) {
            self.bodies.remove(handle, &mut self.islands, &mut self.colliders, &mut self.impulse_joints, &mut self.multibody_joints, &mut self.soft_bodies, true);
        }
    }

    /// Whether he is in the world as solid matter.
    pub fn has_giant(&self) -> bool {
        !self.giant.is_empty()
    }
}
