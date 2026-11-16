# Vehicles XIX: the Stridsbåt 90

The Stridsbåt 90 (CB90) is the Swedish navy's fast assault craft, here at its real size: 15.9 m long, 3.8 m across and 2 m from the keel to the deck. Two waterjets push it to 40 knots. It carries a driver, a gunner and two riders. The driver steers and works twin Ksp 58 machine guns on the bow, which follow their look. The gunner lays a 25 mm automatic cannon in a turret on the wheelhouse roof, which turns all the way round. The riders sit on the open after deck and fire their own guns.

## In play

- **The boat** (`KIND_CB90` = 34) has 200 hp.
- **Placement.** It appears only where it is placed in the map editor (**Stridsbåt 90**, L in the palette), and only on open water: placed on land or on ice, it is skipped. The automatic line-up on worlds without editor placements does not include it. Vindeln has one, 33 m south of the pirate ship.
- **It floats** 0.8 m deep and rides the waves as the ship does. It is boarded from the after deck, behind the wheelhouse, and whoever leaves it stands on the after deck.
- **Seat 0 is the driver** (the HUD's `coxswain`).
  - W drives it ahead. It reaches 30 knots in about eight seconds and tops out at 20.6 m/s (40 knots).
  - S, while it still runs ahead, drops the reversing buckets: it stops from full speed in under three lengths (about 40 m). Held on, it goes astern at up to 4 m/s. Space does the same as the buckets.
  - Let go, it drops off the plane and slows.
  - A/D steer it even standing still, the jets swinging it round at 0.3 rad/s. From 5 m/s on it turns at 0.45 rad/s, a circle about 90 m across at full speed.
  - As it speeds up its bow lifts, most at about 6 m/s, while it climbs onto the plane. From about 14 m/s it is on the plane: its middle rides 0.35 m higher, its bow only a little up.
  - It banks into its turns, up to 0.2 rad.
  - Driven at a bank, it stops against it. A wreck fills and sinks.
- **The bow guns.** The driver's camera sits behind and above the twin machine guns on their post at the front of the troop compartment. The mouse aims the guns while W/S/A/D steer, as the walker's driver aims its chin guns.
  - They turn 2.2 rad (126°) either side of dead ahead, which keeps them clear of the wheelhouse, and elevate from −17° to +52°.
  - The two barrels take turns, twenty rounds a second between them. Each round is a 7.62 mm bullet: 14 damage, 3 to a vehicle, 6 to an aircraft, out to 350 m, through 10 mm of plate up close.
  - Holding the trigger jams them after about five seconds. The HUD shows their heat as it does a tank's roof gun.
- **Seat 1 is the gunner,** who lays the cannon in the roof turret. It is the Pbv 302's gunner's seat, with a gun sight and the heat gauge.
  - It turns all the way round at 1.8 rad/s and elevates from −6°, which keeps its rounds over the bow guns, to +52°, steep enough for aircraft.
  - It fires seven and a half rounds a second. Each is a 25 mm bullet: 50 damage, 12 to a vehicle, 24 to an aircraft, out to 500 m, through 35 mm up close.
  - Holding the trigger jams it after about five seconds.
- **Seats 2 and 3 are riders** on the after deck. They fire their own guns, as a car's passengers do. Neither the driver nor the gunner can fire their own gun: their hands are on the boat's.
- **The crew can be shot.** It is `Exposure::Glazed`: the driver and the gunner sit behind the wheelhouse's glass and the riders in the open, all drawn as soldiers.
- **The armour** is welded aluminium: 5 mm on the hull, 6 mm on the troop compartment, 8 mm on the wheelhouse and the turret. That is proof against splinters but not against a rifle up close. The two diesels are under the after deck with the fuel ahead of them, and the cannon's rounds are in the hull under the turret. Shooting out the breech stops the cannon. The turret can be blown off, as a tank's can.

## What changed to make it

- **`Lift::Boat`** (`crates/shared/src/vehicles/spec.rs`) is a planing hull on waterjets, and `crates/sim/src/vehicle/boat.rs` moves it. Like `sail` and `dive` it sets the body's velocities from its state, the controls and the tick alone, so the client predicts its own boat with the same code. Its tuning numbers are the `CB90_*` constants. Its tests in `boat.rs` cover floating at its draft, top speed and planing, stopping, turning on the spot, banking, the shore and sinking.
- **`Lift::afloat`** says whether a kind is a hull that only floats: a ship, a submarine or a boat. Such a kind starts on the water, a wreck of it fills and sinks rather than being thrown, and it drifting alongside a swimmer only nudges them. These were matches on `Sail | Dive` before.
- **A hull's own gun can be shown as the `pintle`.** A vehicle replicates at most one `turret`, which here is the cannon. The bow guns are `Show::Pintle` with `Aim::Mount`, so their yaw on the wire is from the hull, not from the turret as the tank's roof gun's is. The wire format is unchanged.
- **`crew_gun`** now leaves out every seat that works one of the vehicle's mounts, not only a `mount_seat`. The client reads which seats those are from the new `gunners` list in `tables.generated.json`.

## The client

- **`client/src/kinds/cb90.ts`** is the kind.
  - The driver's seat is the `pintle` rig with `hull: true` and its own camera offset (`CB90_AIM_BACK`, `CB90_AIM_UP`), so the look is kept within the guns' arc of the hull.
  - The new role `coxswain` both moves the boat and aims with a reticle, and has its own line of key hints.
  - The gunner's seat is a `gun` rig. Its wake comes from `fx.floats`, and the tank music plays for its crew.
- **`client/src/render/cb90.ts`** is `Cb90Model`, a `TankModel` with no wheels. Its turret and cannon are `turret` and `gun`, and its bow guns are `mg` and `mg_gun` on the hull. `bowGun(side)` gives either barrel, and the roof-gun presenter fires them in turn. `TankLayout.driverHead` may now be `null`, as the crew are drawn as soldiers.

## The model

`tools/vehicles/build_cb90.py` builds `client/public/models/cb90.glb`, about 1,200 triangles.

- **The hull** is a deep V, square at the transom with the two jet nozzles and their buckets. Its sides flare out above the chine. Its deck climbs towards a raked stem, and it narrows to a fine, pointed bow over its forward half (`CB90_BOW` is 4.5 m for the collider). A rubbing strake runs along the sheer abaft the bow.
- **On the deck:**
  - the bow ramp folded flat on the fore deck;
  - one long, low, faceted superstructure with sides leaning in, its front narrowing to follow the bow: the troop compartment with portholes under the low roof, with the bow guns on its front edge, then the wheelhouse raised behind it, with its raked windscreen, glass all round, the turret on the front of its roof, a life ring each side and the radar mast behind;
  - the after deck with the engine hatch, a bench either side for the riders and a rail round it.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_cb90.py -- --preview <dir>
```

## Protocol

`PROTOCOL_VERSION` is unchanged: the only new thing on the wire is the kind id, `KIND_CB90` (34).
