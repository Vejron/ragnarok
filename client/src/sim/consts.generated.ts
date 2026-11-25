// GENERATED from crates/shared/src/consts.rs and vehicles/*.rs by scripts/gen-consts.ts. Do not edit:
// change the Rust and run `npm run gen:consts`. Client-only constants live in consts.ts.

/** Wire schema version. Bumped on any incompatible protocol or world-format change. */
export const PROTOCOL_VERSION = 99

/** Why a page closes its session (`Bye`): the tab is closing or reloading. */
export const BYE_LEAVING = 0

/** Why a page closes its session (`Bye`): it rejoins, because the session went dead or out of step. */
export const BYE_REJOINING = 1

/**
 * Entity kinds, the `kind` component: which model a client draws. A vehicle kind's definition is
 * `crate::vehicles`.
 */
export const KIND_SOLDIER = 0

/** A Hedemora rooster, an NPC. Replicated like a soldier, with its own model. */
export const KIND_ROOSTER = 5

/** Vehicle kinds: replicated with an `orientation`. */
export const KIND_JEEP = 1
export const KIND_TANK = 2
export const KIND_HELI = 3
export const KIND_SPORTS = 4

/** The triplane. 5 is the rooster's. */
export const KIND_PLANE = 6

/** The two-legged walker. 5 is the rooster's. */
export const KIND_WALKER = 7

/** The turretless Strv S tank, one seat. */
export const KIND_STRV = 8

/** The towed field mortar, one seat for its gunner. */
export const KIND_MORTAR = 9

/** The Tesla Roadster, a sports car with cold-gas thrusters: boost, hop and hover. Two seats. */
export const KIND_ROADSTER = 10

/** An FPV drone, flown from afar by the player who launched it. Gone once it blows up or its pilot lets go. */
export const KIND_DRONE = 11

/** The hen: a giant robot hen on the walker's legs that jumps, flaps and glides, and lays egg bombs. One seat. */
export const KIND_HEN = 12

/**
 * A loitering drone: a Shahed-sized delta wing with a big warhead, flown from afar like an FPV drone
 * but launched high above and behind its pilot, and always cruising forward.
 */
export const KIND_LOITER = 13

/** The Pak 38: a towed anti-tank gun with a shield and one gunner, who lays it like a tank's gun. */
export const KIND_PAK = 14

/** The Intinor Compact Backpack lying on the ground (see `BACKPACK_*`). Replicated as a place and this kind. */
export const KIND_BACKPACK = 15

/** The tupp: a giant robot Hedemora rooster on the hen's legs and wings, with laser eyes. One seat. */
export const KIND_TUPP = 16

/** The snowmobile: a mountain sled for two that tows a Pak or a mortar up slopes no car can climb. */
export const KIND_SNOWMOBILE = 17

/** The pirate ship: sailed by the wind on open water from its helm, its deck walked, with six guns. */
export const KIND_SHIP = 18

/** One of a pirate ship's guns, fixed to its deck and manned by one gunner. */
export const KIND_SHIP_GUN = 19

/**
 * Wildlife: a deer (a hind) and a stag, NPCs that graze in herds and bolt from people and gunfire.
 * Replicated like the rooster, with their own models.
 */
export const KIND_DEER = 20
export const KIND_STAG = 21

/** The golf cart: two seats side by side under a roof, an electric motor, and a bag rack on the back. */
export const KIND_GOLF_CART = 22

/** The technical: a pickup with a machine gun in its bed, a driver, a passenger and a gunner. */
export const KIND_TECHNICAL = 23

/**
 * A zombie: a soldier's body with no gun and no mind but hunger, that rises at night in a scenario's
 * zombie areas (ZOMBIES.md). Replicated like a soldier, drawn with the soldier's model in rags.
 */
export const KIND_ZOMBIE = 24

/**
 * The submarine: a captain who steers and dives it, and a weapons officer at the periscope who fires
 * guided missiles from its launch tubes.
 */
export const KIND_SUBMARINE = 25

/** A submarine's guided missile, flown from its nose by the officer who fired it, as a drone is. */
export const KIND_MISSILE = 26

/**
 * The seaplane: a two-seat biplane on pontoons that carry small wheels, which flies from water or a road.
 * Armed as the triplane is.
 */
export const KIND_SEAPLANE = 27

/**
 * The Strf 90: a tracked infantry fighting vehicle with a 40 mm autocannon in its turret and two
 * troops in the back, who get out by the rear ramp.
 */
export const KIND_STRF = 28

/**
 * The snowracer: a steerable sled for two with a small motor, which slides down a ski slope as fast
 * as a snowmobile drives and creeps back up it.
 */
export const KIND_SNOWRACER = 29

/**
 * The Titan: a giant robot fifty metres high, whose pilot moves its fists: it punches, picks up and
 * throws, and crushes what it steps on.
 */
export const KIND_TITAN = 30

/**
 * The bus: a twelve-metre city bus with a driver and twelve passengers. On a world with bus lines the
 * server drives them along the major roads, from stop to stop.
 */
export const KIND_BUS = 31

/**
 * The Pbv 302: an amphibious tracked troop carrier with a 20 mm automatic cannon in a small turret. It
 * floats, and its tracks paddle it slowly across open water.
 */
export const KIND_PBV = 32

/**
 * The AMV XA-361: an eight-wheeled armoured mortar carrier, its turret's two 120 mm tubes lobbing a
 * pair of bombs in quick succession before a reload.
 */
export const KIND_AMV = 33

/**
 * The Stridsbåt 90: a fast assault craft on waterjets, its driver working twin machine guns on the bow
 * and its gunner a 25 mm automatic cannon in a turret on the wheelhouse roof.
 */
export const KIND_CB90 = 34

/**
 * The raft: a small inflatable river-rafting boat for two with a powerful outboard, light and shallow
 * enough to run a rapid.
 */
export const KIND_RAFT = 35

/**
 * A delivery's parcel lying on the ground (DELIVERY.md), dropped by a courier who died. Replicated as
 * a place and this kind.
 */
export const KIND_PARCEL = 36

/** The air ambulance: an unarmed helicopter for a pilot and three passengers, with a shrouded tail fan. */
export const KIND_AIR_AMBULANCE = 37

/**
 * The EPA traktor: an old saloon cut down into a two-seat pickup and held to 30 km/h, a Swedish
 * teenager's first car.
 */
export const KIND_EPA = 38

/**
 * Someone who drives a car of the traffic (TRAFFIC.md): at its wheel, or on foot once they have
 * left it or been dragged out. Drawn as a soldier with nothing in hand.
 */
export const KIND_CIVILIAN = 39

/**
 * The period cars of the traffic, 1980 to 1995 (VEHICLES-XXIII.md); the jeep and the sports car are two more.
 * The Saab 900 Turbo: a three-door hatchback with a wraparound windscreen, slow off the line and the
 * fastest of the everyday cars flat out.
 */
export const KIND_SAAB_900 = 40

/** The Saab 9000: a big, quick five-door hatchback with a turbo. */
export const KIND_SAAB_9000 = 41

/** The Volvo 240 estate: the brick, heavy, slow and softly sprung. */
export const KIND_VOLVO_240 = 42

/** The Volvo 740: a square-cut saloon, in the middle of the pack for pace. */
export const KIND_VOLVO_740 = 43

/** The Volkswagen Golf GTI: a light hot hatch, the quickest of the everyday cars off the line. */
export const KIND_GTI = 44

/** The Volkswagen Transporter: a flat-fronted van for seven, tall and slow. */
export const KIND_TRANSPORTER = 45

/** The Opel Kadett: a wedge-shaped five-door hatchback, the everyday small car. */
export const KIND_KADETT = 46

/** The Opel Omega: a long, smooth saloon that cruises fast. */
export const KIND_OMEGA = 47

/** The Ford Sierra: a rounded five-door family hatchback. */
export const KIND_SIERRA = 48

/** The Ford Escort: a small, light, slow hatchback. */
export const KIND_ESCORT = 49

/** A police car: a white Volvo 240 with a blue stripe, a light bar and a siren (`FLAG_SIREN`). */
export const KIND_POLICE_CAR = 50

/**
 * An ambulance on the road: a Transporter van with a light bar and a siren. Not the helicopter
 * (`KIND_AIR_AMBULANCE`).
 */
export const KIND_AMBULANCE = 51

/**
 * A police officer on foot or in a seat: a person the server runs, drawn as a soldier in a dark
 * blue uniform. Replicated as a civilian is.
 */
export const KIND_OFFICER = 52

/**
 * An ambulance medic on foot or in a seat: a person the server runs, drawn as a soldier in green
 * and yellow. Replicated as a civilian is.
 */
export const KIND_MEDIC = 53

/** Fixed simulation rate, in ticks per second. */
export const TICK_RATE = 60

/** Seconds per simulation tick. */
export const TICK_DT = 1.0 / TICK_RATE

/** A snapshot is sent every this many ticks (60 / 2 = 30 Hz). */
export const SNAPSHOT_EVERY = 2

/**
 * Players admitted to the single room. Interest management and the packet budget (`MAX_PACKET`)
 * keep a snapshot within one datagram however many of them are in view.
 */
export const MAX_PLAYERS = 64

/** Outfit colours a player can pick from (`client/src/profile.ts` holds the colours themselves). */
export const PLAYER_COLOR_COUNT = 12

/** Spawn points a player may choose, used in turn each time they spawn. */
export const MAX_SPAWNS = 5

/** Longest player name, in characters. */
export const MAX_NAME_CHARS = 16

/** Datagrams above this size are dropped unread. Stays under a typical QUIC datagram MTU. */
export const MAX_PACKET = 1200

/**
 * A player is sent the entities, vehicles and effects within this many metres of their soldier
 * (measured on the ground plane), and keeps them until they are `INTEREST_HYSTERESIS` further out,
 * so something at the edge does not flicker in and out.
 */
export const INTEREST_RADIUS = 300.0
export const INTEREST_HYSTERESIS = 40.0

/**
 * Snapshots the server remembers per player to diff against, and the client remembers to
 * rebuild from. The client keeps at least as many as the server, so any baseline the server
 * still offers is one the client still has. At 30 Hz, 32 is about a second.
 */
export const BASELINE_WINDOW = 32
export const BASELINE_WINDOW_CLIENT = 64

/** Each input datagram repeats this many of the most recent frames, so a lost packet costs nothing. */
export const INPUT_REDUNDANCY = 4

/** Input buttons, as bits of `InputFrame::buttons`. */
export const BUTTON_JUMP = 1 << 0
export const BUTTON_RUN = 1 << 1
export const BUTTON_USE = 1 << 2

/** Gunner's trigger. Held to keep firing; the gun's reload paces it. */
export const BUTTON_FIRE = 1 << 3

/** Aim held (right mouse): looks down the scope of a gun that has one, and tightens any gun's spread. */
export const BUTTON_ALT = 1 << 4

/** Held to crouch: slower, and a lower eye. */
export const BUTTON_CROUCH = 1 << 5

/** One press, sent with run and crouch held: starts a slide. Like a jump it is not repeated. */
export const BUTTON_SLIDE = 1 << 6

/** One press: reload the selected gun. Like a jump it is not repeated. */
export const BUTTON_RELOAD = 1 << 7

/** Held to lie down: slower, and a low eye. Lets go to stand again, if there is room overhead. */
export const BUTTON_PRONE = 1 << 8

/** One press: hitch a free mortar to the car in front of you, or unhitch the one it tows. Like a jump it is not repeated. */
export const BUTTON_HITCH = 1 << 9

/** Held by a mortar's gunner to walk the range dial out and in (W and S). */
export const BUTTON_RANGE_UP = 1 << 10
export const BUTTON_RANGE_DOWN = 1 << 11

/**
 * One press: open or cut the parachute while in the air, or, for a Titan's pilot, send up a missile
 * from its shoulder (G). Like a jump it is not repeated.
 */
export const BUTTON_CHUTE = 1 << 12

/** One press, at a gun that has two kinds of round: load the other one (Q). Like a jump it is not repeated. */
export const BUTTON_AMMO = 1 << 13

/** One press: the headlamp on or off (L). Like a jump it is not repeated. */
export const BUTTON_LAMP = 1 << 14

/** Puts skis on or takes them off, in a winter world, nearly still (`SKI_CLIP_SPEED`). The last bit. */
export const BUTTON_SKIS = 1 << 15

/** Soldier movement, in metres and seconds. */
export const WALK_SPEED = 4.0
export const RUN_SPEED = 7.5

/** Crouching overrides running. */
export const CROUCH_SPEED = 2.0

/** Lying down overrides both: crawling. */
export const PRONE_SPEED = 1.0

/** A slide is a burst of knockback along the run direction that bleeds off slowly while crouch is held. */
export const SLIDE_SPEED = 12.0
export const SLIDE_KEEP = 0.96

/** A new slide can start once the last has slowed below this, in m/s. */
export const SLIDE_RETRIGGER = 1.0

/** Above this much leftover velocity, a grounded crouching player is sliding. */
export const SLIDE_MIN_SPEED = 2.0

/** A rested jump's take-off speed, m/s: 1.6 times the height of a winded one (`JUMP_SPEED_WINDED`), as height goes by its square. */
export const JUMP_SPEED = 8.22
export const JUMP_SPEED_WINDED = 6.5

/**
 * Stamina on foot, in whole points so the server and the prediction agree on it exactly. Running on the
 * ground spends `STAMINA_RUN_DRAIN` a tick (a full bar lasts 5 s) and a jump `STAMINA_JUMP_COST`;
 * otherwise, with a foot down, `STAMINA_REFILL` a tick comes back. In the air it neither drains nor fills.
 */
export const STAMINA_FULL = 6000
export const STAMINA_RUN_DRAIN = 20
export const STAMINA_JUMP_COST = 900
export const STAMINA_REFILL = 15

/**
 * Once it runs out the player is winded: running is walking and a jump is a `JUMP_SPEED_WINDED` one,
 * until this much has come back.
 */
export const STAMINA_RECOVER = 1800
export const GRAVITY = 20.0

/**
 * Knockback (a car hitting a soldier) is a horizontal velocity added to the walking velocity.
 * Each tick it keeps this fraction of itself: quickly on the ground, barely in the air.
 */
export const KNOCK_KEEP_GROUND = 0.92
export const KNOCK_KEEP_AIR = 0.99

/** Below this speed (m/s) knockback is dropped, so it always dies out exactly. */
export const KNOCK_MIN_SPEED = 0.05

/** Fastest knockback the wire can carry and the server will give, m/s. */
export const KNOCK_MAX_SPEED = 20.0

/** Terminal fall speed, so `vy` always fits its i16 cm/s wire field. */
export const MAX_FALL_SPEED = 50.0

/** Parachute. An open canopy settles the fall at this speed, m/s; a faster fall is braked at `CHUTE_DRAG` m/s². */
export const CHUTE_DESCENT = 5.0
export const CHUTE_DRAG = 40.0

/** Glide speed under a canopy: full forward, backward (braking) and sideways, m/s. */
export const CHUTE_FORWARD = 15.0
export const CHUTE_BACK = 3.0
export const CHUTE_SIDE = 8.0

/** How much of the gap to the wanted glide velocity is closed each tick: a canopy turns lazily. */
export const CHUTE_STEER = 0.03

/** Respawns drop in from this far above the ground, metres, with the canopy open. */
export const SPAWN_ALTITUDE = 100.0

/**
 * A river runs white from the first of these speeds of its current, m/s, and all of it is white by
 * the second. The importer makes its foam there (`riverflow::foam`) and the water shader whitens it.
 */
export const WHITE_WATER_SPEEDS = [1.5, 3.2] as const

/**
 * The colour of a river's water where it is deep, sRGB red, green and blue, on a world that does
 * not set its own (`Manifest::river_tint`): the brown-green of a northern forest river, stained by
 * the bogs it drains.
 */
export const RIVER_TINT = [87, 82, 42] as const

/**
 * Swimming (SWIMMING.md). Depths, the float height and whether the eyes are under all follow the
 * waves on the water (`shared::waves`). Water this deep over the feet, metres, floats a soldier off them: wading turns
 * into swimming. Swimming stops once it is no deeper than `SWIM_LEAVE_DEPTH`, so the change does not
 * flicker on a bank; a swimmer floating at `SWIM_FLOAT` has exactly `SWIM_DEPTH` over the feet.
 */
export const SWIM_DEPTH = 1.3
export const SWIM_LEAVE_DEPTH = 1.1

/**
 * How far below the still surface a floating swimmer's capsule centre is, metres: the eye
 * (`EYE_HEIGHT` over the centre) is just out of the water.
 */
export const SWIM_FLOAT = 0.45

/** Swimming speed, m/s, and with run held, which spends stamina as running does. */
export const SWIM_SPEED = 2.0
export const SWIM_FAST_SPEED = 3.2

/** Space kicks a swimmer up at this speed, m/s, and crouch takes them down at `SWIM_SINK`. */
export const SWIM_RISE = 1.6
export const SWIM_SINK = 1.4

/** A diver who is not swimming drifts up at this speed, m/s. */
export const SWIM_DRIFT = 0.5

/**
 * A swimmer this far below the float height, metres, is under: looking up or down steers where they
 * swim. Above it the water floats them back up unless they are diving, so nobody hangs just under the
 * surface with their eyes in and out of it.
 */
export const SWIM_UNDER = 0.9

/**
 * At the surface, swimming forward while looking down by more than this, radians (52°, near the feet),
 * dives: far past where the camera looks while swimming along, so only a swimmer who means it goes under.
 */
export const SWIM_DIVE_PITCH = 0.9

/** A dive from the surface goes down at least this fast, m/s, until the swimmer is under (`SWIM_UNDER`). */
export const SWIM_DUCK = 1.5

/** How hard the water pulls a swimmer back to the float height, m/s for each metre off it. */
export const SWIM_BUOYANCY = 4.0

/**
 * How fast a swimmer's vertical speed comes round to what they want, m/s², plus `SWIM_DRAG` m/s²
 * for every m/s it is off: a plunge from a height is braked hard, a stroke changes gently.
 */
export const SWIM_ACCEL = 6.0
export const SWIM_DRAG = 3.0

/** Each tick in the water knockback keeps this fraction of itself. */
export const SWIM_KNOCK_KEEP = 0.9

/** A swimmer climbs out over a bank or a jetty's edge this high, metres, and up slopes this steep, degrees. */
export const SWIM_CLIMB = 0.6
export const SWIM_CLIMB_DEG = 75.0

/**
 * Air held on one breath, ticks (30 s at `TICK_RATE`), and air regained every tick with the eyes out
 * of the water (empty to full in 5 s).
 */
export const BREATH_FULL = 1800
export const BREATH_REFILL = 6

/**
 * Out of air, a diver drowns: this much off the torso every `DROWN_EVERY` ticks (15 hp a second, so
 * full health lasts 10 s) until they come up for air.
 */
export const DROWN_DAMAGE = 15
export const DROWN_EVERY = 60

/**
 * Soldier collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
 * body origin at the capsule centre.
 */
export const CAPSULE_HALF_HEIGHT = 0.5
export const CAPSULE_RADIUS = 0.35

/**
 * Lying down, the body is a ball of this radius (0.6 m across) resting on the ground. Its bottom is
 * where the standing capsule's is, so going prone drops the centre by `STAND_CENTRE - PRONE_CENTRE`.
 */
export const PRONE_RADIUS = 0.3

/** Capsule-centre height above the feet, standing and prone. */
export const STAND_CENTRE = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS
export const PRONE_CENTRE = PRONE_RADIUS

/** Stance, as `replicated::Flags::stance` on the wire. Crouching is a held button, not a stance. */
export const STANCE_STANDING = 0
export const STANCE_PRONE = 1

/** Hanging under an open parachute. */
export const STANCE_CHUTE = 2

/** Knocked down by a blast: thrown, then lying helpless until `Character::down` runs out (KNOCKDOWN.md). */
export const STANCE_DOWN = 3

/** Hands up: a bot that has given in. */
export const STANCE_SURRENDER = 4

/** Swimming in deep water with the head out (`Character::swimming`). The gun is put away. */
export const STANCE_SWIM = 5

/** Swimming under the water, the eyes below its surface: holding the breath (`BREATH_FULL`). */
export const STANCE_DIVE = 6

/** A zombie climbing out of the ground (`ZOMBIE_RISE_TICKS`): it neither moves nor claws until it is up. */
export const STANCE_RISE = 7

/**
 * A dead player come back as a ghost (GHOSTS.md): a shadowy cloud that flies through everything and
 * can touch nothing, and nothing can touch it.
 */
export const STANCE_GHOST = 8

/**
 * A ghost taking shape again (`GHOST_FORM_TICKS`): a dark swirl sinking to the ground, still
 * untouchable and unable to act, that stands up as the soldier.
 */
export const STANCE_FORM = 9

/** A blast that throws a soldier at least this hard, m/s, knocks them down. */
export const KNOCKDOWN_POWER = 7.0

/**
 * Ticks they lie there once landed: this many at `KNOCKDOWN_POWER`, rising evenly to
 * `KNOCKDOWN_MAX_TICKS` at `KNOCK_MAX_SPEED`.
 */
export const KNOCKDOWN_MIN_TICKS = 48
export const KNOCKDOWN_MAX_TICKS = 150

/**
 * Ticks getting up off the ground takes once a knockdown is over (the `getup_back` and
 * `getup_front` clips): a mind that drives a soldier keeps them where they are meanwhile.
 */
export const GETUP_TICKS = 66

/** A round that hits above this height over the capsule centre hit the head. */
export const HEAD_ZONE_Y = 0.45

/**
 * Body-part health. A soldier's torso has `PLAYER_MAX_HP`, its head `HEAD_HP` and each limb `LIMB_HP`.
 * Only the torso or the head running out kills; a limb that runs out comes off, and what the round had
 * left over is lost. A sniper round to the head kills, as do three revolver rounds or seven AK5 rounds.
 */
export const HEAD_HP = 75
export const LIMB_HP = 60

/**
 * A soldier mends by themselves: once they have gone `MEND_DELAY_SECONDS` unhurt, the torso and the head
 * heal evenly back to full over `MEND_SECONDS` from nothing. Limbs do not; a lost one needs a bandage.
 */
export const MEND_SECONDS = 60.0
export const MEND_DELAY_SECONDS = 5.0

/**
 * Limbs. A round below this height over the capsule centre hit a leg (the hips are about 0.75 m up),
 * and one between there and the head this far or more to the side of the body's middle (the outer
 * edge of the chest), an arm.
 */
export const HIP_ZONE_Y = -0.1
export const ARM_ZONE_X = 0.25

/**
 * How far out a round finds a soldier from the middle of their body: wider than the 0.35 m they
 * move with, for the arms as they are drawn.
 */
export const HURT_RADIUS = 0.43

/** One leg: each step is a hop up at this speed, m/s, at this share of walking speed; no running or sliding. */
export const HOP_SPEED = 2.6
export const HOP_SCALE = 0.55

/** One leg: a jump's take-off speed. */
export const HOP_JUMP_SPEED = 4.5

/** No legs: always lying down, crawling at this share of `PRONE_SPEED`. */
export const LEGLESS_CRAWL_SCALE = 0.7

/** One arm: a gun's spread grows by this, radians (aiming no longer steadies it), and a reload takes this many times as long. */
export const ONE_ARM_SPREAD = 0.035
export const ONE_ARM_RELOAD = 2

/** One arm: how far the aim wanders, radians, as the client sways the look it sends. */
export const ONE_ARM_SWAY = 0.03

/** Character controller tuning. The client builds its Rapier controller from the same values. */
export const CONTROLLER_OFFSET = 0.02
export const MAX_SLOPE_CLIMB_DEG = 50.0
export const AUTOSTEP_HEIGHT = 0.35
export const AUTOSTEP_MIN_WIDTH = 0.2

/**
 * How far below its feet a standing character is pulled back onto the ground when it walks off
 * a slope, metres. Without it, running downhill is a series of small falls.
 */
export const SNAP_DISTANCE = 0.3

/**
 * Share of walking speed lost per unit of grade (height over distance) going uphill, and the least
 * that can leave. A 10% climb costs 15%; the steepest is a slog. Going down costs nothing, and gives
 * nothing: any faster and a runner leaves the ground.
 */
export const SLOPE_CLIMB_DRAG = 1.5
export const SLOPE_SCALE_MIN = 0.4

/**
 * Share of walking speed stony ground takes at its stoniest (`Tile::stones`), for cobbles and for a
 * boulder field's blocks: ISOM's grades come out near slow running 0.85, walk 0.6 and fight 0.4.
 */
export const STONY_DRAG = 0.6
export const BOULDER_DRAG = 0.66

/** Further than this above the terrain, metres, a soldier is on a roof or a bridge, and its slope is not theirs. */
export const SLOPE_MAX_CLEARANCE = 2.0

/** Anything that falls below this is respawned (e.g. a player squeezed through a gap in the mesh). */
export const KILL_Y = -50.0

/**
 * Vehicles. Chassis and wheel geometry are shared with the client, which draws the car from them.
 * Body-local axes: +X right, +Y up, -Z forward (yaw 0 faces -Z, like a soldier).
 * Seats in a jeep.
 */
export const CAR_SEATS = 4

/** The most seats any vehicle has (the bus); bounds the seat list of any vehicle. */
export const MAX_SEATS = 13

/**
 * Vehicles in the world; bounds the vehicle list in a snapshot. The map editor's placed vehicles and the
 * drones players launch share it, so it leaves room past the editor's full count for drones to launch into.
 */
export const MAX_VEHICLES = 512

/** A player can enter a vehicle whose centre is within this distance of their capsule centre. */
export const ENTER_RADIUS = 4.0

/** How far a car's door swings open, radians, and the damage it takes before it comes off (DOORS.md). */
export const DOOR_SWING = 1.1
export const DOOR_HP = 120

/** How long a door takes to swing open or shut, seconds: who gets out waits for it to open first. */
export const DOOR_OPEN_S = 0.25

/**
 * How long getting out of a seat by its door takes, once the door is open, seconds; getting in
 * takes as long, the same moves the other way round.
 */
export const STEP_OUT_S = 0.8

/** How long after they are out someone shuts the door behind them, seconds. */
export const DOOR_SHUT_S = 0.25

/**
 * How hard the air pushes on a door standing open, per (m/s)² of the wind across it, over the door's
 * mass, per metre: driving forwards blows an open door shut, reversing blows it open.
 */
export const DOOR_AIR = 0.03

/** How fast a swinging door's hinge slows its swing, per second. */
export const DOOR_DAMPING = 1.5

/**
 * A door left standing still on its hinge stays where it is until something swings it harder than
 * this, radians a second squared: a parked car's door on a gentle slope stays open.
 */
export const DOOR_HINGE_HOLD = 1.0

/**
 * A door that slams onto its open stop faster than this, radians a second, is wrenched by it, and
 * takes `DOOR_STOP_HURT` for each radian a second over.
 */
export const DOOR_STOP_TEAR = 2.0
export const DOOR_STOP_HURT = 80.0

/**
 * A door held against its open stop harder than this, radians a second squared (by the wind of a car
 * reversing fast), is bent past it, and takes `DOOR_STRAIN_HURT` a second for each radian a second
 * squared over.
 */
export const DOOR_STOP_STRAIN = 1.5
export const DOOR_STRAIN_HURT = 150.0

/** How much of its swing a door keeps as it bounces back off its open stop. */
export const DOOR_STOP_BOUNCE = 0.3

/** An open door that snags on something going faster than this past it, m/s, is torn off at once. */
export const DOOR_SNAG_TEAR = 2.5

/**
 * What an open door takes for each second it scrapes past something slower than `DOOR_SNAG_TEAR`,
 * for each m/s it goes past it.
 */
export const DOOR_SNAG_HURT = 60.0

/**
 * Where someone who got out by a door stands: this far out from the side of the body, metres, and
 * this share of the way back along the door from its hinge, in the gap between the open door and the car.
 */
export const STEP_OUT_SIDE = 0.55
export const STEP_OUT_BACK = 0.7

/**
 * Leaving a car going faster than this, m/s, is bailing out: no door, no climbing out, but falling out
 * onto the road at the car's speed, knocked down.
 */
export const BAIL_SPEED = 6.0

/**
 * Gravity acting on vehicle bodies. Soldiers use `GRAVITY`; the ray-cast suspension is tuned
 * around the real-world value.
 */
export const CAR_GRAVITY = 9.81

/**
 * Highest relief a world may have, metres from its lowest dry ground (y = 0) to its highest. The
 * importer refuses more; the boundary walls reach above it, and fliers find the terrain under them by
 * a ray from just over it.
 */
export const MAX_RELIEF = 1500.0

export const ROCKET_ACCEL = 110.0
export const ROCKET_MAX_SPEED = 170.0

export const GUN_COOL = 1.0
export const GUN_UNJAM = 77.0
export const BOMB_RADIUS = 6.0
export const BOMB_DAMAGE = 110.0
export const BOMB_KNOCK = 16.0

/**
 * Craters (CRATERS.md): anything explosive that bursts on the ground dents it in a smooth bowl
 * `radius` metres across the way out and `depth` metres deep at the middle. The lighter blasts dig
 * about 0.8 of their blast radius, as the tank shell does.
 */
export const CRATER_SHELL_RADIUS = 5.6
export const CRATER_SHELL_DEPTH = 1.0
export const CRATER_MORTAR_RADIUS = 8.8
export const CRATER_MORTAR_DEPTH = 1.6
export const CRATER_BOMB_RADIUS = 7.0
export const CRATER_BOMB_DEPTH = 2.2
export const CRATER_ROCKET_RADIUS = 4.0
export const CRATER_ROCKET_DEPTH = 0.7
export const CRATER_GRENADE_RADIUS = 3.5
export const CRATER_GRENADE_DEPTH = 0.5
export const CRATER_CANNON_RADIUS = 2.0
export const CRATER_CANNON_DEPTH = 0.3
export const CRATER_EGG_RADIUS = 5.0
export const CRATER_EGG_DEPTH = 0.9

/** A shell digs only if it bursts within this far above the terrain, metres: not on a roof or in the air. */
export const CRATER_REACH = 1.5

/** A `ShotEvent`'s `yaw` and `dip` when what went off had no heading: straight down. */
export const SHOT_NO_YAW = 0
export const SHOT_NO_DIP = 255

/**
 * How much of its contact with the ground a burst keeps when its shell struck a vehicle and not the
 * ground itself (`CraterEntry::contact`): the crater is marked as one blown from above.
 */
export const CRATER_DIRECT_CONTACT = 0.5

/**
 * How deep under the still water a gun's round, hitscan or in flight, goes before the water has
 * stopped it, metres, and the most it goes through the water on the way, metres: enough for a
 * submarine's sail just under the surface, not for one at periscope depth.
 */
export const ROUND_WATER_DEPTH = 1.0
export const ROUND_WATER_PATH = 4.0

/**
 * The same for a shell or a rocket, metres: it reaches a submarine's hull at periscope depth. What it
 * meets under there is struck as it would be in the air; what it does not, it bursts on the surface.
 */
export const SHELL_WATER_DEPTH = 3.5
export const SHELL_WATER_PATH = 12.0

/** However many craters land on a place, the ground goes no lower than this under what it was, metres. */
export const CRATER_MAX_DEPTH = 4.0

/** The most craters whose marks a joiner is sent (the oldest are dropped first). Their heights are sent whole, in `Dug`. */
export const CRATER_MAX_KEPT = 2000

/** A crater's scorch fades out over this long, seconds; then it is forgotten. Its hole stays. */
export const CRATER_FADE_SECONDS = 30 * 60

/**
 * Craters heal (CRATERS.md, "Healing"): ground a crater dug goes back to what the importer left this
 * long after the last crater that dug it, seconds, once nobody can see it happen.
 */
export const CRATER_HEAL_SECONDS = 7 * 60

/**
 * The world does not heal (craters filling in, trees growing back, fences mended) where a player is
 * nearer than this, metres on the ground plane, so that nobody sees it happen.
 */
export const HEAL_CLEAR = 400.0

/**
 * Ground does not heal while anything (a soldier, a vehicle, an item, a placed cube) is within this
 * of it, metres, so that nothing is buried as the ground rises.
 */
export const CRATER_HEAL_MARGIN = 4.0

/** How often the server looks for ground to heal and trees to grow, ticks. */
export const HEAL_CHECK_TICKS = 60

/**
 * Regrowth (REGROWTH.md): a felled tree grows back through stages, each taken only where nobody can
 * see it. `GROWN_LOG`: it lies felled. `GROWN_SAPLING`: the log has rotted and a sapling stands, not
 * solid. `GROWN_YOUNG`: a young tree, solid, which can be felled again. `GROWN_WHOLE`: as it was.
 * A broken piece of fence or wall goes from `GROWN_LOG` (broken) straight to `GROWN_WHOLE`.
 */
export const GROWN_LOG = 0
export const GROWN_SAPLING = 1
export const GROWN_YOUNG = 2
export const GROWN_WHOLE = 3

/**
 * How big a sapling and a young tree are, as a share of the tree they grow into: drawn, and for a
 * young tree its trunk and its health too.
 */
export const TREE_SAPLING_SCALE = 0.25
export const TREE_YOUNG_SCALE = 0.6

/** When each stage may come, seconds after the tree was felled. */
export const REGROW_SAPLING_SECONDS = 4 * 60
export const REGROW_YOUNG_SECONDS = 9 * 60
export const REGROW_WHOLE_SECONDS = 15 * 60

/** When a broken piece of fence or wall may be mended, seconds after it broke. */
export const LINE_MEND_SECONDS = 15 * 60

/**
 * What becomes solid again waits while anything is within this of it, metres: a tree's trunk, a piece
 * of fence or wall.
 */
export const REGROW_CLEARANCE = 1.0

/** The tank gun. */
export const SHELL_COOLDOWN_TICKS = 90

/**
 * Speed a shell leaves the muzzle at, m/s. It falls under `GRAVITY` from there, so at 100 m it has
 * dropped about 5 m and at 200 m about 20.
 */
export const SHELL_SPEED = 140.0

/**
 * A sniper round flies at twice a tank shell's speed, m/s, and falls under a quarter of gravity, so
 * at 200 m (0.7 s) it has dropped about 1 m.
 */
export const BULLET_SPEED = 2.0 * SHELL_SPEED
export const BULLET_GRAVITY_SCALE = 0.25

/**
 * Where the camera behind a mounted gun sits: this far back from the gun's pivot along the look, and
 * this far above that, looking along the look. The middle of the screen is that ray, and the server
 * points the gun at what the ray meets (`MountDef::eye`), so a round goes where the reticle is.
 * A gunner's: a tank's, a technical's, a helicopter's nose gun, a plane's rear gun, a Pak's, a ship's gun.
 */
export const GUNNER_BACK = 6.5
export const GUNNER_UP = 2.6

/** Over the shoulder of the gunner on a tank's roof. */
export const ROOF_AIM_BACK = 3.5
export const ROOF_AIM_UP = 1.2

/** Behind a walker's guns or a tupp's eyes, high enough to see over the cab. */
export const WALKER_AIM_BACK = 12.0
export const WALKER_AIM_UP = 5.0

/** Behind a Strv S's gun trunnion. */
export const STRV_AIM_BACK = 11.0
export const STRV_AIM_UP = 3.6

/**
 * How far the camera's ray is followed to find what a mounted gun's reticle is on, metres, when the
 * gun's own reach does not say.
 */
export const MOUNT_AIM_RANGE = 1500.0

/** A gun sight's eye sits this far above the gun's pivot, on the mantlet, and looks along the look. */
export const SIGHT_EYE_UP = 0.5

/**
 * How quickly, seconds, a mounted gun's lay eases to a new distance when the reticle moves onto
 * something nearer or further: long enough that the barrel glides, short enough to be laid before a
 * shot is lined up. Eased in proportion (a log scale), so 10 m to 300 m takes as long as 300 m to 10 m.
 */
export const MOUNT_AIM_EASE = 0.25

/**
 * Out to this far, metres, a mounted gun's barrel is raised for the shell's fall and the shell lands
 * where the reticle is; further off it falls short of it, and the gunner aims high to make up the rest.
 */
export const MOUNT_ZERO_RANGE = 100.0

/** The reticle camera is lifted to stay this far above the ground, metres, still looking along the look. */
export const MOUNT_EYE_CLEARANCE = 0.3

/** A gun fired with aim held scatters over this fraction of its hip-fire cone. */
export const AIM_SPREAD_SCALE = 0.25

/** ...and over this fraction of the bloom held fire has added to it (`items::Bloom`). */
export const AIM_BLOOM_SCALE = 0.6

/** How fast that bloom shrinks again once the trigger is let go, radians a second. */
export const BLOOM_RECOVER = 0.12

/** How long a shell flies before it bursts where it is. */
export const SHELL_TTL_TICKS = 6 * TICK_RATE

/** Speed a fired shell takes from the tank, m/s, backwards along the barrel. */
export const SHELL_RECOIL = 1.6

/** The tow point sits this far behind a car's rear face, metres. */
export const TOW_BACK = 0.25

/** How close a mortar's tongue must be to a car's tow point to hitch, metres. */
export const HITCH_REACH = 2.5

/** A hitch or an unhitch needs the car at a near standstill, m/s. */
export const HITCH_MAX_SPEED = 1.5
export const EXPLOSION_RADIUS = 7.0

/** The blast of an autocannon round: a third of a tank shell's radius, a third of its damage. */
export const CANNON_RADIUS = EXPLOSION_RADIUS / 3.0

/** Knockback speed at the centre of a blast, m/s; it fades to zero at the edge. */
export const EXPLOSION_KNOCK = 18.0
export const CANNON_KNOCK = EXPLOSION_KNOCK / 2.0

/**
 * The backpack mode (BACKPACK.md): a world whose scenario places one Intinor Compact Backpack. Whoever
 * carries it walks `BACKPACK_SPEED` times as fast on foot and heals `BACKPACK_REGEN` health a second.
 * Its modems give it away: everyone's map shows a circle with it somewhere inside, `BACKPACK_WIDE`
 * metres across while it is new to its carrier (or lying about), narrowing to `BACKPACK_NARROW` over
 * `BACKPACK_NARROW_SECONDS` in one carrier's hands.
 */
export const BACKPACK_SPEED = 2.0
export const BACKPACK_REGEN = 5.0
export const BACKPACK_WIDE = 150.0
export const BACKPACK_NARROW = 20.0
export const BACKPACK_NARROW_SECONDS = 300.0

/** How near a soldier on foot must walk to take it, metres across the ground and up or down. */
export const BACKPACK_REACH = 1.5
export const BACKPACK_REACH_Y = 2.5

/** Seconds it may lie untouched before it goes back where the scenario put it. */
export const BACKPACK_RETURN_SECONDS = 60.0

/** The `carry` component's value for the backpack. */
export const CARRY_BACKPACK = 1

/** Skis on: `carry` is a mask of what is worn. */
export const CARRY_SKIS = 2

/** The headlamp is on. */
export const CARRY_LAMP = 4

/** Poling hard: on skis on the ground, pushing with run held. */
export const CARRY_POLING = 8

/** A delivery's parcel on the back (DELIVERY.md). */
export const CARRY_PARCEL = 16

/**
 * The delivery mode (DELIVERY.md): jobs on a phone to fetch a parcel at a shop's door and carry it to a
 * house's. Whoever hands it in is paid `DELIVERY_BASE` plus `DELIVERY_PER_KM` for each kilometre from
 * the shop to the house, in full within the job's par time (`DELIVERY_PAR_GRACE` seconds plus the
 * distance at `DELIVERY_PAR_SPEED`), then less and less, down to `DELIVERY_FLOOR` of it once as long
 * again has gone by.
 */
export const DELIVERY_BASE = 20.0
export const DELIVERY_PER_KM = 60.0

/** Seconds every job's par time starts with, for the walk to the street. */
export const DELIVERY_PAR_GRACE = 60.0

/** The speed a job's par time allows for the way from shop to house, m/s: a brisk run. */
export const DELIVERY_PAR_SPEED = 5.0

/** The share of a job's pay that is left however late it is handed in. */
export const DELIVERY_FLOOR = 0.25

/** How far apart a job's shop and house may be, metres across the ground. */
export const DELIVERY_MIN_DISTANCE = 150.0
export const DELIVERY_MAX_DISTANCE = 1500.0

/**
 * How near a door a soldier on foot must walk to fetch or hand in a parcel, or to a parcel lying on
 * the ground to take it, metres across the ground and up or down.
 */
export const DELIVERY_REACH = 3.0
export const DELIVERY_REACH_Y = 2.5

/** Seconds a dropped parcel may lie untouched before it spoils and its job is over. */
export const DELIVERY_SPOIL_SECONDS = 120.0

/** Jobs on the board at once, unless the scenario says otherwise. */
export const DELIVERY_JOBS = 6

/** What a `Phone` message asks: to take a job off the board, or to give back one not yet fetched. */
export const PHONE_ACCEPT = 0
export const PHONE_CANCEL = 1

/**
 * `JobState::stage`: on the way to the shop, carrying the parcel to the house, or your job's parcel
 * lost (dropped where you died, or in someone else's hands).
 */
export const JOB_FETCH = 0
export const JOB_CARRY = 1
export const JOB_LOST = 2

/** Pull of gravity on a skier's slope, m/s². Real, unlike the game's heavier `GRAVITY` for flight. */
export const SKI_GRAVITY = 9.81

/** Friction on a groomed track and on loose snow. */
export const SKI_MU_TRACK = 0.03
export const SKI_MU_SNOW = 0.08

/** Friction in a groove of a track: the best going there is. */
export const SKI_MU_GROOVE = 0.022

/** How far each classic groove lies from a track's centre line, metres. */
export const SKI_GROOVE_OFFSET = 0.8

/** Cosine of how far off a groove skis can point or go and stay in it, or drop into it (28°). */
export const SKI_GROOVE_HOLD = 0.883

/** How hard a groove draws skis to its line: m/s sideways per metre off it, and the most. */
export const SKI_GROOVE_PULL = 7.0
export const SKI_GROOVE_PULL_MAX = 2.5

/** Air drag per (m/s)², upright and in a tuck. */
export const SKI_DRAG = 0.005
export const SKI_DRAG_TUCK = 0.0025

/** What the poles put out, W/kg: over speed, it is the push. */
export const SKI_POWER = 3.0

/** What the legs alone put out, W/kg: a gun in hand leaves no hand for the poles. */
export const SKI_KICK_POWER = 1.0

/** What run held adds to the push from a standstill, m/s². It fades away to nothing at `SKI_BOOST_TOP`. */
export const SKI_BOOST = 3.5

/** The speed past which poling hard gives no more, m/s: 43 km/h. */
export const SKI_BOOST_TOP = 12.0

/** The speed a push is worked out from below this: a standing start does not push forever harder. */
export const SKI_PUSH_MIN_SPEED = 0.6

/** Climbing steeper than this along the skis is a herringbone: stepping up on the inner edges, radians (12°). */
export const SKI_HERRING_FROM = 0.21

/** The steepest a herringbone gets up, radians: 30° at a walk, 42° with run held. */
export const SKI_HERRING_SLOPE = 0.524
export const SKI_HERRING_SLOPE_RUN = 0.733

/** The pace it keeps on easy ground, m/s, walking and with run held. It falls by `SKI_HERRING_SLOWING` at the limit. */
export const SKI_HERRING_PACE = 1.5
export const SKI_HERRING_PACE_RUN = 2.4
export const SKI_HERRING_SLOWING = 0.6

/** How fast a herringbone gets up to its pace, m/s². */
export const SKI_HERRING_ACCEL = 5.0

/** Skiing hard spends stamina this many times slower than running. */
export const SKI_STAMINA_SHARE = 3
export const SKI_MAX_SPEED = 25.0

/** Fastest a skier slides backwards. */
export const SKI_SLIP_SPEED = 4.0
export const SKI_JUMP_SPEED = 4.5

/** Gravity on a skier in the air, m/s²: lighter than `GRAVITY`, the game's heavy one for runners. */
export const SKI_AIR_GRAVITY = 13.0

/** Over a crest the ground ahead is felt this far off, seconds of travel, and from this speed, m/s. */
export const SKI_CREST_LOOK = 0.12
export const SKI_CREST_SPEED = 6.0

/** How much the ground's climb rate can fall over that look before the legs cannot follow it, m/s. */
export const SKI_CREST_DROP = 1.7

/** Slowing a full snowplow adds, m/s², and how much better it holds a turn. */
export const SKI_PLOW_BRAKE = 3.5
export const SKI_PLOW_GRIP = 1.6

/** Most sideways acceleration the edges give, m/s²: how fast the velocity follows the skis. */
export const SKI_GRIP = 7.0

/** Below this the skis are stepped round and the velocity goes where they point, m/s. */
export const SKI_SHUFFLE_SPEED = 1.5

/** How far strafing turns the skis from the look (the tangent of 45°). */
export const SKI_STRAFE_TURN = 1.0

/** Too tight: speed × the angle between skis and travel past this (m/s × radians) strains the edges. */
export const SKI_EDGE = 11.0

/** No speed under this catches an edge, m/s. */
export const SKI_FALL_MIN_SPEED = 5.0

/** Ticks of strain that are a fall, and how fast it eases off. */
export const SKI_STRAIN_FALL = 12
export const SKI_STRAIN_EASE = 2

/** Hardest landing the legs take, m/s across the ground, and how much more in a tuck. */
export const SKI_LANDING = 8.0
export const SKI_LANDING_TUCK = 1.3

/** Landing above this speed with the skis more than this off the way they are going is a fall. */
export const SKI_LANDING_SKEW_SPEED = 6.0
export const SKI_LANDING_SKEW = 0.524

/** Above this speed, being stopped short by something is a crash, m/s. */
export const SKI_CRASH_SPEED = 8.0

/** Share of the wanted move that counts as stopped short. */
export const SKI_CRASH_MOVED = 0.4

/** How far a fall bounces the skier up, m/s. */
export const SKI_FALL_HOP = 2.5

/** Share of the speed a fall keeps each tick on the ground: a tumble, not a stop. */
export const SKI_TUMBLE_KEEP = 0.975

/**
 * Snap to the ground on skis, metres. Under the walkers': at speed a crest falls away faster than
 * this a tick, and the skier flies.
 */
export const SKI_SNAP_DISTANCE = 0.12

/** Fastest skis go on or off, m/s. */
export const SKI_CLIP_SPEED = 2.0

/** Combat. Every player starts a life with `PLAYER_MAX_HP` and dies at zero. */
export const PLAYER_MAX_HP = 150

/** Ticks a dead player waits before respawning. */
export const RESPAWN_TICKS = 3 * TICK_RATE

/**
 * Ghosts (GHOSTS.md). A killed player comes back as a ghost after `RESPAWN_TICKS`, and stays one at
 * most this many ticks (30 s): then it takes shape where it is, or drops at an entry point if it may not.
 */
export const GHOST_TICKS = 1800

/** A ghost may choose to take shape only after this many ticks (5 s) as one. */
export const GHOST_MIN_TICKS = 300

/** Ticks a ghost takes to take shape again (2 s), seen by everyone around it. */
export const GHOST_FORM_TICKS = 120

/**
 * A ghost may not take shape within this far of a living enemy, metres along the ground: nobody
 * comes back right beside someone to shoot them in the back.
 */
export const GHOST_KEEPOUT = 40.0

/** How fast a ghost flies where it looks, metres per second. */
export const GHOST_SPEED = 20.0

/** How fast a ghost rises with jump held, or sinks with run held, metres per second. */
export const GHOST_CLIMB = 10.0

/** How quickly a ghost comes up to the velocity it wants, m/s². */
export const GHOST_ACCEL = 60.0

/**
 * A ghost's centre is kept at least this far over the ground, metres: it goes through walls and
 * roofs, but not under the earth.
 */
export const GHOST_FLOOR = 0.6

/** And at most this far over it, metres, as high as a drone flies. */
export const GHOST_CEILING = 150.0

/** How far over the body a ghost rises out of it, metres. */
export const GHOST_RISE = 1.0

/**
 * Why a ghost may not take shape where it is (`replicated::Vitals::block`): it may; it has not been one
 * for `GHOST_MIN_TICKS` yet; a living enemy is within `GHOST_KEEPOUT`; or the ground under it is no place
 * to stand (water, a building, outside the world).
 */
export const GHOST_BLOCK_NONE = 0
export const GHOST_BLOCK_SOON = 1
export const GHOST_BLOCK_ENEMY = 2
export const GHOST_BLOCK_GROUND = 3

/**
 * Guns are hitscans fired from the eye along the look direction; their stats are in `items`.
 * Height of a soldier's eye above its capsule centre, where a rifle shot leaves.
 */
export const EYE_HEIGHT = 0.6

/** How far a crouch lowers the eye. The capsule keeps its size, so collision is unchanged. */
export const CROUCH_EYE_DROP = 0.3

/** How far a crouch lowers the top of the hurtbox, with the head zone and the hips: as far as the eye. */
export const CROUCH_DROP = CROUCH_EYE_DROP

/**
 * Lying down, the hurtbox is a capsule on its side along the heading, the head at the front: this far
 * from the centre to the centre of either end, and this thick. 1.7 m long and under 0.8 m high.
 */
export const PRONE_HURT_HALF_LENGTH = 0.47
export const PRONE_HURT_RADIUS = 0.38

/** A whole day and night, seconds (an hour), and where in it (0 midnight, 0.5 noon) tick 0 falls. */
export const DAY_SECONDS = 3600.0
export const DAY_START_PHASE = 0.36

/** A prone soldier's eye is 0.2 m above its feet, so this is relative to the capsule centre. */
export const PRONE_EYE_HEIGHT = 0.2 - PRONE_CENTRE

/**
 * How high a field of wheat stands, metres (`Tile::crops`): over a crouched soldier's head and under
 * a standing one's shoulders, so the first is hidden in it and the second is not.
 */
export const CROP_HEIGHT = 1.3

/**
 * The on-foot camera sits behind a pivot this far to the right of the capsule centre and this far
 * above it. The server rebuilds the camera ray from it, so a shot lands where the crosshair is.
 * Mirrors `ON_FOOT_SHOULDER` and `ON_FOOT_HEIGHT` in the client.
 */
export const ON_FOOT_SHOULDER = 0.6
export const ON_FOOT_HEIGHT = 0.6

/**
 * Building with bricks (`ITEM_BRICKS`, `crate::construct`). Fire puts a cube where the crosshair ray
 * from the on-foot camera pivot meets the world, on the near side of the face it meets; aim (`BUTTON_ALT`)
 * with fire digs the placed cube there out instead. Neither reaches further than `BUILD_REACH` metres
 * from the eye to the cube's middle.
 */
export const BUILD_REACH = 6.0

/** Ticks between two cubes placed or dug by one player while fire is held. */
export const BUILD_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 6)

/**
 * Most cubes placed in the whole world at once: past it bricks place nothing until some are dug or
 * blasted. It bounds the memory, the catch-up a joiner gets and what is drawn.
 */
export const MAX_BUILT = 200000

/**
 * A piece of cubes cut off from what held it is looked through no further than this many cubes; a
 * bigger one is taken as held, so one blast never costs a long tick.
 */
export const BUILD_SUPPORT_SEARCH = 4096

/**
 * The seated capsule's centre is this far below the head point, and it spans `SEATED_HALF_HEIGHT`
 * above and below that, plus `SEATED_RADIUS`.
 */
export const SEATED_CENTRE_DROP = 0.3
export const SEATED_HALF_HEIGHT = 0.25
export const SEATED_RADIUS = 0.25

/** A round that hits a seated body above this far below its head point hit the head. */
export const SEATED_HEAD_ZONE = 0.12

/**
 * A passenger who can shoot looks over their own shoulder: the camera's pivot is this far to the
 * right of their head point and this far above it, and the camera sits `PASSENGER_DISTANCE` behind.
 * The server rebuilds the crosshair ray from it, as on foot.
 */
export const PASSENGER_SHOULDER = 0.6
export const PASSENGER_HEIGHT = 0.5
export const PASSENGER_DISTANCE = 3.0

/**
 * A hurt building is drawn in this many worse states. Stage `n` begins when its health falls to
 * `DAMAGE_STAGE_HEALTH[n - 1]` of full (percent); the last is about to collapse.
 */
export const DAMAGE_STAGES = 3
export const DAMAGE_STAGE_HEALTH = [66, 33, 15] as const

/** A blast hurting a building by at least this much leaves a scorch mark, and is announced. */
export const DAMAGE_MARK_MIN = 5

/**
 * Damage at the centre of an explosion; it falls off linearly to zero at `EXPLOSION_RADIUS`.
 * More than a full life, so a shell that lands within about 1.4 m kills outright.
 */
export const EXPLOSION_DAMAGE = 125.0
export const CANNON_DAMAGE = EXPLOSION_DAMAGE / 3.0

/** An armour-piercing shell's small bursting charge, which goes off behind the plate it gets through. */
export const AP_BLAST_RADIUS = 3.0
export const AP_BLAST_DAMAGE = 70.0
export const AP_BLAST_KNOCK = 8.0
export const CRATER_AP_RADIUS = 1.6
export const CRATER_AP_DEPTH = 0.3

/**
 * The knock a round gives the vehicle it strikes, where it strikes it (`Physics::strike_car`): the
 * change in the whole vehicle's speed, m/s, pushed at the point it met, so a hit high on a tall hull
 * rocks it. On top of what its burst does (the blast pushes from the middle). An armour-piercing
 * shell's is the hardest, solid steel at speed; then a cannonball's and a bomb's, a high-explosive
 * shell's, a rocket's and a mortar bomb's, and an autocannon's rounds'. Bullets give none.
 */
export const AP_HIT_KNOCK = 0.6
export const BALL_HIT_KNOCK = 0.5
export const BOMB_HIT_KNOCK = 0.5
export const SHELL_HIT_KNOCK = 0.4
export const ROCKET_HIT_KNOCK = 0.3
export const MORTAR_HIT_KNOCK = 0.3
export const CANNON_AP_HIT_KNOCK = 0.25
export const CANNON_HIT_KNOCK = 0.15

/**
 * How far round the point where an autocannon's armour-piercing round strikes a wall it punches the
 * wall out, metres: a cell or two (`carve::CELL`, 0.72 m) a round. Its little charge is too small to
 * bite a wall by itself (`CARVE_MIN_RADIUS` in `combat.rs`).
 */
export const CANNON_AP_WALL_HOLE = 0.85

/** The share of its knock a round that glances off gives. */
export const RICOCHET_KNOCK_SHARE = 0.3

/** Most shot events one snapshot carries; the rest are cosmetic and dropped. A shotgun pull is ten. */
export const MAX_SHOT_EVENTS = 24

/** Most hull marks (`S_MARKS`) a player is sent with one snapshot. */
export const MAX_HULL_MARKS = 32

/** Most bullet spawns one snapshot carries (nearest first); they are cheap, and are sent twice. */
export const MAX_BULLET_SPAWNS = 32

/** The rocket launcher: a projectile that flies straight and bursts on the first thing it meets. */
export const ROCKET_SPEED = 45.0
export const ROCKET_TTL_TICKS = 4 * TICK_RATE

/** How far in front of the eye a rocket starts. */
export const ROCKET_MUZZLE = 1.0
export const ROCKET_BLAST_RADIUS = 5.0
export const ROCKET_DAMAGE = 110.0

/** Knockback speed at the centre of a rocket's blast, m/s. */
export const ROCKET_KNOCK = 16.0

/** Rockets in flight at once, which bounds the projectile list in a snapshot. */
export const MAX_ROCKETS = 8

/** How long a destroyed vehicle burns before it is put back at its home, and the blast it goes up in. */
export const WRECK_TICKS = 10 * TICK_RATE

/** Angular speed (rad/s) a wreck is thrown into, about a horizontal axis: about one turn in the air. */
export const WRECK_SPIN = 6.5
export const WRECK_BLAST_RADIUS = 8.0
export const WRECK_BLAST_DAMAGE = 100.0
export const WRECK_BLAST_KNOCK = 14.0

/**
 * How far back the server will rewind other players to meet a shooter's view, in ticks (250 ms).
 * A shot never reaches further into the past than this, so a laggy shooter has a cap on how far
 * ahead of themselves they can lead their targets.
 */
export const MAX_REWIND_TICKS = 15

/** How far from home it wanders, metres. */
export const ROOSTER_WANDER_RADIUS = 8.0

/** Walking speed, m/s, and how fast it turns, rad/s. */
export const ROOSTER_SPEED = 1.2
export const ROOSTER_TURN_RATE = 4.0
export const ROOSTER_HP = 30

/** A killed rooster is back at home after this long. */
export const ROOSTER_RESPAWN_TICKS = 60 * TICK_RATE

/** Length of a crow; the client's `crow` clip is the same length. */
export const ROOSTER_CROW_TICKS = 100

/** Hit capsule: centre this high above its feet, metres. */
export const ROOSTER_CAPSULE_CENTRE = 0.4
export const ROOSTER_CAPSULE_HALF_HEIGHT = 0.1
export const ROOSTER_CAPSULE_RADIUS = 0.3

/**
 * Deer, placed in herds by the map editor (`animals` in area.json). They graze about their herd's
 * home, turn to look at someone who comes near and bolt from one who comes nearer, from gunfire and
 * blasts, and from being hurt; a herd bolts together. The stag lets people come closer.
 * How far from home a grazing herd drifts, metres.
 */
export const DEER_HERD_RADIUS = 14.0

/** Walking speed and galloping (fleeing) speed, m/s, and how fast it turns walking and galloping, rad/s. */
export const DEER_WALK_SPEED = 1.3
export const DEER_GALLOP_SPEED = 9.0
export const DEER_TURN_RATE = 2.5
export const DEER_FLEE_TURN_RATE = 5.0

/** How far it runs from what scared it, metres. */
export const DEER_FLEE_DISTANCE = 70.0

/**
 * Someone this close looks up from grazing, metres: walking, running, and crouched or prone and
 * still. Someone in a vehicle is noticed at the running distance plus a half.
 */
export const DEER_NOTICE_WALK = 24.0
export const DEER_NOTICE_RUN = 36.0
export const DEER_NOTICE_SNEAK = 8.0

/** Of the noticing distance: someone nearer than this share of it makes it bolt. */
export const DEER_BOLT_SHARE = 0.6

/** The stag's noticing distances are the hind's times this. */
export const STAG_NERVE = 0.7
export const DEER_HP = 60
export const STAG_HP = 90

/** A killed deer is back at its herd's home after this long. */
export const DEER_RESPAWN_TICKS = 90 * TICK_RATE

/**
 * Hit capsule: centre this high above its hooves, metres. Hurtboxes stand upright, so it is a fat one
 * that takes in the body from chest to rump, and the head held up.
 */
export const DEER_CAPSULE_CENTRE = 0.9
export const DEER_CAPSULE_HALF_HEIGHT = 0.25
export const DEER_CAPSULE_RADIUS = 0.55
export const STAG_CAPSULE_CENTRE = 1.05
export const STAG_CAPSULE_HALF_HEIGHT = 0.3
export const STAG_CAPSULE_RADIUS = 0.6

/**
 * Zombies (ZOMBIES.md): soldiers' bodies that rise in hordes at night and hunt the living by smell.
 * They walk as soldiers do, so they lose arms and legs as soldiers do, and crawl on with no legs;
 * only the head or the torso kills one.
 * How far a zombie smells the living, metres: it needs no line of sight.
 */
export const ZOMBIE_SMELL_RANGE = 80.0

/** Its shambling pace, slowest and fastest, as a forward axis value of 127: each zombie has its own. */
export const ZOMBIE_PACE = [55, 95] as const

/** Within this of its prey, metres, it lunges at full walking pace. */
export const ZOMBIE_LUNGE = 5.0

/** One zombie in this many runs instead of shambling. */
export const ZOMBIE_RUNNER_ONE_IN = 8

/** It claws at someone within this of its body's centre, metres. */
export const ZOMBIE_CLAW_REACH = 1.4

/** A claw: ticks of reaching back before it lands, and of recovering after. */
export const ZOMBIE_CLAW_WINDUP = 24
export const ZOMBIE_CLAW_RECOVER = 30

/** The harm a claw does, and a bite (a zombie with no arms left bites instead). */
export const ZOMBIE_CLAW_DAMAGE = 22
export const ZOMBIE_BITE_DAMAGE = 14

/**
 * Ticks a zombie takes to claw its way out of the ground (6 s at `TICK_RATE`): a hand first, then the
 * head, then it heaves itself out and stands. It does nothing else meanwhile, but can be shot.
 */
export const ZOMBIE_RISE_TICKS = 360

/** The zombies of a horde come up one by one over this many ticks (10 s), each at a moment of its own. */
export const ZOMBIE_RISE_STAGGER = 600

/** The most zombies up at once in a room, whatever a scenario asks for. */
export const MAX_ZOMBIES = 96

/**
 * The zombies' side: they never hurt each other, and everyone else is their prey. Their profiles say
 * so, for the client to keep them off the scoreboard.
 */
export const TEAM_ZOMBIES = 255

/**
 * Golf (`ITEM_DRIVER`, `ITEM_PUTTER`, `PROJECTILE_GOLF_BALL`). A ball falls at this share of
 * `GRAVITY`, which is twice the earth's: a ball wants the real thing to carry like one.
 */
export const GOLF_GRAVITY_SCALE = 0.5
export const GOLF_BALL_RADIUS = 0.05

/**
 * A full swing's speed off the club, m/s, and how far above level it leaves, radians: the driver's
 * own, and with aim held, a high pitch that stops quickly.
 */
export const GOLF_DRIVER_SPEED = 70.0
export const GOLF_DRIVER_LOFT = 0.21
export const GOLF_DRIVER_HIGH_LOFT = 0.75
export const GOLF_PUTTER_SPEED = 6.0

/**
 * The swing meter (`golf::Meter`), counted in input frames, one a tick. Held fire fills it to a full
 * swing in `GOLF_CHARGE_TICKS`, and on into an overswing to `GOLF_OVERSWING`, where it lets go by
 * itself. Let go, a marker runs back across it, a full bar in `GOLF_RETURN_FRAMES`, to the line.
 */
export const GOLF_CHARGE_TICKS = Math.trunc(TICK_RATE * 6 / 5)
export const GOLF_OVERSWING = 1.1
export const GOLF_RETURN_FRAMES = Math.trunc(TICK_RATE * 9 / 10)

/**
 * Fire pressed again this many frames either side of the line strikes the ball dead straight; further
 * off it curves, early to the left and late to the right, fully `GOLF_CURVE_FRAMES` beyond that; more
 * than `GOLF_MISS_FRAMES` off (or never) is a mishit, topped, with `GOLF_MISHIT_POWER` of the power.
 */
export const GOLF_PURE_FRAMES = 1
export const GOLF_CURVE_FRAMES = 6
export const GOLF_MISS_FRAMES = 9
export const GOLF_MISHIT_POWER = 0.4

/** Frames after a strike before the meter can be started again. */
export const GOLF_REST_FRAMES = TICK_RATE

/**
 * A fully curved ball's sideways pull in the air, m/s² for each m/s it flies at; a putt has no air to
 * curve in, so it is pulled or pushed off line by this much, radians.
 */
export const GOLF_SPIN = 0.12
export const GOLF_PUTT_PULL = 0.1

/** A ball this far (metres, along the ground) from the golfer's feet, ahead of them, is in reach. */
export const GOLF_REACH = 1.6

/** How much of its speed into a surface a ball keeps bouncing off it, and of its speed along it. */
export const GOLF_BOUNCE = 0.35
export const GOLF_SKID = 0.75

/** Rolling friction, of the ball's weight: on a green, a fairway or tee, a bunker, and anywhere else. */
export const GOLF_ROLL_GREEN = 0.065
export const GOLF_ROLL_FAIRWAY = 0.16
export const GOLF_ROLL_BUNKER = 2.0
export const GOLF_ROLL_ROUGH = 0.4

/** Slower than this (m/s) on a slope gentler than its friction holds, a rolling ball stops. */
export const GOLF_REST_SPEED = 0.08

/** A ball within this of the cup's middle (metres) going slower than `GOLF_CUP_SPEED` (m/s) drops in. */
export const GOLF_CUP_RADIUS = 0.16
export const GOLF_CUP_SPEED = 1.8

/** A ball nobody has struck for this long is gone; another waits on its tee. */
export const GOLF_BALL_TTL_TICKS = 300 * TICK_RATE

/** Golf balls in the world at once. */
export const MAX_GOLF_BALLS = 32

/**
 * Speed of a ski lift's cable and its chairs, m/s: quicker than a real chair lift, so a ride up a
 * hill takes minutes, not a quarter of an hour (LIFTS.md).
 */
export const LIFT_SPEED = 4.0

/** Distance between chairs along the cable, metres, about: a lift's loop holds a whole number of them. */
export const LIFT_CHAIR_SPACING = 40.0

/** Distance between the cable going up and the one coming down, metres: the bull wheel's diameter. */
export const LIFT_GAUGE = 4.4

/** How far a chair's seat hangs under the cable, metres. */
export const LIFT_CHAIR_DROP = 2.6

/** Height of the cable over the ground at a station, metres: the seat is at a rider's knees there. */
export const LIFT_STATION_CABLE = 3.1

/** Height of the cable over the ground at a pylon, metres, at the least, and at the most. */
export const LIFT_PYLON_CABLE = 9.0
export const LIFT_PYLON_MAX = 30.0

/**
 * Height a chair's seat keeps over the ground between the pylons, metres, at the least, once it is
 * clear of the stations.
 */
export const LIFT_CLEARANCE = 2.5

/** Longest span between two pylons, metres: a longer one gets pylons put in. */
export const LIFT_SPAN = 220.0

/** Width of one rider's place on a chair, metres. */
export const LIFT_SEAT_PITCH = 0.6

/** Riders a chair carries when OpenStreetMap does not say, and the most it carries. */
export const LIFT_SEATS = 2
export const LIFT_MAX_SEATS = 4

/**
 * A soldier this near a chair's seat (metres, across the ground), where the seat is no higher
 * than `LIFT_BOARD_HEIGHT` (metres) over the ground, can sit down on it: only at the stations.
 */
export const LIFT_BOARD_RADIUS = 2.2
export const LIFT_BOARD_HEIGHT = 1.3

/** Segments of the half circle a chair goes round a station's bull wheel on. */
export const LIFT_WHEEL_SEGMENTS = 8

/**
 * Height of a riding soldier's capsule centre over the chair's seat, metres: the legs hang below it,
 * and the eye (crouched) is a sitting one's.
 */
export const LIFT_RIDER_CENTRE = 0.45

/**
 * The air ambulance (`KIND_AIR_AMBULANCE`): a light twin-engined helicopter after the EC135, with
 * skids, a shrouded tail fan and no weapons. The pilot sits front right, as helicopter pilots do, with
 * one passenger beside them and two behind. It flies as the gunship does (`Lift::Rotor`), faster and
 * climbing slower. Same body axes as the car.
 */
export const AIR_AMBULANCE_SEATS = 4

/** Half the box round the cabin, from the skids' soles to the top of the windows, metres. */
export const AIR_AMBULANCE_HALF_EXTENTS = [0.85, 1.1, 2.55] as const

/** Half the engines' housing on the cabin's roof, and its middle in the hull frame, metres. */
export const AIR_AMBULANCE_HOUSING_HALF = [0.55, 0.3, 1.3] as const
export const AIR_AMBULANCE_HOUSING_CENTRE = [0.0, 1.4, 1.1] as const

/**
 * Half the tail boom and its middle, then half the fin round the fan and its middle, metres: the tail
 * reaches five metres past the cabin's box.
 */
export const AIR_AMBULANCE_BOOM_HALF = [0.18, 0.25, 1.7] as const
export const AIR_AMBULANCE_BOOM_CENTRE = [0.0, 0.68, 4.2] as const
export const AIR_AMBULANCE_FIN_HALF = [0.16, 1.2, 0.8] as const
export const AIR_AMBULANCE_FIN_CENTRE = [0.0, 1.05, 6.7] as const

/** The main rotor's hub in the hull frame and its radius, then the tail fan's hub, metres. */
export const AIR_AMBULANCE_ROTOR_HUB = [0.0, 2.1, 0.7] as const
export const AIR_AMBULANCE_ROTOR_RADIUS = 5.1
export const AIR_AMBULANCE_FAN_HUB = [0.0, 0.65, 6.55] as const

/** The lamp under the nose that is its searchlight, in the hull frame, metres. */
export const AIR_AMBULANCE_LAMP = [0.0, -0.5, -2.1] as const

/**
 * The lever that hovers it out of ground effect, measured by
 * `the_air_ambulances_hover_lever_holds_it_out_of_ground_effect`.
 */
export const AIR_AMBULANCE_HOVER_LEVER = 0.56

/** The pilot front right and a passenger front left, then two passengers behind them. */
export const AIR_AMBULANCE_SEAT_HEADS = [[0.31, 0.72, -0.85], [-0.31, 0.72, -0.85], [-0.31, 0.74, 0.35], [0.31, 0.74, 0.35]] as const

/**
 * The ambulance (`KIND_AMBULANCE`): a Volkswagen Transporter T3 in white and yellow with a red
 * stripe and a light bar on its roof. Not the air ambulance (`KIND_AIR_AMBULANCE`), which flies. The
 * same body, weight and springs as the Transporter (`transporter.rs`) and its seven seats behind
 * glass: the driver and a passenger over the front wheels, then two by the sliding door and three at
 * the back. A little faster flat out than the van it is built on.
 */
export const AMBULANCE_SEATS = 7
export const AMBULANCE_HALF_EXTENTS = [0.92, 0.5, 2.25] as const
export const AMBULANCE_WHEEL_RADIUS = 0.33
export const AMBULANCE_TRACK_HALF = 0.785
export const AMBULANCE_AXLE_Z = 1.23
export const AMBULANCE_HARD_POINT_Y = 0.0
export const AMBULANCE_SUSPENSION_REST = 0.486

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const AMBULANCE_RIDE_HEIGHT = 0.685
export const AMBULANCE_MAX_STEER = 0.6

/** Flat out, m/s: a little more than the Transporter's. */
export const AMBULANCE_MAX_SPEED = 34.0

/** The driver on the left, then the passengers, front to back. */
export const AMBULANCE_SEAT_HEADS = [[-0.45, 0.87, -1.25], [0.45, 0.87, -1.25], [-0.4, 0.87, 0.05], [0.4, 0.87, 0.05], [-0.5, 0.87, 1.15], [0.0, 0.87, 1.15], [0.5, 0.87, 1.15]] as const

/**
 * The AMV XA-361 (`KIND_AMV`): an eight-wheeled armoured vehicle with a turret on its back carrying
 * two 120 mm mortar tubes side by side, which lob their bombs as the field mortar does: the range dial
 * sets how far, and the tubes rise to the high lob that lands them there. They are shorter-ranged than
 * the field mortar's (200 m against 300) and burst a little smaller, but fire two in quick succession,
 * after which the turret takes three seconds to load both tubes again. The driver (seat 0) sits at the
 * front left beside the engine, the mortar operator (seat 1) under the turret, and one rider (seat 2)
 * in the back, all sealed in; they get out by the rear door. It floats, and two water jets at the
 * back push it slowly across open water. Same body axes as a car, and true to size:
 * 7.7 m long, 2.8 m wide and 2.3 m to the hull roof, 3 m to the turret's top.
 */
export const AMV_SEATS = 3

/**
 * Half the hull's box, metres: 7.7 m long, 2.8 m wide and 1.85 m deep, its belly about 0.45 m off the
 * ground.
 */
export const AMV_HALF_EXTENTS = [1.4, 0.925, 3.85] as const

/** Wheel radius, metres: tall run-flat tyres, 1.2 m across. */
export const AMV_WHEEL_RADIUS = 0.6

/** How far each wheel is from the middle, metres. */
export const AMV_TRACK_HALF = 1.12

/** Z of the four axles, front first, metres: two pairs, 1.5 m apart within a pair. */
export const AMV_AXLES_Z = [-2.35, -0.85, 0.85, 2.35] as const
export const AMV_HARD_POINT_Y = -0.3
export const AMV_SUSPENSION_REST = 0.5

/** Where the hull's centre settles over flat ground, metres: measured by `ride_heights_differ_between_cars`. */
export const AMV_RIDE_HEIGHT = 1.35

/** How far the front axle's wheels turn at a standstill, radians. */
export const AMV_MAX_STEER = 0.5

/** How many axles from the front steer: the front pair, the second turning less than the first. */
export const AMV_STEER_AXLES = 2

/** Top speed on the flat, m/s (90 km/h). */
export const AMV_MAX_SPEED = 25.0

/**
 * How deep its hull floats, metres: 24 tonnes on its 2.8 × 7.7 m footprint, so the roof is 0.65 m clear
 * of the water and the turret's top 1.35 m.
 */
export const AMV_DRAFT = 1.2

/**
 * Top speed afloat, m/s (10 km/h), how hard its two water jets push it there, m/s², and its yaw rate at
 * full stick, rad/s: a long hull, slower to swing round than the Pbv 302's.
 */
export const AMV_SWIM_SPEED = 2.8
export const AMV_SWIM_THRUST = 2.0
export const AMV_SWIM_TURN = 0.35

/**
 * The turret's solid: a low drum on the hull roof behind the middle, 2.2 m across, its top 3 m off the
 * ground, metres. It is part of the hull: it does not come off.
 */
export const AMV_TURRET_RING = [0.0, 1.275, 0.9] as const
export const AMV_TURRET_RADIUS = 1.1
export const AMV_TURRET_HALF_HEIGHT = 0.35

/**
 * Where the tubes turn and elevate, in the hull frame: on the turret's trunnions, a little ahead of
 * the ring's middle.
 */
export const AMV_GUN_PIVOT = [0.0, 1.45, 0.7] as const

/** How far apart the two tubes are either side of the middle, metres, and how long they are. */
export const AMV_TUBE_X = 0.3
export const AMV_BARREL_LENGTH = 3.0

/**
 * The range dial, metres: the longest lob is 200 m (`GRAVITY` and the muzzle speed at 45 degrees),
 * and the steepest the tubes stand reaches about 35 m, so the dial stops at 40.
 */
export const AMV_RANGE_MIN = 40.0
export const AMV_RANGE_MAX = 200.0
export const AMV_RANGE_START = 100.0
export const AMV_RANGE_RATE = 50.0

/** Muzzle speed of its bomb, m/s: under `GRAVITY` it reaches 200 m at 45 degrees. */
export const AMV_SHELL_SPEED = 63.25

/**
 * How fast the turret follows the operator's look, rad/s, and the tubes the range, rad/s: powered,
 * so far quicker than the field mortar's crank.
 */
export const AMV_TRAVERSE_SLEW = 1.0
export const AMV_ELEVATION_SLEW = 0.6

/** Ticks between the two rounds of a pair: a third of a second. */
export const AMV_COOLDOWN_TICKS = 20

/** Rounds in the tubes when loaded: one in each. */
export const AMV_ROUNDS = 2

/** Ticks after the last round before both tubes are loaded again: three seconds. */
export const AMV_RELOAD_TICKS = 3 * TICK_RATE

/** Speed the hull is shoved back at when a tube fires, m/s: hardly anything, on 24 tonnes. */
export const AMV_RECOIL = 0.3

/**
 * Blast of its bomb: a little smaller and weaker than the field mortar's (`MORTAR_RADIUS`,
 * `MORTAR_DAMAGE`, `MORTAR_KNOCK`), and the crater it digs, metres.
 */
export const AMV_BOMB_RADIUS = 9.5
export const AMV_BOMB_DAMAGE = 210.0
export const AMV_BOMB_KNOCK = 25.0
export const AMV_BOMB_CRATER = [7.5, 1.3] as const

/**
 * Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
 * rear door.
 */
export const AMV_DOOR_EXIT = [0.0, -0.4, 4.6] as const

/**
 * The bus (`KIND_BUS`): a twelve-metre city bus on two axles, with a driver at the front and twelve
 * passengers in six rows of window seats behind. It is heavy, slow to get going and slow flat out,
 * and it needs most of a road to turn. On a world with bus lines the server drives them
 * (`Buses`); one whose driver is gone drives like any other car.
 */
export const BUS_SEATS = 13

/** Half the frame under the floor, metres: the body above it is a solid of its own (`BUS_BODY_HALF`). */
export const BUS_HALF_EXTENTS = [1.25, 0.5, 6.0] as const

/**
 * Half the body over the frame, and its middle in the hull frame, metres: the roof is 2.3 m over the
 * hull's middle.
 */
export const BUS_BODY_HALF = [1.25, 0.9, 6.0] as const
export const BUS_BODY_CENTRE = [0.0, 1.4, 0.0] as const
export const BUS_WHEEL_RADIUS = 0.5
export const BUS_TRACK_HALF = 1.05

/** The axles are six metres apart, with three metres of bus past each. */
export const BUS_AXLE_Z = 3.0
export const BUS_HARD_POINT_Y = -0.2
export const BUS_SUSPENSION_REST = 0.45

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const BUS_RIDE_HEIGHT = 1.057

/** A sharp lock for so long a wheelbase: it turns in a circle of about nine metres' radius. */
export const BUS_MAX_STEER = 0.6

/** About 58 km/h. */
export const BUS_MAX_SPEED = 16.0

/**
 * Where it is boarded from, in the hull frame: its front door, on the right behind the front axle's
 * overhang.
 */
export const BUS_DOOR = [1.25, 0.0, -4.6] as const

/**
 * Where a rider who gets off stands, in the hull frame (the middle of a standing soldier): on the
 * kerb outside the front door.
 */
export const BUS_DOOR_EXIT = [2.1, -0.1, -4.6] as const

/**
 * The Stridsbåt 90 (`KIND_CB90`): the Swedish fast assault craft, true to size, 15.9 m long and 3.8 m
 * across, pushed by two waterjets to 40 knots. It floats only on open water, planes at speed, banks
 * into its turns and stops in a few lengths with its reversing buckets. The driver (seat 0) sits in the
 * low cockpit forward and works the twin Ksp 58 machine guns fixed on its front, which only elevate:
 * the driver aims them sideways by turning the boat, as a Strv S's. Behind the cockpit the gunner
 * (seat 1) stands in an open ring on the troop compartment's roof, out in the open, and turns the 25 mm
 * automatic cannon all the way round. The troop compartment is down in the hull, its roof a little over
 * the gunwale, and behind it two riders (seats 2 and 3) sit on the open after deck, firing their own
 * guns. Its bow tapers to a point over `CB90_BOW`.
 */
export const CB90_SEATS = 4

/** Half the hull's box, metres: 3.8 m across, 2 m from the keel to the deck and 15.9 m long. */
export const CB90_HALF_EXTENTS = [1.9, 1.0, 7.95] as const

/** How far back from the stem the sides taper out to the full beam, metres. */
export const CB90_BOW = 4.5

/**
 * How deep the hull floats at rest, metres: the deck stands this much less than its height out of the
 * water.
 */
export const CB90_DRAFT = 0.8

/** Top speed ahead on calm water, m/s (45 knots). */
export const CB90_MAX_SPEED = 23.0

/** How hard the waterjets push it ahead from a standstill, m/s²: up to 30 knots in under five seconds. */
export const CB90_THRUST = 3.6

/**
 * How hard the reversing buckets stop it while it still runs ahead, m/s²: from full speed inside two
 * lengths, as the real boat's crash stop does.
 */
export const CB90_BRAKE = 10.0

/**
 * In a crash stop its bow digs in, this far down, radians, and its hull squats this much deeper into
 * the water, metres, both most from `CB90_PLANE_SPEED`'s top and gone by the time it has stopped.
 */
export const CB90_BRAKE_DIP = 0.13
export const CB90_BRAKE_SQUAT = 0.35

/** Top speed astern, m/s. */
export const CB90_ASTERN_SPEED = 4.0

/** Yaw rate at full helm under way, rad/s: a turn about 100 m across at full speed. */
export const CB90_TURN_RATE = 0.45

/** Yaw rate at full helm standing still, rad/s: the jets swing it round on the spot. */
export const CB90_SPIN_RATE = 0.3

/**
 * How far its middle rises out of the water once it planes, metres, and the speeds over which it
 * climbs onto the plane, m/s.
 */
export const CB90_PLANE_LIFT = 0.35
export const CB90_PLANE_SPEED = [6.0, 14.0] as const

/**
 * How far its bow lifts as it climbs onto the plane (most at the bottom of `CB90_PLANE_SPEED`) and how
 * far it stays lifted planing, radians.
 */
export const CB90_BOW_RISE = 0.07
export const CB90_BOW_PLANING = 0.025

/** How far it banks into a turn, radians per m/s² of turning, and at most, radians. */
export const CB90_BANK = 0.022
export const CB90_BANK_MAX = 0.2

/** The deck, in the hull frame. */
export const CB90_DECK_Y = CB90_HALF_EXTENTS[1]

/**
 * The cockpit, low and forward, where the driver sits: half its box, metres, and where its middle is.
 * Its roof is 1.15 m over the deck.
 */
export const CB90_COCKPIT_HALF = [1.4, 0.575, 1.45] as const
export const CB90_COCKPIT_AT = [0.0, 1.575, -3.15] as const

/**
 * The troop compartment's roof behind it, 0.35 m over the deck, down to the after deck: half its box,
 * metres, and where its middle is.
 */
export const CB90_TROOP_HALF = [1.6, 0.175, 2.15] as const
export const CB90_TROOP_AT = [0.0, 1.175, 0.45] as const

/**
 * The gunner's ring on that roof, just behind the cockpit: where its middle is, its radius and half its
 * height, metres.
 */
export const CB90_RING_AT = [0.0, 1.55, -0.9] as const
export const CB90_RING_RADIUS = 0.75
export const CB90_RING_HALF_HEIGHT = 0.2

/**
 * The cannon on its pedestal in the ring: where it turns and elevates, high enough to fire over the
 * cockpit and its machine guns.
 */
export const CB90_CANNON_PIVOT = [0.0, 2.75, -0.9] as const

/**
 * The gunner's head from the pivot, with the gun pointing ahead: standing in the ring's hatch behind the
 * gun, on the troop compartment's floor.
 */
export const CB90_GUNNER_HEAD = [0.0, -0.55, 0.7] as const

/**
 * The cannon: how far its muzzle is from the pivot, metres, its elevation, radians (+ is up), which
 * keeps its rounds over the cockpit, and how fast it follows the gunner's look, rad/s.
 */
export const CB90_CANNON_LENGTH = 2.3
export const CB90_CANNON_PITCH_MIN = -0.08
export const CB90_CANNON_PITCH_MAX = 0.9
export const CB90_CANNON_SLEW = 1.8

/** Ticks between its rounds: seven and a half a second. */
export const CB90_CANNON_COOLDOWN_TICKS = 8
export const CB90_CANNON_SPREAD = 0.005
export const CB90_CANNON_RANGE = 500.0

/** What a 25 mm round takes off a soldier, a vehicle's hull and an aircraft. */
export const CB90_CANNON_DAMAGE = 50
export const CB90_CANNON_VEHICLE_DAMAGE = 12
export const CB90_CANNON_PLANE_DAMAGE = 24

/** Heat a round puts in the barrel, of 255: about five seconds of holding the trigger jams it. */
export const CB90_CANNON_HEAT = 15.0

/**
 * The twin machine guns fixed on the front of the cockpit's roof, firing ahead over the bow: where they
 * elevate, how far either side of it each barrel is, metres, how far they tilt down and up, radians
 * (+ is up), and how fast, rad/s. They do not turn: the boat aims them.
 */
export const CB90_MG_PIVOT = [0.0, 2.4, -3.9] as const
export const CB90_MG_BARREL_X = 0.16
export const CB90_MG_LENGTH = 1.1
export const CB90_MG_PITCH_MIN = -0.15
export const CB90_MG_PITCH_MAX = 0.2
export const CB90_MG_SLEW = 0.8

/** Ticks between rounds, the two barrels taking turns: twenty a second. */
export const CB90_MG_COOLDOWN_TICKS = 3
export const CB90_MG_SPREAD = 0.01
export const CB90_MG_RANGE = 350.0

/** What a 7.62 mm round takes off a soldier, a vehicle's hull and an aircraft. */
export const CB90_MG_DAMAGE = 14
export const CB90_MG_VEHICLE_DAMAGE = 3
export const CB90_MG_PLANE_DAMAGE = 6

/** Heat a round puts in the barrels, of 255: about five seconds of holding the trigger jams them. */
export const CB90_MG_HEAT = 5.5

/**
 * Where the driver's reticle camera sits behind and above the machine guns, metres: over the after
 * deck, looking down the boat over the gunner's head.
 */
export const CB90_AIM_BACK = 7.0
export const CB90_AIM_UP = 2.3

/**
 * The driver in the cockpit, the gunner as they stand with the cannon ahead (`CB90_CANNON_PIVOT` plus
 * `CB90_GUNNER_HEAD`), then the two riders on the after deck.
 */
export const CB90_SEAT_HEADS = [[-0.5, 1.95, -2.9], [0.0, 2.2, -0.2], [-0.9, 2.3, 4.6], [0.9, 2.3, 4.6]] as const

/**
 * Where it is boarded from, on the after deck, and where whoever leaves it stands (the middle of a
 * standing soldier), a little further aft.
 */
export const CB90_BOARD = [0.0, 1.0, 5.6] as const
export const CB90_EXIT = [0.0, CB90_DECK_Y + STAND_CENTRE + 0.05, 6.5] as const

/**
 * The FPV drone: a quadcopter the size of a dinner plate. It is flown by velocity: forward stick asks
 * for `DRONE_SPEED` along where its pilot looks (diving or climbing with the look's pitch), strafe for
 * `DRONE_STRAFE_SPEED` sideways, jump and run for `DRONE_CLIMB_SPEED` / `DRONE_SINK_SPEED` straight up
 * or down, and nothing held for a hover. The rotors close the gap at `DRONE_ACCEL_GAIN` per second, never
 * pushing harder than `DRONE_MAX_ACCEL` m/s² beyond holding it up, and the hull leans the way they push.
 * The look turns it: `DRONE_YAW_GAIN` rad/s per radian off, at most `DRONE_YAW_RATE`.
 */
export const DRONE_SEATS = 1
export const DRONE_HALF_EXTENTS = [0.22, 0.06, 0.22] as const
export const DRONE_SPEED = 30.0
export const DRONE_STRAFE_SPEED = 12.0
export const DRONE_CLIMB_SPEED = 10.0
export const DRONE_SINK_SPEED = 10.0
export const DRONE_ACCEL_GAIN = 2.0
export const DRONE_MAX_ACCEL = 18.0
export const DRONE_YAW_GAIN = 6.0
export const DRONE_YAW_RATE = 4.0

/** Climbing fades out over the last 20 m below this height, and above it the rotors push down. */
export const DRONE_CEILING = 150.0

/**
 * How firmly the hull follows its lean (rad/s per unit of error), and how fast the angular velocity
 * follows that, per second.
 */
export const DRONE_LEVEL_GAIN = 8.0
export const DRONE_RESPONSE = 14.0

/** Air drag on the body, which is all that slows a drone whose motors are off. */
export const DRONE_DAMPING = 0.1

/**
 * The EPA traktor (`KIND_EPA`): an old boxy saloon cut down into a two-seat pickup, as Swedish teenagers
 * drive them. The back doors are welded shut and the roof ends behind the front seats, where a wall with
 * a window and a slow-vehicle triangle closes the cab off from a short bed. Its gearbox is locked so that
 * it can go no faster than 30 km/h, but it keeps the saloon's big engine, so it gets there briskly. A
 * driver and a passenger sit behind glass.
 */
export const EPA_SEATS = 2
export const EPA_HALF_EXTENTS = [0.87, 0.3, 2.38] as const

/** Low-profile tyres on big aftermarket rims, metres. */
export const EPA_WHEEL_RADIUS = 0.32
export const EPA_TRACK_HALF = 0.74
export const EPA_AXLE_Z = 1.385
export const EPA_HARD_POINT_Y = 0.0
export const EPA_SUSPENSION_REST = 0.22

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const EPA_RIDE_HEIGHT = 0.447
export const EPA_MAX_STEER = 0.55

/** 30 km/h, the most the law lets an EPA traktor do, m/s. */
export const EPA_MAX_SPEED = 8.33

/** The driver on the left and the passenger beside them, under the front of the roof. */
export const EPA_SEAT_HEADS = [[-0.38, 0.75, 0.1], [0.38, 0.75, 0.1]] as const

/**
 * The Ford Escort Mk3 (`KIND_ESCORT`): a small hatchback with a bustle at its back, cheap, light
 * and slow. Five seats behind glass.
 */
export const ESCORT_SEATS = 5
export const ESCORT_HALF_EXTENTS = [0.82, 0.3, 2.0] as const
export const ESCORT_WHEEL_RADIUS = 0.28
export const ESCORT_TRACK_HALF = 0.7
export const ESCORT_AXLE_Z = 1.2
export const ESCORT_HARD_POINT_Y = 0.0
export const ESCORT_SUSPENSION_REST = 0.258

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const ESCORT_RIDE_HEIGHT = 0.445
export const ESCORT_MAX_STEER = 0.55

/** Slow flat out, m/s. */
export const ESCORT_MAX_SPEED = 35.0

/** The driver on the left, then the passengers, front to back. */
export const ESCORT_SEAT_HEADS = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75], [0.0, 0.71, 0.75]] as const

/**
 * The golf cart (`KIND_GOLF_CART`): two seats side by side under a canopy roof, small wheels and an
 * electric motor. Light and slow, it potters between the holes at the speed of a brisk cycle ride,
 * with a bag rack on the back.
 */
export const GOLF_CART_SEATS = 2
export const GOLF_CART_HALF_EXTENTS = [0.6, 0.28, 1.2] as const
export const GOLF_CART_WHEEL_RADIUS = 0.23
export const GOLF_CART_TRACK_HALF = 0.5
export const GOLF_CART_AXLE_Z = 0.82
export const GOLF_CART_HARD_POINT_Y = -0.06
export const GOLF_CART_SUSPENSION_REST = 0.2

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const GOLF_CART_RIDE_HEIGHT = 0.397
export const GOLF_CART_MAX_STEER = 0.55

/** About 43 km/h, which is quick for a golf cart. */
export const GOLF_CART_MAX_SPEED = 12.0
export const GOLF_CART_SEAT_HEADS = [[-0.28, 0.82, 0.25], [0.28, 0.82, 0.25]] as const

/**
 * The Volkswagen Golf GTI Mk2 (`KIND_GTI`): a short, light three-door hot hatch. The quickest of the
 * everyday cars off the line and darting through traffic, though a Saab turbo walks away from it
 * flat out. Four seats behind glass.
 */
export const GTI_SEATS = 4
export const GTI_HALF_EXTENTS = [0.84, 0.3, 2.0] as const
export const GTI_WHEEL_RADIUS = 0.29
export const GTI_TRACK_HALF = 0.715
export const GTI_AXLE_Z = 1.235
export const GTI_HARD_POINT_Y = 0.0
export const GTI_SUSPENSION_REST = 0.248

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const GTI_RIDE_HEIGHT = 0.445
export const GTI_MAX_STEER = 0.55

/** Flat out, m/s. */
export const GTI_MAX_SPEED = 44.0

/** The driver on the left, then the passengers, front to back. */
export const GTI_SEAT_HEADS = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75]] as const

/**
 * The helicopter: a hull flown by a pilot (seat 0) with a gunner (seat 1) on the nose gun. There are no
 * wheels; the rotor is a thrust along the hull's up axis, the cyclic leans the hull to go anywhere,
 * the pedals turn it towards where the pilot looks and the collective lever sets the lift. Same body
 * axes as the car.
 */
export const HELI_SEATS = 2
export const HELI_HALF_EXTENTS = [1.0, 0.8, 2.4] as const

/** How fast jump and run move the pilot's collective lever, share of its travel per second. */
export const HELI_LEVER_RATE = 0.6

/**
 * The lever that hovers a fully crewed helicopter out of ground effect, measured by
 * `the_hover_lever_holds_it_steady_out_of_ground_effect`. A pilot who takes the seat in the air starts on it.
 */
export const HELI_HOVER_LEVER = 0.53

/**
 * The thinning air takes the engine's power away over the last 40 m below this height above the
 * terrain, until it only just hovers there. Every flier's ceiling is measured from the terrain under it,
 * not from y = 0.
 */
export const HELI_CEILING = 120.0

/** Under this share of its governed speed the rotor is low, and the pilot is warned. */
export const HELI_LOW_ROTOR = 0.9

/**
 * Through the air at about this speed, m/s, the rotor flies into clean air (translational lift), and
 * the airframe shudders on the way.
 */
export const HELI_ETL_SPEED = 10.0

/**
 * The nose gun: a four-barrel autocannon on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
 * frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
 * `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
 * hull's frame), at `HELI_GUN_SLEW` rad/s. It fires a shell (`SHELL_SPEED`, same fall and lifetime) every
 * `HELI_GUN_COOLDOWN_TICKS`, three a second, scattered by up to `HELI_GUN_SPREAD` radians. It bursts
 * like a tank shell on a smaller scale: a third of the radius and a third of the damage.
 */
export const HELI_GUN_PIVOT = [0.0, -0.6, -2.0] as const
export const HELI_GUN_LENGTH = 1.3
export const HELI_GUN_YAW_LIMIT = 1.0
export const HELI_GUN_PITCH_MIN = -0.75
export const HELI_GUN_PITCH_MAX = 0.2
export const HELI_GUN_SLEW = 3.0
export const HELI_GUN_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 3)
export const HELI_GUN_SPREAD = 0.01

/**
 * The rocket pods under the stub wings, fired by the pilot (seat 0) on the fire button. `HELI_POD_PIVOTS`
 * are the four tube mouths in the hull frame, used in turn, one rocket every `HELI_ROCKET_COOLDOWN_TICKS`.
 * A rocket leaves at `HELI_ROCKET_LAUNCH_SPEED` over the helicopter's own velocity, along the nose, and
 * is driven at `ROCKET_ACCEL` m/s² up to `ROCKET_MAX_SPEED`: it flies straight, no gravity.
 */
export const HELI_POD_PIVOTS = [[-1.0, -0.39, -0.3], [1.0, -0.39, -0.3], [-1.75, -0.525, -0.3], [1.75, -0.525, -0.3]] as const
export const HELI_ROCKET_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 5)
export const HELI_ROCKET_LAUNCH_SPEED = 20.0

/** The pilot raised behind and above the gunner in the nose. */
export const HELI_SEAT_HEADS = [[0.0, 0.45, -0.45], [0.0, 0.1, -1.55]] as const

/**
 * The hen (`KIND_HEN`): a giant robot hen on the walker's legs, one driver in a cockpit on its back. It
 * walks and balances exactly as the walker does (the `WALKER_*` legs, spring and balance), and adds
 * wings: a leap off the ground (Space), flapping that holds a slow climb while Space stays held in the
 * air, and a glide that holds its fall to a slow sink once the flapping has run out. Jumping and
 * flapping drain the wings' stamina, which comes back while it stands with Space let go.
 */
export const HEN_SEATS = 1

/** The wings' stamina, as a share of full. */
export const HEN_STAMINA = 1.0

/**
 * Upward speed a leap adds, m/s (about a 6 m leap), plus this much along its heading at full stick
 * (half of it backwards), and the stamina it costs.
 */
export const HEN_JUMP_SPEED = 11.0
export const HEN_JUMP_FORWARD = 6.0
export const HEN_JUMP_COST = 0.2

/**
 * Flapping, once the leap has topped out, holds this upward speed, m/s, chasing it this hard (per
 * second) by at most this much on top of gravity's worth, m/s², and uses this share of the stamina
 * per second: about three seconds of it.
 */
export const HEN_FLAP_CLIMB = 1.5
export const HEN_FLAP_GAIN = 3.0
export const HEN_FLAP_ACCEL = 6.0
export const HEN_FLAP_DRAIN = 0.33

/**
 * Held in the air, the wings carry it forward towards this speed at full stick, m/s, by at most
 * this much, m/s².
 */
export const HEN_GLIDE_SPEED = 12.0
export const HEN_GLIDE_ACCEL = 4.0

/**
 * With no stamina left the wings spread and hold its fall to this sink, m/s, chasing it this hard,
 * per second.
 */
export const HEN_GLIDE_SINK = 4.0
export const HEN_GLIDE_GAIN = 3.0

/**
 * In the air the wings keep it level, leaning it this far (radians, roughly) with the stick, levelling
 * it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
 */
export const HEN_AIR_TILT = 0.2
export const HEN_AIR_LEVEL = 4.0
export const HEN_AIR_RESPONSE = 6.0
export const HEN_AIR_YAW = 1.2

/** Stamina comes back at this share per second while a foot is down and Space is let go. */
export const HEN_STAMINA_REFILL = 0.35

/**
 * Eggs: a clutch of `HEN_EGGS`, laid one at most every `HEN_EGG_COOLDOWN_TICKS` from `HEN_VENT` (hull
 * frame, under the tail) with the hull's velocity and `HEN_EGG_TOSS` m/s backwards, and one growing back
 * every `HEN_EGG_REGROW_TICKS`. An egg falls and bounces like a grenade (`HEN_EGG_RADIUS`, keeping
 * `HEN_EGG_BOUNCE` of its speed), bursts when `HEN_EGG_FUSE_TICKS` run out, and at once when it meets
 * something at more than `HEN_EGG_SMASH_SPEED` m/s, which a laid egg never does but a dropped one does.
 */
export const HEN_EGGS = 6
export const HEN_EGG_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 2)
export const HEN_EGG_REGROW_TICKS = TICK_RATE * 4
export const HEN_VENT = [0.0, -1.0, 2.0] as const
export const HEN_EGG_TOSS = 3.0
export const HEN_EGG_RADIUS = 0.35
export const HEN_EGG_BOUNCE = 0.3
export const HEN_EGG_FUSE_TICKS = TICK_RATE * 2
export const HEN_EGG_SMASH_SPEED = 16.0

/** An egg's blast, as a plane's bomb. */
export const HEN_EGG_BLAST_RADIUS = 6.0
export const HEN_EGG_DAMAGE = 110.0
export const HEN_EGG_KNOCK = 16.0

/**
 * The jeep (`KIND_JEEP`) is a Ford Bronco of the early eighties with its top off: a short, wide
 * four-by-four on big wheels with a roll bar over the front seats and plenty of ground clearance.
 * Slow on the road, but it rides over kerbs and rough ground. Half extents of the chassis box, metres.
 */
export const JEEP_HALF_EXTENTS = [1.0, 0.42, 1.95] as const
export const JEEP_WHEEL_RADIUS = 0.55

/**
 * Wheel hard points relative to the chassis centre: half the track, and the distance of the
 * axles from the centre along Z. Front axle is at -Z.
 */
export const JEEP_TRACK_HALF = 0.9
export const JEEP_AXLE_Z = 1.25
export const JEEP_HARD_POINT_Y = 0.0
export const JEEP_SUSPENSION_REST = 0.4

/**
 * Height of the chassis centre above flat ground once the suspension has settled (measured by
 * `ride_heights_differ_between_cars` in vehicle.rs). The clearance under the box is this minus the half height.
 */
export const JEEP_RIDE_HEIGHT = 0.857

/** Largest steering angle of the front wheels, radians. The wire carries it as -127..=127. */
export const JEEP_MAX_STEER = 0.5

/** Forward speed above which engine force fades to zero, m/s: a big V8 and a brick's shape. */
export const JEEP_MAX_SPEED = 32.0

/**
 * Seated players. Each seat of a vehicle with open seats has a head point in the hull frame, where
 * the client draws the sitting body's head (a car's `seat_N` markers, built by `build_car.py`). The
 * body that can be shot is a short upright capsule hanging below it: head and shoulders down to the
 * waist, so the hull box in front of the lower body takes the rounds aimed there.
 */
export const JEEP_SEAT_HEADS = [[-0.4, 0.62, -0.3], [0.4, 0.62, -0.3], [-0.4, 0.62, 0.75], [0.4, 0.62, 0.75]] as const

/**
 * The Opel Kadett E (`KIND_KADETT`): a wedge-shaped five-door hatchback, the everyday small car of
 * the late eighties. Five seats behind glass.
 */
export const KADETT_SEATS = 5
export const KADETT_HALF_EXTENTS = [0.83, 0.3, 2.0] as const
export const KADETT_WHEEL_RADIUS = 0.29
export const KADETT_TRACK_HALF = 0.7
export const KADETT_AXLE_Z = 1.26
export const KADETT_HARD_POINT_Y = 0.0
export const KADETT_SUSPENSION_REST = 0.248

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const KADETT_RIDE_HEIGHT = 0.445
export const KADETT_MAX_STEER = 0.55

/** Flat out, m/s. */
export const KADETT_MAX_SPEED = 38.0

/** The driver on the left, then the passengers, front to back. */
export const KADETT_SEAT_HEADS = [[-0.38, 0.73, -0.05], [0.38, 0.73, -0.05], [-0.4, 0.71, 0.75], [0.4, 0.71, 0.75], [0.0, 0.71, 0.75]] as const

/**
 * The loitering drone: a delta wing about 2.5 m across and 3.5 m long, with a pusher prop. It flies as
 * a simplified, nimble plane: always at `LOITER_SPEED` along its nose, with no throttle. The wings and
 * motor close any gap between its velocity and that at `LOITER_GRIP` per second, never harder than
 * `LOITER_MAX_ACCEL` m/s², so it goes where its nose points, and they hold it up whatever its attitude.
 * The nose is steered as a plane's is: the mouse sets where to go and it banks into the turn and pulls
 * (see `autopilot`), and W/S pitch and A/D roll by hand while held. It turns at most `LOITER_PITCH_RATE`,
 * `LOITER_YAW_RATE` and `LOITER_ROLL_RATE` rad/s about its own axes; its angular velocity follows the
 * controls at `LOITER_RESPONSE` per second.
 */
export const LOITER_SEATS = 1
export const LOITER_HALF_EXTENTS = [1.25, 0.25, 1.75] as const
export const LOITER_SPEED = 35.0
export const LOITER_GRIP = 10.0
export const LOITER_MAX_ACCEL = 80.0
export const LOITER_PITCH_RATE = 2.0
export const LOITER_ROLL_RATE = 4.0
export const LOITER_YAW_RATE = 0.8
export const LOITER_RESPONSE = 8.0

/**
 * The most the autopilot banks it into a turn, radians (about 57 degrees): less than the plane, so a
 * hard reversal stays a turn rather than a roll on to its back.
 */
export const LOITER_MAX_BANK = 1.0

/** Climbing fades out over the last 20 m below this height, and above it it is pushed down. */
export const LOITER_CEILING = 500.0

/** Air drag on the body, which is all that slows one whose motor is off. */
export const LOITER_DAMPING = 0.05

/**
 * The guided missile (`KIND_MISSILE`): a slim rocket about 2 m long with cruciform fins, fired
 * straight up out of a submarine's launch tube (`SUB_TUBE_AT`) and flown from its nose by the weapons
 * officer who fired it, as a loitering drone is. Its motor always drives it at `MISSILE_SPEED` along its
 * nose, closing any gap at `MISSILE_GRIP` per second and never harder than `MISSILE_MAX_ACCEL`; the
 * mouse sets where it goes, and it points its nose there with pitch and yaw and keeps its wings level. It leaves the tube slowly and
 * nose up, and its motor speeds it up along its nose by no more than `MISSILE_SPEED_UP` m/s²; its fins
 * bite with the square of its share of `MISSILE_FIN_SPEED`, so it climbs almost straight up at first and
 * pitches over towards the look as it gathers speed. At full speed it turns at most `MISSILE_PITCH_RATE`,
 * `MISSILE_YAW_RATE` and `MISSILE_ROLL_RATE` rad/s about its own axes, its angular velocity following
 * at `MISSILE_RESPONSE` per second.
 */
export const MISSILE_SEATS = 1
export const MISSILE_HALF_EXTENTS = [0.2, 0.2, 1.0] as const
export const MISSILE_SPEED = 70.0
export const MISSILE_GRIP = 4.0
export const MISSILE_MAX_ACCEL = 90.0
export const MISSILE_SPEED_UP = 18.0
export const MISSILE_FIN_SPEED = 65.0
export const MISSILE_PITCH_RATE = 1.8
export const MISSILE_ROLL_RATE = 5.0
export const MISSILE_YAW_RATE = 1.2
export const MISSILE_RESPONSE = 10.0

/** The most it banks into a turn, radians (about 70 degrees). */
export const MISSILE_MAX_BANK = 1.2

/** How steeply its flier may ask it to climb or dive, radians. */
export const MISSILE_MAX_PITCH = 1.4

/** Climbing fades out over the last 20 m below this height, and above it it is pushed down. */
export const MISSILE_CEILING = 400.0

/** Air drag on the body, which is all that slows one whose motor has burnt out. */
export const MISSILE_DAMPING = 0.02

/** How long its motor burns, ticks, and how far from its flier's body it can be flown, metres. */
export const MISSILE_ENERGY_TICKS = 20 * TICK_RATE
export const MISSILE_RANGE = 1500.0

/** Its warhead: smaller than a loitering drone's, bigger than a rocket's. */
export const MISSILE_BLAST_RADIUS = 8.0
export const MISSILE_DAMAGE = 250.0
export const MISSILE_KNOCK = 20.0
export const MISSILE_CRATER = [6.0, 1.2] as const

/** Shot down, its warhead only partly goes off. */
export const MISSILE_POP_RADIUS = 3.0
export const MISSILE_POP_DAMAGE = 40.0
export const MISSILE_POP_CRATER = [2.4, 0.4] as const

/**
 * A contact that takes this much speed (m/s) out of it sets it off, as does coming this close to a
 * soldier (metres); out of fuel or its link lost, it falls this many ticks at most before it goes off.
 */
export const MISSILE_IMPACT_DV = 3.0
export const MISSILE_PROXIMITY = 1.5
export const MISSILE_FALL_TICKS = 3 * TICK_RATE

/**
 * How far its nose is up from level as it leaves a launch tube, radians: all but straight up (about 89
 * degrees), tipped just enough towards where its flier looks that it pitches over that way.
 */
export const MISSILE_LAUNCH_PITCH = 1.55

/**
 * The field mortar (`KIND_MORTAR`): a towed, unpowered two-wheeler with one gunner, who aims its bearing
 * with the look and its range with the dial. It is towed by a car's rear tow point through a spherical joint.
 */
export const MORTAR_SEATS = 1
export const MORTAR_HALF_EXTENTS = [1.0, 0.5, 1.5] as const
export const MORTAR_WHEEL_RADIUS = 0.45
export const MORTAR_TRACK_HALF = 0.85

/** Where the mount sits in the hull frame, and the barrel's length from it. */
export const MORTAR_GUN_PIVOT = [0.0, 0.7, -0.2] as const
export const MORTAR_BARREL_LENGTH = 3.2

/**
 * Elevation travel, radians: a high lob at the bottom, nearly vertical at the top. The gunner never sets
 * it by hand: the range dial does (`Dial::pitch`).
 */
export const MORTAR_PITCH_MIN = 0.785
export const MORTAR_PITCH_MAX = 1.484

/**
 * The range dial, metres: W and S walk it at `MORTAR_RANGE_RATE` while `BUTTON_RANGE_UP` /
 * `BUTTON_RANGE_DOWN` are held. The longest lob is 300 m (`GRAVITY` and the muzzle speed at 45 degrees),
 * the shortest the high lob reaches is about 52 m, so the dial stops at 55.
 */
export const MORTAR_RANGE_START = 120.0
export const MORTAR_RANGE_MIN = 55.0
export const MORTAR_RANGE_MAX = 300.0
export const MORTAR_RANGE_RATE = 60.0

/**
 * How fast the mount follows the gunner's look, rad/s: a heavy gun cranked round by hand, so a new bearing takes
 * seconds, a half turn about six.
 */
export const MORTAR_TRAVERSE_SLEW = 0.5

/**
 * How fast the tube rises or falls to the elevation the range calls for, rad/s: from its flattest lob to its
 * steepest takes about four seconds.
 */
export const MORTAR_ELEVATION_SLEW = 0.17

/** Muzzle speed of a mortar bomb, m/s. Under `GRAVITY` it reaches about 300 m at 45 degrees and 52 m at 85. */
export const MORTAR_SHELL_SPEED = 77.46

/** A reload is long: the gunner waits between bombs. */
export const MORTAR_COOLDOWN_TICKS = 7 * TICK_RATE
export const MORTAR_SHELL_TTL_TICKS = 10 * TICK_RATE

/** Speed the mortar is shoved back at when it fires, m/s, along the barrel's horizontal heading. */
export const MORTAR_RECOIL = 3.0

/** Blast of a mortar bomb: wider and harder than a tank shell. */
export const MORTAR_RADIUS = 11.0
export const MORTAR_DAMAGE = 260.0
export const MORTAR_KNOCK = 30.0

/** The mortar's gunner rides its mount, so this is in the mount's frame: `MORTAR_GUN_PIVOT` turned by the gun's yaw. */
export const MORTAR_SEAT_HEAD = [0.6, 0.45, 0.6] as const

/**
 * The Opel Omega A (`KIND_OMEGA`): a long, smooth saloon with flush glass and a six under the
 * bonnet, that cruises fast and quietly. Five seats behind glass.
 */
export const OMEGA_SEATS = 5
export const OMEGA_HALF_EXTENTS = [0.88, 0.33, 2.33] as const
export const OMEGA_WHEEL_RADIUS = 0.31
export const OMEGA_TRACK_HALF = 0.725
export const OMEGA_AXLE_Z = 1.365
export const OMEGA_HARD_POINT_Y = 0.0
export const OMEGA_SUSPENSION_REST = 0.258

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const OMEGA_RIDE_HEIGHT = 0.475
export const OMEGA_MAX_STEER = 0.55

/** Flat out, m/s. */
export const OMEGA_MAX_SPEED = 42.0

/** The driver on the left, then the passengers, front to back. */
export const OMEGA_SEAT_HEADS = [[-0.38, 0.7, 0.0], [0.38, 0.7, 0.0], [-0.4, 0.68, 0.88], [0.4, 0.68, 0.88], [0.0, 0.68, 0.88]] as const

/**
 * The Pak 38 (`KIND_PAK`): a towed 5 cm anti-tank gun on two wheels with split trails and a shield,
 * one gunner. Its muzzle points down the hull's -Z, the trails run back to +Z, and it is towed by the
 * trails' eye (`PAK_TONGUE_Z`), so it travels muzzle-last. It fires the tank's shell, laid by the look
 * within its traverse and elevation, as a hull-mounted gun (`Aim::Mount`).
 */
export const PAK_SEATS = 1
export const PAK_HALF_EXTENTS = [0.8, 0.35, 1.4] as const
export const PAK_WHEEL_RADIUS = 0.4
export const PAK_TRACK_HALF = 0.8

/** The road wheels' axle, and the trails' spades it rests on at the back (the physics' second axle). */
export const PAK_AXLES_Z = [-0.7, 1.25] as const

/** The towing eye where the closed trails meet, behind the hull box. */
export const PAK_TONGUE_Z = PAK_HALF_EXTENTS[2] + TOW_BACK

/** The trunnions, over the axle, and the barrel's length from them. */
export const PAK_GUN_PIVOT = [0.0, 0.25, -0.7] as const
export const PAK_BARREL_LENGTH = 3.0

/**
 * Traverse either side of the trails' centre line, and elevation, radians: about 65 degrees in all,
 * and -8 to +27 degrees.
 */
export const PAK_TRAVERSE = 0.57
export const PAK_PITCH_MIN = -0.14
export const PAK_PITCH_MAX = 0.47

/** How fast the gun is laid onto the look, rad/s: hand wheels, a little quicker than a tank's turret. */
export const PAK_SLEW = 1.2

/** Speed it is shoved back at when it fires, m/s: its spades are dug in. */
export const PAK_RECOIL = 1.0

/** The shield across the front, in the hull frame: it stops what would hit the gunner from ahead. */
export const PAK_SHIELD_HALF = [0.85, 0.4, 0.03] as const
export const PAK_SHIELD_CENTRE = [0.0, 0.15, -0.95] as const

/** The Pak 38's gunner kneels behind the shield, left of the breech, at the sight: in the mount's frame. */
export const PAK_SEAT_HEAD = [-0.4, 0.2, 0.6] as const

/**
 * The Pbv 302 (`KIND_PBV`): an amphibious tracked troop carrier. The driver (seat 0) sits in the middle
 * of the front; the gunner (seat 1) sits to their left under a small turret with a 20 mm automatic
 * cannon; two troops (seats 2 and 3) ride sealed in the back and get out by the rear doors. It seats as
 * many as the Strf 90, is nearly as quick, thinner-skinned and far more lightly armed, and it floats:
 * on open water its tracks paddle it slowly along. Same body axes as the tank.
 */
export const PBV_SEATS = 4

/**
 * Half the hull's bounding box, metres: 5.35 m long, 2.86 m wide over the tracks and 1.4 m deep from the
 * belly to the hull roof, which is 1.8 m off the ground.
 */
export const PBV_HALF_EXTENTS = [1.43, 0.7, 2.675] as const

/** Road wheel radius, metres. */
export const PBV_WHEEL_RADIUS = 0.42

/** How far each track runs from the middle, metres. */
export const PBV_TRACK_HALF = 1.24

/**
 * Z of the three axles, front first, metres: set well out towards the ends of the hull, so that its
 * tail stays off the ground on a steep climb.
 */
export const PBV_AXLES_Z = [-2.0, 0.0, 2.0] as const

/** Where the hull's centre settles over flat ground, metres: measured by `a_pbv_settles_at_its_ride_height`. */
export const PBV_RIDE_HEIGHT = 1.1
export const PBV_HARD_POINT_Y = -0.3
export const PBV_SUSPENSION_REST = 0.44

/** Top speed on the flat, m/s (66 km/h). */
export const PBV_MAX_SPEED = 18.3

/** Yaw rate at full stick, rad/s. */
export const PBV_TURN_RATE = 1.3

/** How deep its hull floats, metres: the roof is 0.4 m clear of the water. */
export const PBV_DRAFT = 1.0

/**
 * Top speed afloat, m/s (9 km/h), how hard its tracks push it there, m/s², and its yaw rate at full
 * stick, rad/s.
 */
export const PBV_SWIM_SPEED = 2.5
export const PBV_SWIM_THRUST = 2.0
export const PBV_SWIM_TURN = 0.5

/**
 * Where the turret's ring sits in the hull frame (the middle of the turret's solid): at the front
 * left, over the gunner and beside the driver.
 */
export const PBV_TURRET_PIVOT = [-0.75, 0.95, -0.95] as const

/**
 * The turret's solid: a small cylinder on the ring, its base on the hull's roof and its top 2.3 m off
 * the ground, metres.
 */
export const PBV_TURRET_RADIUS = 0.5
export const PBV_TURRET_HALF_HEIGHT = 0.25

/** How far the muzzle is from the ring, metres: it reaches a little past the nose. */
export const PBV_BARREL_LENGTH = 2.0

/** How fast the turret and barrel follow the gunner's aim, rad/s. */
export const PBV_TURRET_SLEW = 1.8
export const PBV_BARREL_SLEW = 1.4

/** Barrel elevation limits, radians (+ is up): −10 to +37 degrees. */
export const PBV_BARREL_MIN = -0.17
export const PBV_BARREL_MAX = 0.65

/** Ticks between rounds: ten a second. */
export const PBV_CANNON_COOLDOWN_TICKS = 6

/** How far a round strays from the bore, radians. */
export const PBV_CANNON_SPREAD = 0.006

/** Heat a round puts in the barrel, of 255: about four seconds of holding the trigger jams it. */
export const PBV_CANNON_HEAT = 12.0

/** How far a round flies, metres. */
export const PBV_CANNON_RANGE = 400.0

/** What a round takes off a soldier, a vehicle's hull and an aircraft. */
export const PBV_CANNON_DAMAGE = 45
export const PBV_CANNON_VEHICLE_DAMAGE = 10
export const PBV_CANNON_PLANE_DAMAGE = 20

/**
 * Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
 * rear doors.
 */
export const PBV_DOOR_EXIT = [0.0, -0.1, 3.6] as const

/**
 * The triplane: a Fokker Dr.I stretched into a two-seater, flown by a pilot (seat 0) who also fires the
 * two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind. It rolls on
 * ray-cast wheels like a car, and in the air its wings hold it up once it is fast enough (see `wing` in
 * `physics/vehicle.rs`). Same body axes as the car.
 */
export const PLANE_SEATS = 2

/** The fuselage box. The wings are a second, weightless box (`PLANE_WING_*`), centred in the hull frame. */
export const PLANE_HALF_EXTENTS = [0.5, 0.6, 2.8] as const
export const PLANE_WING_CENTRE = [0.0, 0.45, -1.1] as const
export const PLANE_WING_HALF_EXTENTS = [3.6, 0.85, 0.55] as const
export const PLANE_WHEEL_RADIUS = 0.35
export const PLANE_TRACK_HALF = 0.8

/** The main gear ahead of the centre, the tail skid behind it. The front pair steers on the ground. */
export const PLANE_AXLES_Z = [-1.5, 2.4] as const
export const PLANE_HARD_POINT_Y = -0.5
export const PLANE_SUSPENSION_REST = 0.3
export const PLANE_MAX_STEER = 0.5

/**
 * Acceleration the engine gives at full throttle, m/s². With the stick released it runs at
 * `PLANE_CRUISE_THROTTLE` of that, and with the stick pulled back the blip switch cuts it to `PLANE_IDLE_THROTTLE`.
 */
export const PLANE_THRUST = 6.5
export const PLANE_CRUISE_THROTTLE = 0.6
export const PLANE_IDLE_THROTTLE = 0.03

/** Air drag per (m/s)², which tops a level flight out at about 45 m/s on full throttle and 35 at cruise. */
export const PLANE_DRAG = PLANE_THRUST / (45.0 * 45.0)

/** Below this airspeed (m/s) the wings stall: their lift fades out over the last 30% of it and the nose drops. */
export const PLANE_STALL_SPEED = 17.0

/** Above this airspeed the wings carry the plane level by themselves; below it they carry less, by the square. */
export const PLANE_LEVEL_SPEED = 28.0

/** The speed the controls are tuned for: they lose authority below it, by its square. */
export const PLANE_CRUISE_SPEED = 35.0

/**
 * How fast the wings turn the flight path towards where the nose points, per second, and how
 * fast the fin kills a sideslip.
 */
export const PLANE_AOA_GAIN = 4.0
export const PLANE_SIDE_GAIN = 3.0

/** The most the wings can pull, m/s² (about 4 g). */
export const PLANE_MAX_LIFT = 40.0

/** Turn rates at full deflection, rad/s, and how fast the body's spin follows them, per second. */
export const PLANE_PITCH_RATE = 1.4
export const PLANE_ROLL_RATE = 3.0
export const PLANE_YAW_RATE = 0.5
export const PLANE_RESPONSE = 6.0

/** How hard a stalled plane drops its nose towards where it is going, rad/s. */
export const PLANE_STALL_DROP = 1.5

/** Where a plane with nobody at the stick points its nose, radians below the horizon: it glides down. */
export const PLANE_GLIDE_PITCH = -0.2

/** Off-target angle (radians) beyond which the plane banks into the turn instead of levelling its wings. */
export const PLANE_BANK_ANGLE = 0.2

/** The steepest bank the plane rolls into by itself, radians (about 75°): past it the pull does the turning. */
export const PLANE_MAX_BANK = 1.3

/** Rate asked for per radian the target is off the nose, as a share of the full rate. */
export const PLANE_AIM_GAIN = 2.0

/** How steeply a pilot's mouse can ask the plane to climb or dive, radians (75 degrees). */
export const PLANE_AIM_MAX_PITCH = 1.3

/** With the stick centred the nose settles back to the horizon at this share of full pitch per radian off it. */
export const PLANE_LEVEL_GAIN = 1.5

/** The engine fades out over the last 20 m below this height; the boundary walls stop at 200 m. */
export const PLANE_CEILING = 160.0

/** Within this many metres of the edge of the world the plane is steered back towards the middle. */
export const PLANE_EDGE_MARGIN = 120.0

/**
 * The two forward guns on the cowling, which fire in turn along the nose. `PLANE_GUN_COOLDOWN_TICKS`
 * between rounds makes twelve a second between them. A round is a hitscan out to `PLANE_GUN_RANGE`.
 */
export const PLANE_GUN_PIVOTS = [[-0.16, 0.72, -2.0], [0.16, 0.72, -2.0]] as const
export const PLANE_GUN_COOLDOWN_TICKS = 5
export const PLANE_GUN_RANGE = 300.0
export const PLANE_GUN_SPREAD = 0.012

/** What a round does to a soldier, and to a vehicle: a plane is all canvas, anything else shrugs it off. */
export const PLANE_GUN_DAMAGE = 14
export const PLANE_GUN_PLANE_DAMAGE = 7
export const PLANE_GUN_VEHICLE_DAMAGE = 3

/**
 * Guns heat by this much a round and cool by `GUN_COOL` a tick, out of 255. At 255 they jam until
 * they have cooled to `GUN_UNJAM`: about four seconds of fire, then three of waiting.
 */
export const PLANE_GUN_HEAT = 10.5

/**
 * The gunner's ring mount behind the pilot: a pivot in the hull frame, the barrel's length, how far
 * it lifts and dips, and its rate. It turns all the way round, but holds fire within
 * `PLANE_REAR_DEAD_ZONE` of the nose, where the pilot sits.
 */
export const PLANE_REAR_PIVOT = [0.0, 0.85, 0.7] as const
export const PLANE_REAR_LENGTH = 0.9
export const PLANE_REAR_PITCH_MIN = -0.35
export const PLANE_REAR_PITCH_MAX = 1.0
export const PLANE_REAR_SLEW = 4.0
export const PLANE_REAR_DEAD_ZONE = 0.6
export const PLANE_REAR_COOLDOWN_TICKS = 8
export const PLANE_REAR_HEAT = 16.5

/**
 * Bombs on the rack under the fuselage, one dropped at most every `PLANE_BOMB_COOLDOWN_TICKS`. A
 * bomb leaves with the plane's velocity and falls; it bursts on the first thing it meets. The
 * rack fills again on the ground once the plane has stood still for `PLANE_REARM_TICKS`.
 */
export const PLANE_BOMBS = 4
export const PLANE_BOMB_PIVOT = [0.0, -0.85, -0.3] as const
export const PLANE_BOMB_COOLDOWN_TICKS = 30

/** The rack holds on to its bombs below this height above the ground, metres. */
export const PLANE_BOMB_MIN_HEIGHT = 4.0
export const PLANE_REARM_TICKS = 3 * TICK_RATE

/** The pilot behind the wings, the gunner standing in the ring. */
export const PLANE_SEAT_HEADS = [[0.0, 0.98, -0.45], [0.0, 1.15, 0.7]] as const

/**
 * The police car (`KIND_POLICE_CAR`): a Volvo 240 estate of the 1980s in white with a blue stripe
 * and a light bar on its roof. The same body, weight and springs as the Volvo 240 (`volvo_240.rs`),
 * with an engine that keeps pulling to a higher top speed, so it can catch the traffic. Five seats
 * behind glass.
 */
export const POLICE_CAR_SEATS = 5
export const POLICE_CAR_HALF_EXTENTS = [0.86, 0.36, 2.4] as const
export const POLICE_CAR_WHEEL_RADIUS = 0.32
export const POLICE_CAR_TRACK_HALF = 0.73
export const POLICE_CAR_AXLE_Z = 1.32
export const POLICE_CAR_HARD_POINT_Y = 0.0
export const POLICE_CAR_SUSPENSION_REST = 0.303

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const POLICE_CAR_RIDE_HEIGHT = 0.506
export const POLICE_CAR_MAX_STEER = 0.55

/** Flat out, m/s: about 15 % more than the Volvo 240's, well over what the traffic cruises at. */
export const POLICE_CAR_MAX_SPEED = 41.5

/** The driver on the left, then the passengers, front to back. */
export const POLICE_CAR_SEAT_HEADS = [[-0.38, 0.67, 0.0], [0.38, 0.67, 0.0], [-0.4, 0.65, 0.88], [0.4, 0.65, 0.88], [0.0, 0.65, 0.88]] as const

/**
 * The raft (`KIND_RAFT`): a small inflatable river-rafting boat for two, 3.6 m long and 1.6 m across
 * over its tubes, with a big outboard on its transom. Its driver sits on the left tube at the stern
 * with a hand on the tiller; the other sits on the thwart up in the bow. It floats a hand's breadth
 * deep, so it runs where the river is too thin for a heavier boat, and the motor shoves it out of the
 * water onto the plane at once.
 */
export const RAFT_SEATS = 2

/** Half the box over its tubes, metres: 3.6 m long, 1.6 m across and 0.5 m deep, the tubes' height. */
export const RAFT_HALF_EXTENTS = [0.8, 0.25, 1.8] as const

/** How deep it floats at rest, metres: an empty raft and its motor are light. */
export const RAFT_DRAFT = 0.12

/** About 27 knots: a 40 hp outboard on a boat this light. */
export const RAFT_MAX_SPEED = 14.0

/** How hard the outboard pushes it, m/s² at a standstill: it leaps off the mark. */
export const RAFT_THRUST = 6.0

/**
 * How hard it stops with the outboard put astern, m/s², and how far its bow digs in (radians) and its
 * tubes squat (metres) as it does.
 */
export const RAFT_BRAKE = 4.5
export const RAFT_BRAKE_DIP = 0.06
export const RAFT_BRAKE_SQUAT = 0.05

/** Its top speed astern, m/s. */
export const RAFT_ASTERN_SPEED = 3.0

/**
 * How fast it turns under way with the outboard pushing, rad/s: it whips its light stern round, a
 * circle about 15 m across flat out.
 */
export const RAFT_TURN_RATE = 1.9

/** How fast the outboard swings it round standing still, rad/s. */
export const RAFT_SPIN_RATE = 1.4

/**
 * How much higher it rides on the plane, metres, and the speeds it climbs onto it between, m/s: early,
 * as a light flat floor does.
 */
export const RAFT_PLANE_LIFT = 0.06
export const RAFT_PLANE_SPEED = [3.0, 7.0] as const

/**
 * How far its bow lifts as it climbs onto the plane and how far it stays up once on it, radians: the
 * light bow rears up under the motor's shove.
 */
export const RAFT_BOW_RISE = 0.16
export const RAFT_BOW_PLANING = 0.05

/** How far it banks into a turn, radians for each rad/s of turn and m/s of speed, and at most. */
export const RAFT_BANK = 0.016
export const RAFT_BANK_MAX = 0.22

/** How quickly its rise and fall follows the water's, per second: the face of a wave lifts it at once. */
export const RAFT_HEAVE = 10.0

/**
 * The share of its sideways slide its floor and the outboard's leg take out a second: it carves its
 * turns, with a little slide at the start of a hard one.
 */
export const RAFT_KEEL = 3.5

/** How hard it slows with the outboard let go, m/s²: it sits down in the water at once. */
export const RAFT_COAST = 3.0

/**
 * How hard it is brought level with the water under it, per second², and how quickly its roll and
 * pitch die away, per second: it pitches and rolls over every wave, and bobs.
 */
export const RAFT_LEVEL = 40.0
export const RAFT_ROCK = 2.5

/**
 * The share of its way through the water the water takes out a second while it coasts: the river
 * takes it up at once.
 */
export const RAFT_GRIP = 1.5

/** How quickly its rate of turn follows the helm and the water, per second: it snaps round at once. */
export const RAFT_HELM = 7.0

/**
 * The share of its turning that comes from the outboard's thrust: let go, it answers the helm only
 * lazily.
 */
export const RAFT_THRUST_STEER = 0.65

/**
 * How far the water's own turning turns it, a share of the water's rotation: more than a speck of
 * foam turns, as the slack water holds one end while the current takes the other.
 */
export const RAFT_LEVER = 2.0

/** The share of its drive and grip it keeps scraping over a shoal on its rubber floor. */
export const RAFT_SCRAPE = 0.5

/**
 * The outboard on the transom: where it turns on its bracket, in the hull frame, how long its tiller
 * is, metres, and how far it swings either way, radians.
 */
export const RAFT_MOTOR_PIVOT = [0.0, 0.1, 1.82] as const
export const RAFT_TILLER = 0.75
export const RAFT_MOTOR_SWING = 0.5

/** The driver on the left tube at the stern, a hand on the tiller, and the other on the thwart in the bow. */
export const RAFT_SEAT_HEADS = [[-0.5, 1.05, 1.15], [0.0, 1.0, -0.85]] as const

/**
 * The Roadster (`KIND_ROADSTER`): Tesla's 2026 two-seater with SpaceX cold-gas thrusters fed from a
 * tank of compressed air. A low, flat chassis on small wheels, a strong motor, and three
 * tricks: rear nozzles that push it far past what the tyres could (Shift), a hop straight up off the
 * ground (Space), and lift nozzles that hold it in the air while Space stays held. Thrust drains
 * the tank, which refills while it sits on its wheels.
 */
export const ROADSTER_SEATS = 2
export const ROADSTER_HALF_EXTENTS = [0.95, 0.3, 2.2] as const
export const ROADSTER_WHEEL_RADIUS = 0.32
export const ROADSTER_TRACK_HALF = 0.82
export const ROADSTER_AXLE_Z = 1.45
export const ROADSTER_HARD_POINT_Y = 0.05
export const ROADSTER_SUSPENSION_REST = 0.22

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const ROADSTER_RIDE_HEIGHT = 0.397
export const ROADSTER_MAX_STEER = 0.4
export const ROADSTER_MAX_SPEED = 62.0

/** The air tank, as a share of full. */
export const ROADSTER_AIR = 1.0

/** The rear nozzles' push along the nose, m/s²: well past the tyres' grip. */
export const ROADSTER_BOOST_ACCEL = 24.0

/** Air the rear nozzles use, share of the tank per second: about three seconds of boost. */
export const ROADSTER_BOOST_DRAIN = 0.33

/** Upward speed a hop adds, m/s: enough to clear a row of cars. */
export const ROADSTER_HOP_SPEED = 9.0

/** Air a hop uses, share of the tank. */
export const ROADSTER_HOP_COST = 0.15

/** While hovering, the lift nozzles hold this upward speed, m/s, and use this share of the tank per second. */
export const ROADSTER_HOVER_CLIMB = 1.0
export const ROADSTER_HOVER_DRAIN = 0.25

/** How hard the lift nozzles chase that speed, per second, and the most they add to gravity's worth, m/s². */
export const ROADSTER_HOVER_GAIN = 3.0
export const ROADSTER_HOVER_ACCEL = 6.0

/**
 * While hovering, the lift nozzles lean the car this far (radians, roughly) with the stick, level
 * it this hard and this fast (per second), and turn it at this yaw rate, rad/s.
 */
export const ROADSTER_HOVER_TILT = 0.2
export const ROADSTER_HOVER_LEVEL = 4.0
export const ROADSTER_HOVER_RESPONSE = 6.0
export const ROADSTER_HOVER_YAW = 1.2

/** The lift nozzles fade out over 20 m below this height. */
export const ROADSTER_CEILING = 40.0

/** Refill rate on the ground with no nozzle firing, share of the tank per second. */
export const ROADSTER_AIR_REFILL = 0.2
export const ROADSTER_SEAT_HEADS = [[-0.33, 0.42, 0.12], [0.33, 0.42, 0.12]] as const

/**
 * The Saab 900 Turbo (`KIND_SAAB_900`): a three-door hatchback from Trollhättan with a wraparound
 * windscreen and a long, sloping nose. Its turbo wakes late, so it is no quicker than a family car
 * off the line but runs on to the highest top speed of the everyday cars. Four seats behind glass.
 */
export const SAAB_900_SEATS = 4
export const SAAB_900_HALF_EXTENTS = [0.85, 0.32, 2.33] as const
export const SAAB_900_WHEEL_RADIUS = 0.31
export const SAAB_900_TRACK_HALF = 0.72
export const SAAB_900_AXLE_Z = 1.26
export const SAAB_900_HARD_POINT_Y = 0.0
export const SAAB_900_SUSPENSION_REST = 0.248

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const SAAB_900_RIDE_HEIGHT = 0.465
export const SAAB_900_MAX_STEER = 0.5

/** The fastest of the everyday cars flat out, m/s. */
export const SAAB_900_MAX_SPEED = 48.0

/** The driver on the left, then the passengers, front to back. */
export const SAAB_900_SEAT_HEADS = [[-0.38, 0.71, 0.0], [0.38, 0.71, 0.0], [-0.4, 0.69, 0.88], [0.4, 0.69, 0.88]] as const

/**
 * The Saab 9000 CS (`KIND_SAAB_9000`): a big five-door hatchback with a long glass tailgate, roomy
 * and quick, its turbo pulling hard through the middle gears. Five seats behind glass.
 */
export const SAAB_9000_SEATS = 5
export const SAAB_9000_HALF_EXTENTS = [0.88, 0.33, 2.3] as const
export const SAAB_9000_WHEEL_RADIUS = 0.32
export const SAAB_9000_TRACK_HALF = 0.76
export const SAAB_9000_AXLE_Z = 1.335
export const SAAB_9000_HARD_POINT_Y = 0.0
export const SAAB_9000_SUSPENSION_REST = 0.248

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const SAAB_9000_RIDE_HEIGHT = 0.475
export const SAAB_9000_MAX_STEER = 0.5

/** Flat out, m/s. */
export const SAAB_9000_MAX_SPEED = 45.0

/** The driver on the left, then the passengers, front to back. */
export const SAAB_9000_SEAT_HEADS = [[-0.38, 0.7, 0.0], [0.38, 0.7, 0.0], [-0.4, 0.68, 0.88], [0.4, 0.68, 0.88], [0.0, 0.68, 0.88]] as const

/**
 * The seaplane: a two-seat biplane of the 1920s on twin pontoons, flown by a pilot (seat 0) who also
 * fires the two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind.
 * It is an amphibian: each pontoon carries a pair of small wheels, so it rolls on a road as the
 * triplane does, and on open water the pontoons carry it (`floats` in `crates/sim`). Heavier and slower
 * than the triplane, and tougher. Same body axes as the car.
 */
export const SEAPLANE_SEATS = 2

/** The fuselage box. The wings and the pontoons are weightless boxes beside it, in the hull frame. */
export const SEAPLANE_HALF_EXTENTS = [0.5, 0.6, 3.0] as const
export const SEAPLANE_WING_CENTRE = [0.0, 0.45, -1.0] as const
export const SEAPLANE_WING_HALF_EXTENTS = [4.4, 0.85, 0.6] as const

/**
 * The two pontoons: how far either side of the centre line they are, the middle of each (y and z) and
 * half their size, metres.
 */
export const SEAPLANE_FLOAT_HALF_TRACK = 1.1
export const SEAPLANE_FLOAT_CENTRE = [-1.25, -0.4] as const
export const SEAPLANE_FLOAT_HALF_EXTENTS = [0.3, 0.3, 2.3] as const

/** How far below the hull's centre the pontoons' undersides are, and how deep they sit at rest, metres. */
export const SEAPLANE_KEEL = 1.55
export const SEAPLANE_DRAFT = 0.25

/** The wheels on the pontoons, a pair ahead of the centre and a pair behind. The front pair steers. */
export const SEAPLANE_WHEEL_RADIUS = 0.25
export const SEAPLANE_AXLES_Z = [-1.4, 1.6] as const
export const SEAPLANE_HARD_POINT_Y = -1.25
export const SEAPLANE_SUSPENSION_REST = 0.3
export const SEAPLANE_MAX_STEER = 0.5

/** How high its chassis centre rides over a road on its wheels, metres (measured by the sim's tests). */
export const SEAPLANE_RIDE_HEIGHT = 1.71

/** Acceleration the engine gives at full throttle, m/s²: less than the triplane's, for the weight of the pontoons. */
export const SEAPLANE_THRUST = 5.5

/** Air drag per (m/s)², which tops a level flight out at about 38 m/s on full throttle and 29 at cruise. */
export const SEAPLANE_DRAG = SEAPLANE_THRUST / (38.0 * 38.0)

/** Its wings stall below this airspeed, carry it level above the next, and its controls are tuned for the last, m/s. */
export const SEAPLANE_STALL_SPEED = 18.0
export const SEAPLANE_LEVEL_SPEED = 25.0
export const SEAPLANE_CRUISE_SPEED = 29.0

/** Turn rates at full deflection, rad/s: it rolls and pitches more slowly than the triplane. */
export const SEAPLANE_PITCH_RATE = 1.2
export const SEAPLANE_ROLL_RATE = 2.2
export const SEAPLANE_YAW_RATE = 0.45

/** The most the wings can pull, m/s² (about 3.5 g). */
export const SEAPLANE_MAX_LIFT = 34.0

/**
 * The two forward guns on the cowling, the gunner's ring behind the pilot and the bomb rack between the
 * pontoons: pivots in the hull frame. The guns, the ring and the rack work as the triplane's do
 * (`PLANE_GUN_*`, `PLANE_REAR_*`, `PLANE_BOMB*`).
 */
export const SEAPLANE_GUN_PIVOTS = [[-0.16, 0.72, -2.2], [0.16, 0.72, -2.2]] as const
export const SEAPLANE_REAR_PIVOT = [0.0, 0.85, 0.9] as const
export const SEAPLANE_BOMB_PIVOT = [0.0, -0.85, -0.3] as const

/** The pilot behind the upper wing, the gunner standing in the ring. */
export const SEAPLANE_SEAT_HEADS = [[0.0, 0.98, -0.3], [0.0, 1.15, 0.9]] as const

/**
 * The pirate ship (`KIND_SHIP`): a single-masted brig of about 10 m, sailed from the helm at the
 * stern, with three guns run out on each broadside (`SHIP_GUN_SLOTS`, each its own `KIND_SHIP_GUN`).
 * It floats and sails only on open water, driven by the wind in its sail (`crate::wind`), and is
 * stranded wherever it runs aground. The hull's top is its deck, walked on between the guns; the
 * bulwarks along its sides keep whoever walks it aboard. Its bow tapers to a point over `SHIP_BOW`.
 */
export const SHIP_SEATS = 1
export const SHIP_HALF_EXTENTS = [1.8, 0.7, 5.0] as const

/** How far back from the stem the sides taper out to the full beam, metres. */
export const SHIP_BOW = 2.2

/** How deep the hull floats, metres: the deck stands this much less than its height out of the water. */
export const SHIP_DRAFT = 0.6

/** Top speed on a beam reach in a full wind, m/s (about 20 knots). */
export const SHIP_MAX_SPEED = 10.0

/** How quickly it turns under way, at full rudder, rad/s. */
export const SHIP_TURN_RATE = 0.6

/**
 * The share of a full sail's drive her motor gives whatever the wind, hidden below decks: into the
 * wind, where the sail only flaps, it still pushes her along slowly.
 */
export const SHIP_MOTOR = 0.35

/** The deck, in the hull frame. */
export const SHIP_DECK_Y = SHIP_HALF_EXTENTS[1]

/**
 * The bulwarks: waist high over the deck, too high to climb over, with the guns firing through ports
 * in them; and as thick as planks.
 */
export const SHIP_BULWARK_HEIGHT = 0.75
export const SHIP_BULWARK_THICK = 0.1

/** The mast, from the deck: how tall and how thick, and how far forward of the middle it stands. */
export const SHIP_MAST_HEIGHT = 8.0
export const SHIP_MAST_RADIUS = 0.15
export const SHIP_MAST_Z = -0.7

/** The helm, where its helmsman stands and from where it is boarded, near the stern. */
export const SHIP_HELM = [0.0, SHIP_DECK_Y + 0.9, 4.0] as const
export const SHIP_HELM_HEAD = [0.0, SHIP_DECK_Y + 1.6, 4.3] as const

/** Where the helmsman steps down onto the deck, the middle of a soldier standing just ahead of the helm. */
export const SHIP_HELM_EXIT = [0.0, SHIP_DECK_Y + STAND_CENTRE + 0.05, 3.3] as const

/** How far out from the centre line the guns stand, and how far along it each pair stands, metres. */
export const SHIP_GUN_X = 1.15
export const SHIP_GUN_Z = [-1.8, 0.4, 2.6] as const

/**
 * A ship's gun (`KIND_SHIP_GUN`): a long gun on a wooden truck carriage, fixed to its ship's deck
 * (`Fitting`, `SHIP_GUN_SLOTS`) and carried with it, so nothing pushes or drives it. One gunner lays it
 * by the look, a little either way along the broadside and up and down, and fires round shot
 * (`PROJECTILE_BALL`): slower than a shell, so it arcs, and bursting small where it lands. Its
 * muzzle points down its own -Z, over the side.
 */
export const SHIP_GUN_SEATS = 1
export const SHIP_GUN_HALF_EXTENTS = [0.3, 0.3, 0.5] as const

/** The trunnions, over the carriage's front, and the barrel's length from them. */
export const SHIP_GUN_PIVOT = [0.0, 0.25, -0.12] as const
export const SHIP_GUN_BARREL_LENGTH = 1.4

/** Traverse either side of square to the ship, and elevation, radians. */
export const SHIP_GUN_TRAVERSE = 0.5
export const SHIP_GUN_PITCH_MIN = -0.12
export const SHIP_GUN_PITCH_MAX = 0.35

/** How fast it is laid onto the look, rad/s: handspikes and tackle, slow. */
export const SHIP_GUN_SLEW = 0.8

/** Ticks between shots: swabbed, loaded and run out again. */
export const SHIP_GUN_COOLDOWN_TICKS = 4 * TICK_RATE

/** How hard it kicks back, m/s, which its ship takes at the gun: a broadside rolls it. */
export const SHIP_GUN_RECOIL = 1.5

/** The round shot's muzzle speed, m/s. */
export const BALL_SPEED = 90.0

/**
 * Its small burst where it lands, beside the harm of the iron itself to whatever it goes through
 * (radius metres, damage, knock m/s), and the hole it knocks in the ground (radius, depth, metres).
 */
export const BALL_BLAST_RADIUS = 2.5
export const BALL_BLAST_DAMAGE = 55.0
export const BALL_BLAST_KNOCK = 7.0
export const CRATER_BALL_RADIUS = 1.3
export const CRATER_BALL_DEPTH = 0.3

/** The gunner stands behind the breech, from the pivot, with the gun pointing ahead. */
export const SHIP_GUN_SEAT_HEAD = [0.25, 0.5, 0.85] as const

/** Where the gun is manned from, behind its breech, and where its gunner steps back onto the deck. */
export const SHIP_GUN_BOARD = [0.0, 0.2, 0.6] as const
export const SHIP_GUN_EXIT = [0.0, STAND_CENTRE + 0.05 - SHIP_GUN_HALF_EXTENTS[1], SHIP_GUN_HALF_EXTENTS[2] + CAPSULE_RADIUS + 0.3] as const

/**
 * The Ford Sierra (`KIND_SIERRA`): the rounded "jelly mould" five-door hatchback, a family car of
 * middling pace. Five seats behind glass.
 */
export const SIERRA_SEATS = 5
export const SIERRA_HALF_EXTENTS = [0.85, 0.32, 2.2] as const
export const SIERRA_WHEEL_RADIUS = 0.3
export const SIERRA_TRACK_HALF = 0.725
export const SIERRA_AXLE_Z = 1.305
export const SIERRA_HARD_POINT_Y = 0.0
export const SIERRA_SUSPENSION_REST = 0.258

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const SIERRA_RIDE_HEIGHT = 0.465
export const SIERRA_MAX_STEER = 0.55

/** Flat out, m/s. */
export const SIERRA_MAX_SPEED = 40.0

/** The driver on the left, then the passengers, front to back. */
export const SIERRA_SEAT_HEADS = [[-0.38, 0.71, 0.0], [0.38, 0.71, 0.0], [-0.4, 0.69, 0.88], [0.4, 0.69, 0.88], [0.0, 0.69, 0.88]] as const

/**
 * The snowmobile (`KIND_SNOWMOBILE`): a modern mountain sled, light and very powerful, for two
 * riders one behind the other. The physics drives it as a car: the front pair of wheels are its
 * skis, which steer, and two pairs behind stand in for the track, whose long footprint keeps it
 * straight with a gun in tow. Its power to weight takes it up slopes no car can manage, and a Pak
 * or a mortar up steep ones.
 */
export const SNOWMOBILE_SEATS = 2
export const SNOWMOBILE_HALF_EXTENTS = [0.55, 0.3, 1.6] as const
export const SNOWMOBILE_WHEEL_RADIUS = 0.22

/** Half the ski stance: the skis are 1.1 m apart. */
export const SNOWMOBILE_TRACK_HALF = 0.55
export const SNOWMOBILE_AXLE_Z = 1.2

/** The track touches down from just behind the seat's front to the tail: a middle axle stands in for its front end. */
export const SNOWMOBILE_TRACK_Z = 0.3
export const SNOWMOBILE_HARD_POINT_Y = -0.05

/** Long-travel suspension, for bumps and drifts. */
export const SNOWMOBILE_SUSPENSION_REST = 0.35
export const SNOWMOBILE_RIDE_HEIGHT = 0.559

/** Skis turn far at a crawl, and the lock fades with speed, fast (`SNOWMOBILE_STEER_FADE`). */
export const SNOWMOBILE_MAX_STEER = 0.6

/**
 * The speed at which the skis' lock has halved, m/s: far lower than a car's, so a sled at speed
 * steers a little and a sled at a crawl steers a lot (0.3 rad at 7 m/s, 0.16 at 20).
 */
export const SNOWMOBILE_STEER_FADE = 7.0
export const SNOWMOBILE_MAX_SPEED = 20.0

/** The driver sits up front, the passenger behind and a little higher on the seat's rise. */
export const SNOWMOBILE_SEAT_HEADS = [[0.0, 1.05, 0.1], [0.0, 1.15, 0.8]] as const

/**
 * The snowracer (`KIND_SNOWRACER`): a steerable sled, a short steel frame on three skis, with a seat
 * for a rider who steers by a little wheel and one more behind. It is built for the ski slopes: down
 * one it slides as fast as a snowmobile drives, with nothing but gravity pulling it. A small motor
 * under the seat takes it about on the flat and up a piste again at a crawl. The physics drives it
 * as a car: the front pair of wheels, close together, are its one steering ski, and the rear pair its
 * two runners. It slides on them rather than rolls (`Runners`).
 */
export const SNOWRACER_SEATS = 2
export const SNOWRACER_HALF_EXTENTS = [0.3, 0.15, 0.75] as const
export const SNOWRACER_WHEEL_RADIUS = 0.1

/** Half the runners' stance: they are 0.5 m apart. The front ski stands on the same pair of points. */
export const SNOWRACER_TRACK_HALF = 0.25

/** The front ski is under the nose, and the runners' middle under the passenger. */
export const SNOWRACER_AXLE_Z = 0.55
export const SNOWRACER_HARD_POINT_Y = -0.05

/** A little give in the skis' mounts, for bumps. */
export const SNOWRACER_SUSPENSION_REST = 0.15
export const SNOWRACER_RIDE_HEIGHT = 0.202

/** The front ski turns far at a crawl, and the lock fades with speed, fast (`SNOWRACER_STEER_FADE`). */
export const SNOWRACER_MAX_STEER = 0.5

/** The speed at which the ski's lock has halved, m/s: at a slope's pace it steers a little. */
export const SNOWRACER_STEER_FADE = 8.0

/**
 * The motor's top speed on the flat, m/s. Downhill the motor gives nothing past it, and gravity
 * takes the sled on to as fast as its drag lets it (`SNOWRACER_DRAG`).
 */
export const SNOWRACER_MAX_SPEED = 6.0

/** Sliding friction on snow, as a waxed ski's (`SKI_MU_SNOW`). */
export const SNOWRACER_MU = 0.05

/** Air drag per (m/s)², m/s² per (m/s)², of two riders sitting up on a sled. */
export const SNOWRACER_DRAG = 0.0006

/**
 * In the air the nose swings onto the line it flies at this rate, per second, so it neither dives
 * nor tumbles off a bump.
 */
export const SNOWRACER_AIR_FOLLOW = 5.0

/** How far the rider leans the nose down (W) or up (S) off that line in the air, radians (20°). */
export const SNOWRACER_AIR_LEAN = 0.35

/** How fast the rider turns it in the air at full stick, rad/s. */
export const SNOWRACER_AIR_TURN = 1.5

/** The share of its weight it is spared in the air, so that it flies far off a bump. */
export const SNOWRACER_AIR_FLOAT = 0.45

/** Both riders sit low on the seat, the passenger behind the rider with their legs either side of them. */
export const SNOWRACER_SEAT_HEADS = [[0.0, 0.85, 0.0], [0.0, 0.85, 0.5]] as const

/** A full boost, the `Spec::tank` of a kind with a `Boost`. */
export const BOOST_FULL = 1.0

/** What a second of boost held with the throttle open uses, a share of full: about four seconds of it. */
export const BOOST_DRAIN = 0.25

/** What a second let go gives back, a share of full: it fills again in about six. */
export const BOOST_REFILL = 0.17

/** The speed at which a car's steering lock has halved, m/s: the usual `Spec::steer_fade`. */
export const STEER_FADE = 20.0

/** Only the front axle steers, as on a car. */
export const FRONT_STEER = 1

/** The `sweep` of a hull whose underside is flat from end to end. */
export const FLAT = 0.0

/** Over how many metres below the ceiling the thinning air takes the engine's power away. */
export const CEILING_FADE = 40.0

/**
 * The sports car (`KIND_SPORTS`): an Opel Manta GT/E, a low rear-drive coupé from the early eighties
 * with a long bonnet and a short roof. The quickest car on the road away from a standstill, with the
 * chassis only about 12 cm off the ground. Four seats behind glass, the back two cramped.
 */
export const SPORTS_SEATS = 4
export const SPORTS_HALF_EXTENTS = [0.84, 0.3, 2.2] as const
export const SPORTS_WHEEL_RADIUS = 0.3
export const SPORTS_TRACK_HALF = 0.7
export const SPORTS_AXLE_Z = 1.26
export const SPORTS_HARD_POINT_Y = 0.05
export const SPORTS_SUSPENSION_REST = 0.26

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const SPORTS_RIDE_HEIGHT = 0.417
export const SPORTS_MAX_STEER = 0.5
export const SPORTS_MAX_SPEED = 46.0

/** The driver on the left, a passenger beside them, and two more on the low back bench. */
export const SPORTS_SEAT_HEADS = [[-0.37, 0.72, 0.1], [0.37, 0.72, 0.1], [-0.38, 0.66, 0.85], [0.38, 0.66, 0.85]] as const

/**
 * The Strf 90 (`KIND_STRF`): a tracked infantry fighting vehicle. The driver (seat 0) sits front left
 * beside the engine; the gunner (seat 1) works a 40 mm autocannon in a turret set forward of the
 * middle; two troops (seats 2 and 3) ride sealed in the back and get out by the rear ramp. Lighter
 * and quicker than the tank, and thinner-skinned. Same body axes as the tank.
 */
export const STRF_SEATS = 4

/**
 * Half the hull's bounding box, metres: 6.55 m long, 3.2 m wide over the tracks and 1.3 m deep from the
 * belly to the hull roof, which is 1.75 m off the ground. The size of a CV90.
 */
export const STRF_HALF_EXTENTS = [1.6, 0.65, 3.275] as const

/** Road wheel radius, metres. */
export const STRF_WHEEL_RADIUS = 0.42

/** How far each track runs from the middle, metres. */
export const STRF_TRACK_HALF = 1.33

/** Z of the three axles, front first, metres. */
export const STRF_AXLES_Z = [-2.3, 0.0, 2.3] as const

/** Where the hull's centre settles over flat ground, metres: measured by `a_strf_settles_at_its_ride_height`. */
export const STRF_RIDE_HEIGHT = 1.1
export const STRF_HARD_POINT_Y = -0.3
export const STRF_SUSPENSION_REST = 0.44

/** Top speed on the flat, m/s (about 70 km/h). */
export const STRF_MAX_SPEED = 19.0

/** Yaw rate at full stick, rad/s. */
export const STRF_TURN_RATE = 1.4

/**
 * Where the turret ring sits in the hull frame (the middle of the turret's solid), forward of the
 * middle with the troop bay behind it.
 */
export const STRF_TURRET_PIVOT = [0.0, 1.025, -0.4] as const

/**
 * The turret's solid: a cylinder on the ring, its base on the hull's roof and its top 2.5 m off the
 * ground, metres.
 */
export const STRF_TURRET_RADIUS = 1.0
export const STRF_TURRET_HALF_HEIGHT = 0.375

/** How far the muzzle is from the ring, metres: it reaches a little past the nose. */
export const STRF_BARREL_LENGTH = 3.1

/** How fast the turret and barrel follow the gunner's aim, rad/s: quicker than the tank's. */
export const STRF_TURRET_SLEW = 2.0
export const STRF_BARREL_SLEW = 1.4

/** Barrel elevation limits, radians (+ is up): steep enough to fire at aircraft. */
export const STRF_BARREL_MIN = -0.14
export const STRF_BARREL_MAX = 0.85

/** Ticks between rounds: five a second. */
export const STRF_CANNON_COOLDOWN_TICKS = 12

/** How far a round strays from the bore, radians. */
export const STRF_CANNON_SPREAD = 0.004

/** Heat a round puts in the barrel, of 255: about five seconds of holding the trigger jams it. */
export const STRF_CANNON_HEAT = 22.0

/**
 * Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
 * rear ramp.
 */
export const STRF_RAMP_EXIT = [0.0, -0.1, 4.2] as const

/**
 * The Strv S (`KIND_STRV`): a turretless tank with the gun fixed in the hull, the size and shape of a
 * Stridsvagn 103. One seat: the driver steers by pivoting the hull and fires the main gun, which only
 * the barrel's elevation can aim. Quicker and lower than the tank, and a wedge: its glacis is half its
 * length. Same body axes as the tank; mirrors `STRV_*` in `sim/consts.ts`.
 */
export const STRV_SEATS = 1

/**
 * Half the hull's bounding box, metres: 7.04 m long, 3.4 m wide over the tracks and 1.4 m deep from the
 * belly to the roof, which is 1.9 m off the ground.
 */
export const STRV_HALF_EXTENTS = [1.7, 0.7, 3.52] as const

/**
 * Bigger wheels and a longer suspension than the tank's, with the hull higher on them: it steps over
 * obstacles the tank has to go around.
 */
export const STRV_WHEEL_RADIUS = 0.5
export const STRV_TRACK_HALF = 1.36

/** Z of the three axles, front first. */
export const STRV_AXLES_Z = [-2.4, 0.0, 2.4] as const

/** Where the hull's centre settles over flat ground, metres: measured by `scripted_drive_matches_fixture`. */
export const STRV_RIDE_HEIGHT = 1.2
export const STRV_HARD_POINT_Y = -0.3
export const STRV_SUSPENSION_REST = 0.46
export const STRV_MAX_SPEED = 16.0

/** Yaw rate at full stick, rad/s. */
export const STRV_TURN_RATE = 1.5

/**
 * Where the gun's trunnion is in the hull frame (where the barrel leaves the glacis), how far the muzzle
 * is from it (2 m past the nose), how fast the barrel elevates (rad/s) and its limits, radians (+ is
 * up). The gun points where the hull does.
 */
export const STRV_GUN_PIVOT = [0.0, 0.5, -1.0] as const
export const STRV_BARREL_LENGTH = 4.5
export const STRV_BARREL_SLEW = 1.2
export const STRV_BARREL_MIN = -0.15
export const STRV_BARREL_MAX = 0.3

/**
 * The submarine (`KIND_SUBMARINE`): a small diesel-electric boat of about 22 m, with a sail (its conning
 * tower) forward of the middle, a periscope that rises out of it, and four vertical launch tubes in
 * the deck behind it. Its captain (seat 0) drives it and takes it up and down: jump rises at
 * `SUB_DIVE_RATE`, run dives, and with neither it holds its depth. It floats on the surface at
 * `SUB_SURFACE_DEPTH` and goes down as far as the bed lets it. Its weapons officer (seat 1) looks
 * through the periscope while its head is above the water, and from no deeper than
 * `SUB_LAUNCH_DEPTH` fires a guided missile (`KIND_MISSILE`) out of a loaded tube, which they then fly
 * from its nose. Both are sealed in its hull. Its bow tapers over `SUB_BOW`.
 */
export const SUB_SEATS = 2
export const SUB_HALF_EXTENTS = [1.4, 1.3, 11.0] as const

/** How far back from the stem the hull tapers out to its full beam, metres. */
export const SUB_BOW = 3.0

/**
 * The sail, from the deck: half its width, height and length, metres, and how far forward of the
 * middle it stands (its centre's Z in the hull frame).
 */
export const SUB_SAIL_HALF = [0.6, 1.0, 2.0] as const
export const SUB_SAIL_Z = -3.0

/** The top of the sail, in the hull frame: where its hatch is, which it is boarded from. */
export const SUB_HATCH = [0.0, 3.3, -3.0] as const

/** Where whoever leaves it stands, on the deck behind the sail (the middle of a standing soldier). */
export const SUB_EXIT = [0.0, 1.35 + STAND_CENTRE, 4.0] as const

/**
 * How far below the surface its middle floats when it is up, metres: the deck stands
 * `SUB_HALF_EXTENTS[1] - SUB_SURFACE_DEPTH` out of the water.
 */
export const SUB_SURFACE_DEPTH = 0.6

/**
 * How high above its middle the periscope's head stands when it is raised, metres. It sees out while
 * that is above the water, so from any depth of its middle less than this.
 */
export const SUB_MAST_HEIGHT = 5.5

/** The deepest its middle may be for a tube to fire, metres: periscope depth. */
export const SUB_LAUNCH_DEPTH = 4.5

/** How fast it rises or dives with jump or run held, m/s. */
export const SUB_DIVE_RATE = 2.0

/** Top speed ahead, m/s (about 16 knots), and how hard the motor pushes it along, m/s². */
export const SUB_MAX_SPEED = 8.0
export const SUB_ACCEL = 1.2

/** How quickly it turns under way at full rudder, rad/s. */
export const SUB_TURN_RATE = 0.35

/**
 * Its launch tubes: how many, and where each one's mouth is, on the deck behind the sail, in the hull
 * frame.
 */
export const SUB_TUBES = 4
export const SUB_TUBE_AT = [[-0.55, 1.3, 0.6], [0.55, 1.3, 0.6], [-0.55, 1.3, 1.9], [0.55, 1.3, 1.9]] as const

/** How long a fired tube takes to load again, ticks. */
export const SUB_TUBE_RELOAD = 20 * TICK_RATE

/** How fast a missile leaves its tube, straight up, m/s. */
export const SUB_LAUNCH_SPEED = 12.0

/**
 * The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0), a gunner (seat 1)
 * who aims the turret and fires, and a machine gunner (seat 2) out of the roof hatch. It is the size
 * and shape of a Strv 104 (a Centurion). Same body axes as the car.
 */
export const TANK_SEATS = 3

/**
 * Half the hull's bounding box, metres: 7.6 m long, 3.4 m wide over the tracks and 1.25 m deep from
 * the belly to the hull roof, which is 1.75 m off the ground.
 */
export const TANK_HALF_EXTENTS = [1.7, 0.625, 3.8] as const
export const TANK_WHEEL_RADIUS = 0.45
export const TANK_TRACK_HALF = 1.4

/** Z of the three axles, front first. */
export const TANK_AXLES_Z = [-2.6, 0.0, 2.6] as const

/**
 * Where the hull's centre settles over flat ground, metres: measured by `scripted_drive_matches_fixture`.
 * A whole number of centimetres, as a snapshot carries it: a settled tank's prediction then starts
 * from where the server has it.
 */
export const TANK_RIDE_HEIGHT = 1.13
export const TANK_HARD_POINT_Y = -0.3
export const TANK_SUSPENSION_REST = 0.44
export const TANK_MAX_SPEED = 12.0

/** Yaw rate at full stick, rad/s. */
export const TANK_TURN_RATE = 1.1

/**
 * Where the turret ring sits in the hull frame (the middle of the turret's solid), a little ahead of
 * the hull's middle, and how far the muzzle is from the ring: the long gun reaches 2.3 m past the nose.
 */
export const TANK_TURRET_PIVOT = [0.0, 1.125, -0.4] as const
export const TANK_BARREL_LENGTH = 5.7

/**
 * The turret is solid: a cylinder on the ring, so it is the same shape whichever way it points.
 * Its base is the hull's top, and its roof is 2.75 m off the ground.
 */
export const TANK_TURRET_RADIUS = 1.15
export const TANK_TURRET_HALF_HEIGHT = 0.5

/** How fast the turret and barrel follow the gunner's aim, rad/s. */
export const TANK_TURRET_SLEW = 1.4
export const TANK_BARREL_SLEW = 0.9

/** Barrel elevation limits, radians (+ is up). */
export const TANK_BARREL_MIN = -0.12
export const TANK_BARREL_MAX = 0.5

/**
 * What it takes to knock the turret off: damage from hits on the turret and the hull's roof, which a
 * hull hit never adds to. Past it `DAMAGE_TURRET` is set, the gunner dies and the gun is gone until the
 * tank respawns. Mirrors `TANK_TURRET_HP` in `sim/consts.ts`.
 */
export const TANK_TURRET_HP = 90

/** The seat that works the roof machine gun. */
export const TANK_MG_SEAT = 2

/** Where the roof gun turns, from the turret ring in the turret's frame, over the hatch on its right. */
export const TANK_MG_AT = [0.5, 0.75, 0.35] as const

/**
 * The machine gunner's head from the ring, with the turret ahead: out of the hatch, behind the gun, and
 * clear over the turret's solid, so they can be shot from all round.
 */
export const TANK_MG_HEAD = [0.5, 1.15, 0.85] as const
export const TANK_MG_LENGTH = 1.1

/**
 * How far the roof gun turns either side of where the cannon points, and its elevation, radians (+ is
 * up): steep enough for aircraft.
 */
export const TANK_MG_YAW_LIMIT = 1.57
export const TANK_MG_PITCH_MIN = -0.25
export const TANK_MG_PITCH_MAX = 0.9
export const TANK_MG_SLEW = 2.5
export const TANK_MG_COOLDOWN_TICKS = 6
export const TANK_MG_SPREAD = 0.012
export const TANK_MG_RANGE = 300.0
export const TANK_MG_DAMAGE = 16
export const TANK_MG_PLANE_DAMAGE = 8
export const TANK_MG_VEHICLE_DAMAGE = 4

/** Heat a round puts in the barrel: about six seconds of holding the trigger jams it. */
export const TANK_MG_HEAT = 10.5

/**
 * The technical (`KIND_TECHNICAL`): a long-bonnet pickup with a heavy machine gun on a post in its open
 * bed. A driver and a passenger sit in the cab (seats 0 and 1, the passenger firing their own guns) and
 * the gunner (seat 2) stands in the bed behind the gun, turning with it all the way round. It is tall,
 * on big tyres and long soft springs: it wallows through corners but rides over rough ground.
 */
export const TECHNICAL_SEATS = 3
export const TECHNICAL_HALF_EXTENTS = [0.95, 0.4, 2.45] as const
export const TECHNICAL_WHEEL_RADIUS = 0.58
export const TECHNICAL_TRACK_HALF = 0.85
export const TECHNICAL_AXLE_Z = 1.6
export const TECHNICAL_HARD_POINT_Y = -0.1
export const TECHNICAL_SUSPENSION_REST = 0.6

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const TECHNICAL_RIDE_HEIGHT = 1.08
export const TECHNICAL_MAX_STEER = 0.5

/** About 108 km/h, a little slower than the jeep. */
export const TECHNICAL_MAX_SPEED = 30.0

/** The seat that works the machine gun. */
export const TECHNICAL_MG_SEAT = 2

/** Where the gun turns on top of its post, over the bed and above the cab's roof, so it fires over it. */
export const TECHNICAL_MG_PIVOT = [0.0, 1.45, 0.9] as const

/**
 * The gunner's head from the pivot, with the gun pointing ahead: standing in the bed behind it, their
 * feet on its floor (0.15 m over the hull's middle) and their head 1.64 m above them.
 */
export const TECHNICAL_MG_HEAD = [0.0, 0.34, 0.75] as const
export const TECHNICAL_MG_LENGTH = 1.2

/** Its elevation, radians (+ is up): steep enough for aircraft. It turns all the way round. */
export const TECHNICAL_MG_PITCH_MIN = -0.3
export const TECHNICAL_MG_PITCH_MAX = 0.9
export const TECHNICAL_MG_SLEW = 2.5

/**
 * A heavy, slow gun: four rounds a second, each one hitting hard. It deals what the tank's roof gun does
 * in a second, but in fewer, bigger blows: four put a soldier down.
 */
export const TECHNICAL_MG_COOLDOWN_TICKS = 15
export const TECHNICAL_MG_SPREAD = 0.014
export const TECHNICAL_MG_RANGE = 300.0
export const TECHNICAL_MG_DAMAGE = 40
export const TECHNICAL_MG_PLANE_DAMAGE = 20
export const TECHNICAL_MG_VEHICLE_DAMAGE = 10

/**
 * Heat a round puts in the barrel: about four seconds of holding the trigger jams it, a little sooner
 * than the tank's roof gun, and about three more before it has cooled enough to clear.
 */
export const TECHNICAL_MG_HEAT = 31.0

/**
 * The driver on the left and the passenger in the cab, then the gunner as they stand with the gun ahead
 * (`TECHNICAL_MG_PIVOT` plus `TECHNICAL_MG_HEAD`).
 */
export const TECHNICAL_SEAT_HEADS = [[-0.42, 1.0, -0.35], [0.42, 1.0, -0.35], [0.0, 1.79, 1.65]] as const

/**
 * The Titan (`KIND_TITAN`): a giant robot about fifty metres high, one pilot sealed in its head. Its
 * body is a single box, its torso, held up by two spring legs that are ray casts to the ground and
 * kept upright by a torque, exactly as a walker's is (its `Gait`). Its pilot moves its two fists
 * (`TITAN_ARMS`): they hit what they meet and hold what they close on. The right one holds a
 * three-barrel rotary cannon that fires heavy rounds along the forearm, and in the top of its left shoulder block are four vertical tubes
 * that send up the submarine's guided missile (`KIND_MISSILE`), which the pilot then flies. Its feet are solid
 * and crush what they come down on. Rounds are not shot at the box but at the model's own shape: its
 * torso's `Role::Hit` solids, and its arms and legs bent as the client draws them (`bend`).
 */
export const TITAN_SEATS = 1

/** Half the torso's box, metres. The hull frame has its origin at the box's centre. */
export const TITAN_HALF_EXTENTS = [6.0, 8.0, 4.0] as const

/**
 * Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
 * bottom of the torso to the ground when it stands.
 */
export const TITAN_FOOT_HALF = 5.0
export const TITAN_LEG_LENGTH = 26.0

/** How much shorter its legs are when it crouches, metres, and how far it leans forward then, as a tilt. */
export const TITAN_CROUCH = 16.0
export const TITAN_CROUCH_LEAN = 0.3

/** How fast it stands up from a crouch, m/s. */
export const TITAN_RISE = 7.0

/** Top speed, m/s, and its yaw rate at full stick, rad/s. */
export const TITAN_MAX_SPEED = 11.0
export const TITAN_TURN_RATE = 0.6

/**
 * The leg spring, per kilogram of the body: soft, so it settles `CAR_GRAVITY / TITAN_LEG_SPRING` into
 * its legs and takes a second to crouch or to stand.
 */
export const TITAN_LEG_SPRING = 12.0

/**
 * Where the hull's centre settles over flat ground when it stands, metres. Measured by
 * `a_titan_stands_fifty_metres_high_on_its_legs`.
 */
export const TITAN_RIDE_HEIGHT = 33.2

/** Where the right shoulder is in the hull frame; the left is its mirror in X. */
export const TITAN_SHOULDER = [8.5, 6.0, 0.0] as const

/** The furthest and the nearest a fist's middle can be from its shoulder, metres, and a fist's radius. */
export const TITAN_REACH = 24.0
export const TITAN_MIN_REACH = 6.0
export const TITAN_FIST_RADIUS = 2.5

/** A fist's top speed against the hull, m/s, and how hard it speeds up and slows, m/s². */
export const TITAN_FIST_SPEED = 45.0
export const TITAN_FIST_ACCEL = 140.0

/** Where the right fist hangs when nobody asks anything of it, in the hull frame. */
export const TITAN_FIST_REST = [10.0, -15.0, -3.0] as const

/** The heaviest vehicle a closed fist picks up, kilograms: a tank, but not a ship. */
export const TITAN_LIFT_MASS = 60000.0

/** Half extents of a shin's column, metres, which stands where the foot is planted. */
export const TITAN_SHIN_HALF = [2.2, 6.0, 3.2] as const

/** The fuel its thrusters burn, as `Spec::tank`: 1 is full. */
export const TITAN_FUEL = 1.0

/**
 * The upward speed its leap gives, the climb its thrusters hold and how fast they carry it along, m/s,
 * and how high over the ground they take it, metres.
 */
export const TITAN_LEAP_SPEED = 14.0
export const TITAN_JET_CLIMB = 8.0
export const TITAN_JET_SPEED = 20.0
export const TITAN_CEILING = 120.0

/**
 * Coming down at this speed or faster, m/s, its landing is a slam, which is at its worst at
 * `TITAN_SLAM_FULL`.
 */
export const TITAN_SLAM_SPEED = 8.0
export const TITAN_SLAM_FULL = 35.0

/** How far out a slam reaches, metres, at `TITAN_SLAM_SPEED` and at `TITAN_SLAM_FULL`. */
export const TITAN_SLAM_RADIUS = 20.0
export const TITAN_SLAM_RADIUS_FULL = 50.0

/**
 * The speed a soldier or a vehicle at the middle of the worst slam is thrown outward with, m/s, and
 * the health each loses there. Both fall away to nothing at its edge.
 */
export const TITAN_SLAM_KNOCK = 22.0
export const TITAN_SLAM_DAMAGE = 160.0
export const TITAN_SLAM_VEHICLE_DAMAGE = 220.0

/**
 * Its longest step and the shortest it walks with, metres; how long a foot is in the air and how long
 * the weight takes to go from one foot to the other, seconds; and how high a foot lifts in the middle
 * of a step, metres. A leg 26 m long swings no faster than this: the slowness is what makes it big.
 */
export const TITAN_STRIDE = 18.0
export const TITAN_STRIDE_MIN = 10.0
export const TITAN_STEP_TIME = 1.3
export const TITAN_SHIFT_TIME = 0.35
export const TITAN_STEP_LIFT = 4.5

/** Half extents of each thigh, a solid on the hull that hangs from the hip, metres. */
export const TITAN_THIGH_HALF = [2.0, 3.5, 2.4] as const

/**
 * The bones of its limbs as the model is built (`UPPER`, `FORE`, `THIGH`, `SHIN` and `PAD` in
 * `tools/vehicles/build_titan.py`): shoulder to elbow, elbow to the fist's middle, hip to knee and knee
 * to ankle, metres, and how far the ankle is above the sole.
 */
export const TITAN_UPPER_ARM_LENGTH = 12.0
export const TITAN_FOREARM_LENGTH = 12.0
export const TITAN_THIGH_LENGTH = 13.5
export const TITAN_SHIN_LENGTH = 13.5
export const TITAN_ANKLE_HEIGHT = 1.5

/** Which way the right elbow bends, in the hull frame: out, down and back. The left mirrors it in X. */
export const TITAN_ELBOW_POLE = [0.7, -0.5, 0.6] as const

/** Which way the knees bend, in the hull frame: forward, a little down. */
export const TITAN_KNEE_POLE = [0.0, -0.15, -1.0] as const

/**
 * How thick each bone is to a round, as the radius of a capsule along it, metres: the upper arm, the
 * forearm, the thigh and the shin with their plates.
 */
export const TITAN_UPPER_ARM_RADIUS = 2.1
export const TITAN_FOREARM_RADIUS = 2.5
export const TITAN_THIGH_RADIUS = 2.6
export const TITAN_SHIN_RADIUS = 2.7

/**
 * Where it is boarded from and where its pilot is set down, in the hull frame: on the ground between
 * its feet, and ahead of them.
 */
export const TITAN_BOARD = [0.0, -32.2, 0.0] as const
export const TITAN_EXIT = [0.0, -32.2, -10.0] as const

/** The pilot's eye, in the hull frame: at the visor, just clear of the head's own plates. */
export const TITAN_EYE = [0.0, 12.0, -3.2] as const

/** The neck its head turns on, in the hull frame: the middle of the collar's top. */
export const TITAN_HEAD_PIVOT = [0.0, 9.4, 0.0] as const

/**
 * How far its head turns to follow its pilot's look, radians: to either side of the way the hull
 * faces, down and up.
 */
export const TITAN_HEAD_TURN = 1.0
export const TITAN_HEAD_DOWN = 1.2
export const TITAN_HEAD_UP = 0.7

/** How fast its head turns, rad/s. */
export const TITAN_HEAD_SLEW = 4.0

/** A fist moving at least this fast, m/s, hits what it meets; slower, it only shoves. */
export const TITAN_PUNCH_SPEED = 8.0

/** What a punch takes off a soldier, per m/s of the fist's speed, and off a vehicle. */
export const TITAN_PUNCH_DAMAGE = 6.0
export const TITAN_PUNCH_VEHICLE_DAMAGE = 4.0

/**
 * How far around a foot coming down soldiers are knocked off their feet, metres, and how hard they
 * are thrown at its edge, m/s. Under the foot itself they are crushed.
 */
export const TITAN_STOMP_RADIUS = 14.0
export const TITAN_STOMP_KNOCK = 9.0

/** What a foot coming down takes off a vehicle under it. */
export const TITAN_STOMP_VEHICLE_DAMAGE = 120.0

/** A soldier thrown from a fist keeps this share of the fist's speed. */
export const TITAN_THROW_SHARE = 1.0

/** The fist that holds the rotary cannon: the right (0 is the left). */
export const TITAN_GUN_HAND = 1

/** How far the gun's muzzles are ahead of the fist's middle along the forearm, metres. */
export const TITAN_GUN_LENGTH = 8.0

/**
 * Its barrels, which turn a third of a turn round the forearm's line between rounds, and how far each
 * is from that line, metres. Its rounds leave on the line itself.
 */
export const TITAN_GUN_BARRELS = 3
export const TITAN_GUN_BARREL_OFFSET = 0.75

/** Ticks between rounds: two and a half a second. */
export const TITAN_GUN_COOLDOWN = 24

/** How far a round strays from the bore, radians: a fist is no turret. */
export const TITAN_GUN_SPREAD = 0.006

/** Heat a round puts in the barrels, of 255: about eight seconds of holding the trigger jams it. */
export const TITAN_GUN_HEAT = 37.0

/** How fast its round leaves the muzzle, m/s: slower than a tank's shell, so it is seen to fly. */
export const TITAN_ROUND_SPEED = 110.0

/**
 * Its round's burst (`PROJECTILE_ROTARY`): how far it reaches, metres, what it takes off at the middle
 * and how hard it throws, m/s. Bigger than a tank's armour-piercing shell's, but a little over a third
 * of a mortar bomb's reach, so it bites a wall about 1.8 m round (`CARVE_REACH`) to the mortar's 5 m.
 */
export const TITAN_ROUND_RADIUS = 4.0
export const TITAN_ROUND_DAMAGE = 110.0
export const TITAN_ROUND_KNOCK = 14.0

/** The crater it digs, radius and depth, metres, and how hard it knocks a vehicle it strikes, m/s. */
export const TITAN_ROUND_CRATER = [3.0, 0.5] as const
export const TITAN_ROUND_HIT_KNOCK = 0.35

/**
 * The missile tubes: how many, and where each one's mouth is, in the hull frame. They are hatches set
 * in the top of the left shoulder block (`TUBES` in `tools/vehicles/build_titan.py`).
 */
export const TITAN_TUBES = 4
export const TITAN_TUBE_AT = [[-9.6, 11.3, -1.0], [-7.6, 11.3, -1.0], [-9.6, 11.3, 1.0], [-7.6, 11.3, 1.0]] as const

/** Ticks to load one more tube: as long as a submarine's tube takes. */
export const TITAN_TUBE_RELOAD = 20 * TICK_RATE

/** Ticks between launches, at the least. */
export const TITAN_TUBE_COOLDOWN = TICK_RATE

/**
 * The Volkswagen Transporter T3 (`KIND_TRANSPORTER`): a flat-fronted van with its engine under the
 * floor at the back. Its driver sits over the front wheels with one passenger beside them, then a
 * bench of two by the sliding door and a bench of three at the back. Tall and slow, the slowest
 * car on the road but the EPA. Seven seats behind glass.
 */
export const TRANSPORTER_SEATS = 7
export const TRANSPORTER_HALF_EXTENTS = [0.92, 0.5, 2.25] as const
export const TRANSPORTER_WHEEL_RADIUS = 0.33
export const TRANSPORTER_TRACK_HALF = 0.785
export const TRANSPORTER_AXLE_Z = 1.23
export const TRANSPORTER_HARD_POINT_Y = 0.0
export const TRANSPORTER_SUSPENSION_REST = 0.486

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const TRANSPORTER_RIDE_HEIGHT = 0.685
export const TRANSPORTER_MAX_STEER = 0.6

/** Flat out, m/s. */
export const TRANSPORTER_MAX_SPEED = 30.0

/** The driver on the left, then the passengers, front to back. */
export const TRANSPORTER_SEAT_HEADS = [[-0.45, 0.87, -1.25], [0.45, 0.87, -1.25], [-0.4, 0.87, 0.05], [0.4, 0.87, 0.05], [-0.5, 0.87, 1.15], [0.0, 0.87, 1.15], [0.5, 0.87, 1.15]] as const

/**
 * The tupp (`KIND_TUPP`): a giant robot rooster of the Swedish Hedemora breed, one driver in a cockpit
 * on its back. It walks, leaps, flaps and glides exactly as the hen does (the `WALKER_*` legs and the
 * `HEN_*` wings), and lays nothing: its head follows the driver's look and its eyes burn a laser.
 */
export const TUPP_SEATS = 1

/**
 * The head turns within `TUPP_HEAD_LIMIT` of the way the legs face, and `TUPP_HEAD_PITCH_MIN..=_MAX` up
 * and down (radians, hull frame), at `TUPP_HEAD_SLEW` rad/s, about `TUPP_HEAD_PIVOT` in the hull frame.
 */
export const TUPP_HEAD_LIMIT = 0.9
export const TUPP_HEAD_PITCH_MIN = -0.6
export const TUPP_HEAD_PITCH_MAX = 0.5
export const TUPP_HEAD_SLEW = 3.5
export const TUPP_HEAD_PIVOT = [0.0, 2.45, -2.3] as const

/**
 * How far the beam starts ahead of the pivot, and how far either side of the middle the eyes are,
 * metres. It is settled from between them; the two beams are drawn converging on where it ended.
 */
export const TUPP_EYE_AHEAD = 0.55
export const TUPP_EYE_X = 0.27

/**
 * The laser: `TUPP_LASER_CHARGE_TICKS` of holding the trigger, then a beam for `TUPP_LASER_BURN_TICKS`
 * that strikes every `TUPP_LASER_HIT_TICKS` out to `TUPP_LASER_RANGE`, then `TUPP_LASER_RECOVER_TICKS`
 * to cool. One whole burn finishes anything short of a tank, through up to `TUPP_LASER_PEN` mm of plate.
 */
export const TUPP_LASER_CHARGE_TICKS = Math.trunc(TICK_RATE * 5 / 4)
export const TUPP_LASER_BURN_TICKS = Math.trunc(TICK_RATE * 3 / 2)
export const TUPP_LASER_HIT_TICKS = Math.trunc(TICK_RATE / 10)
export const TUPP_LASER_RECOVER_TICKS = TICK_RATE * 3
export const TUPP_LASER_RANGE = 250.0

/** How wide the beam is, metres from its middle to its edge: it burns the first soldier or vehicle inside that. */
export const TUPP_LASER_RADIUS = 1.5
export const TUPP_LASER_DAMAGE = 30
export const TUPP_LASER_VEHICLE_DAMAGE = 14
export const TUPP_LASER_PLANE_DAMAGE = 30
export const TUPP_LASER_PEN = 60.0

/**
 * The Volvo 240 estate (`KIND_VOLVO_240`): the brick. Long, square and heavy, with a vertical
 * tailgate and bumpers built for elks, it is slow to get going, slow flat out and soft on its
 * springs. Five seats behind glass.
 */
export const VOLVO_240_SEATS = 5
export const VOLVO_240_HALF_EXTENTS = [0.86, 0.36, 2.4] as const
export const VOLVO_240_WHEEL_RADIUS = 0.32
export const VOLVO_240_TRACK_HALF = 0.73
export const VOLVO_240_AXLE_Z = 1.32
export const VOLVO_240_HARD_POINT_Y = 0.0
export const VOLVO_240_SUSPENSION_REST = 0.303

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const VOLVO_240_RIDE_HEIGHT = 0.506
export const VOLVO_240_MAX_STEER = 0.55

/** Slow flat out, m/s. */
export const VOLVO_240_MAX_SPEED = 36.0

/** The driver on the left, then the passengers, front to back. */
export const VOLVO_240_SEAT_HEADS = [[-0.38, 0.67, 0.0], [0.38, 0.67, 0.0], [-0.4, 0.65, 0.88], [0.4, 0.65, 0.88], [0.0, 0.65, 0.88]] as const

/**
 * The Volvo 740 saloon (`KIND_VOLVO_740`): a square-cut four-door, solid and unhurried, in the
 * middle of the pack for pace. Five seats behind glass.
 */
export const VOLVO_740_SEATS = 5
export const VOLVO_740_HALF_EXTENTS = [0.88, 0.34, 2.4] as const
export const VOLVO_740_WHEEL_RADIUS = 0.32
export const VOLVO_740_TRACK_HALF = 0.735
export const VOLVO_740_AXLE_Z = 1.385
export const VOLVO_740_HARD_POINT_Y = 0.0
export const VOLVO_740_SUSPENSION_REST = 0.258

/** Measured by `ride_heights_differ_between_cars` in vehicle.rs. */
export const VOLVO_740_RIDE_HEIGHT = 0.485
export const VOLVO_740_MAX_STEER = 0.55

/** Flat out, m/s. */
export const VOLVO_740_MAX_SPEED = 40.0

/** The driver on the left, then the passengers, front to back. */
export const VOLVO_740_SEAT_HEADS = [[-0.38, 0.69, 0.0], [0.38, 0.69, 0.0], [-0.4, 0.67, 0.88], [0.4, 0.67, 0.88], [0.0, 0.67, 0.88]] as const

/**
 * The walker (`KIND_WALKER`): a two-legged assault walker, one driver in the head. Its body is a single
 * box held up by two spring legs that are ray casts to the ground, and kept upright by a torque that a
 * big enough blow overpowers. Mirrors `WALKER_*` in `sim/consts.ts`.
 */
export const WALKER_SEATS = 1
export const WALKER_HALF_EXTENTS = [1.0, 1.2, 1.8] as const
export const HEN_HALF_EXTENTS = WALKER_HALF_EXTENTS
export const TUPP_HALF_EXTENTS = WALKER_HALF_EXTENTS

/**
 * Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
 * bottom of the hull to the ground when it stands.
 */
export const WALKER_FOOT_HALF = 1.7
export const WALKER_LEG_LENGTH = 4.2

/**
 * The legs are solid, so a car or a shell can hit them: a box on each hip, this wide and deep (half
 * extents, metres), ending this far above the ground when it stands so the feet's own springs carry it.
 */
export const WALKER_LEG_HALF_WIDTH = 0.25
export const WALKER_LEG_HALF_DEPTH = 0.3
export const WALKER_LEG_CLEARANCE = 0.5

/** Top speed forwards, backwards (a share of it), m/s, and the acceleration towards it, m/s². */
export const WALKER_MAX_SPEED = 9.0
export const WALKER_REVERSE_SHARE = 0.5
export const WALKER_ACCEL = 4.5

/** Deceleration, m/s², when it slows or reverses: it plants its feet and leans back. */
export const WALKER_BRAKE = 6.0

/** Planted feet do not slide sideways: the push against it, per m/s of sideways speed, and its cap, m/s². */
export const WALKER_GRIP_GAIN = 8.0
export const WALKER_GRIP = 8.0

/** Yaw rate at full stick, rad/s, and the angular acceleration towards it, rad/s². */
export const WALKER_TURN_RATE = 1.7
export const WALKER_TURN_ACCEL = 8.0

/**
 * Leg spring and damper, per kilogram of the whole body for the pair: the spring settles at
 * `CAR_GRAVITY / WALKER_LEG_SPRING` of compression.
 */
export const WALKER_LEG_SPRING = 80.0
export const WALKER_LEG_DAMPING = 14.0

/**
 * How hard the hull is pulled upright, rad/s² per radian of tilt, and the damping on its tilt rate, per
 * second. The pull never exceeds `WALKER_BALANCE_MAX` rad/s²: a blow that tips it further than that
 * can undo goes over.
 */
export const WALKER_BALANCE_GAIN = 10.0
export const WALKER_BALANCE_DAMP = 4.0
export const WALKER_BALANCE_MAX = 3.0

/**
 * Pitching (forwards and backwards) is the axis a walker can fight best: its feet are in line with the
 * tilt, so the pull is up to this, and it also takes a catching step. The step is a push towards where
 * the top is falling, by `WALKER_CATCH_GAIN` m/s² per unit of lean from where it should be and
 * `WALKER_CATCH_DAMP` per rad/s of fall, at most `WALKER_CATCH_MAX`.
 */
export const WALKER_BALANCE_MAX_PITCH = 6.0
export const WALKER_CATCH_GAIN = 20.0
export const WALKER_CATCH_DAMP = 3.0
export const WALKER_CATCH_MAX = 8.0

/**
 * Lean (up's horizontal part along the heading beyond its walking lean) and fall rate (rad/s) it
 * tolerates before it takes a step, so a walk's own lean and its pushes are left alone.
 */
export const WALKER_CATCH_LEAN_MARGIN = 0.15
export const WALKER_CATCH_FALL_MARGIN = 0.5

/** Extra lean into its heading at top speed, as a tilt: the walker walks a little forward of its feet. */
export const WALKER_CRUISE_LEAN = 0.08

/**
 * How much of the tilt that would match its push it leans by: the push goes through the middle of
 * the hull, so the lean is for show and for how it handles.
 */
export const WALKER_ACCEL_LEAN = 0.5

/** A blast pushes a walker at its shoulders, this far above the middle of the hull, so it tips it. */
export const WALKER_BLAST_LEVER = 3.0

/**
 * Below this `up_y` it has fallen: the legs no longer push or drive, and only a weak pull
 * (`WALKER_GETUP_SHARE` of the full one) stands it up again.
 */
export const WALKER_FALLEN_UP = 0.6
export const WALKER_GETUP_SHARE = 0.25

/**
 * The chin guns: the driver aims them with the look, and the upper body turns to point them, up to
 * `WALKER_TORSO_LIMIT` either side of the way the legs face (and `WALKER_GUN_PITCH_MIN..=_MAX` up and
 * down, radians, hull frame), at `WALKER_GUN_SLEW` rad/s. `WALKER_GUN_PIVOT` is where the upper body turns
 * in the hull frame and `WALKER_GUN_LENGTH` how far the muzzle is from it. A round is a cannon shell, as
 * in a helicopter's nose gun, fired every `WALKER_GUN_COOLDOWN_TICKS` and scattered by up to `WALKER_GUN_SPREAD`.
 */
export const WALKER_TORSO_LIMIT = 0.52
export const WALKER_GUN_PITCH_MIN = -0.5
export const WALKER_GUN_PITCH_MAX = 0.4
export const WALKER_GUN_SLEW = 2.5
export const WALKER_GUN_PIVOT = [0.0, 0.1, 0.0] as const
export const WALKER_GUN_LENGTH = 3.7

/**
 * The two barrels hang `WALKER_GUN_BARREL_DROP` below the pivot, `WALKER_GUN_BARREL_X` either side of
 * it, and fire in turn.
 */
export const WALKER_GUN_BARREL_X = 0.3
export const WALKER_GUN_BARREL_DROP = 1.0
export const WALKER_GUN_COOLDOWN_TICKS = Math.trunc(TICK_RATE / 4)
export const WALKER_GUN_SPREAD = 0.01
