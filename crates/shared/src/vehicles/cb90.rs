//! The Stridsbåt 90: everything the game knows about it.

use super::*;

/// The Stridsbåt 90 (`KIND_CB90`): the Swedish fast assault craft, true to size, 15.9 m long and 3.8 m
/// across, pushed by two waterjets to 40 knots. It floats only on open water, planes at speed, banks
/// into its turns and stops in a few lengths with its reversing buckets. The driver (seat 0) sits in the
/// low cockpit forward and works the twin Ksp 58 machine guns fixed on its front, which only elevate:
/// the driver aims them sideways by turning the boat, as a Strv S's. Behind the cockpit the gunner
/// (seat 1) stands in an open ring on the troop compartment's roof, out in the open, and turns the 25 mm
/// automatic cannon all the way round. The troop compartment is down in the hull, its roof a little over
/// the gunwale, and behind it two riders (seats 2 and 3) sit on the open after deck, firing their own
/// guns. Its bow tapers to a point over `CB90_BOW`.
pub const CB90_SEATS: usize = 4;
/// Half the hull's box, metres: 3.8 m across, 2 m from the keel to the deck and 15.9 m long.
pub const CB90_HALF_EXTENTS: [f32; 3] = [1.9, 1.0, 7.95];
/// How far back from the stem the sides taper out to the full beam, metres.
pub const CB90_BOW: f32 = 4.5;
/// How deep the hull floats at rest, metres: the deck stands this much less than its height out of the
/// water.
pub const CB90_DRAFT: f32 = 0.8;
/// Top speed ahead on calm water, m/s (45 knots).
pub const CB90_MAX_SPEED: f32 = 23.0;
/// How hard the waterjets push it ahead from a standstill, m/s²: up to 30 knots in under five seconds.
pub const CB90_THRUST: f32 = 3.6;
/// How hard the reversing buckets stop it while it still runs ahead, m/s²: from full speed inside two
/// lengths, as the real boat's crash stop does.
pub const CB90_BRAKE: f32 = 10.0;
/// In a crash stop its bow digs in, this far down, radians, and its hull squats this much deeper into
/// the water, metres, both most from `CB90_PLANE_SPEED`'s top and gone by the time it has stopped.
pub const CB90_BRAKE_DIP: f32 = 0.13;
pub const CB90_BRAKE_SQUAT: f32 = 0.35;
/// Top speed astern, m/s.
pub const CB90_ASTERN_SPEED: f32 = 4.0;
/// Yaw rate at full helm under way, rad/s: a turn about 100 m across at full speed.
pub const CB90_TURN_RATE: f32 = 0.45;
/// Yaw rate at full helm standing still, rad/s: the jets swing it round on the spot.
pub const CB90_SPIN_RATE: f32 = 0.3;
/// How far its middle rises out of the water once it planes, metres, and the speeds over which it
/// climbs onto the plane, m/s.
pub const CB90_PLANE_LIFT: f32 = 0.35;
pub const CB90_PLANE_SPEED: [f32; 2] = [6.0, 14.0];
/// How far its bow lifts as it climbs onto the plane (most at the bottom of `CB90_PLANE_SPEED`) and how
/// far it stays lifted planing, radians.
pub const CB90_BOW_RISE: f32 = 0.07;
pub const CB90_BOW_PLANING: f32 = 0.025;
/// How far it banks into a turn, radians per m/s² of turning, and at most, radians.
pub const CB90_BANK: f32 = 0.022;
pub const CB90_BANK_MAX: f32 = 0.2;
/// The deck, in the hull frame.
pub const CB90_DECK_Y: f32 = CB90_HALF_EXTENTS[1];
/// The cockpit, low and forward, where the driver sits: half its box, metres, and where its middle is.
/// Its roof is 1.15 m over the deck.
pub const CB90_COCKPIT_HALF: [f32; 3] = [1.4, 0.575, 1.45];
pub const CB90_COCKPIT_AT: [f32; 3] = [0.0, 1.575, -3.15];
/// The troop compartment's roof behind it, 0.35 m over the deck, down to the after deck: half its box,
/// metres, and where its middle is.
pub const CB90_TROOP_HALF: [f32; 3] = [1.6, 0.175, 2.15];
pub const CB90_TROOP_AT: [f32; 3] = [0.0, 1.175, 0.45];
/// The gunner's ring on that roof, just behind the cockpit: where its middle is, its radius and half its
/// height, metres.
pub const CB90_RING_AT: [f32; 3] = [0.0, 1.55, -0.9];
pub const CB90_RING_RADIUS: f32 = 0.75;
pub const CB90_RING_HALF_HEIGHT: f32 = 0.2;
/// The cannon on its pedestal in the ring: where it turns and elevates, high enough to fire over the
/// cockpit and its machine guns.
pub const CB90_CANNON_PIVOT: [f32; 3] = [0.0, 2.75, -0.9];
/// The gunner's head from the pivot, with the gun pointing ahead: standing in the ring's hatch behind the
/// gun, on the troop compartment's floor.
pub const CB90_GUNNER_HEAD: [f32; 3] = [0.0, -0.55, 0.7];
/// The cannon: how far its muzzle is from the pivot, metres, its elevation, radians (+ is up), which
/// keeps its rounds over the cockpit, and how fast it follows the gunner's look, rad/s.
pub const CB90_CANNON_LENGTH: f32 = 2.3;
pub const CB90_CANNON_PITCH_MIN: f32 = -0.08;
pub const CB90_CANNON_PITCH_MAX: f32 = 0.9;
pub const CB90_CANNON_SLEW: f32 = 1.8;
/// Ticks between its rounds: seven and a half a second.
pub const CB90_CANNON_COOLDOWN_TICKS: u32 = 8;
pub const CB90_CANNON_SPREAD: f32 = 0.005;
pub const CB90_CANNON_RANGE: f32 = 500.0;
/// What a 25 mm round takes off a soldier, a vehicle's hull and an aircraft.
pub const CB90_CANNON_DAMAGE: u8 = 50;
pub const CB90_CANNON_VEHICLE_DAMAGE: u8 = 12;
pub const CB90_CANNON_PLANE_DAMAGE: u8 = 24;
/// Heat a round puts in the barrel, of 255: about five seconds of holding the trigger jams it.
pub const CB90_CANNON_HEAT: f32 = 15.0;
/// The twin machine guns fixed on the front of the cockpit's roof, firing ahead over the bow: where they
/// elevate, how far either side of it each barrel is, metres, how far they tilt down and up, radians
/// (+ is up), and how fast, rad/s. They do not turn: the boat aims them.
pub const CB90_MG_PIVOT: [f32; 3] = [0.0, 2.4, -3.9];
pub const CB90_MG_BARREL_X: f32 = 0.16;
pub const CB90_MG_LENGTH: f32 = 1.1;
pub const CB90_MG_PITCH_MIN: f32 = -0.15;
pub const CB90_MG_PITCH_MAX: f32 = 0.2;
pub const CB90_MG_SLEW: f32 = 0.8;
/// Ticks between rounds, the two barrels taking turns: twenty a second.
pub const CB90_MG_COOLDOWN_TICKS: u32 = 3;
pub const CB90_MG_SPREAD: f32 = 0.01;
pub const CB90_MG_RANGE: f32 = 350.0;
/// What a 7.62 mm round takes off a soldier, a vehicle's hull and an aircraft.
pub const CB90_MG_DAMAGE: u8 = 14;
pub const CB90_MG_VEHICLE_DAMAGE: u8 = 3;
pub const CB90_MG_PLANE_DAMAGE: u8 = 6;
/// Heat a round puts in the barrels, of 255: about five seconds of holding the trigger jams them.
pub const CB90_MG_HEAT: f32 = 5.5;
/// Where the driver's reticle camera sits behind and above the machine guns, metres: over the after
/// deck, looking down the boat over the gunner's head.
pub const CB90_AIM_BACK: f32 = 7.0;
pub const CB90_AIM_UP: f32 = 2.3;
/// The driver in the cockpit, the gunner as they stand with the cannon ahead (`CB90_CANNON_PIVOT` plus
/// `CB90_GUNNER_HEAD`), then the two riders on the after deck.
pub const CB90_SEAT_HEADS: [[f32; 3]; 4] = [[-0.5, 1.95, -2.9], [0.0, 2.2, -0.2], [-0.9, 2.3, 4.6], [0.9, 2.3, 4.6]];
/// Where it is boarded from, on the after deck, and where whoever leaves it stands (the middle of a
/// standing soldier), a little further aft.
pub const CB90_BOARD: [f32; 3] = [0.0, 1.0, 5.6];
pub const CB90_EXIT: [f32; 3] = [0.0, CB90_DECK_Y + STAND_CENTRE + 0.05, 6.5];

/// Welded aluminium: proof against splinters, not against a rifle up close; the cockpit and the ring a
/// little thicker. The two diesels are under the after deck with the fuel ahead of them, and the cannon's
/// rounds in the hull under the ring.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::hull("gun ring", Zone::Side, 8.0).on(0).all_round().glances(70.0),
        Plate::hull("cockpit", Zone::Side, 8.0).on(1).all_round().glances(75.0),
        Plate::hull("troop compartment", Zone::Side, 6.0).on(2).all_round().glances(78.0),
        Plate::hull("aluminium hull", Zone::Front, 5.0).all_round().glances(80.0),
    ],
    modules: &[
        Module::new(Part::Engine, [0.0, -0.1, 6.0], [1.2, 0.5, 1.3], 90),
        Module::new(Part::Fuel, [0.0, -0.5, 4.2], [1.0, 0.3, 0.6], 60),
        Module::new(Part::Ammo, [0.0, 0.3, -0.9], [0.4, 0.3, 0.4], 50),
        Module::new(Part::Breech, [0.0, 2.75, -1.2], [0.12, 0.12, 0.4], 50),
    ],
    canvas: false,
};

/// A 25 mm automatic cannon's round: through a car, a troop carrier's sides or another boat, and a
/// tank only from behind or above.
const CANNON: Fire = Fire::Bullet {
    length: CB90_CANNON_LENGTH,
    damage: CB90_CANNON_DAMAGE,
    vehicle: CB90_CANNON_VEHICLE_DAMAGE,
    plane: CB90_CANNON_PLANE_DAMAGE,
    weapon: WEAPON_AUTOCANNON,
    range: CB90_CANNON_RANGE,
    pen: Pen::ball(35.0, 20.0, CB90_CANNON_RANGE, 25.0),
};

/// A Ksp 58's 7.62 mm round: through a car, not through armour.
const MG: Fire = Fire::Bullet {
    length: CB90_MG_LENGTH,
    damage: CB90_MG_DAMAGE,
    vehicle: CB90_MG_VEHICLE_DAMAGE,
    plane: CB90_MG_PLANE_DAMAGE,
    weapon: WEAPON_MG,
    range: CB90_MG_RANGE,
    pen: Pen::ball(10.0, 6.0, CB90_MG_RANGE, 7.62),
};

/// Its planing hull on two waterjets, with their reversing buckets.
const CB90_HULL: Boat = Boat {
    draft: CB90_DRAFT,
    thrust: CB90_THRUST,
    brake: CB90_BRAKE,
    brake_dip: CB90_BRAKE_DIP,
    brake_squat: CB90_BRAKE_SQUAT,
    astern_speed: CB90_ASTERN_SPEED,
    spin_rate: CB90_SPIN_RATE,
    plane_lift: CB90_PLANE_LIFT,
    plane_speed: CB90_PLANE_SPEED,
    bow_rise: CB90_BOW_RISE,
    bow_planing: CB90_BOW_PLANING,
    bank: CB90_BANK,
    bank_max: CB90_BANK_MAX,
    // A deep vee: it holds its line, and drives on through whatever the water does.
    heave: 3.0,
    coast: 0.8,
    level: 10.0,
    rock: 3.0,
    keel: 2.5,
    grip: 0.0,
    helm: 3.0,
    thrust_steer: 0.0,
    lever: 0.3,
    scrape: 0.0,
};

/// It heaves into a tree on the shore rather than stopping on it, and starts floating at its draft.
const CB90_BODY: Body = Body { drop_y: CB90_HALF_EXTENTS[1] - CB90_DRAFT, heavy: true, ram: 2.0, crash: 0.3, ..CAR };

pub(super) const CB90: VehicleDef = VehicleDef {
    kind: KIND_CB90,
    name: "Stridsbåt 90",
    hp: 200,
    seats: CB90_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Glazed,
    seat_heads: &CB90_SEAT_HEADS,
    crew_guns: true,
    body: CB90_BODY,
    spec: Spec {
        half_extents: CB90_HALF_EXTENTS,
        // About 120 m³ of hull box: some 16 tonnes, its weight empty.
        density: 130.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: CB90_MAX_SPEED,
        steer_lock: 0.0,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: CB90_TURN_RATE,
        lift: Lift::Boat(&CB90_HULL),
        linear_damping: 0.0,
        // Its engines low in the hull: it rolls back upright.
        com_drop: 0.4,
        tank: Some(BOOST_FULL),
        buttons: Buttons::Boost,
        boost: Some(Boost::BOAT),
        aims: false,
        solids: &[
            Solid { shape: Shape::Cylinder { half_height: CB90_RING_HALF_HEIGHT, radius: CB90_RING_RADIUS }, at: CB90_RING_AT, role: Role::Part },
            Solid { shape: Shape::Cuboid(CB90_COCKPIT_HALF), at: CB90_COCKPIT_AT, role: Role::Part },
            Solid { shape: Shape::Cuboid(CB90_TROOP_HALF), at: CB90_TROOP_AT, role: Role::Part },
        ],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: None,
        bow: CB90_BOW,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        rotor: None,
        gait: None,
        arms: None,
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: Some(MountSeat { seat: 1, pivot: CB90_CANNON_PIVOT, head: CB90_GUNNER_HEAD }),
    board: Some(CB90_BOARD),
    exit: Some(CB90_EXIT),
    fittings: &[],
};

/// The gunner's cannon in the ring, all the way round, and the driver's twin machine guns fixed on the
/// cockpit, which only elevate: the driver aims them sideways by turning the boat.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: None, pitch: (CB90_CANNON_PITCH_MIN, CB90_CANNON_PITCH_MAX), slew: CB90_CANNON_SLEW },
        muzzle: Muzzle::One(CB90_CANNON_PIVOT),
        fire: CANNON,
        trigger: BUTTON_FIRE,
        cooldown: CB90_CANNON_COOLDOWN_TICKS,
        spread: CB90_CANNON_SPREAD,
        recoil: 0.0,
        heat: CB90_CANNON_HEAT,
        ammo: None,
        needs: &[Need::Breech],
        show: Show::Turret,
        eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
        sight: false,
    },
    MountDef {
        seat: 0,
        aim: Aim::Bore { pitch: (CB90_MG_PITCH_MIN, CB90_MG_PITCH_MAX), slew: CB90_MG_SLEW },
        muzzle: Muzzle::Twin { pivot: CB90_MG_PIVOT, x: CB90_MG_BARREL_X, drop: 0.0 },
        fire: MG,
        trigger: BUTTON_FIRE,
        cooldown: CB90_MG_COOLDOWN_TICKS,
        spread: CB90_MG_SPREAD,
        recoil: 0.0,
        heat: CB90_MG_HEAT,
        ammo: None,
        needs: &[],
        show: Show::Pintle,
        eye: Some(Eye { back: CB90_AIM_BACK, up: CB90_AIM_UP }),
        sight: false,
    },
];
