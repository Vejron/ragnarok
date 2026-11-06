# Vehicles IX: the technical

The technical is a long-bonnet pickup in the style of a Land Cruiser 70, with a heavy machine gun on a post in its open bed. It rides tall on big tyres and long, soft springs, so it crosses rough ground that would shake the jeep. In return it leans and pitches, and the gun overheats when it is held on.

## In play

- **The technical** (`KIND_TECHNICAL` = 23) has 130 hp and three seats. It is never placed on its own: it appears only where it is placed in the map editor (**Technical**, Q in the palette).
- **Seat 0 is the driver**, on the left of the cab. It tops out at 30 m/s, a little slower than the jeep.
- **Seat 1 is the passenger**, on the right. Like a jeep's passengers, they fire the guns in their own hotbar.
- **Seat 2 is the gunner**, standing in the bed behind the gun (drawn standing, not sitting: `stand` on its `SeatView`). The gun follows their look all the way round, from 0.3 rad down to 0.9 rad up, which is steep enough for aircraft. It is a heavy, slow 14.5 mm gun: 4 rounds a second, 40 to a soldier (four put one down), 10 to a hull and 20 to a plane, out to 300 m. That is the tank's roof gun's damage a second in fewer, bigger blows, and through more plate. Its post puts it above the cab's roof, so it fires forward over the cab. The gunner's hands are on the gun, so they cannot fire their own (`crew_gun` leaves out a `mount_seat`).
- **It overheats.** Each round adds `TECHNICAL_MG_HEAT` (31) of heat, out of 255, and the barrel sheds `GUN_COOL` (1) each tick. Held on, the gun jams after about four seconds and clears after about three more of cooling. Short bursts never jam it. The gunner's HUD shows the heat bar, which reads JAMMED when it jams, and their seat hint says "overheating" above 60%. A round through the receiver (`Part::Breech`) stops the gun firing.
- Its crew are in the open, and the cab has no doors, so all three can be shot.

## The springs

Until now every car rode on the same wheel tuning, hard-coded in `crates/sim/src/vehicle.rs`. It now comes from `Spec::springs`. `Springs::FIRM` holds the old values and every other kind uses it. The technical's are about half as stiff, with weaker damping and a 0.5 m travel against 0.3 m:

| | stiffness | compression | damping (relax) | travel |
|---|---|---|---|---|
| `Springs::FIRM` | 25 | 2.2 | 3.0 | 0.3 |
| technical | 12 | 0.9 | 1.6 | 0.5 |

Rapier multiplies the spring and damper forces by the chassis' mass, so the sag at rest is g/(4·stiffness): about 0.1 m on firm springs and 0.2 m on the technical's. The body bounces at about 1.1 Hz instead of 1.6. The compression damping matters as much as the stiffness: at landing it does most of the stopping, so softer springs with the old damping landed no deeper than the jeep. With a 0.6 m rest length and 0.58 m tyres it settles 1.08 m up (`TECHNICAL_RIDE_HEIGHT`), with about 0.68 m of clearance under the box against the jeep's 0.44 m.

So much lean put it on its roof at full lock flat out. Its centre of mass is 0.5 m below the box's middle (`com_drop`): at 0.35 m it rolls over, at 0.42 m it just stays on its wheels, and at 0.5 m it barely leans. The tests are `a_technical_is_softer_sprung_than_the_jeep` and `a_technical_gets_going_and_stays_on_its_wheels_at_full_lock` in `crates/sim/src/vehicle/tests.rs`.

## The model

`build_technical` in `tools/vehicles/build_car.py` builds `client/public/models/technical.glb`, about 2,000 triangles:

```
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only technical
```

Like the other cars it has `wheel_*` on `pivot_*` and `seat_N` markers. The gun's mount, `turret`, has its origin on `TECHNICAL_MG_PIVOT`, and the gun, `gun`, is its child at the same point. `TechnicalModel` (`render/technical.ts`) extends `CarModel`. It turns the mount and elevates the gun as the server aims them, and its `barrel()` serves the gun camera and the tracers. The `Turrets` presenter draws each new round on the counter as a bullet, with a flash and the rattle.

## Protocol

To give a turret's gunner a heat bar, the `turret` component now carries `heat` (a byte, 0..255) between `range` and `flags`, and a `TURRET_JAMMED` flag (`1 << 4`). Every turret sends it; it stays 0 on guns that never heat. `PROTOCOL_VERSION` is 70. A seat whose `gauges` include `'heat'` shows it, through `mountGun` in `client/src/gauges.ts`.
