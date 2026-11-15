# Buses: public transport

A world can have public transport. With `"buses": true` in its `area.json` the importer finds up to three bus lines along the world's major roads, and the server runs buses up and down them, halting at every stop. Anyone can ride. The bus itself is an ordinary vehicle, described in [VEHICLES-XV.md](VEHICLES-XV.md).

## In play

- **A line** runs along major road (motorway down to secondary) from one end to the other. Buses drive it out and back, keeping to the right of the road's middle, at up to 13 m/s.
- **A stop** has a blue sign on a pole on each side of the road. A bus halts at every stop for 8 seconds. The lines and their stops are drawn in red on the minimap.
- **Boarding** is E at the front door, as for any vehicle. The driver has seat 0, so you get the first free passenger seat. E again gets you off, wherever the bus is.
- **At the end of a line** everyone is set down. The bus waits out its halt and is then turned to face back along the line on the other side of the road. A twelve-metre bus cannot turn round on a ten-metre road, so this is a snap, not a manoeuvre.
- **The driver can be killed.** The driver is a civilian with empty hands and no team, who does not come back. A bus with no driver is anyone's: its driver's seat is free and it drives like any car. Sixty seconds later a fresh bus with a new driver starts from an end of the line. The old bus is taken away once it has stood empty for a minute, and a wrecked one after 8 seconds.
- **How many.** Each line gets enough buses for a wait of about three minutes at a stop, never fewer than two, and a world never runs more than twelve (`BUS_MAX`).

A bus does not brake for people on the road: like every vehicle it runs them over. It does slow and stop for a vehicle or a wall ahead.

## Turning it on

| Where | What |
|---|---|
| `worlds/<name>/area.json` | `"buses": true` |
| Map editor | **Public transport (buses)** in the world panel. Generate the world again to apply. |
| `osm-import` | Reads the option, finds the lines once the tiles are built (`buslines.rs`) and puts them in the manifest. It prints each line's length and stops. |
| `Manifest::bus_lines` | The lines, whole, like the ski lifts: `BusLine { points, stops }`, centimetres. Empty when the option is off. |
| Server | `WorldFile::bus_lines`, copied from the manifest. The `Buses` mechanic does nothing when it is empty. |
| Client | `WorldData.busLines`. `render/busstops.ts` draws the signs. |

After changing what OpenStreetMap elements the importer keeps (bus stops are new), the local extract's index must be rebuilt once: `cargo run --release -p osm-import -- --fetch-osm sweden`.

## Finding the lines (`crates/osm-import/src/buslines.rs`)

The lines are found on the finished world, from the `ROAD_MAJOR` pieces in its tiles, so they work the same for a world built from OpenStreetMap or from an orienteering map.

1. **The network.** Road points within a metre of each other are one node, which joins the halves of a road cut at a tile's edge. Points nearer than 30 m to the world's edge are left out, so a line ends inside the world.
2. **The longest line.** In each separate piece of the network, the node furthest along the roads from any start is one end, and the node furthest from that is the other. The shortest way between them is the line.
3. **Divided roads.** Up one carriageway and back down the other is the same road twice. A line is cut wherever it comes back within 40 m of where it was more than 300 m earlier, and the longest piece is kept. Road within 40 m of a line counts as that line's road, so the next line does not run down the other carriageway.
4. **Hooks.** A line that starts in a roundabout or a junction may begin the wrong way for a few metres. A turn of more than a radian within 60 m of an end is cut off.
5. **More lines.** The same again over road no line has yet, up to three lines (`BUS_LINES`). A line under a kilometre is dropped.
6. **Stops.** An OpenStreetMap bus stop (`highway=bus_stop`) within 25 m of a line is a stop on it. Stops closer together than 150 m are one stop. Both ends are stops, and any stretch longer than 600 m without one gets stops about 400 m apart.

On `worlds/home` (Umeå, 13.8 km²) this gives three lines of 4.1 km, 2.3 km and 1.2 km with 11, 7 and 4 stops, from 46 mapped bus stops. The major roads there are the motorways round the town, so the lines run round it rather than through it: the streets through the town are minor roads.

A stop that lands on a bridge has its signs on the ground under the bridge.

## Running them (`crates/server/src/mechanics/buses.rs`)

`Buses` ticks after `Bots` and `Zombies` and before `Vehicles`, so the input it writes drives the bus the same tick.

- **The driver** is a `Player` with no connection and a `BusDriver` component, as a zombie is a player with made-up input. It is not a `Bot`: it does not fight and takes no bot's place. A player joins and leaves through the room, so `Buses` asks for drivers in `World::bus_calls` and `Game::staff_buses` answers each tick.
- **Steering** is at a point 8 to 16 m ahead on the line (further the faster it goes), 1.75 m right of the road's middle (`BUS_LANE`), turning as the arc through that point asks.
- **Speed** is 13 m/s, less for a bend within 42 m (the turn of the line over each 6 m sets the bend's speed, never under 3 m/s, held until the tail is round it), for the next stop, and for what a ray from the nose finds ahead. It stops 4 m behind a vehicle; after 15 seconds held up it pushes on at a creep.
- **Stuck.** A bus that gets no nearer its stop for 6 seconds backs out for 2. After three of those, or when it is more than 25 m from its line, it is put back on the line where it was, but only when nobody but the driver is aboard.

The tests are in `buses/tests.rs`. `buses_on_a_real_world` (ignored) runs a real world's buses with nobody watching and prints how far each drove and what a tick costs:

```sh
WORLD=$PWD/worlds/home/world.msgpack MINUTES=20 cargo test --release -p server -- --ignored --nocapture buses_on_a_real_world
```

On `worlds/home`, twelve buses each drove about 10.5 km in 20 minutes (8.7 m/s with the halts), none was put back, and the room's tick went from 0.05 ms with no buses to 0.35 ms: about 0.025 ms a bus, of a 16.7 ms tick.

## What it took in the world

Driving real roads for the first time without a player's eyes found places no vehicle could pass. The importer now mends them for every vehicle (`bridge.rs`, `assemble.rs`):

- **Cuttings.** Where a path goes under a road that OpenStreetMap does not map as a bridge, the terrain has a trench across the road. A drivable road over a dip narrower than 24 m and deeper than 1.5 m gets a deck over it, flush with the road and with no parapets (`cuttings`, `Pylons::Flush`). The way under it stays open. `worlds/home` has 40.
- **Short bridges.** A bridge under 12 m is still left to the terrain, unless the ground under it dips more than a metre.
- **Bridges in several ways.** Bridge ways that meet end to end are built as one bridge (`joined_bridges`). Built one by one, each came down to the ground where it ended.
- **Divided roads.** Two bridges side by side share one deck wide enough for both (`twin_decks`), where two decks a road wide each lay over each other at two heights. Where decks still overlap, the parapet between them is left out.
- **Banks.** OpenStreetMap often ends a bridge part of the way down the bank. A bridge whose road goes on rising past its end is lengthened along that road to the top of the rise, up to 40 m (`onto_the_banks`).
- **Ramps** now come all the way down to the lip every deck has over the ground, where they stopped up to 0.4 m short.
- **Piers** are left out where they would stand on a road or a track under the bridge.
