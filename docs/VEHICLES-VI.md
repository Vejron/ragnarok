# Vehicles VI: the Tesla Roadster

Tesla's 2026 Roadster, the one with SpaceX cold-gas thrusters. It is a low electric two-seater that boosts far beyond its tyres' grip, hops over a row of cars, and holds itself in the air for a few seconds. It is built on the sports car's chassis ([VEHICLES-III.md](VEHICLES-III.md)), with forces of its own on top, and the driver's client predicts all of it like any other car ([VEHICLES-II.md](VEHICLES-II.md)).

## The reference

Tesla showed the production car on 15 October 2026. Its "Rocket" trim has ten cold-gas thrusters fed by about 600 litres of rechargeable compressed air, stored in carbon-fibre tanks in the car's empty spaces. The rear nozzles take it to 60 mph in under a second, because they push four to five times harder than the tyres can. In the demo it lifted itself over five other Teslas while moving. Coverage describes brief hops and hovers, not sustained flight, and that is what the game gives it. Sources: [Electrek](https://electrek.co/2026/10/15/tesla-unveils-cyberfied-new-roadster-kickflip/), [TechCrunch](https://techcrunch.com/2026/10/15/tesla-reveals-spacex-powered-roadster-that-flies-and-falls-with-style/), [The EV Report](https://theevreport.com/tesla-roadster-unveil-spacex-cold-gas-thrusters).

## What you get

- **One Roadster** (`KIND_ROADSTER` = 10), parked with the other cars on the road. It has 100 hp and 2 seats, the sports car's wheels and body box, a stronger motor (18 kN) and a top speed of 62 m/s.
- **Boost (Shift).** The four rear nozzles push along the nose at 24 m/s², about 2.5 g, on the ground or in the air. The push is not limited by top speed or the engine's power curve. A full tank holds about three seconds of boost.
- **Hop (Space, on the ground).** It adds 9 m/s upward in one tick, which clears about 4 m. It costs 15% of the tank.
- **Hover (Space, held in the air).** The six lift nozzles under the car level it, and lean it a little with W/S/A/D like a helicopter, so it drifts the way it leans. A/D also turn it. Once the hop has topped out, they hold a 1 m/s climb that fades out below 40 m. A full tank holds about four seconds of hover. When the tank is empty the car falls.
- **The air tank** refills at 20% a second while the car is on its wheels with Shift and Space both released. Holding either keeps the valves open, so an empty tank never hops by itself. Respawning fills it.
- **Handbrake (C).** Space is the hop, so the handbrake moves to crouch for this car only.
- **HUD.** An AIR gauge bottom right; it turns red below 15%. The drive hint lists the keys. On touch screens, the jump button reads HOP, and BOOST and BRAKE buttons appear.
- **Effects and sound.** Each firing nozzle blows white mist (`coldGas` in `render/tyres.ts`). Lift jets near the ground also kick up a ring of dust. It is compressed air, so there is no flame. The motor is an electric whine: a single gear, sine and triangle voices, no exhaust and no backfire. Firing nozzles add a loud high-passed hiss.

## How it works

### Physics

`thrust` in `physics/vehicle/flight.rs` runs once per tick for any `Car` with a `tank` (only the Roadster has one), in `step_cars` after `assist`. It resets the body's forces first, as a wing or a rotor does, then:

1. **Boost** adds a force along the nose and drains `ROADSTER_BOOST_DRAIN · dt`.
2. **Hop** fires if a wheel is down, the tank holds `ROADSTER_HOP_COST`, and the car is rising slower than half the hop speed. That last check is what makes one press one hop: the wheel rays can still touch the ground for a tick after lift-off, and the car is already rising by then. It is a velocity change, not a force.
3. **Hover** fires when the car is airborne with Space held. It sets the angular velocity towards a lean target with `ROADSTER_HOVER_LEVEL` and `ROADSTER_HOVER_RESPONSE`, the way `fly` levels a helicopter. Without it, `assist`'s air pitch and roll from W/S/A/D would keep tilting the car until it flipped. Once the car rises no faster than the climb speed, a force along the hull's up axis holds that climb.
4. **Refill** happens with a wheel down and both controls released.

All of it is a function of the body, the tank and the controls, so `CarSim.thrust` in `client/src/sim/vehicle.ts` replays it. `physics/vehicle/tests.rs` (`THRUST_CHECKPOINTS`) and `vehicle.spec.ts` check that the server and WASM agree on position and tank level through a run that uses all three.

### Controls

`CarControls` has two new fields, `boost` and `hop`. `mechanics/vehicles.rs` sets them from run and jump for a Roadster only, and takes the Roadster's handbrake from crouch. `controlsOf(input, kind)` mirrors this on the client.

### Wire

`VehicleState` and `VehicleDelta` end with `thrust: Option<Thrust>`, `[air u8, flags u8]`. `air` is the tank, 0..255 of full (`air_byte` in `game/state.rs`). `flags` are `THRUST_BOOST` and `THRUST_LIFT`, the nozzles that fired in the last step. It is `nil` for every other kind, and `nil` in a delta when unchanged. `PROTOCOL_VERSION` is 28.

The driver's `VehicleDriver.reconcile` takes the tank from `thrust.air` along with the pose and `drive`. The u8 costs at most 1/510 of a tank in prediction error. The driver's own HUD gauge and jets come from the prediction, so they respond at once. Everyone else draws the jets from the snapshot's `flags`.

### Model

`build_roadster` in `tools/vehicles/build_car.py` builds it, about 900 triangles. It is a long low wedge with wide rear haunches, a glass canopy under a black targa panel, slim headlamps and a full-width red light bar. Its nozzles have empties of their own:

- `nozzle_rear_0..3` sit in the rear diffuser and blow backwards.
- `nozzle_lift_0..5` sit at the nose corners, the sills and the tail corners, and blow down.

`CarModel.jets(flags)` returns each firing nozzle's world position and direction.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only roadster
```

## Tuning

Every number is a `ROADSTER_*` constant in its `crates/shared/src/vehicles/` file, mirrored in `client/src/sim/consts.ts` (`consts-parity.spec.ts` keeps the two equal). Changing any of them moves the scripted run, so regenerate `THRUST_CHECKPOINTS` on both sides.
