# Building styles

Every building used to be a house or a church. A barn drawn as a house has rows of windows up its
walls and a front door a metre wide; a factory has the same, forty metres long. So a building now
has one of a dozen kinds (`Building::kind`, `BUILDING_*` in `crates/shared/src/world.rs`), and its
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
| `BUILDING_SHELTER` 13, a lean-to (vindskydd) | none: one side is open | none | a plank bed, drawn only | tarred timber, falu red or grey boards; sheet or felt; or as its tags say |

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

1. **Its tags** (`by_tags`). A lean-to, `amenity=shelter` with `shelter_type=lean_to`,
   `weather_shelter` or none, is a shelter whatever its `building` says (a bus stop's, a picnic
   shelter, a gazebo and a `basic_hut` are not). `building=barn`, `stable`, `cowshed`, `sty`, and a `farm_auxiliary` of
   80 m² or more, are barns; `garage`, `garages`, `shed`, `carport`, `service` and a smaller
   `farm_auxiliary` outbuildings; `industrial`, `warehouse`, `hangar`, `manufacture` and
   `man_made=works` halls; `retail`, `supermarket`, `commercial`, `kiosk`, anything with a `shop`
   and an `amenity` where people eat, drink, bank or fill up (restaurant, cafe, fast_food, pub, bar,
   bank, pharmacy, post_office, fuel, car_wash) shops; `apartments`, `dormitory`, `hotel`, a `tourism` hotel, hostel, guest_house or motel, and a `yes` with three `building:levels` or more,
   blocks of flats (a hotel tagged on a `retail` or `commercial` building wins over the shop: it is the hotel over its ground floor); `school`, `kindergarten`, `university`, `college`, `hospital`, `civic`, `public`,
   `government`, `train_station`, and an `amenity` of school, kindergarten, library,
   police, town hall, hospital, clinic, community or social centre or courthouse,
   public buildings; `building` or `amenity` `fire_station` fire stations; `cabin`, `hut` and a `tourism` hut or chalet cabins. Any other value than `yes`
   is a house: what a mapper called a house is not second-guessed.
2. **A word in its name**: lada, ladugård, loge or stall make a barn; bygdegård, byastuga, EFS,
   missionshus, bönhus, folkets hus, hembygdsgård, skola and förskola a public building;
   brandstation, räddningstjänst and brandförsvar a fire station.
3. **A node inside it.** Most shops, cafés and police stations are mapped as a point, not on the
   building. A `building=yes` with such a node inside its outline (a `shop`, a `tourism` hotel, hostel,
   guest house or motel, or one of the `amenity` values above, `by_use`) is what the node says: a fire
   station before a public building before a hotel before a shop, where it holds several. A hotel's
   node also wins over a building tagged as a shop (Hotell Forsen in Vindeln is `building=retail`):
   a block of flats has windows on every wall and storey, where a shop has one row along its front. A building mapped as a house stays one, whatever
   is run from it. The nodes are read with everything else (`osm::is_poi` in `wants_node`, and the
   Overpass query), which took `pbf::VERSION` to 7 when hotels were added: an older index is skipped until `--fetch-osm`
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

## Shelters

A lean-to (`crates/osm-import/src/shelter.rs`) has three walls and no fourth. In Umeå they are mapped
as `amenity=shelter`, `shelter_type=lean_to` on a `building=yes` about 2.5 m across, with a
`direction` (`west`, `sse`) the open side faces. The laser has too few returns over one to fit a
roof, so it is built and not fitted:

- **The open side** is the side of at least 1 m that faces `direction` most squarely. `direction` is
  a compass point or degrees from north. Without one it faces the road (`front_of`), and with no road
  near, its longest side is open. `Building::front` is the open side's outward normal, so the client
  knows which way it faces, with no new field.
- **The roof** is one plane, 2.2 m over the floor along the open side (`FRONT`), falling to the back
  at 0.4 a metre (about 22°, `PITCH`). It is pitched less where the back would come lower than 1 m
  (`BACK`). Its kind is `ROOF_SHED`.
- **The walls** are built by `roof::build_with` with a `Walls` that leaves the open side out and lets
  a wall be lower than a house's 2.5 m. The cells are cut from the three walls and the roof as from
  any house's, so a shelter is hit, chipped and blasted as a house is, and hides a soldier inside it
  only from behind and from the sides.
- **No openings.** `openings_of` gives a shelter none, as it gives a bridge none, on the server and in
  `windows.ts`. A shelter is too small for an inside.
- **Colours** come from `building:colour` and `roof:colour` (named, or `#rrggbb`; red is falu red)
  as its `paint`. Without them it takes its kind's palette.
- **Drawn open** (`render/facade.ts`, `eaves.ts`):
  - the boards show their thickness where they end along the open side (`Facade.rims`);
  - the inside is lit nearly as the outside is (`SHELTER_SHADE` 0.6, where a closed house's is 0.18);
  - the roof's high edge over the open side overhangs, with a fascia, as an eave does over a wall;
  - the floor is drawn under the three walls, closed along the open side (`floorOf`).
- **Details** (`render/details.ts`): a plank bed 0.4 m high from the back wall over two thirds of the
  floor, cut from the footprint's own outline and kept 0.2 m in from the walls' outside, and a ring
  of stones with two charred logs 1.7 m out in front of the open side. Both are only drawn, and go
  when the back wall's lowest cell does. A hurt shelter is drawn without the rims.

## Chimneys and porches

`client/src/render/details.ts` adds what the look leaves out, drawn into the tile's mesh with the
building: a brick chimney on the ridge of every house, old house, villa and cabin with a pitched
roof, a third of the way along it; a small roof on two posts over the front door of an old house
and a cabin; a flat canopy over a villa's. They are only drawn (nothing collides with them). A hurt
house, drawn by `lookFracture`, keeps each while the cell it stands on (its `anchor`: the roof just
under the chimney, the wall over the door) stands and has something to hang from.

## Colours, and a style of its own for each building

Every building has a **seed** (`buildingSeed` in `client/src/render/buildingmesh.ts`): a whole number
from 1 to 65 535 hashed from its tile and its index in it, the same on every client and every time
the tile is drawn. Everything below is drawn from it, so no two houses of a street need be alike, and
none of it costs a vertex, a draw or a texture.

**What the map says comes first.** The importer reads `building:colour` and `roof:colour` (`#rrggbb`,
or a name: red is falu red, yellow ochre, white the off-white of linseed paint) into
`Building::paint`, wall and roof each on its own, and `building:material` (or `building:cladding`)
and `roof:material` into `Building::clad` as a `CLAD_*` and a `ROOFING_*`
(`crates/osm-import/src/finish.rs`). Few buildings carry them: 202 of Umeå's 7 691, 4 of Vindeln's
1 335, none on Holmön. In France the roof's colour still comes from the aerial photo where the map
says nothing. A material decides the pattern whatever the colour: a dark red house of `wood` is
boarded, not brick.

**What the map leaves out, the client picks** (`buildingColors`, `buildingSurfaces`):

- **Wall and roof colour**, each by its own draw from the seed, from the palette of what the map says
  the building is made of (`CLAD_COLORS`, `ROOFING_COLORS`), else of its kind (`KIND_COLORS`). The
  two used to be one index stepping through both lists, so neighbours came in the palettes' order.
- **How it is built.** A house the client painted may be built more than one way in a colour
  (`HOUSE_SURFACES`): a red one is now and then lap-boarded, a white one in upright boards or
  rendered. A colour of the map's own that no palette has is boards on a kind built of wood, and
  brick or render by its colour on the rest. A hall's profiled sheet is drawn with the upright-board
  pattern, whose ribs read as sheet metal at that size.
- **Trim** (`Trim`): the window frames, the boards along the roof and the porch are white on most
  houses and now and then cream, grey, dark green, oxblood or blue-grey; a frame colour too near the
  wall's falls back to white (`frameOn`). The door is tarred brown on most, or green, blue, red,
  grey, ochre or white. Only houses and their outbuildings vary; a barn keeps its tarred door.
- **A plinth**: cast concrete from the foot of the walls (the lowest corner of them in the look) up
  30 to 50 cm, about where the floor inside lies (`interior::PLINTH`); 10 to 22 cm on what is driven
  into. A shelter and a bridge have none.

**The shader does the rest** (`wallsLook` in `engine/src/materials/walls.wgsl.ts`). Each patterned
vertex carries the seed, the foot of the walls and the plinth's height in its custom y, z and w, which
were unused. From the seed the shader draws the building's own numbers: a tint a little lighter or
darker and warmer or cooler than the next house of the same colour; the width of its boards (16 to
22 cm, lap boards 14 to 19) and where they start, so boards do not line up from house to house; how
far the shades of its boards, bricks or tiles spread; its mortar and its bond; and its **age**. An
old house has paint gone thin and grey in patches, a darker damp foot over the plinth, moss on a
tiled, shingled or felt roof (thicker on the slopes turned north) and rust in streaks down a sheet
roof. The noise of each is worked out only near enough to show.

**Windows** (`glass.ts`, `dressingOf`): each pane's colour, opacity and lamp are its vertices'. About
three panes in ten of a home have a curtain or blind drawn (paler and hardly seen through), one in
ten where people work, none in a barn. After dark (`lightWindows`, as `Scene.night` passes 0.35) a
lamp is lit behind about 45% of a home's panes, 22% of a workplace's and 6% of a barn's, warm, and
dimmer behind a curtain; the lit pane is opaque, so the lamp shows and not the dark room. The
curtain and the lamp are on the sheet only, so a broken pane's teeth are clear and dark.

A hurt house (`lookFracture`) is handed the same style, so it keeps its seed, plinth, trim and
pattern from the first hit. Falling pieces are plain, as before.

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
- **Shelters without a `building` tag are not imported**, and many are mapped so. Taking them in
  needs them in `osm::wants_*` and the Overpass query, and a `pbf::VERSION` bump.
- **Picnic shelters and gazebos** (a roof on posts) are still drawn as small houses.
- **A shelter's bed and fire ring are only drawn**: nothing collides with them, and a soldier cannot
  sit on the bed. The fire ring stands on the shelter's floor height, so on a slope it may sink into
  the ground or float over it.
- **Carports** (`building=carport`) are closed garages, and `building=roof` is left out as before.
- **A fire station is one hall**, with no upper floor over the engines; neither world imported so far has one, so it has been seen in tests only. A block of flats has no balconies or door a stairwell.
- **A house's quarters and its age are a hash**, not the real age of the street. There are no corner boards and no frieze under the eaves: they need each wall vertex to carry how far it is from the wall's ends. No balconies, shutters or drainpipes either: those are new vertices, in the main pass and both shadow maps.
- **Every lamp comes on at once** as it gets dark, and stays on all night. A lit window throws no light on the ground outside.
- **A station house** is a public building like any other.
- **The kinds needed no new field** (`kind` was already a byte); `Building::front` did, with
  `WORLD_VERSION` 26, and the mapped colours and materials (`paint` each on its own, `clad`) with 45. `worlds/vindeln`, `worlds/home` and `fixtures/world` are imported again.
