# Vehicles XXVI: rear-wheel drive, burnouts and Run as a pedal

Each car's engine now drives the wheels it really does: the front pair, the rear pair or all four. A rear-driven car floored at low speed, on its boost especially, spins its rear tyres and lets its tail come round, so it can burn out and spin donuts. The spin is replicated, so every client sees the smoke and the marks. Run (Shift) is also now a pedal of its own: held alone, it floors the throttle on the boost.

## Which wheels drive

`Spec::drive` (`Drive::All`, `Front` or `Rear`) says which wheels the engine force goes to. It is shared among those wheels alone, so the total pull, and how quickly each car gets away, is the same as before.

| Drive | Kinds |
|---|---|
| `Rear` | Volvo 240 and 740, police car, Opel Omega, Ford Sierra, Opel Manta, VW Transporter, ambulance, bus, EPA traktor, golf cart |
| `Front` | Golf GTI, Opel Kadett, Ford Escort, Saab 900 and 9000 |
| `All` | everything else: the Bronco, the technical, trucks, tracks, and every kind that isn't a road car |

This matches what the client's tyre effects already assumed (`fx.drivenFrom` in `client/src/kinds/`). Pushed from behind, a rear-driven car runs a little wider at full lock under power than it did before.

## Wheelspin

In `Physics::drive_car` (`crates/sim/src/vehicle.rs`), a `Drive::Rear` car compares the push each rear wheel is asked for with what it can hold: the load on it (Rapier's `wheel_suspension_force` from the last step, newtons) times `TRACTION` (1.3).

- **Spin** goes from 0 at that grip to 1 at `SPIN_SPAN` (30 %) past it. It only happens in the low gears: wholly up to 8 m/s and fading out by 16 m/s (`SPIN_SPEEDS`), so a car floored round a fast corner keeps its tail.
- **The tail comes loose.** The rear wheels' side grip falls towards `SPIN_GRIP` (0.01) as they spin. How much depends on how far the front wheels are turned: at full lock the tail swings round the front wheels, and with the wheels straight (`SPIN_STRAIGHT`, a fifth as much) a burnout smokes but the car keeps its line.
- **Spinning tyres push less** (`SPIN_LOSS`, up to 30 % at full lock), so a donut stays where it is instead of launching.
- **Grip comes back over half a second** (`SPIN_FADE`) once the driver lets off, rather than snapping straight and throwing the car over.

On the boost, the Volvo 240 and 740, the police car, the Omega and the Sierra spin round where they stand: the 740 turns most of a circle in 3 s within 8 m, with its tail out about 40°. On the throttle alone their tyres grip, and they drive a circle at full lock. The Manta has power enough to spin donuts on the throttle alone, and on its boost it pulls away faster than its tyres let go. The vans and the bus are too heavy for their engines to spin their tyres.

## Run as a pedal

`CarControls::driven` floors the throttle while Run is held and the stick isn't pulled back (`pedal`), for every kind that boosts on Run (`Buttons::Boost` and the Roadster's `Buttons::Stunt`). So Shift alone drives ahead on the boost. Pulling back still brakes or reverses. With the boost tank empty, Run alone still drives ahead at the cruise. This is the shared sim, so the server and the client's prediction agree, and the replicated throttle that the engine sound follows shows it too.

## On the wire (protocol 101)

`Steer` gains `spin: u8 as byte`: how hard the vehicle's driven tyres spin, 0..1 decoded, filled from `Physics::car_wheelspin` while it has a driver. The vehicle presenter hands it to `VehicleFx.spin`. For a driven wheel, `VehicleFx` takes the larger of the slip it reads from the vehicle's motion and the spin, so a burnout at a crawl smokes, lays marks on the tarmac and throws grit, where its motion alone shows nothing.

## Tests

- `crates/sim/src/vehicle/tests.rs`: `a_740_on_the_boost_at_full_lock_spins_round_where_it_stands_and_stays_on_its_wheels`, `a_740_on_the_throttle_alone_at_full_lock_grips_and_drives_a_circle`, `every_rear_driven_car_spins_round_on_the_boost_and_front_driven_ones_do_not`, `a_740_floored_in_a_straight_line_on_the_boost_holds_its_line`, `letting_off_the_boost_mid_spin_the_740_catches_it_and_stays_on_its_wheels`, and `run_alone_floors_the_throttle_on_the_boost_and_pulling_back_still_brakes`. The Manta's scripted-drive checkpoints moved a little and were measured again (mirrored in `vehicle.spec.ts`).
- `crates/server/src/mechanics/vehicles/tests.rs`: `run_alone_spins_a_740_round_and_everyone_is_told_its_tyres_spin`.
- `client/src/__tests__/tyres.spec.ts`: a rear-driven car spinning its tyres at a crawl smokes and marks the tarmac.
