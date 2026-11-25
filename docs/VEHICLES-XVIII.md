# Vehicles XVIII: the AMV XA-361

The AMV XA-361 is an eight-wheeled amphibious armoured mortar carrier, modelled on the Finnish Patria AMV with the AMOS turret. It carries a driver, a mortar operator and one rider. The operator works two 120 mm mortar tubes side by side in a turret on its back. They lob their bombs as the field mortar does: the range dial sets how far, and the tubes rise to the high lob that lands the bomb there. They reach two thirds as far as the field mortar's tube and burst a little smaller. In return they fire two bombs in quick succession, after which the turret takes three seconds to load both tubes again.

## In play

- **The AMV** (`KIND_AMV` = 33) has 180 hp, as much as the Strf 90.
- **Placement.** It appears only where it is placed in the map editor (**AMV mortar**, C in the palette). The home world has one, beside the first Pbv 302.
- **It is true to size,** next to the other military vehicles: 7.7 m long, 2.8 m wide, 2.3 m to the hull roof and 3 m to the turret's top. That is longer than the Strf 90 (6.55 m) and the Pbv 302 (5.35 m), and the same width.
- **Seat 0 is the driver.** It drives as a car does, but its first two axles steer, as the real one's do. The second turns about three fifths as far as the first, so every wheel rolls round the same point. At a crawl on full lock it goes round a 9.7 m radius, against 11.2 m on its front axle alone, the bus's 11.2 m and the jeep's 6.9 m. It weighs 24 t, gets to 50 km/h in about three seconds and tops out at 25 m/s (90 km/h). It stays flat at full lock flat out.
- **It swims.** Most AMVs are amphibious; the heavy AMOS version usually is not, but here it is. It floats as the Pbv 302 does ([VEHICLES-XVII.md](VEHICLES-XVII.md)), 1.2 m deep, its roof 0.65 m clear of the water.
  - Afloat, its two water jets push it at up to 2.8 m/s (10 km/h) ahead and half that astern. It swings round at 0.35 rad/s, slower than the Pbv's 0.5: it is a longer hull.
  - Driven in at speed it ploughs in until its roof is nearly awash, then rises to its draft. Its wheels reach the bed again where the water is under about 1.6 m deep, and it drives out.
  - A wreck fills and sinks.
- **Seat 1 is the mortar operator,** who lays the tubes exactly as a field mortar's gunner does ([VEHICLES-V.md](VEHICLES-V.md)):
  - the mouse turns the turret all the way round, at 1 rad/s (the field mortar is cranked round at 0.5);
  - W and S walk the range dial from 40 m to 200 m, 50 m a second, starting at 100 m;
  - holding aim lifts the view over the mark, which the mouse moves over the ground, and the tubes are laid to land on it;
  - the landing mark is drawn the size of this bomb's burst.
  - The tubes elevate at 0.6 rad/s, between 45° (200 m) and about 83° (40 m).
- **Firing.** Holding the trigger fires one tube, then the other a third of a second later. Both tubes are loaded again three seconds after the last round fired. A single round fired and left is loaded back in the same three seconds. The HUD's reload ring shows the short wait between the tubes, and then the long one, while the turret reports `TURRET_EMPTY`.
- **The bomb** (`PROJECTILE_AMV_BOMB`) flies at 63.25 m/s, which lands it 200 m out at 45°. Its burst is 9.5 m wide, takes 210 off at its middle and knocks with 25. The field mortar's is 11 m, 260 and 30. It digs a crater 7.5 m wide and 1.3 m deep, and goes through 15 mm of plate, as a mortar bomb does. Kills are credited as the mortar's.
- **Seat 2 is a rider** in the back. Everyone is sealed in, out of sight and out of reach of small arms, and leaves by the rear door.
- **The armour** is 30 mm in front, 18 mm on the sides, 14 mm on the rear door, 10 mm on the roof and 16 mm underneath; the turret is 14 mm with an 8 mm roof. That is proof against rifles all round and against heavy machine guns from ahead. The driver's periscope is a weak spot. Shooting out the breeches stops the mortar. The engine is at the front right, the bombs are racked behind the turret and the fuel is under the floor at the back.
- **Its wheels cannot be shot off,** and it does not tow. The run-flat tyres sit behind armoured flanks, so it is not given a car's four breakable wheels (`car_wheels: false`).
- **Its turret stays on.** The turret's solid is a `Role::Part`, not a `Role::Turret`, so it cannot be blown off as a tank's can. A wheeled kind's damage slots 4 to 7 are its wheels, which a shed turret would share.

## What changed to make it

Two things that were the field mortar's alone became data that any mount can carry. No gunnery code branches on the kind.

- **The range dial is a `Dial`** (`crates/shared/src/mounts.rs`), carried by `Aim::Lob { slew, pitch_slew, dial }`. It holds the bomb's muzzle speed, the dial's ends, its start and its rate. `Dial::pitch` and `Dial::range` replace gunnery's `lob_pitch` and `lob_range`, which were fixed to `MORTAR_SHELL_SPEED`. A test holds every dial's speed equal to its mount's `Fire::Shell::speed`, and its far end within the bomb's reach.
- **`Refill::Reload(ticks)`** loads a mount to full `ticks` after its last round, each round starting the count over. Fired dry, it shows `TURRET_EMPTY` (bit 5 of the turret's flags).

The AMV's mount is then one row: `Aim::Lob` with its own dial, `Muzzle::Twin` (the walker's alternating barrels), `Fire::Shell` with the new bomb, a 20-tick cooldown and `Ammo { full: 2, refill: Refill::Reload(180) }`.

## The definition

`crates/shared/src/vehicles/amv.rs` holds everything about it.

- **The hull box** is 2.8 × 1.85 × 7.7 m at 600 kg/m³, which is 24 t. Its engine pulls 120 kN.
- **The axles** are four, in two pairs (`AMV_AXLES_Z`). The physics lays its wheels from `Spec::axles`, so eight wheels needed nothing new.
- **Two axles steer.** `Spec` has a new field, `steer_axles`: how many axles from the front steer. It is `Spec::FRONT_STEER` (1) for every other kind and `AMV_STEER_AXLES` (2) for the AMV. `Spec::steer_share` gives each steered axle's share of the lock: its distance from the middle of the unsteered axles over the first axle's. For the AMV that is 1 and 2.45/3.95 ≈ 0.62. The sim steers each wheel by its axle's share and spares every steered wheel the handbrake. The client's copy is `steerShares` in `sim/slip.ts`, which turns the model's `pivot_FL2`/`FR2` and the tyre marks the same way.
- **It rides** `AMV_RIDE_HEIGHT` (1.35 m) above the ground, measured by `ride_heights_differ_between_cars`. That leaves 0.43 m under the belly. Its centre of mass is dropped 0.6 m, which keeps it upright at full lock.
- **It floats** by `Spec::swim` (`AMV_DRAFT`, `AMV_SWIM_SPEED`, `AMV_SWIM_THRUST`, `AMV_SWIM_TURN`), the Pbv's swimming with no new code. One change in the sim: a car whose wheels are off the ground gets air control, which pitches its nose with the throttle. A hull that floats no longer gets that while it is over water, or the jets' push would stand it on its tail.
- **The turret** is a drum on the roof behind the middle (`AMV_TURRET_RING`), 2.2 m across. The tubes pivot at `AMV_GUN_PIVOT`, 0.3 m either side of the middle and 3 m long.

## The client

- **`client/src/kinds/amv.ts`** is the kind. The driver drives; the operator's seat is the field mortar's rig, gauge and landing mark; the rider rides along. `RideView` has a new `refill`, the long reload, which the HUD uses while the turret is empty.
- **`client/src/render/amv.ts`** is `AmvModel`, a `CarModel` from `amv.glb` that turns `turret` and elevates `gun` as the technical's model does. `barrel(side)` gives either tube, and the gun presenter fires them in turn, with the mortar's flash and smoke.
- **`LobGun`** (`render/mortar.ts`) is what a model with a mortar's tubes provides: its `Lob` (`sim/lob.ts`, the client's copy of the dial and bomb) and `barrel()`. The mortar rig in `cameras.ts`, the landing arc in `arcs.ts` and `layTube` take it, so the field mortar and the AMV share them.
- **`CarModel`** now spins `wheel_FL2`-style middle wheels too, and a kind can be painted one colour (`paint` in `KINDS`): the AMV is army green whoever drives it.
- **Its wheels' marks and dust** come from `AMV_WHEELS` in `sim/slip.ts`, all eight of them. The tank music plays for its crew.

## The model

`build_amv` in `tools/vehicles/build_car.py` builds `client/public/models/amv.glb`, about 2,300 triangles.

- **The upper hull overhangs the wheels.** It has an upright nose plate, a glacis sloping up to a flat roof, stowage bins on the sides and an upright rear with one big door. Below it the lower hull runs narrow between the wheels, behind a dark skirt.
- **The roof** carries the driver's hatch and periscopes at the front left, the engine's grilles at the front right, and the turret ring.
- **The turret** is a wide, flat-topped box with a chamfered face, the commander's hatch, a sight and smoke dischargers.
- **The tubes** are a pair in armoured sleeves behind one mantlet, pitching about the pivot.
- **The wheels** are eight, `pivot_FL`/`FR` and `pivot_FL2`/`FR2` (which steer), `pivot_RL2`/`RR2` and `pivot_RL`/`RR`.

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- --only amv --preview <dir>
```

## Protocol

`PROTOCOL_VERSION` is 85. The message layouts are unchanged; what is new is `KIND_AMV` (33), `PROJECTILE_AMV_BOMB` (14) and the turret flag `TURRET_EMPTY` (bit 5).
