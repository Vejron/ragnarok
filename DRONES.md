# FPV drones

An FPV drone is a hotbar item. Using it launches a small quadcopter loaded with a charge. The player then flies it through its nose camera, and their body stays kneeling where they launched it. A flight ends when the pilot sets the charge off, recalls the drone, runs the battery flat, gets hurt, flies out of range, or is shot down. The drone is a short-lived vehicle (`KIND_DRONE`) held through a *remote* seat. Its pilot's input drives it like any other pilot's, and their client predicts it like any other vehicle ([VEHICLES-II.md](VEHICLES-II.md)).

## What you get

- **The item** (`ITEM_DRONE` = 8, "FPV drone"). One per slot. It is scattered with the other pickups (weight 2) and can be placed in the map editor. It is not in the starting loadout.
- **Launch.** Click or F with the drone in hand. It lifts off a metre in front of the eye and half a metre above it, and the view switches to its camera. A launch fails if there is no room in front of the player or the room is at `MAX_VEHICLES`.
- **Flying.**
  - The mouse is the look, and the drone turns to follow it.
  - W flies where you look, at up to 30 m/s, diving or climbing with the look. S backs off, level.
  - A/D strafe.
  - Space climbs and Shift sinks, at 10 m/s each.
  - With nothing held it hovers in place.
- **Ways it ends:**
  - **Detonate (click or F, a fresh press).** The charge goes off where the drone is: a 4 m blast, 120 damage at the centre. It hurts players, vehicles, trees and buildings like a grenade, credited to the pilot as `WEAPON_DRONE`.
  - **Impact.** A flown drone that hits something hard (a contact taking more than 4 m/s out of it), or comes within 0.7 m of another stickman's body, goes off the same way. Flying into someone is the usual way to use it.
  - **Recall (E).** The drone is gone with no blast, and the pilot is back in their body.
  - **Link lost.** The pilot is back in their body, and the drone falls with its motors off. It goes off on whatever it lands on, or after 5 s. The link is lost when:
    - the 25 s battery runs out;
    - the pilot's body loses any health, or the pilot dies or leaves;
    - the drone goes more than 250 m from the body.
  - **Shot down.** It has 1 hp, so any hit wrecks it. It pops in a 1.5 m, 20-damage burst credited to the shooter, and the pilot is back in their body.
- **The body** stays where it stood, kneeling (it is sent as crouching), and can be shot, blasted and run over as usual. Being hit loses the link.
- **HUD.** While flying:
  - a scanline and vignette overlay, with static that grows as the signal weakens;
  - a centre cross and a blinking LIVE tag;
  - BATTERY and SIGNAL gauges bottom right;
  - a hint line with the keys.

  When the feed ends, a "SIGNAL LOST" static flash shows for 0.7 s. On touch screens the buttons read BOOM, RECALL, UP and DOWN.
- **Model and sound.** A procedural quad (`render/drone.ts`): a dark frame, a charge slung under it, spinning props, and a camera light in the pilot's colour. The props wind down once the link is lost. The sound is a thin, beating propeller whine.

## How it works

### Server

`mechanics/drones.rs` is the `Drones` mechanic. It runs after `Pickups` and before `Vehicles`.

- **Launch.** `Combat::use_item` calls `drones::launch` on a fresh fire press with a drone in hand, and spends the item only if the launch succeeds. Launching:
  - adds a `KIND_DRONE` vehicle with a `Drone { owner, energy, linked, pilot_hp, fall_left }`;
  - puts the owner in its seat 0 as `Seat { remote: true }`.
- **Each tick,** before `Vehicles` steps the physics, it settles each drone's fate from the pilot's input and state and from `Physics::car_impacts` of the last step:
  - fire detonates;
  - use recalls, and clears the use bit so `Vehicles` does not board something with it;
  - a lost link cuts it;
  - a hard contact or a nearby stickman detonates it.
- **`cut`** empties the seat, which leaves `Vehicles` with no driver, so the motors are off (`powered: false`).
- **`burst`** removes the drone before the blast, so the blast never finds it.
- **Shot down:** `Combat::wreck` hands a drone to `drones::pop`, which skips the burning-wreck path. The pilot is never killed with it.

`Player::embodied()` is true on foot or in a remote seat. It replaces `seat.is_none()` wherever the question is whether the body is in the world:
- hit targets;
- blasts;
- roadkill;
- the entity list;
- the lag-compensation history.

Movement, pickups and item use still check `seat`, so the body is frozen and cannot use items.

`Vehicles` leaves a drone out of the following:
- boarding;
- the seated riders follow their vehicle;
- crash damage and wall ramming;
- knocking players over;
- felling trees.

It hands the pilot's look to the drone as `aim`, as it does for a plane.

### Flight

`fly_drone` in `physics/vehicle/flight.rs` (`Lift::Drone`) flies it by velocity:
1. The stick and the look give a wanted velocity.
2. The rotors push towards it at `DRONE_ACCEL_GAIN` per second, never harder than `DRONE_MAX_ACCEL` (18 m/s²), on top of a force that cancels gravity. A force, not an impulse, so hovering is exact.
3. The hull leans the way the push goes, as far as about 60°.
4. Its yaw rate turns it towards the look.

A drone's body has CCD on, so it cannot fly through a wall in one tick.

The model has no state beyond the rigid body, so `CarSim.flyDrone` in `client/src/sim/vehicle.ts` replays it. `DRONE_CHECKPOINTS` (`physics/vehicle/tests.rs`) and `vehicle.spec.ts` check that the server and WASM agree through a hover, a climb, a dive along the look, a turn while strafing, and a sinking back-off.

An earlier version leaned the hull like the helicopter and held height with the rotor's push along its up axis. It could not dive: a steep sink asks the rotor for no push at all, so the tilted drone had nothing driving it forward.

### Wire

`PROTOCOL_VERSION` is 29.

- `KIND_DRONE` = 11.
- `WEAPON_DRONE` = 19, a blast (`DEATH_BLAST`). A pop is sent as a shot of weapon 6 (the autocannon's burst, about its size), but its damage is still credited as `WEAPON_DRONE`.
- The battery rides in the existing `thrust` field: `air` is the battery, 0..255 of full, and `THRUST_LINKED` (bit 2) is set while it is flown. There is no new field.
- A drone is a vehicle like any other in snapshots. It is gone from the list (and in `removed`) once it ends.
- The pilot's body stays in the entity list with the crouching flag.
- A remote pilot's interest circle is centred on their drone, not their body. `DRONE_RANGE` (250 m) is under `INTEREST_RADIUS` (300 m), so the body is always in it too.

### Client

The seat role is `drone`. In `game.ts`:
- **Camera:** `Scene.fpvFrom`, a 100° first-person view from the drone, along the look, rolled with the drone's bank.
- **Your own drone model** is hidden while you fly it, and shown again if it falls on after the link is lost.
- **Your own body** keeps being drawn from the predictor's frozen state, kneeling.
- **Other players' pilots** stay in the entity interpolation, which normally drops anyone in a seat.
- **Tile streaming** follows the drone.
- **The E prompt** never offers to board a drone.

A `WEAPON_DRONE` shot is drawn as a grenade-sized blast.

## Tuning

- Flight: the `DRONE_*` constants in `crates/shared/src/consts.rs`, mirrored in `client/src/sim/consts.ts`.
- Gameplay (battery, range, blast, pop, impact and proximity fuses, fall time): the `DRONE_*` constants in `crates/shared/src/items.rs`. The client mirrors those it shows.

`consts-parity.spec.ts` keeps each mirrored pair equal. Changing a flight constant moves the scripted run, so regenerate `DRONE_CHECKPOINTS` on both sides.

## Not done

- **No held model.** In hand the drone shows as nothing, like the golden egg: the character GLB has no mesh for it.
- **Drones count against `MAX_VEHICLES` (32)** with the parked vehicles. A full room refuses a launch, and the item stays in hand.
- **The proximity fuse looks only for stickmen.** Vehicles set it off by contact, roosters not at all.
- **No video-link physics.** Buildings do not block the signal: the link is a plain distance check.
- **A pilot who recalls or loses the drone does not get it back.** The item is spent at launch.
