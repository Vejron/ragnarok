# Vehicles XXV: the police car and the ambulance

Two vehicles for the emergency services, and the two kinds of people who ride in them. The police car is a Swedish police Volvo 240 of the 1980s, and the ambulance a VW Transporter. Each carries a light bar on its roof and a siren, which the server switches with one new bit in the vehicle's flags. This write-up covers the vehicles, their models, the light bar, the siren and that bit. Who drives them, where to and why is the server's dispatch, which is written up with it.

## In play

| Id | Kind | In the game | Seats | Top speed | Off the line | What it is |
|---|---|---|---|---|---|---|
| 50 | `KIND_POLICE_CAR` | Police car | 5 | 41.5 m/s | 5 m/s² | The Volvo 240 estate in white with a blue stripe, POLIS on its doors and bonnet, and a light bar |
| 51 | `KIND_AMBULANCE` | Ambulance | 7 | 34 m/s | 4 m/s² | The Transporter van in yellow under a white top, with a red stripe, AMBULANS on its sides, and a light bar |
| 52 | `KIND_OFFICER` | Police officer | | | | A person the server runs, drawn as a soldier with nothing in hand |
| 53 | `KIND_MEDIC` | Ambulance medic | | | | A person the server runs, drawn as a soldier with nothing in hand |

- **The police car is the Volvo 240** (`vehicles/police_car.rs`, copied from `volvo_240.rs` with its own `POLICE_CAR_*` constants): the same box, weight, springs, engine force and seats. Only its top speed differs, 41.5 m/s where the Volvo does 36, about 15 % more. It pulls away exactly as the Volvo does and keeps pulling for longer. That is faster than the Volvos, the Kadett, the Sierra, the Escort, the van and the Bronco, and well over what the traffic cruises at. The Saabs, the Manta, the Golf and the Omega are still faster flat out (42 to 48 m/s), so a player in one of those can outrun it on a long straight.
- **The ambulance is the Transporter** (`vehicles/ambulance.rs`, from `transporter.rs`): the same body and its seven seats, the driver and a passenger over the front wheels, two by the sliding door and three on the back bench. It does 34 m/s where the van does 30.
- **Not the air ambulance.** `KIND_AIR_AMBULANCE` (37, [VEHICLES-XXI.md](VEHICLES-XXI.md)) is the helicopter a player flies. `KIND_AMBULANCE` is the van.
- **Anyone can drive either,** as any car: seat 0 drives, the rest ride behind glass and fire their own guns. Neither is in the traffic's `KINDS`, so none drives by unless something puts it there.
- **Placement.** In the map editor they are "Police car" (★) and "Ambulance" (♥); every letter and digit was taken, and the air ambulance has the cross.
- **Paint** is fixed (`paint` in `render/car.ts`), as the AMV's and the EPA's is: every police car is white, and every ambulance yellow under its white top.
- **On the minimap** a police car in view is a blue square and an ambulance a red one (`emergencyBlips` in `blips.ts`), drawn from the vehicles the client already has, so the wire carries nothing for it. A wreck is not marked.
- **Sound.** The police car has the Volvo's petrol four with a higher redline, and the ambulance the van's flat four.

## The light bar and the siren

A vehicle's `Vehicle::siren` on the server says its siren sounds and its beacons flash. It starts off, and nothing in this change switches it on but a test: the dispatch will. While it is on, the vehicle's replicated `flags` carry `FLAG_SIREN`.

On the client the `Beacons` presenter (`entities/presenters/beacons.ts`) shows it, for a police car or an ambulance that is not a wreck:

- **The lenses take turns,** a quarter of a second each (`BEACON_SIDE_MS`), so the pair goes round twice a second. The time is the frame's `now`, the clock the other presenters animate by. A vehicle starts on the side its id gives it, so two parked side by side do not flash as one.
- **The lens whose turn it is is drawn alight:** bright blue and unlit (`CarModel.flash`), where the other keeps the model's dark blue.
- **A blue point light is held over the lit lens** (`lights.hold`), reaching 18 m, at full strength in the dark and 30 % of it by day.
- **The siren** is a `SirenVoice` in `vehicleaudio.ts`: a European two-tone hi-lo, 435 and 580 Hz, each held 0.55 s by the audio clock. It is a square wave with its edge taken off, through the same panner as an engine, so it is heard from where the vehicle is, fades with distance out to 250 m, and rises and falls in pitch as the vehicle passes. Its level at the vehicle is 0.3, where an engine's is about 0.4. The nearest three sirens sound at once.

When the flag clears, both lenses go dark and the siren stops. When the vehicle leaves the client's view, its siren is dropped with it.

## The models

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only police_car --preview <dir>
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only ambulance --preview <dir>
```

Both are `build_saloon` ([VEHICLES-XXIII.md](VEHICLES-XXIII.md)) with another car's measurements and their own `details`. The Volvo's and the van's measurements moved into `volvo_240_shape()` and `transporter_shape()` so that both cars read the same numbers; `volvo_240.glb` and `transporter.glb` rebuild byte for byte as they were.

- **The light bar** (`light_bar`) stands on the roof over the front seats: a black foot, the siren's horn in the middle, and a lens either side of it.
- **The lenses are nodes of their own,** `beacon_L` (the driver's side) and `beacon_R`. Each node's origin is the middle of its lens. They sit beside `body` in the model and not under it, because `CarWear` owns the material of every mesh under `body` and would put a flashing lens back to its dark one on a damaged car. `CarModel` finds the two by name (`hasBeacons`, `flash`); a car without them has no bar.
- **The livery's colours are the model's own.** `livery(name, rgb)` adds a material after the shared ones for the model being built only (`reset_scene` starts each model without it), so `MATERIAL_COLORS` and every other model's materials are as they were. The police car has `blue` for its stripe and lettering and both have `beacon` for the lenses; the ambulance's stripe and lettering are the shared `red`.
- **The lettering** (`lettering`) is block capitals, five dots across and seven down, as flat panels: each run of dots along a row is one panel, joined with the same run in the rows under it, so POLIS is 38 triangles. It is laid out to read from outside the car on both sides, and on the police car's bonnet from in front of it. There is no crest and no badge.
- **The police car** drops the Volvo's rubbing strip, where its lettering is, and its roof rails. It keeps the tall lamps up the tailgate.
- **The ambulance** keeps the van's sliding-door track and second grille, and has no engine vents, where its stripe runs.
- They come to 1,426 (police car) and 1,452 (ambulance) triangles.

## The people

`KIND_OFFICER` and `KIND_MEDIC` are for the people the dispatch will run. The client draws them as it draws a driver of the traffic (`KIND_CIVILIAN`, [TRAFFIC.md](TRAFFIC.md)):

- **On foot,** `Soldiers` draws one as a soldier with whatever its flags say it holds, from `kind`, `place`, `heading` and `flags`, and remembers it as nobody's kill: it is left out of the kill feed and the scoreboard, as a civilian is.
- **In a seat,** `Riders` draws anyone who has `seated` and no `place`, whatever their kind and whichever seat, with `heading`, `look` and `flags` saying where they look and what they hold.
- **Their clothes are a soldier's,** in the colour their id picks, as a civilian's are. The civilian's path has no colour of its own to hook a uniform onto.
- **Dragging out.** A car with a `KIND_CIVILIAN` in seat 0 offers "E: drag the driver out" (`Seating.manned`). An officer or a medic at the wheel does not make it so: the prompt is the ordinary one for the first free seat.

## Protocol

No message and no component changed. `Flags::flags` has one more meaning: on a vehicle, bit 1 is `FLAG_SIREN`. On a person the same bit is `FLAG_DEAD`, as bit 0 is `FLAG_GROUNDED` on a person and `FLAG_LAMP` on a vehicle. A vehicle is wrecked by its `health`, and no client reads `FLAG_DEAD` off one. A vehicle now has `flags` while its lamps or its siren are on. The kinds a client must know grew by four. [NETWORKING.md](NETWORKING.md) has both. The protocol is 95, for these and for the wanted level that came with them ([EMERGENCY.md](EMERGENCY.md)), which sets the siren: on while a unit is on its way somewhere.

## Tests

- `crates/sim/src/vehicle/tests.rs`:
  - `ride_heights_differ_between_cars` covers both.
  - `a_police_car_is_the_volvo_off_the_line_outruns_it_flat_out_and_stays_on_its_wheels`.
  - `an_ambulance_is_a_little_faster_flat_out_than_the_transporter_and_stays_on_its_wheels`.
  - Neither is in `PERIOD_CARS`: `the_period_cars_differ_off_the_line_and_flat_out` holds the van to being the slowest of them away, and the ambulance pulls away exactly as it does.
- `a_vehicle_with_its_siren_on_says_so_in_its_flags_and_one_without_sends_none` in `game/replicate/tests.rs`.
- `beacons.spec.ts`: the lenses are on the roof, one each side, and only on these two; the bar flashes and the siren is reported only while the flag is set; the sides alternate by the frame's clock, which the spec sets; a wreck is dark; the lamps' bit alone lights nothing.
- `vehicleaudio.spec.ts`: the nearest three sirens sound, each stops when it is no longer reported, and the tone alternates by the audio clock.
- `car.spec.ts` checks both models' wheels, seat markers, headlights and triangle budget.
- `presenters.spec.ts`: an officer and a medic on foot are drawn and remembered as civilians are.
