# Vehicles XVII: the Pbv 302

The Pbv 302 is an amphibious tracked troop carrier, modelled on the Swedish Pansarbandvagn 302 of the 1960s. A driver, a gunner and two troops ride in it, the same four seats as the Strf 90. The gunner works a 20 mm automatic cannon in a small turret at the front left, and the troops ride sealed in the back and get out by the two rear doors. It is nearly as quick as the Strf 90, thinner-skinned and far more lightly armed, and it is the first wheeled or tracked kind that floats: it drives into a lake, swims across on its tracks and climbs out on the far shore.

## In play

- **The Pbv 302** (`KIND_PBV` = 32) has 150 hp; the Strf 90 has 180 and the tank 250.
- **Placement.** It appears only where it is placed in the map editor (**Pbv 302**, A in the palette). The automatic line-up on worlds without editor placements does not include it. The home world has two, each beside a Strf 90.
- **Seat 0 is the driver.** The controls are the tank's: it pivots on the spot. It reaches 18.3 m/s (66 km/h), between the tank's 12 and the Strf 90's 19, and stays upright at full stick flat out.
- **On water it floats.** Driven into open water it rises off the bed once the water is about 1.3 m deep and rides with its roof 0.4 m clear, level, rising and falling with the waves.
  - Afloat its tracks paddle it at up to 2.5 m/s (9 km/h) ahead and half that astern, and turn it at 0.5 rad/s, on the spot too.
  - It comes into the water no faster than that for long: the water takes off what speed it brings from the land within a second or two.
  - Where the bed shelves up, its tracks reach it and drive it out as on land. A bank too steep to drive up on land is too steep to leave the water by.
  - A broken track paddles nothing, so with one gone it is pushed half as hard and turns half as fast.
  - A wreck fills and sinks to the bed, as every other kind does.
  - A frozen lake is driven across, as by every kind.
- **Seat 1 is the gunner**, who aims the turret all the way round with the mouse.
  - The turret slews at 1.8 rad/s and the barrel at 1.4.
  - The barrel elevates from −10° to +37°.
  - Holding the trigger fires ten rounds a second.
  - The barrel heats as it fires and jams after about four seconds of holding the trigger. The heat bar shows how close it is, and the gun clears again once it has cooled.
- **Seats 2 and 3 are troops.** They ride sealed in the back, out of sight and out of reach of small arms. Anyone who leaves the Pbv stands behind it at the doors, the gunner and driver included. Where the doors are blocked, they are set down beside it as from any car.
- **The round** is a solid 20 mm slug, not a shell: it flies as a machine gun's bullet does, out to 400 m, and does not burst. It takes 45 off a soldier, 10 off a vehicle's hull and 20 off an aircraft, and goes through 28 mm close in and 16 mm at the end of its range. That is through a car, another Pbv 302's sides, roof and doors, and a Strf 90's side up close. A tank's front and sides are proof against it; its rear, roof and engine grille are not.
- **The armour** is thin all round: 23 mm in front, where the glacis lies 38° above level (38 mm along a level shot), 14 mm sides, 10 mm rear doors, and 8 mm on the roof and underneath. The turret is 12 mm. The driver's periscope and the gunner's sight are weak spots. Every seat has a crew module, so a round that gets in can kill a troop in the back. Every plate is at least 8 mm, so it still counts as armoured against splash.

## Swimming

`Spec` has a new field, `swim: Option<Swim>`, `None` for every other kind. A `Swim` is the hull's `draft`, its top `speed` afloat, the `thrust` of its tracks there and its `turn` rate.

`wade` in `crates/sim/src/vehicle/water.rs` hands a kind that has one to `swim` instead of sinking it:

- **Buoyancy.** The water lifts it by gravity times the share of its draft that is under, so it rests at its draft, and its rise and fall follow the surface under it (the waves are `shared::waves`, on the tick). Dropped in deep, it comes back up.
- **Afloat** is from 60 % to 90 % of its draft under: that is where its tracks leave the bed. There the tracks push it, against a top speed, the water takes out its sideways drift, and its yaw rate follows the stick.
- **In the shallows** the tracks still touch, and the usual wheel physics drives it. The water carries part of its weight there, so it has less grip the deeper it is.
- **Drag** takes speed over its swimming speed off quickly, so a hull driven in at full speed, or still pushed along the bed, slows to it.
- **Level.** It is brought level with the water under it, as a ship is.

Like `wade` and `sail`, it is a function of the body's state, the controls and the tick alone, so the client's prediction (the same code as WASM) swims with the server.

## The definition

`crates/shared/src/vehicles/pbv.rs` holds everything about it.

- **The hull** is the real one's size, 2.86 × 1.4 × 5.35 m, with a blunt boat's side outline of its own (as the other tracked kinds have, [VEHICLES-XVI.md](VEHICLES-XVI.md)), at 654 kg/m³: 14 t. Its engine pulls 110 kN.
- **The axles** sit well out towards the ends of the hull (`PBV_AXLES_Z`), which keeps its tail off the ground on a steep climb.
- **It rides** `PBV_RIDE_HEIGHT` (1.1 m) above the ground. The sim's `a_pbv_settles_at_its_ride_height` measures it.
- **It floats** `PBV_DRAFT` (1.0 m) deep, and swims at `PBV_SWIM_SPEED`.
- **The turret** is a weightless solid cylinder at `PBV_TURRET_PIVOT`, left of the hull's middle, as the tank's turret is, so a hit on it counts against the turret.
- **The cannon** is one `MountDef` row: `Aim::Turret`, `Fire::Bullet` (the tank's roof gun's way of firing) with a heavier round, and `heat` for the jam. No gunnery code is new.
- **The doors** are `exit: Some(PBV_DOOR_EXIT)`, as the Strf 90's ramp is.

## The client

- **`client/src/kinds/pbv.ts`** is the kind. It has four seats, the gunner's with the `heat` gauge, a diesel a little lower than the Strf 90's, and the engine bay under the roof grille behind the driver, where a hurt engine smokes.
- **`client/src/render/pbv.ts`** is `PbvModel`, a `TankModel` on its own `.glb` and layout, in the Strf 90's splinter camo. The driver's and gunner's heads show in their hatches. The troops are never drawn.
- **The gun presenter** (`Turrets` in `guns.ts`) draws each round on the turret's counter twice over: the autocannon's flash, smoke and report at the muzzle, and a bullet in flight out to what it meets, since the server replicates no projectile for it. A `Cannon` row's new `range` field asks for the bullets. The tank music plays for its crew.
- **Afloat** nothing is new on the client: the prediction swims it, and its hull throws the spray any hull in the water does.

## The model

`tools/vehicles/build_pbv.py` builds `client/public/models/pbv.glb`, about 2,350 triangles. It has the node names of the tank's model.

- **The hull** is a boat: the full width over the tracks, with a blunt bow: a bow plate rising from the belly to a short upright nose plate at mid height, and a glacis leaning well back from it with the trim vane folded on it. The vane is for show and stays folded afloat.
- **The roof** carries the driver's hatch and periscopes in the middle of the front, the commander's hatch to the right, the engine's grille, two long hatches over the troops, three smoke dischargers at each rear corner (for show) and a whip aerial; the upright rear has the two doors.
- **The running gear** is five road wheels a side, with a drive sprocket at the front and an idler at the back, under a skirt.
- **The turret** is a small drum leaning in to a flat roof, with a hatch, vision blocks and the sight's head, and a long thin barrel with a muzzle brake out of a boxed cradle in its face.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_pbv.py -- --preview <dir>
```

## Protocol

The layout of the messages is unchanged. `KIND_PBV` (32) is a new value in an existing field, so `PROTOCOL_VERSION` stays where it was.
