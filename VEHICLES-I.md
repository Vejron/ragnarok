# Vehicles I: what was built

Roadmap stage 2 from [design.md §14](design.md#14-roadmap): a car, seats, enter and exit, driver input routing, server-simulated and interpolated on the client. This file records what was added, the decisions behind it and what was left out. [design.md §10](design.md#10-entities-and-vehicles) and [NETWORKING.md](NETWORKING.md) hold the resulting design and wire format.

## What you get

- Six cars park on the roads of the Södermalm block, facing along the road. They are placed at spawn points, preferring ones players don't start on, and any spot where the chassis would touch a building is skipped.
- Walk to within 4 m of a car and press **E**. The first player in takes the driver's seat, the next ones take the three passenger seats. **E** again gets out, beside the car.
- The driver uses **W/S** for gas and brake (brake, then reverse), **A/D** to steer and **Space** for the handbrake. Passengers just ride.
- A HUD hint shows what **E** does when a car is in reach, and the driving keys while at the wheel.
- The camera pulls back while in a car and swings behind it when the mouse is idle.
- Seated players show as coloured heads in the open-top car, in their own colour.
- Cars are solid: a stickman can't walk through one, and can climb on top and ride along. A car that drives into a stickman throws them forward and up (see Knockback).

## Two cars

The single car was later split into two kinds that share the ray-cast physics and differ in their `Spec` (server `physics/vehicle.rs`, client `sim/vehicle.ts`). Models are built in Blender by [tools/vehicles/build_car.py](tools/vehicles/build_car.py) into `client/public/models/jeep.glb` and `sports.glb`; the three vehicles' scripts follow the same conventions.

| | Jeep (`KIND_JEEP`, 1) | Sports car (`KIND_SPORTS`, 4) |
|---|---|---|
| Body | open tub, roll cage, spare wheel | open wedge, wing |
| Box (half extents) | 1.0 × 0.42 × 1.95 m | 0.95 × 0.3 × 2.2 m |
| Wheel radius | 0.55 m | 0.32 m |
| Ground clearance | about 0.44 m | about 0.10 m |
| Engine, top speed | 7500 N, 28 m/s | 11 000 N, 44 m/s |
| Steering lock | 0.5 rad | 0.4 rad |
| Seats | 4 | 2 |

Half of the six parked cars are of each kind. `PROTOCOL_VERSION` is 14, because the new kind changes both the wire and what a client must simulate. Where this file says "the car" below it describes the original (now jeep-like) numbers: the fixtures in `vehicle.rs` and `vehicle.spec.ts` hold the current ones.

## Protocol changes

`PROTOCOL_VERSION` is now **4** (2 added vehicles, 3 knockback, 4 riding). Old clients are refused with the existing "reload" banner.

- `Snapshot` has a fifth element, a list of `VehicleState`: `[id, kind, x, y, z, qx, qy, qz, steer, [seat player ids]]`. The orientation is a quaternion packed as `x, y, z` in an i16 each (`w` is rebuilt, with the sign picked so that `w >= 0`).
- Players in a seat are left out of the stickman list. The seat list is how everyone, including the player themselves, learns who sits where.
- `Snapshot` gained a per-receiver `ride` field: the vehicle the player stands on, or 0.
- `EntityState` gained `knock_x` and `knock_z` (cm/s), so a thrown player's slide survives reconciliation.
- `BUTTON_USE` is now acted on. The jitter buffer strips it from repeated inputs, like jump, so an underrun can't board or leave twice.
- The size test now checks every combination of cars and seated players. A full room still fits one datagram, because each seated player is a stickman fewer.
- The byte fixtures in `protocol.rs` and `codec.spec.ts` were regenerated together.

## Server

| File | What changed |
|---|---|
| `crates/shared/src/consts.rs`, `protocol.rs`, `quant.rs` | Car constants, `VehicleState`, quaternion packing |
| `crates/server/src/physics/mod.rs` | `physics.rs` became a directory. `Physics` now owns a persistent Rapier pipeline, islands and so on, and steps it each tick. Characters query with `exclude_dynamic()`. New `touches_static` and `character_fits` helpers |
| `crates/server/src/physics/vehicle.rs` | The car: a dynamic box with four ray-cast wheels |
| `crates/server/src/mechanics/vehicles.rs` | The `Vehicles` mechanic: placement, boarding, leaving, input routing, stepping, riders, unflip |
| `crates/server/src/game.rs` | `World::vehicles`, `Player::seat`, a shared id counter, `vehicle_states`, seated players left out of `entity_states` |
| `crates/server/src/mechanics/mod.rs`, `movement.rs` | New `Mechanic::on_start`. `Movement` skips seated players. `Vehicles` runs before `Movement` |

### The car

A 1.8 × 0.8 × 4.2 m box at 200 kg/m³ (about 1.2 t), with a ray at each corner. Rapier's `DynamicRayCastVehicleController` does the suspension and tyre grip. The wheel axle points right, so the controller's rolling direction is −Z and a positive engine force drives the car forward.

Per tick, `drive_car` turns the controls into wheel settings, and `step_cars` updates the wheels and then steps the rigid-body pipeline:

- **Engine:** 7000 N over all four wheels, fading linearly to zero at 30 m/s. Reverse is half of that.
- **Braking:** pressing against the direction of travel brakes instead of reversing the engine. The handbrake brakes the rear wheels. With no pedal, a light brake stands in for rolling resistance, so a coasting car slows down.
- **Steering:** the front wheels ease towards the target at 3 rad/s. The lock is 0.5 rad at standstill and narrows with speed: `0.5 / (1 + speed / 14)`.
- **Empty cars** hold their handbrake.

Measured on flat ground, from the tests: 0 to 18 m/s in 6 s, about 12 m turn radius at 16 m/s, and a ride height of 0.66 m.

### Seats

- `Vehicles::on_tick` reads each player's `BUTTON_USE`. Seated players leave, standing ones board the nearest car with a free seat within 4 m.
- Leaving tries the seat's own side first (driver and back-left on the left), then the other side, the back and the front, with a capsule-fits check against the static world. When all four are blocked, the stickman appears 2.5 m above the car.
- Riders' character positions follow the car every tick, so the `KILL_Y` respawn and any future proximity check keep working.
- A player who disconnects frees their seat.

### Recovery

- A car on its side or roof, almost stopped, for 3 s is set back on its wheels where it lies.
- A car that falls below `KILL_Y` returns to where it was parked.

### Riding on top

- **Detecting it.** After `Movement` moves a standing player, a capsule 6 cm lower than the feet is tested against cars (`Physics::car_under`). A hit while grounded sets `Character::ride`. The 6 cm reaches the roof because the controller rests 2 cm above whatever it stands on.
- **Carrying.** `Vehicles::on_tick` notes each car's pose, steps the cars, and moves every rider by the car's rigid motion since then (`CarPose::carry`: into the old car frame, out of the new one). Pitch and roll carry the rider too. This happens before `Movement`, so the rider then walks about in the car's new frame.
- **No self run-over.** A rider is skipped by `knock_players`, so a bouncing roof never throws its own passenger.
- **Leaving.** Jumping from a car adds its horizontal velocity to the knockback before the jump step. Walking off the edge into the air does the same after the step. Both are capped at 20 m/s. Landing bleeds it off at 8% per tick, so they run on a few steps like a person would.
- **On the client.** `CharacterState` has a `ride` too, set from the snapshot, and `stepCharacter` applies the same takeoff rule using the car's velocity from the last two snapshots (`Interpolator.latestVelocity`). The predicted stickman lives in a frame where the car is still, at its pose in the newest snapshot (the collider `setVehicles` holds), which keeps replay after a snapshot exact. `Game.ridden` then draws it relative to the car as it is interpolated now. So the car moves smoothly, the rider moves with it, and the rider's own steps on the roof are instant.
- **Tests.** `a_player_on_the_roof_rides_along` and `jumping_off_a_moving_car_keeps_its_speed` on the server; four `riding a car` cases in `predict.spec.ts`.

### Knockback

- `Vehicles::on_tick` runs `knock_players` right after the cars step and riders are carried. A standing player whose capsule overlaps a car (found with `Physics::car_hit`) gets `Character::knock`, a hop (`vy`) and `grounded = false`.
- Power is 1.1 × the car's horizontal speed, between 2.5 and 20 m/s. The direction is 75% the car's heading and 25% outward from the car's centre. At under 1 m/s the push is purely outward, so a creeping car nudges people out of the way.
- `knock` is added to the walking velocity in `desired_translation` (Rust `movement.rs`, TS `movement.ts`) and decays with `decay_knock`: ×0.92 per tick on the ground, ×0.99 in the air, zero below 0.05 m/s.
- A 12-tick cooldown per player makes a single pass a single hit. Seated players are never hit.
- Test: `a_driven_car_knocks_back_a_player_in_its_way` drives a car into a standing player and checks the knockback, the hop and the distance thrown.
- No damage is dealt. Health arrives with Combat, which can hook this spot.

## Client

| File | What changed |
|---|---|
| `sim/consts.ts`, `net/codec.ts` | Version 2, car constants, `VehicleState` decoding |
| `net/interp.ts` | `Interpolator<T>` is generic and takes a blend function. `blendEntity` is the old behaviour. `blendVehicle` lerps position, steering and the quaternion (the short way round) |
| `render/car.ts` | The toy car model, wheel roll and steering, rider heads |
| `render/stickman.ts`, `render/scene.ts` | `colorFor`, a `hidden` flag, a camera distance parameter |
| `sim/input.ts` | `followYaw`: eases the camera behind the car unless the mouse moved in the last 1.5 s |
| `sim/predict.ts` | `record` and `acknowledge` |
| `sim/movement.ts`, `sim/physics.ts` | Knockback in the character state and step. `RapierMotor.setVehicles` makes cars solid |
| `game.ts`, `App.vue` | Seat tracking, car rendering, the camera, the E prompt and the driving hint |

### How the client handles being seated

1. A snapshot whose vehicle list has the local id in a seat puts the client in "seated" mode. This is derived only from snapshots.
2. In that mode the stickman is hidden and not simulated. Inputs keep going to the server, because that is how the car is driven. They are kept with `record` instead of `apply`, and `acknowledge` trims them as acks arrive.
3. The first snapshot with the player back in the stickman list ends it. `reconcile` takes the server's position and replays the inputs newer than the ack, so inputs sent between the server's exit and the snapshot's arrival are not lost. A jump of more than 3 m is a snap, not a slide.
4. The driver sees their car through the normal interpolation delay, like a remote entity. That is the small input delay design.md accepted for this stage.

## Decisions worth knowing

- **Seats travel in snapshots, not reliable events.** design.md said seat changes would be reliable events. A snapshot list cannot disagree with the snapshots the client already draws, and there is no ordering problem between a reliable stream and datagrams. Reliable events remain the plan for things that are truly one-off.
- **A separate vehicle list**, not extra fields on every entity. Adding the pitch, roll and seat data to `EntityState` would have pushed a full 32-player snapshot past 1200 bytes.
- **Solid cars, mirrored on the client.** The client's Rapier world gets a box per car at the newest snapshot pose (`RapierMotor.setVehicles`). For parked cars that is exact. For a moving car it is about half an RTT behind, which can cost a small correction right at the contact. That is also where the server throws the player, so the correction is lost in the hit.
- **Gravity of 9.81 for cars** and 20 for stickmen. The suspension is tuned around the realistic value; the stickman value is a game-feel choice.
- **The car's quaternion is not a yaw.** Cars can pitch on kerbs or roll in a crash; flat worlds hide this, but the wire already supports slopes and flips.

## Tests

Run `cargo test --workspace` and `cd client && npm test`.

- `physics/vehicle.rs`: settling on the suspension, acceleration and top speed, reversing, steering in both directions, braking and coasting, stopping at a wall, spawn overlap checks, cars blocking characters, a moving car overlapping a character in its path.
- `mechanics/vehicles.rs`: placement along a road, boarding and leaving (including the standing position), range limits, a driver driving with a passenger riding, a disconnect freeing the seat, unflipping, and that every car in the shipped `world.msgpack` settles and stays put.
- `protocol.rs`, `codec.spec.ts`: the new fixtures, in both languages, and the size bound.
- `interp.spec.ts`: the vehicle blend, including the quaternion sign flip.
- `predict.spec.ts`: inputs recorded in a car are replayed after stepping out, a server-side hit is picked up and keeps sliding, knockback fades, and the WASM car collider blocks the stickman like the server's.
- `movement.spec.ts`: knockback adds to walking and dies out.

## Not done, or not checked

- **Not exercised in a browser.** The Rust and TypeScript tests pass, the client builds, and the server starts and parks six cars in the real world. WebTransport needs Chrome, Edge or Firefox with the dev server running, and I did not drive a car there. The camera feel, the E prompt and the wheel animation are the parts to look at first.
- No driver-side prediction (Vehicles II), tank or helicopter.
- No damage: the knockback has no health to take from yet.
- Cars don't feel the stickman they hit (no slowdown). Characters are not physics bodies.
- The client cannot tell a step off the roof onto the ground from a step along it, so its `ride` flag lasts until the next snapshot corrects it (up to one round trip). Mispredicting that only shifts the stickman by the car's frame, and the correction is smoothed.
- Riders are carried but the car is not affected by their weight. Standing in the open-top cabin is not special: it is the same as the roof.
- A car's wheels do not follow the suspension travel visually.
- Cars are placed once at start and never stream in or out. Interest management (stage 5) needs to include them.
