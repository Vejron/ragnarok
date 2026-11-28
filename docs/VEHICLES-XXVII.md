# Vehicles XXVII: the police van

A third vehicle for the emergency services: the van that brings the police's armed squad. It is a VW Transporter in police livery, the ambulance's body again ([VEHICLES-XXV.md](VEHICLES-XXV.md)) with the police car's colours. Who rides in it, when it is sent and what its officers carry is the dispatch's ([EMERGENCY.md](EMERGENCY.md)); this covers the vehicle.

## In play

| Id | Kind | In the game | Seats | Top speed | Off the line | What it is |
|---|---|---|---|---|---|---|
| 54 | `KIND_POLICE_VAN` | Police van | 7 | 34 m/s | 4 m/s² | The Transporter van in white with a blue stripe, POLIS on its sides, and a light bar |

- **It is the ambulance under its paint** (`vehicles/police_van.rs`, copied from `ambulance.rs` with its own `POLICE_VAN_*` constants): the same box, weight, springs, engine force, cabin and seven seats behind glass, and the same 34 m/s flat out, where the Transporter does 30. It is slower than the police car (41.5 m/s) and arrives after it.
- **Four officers ride in it**, in its first four seats: the driver and their mate in front, and two on the bench by the sliding door (`traffic::crew_size`). They are seen sitting there as it drives up. The two in front get out by their doors; the two behind are put on the ground beside it, since the sliding door is not a door the game can open (`doors` lists the front pair only).
- **It comes from four stars**: two vans at four stars and four at five, on top of the four police cars. The fleet has eight (`fleet::VANS`), and they do not patrol.
- **Anyone can drive it** once its crew has left it, as any car, and taking it is the crime taking a police car is. It is not in the traffic's `KINDS` nor at the dealer.
- **Placement.** In the map editor it is "Police van" (☆), beside the police car's ★.
- **Paint** is fixed: police white (`paint` in `render/car.ts`), the police car's.
- **Light bar and siren** are the police car's: it is in the `Beacons` presenter's kinds, and its `FLAG_SIREN` is set while it is on its way.
- **On the minimap** it is the police car's blue square (`emergencyBlips`).
- **Sound.** The van's flat four, as the ambulance.

## The model

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only police_van --preview <dir>
```

`build_police_van` is `build_saloon` on `transporter_shape()`, with the ambulance's `details` in the police car's livery: the sliding-door track and second grille, the `light_bar` over the front seats with its `beacon_L` and `beacon_R` nodes, a `blue` stripe down each side under the windows, and POLIS in `blue` on the panels between the cab's doors and the back wheels. It comes to 1,704 triangles.

## Protocol

No message and no component changed. The kinds a client must know grew by one ([NETWORKING.md](NETWORKING.md)).

## Tests

- `crates/sim/src/vehicle/tests.rs`: `ride_heights_differ_between_cars` covers it, and `a_police_van_is_as_fast_flat_out_as_the_ambulance_and_stays_on_its_wheels`.
- `crates/server/src/mechanics/emergency/tests.rs`: `at_four_stars_two_police_vans_come_as_well_as_the_four_cars`, `four_officers_get_out_of_a_police_van_each_with_a_rifle_and_two_with_a_carl_gustaf`, `a_vans_officers_all_get_back_in_and_it_drives_off_when_the_suspect_is_wanted_no_more`.
- `car.spec.ts` checks its wheels, seat markers, doors, headlights and triangle budget, and `beacons.spec.ts` its lenses.
