# Wrecks

A vehicle whose health is gone does not go up at once. It burns for `DOOM_TICKS` (3 s) first, and its crew
has that long to get out. Then it goes up, and whoever is still aboard dies with it. How it goes up depends
on its kind: a car is thrown into the air, a tank burns where it stands, and a Titan falls over.

## The grace

When `damage_vehicle_at` (`crates/server/src/mechanics/combat.rs`) takes a vehicle's health to 0, it
calls `doom`. That puts a `Doomed` component on the vehicle, and the `Vehicles` mechanic counts it down
(`doomed` in `crates/server/src/mechanics/vehicles.rs`).

While a vehicle is doomed, its health is 0, so everything that asks `entities.dead` already treats it as a
wreck:

- Its controls are cut. It rolls on with nobody at the wheel, and no autopilot drives it.
- Nobody can board it, and it takes no more harm.
- Anyone can get out as usual, by the door or bailing out at speed. It takes about 1 s to climb out
  (`DOOR_OPEN_S` + `STEP_OUT_S`), which leaves about 2 s to run. Running covers about 15 m, well beyond the
  blast's 8 m. Even someone standing by the door lives through the blast, if their health is full.
- Bots aboard get out the way a player would.
- Traffic drivers are not in `Seats`. The traffic takes a wreck's driver away with the car, as before.

Some vehicles go up at once, with no grace:

- **Ammunition cooking off** (`armor::hit_part`, then `combat::cook_off`). Nobody gets out of that.
- **A drone** pops, as before.
- **A ship's guns** go up the moment their ship does.

## How each kind goes up

The `fall` field of each vehicle's `Body` (`crates/shared/src/vehicles.rs`) says how it dies:

| `Fall` | Kinds | When its health is gone | When it goes up |
|---|---|---|---|
| `Thrown` | cars, helicopters, planes, guns | it burns | it is kicked up 9 m/s and sent tumbling at `WRECK_SPIN`, and its own blast throws it as well |
| `Settles` | tanks, PBV, Strf, Strv, bus, AMV | it burns | it is jolted up `WRECK_JOLT` (2 m/s) and stays where it is. It does not feel its own blast |
| `Topples` | Titan, walker, hen, tupp | its legs let go, and it is pushed over along the blow | where it lies. It does not feel its own blast |

Ships, submarines, boats and seaplanes fill and sink, as before, after the same grace.

### Falling over

A kind on legs stands because its legs hold it up and a balance torque keeps it upright (`walk` in
`crates/sim/src/vehicle/flight.rs`). Both let go once its health is gone (`standing` is
`condition.power > 0.5`).

On top of that, `doom` turns it at `WRECK_TOPPLE_SPIN` about the level axis square to the blow, so its top
moves the way the blow went. Gravity does the rest:

- Struck down from the front, it falls on its back.
- Struck down from behind, it falls on its face.

`away` is the way the blow went: from the attacker to the hull, or else from the point struck. With
neither, the vehicle falls backwards.

A Titan takes about five seconds to come down. It sinks onto its bent legs, then tips over. A wreck on
legs therefore goes up once it lies, with its up axis at most `WRECK_LIES_UP`, or
`WRECK_TOPPLE_TICKS` after the grace at the latest.

### Its own blast

`Physics::blast_cars` takes a vehicle to spare, which neither feels the push nor is hurt. A heavy wreck's
blast spares the wreck itself, and so does a Titan's landing (`blast_sparing`). Before this change, a
Titan's landing pushed the Titan anyway, because the push was applied inside the physics before the
spared vehicle was left out.

## On the screen

The client needs nothing new to show a doomed vehicle: a vehicle at 0 health already burns, with soot,
charred paint and its lamps out. The explosion is the `WEAPON_WRECK` shot, which now comes at the end of
the grace.

Seated in a vehicle at 0 health, you see **GET OUT** over a pulsing orange screen, with a countdown
taken from `DOOM_TICKS` (`doomedIn` in `client/src/game.ts`, drawn in `client/src/hud/panels.ts`).

## Not changed

A *living* Titan or walker whose engine is gone, or badly hurt, also falls. That is because `standing`
reads the engine's power, not whether the vehicle is a wreck. It may be meant ("no engine, no legs"), so
it is left as it is.

A vehicle left out of fuel comes back the same way, whole and at the spot it was placed, but only once
nobody is near ([FUEL.md](FUEL.md), "Stranded vehicles").

## Tests

- `combat/tests.rs`:
  - `the_ak5_chips_a_vehicle_and_a_wreck_kills_the_crew`: the crew is alive through the grace and dies
    when the vehicle goes up.
  - `a_wreck_tumbles_as_it_is_thrown_up`
- `vehicles/tests.rs`:
  - `a_doomed_car_cannot_be_driven_and_its_driver_who_gets_out_and_runs_lives`
  - `a_wrecked_tank_burns_where_it_stands_and_is_not_thrown`
- `titans/tests.rs`:
  - `a_titan_struck_down_from_the_front_falls_on_its_back`
  - `a_titan_struck_down_from_behind_falls_on_its_face`
- `bots/crew/tests.rs`:
  - `a_crew_gets_out_of_their_doomed_tank_and_back_in_when_it_comes_back`
  - `a_crew_gets_back_in_when_their_wrecked_tank_comes_back`: ammunition cooks off, with no grace.
