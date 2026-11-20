# Vehicles XIX: the Stridsbåt 90

The Stridsbåt 90 (CB90) is the Swedish navy's fast assault craft, here at its real size: 15.9 m long, 3.8 m across and 2 m from the keel to the deck. Two waterjets push it to 45 knots, and its reversing buckets stop it dead in under two lengths. It carries a driver, a gunner and two riders. The driver sits in the low cockpit forward. They steer, and fire twin Ksp 58 machine guns fixed on the cockpit's front, which only elevate, so the boat aims them as a Strv S aims its gun. Behind the cockpit the gunner stands in an open ring and turns a 25 mm automatic cannon all the way round. The riders sit on the open after deck and fire their own guns.

## In play

- **The boat** (`KIND_CB90` = 34) has 200 hp.
- **Placement.** It appears only where it is placed in the map editor (**Stridsbåt 90**, L in the palette), and only on open water: placed on land or on ice, it is skipped. The automatic line-up on worlds without editor placements does not include it. Vindeln has one, south of the pirate ship.
- **It floats** 0.8 m deep and rides the waves as the ship does. It is boarded from the after deck, behind the wheelhouse, and whoever leaves it stands on the after deck.
- **Seat 0 is the driver** (the HUD's `coxswain`).
  - W drives it ahead, hard: it reaches 30 knots in under five seconds and tops out at 23 m/s (45 knots).
  - S, while it still runs ahead, drops the reversing buckets for a crash stop, at 10 m/s². From flat out it stops in about 26 m, well inside two lengths. Space does the same.
    - Its bow digs in, up to about 6° down, and its hull squats into the water, about 0.15 m deeper than at rest. Both are most at planing speed and ease off as it stops (`CB90_BRAKE_DIP`, `CB90_BRAKE_SQUAT`).
    - The reversed jets blast sheets of water forward along both sides past the bow, the bow ploughs up a wall of it, and the boat runs into its own cloud of mist and its own wave.
    - Held on once it has stopped, it goes astern at up to 4 m/s, level again.
  - Let go, it drops off the plane and slows.
  - A/D steer it even standing still, the jets swinging it round at 0.3 rad/s. From 5 m/s on it turns at 0.45 rad/s, a circle about 100 m across at full speed.
  - As it speeds up its bow lifts, most at about 6 m/s, while it climbs onto the plane. From about 14 m/s it is on the plane: its middle rides 0.35 m higher, its bow only a little up.
  - It banks into its turns, up to 0.2 rad.
  - Driven at a bank, it stops against it. A wreck fills and sinks.
- **The machine guns.** The driver's camera sits behind and above the twin machine guns, as a Strv S driver's sits behind its gun ([VEHICLES-XVI.md](VEHICLES-XVI.md)). A/D turn the boat to aim them, and the view eases back behind them when the mouse rests.
  - They are fixed ahead and only tilt, from −9° to +11°, at 0.8 rad/s.
  - The two barrels take turns, twenty rounds a second between them. Each round is a 7.62 mm bullet: 14 damage, 3 to a vehicle, 6 to an aircraft, out to 350 m, through 10 mm of plate up close.
  - Holding the trigger jams them after about five seconds. The HUD shows their heat as it does a tank's roof gun.
- **Seat 1 is the gunner,** standing in the ring on the troop compartment's roof, out in the open, as a technical's gunner stands in its bed. They turn round with the gun and can be shot.
  - The cannon turns all the way round at 1.8 rad/s. It elevates from −5°, which keeps its rounds over the cockpit and its machine guns, to +52°, steep enough for aircraft.
  - It fires seven and a half rounds a second. Each is a 25 mm bullet: 50 damage, 12 to a vehicle, 24 to an aircraft, out to 500 m, through 35 mm up close.
  - Holding the trigger jams it after about five seconds. The heat gauge shows how hot it is.
- **Seats 2 and 3 are riders** on the after deck. They fire their own guns, as a car's passengers do. Neither the driver nor the gunner can fire their own gun: their hands are on the boat's.
- **The crew can be shot.** It is `Exposure::Glazed`: the driver sits behind the cockpit's glass, the gunner stands in the ring and the riders sit in the open, all drawn as soldiers.
- **The armour** is welded aluminium: 5 mm on the hull, 6 mm on the troop compartment's roof, 8 mm on the cockpit and the ring. That is proof against splinters but not against a rifle up close. The two diesels are under the after deck with the fuel ahead of them, and the cannon's rounds are in the hull under the ring. Shooting out the breech stops the cannon.

## What changed to make it

- **`Lift::Boat`** (`crates/shared/src/vehicles/spec.rs`) is a planing hull on waterjets, and `crates/sim/src/vehicle/boat.rs` moves it. Like `sail` and `dive` it sets the body's velocities from its state, the controls and the tick alone, so the client predicts its own boat with the same code. Its tuning numbers are the `CB90_*` constants, which it now reads from the `Boat` that `Lift::Boat` carries (`CB90_HULL`; [VEHICLES-XX.md](VEHICLES-XX.md)). Its tests in `boat.rs` cover floating at its draft, top speed and planing, stopping, turning on the spot, banking, the shore and sinking.
- **`Lift::afloat`** says whether a kind is a hull that only floats: a ship, a submarine or a boat. Such a kind starts on the water, a wreck of it fills and sinks rather than being thrown, and it drifting alongside a swimmer only nudges them. These were matches on `Sail | Dive` before.
- **A hull's own gun can be shown as the `pintle`.** A vehicle replicates at most one `turret`, which here is the cannon. The machine guns are `Show::Pintle` with `Aim::Bore`, the Strv S's gun's aim, so their yaw on the wire is always 0 and only their pitch changes. The wire format is unchanged.
- **`crew_gun`** now leaves out every seat that works one of the vehicle's mounts, not only a `mount_seat`. The client reads which seats those are from the new `gunners` list in `tables.generated.json`.

## The client

- **`client/src/kinds/cb90.ts`** is the kind.
  - The driver's seat is the Strv S's `tanker` rig, which now takes the gun's pitch and its camera's offset (`CB90_AIM_BACK`, `CB90_AIM_UP`).
  - The new role `coxswain` both moves the boat and aims with a reticle, and has its own line of key hints.
  - The gunner's seat is a `gun` rig with `stand`. The tank music plays for its crew.
- **Its spray** comes from `fx.drive: 'jets'`: `VehicleFx.jets` (`render/tyres.ts`) hands the boat to `JetSpray` (`render/jetspray.ts`), which throws a rooster tail off the jets under way in layers of streaks, drops, mist and glints ([WATER-WAVES.md](WATER-WAVES.md)). Behind its transom it leaves a white wash in the water (`VehicleFx.wash`).
  - The sheets off its bow and the crash stop's storm of spray (sheets forward past the bow, a wall of water, a cloud of mist) were taken out in [VEHICLES-XX.md](VEHICLES-XX.md)'s round: they did not look like water. Its bow still digs in and its hull squats in a crash stop.
- **Its wake** comes from `fx.floats`: its hull is pushed into the water's wave field (`render/waterwaves.ts`) as it moves, which raises its bow wave and the V behind it.
- **`client/src/render/cb90.ts`** is `Cb90Model`, a `TankModel` with no wheels. Its cannon's pedestal and barrel are `turret` and `gun`, and its machine guns are `mg` and `mg_gun` on the hull. `machineGun(side)` gives either barrel, and the roof-gun presenter fires them in turn. `TankLayout.driverHead` may now be `null`, as the crew are drawn as soldiers.

## The model

`tools/vehicles/build_cb90.py` builds `client/public/models/cb90.glb`, about 1,900 triangles, from bow to stern:

- **The hull** is a deep V, square at the transom with the two jet nozzles and their buckets. Its sides flare out above the chine, and it narrows to a fine, pointed bow with a raked stem (`CB90_BOW` is 4.5 m for the collider). A rubbing strake runs along the sheer, fenders hang on its sides and the troop compartment's portholes are in them.
- **The fore deck** is long and bare, with the bow ramp flat on it.
- **The cockpit** is low, about 1.15 m over the deck, with its front raked, three windows across it and a window band round its sides. The twin machine guns sit on its roof's front edge, and the windscreen's frame bars reach forward over the fore deck.
- **The gunner's ring** is behind it, with a railing round it and a life ring on it, and the cannon on a pedestal in it. A mast stands either side.
- **The troop compartment** is down in the hull. Its roof is only 0.35 m over the deck, with rails along it, vents and a hatch.
- **The after deck** is open, with the engines' hatch, a bench either side for the riders, a rail round it, a life ring and the radar mast on the right.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_cb90.py -- --preview <dir>
```

## Protocol

`PROTOCOL_VERSION` is unchanged: the only new thing on the wire is the kind id, `KIND_CB90` (34).
