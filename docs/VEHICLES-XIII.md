# Vehicles XIII: the snowracer

The snowracer is a steerable sled for two: a padded seat on a steel tube frame over two long runners, and a short ski in front that it steers by handlebars. It is built for the ski slopes, where gravity takes it down about as fast as a snowmobile drives and it flies far off every bump. The real thing has no motor. This one has a small one under the seat, so that it can get about on the flat and creep back up a piste.

## In play

- **The snowracer** (`KIND_SNOWRACER` = 29) is all but indestructible:
  - **Crashes do it no harm.** Its body is `SLED`, which takes no crash damage. Slammed nose first into the ground at 25 m/s, which hurts a jeep, it comes off whole.
  - **It has 255 hp**, the most a vehicle can have, so it takes a great deal of fire to wreck.
  - **Nothing on it can be shot out.** It has no engine or fuel modules, and its runners are not car wheels (`car_wheels: false`), so they cannot be bent or shot off.
  - **Its riders are still in the open**, and can be shot off it.
  - **It cannot tow** a gun, since only a car with wheels can.
- **Placement.** It appears only where it is placed in the map editor (**Snowracer**, Z in the palette). The automatic line-up on worlds without editor placements does not include it.
- **Seat 0 is the rider**, who steers. W runs the motor, S brakes and jump is the brake claws (the handbrake). The steering lock fades quickly with speed, as the snowmobile's does, so at a slope's pace it carves rather than turns.
- **Seat 1 is a passenger** sitting behind. Both ride in the open and can fire their own guns.
- **Down a slope** it picks up speed with nothing but gravity, to well past its motor's top speed. Measured from the top of a 300 m run, against a snowmobile flat out:

  | Slope | Snowracer, coasting | Snowmobile, full throttle |
  |---|---|---|
  | 10° | 23 m/s (84 km/h) | 26 m/s |
  | 20° | 32 m/s (116 km/h) | 37 m/s |
  | 30° | 35 m/s (126 km/h) | 36 m/s |

  The snowmobile's engine keeps it a little ahead on gentler slopes, and on steep ones they are level.
- **On the flat** the motor takes it to 6 m/s (22 km/h). Let go, it slides to a stop in about eleven seconds.
- **Up a slope** the motor creeps up 20° and 25° at about 4 m/s, and stalls on 30°. Ride the lift, or walk, up anything steeper.
- **It stops on a gentle slope.** The snow's friction holds it still on one gentler than about 3°, and lets it go on a steeper one.
- **At full lock at speed** its tail washes out and it spins round, but it stays on its runners.
- **Off a bump it flies.** In the air its nose swings onto the line it is flying and its roll comes back to level, so it neither dives nor tumbles. A share of its weight is held up, so it sails further than it would fall. Off a 1 m kicker:

  | Speed | Snowracer | Snowmobile |
  |---|---|---|
  | 15 m/s | 7 m, 0.7 s in the air | 7 m, 0.6 s |
  | 20 m/s | 17 m, 1.1 s | 14 m, 0.9 s |
  | 30 m/s | 48 m, 2.1 s | 34 m, 1.4 s |

- **In the air the rider steers it.** A and D turn it, up to 1.5 rad/s, banking into the turn. W leans its nose down off its line and S lifts it, by up to 20°, to set it down on a slope.
- **It sinks in open water**, being too small and slow to plane over it as the snowmobile does.

## Sliding on runners

A car rolls freely: coasting, only a light brake (`COAST_BRAKE_PER_KG`) and its `linear_damping` hold it back. A sled slides on runners, so it gets a new `Spec` field, `runners: Option<Runners>`. Every other kind has `None`.

- **`Runners { mu, drag }`** is the soldier's skis' model (`SKI_MU_SNOW`, `SKI_DRAG`). Down a slope a sled reaches the speed at which gravity's pull, `g·(sin θ − mu·cos θ)`, equals `drag·v²`.
- **`on_runners`** (in `crates/sim/src/vehicle/flight.rs`) applies it each tick in `step_cars`:
  - **The friction** works against the slide over the ground, in proportion to how squarely the sled presses on it, and only while it is down. It never reverses the slide.
  - It leaves alone the speed along the hull's up axis. Rapier's ray-cast wheels keep a little of it on a car at rest (about −0.06 m/s), and friction spent on it would let a sled creep down any slope.
  - **The air's drag** works on the ground or off it.
- **A sled has no coast brake**, because its runners' friction does that job.
- **A sled has no downforce** (`assist`), so it leaves the ground over a crest instead of being held to it.
- **In the air** `fly_runners` steers its spin towards what is wanted, at `RUNNERS_AIR_GRIP` (8 per second):
  - **Its nose** swings onto its line at `Runners::follow` (5 per second), offset by the throttle times `lean` (0.35 rad). Its roll comes back to level, banked by the steering.
  - **The steering** turns it at `air_turn` (1.5 rad/s).
  - **`float`** (0.45) holds up that share of its weight, so in the air it falls at about 5.4 m/s².
  - Without any of this, the ski and runners left the lip one after the other and pitched it nose down 30–45°, or flipped it up.
- **Its `linear_damping`** is 0.01, so the drag is what sets its top speed.

The snowracer's runners are `mu` 0.05, a waxed runner on a packed piste, and `drag` 0.0006. The drag is well under a skier's (0.005), so that it keeps up with the snowmobile. The snowmobile has only its light `linear_damping` (0.05) against it, so down a long run it goes on gaining speed past its 20 m/s top. On a run far longer than 300 m both would reach about 70 m/s on 20°.

## The definition

`crates/shared/src/vehicles/snowracer.rs` holds everything about it.

- **The body** is a 0.6 × 0.3 × 1.5 m box at 550 kg/m³, about 150 kg with its riders.
- **Two axles** carry it. The front pair, 0.5 m apart, stand in for the one steering ski, and the rear pair are the runners.
- **The motor** pulls 700 N (about 4.7 m/s²) up to `SNOWRACER_MAX_SPEED`, 6 m/s.
- **It rides** `SNOWRACER_RIDE_HEIGHT` (0.202 m) above the ground, which `ride_heights_differ_between_cars` measures.
- **Its handling** is a `slide` of 0.5, so its runners let go in a hard turn at speed, `roll` 1.0 and a low `com_drop` of 0.2.
- **Its hull** sits only 5 cm off the snow, so its whole front half is swept up (`nose` 0.75). With less, its front edge dug into a kicker, cost it a third of its speed and threw it into a roll.

## The client

- **`client/src/kinds/snowracer.ts`** is the kind:
  - The rider's role is `driver`, not the snowmobile's `sled`, since it has no boost.
  - The sound is a tiny two-stroke's buzz, soon at its top.
  - It has a small exhaust and no snow roost, which is a track's.
- **`SNOWRACER_WHEELS`** in `client/src/sim/slip.ts` lays one ski mark down the middle and a runner's mark either side.
- **It is a plain `CarModel`** (`'snowracer'` in `render/car.ts`). Nothing in it rolls.

## The model

`tools/vehicles/build_car.py` builds `client/public/models/snowracer.glb`, about 900 triangles. It follows the classic steerable sled:

- **The frame** is bent steel tube in the car's paint. A low rail is riveted along each runner and swept in to the steering head, and a hoop rises from the runners' tails to carry the seat. Slanted tubes are drawn by a `tube(part, a, b, radius, mat)` helper.
- **The runners** are long black strips, their tips turned up a little in front and their tails curled up behind. They are part of the body.
- **The seat** is a long padded black one for two, with a white flash on its top.
- **The steering** is a sprung column under a short head tube with a small lamp, up to BMX-style bars with a padded crossbar and grips.
- **The brake claws** are toothed steel plates on levers either side.
- **The motor** sits under the seat.
- **The steering ski** is short and broad, with an upturned tip. It hangs from `pivot_FL` in the middle, and there is no `pivot_FR` and no `wheel_*`.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only snowracer --preview <dir>
```

## Tests

- **`crates/sim/src/vehicle/tests.rs`** runs it down a `piste(deg, length)`, a slope that rises from z = 0 to a flat top, against the snowmobile.
  - It creeps up 25° but not 30°.
  - It tops out and slides to a stop on the flat.
  - It carves and spins at any lock down a piste without rolling.
  - It rests on 2° and slides off 10°.
  - Over a `kicker(height, length)` it flies further than the snowmobile with its nose on its line, and lands upright.
  - The rider turns it and leans it forward or back in the air.
- **`crates/server/src/mechanics/vehicles/tests.rs`** drives it nose first into the ground at 25 m/s: a crash that hurts a jeep leaves it whole.
- **`client/src/__tests__/snowracer.spec.ts`** checks the model:
  - its seat markers against the shared seat heads;
  - the one steering ski, with nothing that rolls;
  - the triangle budget and the headlamp.

## Protocol

No message or component changed. `KIND_SNOWRACER` (29) is a new value in an existing field, so `PROTOCOL_VERSION` stays where it was. The `Spec` is the sim's own and does not go to the client's tables.

## Not yet

- **The runners slide the same on any ground**, a road included. It is meant for winter worlds.
- **It cannot be carried up a ski lift.**
