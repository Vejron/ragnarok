# Vehicles XXII: the EPA traktor

An EPA traktor is a Swedish teenager's first car: an old saloon, often a big boxy Volvo, rebuilt so that it counts as a tractor, which may be driven from fifteen on a moped-class licence. The back doors are welded shut and the roof is cut off behind the front seats, where a wall with a window closes the cab, so that it has two seats. The gearbox is locked so that it cannot go faster than 30 km/h, and an orange slow-vehicle triangle hangs in its rear window. The game's is a generic boxy saloon of that kind.

## In play

- **The EPA traktor** (`KIND_EPA` = 38) has 100 hp and a car's sheet-steel skin. Its engine is under the bonnet and its fuel tank under the bed.
- **Seat 0 drives,** on the left. **Seat 1 is the passenger** beside them, who fires their own guns, as in any car.
- **The two show through the glass,** and can be shot through it (`Exposure::Glazed`). Its glass is drawn see-through, where the other cars' glass is opaque.
- **It goes no faster than 30 km/h** (`EPA_MAX_SPEED`, 8.33 m/s), and on the flat it tops out at about 29 km/h. It keeps the saloon's big engine, so it is at that speed in under two seconds.
- **Placement.** It appears only where it is placed in the map editor (**EPA traktor**, ▲ in the palette, since every letter is taken). Two stand in `home`: beside the jeep and beside a Roadster on the north side.

## Tuning

| | EPA traktor | Jeep | Sports car |
|---|---|---|---|
| Mass | 1.3 t | 1.3 t | 1.0 t |
| Engine force | 8,000 N | 10,000 N | 15,000 N |
| Top speed | 8.33 m/s (30 km/h) | 32 m/s | 50 m/s |
| Wheel radius | 0.32 m | 0.55 m | 0.32 m |
| Ride height | 0.447 m | 0.857 m | 0.397 m |
| Steering lock | 0.55 rad | 0.5 rad | 0.4 rad |

- **The box** is 1.74 × 0.6 × 4.76 m, a 740-sized saloon, with its belly about 15 cm off the ground: it has been lowered onto big rims and thin tyres.
- **It is rear-wheel driven** (`drivenFrom: 2`), as the saloons it is made from are.
- **Firm springs and a little centre-of-mass drop** (0.15 m) are enough: at 30 km/h even full lock does not lean it far.
- **It does not skim.** It is far too slow, so it wades into water and sinks.
- **Its engine** is a petrol four (`PETROL_FOUR`) in two low gears that revs high at its top speed, as everyone in town hears.

## The model

`build_epa()` in `tools/vehicles/build_car.py` writes `client/public/models/epa.glb`, about 1,250 triangles:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only epa --preview <dir>
```

- **The body** is a wedge of a bonnet, the front doors and their windows, then a blank panel where the back doors were, and the cab's back wall with a big window and the slow-vehicle triangle. Behind it is a short open bed over the rear axle.
- **The trim** is black: the bumpers, a strip down each side, the mirrors and the handles. It has square headlamps and amber indicators at the front, and at the back tall tail lamps either side of the plate, a tow ball and an exhaust.
- **A sunroof** is let into the roof. Inside are a dash, a steering wheel and two seats, seen through the glass.
- **It is painted old white** for every one of the kind (`paint` in `render/car.ts`), not in the per-car colours. Its glass has an opacity of 0.4 (`glass` in the same table).
- **`amber`** is a new material in `build_car.py`, for the indicators, the tail lamps' amber and the triangle. Like `white` and `red`, it keeps its colour.

## Tests

- `ride_heights_differ_between_cars` measures its ride height.
- `an_epa_traktor_is_at_30_km_h_in_two_seconds_goes_no_faster_and_stays_on_its_wheels_at_full_lock`, in `crates/sim/src/vehicle/tests.rs`.
- `car.spec.ts` checks its wheels, seat markers, headlights and triangle budget, as for the other cars, and that its glass can be seen through and the bus's cannot.
