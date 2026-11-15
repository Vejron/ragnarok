# Vehicles XV: the bus

The bus is a twelve-metre city bus with a driver and twelve passengers. On a world with public transport the server drives them along the bus lines ([BUSES.md](BUSES.md)); anywhere else, or once its driver is gone, it is a big, slow car anyone can drive.

## In play

- **The bus** (`KIND_BUS` = 31) has 200 hp and thirteen seats. The buses of a world's lines come with the lines. One can also be placed in the map editor (**Bus**, O in the palette), parked with nobody driving it.
- **Seat 0 is the driver**, at the front on the left. It tops out at 16 m/s (58 km/h) and gets there slowly: about 3.5 m/s² away from a stop, under half a jeep's.
- **Seats 1 to 12 are passengers**, in six rows of two window seats. Like a jeep's passengers they fire the guns in their own hotbar.
- **Boarding is at the front door**, on the right (`board`): the middle of a twelve-metre bus is further from its ends than the 4 m a vehicle is boarded from. Whoever gets off stands outside that door (`exit`).
- **Its riders are behind glass** (`Exposure::Glazed`) and can be shot through it, the driver too.
- It turns in a circle of about nine metres' radius, so it needs most of a road to turn.
- It weighs about twelve tonnes: it rams twice as hard as a car and takes half the harm from a crash.

## The numbers

| | bus | jeep |
|---|---|---|
| Frame box, half extents | 1.25 × 0.5 × 6.0 m | 1.0 × 0.42 × 1.95 m |
| Mass | about 12 t | about 1.3 t |
| Wheelbase | 6 m | 2.5 m |
| Steering lock | 0.6 rad | 0.5 rad |
| Engine force | 42 kN | 10 kN |
| Top speed | 16 m/s | 32 m/s |
| Ride height | 1.057 m | 0.857 m |

The physics box is the frame under the floor. The body above it, up to 2.3 m over the hull's middle, is a solid of its own (`BUS_BODY_HALF`), which adds no mass. Its centre of mass is 0.3 m below the frame's middle, and on firm springs it stays on its wheels flat out at full lock. The tests are `ride_heights_differ_between_cars` and `a_bus_gets_going_slowly_and_stays_on_its_wheels_at_full_lock` in `crates/sim/src/vehicle/tests.rs`.

Thirteen seats needed the limit raised: `MAX_SEATS` (13) now bounds a vehicle's seat list, and `CAR_SEATS` (4) is only the jeep's own count. The wire carries a seat as a byte, so nothing in the protocol changed.

## The model

`build_bus` in `tools/vehicles/build_car.py` builds `client/public/models/bus.glb`, about 1,400 triangles:

```
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only bus
```

It is a tub on the frame with open window frames all round above the sill, so the riders show, a glass windscreen, two doors on the right and a seat at each of `BUS_SEAT_HEADS`. Like the other cars it has `wheel_*` on `pivot_*` and `seat_N` markers. The previews of a long body are taken from further off, in step with its length.

On the client it is a plain `CarModel` (`kinds/bus.ts`). Its seats are a driver and a passenger view that every later seat reuses, with the camera twice as far back as a car's.
