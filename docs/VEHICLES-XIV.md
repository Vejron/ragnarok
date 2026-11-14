# Vehicles XIV: the Titan

The Titan is a robot about fifty metres high, with one pilot sealed in its head. It has no guns. Its pilot walks it and moves its two fists: it punches, picks up soldiers and vehicles and throws them, and crushes what it steps on. Everyone else has to bring it down with shells, rockets and bombs.

It is piloted with mouse and keyboard, or from a VR headset with the pilot's own hands moving its fists ([VR.md](VR.md)), which is built but has not yet run on a headset.

## In play

- **The Titan** (`KIND_TITAN` = 30) stands with its torso's middle 33 m up and the top of its head near 50 m.
- **Placement.** It appears only where it is placed in the map editor (**Titan**, Y in the palette). It wants open ground: its feet are 10 m apart.
- **Boarding.** E on the ground between its feet climbs in, and E again sets the pilot down in front of them.
- **Walking.** W and S walk it at up to 11 m/s (40 km/h), half that backwards; A and D turn it, about 35° a second. It takes a few seconds to get going and to stop.
- **Crouching.** Holding C bends its legs: the torso comes down 16 m and leans forward, which is what brings its fists to the ground. Standing up again takes about two seconds.
- **The camera** sits 60 m behind and above its shoulders and looks where the mouse looks, free of where the legs face. The fists go to what the middle of the screen is on: the ground there, or a point out along the look when it meets none within reach.
- **B looks out from its head** instead, and B again goes back behind. From there the look turns with the Titan and can swing 57° to either side of the way it faces; the eye leans and crouches with the torso, and the fists and forearms are in view when they are up.
- **The fists**, under mouse and keyboard (`client/src/titanhands.ts`):
  - With nothing held, both are kept up in a guard towards the point, open.
  - **Left click** throws the left fist at the point, closed, and **right click** the right: a punch. It comes back when the button is let go.
  - **Shift** reaches both hands out to the point, open, one either side of it. An open hand does no harm, however fast it moves.
  - **A click while Shift is held** closes that hand on what is there. Let go of Shift and the arm comes back in with it; let go of the button and the hand opens.
- **A punch** is a closed, empty fist moving at 8 m/s or more. The arm swings at up to 45 m/s.
  - A soldier in its way is thrown the way it is going and knocked down, and loses 6 health for every m/s: any real punch kills.
  - A vehicle is shoved by the fist itself, as by anything solid, and loses 4 health for every m/s.
  - A building or placed cubes lose a bite, as if rammed by something forty times a car's weight. A tree is felled.
  - Another Titan is knocked where the fist lands, and hurt as a vehicle is.
- **A grip** holds what the hand closes on: the nearest soldier within 4 m of the fist's middle, or else a vehicle of 60 tonnes or less (a tank, but not a ship or another Titan).
  - A held soldier stands on the fist. They can look about and shoot, but not walk, and their E does nothing.
  - A held vehicle is fixed to the fist, crew and all.
  - When the hand opens they fly on at the fist's speed. Slower than 12 m/s a soldier is only put down; faster, they are knocked down and lose 3 health for every m/s over it.
- **A step.** Each foot is solid, and where one comes down:
  - soldiers within 3.6 m of its middle are crushed;
  - soldiers on the ground out to 14 m are thrown outwards and knocked off their feet, unhurt;
  - a vehicle under it loses 120 health, which flattens a jeep; buildings, cubes and trees under it are rammed or felled.
- **Kills** by its fists and feet are its pilot's, as `WEAPON_TITAN` (36).
- **What hurts it.**
  - It has 255 health and feels a tenth of any hit (`Body::tough`), rounded: a rifle or a machine gun round rounds to nothing, and a tank's shell of 100 is 10. That is about 25 shells.
  - Its plates are 50 to 90 mm, so armour-piercing rounds still have to get through.
  - Its reactor is in its chest behind the glowing core. Shot out, it has no power and falls.
  - A blast shoves it a twenty-fifth as hard as it would a walker. A great enough blow still staggers it, and it catches itself; laid flat, it gets up by itself.
  - Wrecked, it collapses, burns, and is put back at its place like any vehicle.

## How it is built

"Kinematic plus ragdoll" came out as three kinds of part:

- **The torso is a dynamic body** on two spring legs, exactly as a walker's hull is: `Lift::Walker` and `walk()` in `crates/sim/src/vehicle/flight.rs`. The legs are ray casts from the hips, and a capped torque keeps it upright, so it can be knocked over.
- **The fists and the feet are kinematic bodies** of their own (`crates/sim/src/vehicle/limbs.rs`), which the torso does not feel.
  - A fist goes where it is asked to within its arm's reach (6 to 24 m from the shoulder) and outside the torso's box, speeding up and slowing at 140 m/s². With nobody aboard, or fallen, or wrecked, it goes to where it hangs.
  - A foot stands where it was planted until the hip is 7 m from it, then steps to a stride ahead of where the hip is going, in 0.55 s. The two step in turn. At rest a foot more than 2 m out of place steps back under its hip.
  - They shove any dynamic body they meet. The Titan's own hull leaves their collision group (`LIMB_GROUP`) out of what it touches, so they never push it about. This also means one Titan's fist does not shove another's torso: the server knocks it instead.
  - A shot that meets a fist, a shin or a thigh has hit the Titan.
- **The arms and legs between are only drawn**, by two-bone IK on the client (`client/src/render/titan.ts`) from the fists and feet the server sends. The thighs are solids on the hull, short enough to clear the ground when it crouches.

The server alone decides what a fist hits or holds: `crates/server/src/mechanics/titans.rs`, a `Mechanic` that runs before the vehicles'. A fist is solid and pushes a soldier along ahead of it, so a punch counts when the soldier is within the fist's next two ticks of travel, not once the fist is in them.

## Legs as data

The walker's legs were global `WALKER_*` constants. They are now a `Gait` on the kind's `Spec`, so that something ten times the size can walk the same way:

- `Gait::WALKER` holds the old values, and the walker, the hen and the tupp use it. Their tests and fixtures pass unchanged.
- `TITAN_GAIT` is the Titan's. What it adds:
  - `crouch` and `crouch_lean`, which a pilot's `lift` control works (`Buttons::Stride` puts C on it);
  - `rise`, the fastest its legs straighten (7 m/s), so that standing up from 16 m down does not throw it into the air;
  - a `getup_share` over 1 (3), eased back to 1 as it comes upright, because lifting that weight off the ground takes more than keeping it standing;
  - `blast_share` and `feet`.
- `Spec::arms` is its `Arms`, and `Body::tough` its share of any harm.

Its hull's middle settles at `TITAN_RIDE_HEIGHT` (33.2 m), measured by `a_titan_stands_fifty_metres_high_on_its_legs`.

## The model

`tools/vehicles/build_titan.py` builds `client/public/models/titan.glb` (about 2,200 triangles): slabs of grey armour over a dark frame, a glowing core in the chest and a glowing visor. Every limb is its own node hanging straight down from its joint (`upper_L/R`, `fore_L/R`, `fist_L/R` with `open_` and `closed_` fingers, `thigh_L/R`, `shin_L/R`, `foot_L/R`), and none is another's child: the client places each.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_titan.py -- --preview <dir>
```

## Protocol

Protocol 77 ([NETWORKING.md](NETWORKING.md)).

- **`hands`** (client to server, tag 3): where the pilot wants the two fists, in the hull frame, and which hands are closed. Sent every tick while piloting, apart from the `input` frames, so that a soldier's input did not grow.
- **`titan`** (component 28): the fists in the hull frame, the feet from the hull's centre along the world's axes, and which hands are closed and full.
- **`held`** (component 29): the Titan and the hand a soldier is held in.
- The pilot's own Titan is predicted as any driven vehicle is. Its fists and feet are drawn from the server's snapshots, so they answer about a tenth of a second after the mouse.

## Tests

- **`crates/sim/src/vehicle/tests.rs`**: it stands at its ride height with a foot under each hip; walks to top speed with its feet stepping in turn, stops, and turns on the spot; crouches to bring a fist to the ground and stands again; a fist stays within its reach and its arm's speed, out of its own chest, and hangs when nobody is aboard; a fist shoves a jeep, a closed one carries it off and throws it; a shell's blast barely moves it, a great blow fells it and it gets up.
- **`crates/server/src/mechanics/titans/tests.rs`**: a closed hand picks a soldier up, carries them high, they still fire, and they fly when it opens; an open hand coming down on a soldier does no harm; a closed fist swung through one throws and hurts them; the arms hang unless its own pilot moves them, and when the pilot stops; a foot crushes who is under it and throws those around it, and wrecks a jeep; it feels a tenth of a shell and nothing of a rifle; a fist fells a tree and holes a house; a placed one stands, is boarded from between its feet, walks and sets its pilot down.
- **`client/src/__tests__/titan.spec.ts`**: the model's fists are where the server has them, turned with the hull, on arms of the built length; it stands on the planted feet and reports a footfall; the mouse-and-keyboard guard, punch, reach, grip and carry.

## Not yet

- **VR has not run on a headset** ([VR.md](VR.md)).
- **The pilot's own arms lag** by the round trip to the server, since they are not predicted.
- **A swinging foot does not kick.** A foot hurts only where it comes down. A soldier in the way of one that is moving is pushed by it, as by any solid.
- **It walks over houses**, not through them: its legs stand on a roof as on a hill, until its feet have brought the house down.
- **Bots do not pilot it**, and do not know to run from it.
- **Not played.** The numbers (punch and throw damage, its toughness, its speed) are first guesses.
