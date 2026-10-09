# Vehicles II: the tank and its gunner

First part of roadmap stage 3 (see [design.md §14](design.md#14-roadmap)): a tank with a driver and a separate gunner seat. Driver-side prediction and the helicopter are still to come. [VEHICLES-I.md](VEHICLES-I.md) covers the car this builds on; [NETWORKING.md](NETWORKING.md) has the wire format.

## What you get

- Two tanks park on the roads next to the six cars. Walk up and press **E**: the first player in drives, the second takes the gun. A third can't board (two seats).
- **Driver:** W/S gas and brake, **A/D turns the tank on the spot** (or while driving), Space is the handbrake. Top speed is 12 m/s; the car's is 30.
- **Gunner:** aims with the mouse and fires with a click or **F**. The camera sits behind the turret on the line of the barrel, so the reticle at the screen centre is where the shell goes. The reticle goes dashed while the gun reloads (1.5 s).
- The turret turns on the hull at 1.4 rad/s and the barrel nods at 0.9 rad/s, between −7° and +29°. It is *stabilised*: it follows the gunner's world heading, so a turning hull doesn't drag the aim.
- A shell is an instant hit along the barrel. It stops at the first wall, building, vehicle or stickman and explodes there: a 7 m blast throws stickmen away (up to 18 m/s, with a hop) and shoves vehicles, the shooter's own tank included. Everyone sees a muzzle flash, a tracer and a ball of fire.
- Stickmen can still stand on the tank and ride it, and it knocks them back as a car does. Tanks are solid on the client too.

## The gunner is a separate mechanic

`mechanics/gunnery.rs` is a new `Mechanic`, run after `Vehicles` and before `Movement`. `Vehicles` knows seats, driving and hulls; `Gunnery` only knows guns. A vehicle that has a `Gun` (`game::Gun`: turret yaw, barrel pitch, reload, shot counter, last range) gets worked by whoever sits in its seat 1. Another vehicle with a gun (a helicopter chin gun, say) would only need the same seat and a `Gun`.

Per tick, for each vehicle with a gun:

1. Read the gunner's input frame. Its `yaw` is a world heading, so the target turret angle is that heading minus the hull's heading. The turret steps towards it by at most `TANK_TURRET_SLEW × dt`, taking the short way round. The barrel steps towards `pitch`, clamped to the barrel's travel.
2. If fire is held and the reload is up: a hitscan from the turret ring along the barrel (`Physics::cast_ray`, which skips the firing tank's own body). Stickmen are not physics bodies, so the shell is also tested against each standing player: a hit if their centre passes within 0.8 m of the path before the wall.
3. At the impact point: `Physics::blast_cars` shoves every vehicle in range, and every standing player within 7 m gets a knockback of `18 m/s × (1 − distance / 7 m)` away from the blast, plus a hop. That is the same `Character::knock` the car uses, so the client's reconciliation already knows how to replay it.
4. The shot counter goes up, the range of the flight is saved, and the reload restarts (90 ticks). A held trigger therefore fires once per reload; the jitter buffer strips fire from repeated inputs, so an underrun cannot add a shot.

Nobody in the gunner seat means nobody works the gun: a driver holding fire does nothing.

## Recoil

Each shot gives the hull a 1.6 m/s kick backwards along the barrel (level, so it doesn't dig the nose in), applied on the server by `Physics::kick_car`, so it is real motion everyone sees. The barrel also slides back 0.5 m and eases home over 0.35 s on the client; that part is purely visual.

## The tank's physics

`physics/vehicle.rs` now describes a vehicle by a `Spec` (box size, wheel layout, density, engine, steering). The car's numbers are unchanged. The tank:

- A 2.6 × 1.0 × 5.2 m hull at 250 kg/m³ (3.4 t), six wheels on three axles (the suspension tuning scales with mass), 22 kN of engine over all wheels, top speed 12 m/s.
- **No steered wheels.** Skid steering is done by setting the yaw rate directly: it ramps towards `±1.1 rad/s × stick` at 3 rad/s², but only while at least one wheel touches the ground. The wheels' side grip fights any spin every tick, so the rate is kept in the car's state and written back each time, rather than nudged from whatever grip left. With the stick released, grip and damping stop the turn.
- The handbrake and brake act on all six wheels (a car only brakes its rear wheels with the handbrake).

## Protocol changes

`PROTOCOL_VERSION` is now **5**. Old clients get the "reload" banner.

- `InputFrame` gained `pitch` (milliradians, + looks up) and `BUTTON_FIRE` (8). Every player sends their look pitch; only a gunner's means anything.
- `VehicleState` gained `turret`: `nil` for cars, else `[yaw i16, pitch i16, shots u8, range u16]`. The range is in decimetres. A car pays one byte for the `nil`. The worst-case snapshot test now gives every vehicle a turret and a full seat list, and still fits 1200 bytes.
- `KIND_TANK = 2`. A tank's `seats` list has two entries.
- The byte fixtures in `protocol.rs` and `codec.spec.ts` were regenerated and now contain a car and a tank.

## Client

| File | What changed |
|---|---|
| `sim/consts.ts`, `net/codec.ts` | Version 5, tank constants, `pitch`, `BUTTON_FIRE`, `turret` decoding |
| `net/interp.ts` | Turrets blend by angle; the shot counter and range come from the nearer snapshot, so a shot is drawn with the turret that fired it |
| `sim/input.ts` | `pitch` in the sample, left mouse and F as the trigger, `clampPitch` |
| `sim/physics.ts` | A tank gets a wider collider than a car |
| `render/tank.ts` | The tank model: hull, tracks, six wheels that roll per side (so a pivot turn spins them opposite ways), a turret and a barrel that follow the snapshot, driver and gunner heads |
| `render/effects.ts` | Muzzle flash, tracer and explosion ball |
| `render/car.ts` | `VehicleModel`, the interface both models implement |
| `render/scene.ts`, `game.ts`, `App.vue` | Gunner camera, tank follow distance, the prompts ("E: drive the tank" / "E: man the gun"), reticle and reload state |

On taking the gunner seat, the camera starts where the barrel already points. While there, the camera's pitch is clamped to the barrel's travel so the view never asks for what the gun can't do. The shot effects are keyed to the turret's shot counter in the interpolated state, so they appear with the turret as it is drawn.

## Decisions worth knowing

- **Hitscan, not a flying shell.** It needs no projectile entity on the wire and no lag compensation story yet. The tracer is drawn fast, and a real projectile can replace it when Combat needs one.
- **The gun aims with the hull's yaw only.** Hull pitch and roll are ignored when the turret targets a heading, so a tank on a steep slope has its aim off by the slope. The barrel itself is drawn on the hull's true orientation.
- **The turret has no collider.** Only the hull box is solid, and a stickman walks through the turret. Riding on the hull works as it does on a car.
- **The muzzle sits at the turret ring for the ray.** The ray starts at the ring, so a barrel poking into a wall doesn't make the shell vanish inside it. The client draws the tracer from the muzzle.
- **Gunner input is not predicted.** The turret is as laggy as any server-driven object: a round trip plus the interpolation delay. That's the same small delay as a driver's. Predicting the turret is easy later because it is a pure function of the gunner's input.

## Tests

`cargo test --workspace` and `cd client && npm test`.

- `physics/vehicle.rs`: a tank settles and tops out near 12 m/s; turns on the spot without drifting; stops turning when the stick is released; turns and drives together; `cast_ray` sees walls, ignores its own hull and respects range; a blast shoves nearby cars and misses far ones.
- `mechanics/gunnery.rs`: the turret follows the gunner and the barrel stops at its limit; slewing is rate-limited; a shell hits and throws a stickman in its path; the gun reloads between shots and misses a faraway bystander; no gunner means no shooting; the first thing in the way takes the shell.
- `mechanics/vehicles.rs`: a tank takes two crew and refuses a third and reports its turret; all eight vehicles are placed and settle in the shipped world.
- `protocol.rs`, `codec.spec.ts`: matching fixtures and the size bound.
- `interp.spec.ts`: turrets blend by angle across ±π and the shot counter steps.
- `predict.spec.ts`: a tank's collider is wider than a car's.

## Not done, or not checked

- **Not exercised in a browser.** Camera feel, the reticle, the tank model and the shot effects are untested by eye. The server places 8 vehicles in the shipped world.
- No damage: shells throw people but take no health (Combat). They don't hurt tanks either.
- A shell that lands next to the shooter's own tank still shoves it (the ray ignores the firing hull, the blast doesn't).
- A tank on a steep slope aims by the hull's yaw only (see above).
- No driver-side prediction (the rest of Vehicles II) and no helicopter.
