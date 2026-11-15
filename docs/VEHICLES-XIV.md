# Vehicles XIV: the Titan

The Titan is a robot about fifty metres high, with one pilot sealed in its head. Its pilot walks it and moves its two fists: it punches, picks up soldiers and vehicles and throws them, and crushes what it steps on. Its right fist holds a 40 mm autocannon, and on its left shoulder are four vertical launch tubes for the submarine's guided missile ("Weapons", below). Everyone else has to bring it down with shells, rockets and bombs.

It is piloted with mouse and keyboard, or from a VR headset with the pilot's own hands moving its fists ([VR.md](VR.md)), which is built but has not yet run on a headset.

## In play

- **The Titan** (`KIND_TITAN` = 30) stands with its torso's middle 33 m up and the top of its head near 50 m.
- **Placement.** It appears only where it is placed in the map editor (**Titan**, Y in the palette). It wants open ground: its feet are 10 m apart.
- **Boarding.** E on the ground between its feet climbs in, and E again sets the pilot down in front of them.
- **Walking.** W and S walk it at up to 11 m/s (40 km/h), half that backwards; A and D turn it, about 35° a second. It takes a few seconds to get going and to stop.
  - It walks on one foot at a time. A step takes 1.65 s and is up to 18 m long, so at full speed it takes about nine steps in fifteen seconds.
  - The torso is carried over the planted foot: it rises about 0.8 m as it passes over and sinks into the next step, sways a metre or so towards the foot it is on, and goes about 8 % slower over a foot than between two.
  - Slower, it takes shorter steps (10 m at least) and stands on both feet between them. Stopped, or turning on the spot, it brings its feet back under its hips one at a time.
  - Crouched it keeps 40 % of its speed and takes short steps.
- **Flying.** It has thrusters in its feet (`TITAN_JETS`).
  - **Space** leaps: 14 m/s upward, about 10 m, and a little along the stick. It costs a tenth of the fuel.
  - **Space held in the air** fires the thrusters. They climb at 8 m/s for about seven seconds from a full tank, carry it at up to 20 m/s with W and S, turn it with A and D and keep it level. They take it no higher than 120 m over the ground.
  - Let go, or out of fuel, it falls. Held on the way down they slow a fall by 12 m/s every second.
  - The fuel comes back on the ground in ten seconds, and is the gauge the Roadster's air is.
- **Landing.** How fast it was falling when its feet met the ground decides it.
  - Under 8 m/s it sets down: two footfalls, as in a step.
  - At 8 m/s or more it is a **slam**, a blast without fire under it, at its worst at 35 m/s (a fall of about 60 m). It reaches 20 m at 8 m/s and 50 m at 35 m/s.
    - Soldiers within that are hurt, up to 160 at the middle of the worst, and thrown outward at up to 22 m/s; out to twice as far they are still knocked off their feet.
    - Vehicles are thrown outward and up and lose up to 220 health, so a jeep near the middle is wrecked.
    - Buildings and placed cubes near the middle have a bite taken out of them, up to 22 m across. Trees near it are felled.
  - It takes no harm itself, and neither does whoever it holds in a fist, who stays there.
- **Crouching.** Holding C bends its legs: the torso comes down 16 m and leans forward, which is what brings its fists to the ground. Standing up again takes about two seconds.
- **The camera** sits 60 m behind and above its shoulders and looks where the mouse looks, free of where the legs face. The fists go to what the middle of the screen is on: the ground there, or a point out along the look when it meets none within reach.
- **B looks out from its head** instead, and B again goes back behind. From there the look turns with the Titan and can swing 57° to either side of the way it faces; the eye leans and crouches with the torso, and the fists and forearms are in view when they are up.
- **The fists**, under mouse and keyboard (`client/src/titanhands.ts`):
  - With nothing held, both are kept up in a guard towards the point, open.
  - **Left click** throws the left fist at the point, closed: a punch. It comes back when the button is let go.
  - The right fist holds the gun, laid on the point; **right click** fires it ("Weapons", below).
  - **Shift** reaches both hands out to the point, open, one either side of it. An open hand does no harm, however fast it moves.
  - **A left click while Shift is held** closes the left hand on what is there. Let go of Shift and the arm comes back in with it; let go of the button and the hand opens.
  - **A right click while Shift is held** throws the right fist, gun and all, as a punch.
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
- **Kills** by its fists and feet are its pilot's, as `WEAPON_TITAN` (36). Its gun's and its missiles' are theirs as the Strf 90's autocannon's and the submarine's missile's are.
- **Weapons**, under mouse and keyboard ("Weapons", below):
  - **Right click held** fires the 40 mm in the right fist, and **Q** swaps its rounds between armour-piercing and high explosive.
  - **Shift and a right click** throw the gun fist as a punch instead, and it holds fire. The gun hand never grips: only the left one closes on things.
  - **G** sends a missile up out of the next loaded tube on the left shoulder, for the pilot to fly.
- **What hurts it.**
  - It has 255 health and feels a tenth of any hit (`Body::tough`), rounded: a rifle or a machine gun round rounds to nothing, and a tank's shell of 100 is 10. That is about 25 shells.
  - Its plates are 50 to 90 mm, so armour-piercing rounds still have to get through.
  - Its reactor is in its chest behind the glowing core. Shot out, it has no power and falls.
  - A blast shoves it a twenty-fifth as hard as it would a walker. A great enough blow still staggers it, and it catches itself; laid flat, it gets up by itself.
  - Wrecked, it collapses, burns, and is put back at its place like any vehicle.

## Weapons

Both are rows of `MountDef` in `titan.rs`, worked by `Gunnery` like any other vehicle's guns, with two new variants in `shared::mounts`.

### The autocannon in its right fist

- **What it fires.** It is the Strf 90's gun: 40 mm armour-piercing rounds, with high explosive as the other load (Q), at `SHELL_SPEED`.
  - It fires 7.5 rounds a second (`TITAN_GUN_COOLDOWN`, 8 ticks), against the Strf's 5.
  - Held, it jams after about five seconds (`TITAN_GUN_HEAT`), and clears once it has cooled, as a machine gun does.
- **Where the rounds go.** They leave the fist's middle along the forearm, from the elbow to the fist, as the arm is bent at that moment (`Muzzle::Fist`, `Physics::forearm`). They first appear `TITAN_GUN_LENGTH` (7 m) out, at the muzzle.
  - The gun is aimed by the arm, not by a turret, so the mount is `Aim::Fixed`.
  - Under mouse and keyboard the right fist is held up with its forearm laid on what the middle of the screen is on (`layForearm` in `client/src/titanhands.ts`): the fist goes a forearm's length on from the elbow towards the point, and that is repeated a few times.
  - The fist moves at its own pace and lags the mouse by about a tenth of a second. So the HUD draws a second, open cross where the barrel points now (`titanSight` in `client/src/gauges.ts`), apart from the reticle.
- **On a wall.** An armour-piercing round's charge is too small for its blast to bite a wall (`CARVE_MIN_RADIUS`), so the round punches the wall out `CANNON_AP_WALL_HOLE` (0.85 m) round where it strikes: a cell or two a round (`Burst::hole`, `punch` in `projectiles.rs`). The Strf 90's gun fires the same round, so it does the same.
- **The trigger.** It is right click (`BUTTON_ALT`), but not while Shift is held (`Need::Released(BUTTON_RUN)`): Shift and a right click throw the fist as a punch.
- **The gun hand never grips** (`holds_gun` in `titans.rs`). Closed, it is a fist that punches as the other does.
- **Its own fist.** A ray that ignores a vehicle passes through that vehicle's own fists and feet (`cast_ray_hit`), so a round leaves through the fist that fires it.
- **On the wire.** It shows as the vehicle's `turret` (its shots, heat, jam and load), so the presenter draws the flash, the kick and the bang from its counter (`Turrets.fist`), as it does for a tank.
- **It is fallen**, or its pilot is away flying a missile: then it neither aims nor fires (`Need::Upright`).

### The missile tubes on its left shoulder

- **The launcher.** A box on the left pauldron (`TITAN_LAUNCHER_*`, a `Role::Hit` solid) holds four tubes (`TITAN_TUBE_AT`).
- **A launch.** G (`BUTTON_CHUTE`, which the Titan has no other use for) sends up the submarine's guided missile, `KIND_MISSILE` (`Fire::Guided`):
  - It leaves out of the next tube in turn, nose up at `MISSILE_LAUNCH_PITCH`, tipped towards where the pilot looks, at `SUB_LAUNCH_SPEED`.
  - The pilot then flies it from its nose, exactly as the submarine's weapons officer does (`drones::launch_at` with the Titan's seat as `from`).
  - Their body stays in the Titan's seat. When the missile goes off, or they let it go (E), they are back in the Titan.
- **Meanwhile the Titan stands.**
  - `Vehicles` gives a vehicle a driver's controls only while the driver sits in it, not while they are flying something they launched from it.
  - `Gunnery` works a mount only for an occupant who is in their seat.
  - Its arms hang once the pilot stops sending hands.
- **Reloading.** The tubes are the mount's ammunition: four, and one more every `TITAN_TUBE_RELOAD` (20 s, a submarine tube's time). There is a second at least between launches.
- **On the wire.** They show as the vehicle's `arms` rack, with `bombs` the tubes loaded. The HUD draws them as four pips over the gun's heat bar, and the line of keys counts them.

### In a headset

The right index trigger fires the gun, and pulling the left launches a missile ([VR.md](VR.md)).

## How it is built

"Kinematic plus ragdoll" came out as three kinds of part:

- **The torso is a dynamic body** held up by springs, as a walker's hull is (`Lift::Walker` and `walk()` in `crates/sim/src/vehicle/flight.rs`), and a capped torque keeps it upright, so it can be knocked over. A walker's springs are ray casts down from its hips, so it floats over the ground wherever it goes. The Titan's go to its planted feet ("The walk", below).
- **The fists and the feet are kinematic bodies** of their own (`crates/sim/src/vehicle/limbs.rs`), which the torso does not feel.
  - A fist goes where it is asked to within its arm's reach (6 to 24 m from the shoulder) and outside the torso's box, speeding up and slowing at 140 m/s². With nobody aboard, or fallen, or wrecked, it goes to where it hangs.
  - A planted foot does not move. The two step in turn ("The walk", below).
  - They shove any dynamic body they meet. The Titan's own hull leaves their collision group (`LIMB_GROUP`) out of what it touches, so they never push it about. This also means one Titan's fist does not shove another's torso: the server knocks it instead.
- **The arms and legs between are only drawn**, by two-bone IK on the client (`client/src/render/titan.ts`) from the fists and feet the server sends. The thighs are solids on the hull, short enough to clear the ground when it crouches.
- **Rounds meet the model's shape, not these solids** ("Where rounds meet it", below).

The server alone decides what a fist hits or holds: `crates/server/src/mechanics/titans.rs`, a `Mechanic` that runs before the vehicles'. A fist is solid and pushes a soldier along ahead of it, so a punch counts when the soldier is within the fist's next two ticks of travel, not once the fist is in them.

## Where rounds meet it

The box it walks with is narrower than its pauldrons, shorter than its head and wider than its waist, and its arms had nothing in them. So rounds are not shot at its physics at all (`Physics::cast_ray_hit` and `Physics::armor_strike` in `crates/sim/src/vehicle.rs`), but at:

- **The torso as it is built**: seventeen `Role::Hit` solids in `titan.rs` from the numbers in `build_titan.py` (the chest and waist as `Shape::Frustum`s, the builder's `taper`; the breastplate, hips, codpiece, flanks, collar, head, ear plates, face, crest, pauldrons, reactor and its stacks). A `Role::Hit` solid is never a collider: it pushes and carries nothing, and `extra` is made from the other solids alone (`physical`).
- **Its arms and legs as they are drawn**: a capsule along each bone (`Physics::hit_bones`), bent by the same two-bone IK as the client's (`titan::bend` in `crates/shared`), with the same bone lengths and poles (`TITAN_*_LENGTH`, `TITAN_ELBOW_POLE`, `TITAN_KNEE_POLE`). `the_bend_fixture_matches_the_rust_bend` writes its inputs and answers to `client/src/__tests__/titan-bend.fixture.json` for the client's bend to be checked against.
- **Its fists and feet**, which are colliders anyway.

Every round, line of sight and probe that goes through `cast_ray_hit` sees it so. Other queries (a soldier walking into it, a car's wheels) still meet the box. A strike on any of it is a strike on the hull for its armour (`solid: None`): all its plates are on the hull, and the thighs, which used to count as a bare solid, now meet them too. A point on a limb is given in the hull frame, where the limb is at that moment.

**A round's knock.** A shell, a rocket or a cannonball that strikes any vehicle gives it its `Burst::hit_knock` (`*_HIT_KNOCK`, from 0.6 m/s for an armour-piercing shell down to 0.15 m/s for an autocannon round; a glance gives `RICOCHET_KNOCK_SHARE` of it), pushed where it struck (`Physics::strike_car`), so a tank hit high on its side rocks away from it. A legged kind takes its `blast_share` of the push and its `hit_share` of the turn: the Titan's 4.5 of the turn rocks it about 2° from an armour-piercing shell on its chest, still again in under a second and a half, without sliding it back. Bullets give none.

## The walk

The first Titan floated: its torso slid along at one height and its feet pattered after it. Now the feet lead (`Gait::feet`, a `Feet`; `step_limbs` in `limbs.rs` and the `planted` branch of `walk()`).

- **A step has three parts** (`Step`): both feet planted; the weight shifting off the foot that is to step, in 0.35 s; that foot in the air for 1.3 s.
  - A foot is due to step once the torso has gone half a stride past it, or has no foot ahead of it to go onto. The stride is how far it goes in one step's time, between 10 and 18 m.
  - In the air the foot heads for where its hip will be when it lands plus half a stride, worked out again every tick, so it lands right however the pilot steers meanwhile. It carries no weight.
  - At rest a foot more than 2 m out of place is due; turning, it leads the turn by half a step's time.
  - With the stick let go it only brings its feet back under it, unless it is going faster than 3 m/s (stopping, or thrown by a blow), when it steps to keep up.
- **What the feet do to the torso**, each by the share of the weight on it:
  - *Height.* The leg is a strut of the leg's length from the foot, so it holds the pelvis at `sqrt(length² − out²)` over a foot that is `out` away along the ground. The spring and damper are the walker's.
  - *Sway.* It is pulled 30 % of the way from the middle of its feet towards the one that carries it, and leans 0.04 rad that way.
  - *Pace.* Its top speed is 30 % less straight over a foot than half a stride from it.
- **A foot points the way it was put down** while the torso turns over it. The physics keeps this for the solid shin; the client works it out again from `step` for the drawing.
- **The pilot's prediction.** Where the Titan goes now depends on its feet, so `titan` carries `step` and `weight`, and the pilot's client puts its own copy's feet back to the server's at every snapshot before replaying (`Physics::set_feet`, `ClientSim::setCarFeet`, `VehicleDriver.reconcile`). `a_titans_feet_put_back_where_a_snapshot_has_them_walk_on_the_same` checks that a copy restored from what the wire carries is within 15 cm half a second on (it measures about 1 mm).
- **The walker, the hen and the tupp are untouched**: they have no `Feet`, and `walk()` takes the old path for them.

## Flying and the slam

- **The thrusters** are `jet()` in `crates/sim/src/vehicle/flight.rs`, which runs after `walk()` as the hen's `flap()` does, on the kind's `Gait::jets` (a `Jets`) and its `Spec::tank` of fuel. The hen and the Roadster already replicate a tank (`thrust`) and restore it in the pilot's prediction, so the Titan's fuel needed nothing new on the wire.
- **Off the ground** is a fourth `Step`, `Airborne`. It begins when the torso is lifted clear of every foot that carried it (a leap, or a blast). The feet then hang under the hips, drawn up to 93 % of a leg's length, and carry nothing. When a hanging foot meets the ground on the way down both are planted there, and `LimbsPose::impact` is how fast it was falling in that tick.
- **The landing** is taken by the same leg springs: from 38 m/s the torso sinks about 6.5 m and comes back up (`a_titan_that_comes_down_hard_lands_on_its_feet_and_its_thrusters_break_a_fall`).
- **The slam** (`slam()` in `titans.rs`) is a `combat::Blast` given through `blast_sparing`, which leaves out the Titan itself and whoever it holds. A blast shoves every vehicle alike, so the slam throws them once more by its own strength (`Physics::knock_car`).
- **On the client** the flames are `thrusterFlame` and the landing's dust `slamDust` (`client/src/render/tyres.ts`). `TitanModel` reports a landing once with how fast it came down (`takeSlam`), and the presenter adds the thud, a blast's boom and a shake of any camera within four radii.
- **In a headset** the right hand's A button is jump ([VR.md](VR.md)).

## Legs as data

The walker's legs were global `WALKER_*` constants. They are now a `Gait` on the kind's `Spec`, so that something ten times the size can walk the same way:

- `Gait::WALKER` holds the old values, and the walker, the hen and the tupp use it. Their tests and fixtures pass unchanged.
- `TITAN_GAIT` is the Titan's. What it adds:
  - `crouch` and `crouch_lean`, which a pilot's `lift` control works (`Buttons::Stride` puts C on it);
  - `rise`, the fastest its legs straighten (7 m/s), so that standing up from 16 m down does not throw it into the air;
  - a `getup_share` over 1 (3), eased back to 1 as it comes upright, because lifting that weight off the ground takes more than keeping it standing;
  - `blast_share`, and `feet`: the stride, the times, the sway, the tilt, the change of pace, and the share of its speed it keeps crouched.
- `Spec::arms` is its `Arms`, and `Body::tough` its share of any harm.

Its hull's middle settles at `TITAN_RIDE_HEIGHT` (33.2 m), measured by `a_titan_stands_fifty_metres_high_on_its_legs`.

## The model

`tools/vehicles/build_titan.py` builds `client/public/models/titan.glb` (about 23,000 triangles: its big plates are cut into a 1.2 m grid, `DENT_GRID`, for a shell's dent to press in, [HULL-DAMAGE.md](HULL-DAMAGE.md)): slabs of grey armour over a dark frame, a glowing core in the chest and a glowing visor. Every limb is its own node hanging straight down from its joint (`upper_L/R`, `fore_L/R`, `fist_L/R` with `open_` and `closed_` fingers, `thigh_L/R`, `shin_L/R`, `foot_L/R`), and none is another's child: the client places each. `gun_R` is the autocannon, a child of the right fist, its barrel down the forearm's line out to `GUN` (`TITAN_GUN_LENGTH`). The missile launcher, with its four hatches, is part of `torso`.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_titan.py -- --preview <dir>
```

## Protocol

Protocol 79 ([NETWORKING.md](NETWORKING.md)): 77 brought the Titan, 78 the `step` and `weight` of its walk, 79 the `step` of being off the ground.

- **`hands`** (client to server, tag 3): where the pilot wants the two fists, in the hull frame, and which hands are closed. Sent every tick while piloting, apart from the `input` frames, so that a soldier's input did not grow.
- **`titan`** (component 28): the fists in the hull frame, the feet from the hull's centre along the world's axes, which hands are closed and full, and where it is in its walk (`step`, `weight`).
- **`held`** (component 29): the Titan and the hand a soldier is held in.
- The pilot's own Titan is predicted as any driven vehicle is. Its fists and feet are drawn from the server's snapshots, so they answer about a tenth of a second after the mouse.

## Tests

- **`crates/sim/src/vehicle/tests_hits.rs`**: an armour-piercing shell on its chest rocks it and it stands again; a shell on a tank's side rocks it away from the hit; rounds meet its arms, its legs where they bend, its head, crest and pauldrons, and miss past its waist inside the box.
- **`crates/shared/src/vehicles/titan.rs`**: a bend keeps its bones' lengths and bends towards its pole, and matches the fixture the client's is checked against.
- **`crates/sim/src/vehicle/tests.rs`**: it stands at its ride height with a foot under each hip; walks on one foot at a time with long slow steps, a planted foot never sliding, rising over each foot and going slowest there, stops, turns on the spot and stands again on both; walks on the same from feet put back as a snapshot has them; crouches to bring a fist to the ground and stands again; a fist stays within its reach and its arm's speed, out of its own chest, and hangs when nobody is aboard; a fist shoves a jeep, a closed one carries it off and throws it; a shell's blast barely moves it, a great blow fells it and it gets up; it leaps, its thrusters carry it up and along while the fuel lasts and no higher than its ceiling, and its feet hang; let fall from its ceiling it lands on its feet at a slam's speed, and with the thrusters fired it sets down; put back in the air as a snapshot has it, it flies on the same.
- **`crates/server/src/mechanics/titans/tests.rs`**: a closed hand picks a soldier up, carries them high, they still fire, and they fly when it opens; an open hand coming down on a soldier does no harm; a closed fist swung through one throws and hurts them; the arms hang unless its own pilot moves them, and when the pilot stops; a foot crushes who is under it and throws those around it, and wrecks a jeep; it feels a tenth of a shell and nothing of a rifle; a fist fells a tree and holes a house; a slam throws and hurts soldiers, shakes those further out, throws and breaks a jeep, fells the tree by it and breaks open the house, and is not felt by the Titan; one that leaps and falls slams where it lands, one set down gently does not, and whoever is in its fist comes down unhurt; a placed one stands, is boarded from between its feet, walks and sets its pilot down; a shell its chest plate stops still rocks it.
- **`client/src/__tests__/titan.spec.ts`**: the model's fists are where the server has them, turned with the hull, on arms of the built length; it burns under both feet while its thrusters fire and reports a hard landing once; it stands on the planted feet, lifts the one in the air and reports a footfall when it comes down; a planted foot keeps pointing the way it was put down while the hull turns; the mouse-and-keyboard guard, punch, reach, grip and carry.

## Not yet

- **VR has not run on a headset** ([VR.md](VR.md)).
- **The pilot's own arms lag** by the round trip to the server, since they are not predicted.
- **Nothing shakes when a foot comes down**: there is the thud, and the camera rides the torso's rise and fall, but no jolt, no dust, and the arms do not swing with the walk (the pilot holds them).
- **The new walk, the flight and the slam have not been played**, only measured in tests. The flames and the dust were looked at once in the engine and the flames then doubled in size, which was not looked at again.
- **A slam's dust and shake are the client's own guess** from how fast it saw the Titan come down: the server sends no event for it.
- **Its arms and fists do nothing special in the air**, and a fist does not hit harder for the fall.
- **A swinging foot does not kick.** A foot hurts only where it comes down. A soldier in the way of one that is moving is pushed by it, as by any solid.
- **It walks over houses**, not through them: its legs stand on a roof as on a hill, until its feet have brought the house down.
- **Bots do not pilot it**, and do not know to run from it.
- **Not played.** The numbers (punch and throw damage, its toughness, its speed) are first guesses.
