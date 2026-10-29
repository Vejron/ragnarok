# Knockdown

A blast, or a car, that throws a soldier hard enough knocks them down. They fly, land and lie helpless for a
moment, then get up. Everyone sees them tumble as a physics ragdoll.

## When, and for how long

- A blast throws every soldier out to twice the radius it hurts them in (`BLAST_PUSH_REACH`,
  `blast_at` and `blast_push` in `crates/server/src/mechanics/combat.rs`).
  - The push is `knock × (1 − (distance / reach)²)`, at most `KNOCK_MAX_SPEED` (20 m/s), plus a hop.
  - It fades gently: well outside where the blast hurts, it still knocks soldiers down.
- A push of at least `KNOCKDOWN_POWER` (7 m/s) knocks them down (`shared::movement::knockdown_ticks`):
  - for `KNOCKDOWN_MIN_TICKS` (48, 0.8 s) at 7 m/s;
  - rising evenly to `KNOCKDOWN_MAX_TICKS` (150, 2.5 s) at 20 m/s.
- The count only runs on the ground, so the flight comes on top.
- A car that hits a soldier throws them at 1.1 times its speed (`knock_players`,
  `crates/server/src/mechanics/vehicles.rs`).
  - Any car going 2 m/s or more knocks them down: for at least 0.8 s, and the full 2.5 s at 18 m/s
    (65 km/h) or faster.
  - What it takes off rises with the square of the speed over 3 m/s, and all of a soldier's health at
    25 m/s (90 km/h): about 6 % at 30 km/h, 30 % at 54 km/h and 60 % at 72 km/h. Below that they are
    mostly tossed, and only a very fast car kills.
- A weaker push only shoves them, as before.
- An open canopy is lost. A soldier lying prone is stood up to be thrown, unless they have no legs.

| Blast (damage radius, knock at the centre) | Knocked down out to |
|---|---|
| AP shell (3 m, 8 m/s) | 2.1 m |
| Autocannon (2.3 m, 9 m/s) | 2.2 m |
| Grenade (5 m, 12 m/s) | 6.5 m |
| Rocket (5 m, 16 m/s) | 7.5 m |
| Bomb, hen's egg (6 m, 16 m/s) | 9 m |
| HE shell (7 m, 18 m/s) | 10.9 m |
| Mortar (11 m, 30 m/s) | 19.3 m |

## What it takes away

While `Character::down > 0` (`crates/sim/src/lib.rs`):
- **Movement:** their input is replaced by nothing at all, facing as they were. Gravity, the throw and
  collision carry on, and on the ground the throw slides to a stop at `KNOCK_KEEP_GROUND`. The server
  and the local prediction run the same step.
- **Items:** `use_item` refuses everything: no firing, reloading, bandaging or throwing. A reload or
  bandage in progress is interrupted.
- **Vehicles:** they cannot board one (`board`).

Bots are bound by all of it the same way.

## On the wire

Protocol 55.

- **`flags.stance`:** `STANCE_DOWN` (3) while knocked down. A legless soldier who is down is still
  prone, which prediction works out from their `limbs`.
- **`vitals.down`** (the owner's only): the ticks left, for their prediction.
- **`motion`:** the throw, as before. A ragdoll is launched with it.

## The ragdoll

Client only, and only for show. Where a soldier is stays the server's.

**Physics** (`crates/sim/src/ragdoll.rs`, `ClientSim.spawnRagdoll` and the calls after it):
- **Its own world.** Ragdolls run in a small Rapier world of their own, stepped once a frame in steady
  `TICK_DT` substeps. The client's main physics world is only stepped by car prediction, sometimes
  many times a frame, so it couldn't host them.
- **Scenery** within 15 m of a ragdoll is copied in from the main world every 0.5 s, and dropped when no
  ragdoll is near. That covers ground, buildings, trees and placed cubes. The shapes are shared, so a
  copy is cheap.
- **Vehicles are left out.** They move, and a copy refreshed every 0.5 s would be a car that isn't
  there, or one that a soldier it just hit starts inside. A ragdoll passes through vehicles.
- **The body:**
  - 11 capsules: pelvis, torso, head, and the upper and lower half of each arm and leg. They weigh
    about 75 kg in all.
  - Each is placed along its bones as the soldier stood at the blast.
  - Lost limbs get no part.
  - Parts never touch each other, only the scenery.
- **Joints:**
  - Waist and neck bend from straight, within 0.5 and 0.7 rad.
  - Shoulders and hips swing from where they were, within 1.8 and 1.3 rad.
  - Elbows and knees are hinges that only bend the right way, up to 2.5 rad.
- **Launch:** every part sets off at the soldier's throw (`knock_x`, `vy`, `knock_z`), turning head
  over heels about the level axis across it at 0.45 rad/s per m/s.
- **Tether:** a damped spring pulls the pelvis horizontally towards where the server has the soldier,
  so the ragdoll ends up where they really are and getting up doesn't jump.

**Driving it** (`client/src/sim/ragdoll.ts`, `Character.startRagdoll` / `driveRagdoll` / `endRagdoll`):
- **Who gets one:** every drawn soldier that goes `STANCE_DOWN`, up to `RAGDOLL_CAP` (8), the nearest
  first. Any more just lie down in the prone pose.
- **Posing:** each frame the ragdoll's parts pose 13 bones (hips, spine, chest, neck, head, arms and
  legs) at the offsets they had from their parts at the start. Hands, feet and the gun keep the pose
  they had.
- **Getting up:**
  - When the stance leaves `DOWN`, the ragdoll lets go.
  - The soldier gets up with `getup_back` or `getup_front`, whichever side they lie on, facing the way
    the body lies. That takes 1.1 s, easing out of the ragdoll's pose over the first 0.3 s.
  - Then the stance's clips take over again.
- **Dying:** a soldier who dies while down stays a ragdoll as the body until they respawn, or until a
  blast blows the body apart.
- **The local player:** they see their own ragdoll. The camera still follows their capsule, with a
  jolt as hard as they were thrown.

**Cost:** eight ragdolls tumbling beside a building step and pose in about 0.25 ms a frame (wasm in
V8, worst frame 0.7 ms).
