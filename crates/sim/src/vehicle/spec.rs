//! The tuning of each vehicle kind that the physics reads, which is part of its definition
//! (`shared::vehicles`).

pub(crate) use shared::vehicles::{Buttons, Lift, Role, Shape, Solid, Spec};
use shared::vehicles::vehicle_def;

/// The spec for a vehicle kind. Unknown kinds are jeeps.
pub(crate) fn spec(kind: u8) -> &'static Spec {
    &vehicle_def(kind).spec
}

/// Half extents of the chassis box of a vehicle kind.
pub fn half_extents(kind: u8) -> [f32; 3] {
    spec(kind).half_extents
}
