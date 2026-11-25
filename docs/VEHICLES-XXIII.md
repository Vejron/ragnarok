# Vehicles XXIII: the period cars

The traffic was jeeps, sports cars and EPA traktors, and the jeep and the sports car looked like toys. Now the roads carry twelve civilian cars of 1980 to 1995, of the kinds that filled a Swedish street then: two Saabs, two Volvos, a Golf GTI and a VW Transporter, two Opels and two Fords. The jeep has become a Ford Bronco with its top off, and the sports car an Opel Manta. Each is built close to the real car's size, and they differ more than the old cars did off the line and flat out. The names are the real models', with no badges or logos.

## In play

| Id | Kind | In the game | Seats | Top speed | Off the line | What it is |
|---|---|---|---|---|---|---|
| 1 | `KIND_JEEP` | Ford Bronco | 4, open | 32 m/s | 7.7 m/s² | A tall four-by-four on big tyres, top off, with a roll bar. Rides over kerbs; rolls at full lock |
| 4 | `KIND_SPORTS` | Opel Manta | 4 | 46 m/s | 12 m/s² | A low rear-drive coupé, the quickest away |
| 40 | `KIND_SAAB_900` | Saab 900 Turbo | 4 | 48 m/s | 8 m/s² | A three-door with a wraparound windscreen. Its turbo wakes late, then it is the fastest flat out |
| 41 | `KIND_SAAB_9000` | Saab 9000 | 5 | 45 m/s | 8.5 m/s² | A big, quick turbo hatchback |
| 42 | `KIND_VOLVO_240` | Volvo 240 | 5 | 36 m/s | 5 m/s² | The brick: a heavy estate, slow, softly sprung |
| 43 | `KIND_VOLVO_740` | Volvo 740 | 5 | 40 m/s | 6.5 m/s² | A square saloon, mid-pack |
| 44 | `KIND_GTI` | Golf GTI | 4 | 44 m/s | 10.5 m/s² | A light hot hatch, the quickest of the everyday cars away |
| 45 | `KIND_TRANSPORTER` | VW Transporter | 7 | 30 m/s | 4 m/s² | A flat-fronted van, two-tone, engine at the back. Slowest bar the EPA |
| 46 | `KIND_KADETT` | Opel Kadett | 5 | 38 m/s | 7 m/s² | A wedge-shaped small hatchback |
| 47 | `KIND_OMEGA` | Opel Omega | 5 | 42 m/s | 7 m/s² | A smooth saloon with a six that cruises fast |
| 48 | `KIND_SIERRA` | Ford Sierra | 5 | 40 m/s | 6.5 m/s² | The rounded "jelly mould" family hatchback |
| 49 | `KIND_ESCORT` | Ford Escort | 5 | 35 m/s | 5.5 m/s² | A small, cheap, slow hatchback with a bustle back |

"Off the line" is the engine's pull over the mass, before the fade towards top speed. On the flat a car gets within about a tenth of its top speed in 15 s.

- **Every closed car seats its riders behind glass** (`Exposure::Glazed`), as the EPA does. They can be shot through it, and the passengers fire their own guns from their seats. The glass is drawn see-through (opacity 0.4), so you can see who is inside. The Bronco stays open.
- **Seat 0 drives,** on the left. The others are passengers, front then back. The van's driver and one passenger sit over the front wheels, with two more by the sliding door and three on the back bench.
- **Each car has 100 hp** and a car's sheet-steel skin, with its engine under the bonnet (the van's under the back floor) and its fuel tank under the floor.
- **The traffic** ([TRAFFIC.md](TRAFFIC.md)) lists each of the ten everyday cars twice and the Bronco, the Manta, the van and the EPA once each (`KINDS` in `traffic.rs`). So the Saabs, Volvos, Opels, Fords and the Golf make up most of what drives by.
- **Placement.** In the map editor each new car is under its name, numbered 0 to 9 in the palette since every letter is taken.
- **Paint.** Each kind is painted in colours it came in from the factory (`palette` in `render/car.ts`): Saab cherry red, Volvo navy and beige, Golf Tornado red, Opel signal yellow, Sierra metallic blue and so on. The colour is picked per car. The van's roof and posts are white over its colour.
- **Sound.** The Saabs carry a turbo four (`TURBO_FOUR`, the petrol four with a turbo's whistle) and the van a flat four (`BOXER_FOUR`). The Omega has the straight six, the Bronco a V8, and the rest the petrol four, each with its own gears and redline. The rear-drive cars spin their back wheels on a hard launch (`drivenFrom: 2`). For the front-drive ones `drivenFrom` can only say all four (0).

## Tuning

- **Size.** Each car's box is its real length and width, and about 0.6 m tall (0.72 for the 240 and 1 m for the van). The glasshouse above the box is model, not collider, as on the EPA. Mass is the box's volume at a density chosen for the real car's weight, from 900 kg (Escort) to 1.6 t (van).
- **Ride heights** are measured by `ride_heights_differ_between_cars` and written into `<KIND>_RIDE_HEIGHT`, `SPECS` in `build_car.py` and the slip layouts. They give 12–18 cm under the box.
- **Centre of mass.** At full lock flat out, every saloon rolled with the Bronco's 0.15 m drop. They now carry 0.3 m, and the Saab 900, the Golf and the van, which still rolled, 0.4 m (`every_period_car_nears_its_own_top_speed_and_stays_on_its_wheels_flat_out_at_full_lock`).
- **Springs.** The saloons ride `Springs::FIRM`. The 240 is softer (stiffness 20, travel 0.35 m), and the van softer still (18, 0.4 m), so they lean and pitch.
- **Skimming.** Only the Golf (light and quick) and the Manta skim over water.
- **The Bronco keeps the jeep's physics,** its engine force and top speed, so that everything tested against the jeep holds. Only its look and its name changed.
- **The Manta is new physics:** a narrower box (1.68 m), a shorter wheelbase (2.52 m), 30 cm wheels and 46 m/s. The Roadster used to borrow the sports car's chassis. It now has the old numbers as its own constants, so it drives as it did.

## The models

`tools/vehicles/build_car.py` builds every period car but the Bronco from one function, `build_saloon(name, c, heads, details)`:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only saab_900 --preview <dir>
```

Each car's `c` is read off the real car at its size: heights above the ground and lengths back from the front face, in metres.

- **The lower body** is lofted (`shell`) through cross-sections along the car, about twenty-five of them. They are placed round the corners, at every bend of `top` (the line along the bonnet, the waist and the boot or tail) and round each wheel. Each section is tucked in at the sill and straight up the side to a shoulder crease, then leans in to the top (`shoulder`, `tuck`), with an optional dark band of cladding along the bottom (`clad`, the 9000 and the Sierra). Seen from above, the corners are rounded (`nose_round`, `tail_round`): nearly square on the Volvos, round on the Sierra. The sides are cut round each wheel in a true arch, with a dark well inside.
- **The glasshouse** is lofted along `glass`, the line over the windscreen, the roof and the back glass, and leans in towards the top (`tumble`). Each cross-section is a side up to the top of the side glass, a band round the corner as broad as an A post (`a_post`, 7 cm), and a top across. The windscreen and back glass bend through their points, so the Saab 900's screen wraps and the Sierra's hatch curves.
  - **The posts are painted into the glasshouse, flush with the glass,** not laid on it. The corner band is painted all along, so it is the A posts up the windscreen's edges, the roof's edges and the rear posts down the back glass's.
  - **In the side glass,** a B post is a band `b_post` broad round each of `posts`. The rear post is everything behind a line `c_post` ahead of the back glass and parallel to it, so the Golf's is thick and slanted and the 240's a thin upright D post. The side faces are cut along those lines (`clip`).
  - **The windscreen stops where the glass line flattens into the roof,** so the curve over its top is painted, as on the real cars.
  - Trim runs along the foot of the side windows, chrome on the Volvos.
- **Bumpers** wrap round the corners and back along the sides (`wrap`), deep and black on the Saab 900 and the Volvos and painted on the Omega. Number plates sit front and back.
- **Faces nobody sees are left out:** the top of the lower body under the glasshouse, the glasshouse's floor, the bumpers' backs against the body. A `shell` face of material `None` is not made.
- **Flat things are panels, not boxes.** Lamps, grilles and their slats and frames, plates, indicators, door seams, handles, rubbing strips, fuel flaps and vents are each a single quad a few millimetres off the surface (`Part.decal`): two triangles where a box was twelve. They are added after the solids' normals are worked out, wound to face out, because the renderer culls back faces.
- **The nose** has the car's lamps (`rect`, with bars dividing each unit, or `round2`, the GTI's two round lamps a side) and its grille, with slats and, on the Volvos, a chrome frame. Indicators wrap round its corners or sit under the lamps (the van). The 9000 carries fog lamps in its bumper. **The tail** has the lamp clusters with their reversing and indicator segments, a black panel between them on the Golf and the Kadett, the plate and the exhaust.
- **Blacked-out B posts** on the cars that had them: the 9000, the Golf, the Kadett, the Omega and the Sierra (`b_mat`).
- **Along the sides** are door seams, handles, door mirrors, a side indicator behind the front wheel, a rubbing strip between the arches where the car had one, the fuel flap, and wipers on the windscreen. Inside are a dash, a seat under each front rider's `<KIND>_SEAT_HEADS` point and a bench across each row behind (`cabin`; `HEADS` in the script is kept in step with the Rust), and a steering wheel.
- **Wheels** (`styled_wheels`) are a tyre and one disc on its outer face (`wheel_face`), whose sectors are coloured in the car's period style, so the spokes cost nothing: black steel wheels under a slotted hubcap (`steel`: the 240, the van, the Kadett, the Sierra and the Escort), flat discs with round holes (`dial`: the 740 and the GTI), or alloys with three (the Saab 900's), five (the 9000's and the Manta's) or six spokes (the Omega's). The spokes and holes show the wheel turning.
- **`details`** adds what is a car's own:
  - the Saab 900: the clamshell bonnet's shut lines and a lip on the hatch;
  - the 240: roof rails and the tall lamps up the tailgate;
  - the Golf: a red line along its grille, a roof spoiler and black arch flares;
  - the van: the sliding door's track, the engine vents and a second grille under the windscreen;
  - the Sierra: the black panel across its grille-less nose;
  - the Manta: the chin spoiler and the boot lip.
- They come to 1,240–1,560 triangles each, the Saab 9000 and the Golf the most, under the 2,500 `car.spec.ts` allows.
- **The Bronco** has its own `build_jeep`: a floor and bonnet cut round the wheels, tub sides up to the waist, a windscreen frame, a roll bar, chrome bumpers, round lamps either side of a square grille, a spare on the tailgate, and off-road tyres. About 1,780 triangles.

## Protocol

No message changed, but the kinds a client must know grew, so `PROTOCOL_VERSION` is 94.

## Tests

- `crates/sim/src/vehicle/tests.rs`:
  - `ride_heights_differ_between_cars` covers every new kind.
  - `every_period_car_nears_its_own_top_speed_and_stays_on_its_wheels_flat_out_at_full_lock`.
  - `the_period_cars_differ_off_the_line_and_flat_out`: the Golf leaves the 240 standing and the Saab 900 Turbo outruns the Golf, the Manta is the quickest away and the van the slowest, and the fastest flat out does half again the slowest's speed.
  - `a_hard_corner_at_speed_rolls_the_bronco_but_a_firm_one_does_not` is now the Bronco's alone.
  - The Manta's checkpoints in `SCRIPTED_CHECKPOINTS` (and `vehicle.spec.ts`) are measured again.
- `the_traffic_is_a_mix_of_the_period_cars_each_riding_at_its_own_height` in `traffic/tests.rs`.
- `a_low_car_driven_fast_over_a_kerb_is_unhurt` has a deeper kerb, because the Manta rolls further before it stops.
- `car.spec.ts` checks every new model's wheels, seat markers, headlights and triangle budget (2,500 a car).
