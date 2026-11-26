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

A station is a lot of the world laid out the same way every time (`shared::station`), in its own frame:
X runs along the road it faces, Z back from that road. `Manifest::fuel_stations` lists them as
`FuelStation { at, yaw, base }`: the middle of the lot (centimetres), how far its frame is turned
(milliradians, its +Z going to `(sin, cos)`), and the height of its paving (`WORLD_VERSION` 43).

```
                     road
 ───────────────────────────────────────────────  -Z
  (sign)
            ┌──────── canopy ────────┐
            │  ▣ pump   ═══   ▣ pump  │           lot: 36 × 24 m
            └────────────────────────┘
      ◀─  ─  ─ │═══ wash bay ════│ ─  ─  ─▶      wash lane along X, either way,
                                                 the lot clear past both ends    +Z
```

Every size and place is a constant in `consts.rs` (`STATION_*`, `WASH_*`): the lot (`STATION_LOT`),
the canopy over the pump island (`STATION_CANOPY_*`), the round sign on its post at the front corner
(`STATION_SIGN_*`), and the wash bay behind the canopy on the same middle, whose doors are at
`WASH_BAY_X` along the lane at `WASH_BAY_Z`, `WASH_BAY_WIDTH` wide and `WASH_BAY_HEIGHT` clear. There is
no shop: a car coming out of the wash either way has 12 m of open lot ahead of it.

### Where the importer lays them

The importer (`crates/osm-import/src/fuel.rs`) takes every `amenity=fuel` of OpenStreetMap, a node as it
is and a way (the station's outline, or its building or canopy) at its middle. The pumps, the canopy and
the shop of one station are often mapped each on its own, so points nearer than `FUEL_STATION_APART`
(60 m) to one already kept are left out (`merged`). Then, for each:

- **Facing** (`lots`): the lot runs along the nearest road a car drives on within `STATION_ROAD_REACH`
  (80 m), on the side of it the station is mapped on. A service road counts only when nothing else is
  that near, since it is usually the station's own way in. The lot is pushed back until its front edge
  is `STATION_ROAD_CLEAR` (3 m) off the road's edge. With no road near, it stands where it is mapped,
  facing north.
- **Clearing** (`clear`): every OpenStreetMap building that stands on any of the lot or the 2 m round it,
  by more than a square metre (`Lot::overlaps`), is left out of the world, wherever its middle is: the
  station's mapped shop, canopy and car wash, a hall reaching over its wash bay, and anything else that stood
  there.
  So are the trees, rocks and bushes on it (`lay`).
- **Levelling**: the lot is a building's pad for the terrain's shaping (`condition.rs`), and after the
  tiles are built the ground under it is levelled to just under its paving, at the middle height of the
  lot's ground, easing back to the ground as it was over 8 m round it (`level`). A station on a slope is
  cut into it on one side and built up on the other.
- **Paving** (`pave`): the lot is asphalt cover (`COVER_ASPHALT`) in the tiles it reaches, so the ground
  under the paving is painted as a car park's and no grass or forest floor grows through it.

A station off the world's tiles is left out. The importer prints what it did:

```
fuel: 19 mapped in OpenStreetMap, 19 stations, 13 buildings cleared off their lots
fuel: 19 stations in the world
```

A world with none has no refuelling and no wash. There is no placing one by hand yet. The synthetic world
(`fixtures/world`) has one, filling the middle of a block whose houses are left out for it.

### What stands, and what is drawn

The solid parts are `shared::station::structure`: the paving, the bay's walls and roof, the canopy's roof
and its two pillars, the pump island and its pumps, and the
sign's post. Both ends' physics stand them from the manifest, as the ski lifts are
(`station_colliders` in `crates/sim/src/lib.rs`, from `Physics::new` and `Physics::streamed`), so they stand
whatever tiles are loaded.

The client (`client/src/render/fuelstations.ts`) draws a 1960s roadside station of a chain of the game's
own, NORD: the solid parts as the physics has them (through the WASM's `stationParts`), white with navy
and orange fascias, rounded orange pumps with white faces and globes, a round orange sign in a navy ring
with a price board under it, "NORD" on the sign and on the canopy's fascia, and "WASH" over the doors.
Every station has the same layout, so each piece is one instanced mesh for all of them. By night
(`nightAt`) the sign's disc glows orange and the canopy's lamps warm white.

The minimap and the phone's map mark each station with a red pump (`MapMarks.stations`,
`client/src/hud/map.ts`) at its middle.

## The car wash

A car that comes into the wash's lane is driven through it, and comes out the far end mended and clean.

### The wash takes the wheel

The lane runs from `WASH_LEAD` (5 m) in front of one door to as far past the other, within
`WASH_TAKE_ASIDE` (2 m) of its middle (`Bay::lane`). A vehicle is the wash's while:

- someone drives it (its controls are `powered`: a parked car with nobody in it is left alone);
- it is on wheels (`Lift::Wheels`), and narrow enough to leave `WASH_ROOM` (0.3 m) either side in the bay;
- its middle is in the lane, within 3 m of the bay's floor in height;
- it heads within `WASH_TAKE_ANGLE` (about 50°) of the lane's line, either way.

Then `Physics::drive_car` (`crates/sim/src/vehicle.rs`) drives it instead of its driver
(`shared::station::pull`): pure pursuit on a point `WASH_STEER_AHEAD` (4 m) along the middle of the lane,
as traffic steers, the way the car faces, at `WASH_SPEED` (2.5 m/s, a walking pace), braking if it came in
fast. Whatever the driver holds does nothing until it is out of the far lead-out. Both the server's physics
and the driver's prediction (the WASM) run `drive_car`, so the client predicts the pull as the server does
and nothing new crosses the wire for it.

### Coming out healed

The `Carwash` mechanic (`crates/server/src/mechanics/carwash.rs`, after `Vehicles`) watches each vehicle
between a bay's doors (`Bay::inside`) and the end it came in at. One that leaves by the other door, alive,
is made whole:

- `Vehicle::renew`: no dents, sound wheels (even one that had come off), its doors back on and shut, its
  armour's parts and turret whole, a full tank;
- `vehicles::restore`: full health, no fire, no longer doomed, its guns restocked;
- `Physics::mend_car`: a turret shot off is back on;
- everyone aboard is wanted no more (`emergency::clear_wanted`), and the police that chased them stand down
  on their own;
- every client is told, with `S_WASHED` (`Washed { vehicles }`, [NETWORKING.md](NETWORKING.md)), and takes
  the dents, holes, scorch and blood off it (`HullMarks::clear`). Its driver is told
  "Washed and mended: the police have lost you".

One that leaves by the door it came in at is not healed.

### The doors and the brushes

Each bay has a ribbed roll-up door at each end and two pairs of tall blue brushes on gantries. These are
only drawn: nothing collides with them. Both doors roll up while any vehicle (`VehiclePresenter.placed`)
is in the lane or a little to either side of it, and roll down `2 s` after it is empty. The brushes spin,
each pair opposite ways, while a vehicle is between the doors. The client works this out for itself, from
where the vehicles are drawn.

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
- `crates/osm-import/src/fuel.rs`: fuel nodes and ways become stations, near ones are one, a station runs
  along the nearest road kept back from it, every building reaching onto its lot is cleared, a lot on a slope is
  levelled and paved, and the synthetic city has one.
- `crates/server/src/mechanics/carwash/tests.rs`:
  - `a_battered_wanted_car_driven_through_the_wash_comes_out_whole_and_its_driver_clean`
  - `a_car_driven_past_the_wash_is_not_healed`
- `crates/sim/src/vehicle/tests.rs`:
  - `the_wash_takes_a_car_floored_at_its_door_through_at_a_walking_pace_down_the_middle`
  - `a_car_across_the_wash_lane_or_with_nobody_driving_is_not_the_washs`
  - `a_stations_bay_walls_are_solid`
- `crates/shared/src/station.rs`: the station's frame, the lane and the bay, the wash's pull, and the
  parts that stand.
- `client/src/__tests__/fuel.spec.ts`: the gauge says it is filling only at a station.
- `client/src/__tests__/vehicle.spec.ts`: `does not drive off on an empty tank, and does once there is fuel in it`
