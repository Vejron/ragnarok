# Vehicles X: the submarine

The submarine is a small diesel-electric boat that can go under. Its captain steers it and takes it up and down. Its weapons officer looks through the periscope and fires guided missiles straight up out of four launch tubes in the deck. Once a missile is up, the officer flies it from its nose with the mouse, as a loitering drone is flown, until it hits something.

## In play

- **The submarine** (`KIND_SUBMARINE` = 25) has 250 hp and two seats, both sealed in its hull. It is never placed on its own. It appears only where it is placed in the map editor (**Submarine**, B in the palette), on open water. Like the ship, it starts floating, and is skipped over dry land.
- **Seat 0 is the captain.** W and S drive it ahead and astern, up to 8 m/s. A and D work the rudder, which bites only while it makes way. Run (Shift) dives it and jump (Space) brings it up, both at `SUB_DIVE_RATE` (2 m/s). With neither held, it holds whatever depth it is at.
  - On the surface it floats with its middle `SUB_SURFACE_DEPTH` (0.6 m) under and heaves with the waves.
  - It cannot rise higher than that, and the bed stops it going lower.
  - The captain's hint line shows the depth.
  - With nobody at the helm it holds its depth and drifts to a stop.
- **Seat 1 is the weapons officer, at the periscope** (the `periscope` rig).
  - The view is first person from the periscope's head, `SUB_MAST_HEIGHT` (5.5 m) above the hull's middle. It turns all the way round with the mouse and tilts ±0.35 rad.
  - Holding right mouse zooms it in as a gun sight does. The round scope mask and crosshair stay on screen.
  - The periscope is raised while they sit there, and everyone sees it rise out of the sail and turn.
  - Once the head is under the water, the view is the murk.
- **Firing.** A fresh press of fire launches a missile from the first loaded tube, but only from no deeper than `SUB_LAUNCH_DEPTH` (4.5 m), which is periscope depth.
  - The missile leaves the tube slowly, almost straight up (about 89°), at `SUB_LAUNCH_SPEED` (12 m/s). Fired from under the water, it comes up out of the surface with a splash.
  - The officer is now flying it, and their camera is its nose camera.
  - The tube's lid stands open until it is loaded again, `SUB_TUBE_RELOAD` (20 s) later. The officer's hint line counts the tubes ready, or says when they are too deep to fire.
  - A lone captain can switch seats with R to fire, as in any vehicle.
- **The missile** (`KIND_MISSILE` = 26) is a drone in all but how it flies.
  - It always drives at `MISSILE_SPEED` (70 m/s) along its nose, and steers as a finned missile does, skid-to-turn: it points its nose at where its flier looks with pitch and yaw and keeps its wings level, rather than banking into turns as the loitering drone does (`Lift::Missile`). Near vertical, where there is no level to find, it holds its roll. Its motor speeds it up by no more than `MISSILE_SPEED_UP` (18 m/s²), and its fins bite with the square of its share of `MISSILE_FIN_SPEED` (65 m/s). So it climbs almost straight up for about 2 s, then pitches over towards the look as it gathers speed, levelling out about 125 m up some 4 s after launch.
  - Fire sets it off. E lets it go.
  - It also goes off on any hard contact, near a soldier, or when its 20 s motor burns out and it has fallen for 3 s more.
  - It reaches out to `MISSILE_RANGE` (1,500 m) from the submarine.
  - Its warhead is between a rocket's and a loitering drone's: 8 m and 250 damage (`WEAPON_MISSILE`).
  - A rifle round or two brings it down.
- **Back at the periscope.** However the missile ends, the officer is back in their seat with the view where the periscope points. Their body never left the submarine: it stays sealed in the hull and goes where the submarine goes. It is not drawn kneeling anywhere.
- **Damage and wrecks.** Rifles and machine guns ring off its 25 mm pressure hull; shells go through.
  - Rounds and rockets stop at the water's surface, so deep under it nothing reaches it.
  - The motors (`Part::Engine`) and the missiles' fuel (`Part::Fuel`) are its modules.
  - Wrecked, it is not thrown: it floods and goes down where it is, as the ship does.

## Diving

`crates/sim/src/vehicle/dive.rs` (`Lift::Dive`) sets the hull's velocities from its state, the controls and the water alone, so the captain's client predicts it with the same code.

- **Holding depth.** The water bears its whole weight. This is a force, not a velocity change: a force is integrated together with gravity, so a depth it holds is held exactly. A velocity change left it creeping up by about 6 cm a second.
- **Trim.** The rise or fall it is asked for is reached at `TRIM` (3) per second:
  - ±`SUB_DIVE_RATE` while jump or run is held;
  - otherwise 0, and it holds its depth;
  - within half a metre of floating, the pull back up to `SUB_SURFACE_DEPTH`.
  
  It never asks for more than that pull, so it surfaces and stops there.
- **Waves.** Their heave fades out over the 3 m below floating, so a submerged hull rides still.
- **Attitude.** It is held level: with the water on the surface, and with the world below it. Its bow dips by up to 0.08 rad as it dives and lifts as it rises.
- **Driving.** The motor, rudder and keel work like the ship's, without the wind.

Test worlds keep their water 10 m deep. Under a terrain the physics has a floor slab 12 m down (`BED_FLOOR`), and a deeper bed is never reached.

The missile flies on the loitering drone's code. `glide` in `flight.rs` is the old `fly_loiter`, generalised over a `Glide` (speed, grip, turn rates, bank, the steepest climb or dive asked for, ceiling). `LOITER_GLIDE` and `MISSILE_GLIDE` are its two uses.

## The server

- **`Submarines`** (`mechanics/submarines.rs`) runs before `Drones`. Each submarine keeps a `Sub` component: each tube's reload countdown, the periscope's yaw from the hull, and whether it is raised.
  - On a fresh fire from the officer, it calls `drones::launch_at`. That is the old `drones::launch` split in two, so that a drone can be started from anywhere, nose up and moving.
  - It then marks the trigger held, so the same press does not set the missile off in `Drones`.
- **A remote seat can now be flown from another vehicle's seat:** `Seat::from`.
  - The officer's `Seats` place in the submarine stays theirs, so their body rides along in it.
  - `Player::embodied` is false for them, so they are not drawn kneeling.
  - When the missile goes, `drones::cut` puts them back in that seat, if it is still theirs.
- The tests are in `mechanics/submarines/tests.rs` and `crates/sim/src/vehicle/dive.rs`. There is also `a_missile_fired_straight_up_pitches_over_to_the_look_and_runs_up_to_its_speed` in the vehicle tests.

## The client

- **Models.** `SubmarineModel` (`render/submarine.ts`) and `MissileModel` (`render/missile.ts`) are built in code from primitives, like the ship.
  - The submarine has a periscope group that slides out of the sail and turns with `sub.scope`, and four lid hinges that swing open while their tube is empty.
  - The missile shows its flame while it is flown.
  - Its smoke trail is a new `'rocket'` exhaust in `KindFx`, drawn with the rockets' `rocketTrail`.
- **Seats and HUD.** The seats are `captain`, `periscope` and `missile`.
  - The periscope's view is `Cameras.periscope`. Its mask is the rifle scope's, through `HudStats.periscope`.
  - `HudStats.depth` and `HudStats.tubes` feed the seat hints.

## Protocol

- **The `sub` component** (bit 26) carries a submarine's loaded tubes as a mask, the periscope's yaw from the hull (mrad), and `SUB_MAST_UP`.
- **New ids:** `KIND_SUBMARINE` (25), `KIND_MISSILE` (26) and `WEAPON_MISSILE` (35).
- `PROTOCOL_VERSION` is 75.
