# Fuel

A vehicle with an engine carries a tank of fuel. A player at the wheel burns it: a full tank lasts about
ten minutes with the throttle wide open, and half as long on the boost. A holed fuel tank leaks it out,
driven or parked. A fuel station fills it again in ten seconds. A vehicle with none left does not answer
its throttle, and one left standing empty is taken back to where it was placed once nobody is near.

## Which kinds burn fuel

`Spec::fuel` (`crates/shared/src/vehicles/spec.rs`) is how many seconds a full tank lasts flat out, or
`Spec::NO_FUEL`. Each kind's file in `crates/shared/src/vehicles/` gives its own.

| Kinds | A full tank lasts |
|---|---|
| The cars, vans, jeep, technical, EPA traktor, golf cart, police car, ambulance, walker, Stridsbåt 90, raft (`Spec::FUEL`) | 600 s |
| Opel Manta, Roadster, helicopter, air ambulance, Titan | 480 s |
| Snowmobile | 420 s |
| Tank, Strv S, Strf 90, Pbv 302, AMV, triplane, seaplane | 540 s |
| Submarine | 720 s |
| Bus | 840 s |

These burn none, and never run dry: the pirate ship (she sails), the snow racer (it slides), the towed
guns and the ship's gun (no engine), the hen and the tupp (animals), and the drones and the missile (a
battery, DRONES.md).

## Burning, leaking and filling

The server keeps each vehicle's fuel as a share of full (`Vehicle::fuel`,
`crates/server/src/game/state.rs`) and steps it every tick in `fuel::step`
(`crates/server/src/mechanics/fuel.rs`), called by the `Vehicles` mechanic as it turns the driver's input
into controls. Each tick it adds up three things.

- **Burn.** Only with a player in the driver's seat. The effort is the throttle or, for a helicopter, the
  collective, whichever is more, ahead or astern alike, and never less than `FUEL_IDLE_BURN` (0.05):
  an engine ticking over with someone at the wheel uses a twentieth of what it uses flat out. With the
  boost open (`THRUST_BOOST`) it burns `FUEL_BOOST_BURN` (2) times as fast. The effort divided by the
  kind's seconds is the share of a tank a second.
- **Leak.** Whoever drives, and with nobody aboard. The fuel tank is the armour's `Part::Fuel`
  (ARMOR.md). Hurt (half its strength or less), a full tank runs out in `FUEL_LEAK_HURT_SECONDS`
  (120 s). Gone, in `FUEL_LEAK_GONE_SECONDS` (20 s), and the vehicle burns as before.
- **Fill.** Within `FUEL_STATION_REACH` (30 m, on the ground plane) of a fuel station, an empty tank
  fills in `FUEL_FILL_SECONDS` (10 s), moving or standing, crewed or empty. That is faster than any
  leak, so a holed tank still fills at a station, and runs out again on the road.

### Who is spared

Fuel is a player's to run out of. `mechanics::human` tells a player from a bot, a bus driver, an officer,
a medic, a civilian and a zombie.

- A bot, a bus driver or one of the services at the wheel burns nothing, and drives on with an empty tank.
- The traffic's drivers are not in `Seats` at all. A car of the traffic drives on its rails or on its
  `Autopilot`, and neither is stopped by an empty tank.
- A car of the traffic that a player has taken burns fuel like any other from then on.

## Dry

A vehicle with no fuel and a player at its wheel, or nobody, gets `CarCondition::dry()`
(`crates/sim/src/vehicle.rs`): its engine gives no power, exactly as with its engine shot out. Every way
of moving already reads that power, so:

- a car, a tank or a boat rolls to a stop;
- a helicopter or a plane loses its engine in the air and comes down;
- a walker or a Titan cannot stand, and falls.

Fuel is replicated as the `fuel` component (`level`, 0..255 of full, protocol 100,
[NETWORKING.md](NETWORKING.md)), on every vehicle of a kind that burns any. The server rounds it up
(`fuel::byte`), so it reads 0 only when the tank is truly empty. The driver's client gives its predicted
vehicle the same dry condition from that byte (`setCarState` and `setNeighbour` in
`crates/sim-wasm/src/lib.rs`), so the prediction stops where the server does.

## Fuel stations

A station is a point of the world: `Manifest::fuel_stations` (`[x, z]`, centimetres, `WORLD_VERSION` 42).
The importer (`crates/osm-import/src/fuel.rs`) takes every `amenity=fuel` of OpenStreetMap, a node as it
is and a way (the station's outline, or its building or canopy) at its middle. The pumps, the canopy and
the shop of one station are often mapped each on its own, so points nearer than `FUEL_STATION_APART`
(60 m) to one already kept are left out. A station off the world's tiles is left out too. The importer
prints how many it found:

```
fuel: 7 mapped in OpenStreetMap, 4 stations in the world
```

A world with none has no refuelling. There is no placing one by hand yet. The synthetic world
(`fixtures/world`) has one, on the verge of its second crossroads.

Fuel ways were not read before, so the local OpenStreetMap index is of a new version
(`pbf::VERSION` 10): make it again with `osm-import --fetch-osm sweden` before importing a world.

On the client a station is drawn as a pump beside a tall red sign (`client/src/render/fuelstations.ts`),
one instanced mesh for them all, like the bus stops. Nothing collides with it. The minimap and the
phone's map mark each with a red pump (`MapMarks.stations`, `client/src/hud/map.ts`).

## What the player sees

- Everyone in a vehicle that burns fuel has a `FUEL` bar in the corner column
  (`client/src/hud/gauges.ts`). It reads `FUEL LOW` and turns red under 15 %, `OUT OF FUEL` at none, and
  `REFUELLING` within reach of a station while the tank is short of full. The client tells that by the
  station's reach, as the server fills it (`client/src/fuel.ts`), not by the level rising: the level it
  draws can step back to an older snapshot's when its picture falls further behind, and a holed tank under
  fire would then read as filling.
- A dry engine is silent and its exhaust stops, for everyone who sees it
  (`client/src/entities/presenters/vehicles.ts`).

## Stranded vehicles

A vehicle that stands with an empty tank and nobody aboard counts the ticks (`Vehicle::dry_ticks`).
After `FUEL_STRANDED_SECONDS` (5 minutes) it is taken home: `vehicles::reclaim_stranded`
(`crates/server/src/mechanics/vehicles.rs`) puts it back where it was placed, whole and with a full
tank, as a burnt-out wreck is (WRECKS.md).

It goes only while no player is within `HEAL_CLEAR` (400 m) of it, the rule the craters heal by
(CRATERS.md): `Game::heal` checks every `HEAL_CHECK_TICKS` with the same eyes, which are the connected
players and what they sit in. Someone who walks off to find another car and comes back within the five
minutes finds theirs where they left it.

Getting into a dry vehicle stops the count, and it starts again from nothing when the last one gets out.

A car of the traffic or a bus that a player drove off is not taken home. The traffic and the buses clear
away their own (`Abandoned`, `Strayed`).

## Tests

- `crates/server/src/mechanics/fuel/tests.rs`:
  - `a_jeep_at_full_throttle_runs_dry_in_about_ten_minutes_and_a_sports_car_sooner`
  - `boosting_burns_the_tank_faster`
  - `a_helicopters_collective_burns_its_fuel`
  - `a_player_driving_uses_fuel_and_more_of_it_on_the_boost`
  - `a_dry_car_does_not_answer_the_throttle_and_is_seen_to_be_empty`
  - `a_bot_at_the_wheel_burns_nothing_and_drives_on_empty`
  - `a_holed_tank_leaks_while_parked_and_a_destroyed_one_faster`
  - `a_station_fills_an_empty_tank_in_ten_seconds_and_only_near_it`
  - `a_stranded_car_stays_while_someone_is_near_and_goes_home_full_once_they_are_gone`
  - `a_dry_car_someone_sits_in_is_not_counted_as_stranded`
  - `every_engine_burns_fuel_but_the_pirate_ship_the_snow_racer_and_what_has_none`
- `crates/server/src/mechanics/buses/tests.rs`: `a_bus_driver_burns_no_fuel_and_drives_on_with_an_empty_tank`
- `crates/osm-import/src/fuel.rs`: fuel nodes and ways become stations, near ones are one, and the
  synthetic city has one.
- `client/src/__tests__/fuel.spec.ts`: the gauge says it is filling only at a station.
- `client/src/__tests__/vehicle.spec.ts`: `does not drive off on an empty tank, and does once there is fuel in it`
