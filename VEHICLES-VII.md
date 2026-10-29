# Vehicles VII: the hen

A giant robot hen that one player drives. It walks and balances on the walker's two spring legs, leaps, flaps its wings into a short climb, glides down when they tire, and lays egg bombs from under its tail. The walker's own notes are in [design.md §10](design.md#10-entities-and-vehicles) and the code (`walk` in `physics/vehicle/flight.rs`); the Roadster ([VEHICLES-VI.md](VEHICLES-VI.md)) is where the tank-of-stamina pattern, the hop and the air trim come from.

## What you get

- **One hen** (`KIND_HEN` = 12) with 180 hp and one seat. It is the walker's size: a 2 × 2.4 × 3.6 m hull about 5 m up on legs, and about 8 m tall to the comb. On a world with no editor placements it takes a spawn point left after the planes and the mortar, so it never blocks a runway. On a world whose vehicles were placed in the map editor, it appears only where it is placed (**Hen** in the editor's palette).
- **Boarding.** Walk up between its feet and press **E**. A walker is boarded the same way now (see [Boarding a walker](#boarding-a-walker)).
- **Walking (W/S, A/D).** Exactly the walker: up to 9 m/s, it plants its feet when let go, takes a catching step when shoved, and a big enough blast knocks it over.
- **Leap (Space, on the ground).** It adds 11 m/s upward, about a 6 m leap, plus up to 6 m/s along its heading with W held (half that backwards). It costs a fifth of the stamina. One press is one leap.
- **Flap (Space, held in the air).** Once the leap tops out, the wings hold a 1.5 m/s climb for about three seconds of stamina. A full gauge reaches about 10 m above where it took off. With W held, the wings also carry it forward towards 12 m/s.
- **Glide.** With the stamina gone and Space still held, the wings spread and hold the fall to 4 m/s. From the top of a flap that is about 6 s in the air and up to 70 m forward. Let go of Space and it drops.
- **In the air** the wings keep the hull level, so every leap lands on its feet. W/S/A/D lean it a little and A/D turn it.
- **Stamina** (the WINGS gauge bottom right; it reads GLIDE when empty) refills at 35% a second while a foot is down and Space is let go. Holding Space on the ground keeps hopping until it runs out.
- **Eggs (click or F).** A clutch of six, shown as eggs under the gauge. One is laid every half second from a hatch under the tail and tossed back at 3 m/s. It bounces and rolls like a grenade and bursts after 2 s with a plane bomb's blast (6 m, 110 damage at the centre). An egg that meets anything faster than 16 m/s smashes at once. A laid egg never does, but one dropped from a flap or a glide is a bomb. One egg grows back every 4 s. A hen that stands on its own eggs is caught in their blast.
- **Effects and sound.** The legs tuck up in the air and land with a heavy double thump. The head bobs as it walks, the wings beat with a whoosh, the tail flicks up as each egg drops with a plop, and every leap is a giant robotic "BA-WK!". Hens come in two breeds, handed out by id: white with a red comb, and silver with a dark grey comb and wattles.
- **Touch.** The jump button reads FLAP and fire reads EGG.

## How it works

### Physics

`HEN` in `physics/vehicle/spec.rs` is the walker's spec with a lighter hull (density 200, about 4.5 t) and a `tank` of `HEN_STAMINA`. `Spec::tank` is new: it is the full tank of whatever a kind's tricks run on, the Roadster's air or the hen's stamina, and `None` for everything else. `Car::tank` starts from it and is refilled from it on a reset. The hen's two leg colliders are the walker's.

`flap` in `physics/vehicle/flight.rs` runs once per tick for a walker-lift car with a tank, in `step_cars` right after `walk`, which resets the forces. It is given whether a foot is within reach, from the same feet rays `walk` uses. In order:

1. With a foot down, a leap is a velocity change. The "rising slower than half the leap speed" check is the Roadster's: the feet can still reach the ground for a tick after take-off. With Space let go, the stamina refills.
2. In the air, the angular velocity is set towards upright, leaning with the stick and yawing with the steering, as the Roadster's hover does. `walk`'s balance does nothing with no foot down, and without this a leap would tumble.
3. With Space held, a force carries it forward with W. With stamina left, once rising no faster than `HEN_FLAP_CLIMB`, a force holds that climb and drains the stamina. With none left, an upward force chases a `HEN_GLIDE_SINK` fall and never pushes it up.

There is no ceiling. Stamina does not come back in the air, so it bounds the climb, and that also holds on hilly terrain, where an absolute height would not. A fallen, wrecked or empty hen does none of it.

`CarSim.flap` in `client/src/sim/vehicle.ts` replays it line by line, and `walk` there now returns whether a foot is down. `physics/vehicle/tests.rs` (`HEN_CHECKPOINTS`) and `vehicle.spec.ts` check that the server and the WASM client agree through a scripted walk, leap, flap, glide and landing.

### Controls

Jump sets `CarControls::hop` for a hen as for a Roadster, with no handbrake and no climb (`mechanics/vehicles.rs`, and `controlsOf` on the client).

### Eggs

`work_hen` in `mechanics/gunnery.rs` runs for every hen after the gunned vehicles. It reuses `PlaneArms`: `bombs` is the clutch and `rearm` counts towards the next egg growing back (`PlaneArms::fresh(kind)` fills a plane's rack or a hen's clutch). An egg is a `Rocket` of kind `PROJECTILE_EGG` with `HEN_EGG_FUSE_TICKS` to live. `combat::step_grenade` flies it with the egg's radius and bounce, bursts it on a hit harder than `HEN_EGG_SMASH_SPEED`, and `burst_grenade` gives it the egg's blast and `WEAPON_EGG`. Clients draw the burst as a bomb's.

### Wire

`PROTOCOL_VERSION` is 35: new `KIND_HEN` (12), `PROJECTILE_EGG` (7), `WEAPON_EGG` (20), and two `Thrust` flags. `VehicleState::thrust` carries the stamina as `air` (`air_byte`; the Roadster's and the hen's tanks are both 1 full). `flags` are `THRUST_LIFT` while it leaps or flaps, `THRUST_GLIDE` while it glides and `THRUST_AIR` while no foot is down. `arms` carries the clutch. The driver's prediction resumes the stamina from `air` as the Roadster's does, and the HUD gauge and the wings follow the prediction.

### Boarding a walker

Boarding measures the distance to a vehicle's chassis centre, which on a walker or a hen is 4.3 m above a standing soldier's, out of `ENTER_RADIUS` (4 m) wherever the soldier stands. Both are now boarded from a point a leg's length below the hull, between the feet (`boarding_point` in `mechanics/vehicles.rs`, mirrored by the prompt in `game.ts`), and the prompt names them ("E: drive the walker", "E: drive the hen") instead of falling through to the jeep's.

### Model and drawing

`tools/vehicles/build_hen.py` builds `client/public/models/hen.glb`, about 3,400 triangles. It shares `build_walker.py`'s conventions and leg nodes (`hip_L/R`, `knee_L/R`, `ankle_L/R`, same lengths), with a feathered drumstick of a thigh and a three-toed foot. The other nodes are `body`, `head` (it bobs), `wing_L/R` (they swing out about the fore-and-aft axis) and `tail` (it flicks up).

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_hen.py -- [--preview DIR]
```

The walker's foot planting and two-bone IK moved into `Legs` (`render/legs.ts`), which both `WalkerModel` and `HenModel` use. It also tucks the feet up while airborne and reports the landing as two full-weight footfalls. `HenModel` (`render/hen.ts`) takes its `THRUST_*` flags through `setFlags`, from the driver's own prediction for the driver. `takeEvents` returns its wing beats, leaps and eggs for `VehicleSounds.wingBeat`, `bawk` and `plop`.

## Tuning

Every number is a `HEN_*` constant in `crates/shared/src/consts.rs`, generated into `client/src/sim/consts.generated.ts` by `npm run gen:consts`. Changing any of the flight constants moves the scripted run, so regenerate `HEN_CHECKPOINTS` on both sides. The legs and balance are the walker's `WALKER_*` constants, shared on purpose; a bigger hen would need its own.
