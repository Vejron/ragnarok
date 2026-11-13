# Ski lifts

Every ski lift OpenStreetMap maps in a world (`aerialway=chair_lift`, `drag_lift`, `t-bar`, `platter`,
`rope_tow`, `gondola`, `cable_car`, `mixed_lift`, `magic_carpet`) is built as a chair lift: steel
pylons, an open station with its bull wheel at each end, a cable loop and chairs going round it. A
soldier at a station presses E beside a chair to sit on it, and rides round and round the loop until
they jump off. Docksta's lift up Skuleberget, Skulebergets Linbana, is the first.

## Import

`crates/osm-import/src/osm_lifts.rs` asks Overpass for the lifts with a query of its own, so its
cache key is new and nothing else is fetched again. It keeps what OSM calls the lift (`LIFT_*`) and
how many a chair carries (`aerialway:occupancy`, at most `LIFT_MAX_SEATS`, else `LIFT_SEATS`).

`assemble::lay_lifts` keeps the lifts wholly inside the world's tiles and turns each one so it runs
uphill. It adds pylons on spans longer than `LIFT_SPAN`. The cable is `LIFT_STATION_CABLE` over the
ground at a station and `LIFT_PYLON_CABLE` at a pylon. Pylons are then raised, never past
`LIFT_PYLON_MAX`, until a chair's seat clears the ground under both cables by `LIFT_CLEARANCE`
between them. Near a station the seat may come down to a rider's knees. The forest is cleared
`LIFT_LANE` either side of the line. Any building within `LIFT_STATION_CLEAR` of either end is
dropped, because it is the real station building and the lift's own open station stands there
instead.

A lift goes in the **manifest** (`Manifest::lifts`), not the tiles. Both the server and the client
need the whole of it wherever the player is. It is a trailing `#[serde(default)]` field, so older
worlds load without it.

## Chairs on a clock

A chair is not an entity. `crates/shared/src/lifts.rs` lays out the loop (`Path`):

1. Up the right of the lift's line, `LIFT_GAUGE / 2` out.
2. Round the top bull wheel.
3. Down the left.
4. Round the bottom wheel.

The cable runs straight from pylon to pylon. Chair `i` is
`tick / TICK_RATE × LIFT_SPEED + i × spacing` metres round the loop, modulo its length. There are
about `LIFT_CHAIR_SPACING` metres between chairs, as many as fit a whole number of times. The seat
hangs `LIFT_CHAIR_DROP` under the cable.

`client/src/sim/lifts.ts` mirrors this, and `lifts.spec.ts` holds it to the Rust through the WASM's
`liftChair`. The loop's length that the clock runs on is worked out in f64 on both sides and rounded
to the millimetre (`clock_length`), so after months of ticks a chair is still where the other side
has it.

## Riding

`mechanics/lifts.rs` (`Lifts`) runs before `Pickups` and `Vehicles`, so its use of E comes first.

- **Boarding.** E sits a soldier on the nearest free place of a chair whose seat is within
  `LIFT_BOARD_RADIUS` of them and no higher than `LIFT_BOARD_HEIGHT` over the ground (that is, at a
  station). The rider is `Player::lift`.
- **Each tick.** The rider's capsule centre is put `LIFT_RIDER_CENTRE` over their place on the chair,
  with no velocity. `Movement` leaves them alone. They count as crouched, they look where they like,
  and they can shoot.
- **Riding on.** Nobody is put off. The chair carries its riders round the bull wheels and back,
  forever.
- **Getting off.** A rider's E does nothing, and is swallowed so no car or item beside the line is
  taken from the chair. A jump lets go with the cable's velocity, as do a blast that knocks them down
  and death. At a station they step onto its deck; between the pylons they fall.

A rider is replicated as a standing soldier, crouched, with `lifted` naming the chair (protocol 76).
The client draws every chair at the server's present tick (`Game.liftTick`), not the render tick
behind it. Riders, local and others (`LiftRiders`), sit in the `sit` clip on the chair drawn at that
tick. The local rider's prediction only acknowledges while they ride, as it does in a seat. Once
they are off, the next snapshot reconciles them from where the server let them go.

## Solid

`shared::lifts::structure` lists a lift's pylons and stations as boxes and upright posts (`Part`).
Each pylon is a steel tube, a crossarm, two hangers and two sheave trains. Each station is a concrete
deck, the bull wheel, its column, and a roof on four posts set clear of the widest chair. Nothing
fells them, as with rocks.

- **Server.** `sim::Physics::new` stands a collider for each part.
- **Client.** The streamed physics stands them from the manifest at the start, not tile by tile.
  Soldiers and vehicles bump into them, they stop rounds, and a rider let go at a station lands on its
  deck. The chairs and the cable are not solid.

## Drawing

`client/src/render/lifts.ts` (`LiftView`) draws the same parts, one mesh a lift, so what is seen is
what is hit. The parts come through the WASM's `liftParts`, so the view is built once the sim is up.
The cable is `lines.ts`'s wire ribbons, with no sag, so it matches the chairs. The chairs, which have
no safety bar, are one instanced mesh for all lifts, placed every frame within `CHAIR_REACH` of the
eye. The hint reads "E: ride the lift" beside a boardable chair and "Space: jump off" on one.

## Not done

- No queue and no lift operator.
- Chairs do not swing.
- A drag lift or gondola is not drawn as what it is.
- Bots never ride.
