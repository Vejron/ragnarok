# Vehicles XVI: the tracked vehicles at their real sizes

The three tracked kinds were 15 to 25 % smaller than the vehicles they stand for, and out of proportion to each other. They are now the size of the real ones, each has a model of the same detail, and each hull is driven on and shot at as its real side outline rather than a box.

## What each one is

| | Tank | Strv S | Strf 90 |
|---|---|---|---|
| Stands for | Strv 104 (Centurion) | Stridsvagn 103 | CV90 with the 40 mm gun |
| Hull, length × width over the tracks | 7.6 × 3.4 m (was 5.2 × 2.6) | 7.04 × 3.4 m (was 5.4 × 2.6) | 6.55 × 3.2 m (was 6.4 × 3.0) |
| Hull roof above the ground | 1.75 m | 1.9 m | 1.75 m |
| Top of the turret's solid | 2.75 m | none | 2.5 m |
| Muzzle past the nose | 2.3 m | 2.0 m | 0.2 m |
| Rides (hull centre above the ground) | `TANK_RIDE_HEIGHT` 1.13 m | `STRV_RIDE_HEIGHT` 1.2 m | `STRF_RIDE_HEIGHT` 1.1 m |
| Belly clear of the ground | 0.5 m | 0.5 m | 0.45 m |

Their weight, engines, top speeds and turn rates are as they were: the density of each went down as its box grew, so they handle as before and ram as before. The tank's ride height is a whole number of centimetres on purpose. A snapshot carries a position to the centimetre, and a tank settled half a centimetre off what its driver's client was told drifted 18 cm from the server's over a few seconds of hard driving.

## The hull's outline

`Spec::profile` is a hull's side outline: the corners of a convex shape as `[z, y]` in the hull frame, which runs the hull's whole width. `Spec::BOX` (empty) is a plain box, which every other kind has. `half_extents` is the outline's bounding box, and still gives the mass, the room a vehicle needs to be placed and where its crew get out.

- **The physics** (`hull` in `crates/sim/src/vehicle.rs`) makes the chassis from the outline. The plates leaning back under the nose and the tail are what a tracked hull rides up a step or a hump on; the fixed sweep the box used to have is gone. The track end wheels are as before.
- **Armour** (`Physics::armor_strike`) meets the outline's faces as they lie, with their real normals. It used to move every hull hit onto the face of the box, and a plate's slope was only a number.
- **The Strv S has no casemate solid** any more. Its outline is the whole wedge.

What they climb now, by `tracked_vehicles_climb_a_step_and_the_strv_and_strf_a_taller_one`: the tank a 1.0 m wall, the Strv S 1.1 m, the Strf 90 1.2 m. The tank is long enough to touch a crest with its belly between two metre-high humps ten metres apart, and slows to about 3 m/s there; the other two keep above 4.

## Armour on the real faces

[ARMOR.md](ARMOR.md) has the table. In short:

- **The Strv S** is a wedge. Its glacis runs from a sharp nose to the roof over half the hull's length, 13 degrees above level, and the plate under the nose leans far back. A level shell from ahead or from a front quarter skids off either one. A shell that comes down at 12 degrees is stopped by 107 mm of plate along its path, and one from 25 degrees up goes through. The sides (25 mm), the tail (20 mm), the roof and the engine deck are thin, and the gunner's sight in the glacis lets a bullet in.
- **The tank's** glacis leans back 57 degrees, as a Centurion's does. It is 32 mm, which is 59 mm along a level shot: a shell goes through it from ahead and from well off the nose, as it did before.
- **The Strf 90's** engine cover is 8 mm lying 14 degrees above level: 33 mm along a level shot, and almost nothing to a round from above. Its lower front plate is 30 mm.
- **A face is told from its normal** (`Zone::of`), so a glacis lying flatter than 45 degrees is struck as *top*. Its plate is a `Zone::Top` plate with a region over the glacis, listed before the plain roof. The plates under a nose and a tail are steep enough to be struck as front and rear, except the Strv S's, which is a `Zone::Belly` plate with a region.
- **A slit in a glacis** is struck on top but looks ahead: `Plate::looks(Zone::Front)` says which way the client draws it.
- `every_face_of_a_tracked_hulls_outline_meets_its_own_plate` in `crates/shared/src/vehicles.rs` walks each outline and checks which plate every face meets.

## The models

Each is built by its script in `tools/vehicles/` to the same outline as the Rust (`PROFILE` in both), under 3,000 triangles as the client counts them. `track_loops` in `common.py` draws a track tight round its wheels, and `tracked_preview` renders the five views.

- **`build_tank.py`**, a Strv 104: a long hull with a short steep glacis, track guards with stowage bins and exhausts, skirt plates over six road wheels in pairs, the sprocket at the back under the engine deck's louvres, and a tall slab-sided turret with side bins, a basket, a cupola, the roof machine gun and a long gun with a fume extractor half way along.
- **`build_strvs.py`**, a Stridsvagn 103: the wedge, the dozer blade folded under the nose, sponsons over the tracks with the flotation screen's lip and nine jerrycans a side, four big road wheels with the sprocket in front, the fixed machine guns' box on the left, the gunner's sight hood, the commander's cupola on the right and the gun with its fume extractor over the nose.
- **`build_strf.py`**, a CV90: a high nose, the engine's louvres and the driver's hatch in the sloping front, seven small road wheels with the sprocket in front, skirts and side stowage, a faceted turret with a thin 40 mm gun ending in a cone, and the troops' door between two boxes at the back.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_tank.py -- --preview <dir>
```

## The client

- `render/tank.ts`, `strvs.ts` and `strf.ts` have the new wheel radii and where each head shows.
- `sim/slip.ts` gives the Strv S its own track marks (`STRV_TRACKS`) at its own ride height; it used the tank's.
- The chase cameras stand further back (`TANK_DISTANCE` 16, `STRV_DISTANCE` 15, `STRF_DISTANCE` 15), and the Strv S's aiming camera is 11 m behind its trunnion and 3.6 m up.

## Protocol

Nothing on the wire changed. The gameplay tables did (`tables.generated.json`: half extents and slits).
