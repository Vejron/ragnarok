use super::*;

const CENTRE: [f32; 3] = [10.0, 1.0, -4.0];

fn at([x, y, z]: [f32; 3]) -> [f32; 3] {
    [CENTRE[0] + x, CENTRE[1] + y, CENTRE[2] + z]
}

#[test]
fn standing_a_round_low_down_is_in_a_leg_and_one_out_by_the_chest_in_an_arm() {
    // Facing -Z: its right is +X.
    assert_eq!(limb_at(CENTRE, 0.0, false, at([0.3, 0.1, 0.0])), Some(LIMB_ARM_R));
    assert_eq!(limb_at(CENTRE, 0.0, false, at([-0.3, 0.1, 0.0])), Some(LIMB_ARM_L));
    assert_eq!(limb_at(CENTRE, 0.0, false, at([0.1, -0.4, 0.0])), Some(LIMB_LEG_R));
    assert_eq!(limb_at(CENTRE, 0.0, false, at([-0.1, -0.6, -0.3])), Some(LIMB_LEG_L));
    assert_eq!(limb_at(CENTRE, 0.0, false, at([0.05, 0.2, -0.3])), None, "the chest");
    assert_eq!(limb_at(CENTRE, 0.0, false, at([0.3, 0.5, 0.0])), None, "above the shoulders");
}

#[test]
fn which_side_turns_with_the_body() {
    // A quarter turn left faces -X, so its right is -Z.
    let yaw = std::f32::consts::FRAC_PI_2;
    assert_eq!(limb_at(CENTRE, yaw, false, at([0.0, 0.1, -0.3])), Some(LIMB_ARM_R));
    assert_eq!(limb_at(CENTRE, yaw, false, at([0.0, -0.4, 0.2])), Some(LIMB_LEG_L));
}

#[test]
fn lying_down_the_legs_are_behind_and_the_arms_out_in_front() {
    assert_eq!(limb_at(CENTRE, 0.0, true, at([0.05, 0.0, 0.25])), Some(LIMB_LEG_R), "behind");
    assert_eq!(limb_at(CENTRE, 0.0, true, at([-0.3, 0.0, -0.1])), Some(LIMB_ARM_L));
    assert_eq!(limb_at(CENTRE, 0.0, true, at([0.0, 0.1, -0.1])), None);
}
