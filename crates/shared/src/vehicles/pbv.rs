//! The Pbv 302: everything the game knows about it.

use super::*;

/// The Pbv 302 (`KIND_PBV`): an amphibious tracked troop carrier. The driver (seat 0) sits in the middle
/// of the front; the gunner (seat 1) sits to their left under a small turret with a 20 mm automatic
/// cannon; two troops (seats 2 and 3) ride sealed in the back and get out by the rear doors. It seats as
/// many as the Strf 90, is nearly as quick, thinner-skinned and far more lightly armed, and it floats:
/// on open water its tracks paddle it slowly along. Same body axes as the tank.
pub const PBV_SEATS: usize = 4;
/// Half the hull's bounding box, metres: 5.35 m long, 2.86 m wide over the tracks and 1.4 m deep from the
/// belly to the hull roof, which is 1.8 m off the ground.
pub const PBV_HALF_EXTENTS: [f32; 3] = [1.43, 0.7, 2.675];
/// Road wheel radius, metres.
pub const PBV_WHEEL_RADIUS: f32 = 0.42;
/// How far each track runs from the middle, metres.
pub const PBV_TRACK_HALF: f32 = 1.24;
/// Z of the three axles, front first, metres: set well out towards the ends of the hull, so that its
/// tail stays off the ground on a steep climb.
pub const PBV_AXLES_Z: [f32; 3] = [-2.0, 0.0, 2.0];
/// Where the hull's centre settles over flat ground, metres: measured by `a_pbv_settles_at_its_ride_height`.
pub const PBV_RIDE_HEIGHT: f32 = 1.1;
pub const PBV_HARD_POINT_Y: f32 = -0.3;
pub const PBV_SUSPENSION_REST: f32 = 0.44;
/// Top speed on the flat, m/s (66 km/h).
pub const PBV_MAX_SPEED: f32 = 18.3;
/// Yaw rate at full stick, rad/s.
pub const PBV_TURN_RATE: f32 = 1.3;
/// How deep its hull floats, metres: the roof is 0.4 m clear of the water.
pub const PBV_DRAFT: f32 = 1.0;
/// Top speed afloat, m/s (9 km/h), how hard its tracks push it there, m/s², and its yaw rate at full
/// stick, rad/s.
pub const PBV_SWIM_SPEED: f32 = 2.5;
pub const PBV_SWIM_THRUST: f32 = 2.0;
pub const PBV_SWIM_TURN: f32 = 0.5;
/// Where the turret's ring sits in the hull frame (the middle of the turret's solid): at the front
/// left, over the gunner and beside the driver.
pub const PBV_TURRET_PIVOT: [f32; 3] = [-0.75, 0.95, -0.95];
/// The turret's solid: a small cylinder on the ring, its base on the hull's roof and its top 2.3 m off
/// the ground, metres.
pub const PBV_TURRET_RADIUS: f32 = 0.5;
pub const PBV_TURRET_HALF_HEIGHT: f32 = 0.25;
/// How far the muzzle is from the ring, metres: it reaches a little past the nose.
pub const PBV_BARREL_LENGTH: f32 = 2.0;
/// How fast the turret and barrel follow the gunner's aim, rad/s.
pub const PBV_TURRET_SLEW: f32 = 1.8;
pub const PBV_BARREL_SLEW: f32 = 1.4;
/// Barrel elevation limits, radians (+ is up): −10 to +37 degrees.
pub const PBV_BARREL_MIN: f32 = -0.17;
pub const PBV_BARREL_MAX: f32 = 0.65;
/// Ticks between rounds: ten a second.
pub const PBV_CANNON_COOLDOWN_TICKS: u32 = 6;
/// How far a round strays from the bore, radians.
pub const PBV_CANNON_SPREAD: f32 = 0.006;
/// Heat a round puts in the barrel, of 255: about four seconds of holding the trigger jams it.
pub const PBV_CANNON_HEAT: f32 = 12.0;
/// How far a round flies, metres.
pub const PBV_CANNON_RANGE: f32 = 400.0;
/// What a round takes off a soldier, a vehicle's hull and an aircraft.
pub const PBV_CANNON_DAMAGE: u8 = 45;
pub const PBV_CANNON_VEHICLE_DAMAGE: u8 = 10;
pub const PBV_CANNON_PLANE_DAMAGE: u8 = 20;
/// Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
/// rear doors.
pub const PBV_DOOR_EXIT: [f32; 3] = [0.0, -0.1, 3.6];

/// The hull's side outline, `[z, y]`, a blunt boat's: a short upright nose plate at mid height, from
/// which the glacis climbs at 38 degrees above level to the roof and the bow plate drops back to the
/// belly; a flat roof; and the upright rear with the two doors in it.
const PROFILE: &[[f32; 2]] = &[[-2.675, 0.1], [-1.9, 0.7], [2.675, 0.7], [2.675, -0.35], [2.4, -0.7], [-2.25, -0.7], [-2.675, -0.2]];

/// Thin plate all round, proof against rifles and splinters: 23 mm in front, where the glacis leans far
/// enough back that a level shot has half as much again to get through. The driver looks out through a
/// periscope in the middle of the glacis and the gunner through a sight in the turret's face; the rear
/// is the two troop doors.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's periscope", Zone::Top, Box3::new([0.0, 0.58, -2.05], [0.16, 0.06, 0.08])).looks(Zone::Front),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([-0.6, 0.95, -1.43], [0.07, 0.04, 0.1])).on(0),
        Plate::hull("turret roof", Zone::Top, 8.0).on(0).glances(80.0),
        Plate::hull("turret", Zone::Side, 12.0).on(0).all_round().glances(70.0),
        Plate::hull("glacis", Zone::Top, 23.0).within(Box3::new([0.0, 0.4, -2.29], [1.43, 0.38, 0.45])).glances(74.0),
        Plate::hull("lower front", Zone::Front, 23.0).glances(66.0),
        Plate::hull("hull side", Zone::Side, 14.0).glances(74.0),
        Plate::hull("rear doors", Zone::Back, 10.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 8.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 8.0).glances(80.0),
    ],
    // The gearbox across the nose, the driver in the middle of the front with the gunner on their left,
    // the engine under the floor behind them, the rounds beside the gunner, fuel under the floor at the
    // back, and the troops on benches there.
    modules: &[
        Module::new(Part::TrackLeft, [-1.24, -0.4, 0.0], [0.2, 0.3, 2.675], 45),
        Module::new(Part::TrackRight, [1.24, -0.4, 0.0], [0.2, 0.3, 2.675], 45),
        Module::new(Part::Engine, [0.3, -0.35, -0.3], [0.6, 0.3, 0.7], 70),
        Module::new(Part::Transmission, [0.0, -0.2, -2.15], [0.85, 0.25, 0.2], 70),
        Module::new(Part::Fuel, [0.0, -0.5, 1.4], [0.9, 0.15, 0.5], 45),
        Module::new(Part::Ammo, [-0.75, 0.1, -0.4], [0.3, 0.3, 0.25], 50),
        Module::crew(0, [0.0, 0.1, -1.6], [0.33, 0.45, 0.4]),
        Module::new(Part::Breech, [-0.75, 0.95, -1.25], [0.1, 0.1, 0.3], 40).in_turret(),
        Module::crew(1, [-0.75, 0.75, -0.95], [0.38, 0.4, 0.38]).in_turret(),
        Module::crew(2, [-0.7, 0.1, 1.5], [0.4, 0.45, 0.9]),
        Module::crew(3, [0.7, 0.1, 1.5], [0.4, 0.45, 0.9]),
    ],
    canvas: false,
};

/// A 20 mm automatic cannon's solid round: through a car, another troop carrier's sides or a Strf 90's
/// side up close, and a tank only from behind or above.
const CANNON: Fire = Fire::Bullet { length: PBV_BARREL_LENGTH, damage: PBV_CANNON_DAMAGE, vehicle: PBV_CANNON_VEHICLE_DAMAGE, plane: PBV_CANNON_PLANE_DAMAGE, weapon: WEAPON_AUTOCANNON, range: PBV_CANNON_RANGE, pen: Pen::ball(28.0, 16.0, PBV_CANNON_RANGE, 20.0) };

pub(super) const PBV: VehicleDef = VehicleDef {
    kind: KIND_PBV,
    name: "Pbv 302",
    hp: 150,
    seats: PBV_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: TRACKS,
    spec: Spec {
        half_extents: PBV_HALF_EXTENTS,
        // The hull's box at 654 kg/m³ is 14 tonnes, about the Strf 90's weight, with a little less engine
        // and nearly its top speed.
        density: 654.0,
        wheel_radius: PBV_WHEEL_RADIUS,
        track_half: PBV_TRACK_HALF,
        axles: &PBV_AXLES_Z,
        hard_point_y: PBV_HARD_POINT_Y,
        suspension_rest: PBV_SUSPENSION_REST,
        max_suspension_force: 820_000.0,
        engine_force: 110_000.0,
        max_speed: PBV_MAX_SPEED,
        steer_lock: 0.0,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: PBV_TURN_RATE,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: Some(BOOST_FULL),
        buttons: Buttons::Boost,
        boost: Some(Boost::HEAVY),
        aims: false,
        solids: &[Solid { shape: Shape::Cylinder { half_height: PBV_TURRET_HALF_HEIGHT, radius: PBV_TURRET_RADIUS }, at: PBV_TURRET_PIVOT, role: Role::Turret }],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: Some(Swim { draft: PBV_DRAFT, speed: PBV_SWIM_SPEED, thrust: PBV_SWIM_THRUST, turn: PBV_SWIM_TURN }),
        bow: 0.0,
        profile: PROFILE,
        springs: Springs::FIRM,
        airframe: None,
        rotor: None,
        gait: None,
        arms: None,
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: Some(PBV_DOOR_EXIT),
    fittings: &[],
    doors: NO_DOORS,
};

/// The gunner's automatic cannon in the turret.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 1,
    aim: Aim::Turret { slew: PBV_TURRET_SLEW, pitch: (PBV_BARREL_MIN, PBV_BARREL_MAX), pitch_slew: PBV_BARREL_SLEW },
    muzzle: Muzzle::One(PBV_TURRET_PIVOT),
    fire: CANNON,
    trigger: BUTTON_FIRE,
    cooldown: PBV_CANNON_COOLDOWN_TICKS,
    spread: PBV_CANNON_SPREAD,
    recoil: 0.0,
    heat: PBV_CANNON_HEAT,
    ammo: None,
    needs: &[Need::Turret, Need::Breech],
    show: Show::Turret,
    eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
    sight: true,
    stand_in: Some(0),
}];
