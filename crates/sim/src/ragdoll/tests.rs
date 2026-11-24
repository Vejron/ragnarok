use super::*;
use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};

fn flat() -> Physics {
    Physics::new(&WorldFile { version: WORLD_VERSION, half_extent: 10_000, ..Default::default() }).unwrap()
}

/// Flat ground with a 2 m tall box from (5, 0, -1) to (6, 2, 1).
fn walled() -> Physics {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(5.0, 0.0, -1.0), v(6.0, 0.0, -1.0), v(6.0, 0.0, 1.0), v(5.0, 0.0, 1.0),
        v(5.0, 2.0, -1.0), v(6.0, 2.0, -1.0), v(6.0, 2.0, 1.0), v(5.0, 2.0, 1.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    Physics::new(&WorldFile { version: WORLD_VERSION, half_extent: 10_000, collision: CollisionMesh { vertices, triangles }, ..Default::default() }).unwrap()
}

/// A soldier standing with their feet at `(x, 0, 0)`, facing -z (their right is +x), as segments.
fn standing(x: f32) -> Vec<f32> {
    let parts: [([f32; 3], [f32; 3]); PARTS] = [
        ([-0.1, 0.95, 0.0], [0.1, 0.95, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 1.45, 0.0]),
        ([0.0, 1.5, 0.0], [0.0, 1.75, 0.0]),
        ([-0.2, 1.42, 0.0], [-0.2, 1.12, 0.0]),
        ([-0.2, 1.12, 0.0], [-0.2, 0.85, 0.0]),
        ([0.2, 1.42, 0.0], [0.2, 1.12, 0.0]),
        ([0.2, 1.12, 0.0], [0.2, 0.85, 0.0]),
        ([-0.1, 0.95, 0.0], [-0.1, 0.5, 0.0]),
        ([-0.1, 0.5, 0.0], [-0.1, 0.08, 0.0]),
        ([0.1, 0.95, 0.0], [0.1, 0.5, 0.0]),
        ([0.1, 0.5, 0.0], [0.1, 0.08, 0.0]),
    ];
    parts.iter().flat_map(|(a, b)| [a[0] + x, a[1], a[2], b[0] + x, b[1], b[2]]).collect()
}

fn run(dolls: &mut Ragdolls, physics: &Physics, seconds: f32) {
    for _ in 0..(seconds * 60.0) as u32 {
        dolls.step(physics, 1.0 / 60.0);
    }
}

fn pose_of(dolls: &Ragdolls, key: u32) -> Vec<[f32; 7]> {
    let mut out = vec![0.0; PARTS * POSE_FLOATS];
    assert!(dolls.pose(key, &mut out));
    out.chunks(POSE_FLOATS).map(|c| c.try_into().unwrap()).collect()
}

#[test]
fn a_thrown_ragdoll_tumbles_and_comes_to_rest_on_the_ground() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    assert!(dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [8.0, 5.0, 0.0], [0.0, 0.0, -6.0], 0));
    run(&mut dolls, &physics, 3.0);
    let pose = pose_of(&dolls, 1);
    for (part, p) in pose.iter().enumerate() {
        assert!(p[1] > -0.05 && p[1] < 0.6, "part {part} lies on the ground: {p:?}");
    }
    assert!(pose[PELVIS][0] > 2.0, "thrown along: {:?}", pose[PELVIS]);
    let pelvis = dolls.bodies.get(dolls.dolls[&1].parts[PELVIS].unwrap()).unwrap();
    assert!(pelvis.linvel().length() < 0.3, "at rest: {:?}", pelvis.linvel());
}

#[test]
fn its_joints_hold_together() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    let segments = standing(0.0);
    dolls.spawn(&physics, 1, &segments, [1.0, 0.0, 0.0], [12.0, 8.0, 3.0], [4.0, 2.0, -9.0], 0);
    let length = |part: usize| {
        let s = &segments[part * SEGMENT_FLOATS..];
        Vector::new(s[3] - s[0], s[4] - s[1], s[5] - s[2]).length()
    };
    for _ in 0..12 {
        run(&mut dolls, &physics, 0.25);
        let pose = pose_of(&dolls, 1);
        for part in 1..PARTS {
            let parent = PARENT[part];
            let gap = Vector::new(pose[part][0] - pose[parent][0], pose[part][1] - pose[parent][1], pose[part][2] - pose[parent][2]).length();
            // The pelvis is a crosspiece and the shoulders hang off the torso's side.
            assert!(gap < (length(part) + length(parent)) * 0.5 + 0.3, "part {part} came off its parent: {gap}");
        }
    }
}

#[test]
fn the_pelvis_is_pulled_to_where_the_soldier_is() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [10.0, 4.0, 0.0], [0.0, 0.0, -5.0], 0);
    dolls.set_target(1, [3.0, 0.85, 1.0]);
    run(&mut dolls, &physics, 3.0);
    let pelvis = pose_of(&dolls, 1)[PELVIS];
    assert!((pelvis[0] - 3.0).hypot(pelvis[2] - 1.0) < 0.5, "{pelvis:?}");
}

#[test]
fn a_ragdoll_thrown_at_a_wall_stops_at_it() {
    let physics = walled();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(3.0), [1.0, 0.0, 0.0], [20.0, 2.0, 0.0], [0.0, 0.0, -8.0], 0);
    run(&mut dolls, &physics, 2.0);
    for (part, p) in pose_of(&dolls, 1).iter().enumerate() {
        assert!(p[0] < 5.05 || p[1] > 2.0, "part {part} went into the wall: {p:?}");
    }
}

#[test]
fn lost_limbs_are_left_out_and_take_their_parents_pose() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [0.0; 3], [0.0; 3], LIMB_LEG_L | LIMB_ARM_R);
    assert_eq!(dolls.dolls[&1].parts.iter().flatten().count(), PARTS - 4);
    let pose = pose_of(&dolls, 1);
    assert_eq!(pose[8], pose[0], "the left shin goes with the pelvis");
    assert_eq!(pose[6], pose[1], "the right forearm with the torso");
    dolls.remove(1);
    assert!(dolls.is_empty());
    assert_eq!(dolls.bodies.len(), 0);
}

#[test]
fn scenery_is_brought_in_near_ragdolls_and_let_go_after() {
    let physics = walled();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(3.0), [1.0, 0.0, 0.0], [0.0; 3], [0.0; 3], 0);
    assert!(!dolls.scenery.is_empty());
    dolls.remove(1);
    dolls.bring_scenery(&physics);
    assert!(dolls.scenery.is_empty());
    assert_eq!(dolls.colliders.len(), 0);
}

/// The angle between the lines of parts `a` and `b`, radians.
fn bend(pose: &[[f32; 7]], a: usize, b: usize) -> f32 {
    let axis = |p: &[f32; 7]| Rotation::from_xyzw(p[3], p[4], p[5], p[6]) * Vector::Y;
    axis(&pose[a]).dot(axis(&pose[b])).clamp(-1.0, 1.0).acos()
}

#[test]
fn a_ragdoll_thrown_at_blast_speed_keeps_every_elbow_knee_and_neck_within_its_limits() {
    let physics = walled();
    let mut dolls = Ragdolls::new();
    // As hard as a blast throws anyone, into a wall, spinning.
    dolls.spawn(&physics, 1, &standing(2.0), [1.0, 0.0, 0.0], [shared::consts::KNOCK_MAX_SPEED, 9.0, 2.0], [3.0, 1.0, -10.0], 0);
    let mut worst = [0.0f32; PARTS];
    for _ in 0..(3 * 60) {
        run(&mut dolls, &physics, 1.0 / 60.0);
        let pose = pose_of(&dolls, 1);
        for part in [2, 4, 6, 8, 10] {
            worst[part] = worst[part].max(bend(&pose, part, PARENT[part]));
        }
    }
    for (part, limit) in [(2, 1.0f32), (4, 2.5), (6, 2.5), (8, 2.5), (10, 2.5)] {
        assert!(worst[part] < limit + 0.35, "part {part} bent {} rad against its limit of {limit}", worst[part]);
    }
}

#[test]
fn an_untethered_ragdoll_comes_to_rest_and_says_so() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [6.0, 4.0, 0.0], [0.0, 0.0, -5.0], 0);
    dolls.set_target(1, [0.0, 0.85, 0.0]);
    dolls.clear_target(1);
    run(&mut dolls, &physics, 0.1);
    assert!(!dolls.asleep(1), "still flying");
    run(&mut dolls, &physics, 5.0);
    assert!(dolls.asleep(1), "lying still");
    assert!(pose_of(&dolls, 1)[PELVIS][0] > 1.5, "where it was thrown, not pulled back");
}

#[test]
fn a_push_after_the_ragdoll_is_made_carries_it_and_one_to_the_head_snaps_it_back() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [0.0; 3], [0.0; 3], 0);
    dolls.push(1, [0.0, 0.0, 5.0], Some(2));
    run(&mut dolls, &physics, 0.1);
    let pose = pose_of(&dolls, 1);
    assert!(pose[2][2] - pose[PELVIS][2] > 0.05, "the head goes first: {:?}", pose[2]);
    dolls.push(1, [6.0, 2.0, 0.0], None);
    run(&mut dolls, &physics, 1.0);
    assert!(pose_of(&dolls, 1)[PELVIS][0] > 1.0, "the whole body is carried");
}

#[test]
fn a_severed_limb_leaves_the_ragdoll_and_the_rest_holds_together() {
    let physics = flat();
    let mut dolls = Ragdolls::new();
    dolls.spawn(&physics, 1, &standing(0.0), [1.0, 0.0, 0.0], [8.0, 5.0, 0.0], [0.0, 0.0, -6.0], 0);
    run(&mut dolls, &physics, 0.2);
    let bodies = dolls.bodies.len();
    dolls.sever(1, LIMB_ARM_L);
    assert_eq!(dolls.bodies.len(), bodies - 2, "upper arm and forearm go");
    run(&mut dolls, &physics, 2.0);
    let pose = pose_of(&dolls, 1);
    assert_eq!(pose[4], pose[1], "the lost forearm takes the torso's pose");
    for part in [2, 5, 7, 9] {
        let gap = Vector::new(pose[part][0] - pose[PARENT[part]][0], pose[part][1] - pose[PARENT[part]][1], pose[part][2] - pose[PARENT[part]][2]).length();
        assert!(gap < 0.7, "part {part} is still on: {gap}");
    }
}
