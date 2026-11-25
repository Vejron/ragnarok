# Doors: getting in and out of a car, bailing out, and doors that come off

A saloon's two front doors open and shut, and its driver and front passenger get in and out by them. A
police car's crew and an ambulance's medics are the same two people at the wheel and out of it, and so
is a driver of the traffic who gets out. A door can be shot or crashed off, and one standing open goes
far sooner. A door left open swings on its hinge as the car goes, and is torn off by whatever it snags on.

## In play

- **Getting out.** Press E in a front seat of a saloon (the thirteen cars built by `build_saloon`: the
  period cars of the traffic, the Opel Manta, the police car and the ambulance).
  - The door swings open over 0.25 s (`DOOR_OPEN_S`).
  - You climb out over 0.8 s (`STEP_OUT_S`): legs out, ducking under the roof, standing up.
  - You end up standing in the gap between the open door and the car (`STEP_OUT_SIDE`, `STEP_OUT_BACK`).
  - The door is pulled shut 0.25 s later (`DOOR_SHUT_S`).
  - Nothing else works meanwhile: no driving, no firing, no switching seats.
- **Getting in.**
  - Standing by a free door, E takes that door's seat. Elsewhere it takes the first free seat, as before.
  - The seat is yours at once, but you climb in over the same steps, the other way round. A driver can't
    drive off until they are sitting.
- **Taking a car of the traffic.**
  - Its driver climbs out by the driver's door, and you wait outside for them to finish.
  - They are the same person (the same id) in the same colour as at the wheel.
- **Bailing out.**
  - Pressing E in a car going faster than 6 m/s (`BAIL_SPEED`) throws you out at once, with no door,
    at the car's speed plus a push away from its side.
  - You are knocked down (KNOCKDOWN.md), and hurt from 10 m/s up, the more the faster (`BAIL_HURT_FROM`, `BAIL_LETHAL`).
  - A car that gets past 6 m/s while you are climbing out throws you out the same way.
  - This holds for every car with four wheels, with or without doors.
- **Seats with no door** (rear seats, and the kinds without doors, such as a jeep, a tank or a
  helicopter) are got in and out of at once, as before.
- **Police and ambulance crews.**
  - Each car drives with two people in it, the driver and the one beside them, both drawn in their seats.
  - Parked, both open their doors together, climb out and become the officers or the medics, under the
    same ids. They leave the doors standing open.
  - Back at the car, each climbs in by a free door and is its crew again, and they pull the doors shut.
  - If nobody was near enough to see into the car as it came, nobody sat in it, and its crew appear at
    the doors as before.
- **Doors that come off.**
  - A door comes off after 120 points of damage (`DOOR_HP`).
  - **Rounds:** a round that meets the door takes half its damage off it, so a rifle magazine or so (`shoot_doors`).
  - **Blasts:** a blast takes off more the nearer it is (`blast_doors`).
  - **Crashes:** a crash on its side counts as well (`crash_doors`).
  - **Open doors:** a door standing open takes four times as much from a crash, and is torn off by a blow
    a shut door shrugs off.
  - **Snags:** an open door that meets anything (a wall, a tree, a post, another car) going past it faster
    than 2.5 m/s, a walk, is torn off at once (`DOOR_SNAG_TEAR`). Slower, it is shoved shut by what it
    meets and dented as it scrapes past (`DOOR_SNAG_HURT`).
  - **Reversing:** the wind of a car reversing faster than about 7 m/s (25 km/h) holds an open door
    against its stop and bends it off within a second or so (`DOOR_STOP_STRAIN`). Slamming onto the stop
    hard does the same (`DOOR_STOP_TEAR`).
  - A torn-off door is left lying where it snagged, not carried along with the car.
  - Once a door is off, getting out by it skips the wait for it to open.
- **Open doors swing.** A door nobody is holding swings freely on its hinge:
  - Pulling away swings it shut, and it latches there. Braking swings it open.
  - Driving forwards, the wind blows it shut (`DOOR_AIR`); reversing blows it open.
  - Leaning the car on a slope swings it.
  - Standing still, it stays where it was left (`DOOR_HINGE_HOLD`).
  - The car is never moved by its doors.
  - The old cut-out door that crash damage used to throw off is gone on these cars; the rear doors stay
    part of the body.

## How it is made

- **The doors** are `VehicleDef::doors`: a `DoorDef` for each, with its seat, hinge, length, height,
  how far it swings and the damage it takes. Kinds without doors have `NO_DOORS`.
  - The hinges are measured off the models (`build_car.py` prints them) and written into each kind's
    `<KIND>_DOOR`.
  - `every_door_hangs_on_its_cars_side…` in `shared` checks them against the body and the seats, and
    `car.spec.ts` checks them against the `door_FL` and `door_FR` nodes of every model.
- **The model.** `build_saloon` splits each front door off the body at its seams, as `door_FL` and
  `door_FR`, with the origin on the hinge.
  - The door carries its glass, handle and livery, with a dark inner skin.
  - The body has dark jambs at the opening's seams and a cabin floor at the sill, so the gap shows
    the seats and the dash, whether the door stands open or has come off.
  - `--open RADIANS` with `--preview` draws them swung open.
- **The climb** is the soldier's `car_out` clip (`car_out_r` to the right), 0.8 s long, in every stance.
  - It starts as `sit` does and ends as `idle` does.
  - It is played forward to get out and backward to get in.
  - It carries the rise from sitting, with the feet staying on the seat's origin.
- **On the server**, `mechanics/vehicles/doors.rs`.
  - Whoever climbs is `Stepping`, a component on a player or on a driver of the traffic.
  - They stay in their seat until they are out, so a player's predicted soldier never moves meanwhile.
  - A player is then set down at the door's spot (`alight_at`).
  - A driver is marked `done`, for whoever put them out to take on: `Traffic::walk_out` makes a
    civilian of them, and `Emergency::alight_crews` an officer or a medic.
  - Back at the car, a crew member is turned into that car's `Driver` again (`EmergencyCall::Aboard`,
    `seat_crew`), climbing in.
  - `Vehicle::doors` (`DoorState`) holds each door's damage, which stand open, which are gone, when
    each is to be pulled shut, and how far open each is and how fast it swings.
  - **The hinge** (`doors::swing`, every tick after `step`). A door being climbed through is swung open
    by hand, and one being pulled shut is swung shut by hand, both at the pace of `DOOR_OPEN_S`.
    - Any other open door is a flat plate on a hinge at its front edge, in the hull frame. It is pushed
      by gravity less the car's acceleration, found from its velocity tick to tick, and by the wind,
      which goes as the square of the speed across its face. It is slowed by `DOOR_DAMPING`, bounces
      off its open stop, and latches when it swings shut.
    - It is no body of the physics. Two rays along the door (at 40% and 85% of its height) and one along
      the path its edge swept since last tick find what it meets, with the car itself left out.
    - What it meets either tears it off, or sets its swing so that the point it touched goes no further
      into it.
- **On the wire** (protocol 96, and 97 for the swing):
  - `seated` gains `step`, 0 in the seat to 1 standing outside;
  - a vehicle has a `doors` component while any door is open or off: `left` and `right`, how far each
    stands open as a share of its swing, and `gone`, a bit mask by door.
- **On the client.**
  - `CarWear.swingDoors` eases each door towards the angle the server last sent, and throws one that
    has come off as debris (`throwNode`). One that came off standing open keeps only a little of the
    car's velocity, so it is left behind where it snagged.
  - `Character.seat` plays the climb for a seated rider whose `step` is above 0. It moves the body out to
    the door's spot while it ducks under the roof, and eases the step between snapshots.
  - Riders draw everyone else that way, and `game.ts` the local player.
  - A seated officer or medic is drawn in the services' colour before their profile arrives, as they will
    be once out.

## Not there yet

- Rear passengers get in and out at once: the rear doors don't open.
- A car's mirror and side indicator stay on the body when the door they sit over is thrown off.
- A swinging door does not strike people on foot.
- A door's swing leaves out the car's turning: a hard turn does not swing it out.
