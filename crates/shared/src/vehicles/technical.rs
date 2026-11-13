//! The technical: everything the game knows about it.

use super::*;

/// The technical (`KIND_TECHNICAL`): a long-bonnet pickup with a heavy machine gun on a post in its open
/// bed. A driver and a passenger sit in the cab (seats 0 and 1, the passenger firing their own guns) and
/// the gunner (seat 2) stands in the bed behind the gun, turning with it all the way round. It is tall,
/// on big tyres and long soft springs: it wallows through corners but rides over rough ground.
pub const TECHNICAL_SEATS: usize = 3;
pub const TECHNICAL_HALF_EXTENTS: [f32; 3] = [0.95, 0.4, 2.45];
pub const TECHNICAL_WHEEL_RADIUS: f32 = 0.58;
pub const TECHNICAL_TRACK_HALF: f32 = 0.85;
pub const TECHNICAL_AXLE_Z: f32 = 1.6;
pub const TECHNICAL_HARD_POINT_Y: f32 = -0.1;
pub const TECHNICAL_SUSPENSION_REST: f32 = 0.6;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const TECHNICAL_RIDE_HEIGHT: f32 = 1.08;
pub const TECHNICAL_MAX_STEER: f32 = 0.5;
/// About 108 km/h, a little slower than the jeep.
pub const TECHNICAL_MAX_SPEED: f32 = 30.0;
/// The seat that works the machine gun.
pub const TECHNICAL_MG_SEAT: usize = 2;
/// Where the gun turns on top of its post, over the bed and above the cab's roof, so it fires over it.
pub const TECHNICAL_MG_PIVOT: [f32; 3] = [0.0, 1.45, 0.9];
/// The gunner's head from the pivot, with the gun pointing ahead: standing in the bed behind it, their
/// feet on its floor (0.15 m over the hull's middle) and their head 1.64 m above them.
pub const TECHNICAL_MG_HEAD: [f32; 3] = [0.0, 0.34, 0.75];
pub const TECHNICAL_MG_LENGTH: f32 = 1.2;
/// Its elevation, radians (+ is up): steep enough for aircraft. It turns all the way round.
pub const TECHNICAL_MG_PITCH_MIN: f32 = -0.3;
pub const TECHNICAL_MG_PITCH_MAX: f32 = 0.9;
pub const TECHNICAL_MG_SLEW: f32 = 2.5;
/// A heavy, slow gun: four rounds a second, each one hitting hard. It deals what the tank's roof gun does
/// in a second, but in fewer, bigger blows: four put a soldier down.
pub const TECHNICAL_MG_COOLDOWN_TICKS: u32 = 15;
pub const TECHNICAL_MG_SPREAD: f32 = 0.014;
pub const TECHNICAL_MG_RANGE: f32 = 300.0;
pub const TECHNICAL_MG_DAMAGE: u8 = 40;
pub const TECHNICAL_MG_PLANE_DAMAGE: u8 = 20;
pub const TECHNICAL_MG_VEHICLE_DAMAGE: u8 = 10;
/// Heat a round puts in the barrel: about four seconds of holding the trigger jams it, a little sooner
/// than the tank's roof gun, and about three more before it has cooled enough to clear.
pub const TECHNICAL_MG_HEAT: f32 = 31.0;
/// The driver on the left and the passenger in the cab, then the gunner as they stand with the gun ahead
/// (`TECHNICAL_MG_PIVOT` plus `TECHNICAL_MG_HEAD`).
pub const TECHNICAL_SEAT_HEADS: [[f32; 3]; 3] = [[-0.42, 1.0, -0.35], [0.42, 1.0, -0.35], [0.0, 1.79, 1.65]];

/// The engine under the bonnet, the fuel tank under the bed and the gun's receiver; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.2, -1.75], [0.45, 0.3, 0.5], 130),
        Module::new(Part::Fuel, [0.0, -0.25, 1.2], [0.45, 0.15, 0.35], 100),
        Module::new(Part::Breech, TECHNICAL_MG_PIVOT, [0.1, 0.1, 0.25], 60),
    ],
    canvas: false,
};

const MG: Fire = Fire::Bullet {
    length: TECHNICAL_MG_LENGTH,
    damage: TECHNICAL_MG_DAMAGE,
    vehicle: TECHNICAL_MG_VEHICLE_DAMAGE,
    plane: TECHNICAL_MG_PLANE_DAMAGE,
    weapon: WEAPON_MG,
    range: TECHNICAL_MG_RANGE,
    // A 14.5 mm round: through more plate than the tank's 12.7 mm.
    pen: Pen::ball(30.0, 18.0, TECHNICAL_MG_RANGE, 14.5),
};

pub(super) const TECHNICAL: VehicleDef = VehicleDef {
    kind: KIND_TECHNICAL,
    name: "Technical",
    hp: 130,
    seats: TECHNICAL_SEATS,
    mounts: MOUNTS,
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &TECHNICAL_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: TECHNICAL_HALF_EXTENTS,
        // 7.4 m³ of box is about two tonnes.
        density: 270.0,
        wheel_radius: TECHNICAL_WHEEL_RADIUS,
        track_half: TECHNICAL_TRACK_HALF,
        axles: &[-TECHNICAL_AXLE_Z, TECHNICAL_AXLE_Z],
        hard_point_y: TECHNICAL_HARD_POINT_Y,
        suspension_rest: TECHNICAL_SUSPENSION_REST,
        max_suspension_force: 90_000.0,
        engine_force: 13_000.0,
        max_speed: TECHNICAL_MAX_SPEED,
        steer_lock: TECHNICAL_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Its weight carried low on the frame, under the tall body: otherwise, flat out at full lock, the soft
        // springs let it lean until it goes over.
        com_drop: 0.5,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Too heavy to skip over water: it sinks and wades along the bed.
        skim: None,
        bow: 0.0,
        // Half as stiff as a car's and with a long travel: it sags, pitches and rolls, and soaks up the bumps.
        springs: Springs { stiffness: 12.0, compression: 0.9, damping: 1.6, travel: 0.5 },
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: Some(MountSeat { seat: TECHNICAL_MG_SEAT, pivot: TECHNICAL_MG_PIVOT, head: TECHNICAL_MG_HEAD }),
    board: None,
    exit: None,
    fittings: &[],
};

/// The machine gun on its post, turned all the way round by the gunner's look.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: TECHNICAL_MG_SEAT,
    aim: Aim::Mount { yaw_limit: None, pitch: (TECHNICAL_MG_PITCH_MIN, TECHNICAL_MG_PITCH_MAX), slew: TECHNICAL_MG_SLEW },
    muzzle: Muzzle::One(TECHNICAL_MG_PIVOT),
    fire: MG,
    trigger: BUTTON_FIRE,
    cooldown: TECHNICAL_MG_COOLDOWN_TICKS,
    spread: TECHNICAL_MG_SPREAD,
    recoil: 0.0,
    heat: TECHNICAL_MG_HEAT,
    ammo: None,
    needs: &[Need::Breech],
    show: Show::Turret,
    eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
    sight: false,
}];
