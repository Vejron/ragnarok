# Vehicles IV: the triplane

Two Fokker Dr.I triplanes, stretched into two-seaters: a pilot who flies and fires the forward guns and drops bombs, and a gunner on a ring behind. [VEHICLES-II.md](VEHICLES-II.md) covers the tank, the helicopter and driver prediction, and [VEHICLES-III.md](VEHICLES-III.md) crashes and sound; the plane is built on both.

## What you get

- **Two planes**, one red and one yellow, park after everything else at the spawn points nothing took, facing along the road whichever way has the longer clear run (see [Placement](#placement)). Walk up and press **E**: the first in flies, the second takes the rear gun, a third can't board. **R** swaps seats.
- **Flying.** **Space** and **Shift** are the throttle, like the helicopter's climb and sink: Space opens it, Shift cuts it (the blip switch: the engine drops out and the plane slows; on the ground it brakes), nothing held cruises. **The mouse steers**, as in War Thunder: the camera sits behind the plane looking where the mouse points, and the plane turns towards that heading at a limited rate, banking into the turn and levelling its wings when it arrives (the mouse can ask for up to 75° of climb or dive). **W** pushes the nose down, **S** pulls it up and **A/D** roll the wings by hand, taking their axis over from the mouse while held; with the mouse centred behind it flies level. A yellow cross shows where the guns point. On the ground it idles until Space is held, A/D steer the wheels, it gets light at about 17 m/s and is a metre clear of the ground after about 65 m with a little **S**. It cruises at 35 m/s, tops out at 45 m/s, and the engine fades out over the last 20 m below 160 m.
- **Stalling.** Below 17 m/s the wings let go: the lift fades away, the nose drops towards where the plane is going and the controls go soft. Diving out of it recovers. Climbing steeply on the blip switch is the quick way to find out. A "STALL" warning blinks below 19 m/s.
- **Forward guns.** Click or **F**: two Spandaus on the cowling fire in turn along the nose, twelve rounds a second between them, out to 300 m, drawn as bullets in flight like a rifle's. A round does 14 to a soldier, 7 to a plane and 3 to anything else (with a wheel hit as a rifle's). They heat up and jam after about four seconds of fire, and clear after about three seconds of rest (the bar bottom right).
- **Bombs.** **C** drops one from the rack under the belly, four in all, one every half second, only once more than 4 m up. A bomb leaves with the plane's speed, falls and bursts on the first thing it meets: 6 m radius, 110 damage at the centre, buildings and trees included. The pilot sees where one would land as a dotted arc. The rack fills again after three seconds standing still on the ground, or with a fresh plane.
- **Rear gun.** The gunner's ring turns all the way round and the gun lifts from −20° to +57°, but it holds fire within 34° of the nose, where the pilot's head is. 7.5 rounds a second, same damage and heat as the forward guns.
- **Fragile.** A plane has 70 hp and takes twice a car's crash damage, so flying into anything at speed or a nose-in is the end of it; a normal landing is not. Riders die with it, as in any vehicle.
- **Edge of the world.** The boundary walls are invisible, so within 120 m of the edge the plane turns back towards the middle by itself, whatever the pilot wants.
- **Sound.** A buzzy nine-cylinder rotary whose note follows airspeed and throttle (no gear changes), a propeller beat on top, the wind in the wires in the air, and the blip switch's cut with the odd "brrp" while it is held. Each round of either gun cracks and thumps; a bomb goes off like a wreck.

## How it works

### Flight, on both ends

`physics/vehicle.rs` used to say whether a vehicle flies with a bool; `Spec::lift` is now `Lift::Wheels`, `Lift::Rotor` (the helicopter) or `Lift::Wing`. A plane is a 600 kg fuselage box with ray-cast wheels: two on the axle, which steer, and a pair standing in for the tail skid, none with any engine force. Its wings are a second, weightless box, like the tank's turret, so they are solid to walls, trees, shells and soldiers.

Each tick `roll_plane` sets the wheels (steered by A/D, braked on Shift or with nobody aboard), the controller updates them, and then `wing` adds the air on top. It is a function of the body's state and the controls alone, as the helicopter's `fly` is:

1. **Thrust** along the nose: `PLANE_THRUST × throttle × power × headroom`. The throttle is full on **W**, 60% with the stick released (idle on the ground), 3% on the blip switch; `power` is the engine's condition; `headroom` fades out below the ceiling.
2. **Lift** along the body's up axis: `flying × (g × carried − PLANE_AOA_GAIN × v·up)`, at most 4 g. `flying` is 0 below 70% of the stall speed and 1 at it. `carried` is 1 above 28 m/s and on the ground, and falls with the square of the airspeed below 28 m/s, so a slow plane must raise its nose to stay up. The second term is the angle of attack: it turns the flight path towards where the nose points.
3. **Side force** from the fin, which kills sideslip, and **drag** by the square of the speed (`PLANE_DRAG`, set so full throttle tops out at 45 m/s).
4. **Controls.** The throttle is `lift` (Space − Shift, as for the helicopter's climb) and the stick is the forward axis (`CarControls::throttle`, W + 1, which pushes the nose *down*). Pitch rate follows the stick; with it centred the nose is pulled back to the horizon. A/D roll at a set rate up to a 75° bank, and with them released the wings level; a little yaw follows the bank and the stick so a banked plane turns. Everything scales with `(airspeed / 35)²`, and on the ground only takes over as the wings start to fly. The result is a wanted angular velocity that the body's follows at 6 per second, as for the helicopter. With the mouse, `CarControls::aim` (the pilot's look yaw and pitch, which are already on the wire) is turned into a direction and the same `autopilot` that turns the plane back at the world's edge supplies pitch, yaw and roll; the keys override an axis while they are held. Only the pilot's look is read, so no new wire field was needed.
5. **Stall.** Below the stall speed the nose is pulled towards the velocity at up to 1.5 rad/s.
6. **Nobody aboard.** No thrust, and an *autopilot* holds the wings level with the nose 0.2 rad down, so an abandoned plane glides on and comes down.
7. **The edge.** Within `PLANE_EDGE_MARGIN` of the world's edge the same autopilot takes the controls from the pilot, completely by a third of the way in, and turns the plane back towards the middle.

The autopilot (`autopilot` in `physics/vehicle.rs`) turns towards a world direction by banking into the turn and pulling once banked, takes a target anywhere behind as level off the wing that is down (so it turns flat instead of looping), and eases off near the stall.

`CarSim` in `client/src/sim/vehicle.ts` mirrors all of it, as it does the helicopter, so the pilot's own plane is predicted and reconciled like any other vehicle. `CarSim` and `VehicleDriver` now know the world's half extent for the edge rule.

### Placement

Cars, tanks and the helicopter are placed as before. Then `place_planes` looks at every spawn point left over, facing both ways along the road. For each it measures the runway with `Physics::runway`: rays at two heights along the middle and out at each wingtip, against the static world, and the ground's height every 4 m, which must stay within 1.5 m. Parked vehicles shorten it too. The longest runways win, and a plane is never parked within the first `PLANE_RUNWAY` (80 m) of another's. If nothing has 80 m, the best there is is used and the server logs a warning. The shipped fixture world gives runways of 216 m and 109 m.

### Weapons

`Gunnery` has two new pieces for a plane:

- `work_cockpit` (seat 0) fires the forward guns and drops bombs. The guns' rounds are hitscans through `combat::hitscan`, which is the tail of the rifle's `fire_gun` taken out into a function. It takes a `Round` (damage to soldiers, to vehicles and to planes, weapon, range), ignores the firing vehicle, and rewinds soldiers to the pilot's view as rifle shots are. A round sends a `WEAPON_MG` shot event only if it hit something; the tracers are drawn by every client from the shot counter, which saves the eight-event budget per snapshot. Bombs are `PROJECTILE_BOMB`s in `world.rockets`, stepped by `step_shells` with the shells (gravity, then a ray, then a burst) and burst as `WEAPON_BOMB`.
- `work_ring` (seat 1) works the rear gun like the helicopter's nose gun, but with no yaw limit, and fires the same hitscan.

Heat is `Gun::heat` (the ring) and `PlaneArms::heat` (the forward guns): plus `PLANE_GUN_HEAT` (or `PLANE_REAR_HEAT`) a round, minus 1 a tick, jammed at 255 until 77.

## Wire

Protocol **18**:

- `KIND_PLANE` (6), two seats, 70 hp. `MAX_VEHICLES` is 11.
- `VehicleState` and `VehicleDelta` gained `arms`: `nil` for anything but a plane, else `Arms(shots, heat, rear_heat, bombs, flags)`, and `nil` in a delta when unchanged. Flags: `ARMS_CUT`, `ARMS_JAMMED`, `ARMS_REAR_JAMMED`. A plane's `Turret` is its ring.
- `WEAPON_MG` (10) and `WEAPON_BOMB` (11), `PROJECTILE_BOMB` (5).
- The byte fixtures in `protocol.rs` and `codec.spec.ts` were regenerated and now include a plane. [NETWORKING.md](NETWORKING.md) has the fields.

## Client

| File | What changed |
|---|---|
| `sim/vehicle.ts` | The plane's `Spec`, `rollPlane`, `wing` and `autopilot`, the world's bound |
| `sim/physics.ts` | `extraSolid`: the second solid of a kind, the tank's turret or the plane's wings |
| `sim/grenade.ts` | `dropTrajectory`: a bomb's fall, stepped like `step_shells` |
| `net/codec.ts`, `net/interp.ts` | `Arms`, the new ids; the shot counter and gauges come from the nearer snapshot |
| `render/plane.ts` | `PlaneModel`: the propeller (slower on the blip switch), the control surfaces (moved by the measured turn rates), the ring and its gun, both forward guns' recoil, the bombs on the rack, heads |
| `render/effects.ts`, `render/rockets.ts` | `round` (a tracer streak that flies out at 700 m/s), `strike`, the bomb in flight |
| `render/tyres.ts`, `sim/slip.ts` | The plane's wheels and tail skid, and castor-oil smoke from under the cowling |
| `game.ts` | The pilot's camera (12 m back and 2.4 m up, free look that eases back behind the plane), the gunner's limits, tracers for every round, the bomb sight, the gauges, prompts |
| `App.vue`, `TouchControls.vue` | The `aviator` seat: hints, the gunsight, the heat bar, the bombs, the stall warning, touch **FASTER**, **SLOWER** and **BOMB** buttons |
| `sim/engine.ts`, `vehicleaudio.ts` | The rotary's mix and voice, the blip switch, `machineGun` |

## The model

`client/public/models/plane.glb` is built by `tools/vehicles/build_plane.py` (Blender, headless; `-- --preview DIR` renders six views). It is a Dr.I in silhouette in about 2 400 triangles:
- three staggered wings with I-struts and cabane struts
- a round cowling with the cylinders showing underneath, a two-blade propeller
- the axle wing, the tail skid, the comma rudder and the tailplane
- Iron Crosses
- twin Spandaus on the decking, a second cockpit with a ring-mounted Parabellum, and four bombs on a rack

The game looks up these nodes:
- `prop`
- `gun_L` and `gun_R`, each with its origin on its muzzle (`PLANE_GUN_PIVOTS`)
- `turret`, and `gun` under it
- `elevator`, `rudder`, `aileron_L`, `aileron_R`
- `bomb_1` to `bomb_4`

The `paint` material is tinted per plane. It sits level, as its box does, where the real one sat nose-high on its skid.

```
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_plane.py
```

## Tests

`cargo test --workspace` and `cd client && npm test`.

- `physics/vehicle.rs`:
  - a parked plane sits on its wheels and idles when a pilot climbs in
  - full throttle lifts it off within the runway
  - it cruises level near 35 m/s and tops out at 42–46
  - pulled up on the blip switch it stalls, drops its nose and recovers
  - Space and Shift are the throttle: Shift slows, nothing cruises, Space speeds up
  - S pulls the nose up and W pushes it down, and letting go levels it
  - A/D bank and turn it, and releasing levels the wings
  - without a pilot it comes down
  - the ceiling holds
  - it turns back before the edge of the world without hitting the ground
  - the wings are solid to their tips
  - the runway ends at a wall, a wingtip and the boundary
  - a scripted 900-tick flight matches a fixture, which `vehicle.spec.ts` flies in WASM to within 1 cm
- `mechanics/vehicles.rs`:
  - both planes are placed in the test world and the fixture world with full runways, never on each other's or a parked vehicle's
  - a pilot and a gunner take one off and a third stays behind
  - a gentle landing does no harm
  - flying into the ground wrecks it
- `mechanics/gunnery.rs`:
  - the forward guns take turns, hit a soldier ahead and never the plane itself
  - they jam when hot and clear when cool
  - a bomb carries the plane's speed and bursts where it lands
  - the rack holds four, holds on to them on the ground and refills
  - the rear gun turns all the way round but won't fire at the pilot, hits a soldier behind, and only works for its gunner
  - rounds hurt a plane more than a car
- `delta.rs`, `protocol.rs`, `codec.spec.ts`: arms travel in full states and deltas and are kept until they change; malformed arms are rejected; the fixtures match.
- `vehicle.spec.ts`: the WASM flight fixture; the replay test now includes a plane; `controlsOf` reads W/S as the stick, Space/Shift as the throttle, and reads the mouse's look as the aim. `the_mouse_turns_it_towards_where_the_pilot_points_banking_into_the_turn` and `the_mouse_climbs_and_dives_it` cover the mouse steering.
- `plane.spec.ts`:
  - the nodes are on the game's pivots
  - it stays under 3000 triangles
  - the forward guns fire along the nose from their muzzles
  - the ring gun turns all the way round and its barrel ends at its muzzle
  - the propeller spins with a pilot and slows on the blip switch
  - spent bombs disappear
  - the rudder and ailerons move the right way in a turn
- `engine.spec.ts`, `vehicleaudio.spec.ts`: the rotary has no gear steps; the blip switch quietens and lowers it; the wind blows in the air only; plane voices start and stop with no leaks; machine-gun rounds share a one-shot slot and don't crowd out an explosion.
- `tyres.spec.ts`, `grenade.spec.ts`: the plane's wheel layout; a bomb's fall lands where the arithmetic says.

## Not done, or not checked

- **Not flown in a browser against a server.** Headless Chrome 153, the build Playwright ships here, fails the WebTransport opening handshake against the dev server (`ERR_QUIC_PROTOCOL_ERROR`) in a bare probe page. That happens before any game message, and nothing in the transport or TLS changed. Instead, the client was run in SOLO mode with plane states fed into its interpolator: both planes drew in the real scene, the guns' counters drew tracers and played the rattle, and a bomb burst through `showShot`. Taking off, boarding and the HUD in a live session were only exercised by the Rust tests.
- Feel, tuning and the sound have not been judged by a person at a keyboard. The numbers in `PLANE_*` and the mix are first guesses that the tests hold within broad bounds.
- Plane-on-plane shots are not lag-compensated: a round hits the other plane where the server has it now, and its tracer is drawn from where the shooter sees their own plane. Soldiers are rewound, as for rifles.
- A rear gunner's camera looks up at most 0.6 rad (the input's own limit), short of the ring's 1.0.
- No spin, no torque from the rotary, no flaps or trim; the edge turn-back is blunt. The plane sits level on the ground instead of on its tail.
- A crew member who steps out in the air falls; there is still no fall damage. There is no parachute.
- Bombs don't whistle, and a bomb dropped onto the plane's own path can't hit it (it ignores its plane).
