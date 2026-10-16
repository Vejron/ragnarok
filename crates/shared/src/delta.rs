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
    let delta = EntityDelta { id: now.id, dx: now.x - base.x, dy: now.y - base.y, dz: now.z - base.z, dyaw: now.yaw as i32 - base.yaw as i32, dvy: now.vy as i32 - base.vy as i32, flags: now.flags, stance: now.stance, dknock_x: now.knock_x as i32 - base.knock_x as i32, dknock_z: now.knock_z as i32 - base.knock_z as i32 };
    let unchanged = delta.dx == 0 && delta.dy == 0 && delta.dz == 0 && delta.dyaw == 0 && delta.dvy == 0 && delta.flags == base.flags && delta.dknock_x == 0 && delta.dknock_z == 0;
    (!unchanged).then_some(delta)
}

fn apply_entity(base: &EntityState, delta: &EntityDelta) -> EntityState {
    EntityState { id: base.id, kind: base.kind, x: base.x + delta.dx, y: base.y + delta.dy, z: base.z + delta.dz, yaw: (base.yaw as i32 + delta.dyaw) as i16, vy: (base.vy as i32 + delta.dvy) as i16, flags: delta.flags, stance: delta.stance, knock_x: (base.knock_x as i32 + delta.dknock_x) as i16, knock_z: (base.knock_z as i32 + delta.dknock_z) as i16 }
}

fn vehicle_delta(base: &VehicleState, now: &VehicleState) -> Option<VehicleDelta> {
    let seats = (now.seats != base.seats).then(|| now.seats.clone());
    let turret: Option<Turret> = if now.turret != base.turret { now.turret } else { None };
    let delta = VehicleDelta { id: now.id, dx: now.x - base.x, dy: now.y - base.y, dz: now.z - base.z, dqx: now.qx as i32 - base.qx as i32, dqy: now.qy as i32 - base.qy as i32, dqz: now.qz as i32 - base.qz as i32, dsteer: now.steer as i32 - base.steer as i32, seats, turret, hp: now.hp, damage: (now.damage != base.damage).then(|| now.damage.unwrap_or_default()), arms: if now.arms != base.arms { now.arms } else { None }, thrust: if now.thrust != base.thrust { now.thrust } else { None } };
    let unchanged = delta.dx == 0 && delta.dy == 0 && delta.dz == 0 && delta.dqx == 0 && delta.dqy == 0 && delta.dqz == 0 && delta.dsteer == 0
        && delta.seats.is_none() && delta.turret.is_none() && now.hp == base.hp && delta.damage.is_none() && delta.arms.is_none() && delta.thrust.is_none();
    (!unchanged).then_some(delta)
}

fn apply_vehicle(base: &VehicleState, delta: &VehicleDelta) -> VehicleState {
    VehicleState { id: base.id, kind: base.kind, x: base.x + delta.dx, y: base.y + delta.dy, z: base.z + delta.dz, qx: (base.qx as i32 + delta.dqx) as i16, qy: (base.qy as i32 + delta.dqy) as i16, qz: (base.qz as i32 + delta.dqz) as i16, steer: (base.steer as i32 + delta.dsteer) as i8, seats: delta.seats.clone().unwrap_or_else(|| base.seats.clone()), turret: delta.turret.or(base.turret), hp: delta.hp, damage: match delta.damage {
            Some(damage) => (damage != [0; 8]).then_some(damage),
            None => base.damage,
        }, arms: delta.arms.or(base.arms), thrust: delta.thrust.or(base.thrust) }
}

/// What to send so that a client holding `base` ends up holding `now`. Both scenes must be
/// ordered by id. A vehicle with a turret or arms that did not change sends `None` for them: the
/// client keeps its old ones. (A vehicle never loses its turret or arms, so `None` is never news.)
pub fn diff(base: &Scene, now: &Scene) -> Delta {
    let mut delta = Delta::default();
    for entity in &now.entities {
        match base.entities.binary_search_by_key(&entity.id, |e| e.id) {
            Ok(i) => delta.entities.extend(entity_delta(&base.entities[i], entity)),
            Err(_) => delta.new_entities.push(*entity),
        }
    }
    for vehicle in &now.vehicles {
        match base.vehicles.binary_search_by_key(&vehicle.id, |v| v.id) {
            Ok(i) => delta.vehicles.extend(vehicle_delta(&base.vehicles[i], vehicle)),
            Err(_) => delta.new_vehicles.push(vehicle.clone()),
        }
    }
    delta.removed.extend(base.entities.iter().map(|e| e.id).filter(|id| now.entities.binary_search_by_key(id, |e| e.id).is_err()));
    delta.removed.extend(base.vehicles.iter().map(|v| v.id).filter(|id| now.vehicles.binary_search_by_key(id, |v| v.id).is_err()));
    delta
}

/// The scene a client holds after receiving `delta` on top of `base`. A delta for something the
/// base does not have is ignored (it cannot happen on a well-formed stream).
pub fn apply(base: &Scene, delta: &Delta) -> Scene {
    let mut entities: Vec<EntityState> = base
        .entities
        .iter()
        .filter(|e| !delta.removed.contains(&e.id))
        .map(|e| match delta.entities.iter().find(|d| d.id == e.id) {
            Some(d) => apply_entity(e, d),
            None => *e,
        })
        .collect();
    entities.extend(delta.new_entities.iter().copied());
    entities.sort_by_key(|e| e.id);
    let mut vehicles: Vec<VehicleState> = base
        .vehicles
        .iter()
        .filter(|v| !delta.removed.contains(&v.id))
        .map(|v| match delta.vehicles.iter().find(|d| d.id == v.id) {
            Some(d) => apply_vehicle(v, d),
            None => v.clone(),
        })
        .collect();
    vehicles.extend(delta.new_vehicles.iter().cloned());
    vehicles.sort_by_key(|v| v.id);
    Scene { entities, vehicles }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ARMS_CUT, Arms, FLAG_GROUNDED, KIND_JEEP, KIND_PLANE, KIND_STICKMAN, KIND_TANK};

    fn person(id: u32, x: i32) -> EntityState {
        EntityState { id, kind: KIND_STICKMAN, x, y: 90, z: -x, yaw: 100, vy: 0, flags: FLAG_GROUNDED, stance: 0, knock_x: 0, knock_z: 0 }
    }

    fn car(id: u32, x: i32, seats: Vec<u32>) -> VehicleState {
        VehicleState { id, kind: KIND_JEEP, x, y: 40, z: 7, qx: 10, qy: -20, qz: 30, steer: 5, seats, turret: None, hp: 100, damage: None, arms: None, thrust: None }
    }

    fn tank(id: u32, yaw: i16) -> VehicleState {
        VehicleState { id, kind: KIND_TANK, x: 0, y: 40, z: 0, qx: 0, qy: 0, qz: 0, steer: 0, seats: vec![0, 0], turret: Some(Turret { yaw, pitch: 100, shots: 1, range: 500, flags: 0 }), hp: 250, damage: None, arms: None, thrust: None }
    }

    fn plane(id: u32, arms: Arms) -> VehicleState {
        VehicleState { id, kind: KIND_PLANE, x: 0, y: 140, z: 0, qx: 0, qy: 0, qz: 0, steer: 0, seats: vec![0, 0], turret: Some(Turret { yaw: 0, pitch: 0, shots: 0, range: 0, flags: 0 }), hp: 70, damage: None, arms: Some(arms), thrust: None }
    }

    #[test]
    fn apply_reproduces_the_new_scene() {
        let base = Scene { entities: vec![person(1, 100), person(2, 200), person(5, 300)], vehicles: vec![car(9, 0, vec![0; 4]), tank(10, 0)] };
        let mut moved = person(2, 230);
        moved.yaw = -3000;
        moved.vy = 650;
        moved.knock_x = -400;
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
        b.x = 400;
        let delta = diff(&Scene { entities: vec![], vehicles: vec![a] }, &Scene { entities: vec![], vehicles: vec![b.clone()] });
        assert_eq!(delta.vehicles[0].seats, None);
        assert_eq!(delta.vehicles[0].turret, None);
        assert_eq!(apply(&Scene { entities: vec![], vehicles: vec![tank(10, 300)] }, &delta).vehicles, vec![b]);
    }

    #[test]
    fn damage_is_sent_when_it_changes_and_a_repair_clears_it() {
        let clean = Scene { entities: vec![], vehicles: vec![car(9, 0, vec![0; 4])] };
        let mut hurt = clean.clone();
        hurt.vehicles[0].damage = Some([90, 0, 0, 20, 0, 0, 0, 255]);
        let delta = diff(&clean, &hurt);
        assert_eq!(delta.vehicles[0].damage, Some([90, 0, 0, 20, 0, 0, 0, 255]));
        assert_eq!(apply(&clean, &delta), hurt);
        // Moving on without new damage does not resend it.
        let mut moved = hurt.clone();
        moved.vehicles[0].x = 50;
        assert_eq!(diff(&hurt, &moved).vehicles[0].damage, None);
        assert_eq!(apply(&hurt, &diff(&hurt, &moved)), moved);
        // A fresh vehicle at home has none.
        let delta = diff(&hurt, &clean);
        assert_eq!(delta.vehicles[0].damage, Some([0; 8]));
        assert_eq!(apply(&hurt, &delta), clean);
    }

    #[test]
    fn arms_are_sent_only_when_they_change() {
        let base = Scene { entities: vec![], vehicles: vec![plane(12, Arms { shots: 0, heat: 0, rear_heat: 0, bombs: 4, flags: 0 })] };
        let mut fired = base.clone();
        fired.vehicles[0].arms = Some(Arms { shots: 3, heat: 31, rear_heat: 0, bombs: 4, flags: ARMS_CUT });
        let delta = diff(&base, &fired);
        assert_eq!(delta.vehicles[0].arms, Some(Arms { shots: 3, heat: 31, rear_heat: 0, bombs: 4, flags: ARMS_CUT }));
        assert_eq!(apply(&base, &delta), fired);
        let mut moved = fired.clone();
        moved.vehicles[0].x = 900;
        let delta = diff(&fired, &moved);
        assert_eq!(delta.vehicles[0].arms, None, "unchanged arms are not resent");
        assert_eq!(apply(&fired, &delta), moved);
        assert!(diff(&moved, &moved).is_empty());
    }
}
