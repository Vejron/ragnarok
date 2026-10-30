# Vehicles III: feel, tyres and crashes

The cars handle faster and feel quicker, their tyres leave smoke, dust and skidmarks, and crashes hurt and show. [VEHICLES-I.md](VEHICLES-I.md) covers the car and [VEHICLES-II.md](VEHICLES-II.md) the tank, the helicopter and driver prediction, which everything here has to respect.

## What you get

- **Quicker cars.** The engine pulls with full force up to 65% of top speed and fades to nothing at it, instead of fading all the way. The jeep has 10 kN (was 7.5) and tops out at 32 m/s (was 28). The sports car has 15 kN (was 11) and 50 m/s (was 44). Brakes are 30% stronger.
- **Sharper steering.** The front wheels turn at 7 rad/s (was 3), straighten 1.5× faster still, and lose less lock at speed (`STEER_FADE_SPEED` 20, was 14).
- **Planted.** A car's centre of mass sits below its box (15 cm on the jeep, 2 cm on the sports car) and downforce of `0.0001 × v²` per kg presses it down while a wheel touches the ground. That is tuned so a firm corner at speed holds, but holding full lock at 25+ m/s tips either car over.
- **Handbrake drifts.** With the handbrake held, a car's rear wheels keep 5% of their side grip, so the tail swings out and can be caught with the steering.
- **Air control.** With no wheel down, throttle pitches the car and steering rolls it (3 rad/s²).
- **A camera that sells speed.** The view widens by up to 14° at 45 m/s. The camera drops back as the car pulls away and closes in under braking, leans into turns, and shakes on real collisions only (never from braking, cornering or bumps).
- **Tyre effects.** Sliding on tarmac leaves black marks and white smoke. Off-road, wheels leave ruts and dust at any speed and throw gravel when they spin or slide. Water sprays. Exhausts puff when you pull away, and a sports car backfires when you lift off at speed. A tank's tracks always mark soft ground, and a low helicopter kicks up a ring of dust.
- **Crash damage.** Hitting anything at more than 5 m/s of sudden speed change costs `5 hp` per m/s over that, so a 25 m/s head-on into a wall wrecks a jeep. Both cars in a collision are hurt, and the other driver is blamed for any deaths. A tank takes a fifth of the damage and a tree hurts half as much. Below half health the engine loses power, down to half at zero. Scraping along something at speed throws sparks.
- **Visible damage.** The body dents where it was hit. At 90 the lights on that end break, at 120 the windscreen cracks, at 160 a bumper falls off and at 200 a door does. Paint dulls with damage, and a wreck is charred.
- **Wheel damage.** A hit near a wheel bends it, and so does gunfire or a blast that lands near one (a few revolver rounds or two sniper rounds take it off). A bent wheel points off towards its own side, so the car pulls, and it grips less. It leans out, so it wobbles as it rolls. At 255 it comes off and rolls away, and that corner drags on the ground in a shower of sparks. Respawning brings a car back whole.
- **Sound.** Every vehicle has an engine you can hear: a jeep's rough burble, a sports car's higher whine, a tank's low diesel with clattering tracks, a helicopter's rotor beat. Pitch climbs with speed and drops at each gear change; it is louder when pulling and quieter when coasting. Other players' vehicles come from where they are (panned, quieter with distance, pitch shifted as they pass). Tyres squeal where they leave marks, gravel crunches off-road and a hard landing thuds. Crashes come in three sizes (a bump, a crunch, a smash with glass), a scrape grinds, a wreck explodes and then crackles while it burns, a bent wheel rattles and a lost one grinds. Below 40% health the engine stutters, below 15% it hisses. Guns thump and a tank's casing clanks, and a door closes when you get in or out.

## How it works

### Physics, on both ends

Everything that changes how a car moves is in `physics/vehicle.rs` and copied into `CarSim` in `client/src/sim/vehicle.ts`, so the driver's prediction stays right. The new functions are `pull` (the engine curve), `assist` (downforce and air control) and the wheel loop in `drive_car`. The lowered centre of mass is set as explicit mass properties on the chassis collider. The client computes them in `f32` with `Math.fround`, so both builds hold the same numbers.

Side grip in Rapier's ray-cast vehicle is the share of a wheel's sideways slip cancelled each tick. At 60 Hz even 0.3 holds firm, which is why `DRIFT_GRIP` is 0.05.

### Condition

`CarCondition { power, wheels }` is what damage does to the physics. The server sets it each tick from the vehicle's `hp` and `damage` (`CarCondition::of`). The client sets the same from the snapshot before each replay (`conditionOf`). Per wheel `i` of a car, with `wear = damage / 255`:

- the steering gets `±TOE_MAX × wear` (0.12 rad): a left wheel turns left, a right one right;
- side grip is multiplied by `1 − 0.85 × wear` and friction slip by `1 − 0.6 × wear`;
- at 255 the wheel has no engine or brake, a 5 cm radius and no suspension, so it never reaches the ground.

### Crashes

After each step, `Physics::car_impacts` lists every contact of a vehicle's solids. Each entry has the contact impulse over the vehicle's mass (`dv`, the speed it took out of the vehicle that tick), the middle of the touching points in the vehicle's frame and the world's, and what was on the other side. Wheel forces are not contacts, so braking never counts.

`crash` in `mechanics/vehicles.rs` takes each vehicle's hardest contact. Above `CRASH_MIN_DV` it costs hp through `combat::damage_vehicle`, so wrecks and kills work as before. `dent` then adds damage to the side or sides it came from (weighted by how far along each half-extent the point lies) and to any wheel within 1.8 m (28 damage per m/s over the threshold, scaled by closeness). Lighter contacts at more than 4 m/s raise a scrape event at most every 4 ticks.

### Wire

Protocol **17**:

- `VehicleState` gained `damage: Option<[u8; 8]>`: front, rear, left, right, then wheels FL, FR, RL, RR. It is `nil` while undamaged, so a clean car costs one byte.
- `VehicleDelta` gained the same, `nil` when unchanged; all zeros means repaired.
- `WEAPON_CRASH` (8) and `WEAPON_SCRAPE` (9) are new shot events. The shooter is the vehicle's id and the point is where it hit. They are never a kill's weapon.

### Client drawing

| File | What it does |
|---|---|
| `render/chase.ts` | `ChaseCam`: field of view, follow distance, lean and shake, all from the drawn pose. |
| `render/particles.ts` | One instanced billboard mesh per blend mode (normal for smoke and dust, additive for sparks and flames), simulated on the CPU with optional ground bounce. |
| `render/skidmarks.ts` | A 4000-quad ring buffer of strips, one strip per wheel. The quads fade in the shader over 15 s, so nothing is touched per frame. |
| `sim/surface.ts` | A 2 m surface grid per tile (grass, tarmac, dirt, sand, water), built once per tile from its cover, roads and water depth. |
| `sim/slip.ts` | Each kind's wheel contact points, and `tyreSlip`: sideways speed of the contact point, locked wheels under hard braking, spinning wheels on a hard launch. |
| `render/tyres.ts` | `VehicleFx` per vehicle: marks, smoke, dust, gravel, spray, exhaust, backfire and downwash. Also `crashBurst` and `scrapeSparks`. |
| `render/carwear.ts` | `CarWear`: dents, broken lights, cracked glass, paint, panels cut out of the body mesh at runtime and thrown off, wheel lean and lost wheels. |
| `render/debris.ts` | Loose pieces that tumble, bounce, settle and shrink away. |

A dent waits up to 150 ms for the crash event that says where the hit was, because the damage in the snapshot can arrive a tick before the event. Without an event, as for a late joiner, it dents the middle of that side. Dents and cut panels go on a per-car copy of the body geometry, which is dropped on a repair.

### Sound

Everything is synthesised, as in `audio.ts`; there are no sample files.

| File | What it does |
|---|---|
| `sim/engine.ts` | Pure: `gearing` (gear and revs for a speed, with a drop at each shift), `mixOf` (a `SoundInput` of speed, acceleration, slip, surface, hp and wheel damage becomes the level and pitch of each layer), `squealGain`, `groundGain`, `landingThud`, `crashSize`. |
| `vehicleaudio.ts` | `VehicleSounds`: a `Voice` per vehicle (oscillators through a low-pass, plus one looping noise split into squeal, ground, hiss, fire and grind, a chopped layer for blades and tracks) through a `PannerNode`; one-shots for crashes, landings, explosions, guns and doors. |

The engines that play are the six nearest within 160 m, with the one you sit in always first. A voice is made for a vehicle with a driver, a wreck, or one that scraped within the last 0.2 s; a vehicle that leaves the nearest six fades out and its nodes stop. Doppler is done by hand (a pitch factor from the closing speed), since panner nodes no longer do it.

The driver's own car is drawn from `CarSim`'s prediction, and `VehicleFx` measures speed, acceleration and slip from the drawn pose, so the engine follows the wheel without delay. The crash sound's size comes from the speed `VehicleFx` saw in the last few tenths of a second, because the crash event itself carries no strength.

## Tests

`cargo test --workspace` and `cd client && npm test`.

- `physics/vehicle.rs`: the handbrake swings the tail out; a firm corner at speed holds but full lock at speed rolls the car; a wrecked front wheel pulls to its side; a lost wheel drops its corner and drags; a battered engine is slower; a damaged jeep's scripted drive matches a fixture. The scripted, ramp and damaged-drive fixtures are mirrored in `vehicle.spec.ts`, and WASM agrees within 1 cm.
- `mechanics/vehicles.rs`: ramming a car hurts both, dents the right sides and blames the driver; a nudge does no harm; a hit beside a wheel bends it and a second takes it off; a wreck comes back whole.
- `delta.rs`, `codec.spec.ts`: damage travels in full states and deltas, a repair clears it, and malformed damage is rejected.
- `engine.spec.ts`: gears climb and drop at each shift; each kind has its own voice; stutter, hiss and rattle follow damage; squeal follows slip and surface; landing and crash sizes. `vehicleaudio.spec.ts` runs the node graph on a fake `AudioContext`: voices start only for running, near vehicles, at most six play, nodes are all started and stopped once, and one-shots are range and count limited.
- `chase.spec.ts`, `tyres.spec.ts`, `carwear.spec.ts`: camera response; slip, surfaces, skidmark strips and fading, and particle life; marks and smoke for a slide but not a cruise, ruts off-road, spray in water; dents in the right place, lights, panels, lost wheels and repair.

## Not done, or not checked

- Driven once in headless Chromium on the dev world: tyre smoke and skidmarks during a handbrake turn, and a crash that dented the car and cost hp. Feel and tuning have not been judged by a person at a keyboard.
- Riders inside take no damage from a crash, and players hit by debris take none.
- The tank's and the helicopter's bodies keep their damage numbers but show no dents.
- Nobody has listened to the sound: pitches, levels and the balance of layers are first guesses, and the nodes are only tested on a fake context. Headless Chromium made one voice without errors. There is no horn (it needs a protocol event), no turret servo, and rockets and grenades still explode silently.
- No repair short of a respawn. Falling out of the world now also resets a vehicle's health and damage.
