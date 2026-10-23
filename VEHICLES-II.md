# Vehicles II: the tank and its gunner

First part of roadmap stage 3 (see [design.md §14](design.md#14-roadmap)): a tank with a driver and a separate gunner seat, then driver-side prediction (see the end) and the helicopter ([below](#the-helicopter)). [VEHICLES-I.md](VEHICLES-I.md) covers the car this builds on; [NETWORKING.md](NETWORKING.md) has the wire format.

## What you get

- Two tanks park on the roads next to the six cars. Walk up and press **E**: the first player in drives, the second takes the gun. A third can't board (two seats).
- **Driver:** W/S gas and brake, **A/D turns the tank on the spot** (or while driving), Space is the handbrake. Top speed is 12 m/s; the car's is 30.
- **Gunner:** aims with the mouse and fires with a click or **F**. The camera sits behind the turret on the line of the barrel, so the reticle at the screen centre is where the barrel points; the shell falls from that line, so there is no aiming aid and long shots are judged by eye. The reticle goes dashed while the gun reloads (1.5 s).
- The turret turns on the hull at 1.4 rad/s and the barrel nods at 0.9 rad/s, between −7° and +29°. It is *stabilised*: it follows the gunner's world heading, so a turning hull doesn't drag the aim.
- A shell is a projectile. It leaves the muzzle at 140 m/s and falls under gravity (about 5 m of drop at 100 m, 20 m at 200 m, so long shots need the barrel raised), takes about 0.7 s to cover 100 m, and explodes at the first wall, building, vehicle or stickman it meets, or after 6 s where it is: a 7 m blast throws stickmen away (up to 18 m/s, with a hop) and shoves vehicles, the shooter's own tank included. Everyone sees a muzzle flash, the shell in flight and a ball of fire where it lands.
- Stickmen can still stand on the tank and ride it, and it knocks them back as a car does. Tanks are solid on the client too, turret included.
- **R switches seat** while seated: a lone crewman moves between driver and gunner, and a driver who left a gap can be replaced by the passenger. With every seat taken it does nothing.

## The gunner is a separate mechanic

`mechanics/gunnery.rs` is a new `Mechanic`, run after `Vehicles` and before `Movement`. `Vehicles` knows seats, driving and hulls; `Gunnery` only knows guns. A vehicle that has a `Gun` (`game::Gun`: turret yaw, barrel pitch, reload, shot counter, last range) gets worked by whoever sits in its seat 1. Another vehicle with a gun (a helicopter chin gun, say) would only need the same seat and a `Gun`.

Per tick, for each vehicle with a gun:

1. Read the gunner's input frame. Its `yaw` is a world heading, so the target turret angle is that heading minus the hull's heading. The turret steps towards it by at most `TANK_TURRET_SLEW × dt`, taking the short way round. The barrel steps towards `pitch`, clamped to the barrel's travel.
2. If fire is held and the reload is up: a shell is added to `world.rockets` (kind `PROJECTILE_SHELL`) at the muzzle, `TANK_BARREL_LENGTH` along the barrel from the ring, moving at `SHELL_SPEED`. If the barrel is poking into a wall (a ray from the ring to the muzzle hits something) it bursts there at once. `Gunnery` steps every shell each tick before the guns: gravity, then a ray along the step (`Physics::cast_ray`, which skips the firing tank's body) and a test against each standing player, since stickmen are not physics bodies (a hit if their centre passes within 0.8 m of the path before the wall). It bursts at the first hit with a `WEAPON_SHELL` shot event and the blast, and expires after `SHELL_TTL_TICKS`.
3. At the impact point: `Physics::blast_cars` shoves every vehicle in range, and every standing player within 7 m gets a knockback of `18 m/s × (1 − distance / 7 m)` away from the blast, plus a hop. That is the same `Character::knock` the car uses, so the client's reconciliation already knows how to replay it.
4. The shot counter goes up, the range of the flight is saved, and the reload restarts (90 ticks). A held trigger therefore fires once per reload; the jitter buffer strips fire from repeated inputs, so an underrun cannot add a shot.

Nobody in the gunner seat means nobody works the gun: a driver holding fire does nothing.

## Recoil

Each shot gives the hull a 1.6 m/s kick backwards along the barrel (level, so it doesn't dig the nose in), applied on the server by `Physics::kick_car`, so it is real motion everyone sees. The barrel also slides back 0.5 m and eases home over 0.35 s on the client; that part is purely visual.

## The tank's physics

`physics/vehicle.rs` now describes a vehicle by a `Spec` (box size, wheel layout, density, engine, steering). The car's numbers are unchanged. The tank:

- A 2.6 × 1.0 × 5.2 m hull at 1000 kg/m³ (13.5 t, ten times a jeep, so it throws cars aside), six wheels on three axles (the suspension tuning scales with mass), 88 kN of engine over all wheels, top speed 12 m/s.
- **No steered wheels.** Skid steering is done by setting the yaw rate directly: it ramps towards `±1.1 rad/s × stick` at 3 rad/s², but only while at least one wheel touches the ground. The wheels' side grip fights any spin every tick, so the rate is kept in the car's state and written back each time, rather than nudged from whatever grip left. With the stick released, grip and damping stop the turn.
- The handbrake and brake act on all six wheels (a car only brakes its rear wheels with the handbrake).

## Protocol changes

`PROTOCOL_VERSION` is now **5**. Old clients get the "reload" banner.

- `InputFrame` gained `pitch` (milliradians, + looks up) and `BUTTON_FIRE` (8). Every player sends their look pitch; only a gunner's means anything.
- `VehicleState` gained `turret`: `nil` for cars, else `[yaw i16, pitch i16, shots u8, range u16]`. The range is in decimetres. A car pays one byte for the `nil`. The worst-case snapshot test now gives every vehicle a turret and a full seat list, and still fits 1200 bytes.
- `KIND_TANK = 2`. A tank has two seats.
- The byte fixtures in `protocol.rs` and `codec.spec.ts` were regenerated and now contain a car and a tank.

## Client

| File | What changed |
|---|---|
| `sim/consts.ts`, `net/codec.ts` | Version 5, tank constants, `pitch`, `BUTTON_FIRE`, `turret` decoding |
| `net/interp.ts` | Turrets blend by angle; the shot counter and range come from the nearer snapshot, so a shot is drawn with the turret that fired it |
| `sim/input.ts` | `pitch` in the sample, left mouse and F as the trigger, `clampPitch` |
| `sim/physics.ts` | A tank gets a wider collider than a car |
| `render/tank.ts` | The tank model, loaded from `models/tank.glb`: wheels that roll per side (so a pivot turn spins them opposite ways), a turret and a barrel that follow the snapshot, driver and gunner heads |
| `render/effects.ts` | Muzzle flash, rifle tracer and explosion ball |
| `render/car.ts` | `VehicleModel`, the interface both models implement |
| `render/scene.ts`, `game.ts`, `App.vue` | Gunner camera, tank follow distance, the prompts ("E: drive the tank" / "E: man the gun"), reticle and reload state |

On taking the gunner seat, the camera starts where the barrel already points. While there, the camera's pitch is clamped to the barrel's travel so the view never asks for what the gun can't do. The shot effects are keyed to the turret's shot counter in the interpolated state, so they appear with the turret as it is drawn.

## Decisions worth knowing

- **A real projectile, on the existing rocket wire.** Shells share `world.rockets` and `ProjectileState` with rockets and grenades (kind 2), so the client interpolates them like those and no protocol bump was needed. The server owns the flight and nothing is predicted. `Turret.range` stays on the wire but is always 0. Shells are not lag-compensated: they hit where the server has things now.
- **The gun aims with the hull's yaw only.** Hull pitch and roll are ignored when the turret targets a heading, so a tank on a steep slope has its aim off by the slope. The barrel itself is drawn on the hull's true orientation.
- **The turret is solid, as a cylinder.** A second collider on the hull body (`TANK_TURRET_RADIUS`, `TANK_TURRET_HALF_HEIGHT`, centred on the ring) with no density, so the hull's mass and balance are as before. A cylinder looks the same at any heading, so nothing has to follow the gun. It stops stickmen and shells and can be stood on like the hull; the barrel is still not solid. The client mirrors it in `VehicleSolids` (`sim/physics.ts`) and on the predicted body.
- **The muzzle check starts at the turret ring.** A ray from the ring to the muzzle catches a barrel poking into a wall, so the shell bursts at the wall instead of starting on its far side.
- **Gunner input is not predicted.** The turret is as laggy as any server-driven object: a round trip plus the interpolation delay. That's the same small delay as a driver's. Predicting the turret is easy later because it is a pure function of the gunner's input.

## Tests

`cargo test --workspace` and `cd client && npm test`.

- `physics/vehicle.rs`: a tank settles and tops out near 12 m/s; turns on the spot without drifting; stops turning when the stick is released; turns and drives together; `cast_ray` sees walls, ignores its own hull and respects range; a blast shoves nearby cars and misses far ones.
- `mechanics/gunnery.rs`: the turret follows the gunner and the barrel stops at its limit; slewing is rate-limited; a shell is in flight for a while, falls, and then hits and throws a stickman in its path; a raised barrel reaches further than a level one; the gun reloads between shots and misses a faraway bystander; no gunner means no shooting; the first thing in the way takes the shell.
- `mechanics/vehicles.rs`: a tank takes two crew and refuses a third and reports its turret; all nine vehicles are placed and settle in the shipped world.
- `protocol.rs`, `codec.spec.ts`: matching fixtures and the size bound.
- `interp.spec.ts`: turrets blend by angle across ±π and the shot counter steps.
- `predict.spec.ts`: a tank's collider is wider than a car's.

## Not done, or not checked

- **Not exercised in a browser.** Camera feel, the reticle, the tank model and the shot effects are untested by eye. The server places 9 vehicles in the shipped world.
- Shells hurt people (a direct hit kills) and vehicles (a tank takes three, a car one or two), and a destroyed vehicle burns for ten seconds, then returns to its home. See design.md §5.6.
- A shell that lands next to the shooter's own tank still shoves it (the ray ignores the firing hull, the blast doesn't).
- A tank on a steep slope aims by the hull's yaw only (see above).
- Driver prediction and the helicopter (below) have not been tried by eye in a browser either.

## The helicopter

One helicopter stands among the vehicles. Walk up and press **E**: the first in is the pilot, the second takes the nose gun, and a third can't board.

- **Pilot:** W/S lean the rotor forward and back, A/D turn (clockwise on D), **Space climbs**, **Shift sinks**. Let go of everything and it hovers where it is, whatever its speed. Top speed is about 25 m/s in a full lean, and it won't climb past 120 m.
- **Gunner:** seat 1 works the chin gun of the Mi-24 (a four-barrel autocannon under the nose). The mouse aims it and a held click or **F** fires three explosive rounds a second; each flies and falls like a tank shell and bursts with a third of its radius and a third of its damage. **R** swaps between the pilot's and gunner's seats when the other is free, as in a tank. It swings 57° either side of the nose and from 43° down to 11° up, in the hull's frame, so the camera stops at those limits. As in a tank, their stick does nothing.
- **Leaving:** E steps out beside the hull. In the air that is a fall; there is no fall damage yet.
- **Empty:** with nobody in seat 0 the rotor is off and it drops like a stone, then sits where it lands.

### Flight model

There are no wheels. `fly` in `physics/vehicle.rs` is a function of the body's state and the controls only, applied once per tick before the rigid-body step:

1. **Lean.** The wanted up axis is straight up, tilted forward by `throttle × HELI_TILT` and sideways by `steer × HELI_BANK`. The angular velocity that carries the hull's up axis there is `up × wanted × HELI_LEVEL_GAIN`, and the body's angular velocity follows it at `HELI_RESPONSE` per second. With no stick it levels out, also after a knock.
2. **Turn.** The yaw rate is set straight from the stick, `HELI_YAW_RATE` at full lock, as for a tank.
3. **Rotor.** A force along the hull's up axis, big enough to cancel gravity however far it leans (divided by the up axis' height, at least 0.5), plus the acceleration that moves the vertical speed towards what the climb input asks for: `HELI_CLIMB_SPEED` up, `HELI_SINK_SPEED` down, zero when neither is held. The force is added to Rapier's force accumulator, which is integrated with gravity, so a hover is exact. It is reset every tick.
4. **Drag.** Linear damping 0.2 is what limits the speed: a full lean pushes about `g × HELI_TILT` horizontally.
5. **Ceiling.** Climbing fades out over the last 20 m below `HELI_CEILING`, and above it the rotor pushes down.

Because the model keeps no state beyond the body's, the client's copy needs nothing new on the wire: `drive` carries velocity and angular velocity as it does for a car, and its `steer` and `spin` are zero.

### What else changed

- Protocol 7: vehicle kind 3, two seats. The version bump is for old clients, which would draw an unknown kind as a car.
- `CarControls` gained `lift` (jump +1, run −1) and `powered` (a pilot is aboard). A car ignores both. The mapping from input to controls is in `Vehicles::on_tick` and `controlsOf` in `client/src/sim/vehicle.ts`.
- `MAX_VEHICLES` went from 8 to 9, and a snapshot with the maximum is still one datagram.
- **Prediction.** The pilot's helicopter is predicted like a car or a tank: `CarSim` has a rotor branch with no wheel controller. `scripted_flight_matches_fixture` and `WASM flies where the native server does` run the same 480-tick flight on both and agree to under a centimetre.
- **Drawing.** `render/heli.ts` loads `heli.glb`, built by `tools/vehicles/build_heli.py` (a Hind in silhouette, about 2300 triangles: double cockpit, engines, stub wings with pods and missiles, tail, tricycle gear, a five-blade rotor and a three-blade tail rotor). Heads show in both cockpits. The `rotor` and `tail_rotor` nodes spin while seat 0 is taken, and `turret` and `gun` swing and nod with the `Turret` angles of the snapshot. The camera sits 16 m behind and follows the hull's heading.
- **Nose gun.** `Gunnery` gives a helicopter a `Gun` like a tank's, and `work_autocannon` works it from seat 1. The gunner's look direction is taken in the world and expressed in the hull's frame (`CarPose::local_dir`), so the gun holds its aim while the helicopter leans and turns. It slews at `HELI_GUN_SLEW`, limited by `HELI_GUN_YAW_LIMIT` and `HELI_GUN_PITCH_MIN/MAX`. A round is a shell like the tank's (`PROJECTILE_CANNON`, `SHELL_SPEED`, same gravity and lifetime), fired every `HELI_GUN_COOLDOWN_TICKS` and scattered by `HELI_GUN_SPREAD`. It bursts with `CANNON_RADIUS`, `CANNON_DAMAGE` and `CANNON_KNOCK` (a third of the tank shell's radius and damage), and the burst event is `WEAPON_AUTOCANNON` (6). The client draws the rounds as small slugs and flashes and kicks the barrels when the gun's shot counter ticks, as for a tank.

### Tests

- `physics/vehicle.rs`: it hovers exactly; without power it falls and rests; climb and sink follow the input and letting go holds altitude; leaning forward flies at 12–30 m/s without losing height; the stick turns it; it levels out after a roll; the ceiling holds.
- `mechanics/vehicles.rs`: a pilot and one passenger lift off together, a third is refused, and it hovers when the stick is released.
- `mechanics/gunnery.rs`: the nose gun follows the gunner within its travel, throws small shells that burst a third as wide as a tank's, fires at its rate, and only seat 1 works it.
- `heli.spec.ts`: the model has its nodes on the game's pivot, stays low-poly, aims with the turret angles, and spins up with a pilot.

### Limits

- Rotors have no collider and no effect: the hull box is the only solid. Nothing is chopped.
- No fall damage and no crash. A pilot can step out at 100 m.
- There is no ground effect, autorotation or stall: the model is deliberately an arcade one.
- The nose gun has unlimited ammunition and never overheats. Protocol version 13 added `PROJECTILE_CANNON` and renamed weapon 6 to `WEAPON_AUTOCANNON`.
- The model was checked in Blender preview renders, not by eye in the browser.

## Driver-side prediction

The driver's own vehicle, car or tank, is simulated on the client with the same Rapier vehicle controller and reconciled against snapshots like the stickman. It removes the round trip and the interpolation delay from steering and throttle.

- **Same code on both ends.** `CarSim` in `client/src/sim/vehicle.ts` is `drive_car` and `step_cars` of `physics/vehicle.rs` written again for Rapier JS: the same `Spec`s, wheel tuning, steering, engine and brake rules, and the tank's directly set yaw rate. `controlsOf` is the server's input-to-controls mapping.
- **Cross-language fixture.** `scripted_drive_matches_fixture` (Rust) and `WASM vehicles drive where the native server does` (TS) run the same 480-tick drive on flat ground, for a car and a tank, against the same checkpoints. The WASM build agrees to about 0.1 mm. That was the question this stage hinged on: the WASM Rapier is an older release than the server's, and the vehicle code is far more sensitive than the character controller.
- **State on the wire.** Protocol 6 adds `drive` to the snapshot, sent to the driver only (about 20 bytes): velocity, angular velocity, front-wheel angle and a tank's yaw rate. See [NETWORKING.md](NETWORKING.md).
- **Reconciling.** When a snapshot arrives with `drive`, `VehicleDriver.reconcile` sets the body to that state, drops acknowledged inputs and replays the rest. The distance between where the vehicle was drawn and where the replay lands becomes a position and rotation offset that decays at 10/s; more than 3 m snaps (respawn, unflipping).
- **Own world.** The prediction runs in its own Rapier world, not the stickman's, so stepping one never moves the other. The static scene is built twice from the same world data.
- **What it sees.** Other vehicles are boxes at the newest snapshot pose, so ramming one is predicted against where it was half a round trip ago.
- **Tests.** `vehicle.spec.ts`: a run replayed over a quantized server state ends within 10 cm for both vehicles (about 5 cm measured), and leaving the seat stops the prediction. `the_driver_steers_and_passengers_ride_along` also checks that only the driver gets `drive`.

Known limits:

- A rider standing on a car being driven is drawn from interpolated entity positions, while the car is drawn from the prediction, so the rider trails the roof by about the interpolation delay at speed.
- The server holds a car at the last input on a jitter-buffer underrun but the client does not know it did. That is one tick of mismatch, corrected by the next snapshot.
- Wheel suspension state is not on the wire, so the first replayed tick after a correction starts from whatever the client's springs were doing. It is a sub-millimetre effect in practice.
- Boarding the vehicle shows the interpolated pose until the first snapshot with `drive` arrives (about a round trip), then jumps to the predicted pose.

## The model

`client/public/models/tank.glb` is built by `tools/vehicles/build_tank.py` (Blender, headless; `-- --preview DIR` renders five views). It is a T-55 in silhouette (low cast turret, long glacis, five road wheels with sprocket and idler a side, fuel drums on the tail, a 100 mm gun with a bore evacuator) in about 1 900 triangles. The game looks up `wheel_L_<n>` / `wheel_R_<n>`, `turret` and `gun` by name; the `paint` material is tinted per tank. The physics hull is unchanged, so the tracks overhang the collider by a few centimetres.

```
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_tank.py
```
