# Building styles

Every building used to be a house or a church. A barn drawn as a house has rows of windows up its
walls and a front door a metre wide; a factory has the same, forty metres long. So a building now
has one of eleven kinds (`Building::kind`, `BUILDING_*` in `crates/shared/src/world.rs`), and its
doors, windows, inside and colours follow from it:

| Kind | Doors | Windows | Inside | Walls and roof |
|---|---|---|---|---|
| `BUILDING_PLAIN` 0, a house | one, 1.0 × 2.1 m, in its front | rows a storey apart on every wall | floors and stairs | the house palette |
| `BUILDING_CHURCH` 1 | see [CHURCHES.md](CHURCHES.md) | | | |
| `BUILDING_BARN` 2 | a sliding gate 3.2 × 3.0 m in its front | 0.7 m square, about 7 m apart, in the two long walls only | one hall | falu red boards, now and then tarred dark or bare grey; sheet metal |
| `BUILDING_OUTBUILDING` 3, a garage or a shed | an overhead garage door 2.4 × 2.1 m | one, 0.7 m square | one room, where it is big enough to have one | red, dark, ochre or white boards; sheet metal |
| `BUILDING_HALL` 4, a factory or a warehouse | an overhead door 4.0 × 4.0 m in its front and in the next longest wall | a ribbon 2.0 × 1.0 m, 3 m up, round every wall | one hall | profiled sheet in grey, blue, green, cream or red; sheet metal or felt |
| `BUILDING_SHOP` 5 | a double door 1.6 × 2.3 m in the front | panes 2.2 × 2.0 m from 0.5 m up along the front; none elsewhere | one hall | light render, boards or brick |
| `BUILDING_FLATS` 6, flats, a dormitory, a hotel | as a house | as a house | floors and stairs | render or brick |
| `BUILDING_PUBLIC` 7, a school, a library, a police station | a double door 1.6 × 2.3 m | 1.8 × 1.6 m, 3 m apart, on every wall and storey | floors and stairs | yellow or red brick, white render |
| `BUILDING_CABIN` 8 | as a house | as a house | as a house | dark timber, sheet metal |
| `BUILDING_FARMHOUSE` 9, an old wooden house | as a house | tall and narrow, 0.9 × 1.5 m, 2.3 m apart, on every wall and storey | floors and stairs | falu red, ochre or white boards |
| `BUILDING_VILLA` 10, a post-war villa | as a house | wide and low, 1.7 × 1.2 m, 3.6 m apart | floors and stairs | brick, pale boards or render |
| `BUILDING_FIRE_STATION` 11 | up to three overhead doors 3.6 × 3.6 m side by side in its front, 0.8 m of wall between them | as a public building | one hall, driven into | red or yellow brick, render |

A door that does not fit its wall is tried smaller (a barn's at 2.4 × 2.4 m), and last as a house's
door. A window keeps clear of the door in its wall. A fire station gets as many doors as its front
has room for, at most three, in a row about its middle.

## The front

A building's front is the wall its door is in, and a shop's panes. `Building::front` says which way
the road it stands by lies from its middle (`front_of` in `assemble.rs`: the nearest point of a
road, a street or a track that is driven on, not a path, within 60 m), and `openings_of` takes for
the front the wall that faces that way most squarely, of the walls at least six tenths as long as
the longest and turned no more than 60° from the road. So a shop's glass and a house's door face
the street, and a barn's gate the farm's road. With no road near, or none of the longer walls
towards it, the front is the longest wall, as it was. A church keeps its door in its end. The field
came with `WORLD_VERSION` 26; of Vindeln's 1 257 buildings 1 219 stand by a road.

## What a building is

OpenStreetMap says what few buildings are. Of the 1 426 in the Vindeln world 87% are `building=yes`
and nothing more, and of the 9 219 in the countryside round it 90%. `crates/osm-import/src/classify.rs`
therefore decides in this order, and the first rule that names a kind wins:

1. **Its tags** (`by_tags`). `building=barn`, `stable`, `cowshed`, `sty`, and a `farm_auxiliary` of
   80 m² or more, are barns; `garage`, `garages`, `shed`, `carport`, `service` and a smaller
   `farm_auxiliary` outbuildings; `industrial`, `warehouse`, `hangar`, `manufacture` and
   `man_made=works` halls; `retail`, `supermarket`, `commercial`, `kiosk`, anything with a `shop`
   and an `amenity` where people eat, drink, bank or fill up (restaurant, cafe, fast_food, pub, bar,
   bank, pharmacy, post_office, fuel, car_wash) shops; `apartments`, `dormitory`, `hotel`, and a `yes` with three `building:levels` or more,
   blocks of flats; `school`, `kindergarten`, `university`, `college`, `hospital`, `civic`, `public`,
   `government`, `train_station`, and an `amenity` of school, kindergarten, library,
   police, town hall, hospital, clinic, community or social centre or courthouse,
   public buildings; `building` or `amenity` `fire_station` fire stations; `cabin`, `hut` and a `tourism` hut or chalet cabins. Any other value than `yes`
   is a house: what a mapper called a house is not second-guessed.
2. **A word in its name**: lada, ladugård, loge or stall make a barn; bygdegård, byastuga, EFS,
   missionshus, bönhus, folkets hus, hembygdsgård, skola and förskola a public building;
   brandstation, räddningstjänst and brandförsvar a fire station.
3. **A node inside it.** Most shops, cafés and police stations are mapped as a point, not on the
   building. A `building=yes` with such a node inside its outline (a `shop`, or one of the
   `amenity` values above, `by_use`) is what the node says: a fire station before a public
   building before a shop, where it holds several. A building mapped as a house stays one, whatever
   is run from it. The nodes are read with everything else (`osm::is_poi` in `wants_node`, and the
   Overpass query), which took `pbf::VERSION` to 6: an older index is skipped until `--fetch-osm`
   is run again.
4. **The land use it stands in** (`Land`, the smallest area round its middle). On
   `landuse=industrial` or `railway` a building of 150 m² or more is a hall and one under 45 m² an
   outbuilding; on `retail` or `commercial` one of 200 m² or more is a shop; on `allotments` all are
   outbuildings.
5. **Its yard.**
   - Under 45 m² with a bigger building within 50 m: an outbuilding. Alone, it stays a house (a cabin).
   - A farm's barn: a plain footprint (at most 8 corners, filling 85% of the box round it) that is
     long (16 m or more, 130 m² or more, 1.6 times as long as wide, at most 14 m wide) or big (20 m
     and 180 m²), not on residential or other mapped land, with at most 15 buildings within 150 m,
     and either a smaller house (60 to 260 m²) within 50 m or fields (`landuse=farmland`, `meadow`,
     `farmyard`) within 120 m.
   - A hay barn: 15 to 130 m², nothing within 50 m, fields within 30 m.
   - A cabin: under 70 m², on no mapped land, at most 3 buildings within 150 m.

6. **A house's style** (`BUILDING_PLAIN` so far, 50 to 260 m², tagged as a dwelling or not at all).
   On a farm or out of any village (no mapped land, at most 15 buildings within 150 m) it is an old
   wooden house. In a village its quarter decides, a square of 200 m, since a street is built at
   once: a third of the quarters are old, a third post-war villas, and in the rest a squat house
   (9 m or more across, 110 m² or more) is a villa and the others stay as they were.

The importer prints what it found: for Vindeln, `284 houses, 427 old houses, 366 villas, 2 churches,
36 barns, 130 outbuildings, 33 halls, 17 shops, 7 blocks of flats, 11 public, 0 fire stations, 22
cabins; 87 known by their tags, 13 by a node in them, 14 by the land they stand on, 144 by their
yard`. The nodes took Vindeln from 6 shops to 17, and the Umeå world (`worlds/home`) names 38
buildings by them.

### How good the yard rules are

They were set against the tagged buildings of northern Sweden (62° to 66° N: 336 692 buildings, 734
tagged as barns, 50 161 as houses), read from the local extract:

| Rule | Tagged barns it finds | Tagged houses it takes for a barn |
|---|---|---|
| A farm's barn | 157 of 734 (21%) | 412 of 50 161 (0.8%) |
| A hay barn | 66 of 734 (9%) | 60 of 50 831 (0.1%, cabins counted too) |

The cabin rule finds 87 of 836 tagged cabins and huts (10%) and takes 274 of 49 964 tagged houses
(0.5%) for one; a small house alone in the woods drawn in dark timber does no harm.

The rules are cautious on purpose: a barn they miss is drawn as a house, as all were before, and a
house drawn as a barn loses its windows. Tagged barns are a mixed lot (their median is 163 m², and a
quarter are under 60 m²), and many stand in villages where footprint alone cannot tell them from a
house. The height of the walls from the laser would be the next thing to try.

## Doors, windows and the inside

`shared::windows::openings_of` has a `Style` for a barn, an outbuilding, a hall, a shop, a public
building, a fire station, an old house and a villa (a block of flats and a cabin are opened as houses): the doors to try, how
many of the walls get one (the front first, then the longest), how many stand side by side, and the
one row of windows with the walls that carry it.
`client/src/render/windows.ts` mirrors it (`STYLES`), and `windows-parity.spec.ts` checks the two
place the same openings to the millimetre, since the server cuts them out of the walls it collides
with.

`shared::interior::rooms` gives a church, a barn, an outbuilding, a hall, a shop and a fire station the ground floor
only, open to the roof; the others have a house's floors and stairs. A
building with a floor has its door cut open (`facade::cut_windows`), so a barn's, a hall's and a fire station's gates
are real openings when they stand open: a car drives in. Their floor is a 10 cm sill (`interior::SILL`) where a house's
is a 30 cm plinth, so wheels roll over it. An outbuilding under 40 m² has no floor, as before, and
its door stays shut.

## Gates

A door 2.2 m wide or more (`windows::GATE_WIDTH`) on a building with a floor is a gate: a barn's, a
hall's, a garage's. A gate is shut until someone opens it, and a soldier on foot within 2.5 m of
its sill opens or shuts it with E (`BUTTON_USE`, `mechanics/gates.rs`). A press that boarded a
vehicle or took an item is spent before it gets to the gate. A gate does not shut on a soldier
standing in it, or with a vehicle within 3.5 m of its opening.

From a seat, E with the vehicle within 8 m of a shut gate opens it and the rider stays seated; with
no shut gate there E gets out as ever, so a second press does. A gate is not shut from a seat. The
mechanic runs twice a tick for this: before `Vehicles` for those seated (`Gates::seated`) and after
it for those on foot, and a press that got someone in or out of a vehicle is spent there.

- **Shut is wall.** `BuildingLook::shut` lists the building's shut gates, and `facade::cut_windows`
  leaves their doorways uncut, so a shut gate is part of the building's collider: it stops rounds
  and vehicles, and a blast takes it out with the cells it stands in. Opening or shutting one
  rebuilds that one building's collider (`World::set_gate`, `Physics::replace_building`).
- **On the wire** it is `Gates` (tag 19, protocol 84): the gates that changed, to everyone, and the
  open ones to a joiner. See [NETWORKING.md](NETWORKING.md).
- **In the client's prediction** the same collider is rebuilt by the same code
  (`Physics::set_gates` through the WASM `setGates`), when the message comes; the player's own
  press is not predicted, so the gate answers a round trip later.
- **Drawn** by `client/src/render/gates.ts`, a mesh a gate, moving for 1.2 s. A hall's and a
  garage's is an overhead door of light ribbed sections that rolls up under the lintel; a barn's is
  a dark plank door on a rail that slides aside along the wall. The tile's own mesh has the gate's
  frame only. A gate whose wall is blasted from round it is no longer drawn.
- Which gates are open lives in the room: after a server restart all are shut.

## Chimneys and porches

`client/src/render/details.ts` adds what the look leaves out, drawn into the tile's mesh with the
building: a brick chimney on the ridge of every house, old house, villa and cabin with a pitched
roof, a third of the way along it; a small roof on two posts over the front door of an old house
and a cabin; a flat canopy over a villa's. They are only drawn (nothing collides with them), and a
house hurt enough to be drawn in pieces loses them.

## Colours

`buildingColors` in `client/src/render/buildingmesh.ts` picks from the kind's own palette
(`KIND_COLORS`). A hall's profiled sheet is drawn with the upright-board pattern, whose ribs read as
sheet metal at that size. Where the importer painted the building (France), its paint is kept.

An outbuilding is painted as the wooden house nearest it within 40 m in its tile (`houseOf`), and
roofed as it where both roofs are pitched, so a yard's garage and sheds match its house. Beside a
brick or rendered house it keeps its own boards.

## Not done

- **A node names only the building it lies in.** A shop mapped as a point out in the street, or
  in a building mapped as a house, changes nothing.
- **The front is found from the building's middle**, so a long or bent building may face the road
  nearest its middle and not the one its real entrance is on. A garage faces the road too, not the
  drive up to it.
- **Bots do not open gates**, and a gate is not shut from a seat.
- **Carports** (`building=carport`) are closed garages, and `building=roof` is left out as before.
- **A fire station is one hall**, with no upper floor over the engines; neither world imported so far has one, so it has been seen in tests only. A block of flats has no balconies or door a stairwell.
- **A house's quarters are a hash**, not the real age of the street, and there are no corner boards or other trim.
- **A station house** is a public building like any other.
- **The kinds needed no new field** (`kind` was already a byte); `Building::front` did, with
  `WORLD_VERSION` 26. `worlds/vindeln`, `worlds/home` and `fixtures/world` are imported again.
