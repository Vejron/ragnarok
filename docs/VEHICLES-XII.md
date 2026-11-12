# Vehicles XII: the Strf 90

The Strf 90 is a tracked infantry fighting vehicle, modelled on the Swedish CV90. A driver, a gunner and two troops ride in it. The gunner works a 40 mm autocannon in a turret set forward of the middle, and the troops ride sealed in the back and get out by the rear ramp. It is quicker than the tank, with thinner armour.

## In play

- **The Strf 90** (`KIND_STRF` = 28) has 180 hp; the tank has 250 and the Strv S 220.
- **Placement.** It appears only where it is placed in the map editor (**Strf 90**, I in the palette). The automatic line-up on worlds without editor placements does not include it.
- **Seat 0 is the driver.** The controls are the tank's: it pivots on the spot. It reaches 19 m/s (about 70 km/h) against the tank's 12, and stays upright at full stick flat out.
- **Seat 1 is the gunner**, who aims the turret all the way round with the mouse.
  - The turret slews at 2 rad/s and the barrel at 1.4; the tank's slew at 1.4 and 0.9.
  - The barrel elevates from −8° to +49°, steep enough to fire at aircraft.
  - A click fires five rounds a second.
  - Q (`BUTTON_AMMO`) swaps between armour-piercing rounds, loaded first, and high explosive. The HUD shows which is loaded.
  - The barrel heats as it fires and jams after about five seconds of holding the trigger. The heat bar shows how close it is, and the gun clears again once it has cooled.
- **Seats 2 and 3 are troops.** They ride sealed in the back, out of sight and out of reach of small arms. Anyone who leaves the Strf stands behind it at the ramp, the gunner and driver included. Where the ramp is blocked, they are set down beside it as from any car.
- **The rounds:**
  - **High explosive** is the helicopter's and walker's autocannon round (`PROJECTILE_CANNON`). It bursts a third as wide as a tank's high explosive shell and goes through 20 mm.
  - **Armour-piercing** is new: `PROJECTILE_CANNON_AP` (12). It goes through about 45 mm close in and 35 mm far out, then bursts behind the plate with half the charge of a tank's armour-piercing shell. That is through the Strf's own sides and the tank's sides, roof and engine deck, but not the tank's sloped glacis or mantlet.
- **The armour** is light all round: a 30 mm glacis sloped at 30°, 30 mm on the turret's face, 18–20 mm sides, 10 mm on the roof and the rear ramp, and 8 mm underneath. The driver's periscope and the gunner's sight are weak spots. Every seat has a crew module, so a round that gets in can kill a troop in the back.

## The definition

`crates/shared/src/vehicles/strf.rs` holds everything about it.

- **The hull** is a 3.0 × 1.2 × 6.4 m box at 600 kg/m³: about 13.8 t, a little more than the tank. It has more engine (115 kN) and the same firm springs.
- **It rides** `STRF_RIDE_HEIGHT` (1.01 m) above the ground. The sim's `a_strf_settles_at_its_ride_height` measures it.
- **The turret** is a weightless solid cylinder on the ring at `STRF_TURRET_PIVOT`, as the tank's is, so a hit on it counts against the turret. It is shot off at the same `TANK_TURRET_HP` as the tank's.
- **The cannon** is one `MountDef` row: `Aim::Turret`, `Fire::Shell` with the armour-piercing and high explosive rounds, and `heat` for the jam. No gunnery code is new.
- **The ramp** is `exit: Some(STRF_RAMP_EXIT)`, the ship's way of setting someone down at a chosen spot.

The armour-piercing round's burst and armour penetration are in `burst_of` and `pen_of` in `crates/server/src/mechanics/projectiles.rs`. It flies as the other gun rounds do.

## The client

- **`client/src/kinds/strf.ts`** is the kind. It has four seats, the gunner's with the `ammo` and `heat` gauges, a diesel a little higher and busier than the tank's, and the engine bay at the front right, where a hurt engine smokes.
- **`client/src/render/strf.ts`** is `StrfModel`, a `TankModel` on its own `.glb` and layout. `TankLayout` gained two optional fields:
  - `paints`: Swedish green and black camo in place of the tank's WW2 schemes;
  - `recoilStroke`: 0.18 m, for the 40 mm's short kick.
  - The driver's and gunner's heads show in their hatches. The troops are never drawn.
- **The gun presenter** draws its rounds with the barrel at `STRF_BARREL_LENGTH` and the autocannon's lighter report. The tank music plays for its crew.
- **Both rounds** are drawn as the small autocannon slug with its trail.

## The model

`tools/vehicles/build_strf.py` builds `client/public/models/strf.glb`, about 1,900 triangles. It has the node names of the tank's model.

- **The hull** is long and flat, with a shallow glacis up to the driver's hatch at the front left and the engine grille at the front right.
- **The upper hull** overhangs the tracks, and there is a tall rear with the ramp and its door.
- **The running gear** is seven road wheels a side, with a drive sprocket at the front and an idler at the back.
- **The turret** is faceted, with bevelled cheeks, smoke dischargers, the gunner's sight on the right and the commander's on the left. The 40 mm barrel is long, with a perforated jacket and a muzzle brake.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strf.py -- --preview <dir>
```

## Protocol

The layout of the messages is unchanged. `KIND_STRF` (28) and `PROJECTILE_CANNON_AP` (12) are new values in existing fields, so `PROTOCOL_VERSION` stays where it was. A client from before this change would draw the Strf as a jeep and its armour-piercing round as a rocket.
