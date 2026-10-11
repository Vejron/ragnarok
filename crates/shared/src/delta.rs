//! Delta compression of snapshots. A snapshot lists only what changed since a *baseline*: the
//! newest snapshot the client says it has rebuilt. Both sides run `apply` over the same lists, so
//! the server knows exactly what the client now holds without being told.
//!
//! All arithmetic is on the wire integers (centimetres, milliradians), never on floats, so the
//! two sides cannot drift. Mirrored in `client/src/net/codec.ts` (`applyDelta`).

use crate::protocol::{EntityDelta, EntityState, Turret, VehicleDelta, VehicleState};

/// Everything a client knows about the world at one snapshot, ordered by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    pub entities: Vec<EntityState>,
    pub vehicles: Vec<VehicleState>,
}

/// The lists of a snapshot that are relative to a baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delta {
    pub new_entities: Vec<EntityState>,
    pub entities: Vec<EntityDelta>,
    pub new_vehicles: Vec<VehicleState>,
    pub vehicles: Vec<VehicleDelta>,
    pub removed: Vec<u32>,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        self.new_entities.is_empty() && self.entities.is_empty() && self.new_vehicles.is_empty() && self.vehicles.is_empty() && self.removed.is_empty()
    }
}

fn entity_delta(base: &EntityState, now: &EntityState) -> Option<EntityDelta> {
    let delta = EntityDelta(
        now.0,
        now.2 - base.2,
        now.3 - base.3,
        now.4 - base.4,
        now.5 as i32 - base.5 as i32,
        now.6 as i32 - base.6 as i32,
        now.7,
        now.8 as i32 - base.8 as i32,
        now.9 as i32 - base.9 as i32,
    );
    let unchanged = delta.1 == 0 && delta.2 == 0 && delta.3 == 0 && delta.4 == 0 && delta.5 == 0 && delta.6 == base.7 && delta.7 == 0 && delta.8 == 0;
    (!unchanged).then_some(delta)
}

fn apply_entity(base: &EntityState, delta: &EntityDelta) -> EntityState {
    EntityState(
        base.0,
        base.1,
        base.2 + delta.1,
        base.3 + delta.2,
        base.4 + delta.3,
        (base.5 as i32 + delta.4) as i16,
        (base.6 as i32 + delta.5) as i16,
        delta.6,
        (base.8 as i32 + delta.7) as i16,
        (base.9 as i32 + delta.8) as i16,
    )
}

fn vehicle_delta(base: &VehicleState, now: &VehicleState) -> Option<VehicleDelta> {
    let seats = (now.9 != base.9).then(|| now.9.clone());
    let turret: Option<Turret> = if now.10 != base.10 { now.10 } else { None };
    let delta = VehicleDelta(
        now.0,
        now.2 - base.2,
        now.3 - base.3,
        now.4 - base.4,
        now.5 as i32 - base.5 as i32,
        now.6 as i32 - base.6 as i32,
        now.7 as i32 - base.7 as i32,
        now.8 as i32 - base.8 as i32,
        seats,
        turret,
        now.11,
    );
    let unchanged = delta.1 == 0 && delta.2 == 0 && delta.3 == 0 && delta.4 == 0 && delta.5 == 0 && delta.6 == 0 && delta.7 == 0
        && delta.8.is_none() && delta.9.is_none() && now.11 == base.11;
    (!unchanged).then_some(delta)
}

fn apply_vehicle(base: &VehicleState, delta: &VehicleDelta) -> VehicleState {
    VehicleState(
        base.0,
        base.1,
        base.2 + delta.1,
        base.3 + delta.2,
        base.4 + delta.3,
        (base.5 as i32 + delta.4) as i16,
        (base.6 as i32 + delta.5) as i16,
        (base.7 as i32 + delta.6) as i16,
        (base.8 as i32 + delta.7) as i8,
        delta.8.clone().unwrap_or_else(|| base.9.clone()),
        delta.9.or(base.10),
        delta.10,
    )
}

/// What to send so that a client holding `base` ends up holding `now`. Both scenes must be
/// ordered by id. A vehicle with a turret whose turret did not change sends `None`: the client
/// keeps its old one.
pub fn diff(base: &Scene, now: &Scene) -> Delta {
    let mut delta = Delta::default();
    for entity in &now.entities {
        match base.entities.binary_search_by_key(&entity.0, |e| e.0) {
            Ok(i) => delta.entities.extend(entity_delta(&base.entities[i], entity)),
            Err(_) => delta.new_entities.push(*entity),
        }
    }
    for vehicle in &now.vehicles {
        match base.vehicles.binary_search_by_key(&vehicle.0, |v| v.0) {
            Ok(i) => delta.vehicles.extend(vehicle_delta(&base.vehicles[i], vehicle)),
            Err(_) => delta.new_vehicles.push(vehicle.clone()),
        }
    }
    delta.removed.extend(base.entities.iter().map(|e| e.0).filter(|id| now.entities.binary_search_by_key(id, |e| e.0).is_err()));
    delta.removed.extend(base.vehicles.iter().map(|v| v.0).filter(|id| now.vehicles.binary_search_by_key(id, |v| v.0).is_err()));
    delta
}

/// The scene a client holds after receiving `delta` on top of `base`. A delta for something the
/// base does not have is ignored (it cannot happen on a well-formed stream).
pub fn apply(base: &Scene, delta: &Delta) -> Scene {
    let mut entities: Vec<EntityState> = base
        .entities
        .iter()
        .filter(|e| !delta.removed.contains(&e.0))
        .map(|e| match delta.entities.iter().find(|d| d.0 == e.0) {
            Some(d) => apply_entity(e, d),
            None => *e,
        })
        .collect();
    entities.extend(delta.new_entities.iter().copied());
    entities.sort_by_key(|e| e.0);
    let mut vehicles: Vec<VehicleState> = base
        .vehicles
        .iter()
        .filter(|v| !delta.removed.contains(&v.0))
        .map(|v| match delta.vehicles.iter().find(|d| d.0 == v.0) {
            Some(d) => apply_vehicle(v, d),
            None => v.clone(),
        })
        .collect();
    vehicles.extend(delta.new_vehicles.iter().cloned());
    vehicles.sort_by_key(|v| v.0);
    Scene { entities, vehicles }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FLAG_GROUNDED, KIND_JEEP, KIND_STICKMAN, KIND_TANK};

    fn person(id: u32, x: i32) -> EntityState {
        EntityState(id, KIND_STICKMAN, x, 90, -x, 100, 0, FLAG_GROUNDED, 0, 0)
    }

    fn car(id: u32, x: i32, seats: Vec<u32>) -> VehicleState {
        VehicleState(id, KIND_JEEP, x, 40, 7, 10, -20, 30, 5, seats, None, 100)
    }

    fn tank(id: u32, yaw: i16) -> VehicleState {
        VehicleState(id, KIND_TANK, 0, 40, 0, 0, 0, 0, 0, vec![0, 0], Some(Turret(yaw, 100, 1, 500)), 250)
    }

    #[test]
    fn apply_reproduces_the_new_scene() {
        let base = Scene { entities: vec![person(1, 100), person(2, 200), person(5, 300)], vehicles: vec![car(9, 0, vec![0; 4]), tank(10, 0)] };
        let mut moved = person(2, 230);
        moved.5 = -3000;
        moved.6 = 650;
        moved.8 = -400;
        let now = Scene {
            entities: vec![person(1, 100), moved, person(7, 50)],
            vehicles: vec![car(9, 120, vec![1, 0, 0, 0]), tank(10, -700)],
        };
        let delta = diff(&base, &now);
        assert_eq!(delta.new_entities, vec![person(7, 50)]);
        assert_eq!(delta.entities.len(), 1, "the one that stood still is not listed");
        assert_eq!(delta.removed, vec![5]);
        assert_eq!(apply(&base, &delta), now);
    }

    #[test]
    fn nothing_changed_is_an_empty_delta() {
        let scene = Scene { entities: vec![person(1, 100)], vehicles: vec![car(9, 0, vec![0; 4]), tank(10, 4)] };
        assert!(diff(&scene, &scene).is_empty());
    }

    #[test]
    fn everything_is_new_against_an_empty_baseline() {
        let now = Scene { entities: vec![person(1, 100), person(2, 5)], vehicles: vec![car(9, 0, vec![0; 4])] };
        let delta = diff(&Scene::default(), &now);
        assert_eq!((delta.new_entities.len(), delta.new_vehicles.len(), delta.removed.len()), (2, 1, 0));
        assert_eq!(apply(&Scene::default(), &delta), now);
    }

    #[test]
    fn a_turret_that_did_not_move_is_not_resent() {
        let a = tank(10, 300);
        let mut b = a.clone();
        b.2 = 400;
        let delta = diff(&Scene { entities: vec![], vehicles: vec![a] }, &Scene { entities: vec![], vehicles: vec![b.clone()] });
        assert_eq!(delta.vehicles[0].8, None);
        assert_eq!(delta.vehicles[0].9, None);
        assert_eq!(apply(&Scene { entities: vec![], vehicles: vec![tank(10, 300)] }, &delta).vehicles, vec![b]);
    }
}
