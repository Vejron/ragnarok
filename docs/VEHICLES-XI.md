# Vehicles XI: the seaplane

The seaplane is a two-seat biplane of the 1920s on twin pontoons. It lands on open water and takes off from it, and it is an amphibian: small wheels beside each pontoon let it use a road as well. It is armed exactly as the triplane is, with two forward guns, four bombs and a gunner on a ring behind the pilot. It is heavier and slower than the triplane, and tougher.

## In play

- **The seaplane** (`KIND_SEAPLANE` = 27) has 90 hp (the triplane has 70) and two seats, both in the open.
- **Placement.** It appears only where it is placed in the map editor (**Seaplane**, F in the palette). Placed over open water it starts afloat; placed anywhere else, or where the water is frozen in a winter world, it stands on its wheels.
- **Seat 0 is the pilot**, who flies it with the triplane's controls: Space for more throttle and Shift for less, the mouse to point it, and W, S, A and D to take pitch and roll over by hand. A click or F fires the two forward guns, and C drops a bomb.
- **Seat 1 is the gunner** on the ring behind, whose gun turns all the way round but holds its fire towards the pilot.
- **On the water** it floats on its pontoons and rides the swell.
  - With the throttle let go it lies still. Space drives it forward.
  - A and D work a water rudder. It turns the seaplane slowly at a standstill, on the propeller's wash, and quickly once it makes way.
  - Shift brakes it. With nobody aboard it stops and lies where it is.
  - It taxis up a shelving beach onto its wheels, and down one into the water.
- **Taking off from water** needs a longer run than from a road, because the water drags on the pontoons until the wings lift them out. From a standing start at full throttle it is about 120 m to 5 m up, against about 80 m from a road.
- **Landing on water.** Come in level and slow with the throttle cut. From 22 m/s it runs about 95 m before it stops.
  - Coming down hard costs speed at once and tips the nose down.
  - One that goes in so deep that its fuselage is under the water, or ends up on its side, has filled. It sinks to the bed and stays there, as a wreck does.
- **Bombs** stay on the rack while it is afloat, as they do on the ground, and the rack is loaded again after it has lain still for three seconds, on water or on land.
- **Against the triplane** it cruises at 29 m/s (the triplane 34) and tops out at 38 m/s (45). It rolls about three quarters as fast and stalls at 18 m/s (17).

## The definition

`crates/shared/src/vehicles/seaplane.rs` holds everything about it.

- **The body** is three kinds of box: the fuselage (the only one with mass, about 800 kg), the wing box (`Role::Wings`) and one box per pontoon (`Role::Part`). The pontoons' undersides are `SEAPLANE_KEEL` (1.55 m) below the fuselage's middle.
- **The wheels** are four ray-cast wheels at the pontoons, the front pair steering, as the triplane's are. On a road its middle rides `SEAPLANE_RIDE_HEIGHT` (1.71 m) up, which the sim's test measures.
- **The weapons** are the triplane's three `MountDef` rows on the seaplane's pivots. They share the triplane's gun (`PLANE_GUN`) and its rates, heat and bomb count, so no gunnery code is new.
- **`Spec::airframe`** is new, and every kind's `Spec` now lists it. An `Airframe` holds what differs between one aeroplane and the next: thrust, drag, the stall, level and cruise speeds, the three turn rates and the most the wings can pull. It also holds `floats`, the pontoons, for a seaplane.
  - The triplane's `PLANE_AIRFRAME` is built from the `PLANE_*` constants it always had, so it flies as before.
  - What all aeroplanes share stays in the `PLANE_*` constants: the autopilot, the throttle's settings, the ceiling and the turn back from the world's edge.

## The physics

`wing` in `crates/sim/src/vehicle/flight.rs` now reads the kind's `Airframe` instead of the `PLANE_*` constants.

`crates/sim/src/vehicle/floats.rs` is what the pontoons do (`float`). It runs for a `Lift::Wing` kind with `floats`, over open water, before `wing`. Like `sail` and `wade` it sets the body's velocities from its state, the controls and the tick alone, so the pilot's client predicts it with the same code.

- **Buoyancy.** The pontoons carry the whole weight at their draft (`SEAPLANE_DRAFT`, 0.25 m), more when pushed deeper and nothing once clear of the water. The hull's rise and fall follows the water's, so it heaves with the swell.
- **On the step.** The water's drag grows with how deep the pontoons sit and with the square of the speed. As the wings take the weight the pontoons rise and the drag falls away, which is what makes the take-off run long and then lets it go.
- **Afloat counts as on the ground.** `float` returns whether the pontoons carry it, and `wing` treats that as its wheels being down: the engine idles with the throttle let go, and the wings lift it off at the stall speed.
- **Level and heading.** The water holds it level with the surface under it and damps its rocking, the pontoons' keels take out its sideways drift, and the water rudder sets its yaw rate. All three fade as the pontoons come out, so the stick has it in the air.
- **A hard arrival.** Meeting the water faster than 2.5 m/s downward slows it, the more the more its nose is down, and tips the nose in.
- **Filled.** A wreck, one leaning more than 60°, or one whose middle is under the surface gets half its weight back from the water and sinks slowly to the bed.
- **Shallows.** The wheels still meet the bed, so where the water is shallow both carry it, and it can be taxied ashore.

## The server

- **`start_position`** (`mechanics/vehicles.rs`) puts a placed seaplane on the water at its draft where there is open water, and over the ground otherwise. A ship or a submarine is still skipped on dry land.
- **`height`** (`mechanics/gunnery.rs`) is what the bomb rack's `Need::Height` and `Refill::Parked` measure. It is now the height over the ground or over the water's surface as the waves have it this tick, whichever is higher. Before, a seaplane on a deep lake counted as high in the air. `Physics::water_surface` is the new query behind it.
- **`Refill::Parked`** judges standing still by speed over the ground alone, so a seaplane heaving on the swell is still parked.

## The client

- **The model** is `client/public/models/seaplane.glb`, built by `tools/vehicles/build_seaplane.py`:

  ```sh
  /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_seaplane.py -- --preview <dir>
  ```

  It is about 2,800 triangles: a slab-sided fuselage behind an open nine-cylinder radial, two wings with two bays of struts, aluminium pontoons with a stepped planing bottom on steel struts, and a fin and rudder.
- **`SeaplaneModel`** (`render/seaplane.ts`) extends `PlaneModel`. The model's moving parts carry the triplane's names, so the propeller, the guns' recoil, the ring, the control surfaces and the bombs are drawn by the same code. A `PlaneModel` now takes a `PlaneLook`: its parsed model, its paints and its bomb pivot.
- **`kinds/seaplane.ts`** gives it the triplane's `aviator` and `gun` seats, a deeper engine note and two additions:
  - `KindFx::keel`, how far below its middle its pontoons reach. The wake and the spray are measured from there, and a seaplane lying on its pontoons counts as down.
  - `RideView::stall`, its stall speed, which the pilot's stall warning now reads.
- **The bomb arc** treats open water as the ground, so it shows nothing while the seaplane is afloat.

## Tests

- `crates/sim/src/vehicle/floats.rs`: at rest at its draft, riding the swell round the waves' whole loop, the take-off run from water against a road, landing on water, the water rudder, diving in, a wreck sinking, and taxiing up a beach.
- `crates/sim/src/vehicle/tests.rs`: the ride height, flying from a road slower than the triplane and rolling slower, and landing on a road.
- `crates/server/src/mechanics/gunnery/tests.rs`: placement afloat and ashore, both seats' guns, and the rack holding and refilling afloat.
- `client/src/__tests__/seaplane.spec.ts`: the model's pivots and size, and the kind's seats, keel and stall speed.

## Protocol

No message or component changed, so `PROTOCOL_VERSION` stays at 75. `KIND_SEAPLANE` (27) is the only new id.
