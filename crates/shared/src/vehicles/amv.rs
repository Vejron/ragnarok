//! The AMV XA-361 mortar carrier: everything the game knows about it.

use super::*;

/// The AMV XA-361 (`KIND_AMV`): an eight-wheeled armoured vehicle with a turret on its back carrying
/// two 120 mm mortar tubes side by side, which lob their bombs as the field mortar does: the range dial
/// sets how far, and the tubes rise to the high lob that lands them there. They are shorter-ranged than
/// the field mortar's (200 m against 300) and burst a little smaller, but fire two in quick succession,
/// after which the turret takes three seconds to load both tubes again. The driver (seat 0) sits at the
/// front left beside the engine, the mortar operator (seat 1) under the turret, and one rider (seat 2)
/// in the back, all sealed in; they get out by the rear door. It floats, and two water jets at the
/// back push it slowly across open water. Same body axes as a car, and true to size:
/// 7.7 m long, 2.8 m wide and 2.3 m to the hull roof, 3 m to the turret's top.
pub const AMV_SEATS: usize = 3;
/// Half the hull's box, metres: 7.7 m long, 2.8 m wide and 1.85 m deep, its belly about 0.45 m off the
/// ground.
pub const AMV_HALF_EXTENTS: [f32; 3] = [1.4, 0.925, 3.85];
/// Wheel radius, metres: tall run-flat tyres, 1.2 m across.
pub const AMV_WHEEL_RADIUS: f32 = 0.6;
/// How far each wheel is from the middle, metres.
pub const AMV_TRACK_HALF: f32 = 1.12;
/// Z of the four axles, front first, metres: two pairs, 1.5 m apart within a pair.
pub const AMV_AXLES_Z: [f32; 4] = [-2.35, -0.85, 0.85, 2.35];
pub const AMV_HARD_POINT_Y: f32 = -0.3;
pub const AMV_SUSPENSION_REST: f32 = 0.5;
/// Where the hull's centre settles over flat ground, metres: measured by `ride_heights_differ_between_cars`.
pub const AMV_RIDE_HEIGHT: f32 = 1.35;
/// How far the front axle's wheels turn at a standstill, radians.
pub const AMV_MAX_STEER: f32 = 0.5;
/// How many axles from the front steer: the front pair, the second turning less than the first.
pub const AMV_STEER_AXLES: usize = 2;
/// Top speed on the flat, m/s (90 km/h).
pub const AMV_MAX_SPEED: f32 = 25.0;
/// How deep its hull floats, metres: 24 tonnes on its 2.8 × 7.7 m footprint, so the roof is 0.65 m clear
/// of the water and the turret's top 1.35 m.
pub const AMV_DRAFT: f32 = 1.2;
/// Top speed afloat, m/s (10 km/h), how hard its two water jets push it there, m/s², and its yaw rate at
/// full stick, rad/s: a long hull, slower to swing round than the Pbv 302's.
pub const AMV_SWIM_SPEED: f32 = 2.8;
pub const AMV_SWIM_THRUST: f32 = 2.0;
pub const AMV_SWIM_TURN: f32 = 0.35;
/// The turret's solid: a low drum on the hull roof behind the middle, 2.2 m across, its top 3 m off the
/// ground, metres. It is part of the hull: it does not come off.
pub const AMV_TURRET_RING: [f32; 3] = [0.0, 1.275, 0.9];
pub const AMV_TURRET_RADIUS: f32 = 1.1;
pub const AMV_TURRET_HALF_HEIGHT: f32 = 0.35;
/// Where the tubes turn and elevate, in the hull frame: on the turret's trunnions, a little ahead of
/// the ring's middle.
pub const AMV_GUN_PIVOT: [f32; 3] = [0.0, 1.45, 0.7];
/// How far apart the two tubes are either side of the middle, metres, and how long they are.
pub const AMV_TUBE_X: f32 = 0.3;
pub const AMV_BARREL_LENGTH: f32 = 3.0;
/// The range dial, metres: the longest lob is 200 m (`GRAVITY` and the muzzle speed at 45 degrees),
/// and the steepest the tubes stand reaches about 35 m, so the dial stops at 40.
pub const AMV_RANGE_MIN: f32 = 40.0;
pub const AMV_RANGE_MAX: f32 = 200.0;
pub const AMV_RANGE_START: f32 = 100.0;
pub const AMV_RANGE_RATE: f32 = 50.0;
/// Muzzle speed of its bomb, m/s: under `GRAVITY` it reaches 200 m at 45 degrees.
pub const AMV_SHELL_SPEED: f32 = 63.25;
/// How fast the turret follows the operator's look, rad/s, and the tubes the range, rad/s: powered,
/// so far quicker than the field mortar's crank.
pub const AMV_TRAVERSE_SLEW: f32 = 1.0;
pub const AMV_ELEVATION_SLEW: f32 = 0.6;
/// Ticks between the two rounds of a pair: a third of a second.
pub const AMV_COOLDOWN_TICKS: u32 = 20;
/// Rounds in the tubes when loaded: one in each.
pub const AMV_ROUNDS: u8 = 2;
/// Ticks after the last round before both tubes are loaded again: three seconds.
pub const AMV_RELOAD_TICKS: u32 = 3 * TICK_RATE;
/// Speed the hull is shoved back at when a tube fires, m/s: hardly anything, on 24 tonnes.
pub const AMV_RECOIL: f32 = 0.3;
/// Blast of its bomb: a little smaller and weaker than the field mortar's (`MORTAR_RADIUS`,
/// `MORTAR_DAMAGE`, `MORTAR_KNOCK`), and the crater it digs, metres.
pub const AMV_BOMB_RADIUS: f32 = 9.5;
pub const AMV_BOMB_DAMAGE: f32 = 210.0;
pub const AMV_BOMB_KNOCK: f32 = 25.0;
pub const AMV_BOMB_CRATER: [f32; 2] = [7.5, 1.3];
/// Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
/// rear door.
pub const AMV_DOOR_EXIT: [f32; 3] = [0.0, -0.4, 4.6];

/// Welded steel proof against heavy machine guns at the front and rifles all round, a belly built
/// against mines, and a thin turret. The driver looks out through a periscope at the front left.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's periscope", Zone::Front, Box3::new([-0.6, 0.75, -3.85], [0.2, 0.06, 0.08])),
        Plate::hull("turret roof", Zone::Top, 8.0).on(0).glances(80.0),
        Plate::hull("turret", Zone::Side, 14.0).on(0).all_round().glances(72.0),
        Plate::hull("glacis", Zone::Front, 30.0).glances(70.0),
        Plate::hull("hull side", Zone::Side, 18.0).glances(75.0),
        Plate::hull("rear door", Zone::Back, 14.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 10.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 16.0).glances(80.0),
    ],
    // The engine at the front right beside the driver, the gearbox behind it, the operator under the
    // turret with the breeches over them, the bombs racked behind, the fuel under the floor at the back
    // and the rider on a seat there.
    modules: &[
        Module::new(Part::Engine, [0.6, 0.0, -2.9], [0.6, 0.5, 0.75], 80),
        Module::new(Part::Transmission, [0.3, -0.5, -1.8], [0.7, 0.25, 0.3], 70),
        Module::new(Part::Fuel, [0.0, -0.6, 2.9], [0.9, 0.2, 0.5], 50),
        Module::new(Part::Ammo, [0.0, -0.1, 2.0], [0.9, 0.4, 0.35], 50),
        Module::new(Part::Breech, [0.0, 0.95, 1.3], [0.5, 0.15, 0.3], 50),
        Module::crew(0, [-0.6, 0.2, -2.7], [0.35, 0.45, 0.45]),
        Module::crew(1, [0.0, 0.2, 0.7], [0.4, 0.45, 0.45]),
        Module::crew(2, [0.0, 0.1, 3.0], [0.4, 0.45, 0.45]),
    ],
    canvas: false,
};

pub(super) const AMV: VehicleDef = VehicleDef {
    kind: KIND_AMV,
    name: "AMV XA-361",
    hp: 180,
    seats: AMV_SEATS,
    mounts: MOUNTS,
    // Run-flat tyres behind armoured flanks: shots and crashes do not take its wheels off.
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: Body { drop_y: 1.6, ram: 2.5, crash: 0.3, ..CAR },
    spec: Spec {
        half_extents: AMV_HALF_EXTENTS,
        // The hull's 40 m³ of box at 600 kg/m³ is 24 tonnes.
        density: 600.0,
        wheel_radius: AMV_WHEEL_RADIUS,
        track_half: AMV_TRACK_HALF,
        axles: &AMV_AXLES_Z,
        hard_point_y: AMV_HARD_POINT_Y,
        suspension_rest: AMV_SUSPENSION_REST,
        // A tonne and a half on each of eight wheels, with room for landing off a crest.
        max_suspension_force: 400_000.0,
        // About 5 m/s² from a standstill: a big diesel, slower off the line than a car.
        engine_force: 120_000.0,
        max_speed: AMV_MAX_SPEED,
        steer_lock: AMV_MAX_STEER,
        steer_axles: AMV_STEER_AXLES,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Its weight is low in the hull, under the turret: otherwise it leans over at full lock.
        com_drop: 0.6,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid { shape: Shape::Cylinder { half_height: AMV_TURRET_HALF_HEIGHT, radius: AMV_TURRET_RADIUS }, at: AMV_TURRET_RING, role: Role::Part }],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: Some(Swim { draft: AMV_DRAFT, speed: AMV_SWIM_SPEED, thrust: AMV_SWIM_THRUST, turn: AMV_SWIM_TURN }),
        bow: 0.0,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        gait: None,
        arms: None,
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: Some(AMV_DOOR_EXIT),
    fittings: &[],
};

/// The dial of its tubes, out to the longest lob of its bomb.
const DIAL: Dial = Dial { speed: AMV_SHELL_SPEED, min: AMV_RANGE_MIN, max: AMV_RANGE_MAX, start: AMV_RANGE_START, rate: AMV_RANGE_RATE };

/// The two tubes, which take turns: a pair of rounds, then both are loaded again.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 1,
    aim: Aim::Lob { slew: AMV_TRAVERSE_SLEW, pitch_slew: AMV_ELEVATION_SLEW, dial: DIAL },
    muzzle: Muzzle::Twin { pivot: AMV_GUN_PIVOT, x: AMV_TUBE_X, drop: 0.0 },
    fire: Fire::Shell { kind: PROJECTILE_AMV_BOMB, alt: PROJECTILE_AMV_BOMB, speed: AMV_SHELL_SPEED, length: AMV_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: AMV_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: AMV_RECOIL,
    heat: 0.0,
    ammo: Some(Ammo { full: AMV_ROUNDS, refill: Refill::Reload(AMV_RELOAD_TICKS) }),
    needs: &[Need::Breech],
    show: Show::Turret,
    eye: None,
    sight: false,
}];
