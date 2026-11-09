---
name: add-vehicle
description: Add a new vehicle kind to Ragnarok end to end: the shared Rust definition, physics tuning, weapon mounts, the client model and presentation, the Blender model, tests, docs and world placement. Use when the user asks for a new car, truck, tank, aircraft, gun or other drivable or crewed vehicle, or for a variant of an existing one.
---

# Adding a vehicle kind

A vehicle kind touches about 15 files across Rust, TypeScript and Blender. Work through the steps in order. Two existing kinds make good templates:

- **The golf cart** (`crates/shared/src/vehicles/golf_cart.rs`, commit `1e89641`): a plain car with no weapons. It is the smallest complete example; `git show 1e89641 --stat` lists every file a car touches.
- **The technical** (`vehicles/technical.rs`): a car with a mounted machine gun, a standing gunner, custom springs and a gun that overheats. Copy from it for anything armed.

Other kinds to borrow from: a turret on a hull is `tank.rs`; a gun on its own carriage is `ship_gun.rs`, `pak.rs` or `mortar.rs`; flying is `heli.rs` or `plane.rs`; legs are `walker.rs` or `hen.rs`.

## 0. Decide before you write

Settle these first, and ask the user about anything the request leaves open:

- **Name.** Use a generic name, not a brand ("Technical", not "Land Cruiser").
- **Kind id.** The next free `KIND_*` in `crates/shared/src/consts.rs`. Animals share the id space, so check them too.
- **Seats.** At most `CAR_SEATS` (4). Seat 0 drives. For each other seat, decide whether it is a passenger who fires their own gun (`crew_guns`) or works a mount.
- **Exposure.** `Exposure::Open` (the crew can be shot), `Glazed` (behind glass, still hittable) or `Sealed` (inside armour).
- **Weapons.** One `MountDef` per weapon. See the table in step 2 for what the client can see of each.
- **Placement.** Most new kinds appear only where the map editor places them. The automatic line-up on worlds without editor placements is in `crates/server/src/mechanics/vehicles.rs`.

## 1. Shared definition (Rust)

Create `crates/shared/src/vehicles/<kind>.rs` with `use super::*;`. In it:

- **Constants**, named `<KIND>_*`: `SEATS`, `HALF_EXTENTS` (half the chassis box, metres), `WHEEL_RADIUS`, `TRACK_HALF`, `AXLE_Z`, `HARD_POINT_Y`, `SUSPENSION_REST`, `RIDE_HEIGHT`, `MAX_STEER`, `MAX_SPEED` and `SEAT_HEADS`. Each gets a doc comment with its units. Write arrays as literals, because `gen-consts` cannot evaluate expressions.
- **Frame.** Hull frame: +Y is up, the nose is −Z, and the box centre is the origin. `SEAT_HEADS` holds one head point per seat. A seat on a turning mount also gets its yaw-0 head here, so that both `seat_heads.len() == seats` and the model's seat markers hold.
- **`ARMOR`.** Plates are usually `SHEET.plates` for a soft-skinned vehicle. Modules are `Part::Engine`, `Part::Fuel`, and `Part::Breech` for a gun that can be shot out.
- **`pub(super) const <KIND>: VehicleDef`.** Copy the template's and fill in every field; there are no defaults.

Then wire it in:

- `consts.rs`: add `pub const KIND_<KIND>: u8 = <id>;` with a doc comment, and `pub use crate::vehicles::<kind>::*;`.
- `protocol.rs`: add the kind to the `pub use crate::consts::{KIND_...}` list.
- `vehicles.rs`: add it to the `crate::protocol::{...}` import, add `pub mod <kind>;`, and append it to `VEHICLES`, which is kept in id order.
- `vehicles.rs` tests: add it to `only_armed_kinds_have_a_gun...` when it is armed.

**Spec tuning notes:**
- `density` × box volume is the mass; aim for something believable.
- `engine_force / mass` is the launch acceleration (the jeep's is about 7.7 m/s²).
- `max_suspension_force` must carry a quarter of the weight per wheel with margin.
- `springs`: `Springs::FIRM` unless it should ride differently. Rapier scales the spring and damper by the chassis mass, so the sag is g/(4·stiffness). At landing the compression damping does most of the work, so a softer ride needs lower `compression` as well as lower `stiffness`.
- `com_drop`: a tall or softly sprung vehicle rolls over at full lock flat out unless its centre of mass is low. The technical needed 0.5 m. Find the value with the full-lock test.
- `skim`: `Some(...)` only for light, fast cars that can skip over water.

## 2. Weapons (if any)

A `MountDef` lists the seat that works it, `aim`, `muzzle`, `fire`, `cooldown` (ticks between rounds), `spread`, `heat` (per round, of 255; `GUN_COOL` takes 1 off a tick), `ammo`, `needs` and `show`. The server's `Gunnery` mechanic works any mount, so arming a vehicle needs no new server code.

`show` decides what reaches the client:

| `Show` | Replicated as | Notes |
|---|---|---|
| `Turret` | `turret`: yaw, pitch, shots, range, heat, flags (`TURRET_JAMMED`) | At most one per kind. The usual choice for a vehicle's main gun |
| `Pintle` | `pintle`: yaw from the turret, heat, jam | Only on top of a `Turret` listed before it (the tank's roof gun) |
| `Guns` / `Rack` | `arms` | A pilot's forward guns and bombs |
| `Hidden` | nothing | Rocket pods and the like |

- **Full-circle traverse:** `Aim::Mount { yaw_limit: None, .. }`.
- **A machine gun** fires `Fire::Hitscan { weapon: WEAPON_MG, pen: Pen::ball(near, far, range, calibre), .. }`. Size `heat` so that holding the trigger jams it after a few seconds: net heat per second = rounds/s × heat − 60, and it jams at 255.
- **A crewed mount the gunner turns with:** `mount_seat: Some(MountSeat { seat, pivot, head })`. `crew_gun` already leaves out a `mount_seat`, so that gunner does not also fire their own rifle.

## 3. Client

1. Run `cd client && npm run gen:consts`, then `UPDATE_PARITY=1 cargo test -p shared`. These regenerate the constants and tables.
2. `client/src/net/codec.ts`: export `KIND_<KIND>`, and any new flag.
3. `client/src/sim/slip.ts`: add `<KIND>_WHEELS` using `corners(...)`, where `width` is the tyre width.
4. **Model.** A car built by `build_car.py` gets a `CarKind` entry in `client/src/render/car.ts` (`KINDS`). If it has a turning gun, add a class extending `CarModel` that finds the `turret` and `gun` nodes, turns them in `update(..., seats, turret)`, and provides `barrel()` and `recoil()` (see `render/technical.ts`). Any other kind needs its own model and loader.
5. **`client/src/kinds/<kind>.ts`** is the `VehicleKind`. Fill in:
   - `model` and `load`;
   - `ride.seats`, one `SeatView` per seat. Each has a rig (`DRIVE`, `RIDE_ALONG`, `{ name: 'gun', hull: [yawLimit, pitchMin, pitchMax] }`, ...), a `role`, a `board` prompt, and optionally `gauges` (`'heat'` for a turret gun's heat bar), `sight` and `stand` (the occupant is drawn standing, not sitting);
   - `sound`: oscillator voices and an engine model;
   - `fx`: wheels, exhaust and `bay`, the engine's position, where a hurt one smokes from.
6. Register it in `KINDS` in `client/src/kinds/index.ts`.
7. Add it to `VEHICLE_KINDS` in `client/src/editor/placements.ts`, with an icon letter no other entry uses.
8. **Gun fire effects.** The `Turrets` presenter in `client/src/entities/presenters/guns.ts` only draws fire for model classes it lists. Add a branch for the new model: machine guns draw bullets with `fireRound`, and cannons go through `fired`.

## 4. The model

For a car, add a `SPECS` entry and a `build_<kind>()` to `tools/vehicles/build_car.py`, and register it in `BUILDERS`.

- **Frame.** Blender is x right, y forward, z up, so a game point `(x, y, z)` is Blender `(x, -z, y)`. The ground is `ride` below the origin.
- **Nodes.** Wheels come from `add_wheels(spec, lugs, rim_ratio)`, which names them `wheel_*` on `pivot_*`. `add_seats([...])` places `seat_N` markers at the `SEAT_HEADS`, converted to Blender's frame.
- **A turning gun** is a separate `Part("turret", location=pivot, parent=body_obj)` with a `Part("gun", parent=turret_obj)` child, both with their origins on the gun pivot.
- **Size.** Keep it under about 2,500 triangles; `car.spec.ts` checks this.

Build it and look at the previews:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only <kind> --preview <scratch dir>
```

Read the `_three`, `_side` and `_rear` PNGs and fix anything that looks off.

## 5. Tests

- **`crates/sim/src/vehicle/tests.rs`:**
  - add the kind to `ride_heights_differ_between_cars`, then write the measured height into `<KIND>_RIDE_HEIGHT`, the `SPECS` `ride` and the slip layout;
  - add a test that it reaches top speed and stays upright at full lock, keeping the run short because the test world ends about 200 m out;
  - add any handling claim the user asked for, e.g. softer springs than the jeep.
- **`crates/server/src/mechanics/gunnery/tests.rs`,** if armed: spawn it with `crate::mechanics::vehicles::add_vehicle`, seat the gunner, and check that it aims, fires, jams and clears. Check what the snapshot shows with `game.seen(vid)`.
- **Client:**
  - add the kind to the `describe.each` in `client/src/__tests__/car.spec.ts`, which checks wheels, seat markers, headlights and the triangle budget;
  - add a `<kind>.spec.ts` for its gun (node parenting, `barrel()` against the server's aim), its seats (`seatHead`, `crewGun`, `stand`) and its gauges.

## 6. Docs and placement

- Write `docs/VEHICLES-<next roman numeral>.md`, following the latest one: what it is in play, its tuning and why, the model and the build command, and any protocol change. Add a line for it to the README's list.
- To put it in a world, add `{"kind": <id>, "lat": ..., "lon": ..., "yaw": ...}` to `worlds/<world>/area.json` `vehicles`. Place it beside existing vehicles, about 5 m apart, matching their yaw. The server skips a vehicle that does not fit its spot and logs "placed vehicle does not fit; skipped".

## 7. Verify

Run the full CI set from `AGENTS.md`: `cargo test --workspace`, `cargo clippy ... -D warnings`, and in the client `npm run build:wasm`, `npm run type-check`, `npm run lint` and `npx vitest run`. Then list what still needs trying in the running game: how it drives, boarding each seat, the gun, and the pose.
