//! The pirate ship: everything the game knows about it.

use super::*;

/// The pirate ship (`KIND_SHIP`): a single-masted brig of about 10 m, sailed from the helm at the
/// stern, with three guns run out on each broadside (`SHIP_GUN_SLOTS`, each its own `KIND_SHIP_GUN`).
/// It floats and sails only on open water, driven by the wind in its sail (`crate::wind`), and is
/// stranded wherever it runs aground. The hull's top is its deck, walked on between the guns; the
/// bulwarks along its sides keep whoever walks it aboard. Its bow tapers to a point over `SHIP_BOW`.
pub const SHIP_SEATS: usize = 1;
pub const SHIP_HALF_EXTENTS: [f32; 3] = [1.8, 0.7, 5.0];
/// How far back from the stem the sides taper out to the full beam, metres.
pub const SHIP_BOW: f32 = 2.2;
/// How deep the hull floats, metres: the deck stands this much less than its height out of the water.
pub const SHIP_DRAFT: f32 = 0.6;
/// Top speed on a beam reach in a full wind, m/s (about 20 knots).
pub const SHIP_MAX_SPEED: f32 = 10.0;
/// How quickly it turns under way, at full rudder, rad/s.
pub const SHIP_TURN_RATE: f32 = 0.6;
/// The share of a full sail's drive her motor gives whatever the wind, hidden below decks: into the
/// wind, where the sail only flaps, it still pushes her along slowly.
pub const SHIP_MOTOR: f32 = 0.35;
/// The deck, in the hull frame.
pub const SHIP_DECK_Y: f32 = SHIP_HALF_EXTENTS[1];
/// The bulwarks: waist high over the deck, too high to climb over, with the guns firing through ports
/// in them; and as thick as planks.
pub const SHIP_BULWARK_HEIGHT: f32 = 0.75;
pub const SHIP_BULWARK_THICK: f32 = 0.1;
/// The mast, from the deck: how tall and how thick, and how far forward of the middle it stands.
pub const SHIP_MAST_HEIGHT: f32 = 8.0;
pub const SHIP_MAST_RADIUS: f32 = 0.15;
pub const SHIP_MAST_Z: f32 = -0.7;
/// The helm, where its helmsman stands and from where it is boarded, near the stern.
pub const SHIP_HELM: [f32; 3] = [0.0, SHIP_DECK_Y + 0.9, 4.0];
pub const SHIP_HELM_HEAD: [f32; 3] = [0.0, SHIP_DECK_Y + 1.6, 4.3];
/// Where the helmsman steps down onto the deck, the middle of a soldier standing just ahead of the helm.
pub const SHIP_HELM_EXIT: [f32; 3] = [0.0, SHIP_DECK_Y + STAND_CENTRE + 0.05, 3.3];
/// How far out from the centre line the guns stand, and how far along it each pair stands, metres.
pub const SHIP_GUN_X: f32 = 1.15;
pub const SHIP_GUN_Z: [f32; 3] = [-1.8, 0.4, 2.6];
/// Where each of the six guns stands on deck (its carriage's middle, in the hull frame) and which way
/// it faces, as a yaw: run out over the side, starboard (+X) guns to the right, port ones to the left.
pub const SHIP_GUN_SLOTS: [([f32; 3], f32); 6] = {
    let y = SHIP_DECK_Y + super::ship_gun::SHIP_GUN_HALF_EXTENTS[1];
    let (x, right, left) = (SHIP_GUN_X, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    let z = SHIP_GUN_Z;
    [([x, y, z[0]], right), ([x, y, z[1]], right), ([x, y, z[2]], right), ([-x, y, z[0]], left), ([-x, y, z[1]], left), ([-x, y, z[2]], left)]
};

/// Planks: a rifle bullet goes in, a shell goes through. The powder room is in the middle of the hold,
/// and a shot that finds it sends the whole ship up.
const ARMOR: Armor = Armor {
    plates: &[Plate::hull("planks", Zone::Front, 5.0).all_round().glances(85.0)],
    modules: &[Module::new(Part::Ammo, [0.0, -0.25, 0.4], [0.5, 0.25, 0.7], 50)],
    canvas: false,
};

const BULWARK_LENGTH: f32 = 2.0 * SHIP_HALF_EXTENTS[2] - SHIP_BOW;
const BULWARK_Y: f32 = SHIP_DECK_Y + SHIP_BULWARK_HEIGHT * 0.5;

/// A ship heaves into a tree on the shore rather than stopping on it.
const SHIP_BODY: Body = Body { drop_y: SHIP_HALF_EXTENTS[1] - SHIP_DRAFT, heavy: true, ram: 3.0, crash: 0.3, ..CAR };

pub(super) const SHIP: VehicleDef = VehicleDef {
    kind: KIND_SHIP,
    name: "Pirate ship",
    hp: 250,
    seats: SHIP_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &[SHIP_HELM_HEAD],
    crew_guns: false,
    body: SHIP_BODY,
    spec: Spec {
        half_extents: SHIP_HALF_EXTENTS,
        // About 45 m³ of hull, some 3 t: as heavy as a loaded truck.
        density: 60.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: SHIP_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: SHIP_TURN_RATE,
        lift: Lift::Sail,
        linear_damping: 0.0,
        // Ballast in the bilge: it rolls back upright from a broadside.
        com_drop: 0.35,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[
            Solid { shape: Shape::Cuboid([SHIP_BULWARK_THICK, SHIP_BULWARK_HEIGHT * 0.5, BULWARK_LENGTH * 0.5]), at: [SHIP_HALF_EXTENTS[0] - SHIP_BULWARK_THICK, BULWARK_Y, SHIP_BOW * 0.5], role: Role::Part },
            Solid { shape: Shape::Cuboid([SHIP_BULWARK_THICK, SHIP_BULWARK_HEIGHT * 0.5, BULWARK_LENGTH * 0.5]), at: [SHIP_BULWARK_THICK - SHIP_HALF_EXTENTS[0], BULWARK_Y, SHIP_BOW * 0.5], role: Role::Part },
            Solid { shape: Shape::Cuboid([SHIP_HALF_EXTENTS[0], SHIP_BULWARK_HEIGHT * 0.5, SHIP_BULWARK_THICK]), at: [0.0, BULWARK_Y, SHIP_HALF_EXTENTS[2] - SHIP_BULWARK_THICK], role: Role::Part },
            Solid { shape: Shape::Cylinder { half_height: SHIP_MAST_HEIGHT * 0.5, radius: SHIP_MAST_RADIUS }, at: [0.0, SHIP_DECK_Y + SHIP_MAST_HEIGHT * 0.5, SHIP_MAST_Z], role: Role::Part },
        ],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        bow: SHIP_BOW,
        springs: Springs::FIRM,
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: Some(SHIP_HELM),
    exit: Some(SHIP_HELM_EXIT),
    fittings: &[
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[0].0, yaw: SHIP_GUN_SLOTS[0].1 },
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[1].0, yaw: SHIP_GUN_SLOTS[1].1 },
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[2].0, yaw: SHIP_GUN_SLOTS[2].1 },
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[3].0, yaw: SHIP_GUN_SLOTS[3].1 },
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[4].0, yaw: SHIP_GUN_SLOTS[4].1 },
        Fitting { kind: KIND_SHIP_GUN, at: SHIP_GUN_SLOTS[5].0, yaw: SHIP_GUN_SLOTS[5].1 },
    ],
};
