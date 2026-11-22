# The helicopter's flight model

The helicopter (`KIND_HELI`) flies on a semi-realistic model, in the spirit of a home flight simulator's helicopters. It has a collective, a cyclic, pedals and a rotor with a speed of its own. It also has the effects a pilot feels: ground effect, translational lift and its shudder, transverse flow, flapback, the tail rotor's drift, vortex ring state, retreating blade stall and autorotation. It flies in the wind. The help it gives is what a real one's stability augmentation gives: it damps the rates and holds no attitude. Only the pedals' turn coordination at speed is more than that, so a mouse can fly it. The unarmed air ambulance flies on the same model with a tuning of its own ([VEHICLES-XXI.md](VEHICLES-XXI.md)): the numbers here are the gunship's. The first model, an arcade one that set the angular velocity and held whatever vertical speed Space and Shift asked for, is in [VEHICLES-II.md](VEHICLES-II.md#the-helicopter).

## Flying it

| Control | What it does |
|---|---|
| Mouse | The pedals. Hovering, the nose turns towards where you look, so the rocket pods point there. Flying ahead above about 10 m/s, the helicopter banks into the turn instead, and the tail keeps the nose on the flight path. |
| W / S | Cyclic pitch: the nose goes down (to fly ahead) or up (to slow down or flare) for as long as it is held. Let go and it **stays at that lean**: fly it back level with the other key. |
| A / D | Cyclic roll, the same way: bank and slide sideways, and roll back level yourself. |
| Space / Shift | Raise and lower the collective lever, at 60% of its travel a second. It **stays where you leave it**. |
| Click / F | Rockets. |

The HUD shows the lever (as COLLECTIVE, with the vertical speed beside it) and the rotor's speed (ROTOR, which flashes LOW ROTOR when it droops in the air).

- **Taking off.** The rotor starts stopped. Get in, raise the lever, and the engine spins the rotor up in about 3 s. It lifts off once there is enough rotor speed.
- **Hovering.** The lever hovers out of ground effect at about 53% (`HELI_HOVER_LEVER`). Near the ground it hovers on less. Nothing holds the height for you: set the lever, and touch it again as the height wanders.
- **Hovering in place.** In a level hover the tail rotor's push drifts it to the right (translating tendency), and the wind (about 6 m/s, gusting) carries it downwind. Hold a little bank against both.
- **Climbing and descending.** Full lever climbs at about 11 m/s, and a low lever sinks at a steady rate. Taking the seat of a helicopter already in the air starts the lever at the hover.
- **Flying fast.** Nose down about 25°, with the lever raised to about 70% to keep the height, it tops out at about 50 m/s. Turns at speed are banked. Let go of the stick at speed and the nose creeps up (flapback). Accelerating through about 6–10 m/s it shudders and rolls right (transverse flow). Diving past about 58 m/s, the retreating blade stalls: it pitches up and rolls left.
- **When it goes wrong.** With the engine shot dead, lower the lever: the air coming up through the disc keeps the rotor turning (autorotation), and it comes down at about 10 m/s. Near the ground, pull the nose up to flare off the forward speed, then raise the lever to cushion the landing.

## The model

All of it is `fly` in `crates/sim/src/vehicle/flight.rs`, tuned by a `Rotorcraft` (`Spec::rotor`), which for the helicopter is `FLIGHT` in `crates/shared/src/vehicles/heli.rs`. Accelerations are per unit of mass, and power is in shares of the hover's induced power.

### The air

Everything is measured through the air, not over the ground: the air's own velocity is the wind (`shared::wind`: its direction veers with the ships' wind, and `speed` gusts about 6 m/s). The wind is a function of the server tick, so the pilot's prediction flies in the same wind.

### Lift

The rotor pushes along the hull's up axis:

```text
lift = thrust · (lever − through / (pitch_flow · rpm)) · rpm² · translational · ground · vortex · stalling
```

- **`through`, the air through the disc.** This is the vertical speed through the air, plus a share (`tilt_flow`) of what the hull's lean adds. A climb or a nose-down dash pushes air down through the disc and flattens the blades' angle, so a dash needs more collective. A descent or a flare brings air up through the disc and steepens the angle, which damps the rise and fall and holds the helicopter up in autorotation.
- **`translational`, clean air.** Above about `etl_speed` (`HELI_ETL_SPEED`) the rotor gets 10% more lift.
- **`ground`, the cushion.** Up to 15% more lift with the hub near the ground, fading out by two rotor radii up.
- **`vortex`, vortex ring state.** Sinking at about `hover_inflow` with little forward speed loses up to 40% of the lift. Flying ahead out of it ends it.
- **`stalling`, retreating blade stall.** Past `stall_speed`, the lift falls away.

### Drag

Drag acts along each of the hull's axes, with the square of the airspeed along that axis.

### Rotor speed

The rotor's speed (`rpm`, a share of governed) is an energy balance:

- **What the rotor takes.** It needs the induced power to drive its downwash (less the faster it flies across clean air, more as air goes down through the disc), the power the hull's drag takes, and the blades' own drag (`profile`, with the cube of their speed).
- **What the engine gives.** The governor gives what the rotor takes, up to `power` (2.6 times the hover's induced power; less with a hurt engine, none with it shot dead). It spins the rotor up at `spool` a second.
- **The thin air.** Over the 40 m below `HELI_CEILING` (120 m over the ground under it), the engine's power falls to what a hover takes, and less above it. Climbing on, the rotor droops (LOW ROTOR) and the helicopter sinks back. A full-collective climb carries it some metres past first.
- **When the power runs short.** Asked for more than the engine gives, the rotor slows (`inertia` sets how fast), and with its speed goes its lift. Air coming up through the disc gives power back: that is autorotation. Spending rotor speed on a flare is how a dead-engine landing is cushioned.
- **With nobody at the controls**, the rotor brakes to a stop.

The rotor's speed is the only state beyond the rigid body's. It is kept in the car's `tank` and sent in `Thrust.air`, which the pilot's prediction resumes from as it does a Roadster's air. `THRUST_LOW_ROTOR` is set while it is low in the air with a pilot aboard.

### Controls

The controls act as torques on the hull's box inertia, with an authority that grows with rotor speed and lift. A rocket's blast or a collision therefore knocks the helicopter about, and the controls fight back. On the ground, with the lever down, there is little authority, so the helicopter does not rock on a slope.

- **Cyclic.** The stick is rate-commanded: held, it turns the hull at `pitch_rate` or `roll_rate`, followed at `rate_gain`. Let go, the stability augmentation only damps the rate (`hold_gain`). The hull keeps the lean it was left at, and the rotor's own moments slowly turn it.
- **Pedals.** Hovering, they turn the nose towards the look (`yaw_gain`, `yaw_rate`). Flying ahead through the air (between `coordinate_speed` and twice it), the look sets the flight path instead. The helicopter banks to turn its path towards the look (up to `bank_max`), its yaw follows the turn its bank makes, and `weathercock` swings out any slip.
- **Torque reaction.** The main rotor's drag turns the hull against it (`torque_yaw` per share of power), and the tail rotor cancels that out of its `yaw_accel`. Turning against the torque at full power is a little slower.

### What the rotor does by itself

The rotor's own moments grow with its speed and its lift:

- **Translating tendency.** The tail rotor's push, at `tail_arm` behind, pushes the whole helicopter to the right.
- **Flapback.** The nose rises in proportion to forward airspeed (`flapback`).
- **Transverse flow.** Passing through the band of translational lift, the rotor rolls the helicopter right (`transverse`). The pilot's camera shudders through the same band (`cameras.ts`, `etlBand`).
- **Retreating blade stall.** Past `stall_speed`, it pitches up and rolls left (`blade_stall` per m/s over).

## Wire and client

- **The lever on the wire.** `InputFrame.collective` (u8, protocol 90) carries the lever. The client keeps it (`client/src/sim/collective.ts`, `CollectiveLever`) for a seat with `lever: true`, starting and moving it by that kind's `rotor` row in the vehicle table (`Rotorcraft::hover_lever`, `lever_rate`). `Buttons::Collective` reads it as the helicopter's `lift`.
- **The rotor on the wire.** `Thrust.air` is the rotor's speed. Everyone's client spins the drawn rotor (`RotorModel.setRotor`) and beats the blades (`SoundInput.rotor`) with it.
- **The camera.** The pilot's seat has its own camera rig, `pilot`. The view follows the mouse and does not ease back behind the hull, since the hull turns to meet it.

## Tests

- **`crates/sim/src/vehicle/tests.rs`:**
  - The tests are flown by a test pilot (`holding`), who works the stick towards a wanted lean, as a person does by eye.
  - The rotor spins up before it lifts off.
  - The hover lever holds its height (and measures `HELI_HOVER_LEVER`).
  - The tail rotor drifts it right, and a little bank holds it.
  - Ground effect holds it up on less lever.
  - Full lever climbs at over 10 m/s, and a low lever sinks steadily.
  - Nose down it tops out near 50 m/s, and a nose-down dash needs more lever.
  - Clean air lifts it on the same lever.
  - Flapback lifts the nose, transverse flow rolls it right, and blade stall pitches it up and rolls it left.
  - It turns its nose to the look, and a turn at speed banks with the nose on the flight path.
  - Let go, the stick leaves it at its bank. A knock is damped out, and the pilot levels it.
  - A hurt engine droops the rotor and sets the low-rotor flag.
  - With the engine dead it autorotates, and a flare sets it down under 4 m/s.
  - Vortex ring state loses lift, and flying ahead escapes it.
  - It drifts downwind in the wind.
  - The ceiling holds by drooping the rotor, over a plateau too.
  - With nobody at the controls the rotor spins down.
  - Every test but the wind's and the fixture's flies in still air (`Physics::calm_wind`).
  - `scripted_flight_matches_fixture` and the client's "WASM flies where the native server does" agree on a 480-tick flight.
- **`crates/server`:**
  - A pilot spins up, lifts off with a passenger and holds a hover on the lever.
  - Everyone is told the rotor's speed.
- **Client:**
  - `collective.spec.ts`: the lever's travel and where it starts.
  - `heli.spec.ts`: the drawn rotor turns at the physics' speed.
  - `vehicle.spec.ts`: the lever comes from the input, not jump or run.

## Limits

- The wind is the same everywhere: there is no turbulence over buildings or hills, and no shelter.
- Rotors still have no collider. Nothing is chopped, and there is no rotor strike.
- There is no dynamic rollover, and the stability augmentation's rate damping cannot be switched off.
- Looking around turns the helicopter, as the mouse works the pedals. There is no free-look key.
- Not yet flown by a person: the tuning is from the tests.
