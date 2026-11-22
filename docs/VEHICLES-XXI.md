# Vehicles XXI: the air ambulance

The air ambulance is a second helicopter, after the EC135 that flies as an ambulance in Sweden: high-visibility yellow with green chequers, AMBULANS on its doors, skids, four blades and a fan shrouded in its fin. It carries a pilot and three passengers and has no guns. It flies on the gunship's flight model ([HELICOPTER.md](HELICOPTER.md)) with a tuning of its own.

## In play

- **The air ambulance** (`KIND_AIR_AMBULANCE` = 37) has 100 hp and the gunship's light alloy skin. Its engines are in the housing on its roof and its fuel is under the floor.
- **Placement.** It appears only where it is placed in the map editor (**Air ambulance**, + in the palette). It is skipped where its cabin or fin would stand in something. The rotor has no collider, so leave it about twelve metres clear by eye.
- **Seat 0 is the pilot,** front right as in a real one. It is flown exactly as the gunship: mouse for the pedals, W/S and A/D for the cyclic, Space and Shift for the collective. L switches the searchlight fixed under the nose, which reaches 80 m.
- **Seats 1 to 3 are passengers,** one beside the pilot and two behind. Their camera circles the helicopter. R moves anyone to a free seat.
- **Nobody aboard can fire.** It has no mounts, and its passengers' own guns are put away (`crew_guns: false`).
- **Those aboard show** through the glass, and can be shot through it (`Exposure::Glazed`).
- **It does not heal.** It is an ambulance in its looks and in what it is good for, which is fetching people quickly.

## How it flies

| | Air ambulance | Gunship |
|---|---|---|
| Mass | 2.6 t | 4.6 t |
| Level top speed over the ground | about 62 m/s | about 50 m/s |
| Full-lever climb | about 8 m/s | about 11 m/s |
| Hover lever, out of ground effect | 56% | 53% |
| Blade stall | 72 m/s | 58 m/s |
| Ceiling over the ground | 120 m | 120 m |

- **The climb** is held down by `pitch_flow` (20 m/s against 30): its blades lose their bite sooner as the air comes down through them.
- **The speed** comes from half the gunship's drag along the hull (`drag`), with less of a dash's lean reaching the blades (`tilt_flow` 0.25 against 0.35) so that it still holds its height.
- **Autorotation wants the lever all the way down.** At 10%, where the gunship keeps its rotor, this one's droops under 85%. Lever down, it keeps it, and a flare sets it down under 4 m/s.

## Its shape in the physics

It is drawn at real size, so it is bigger than the gunship, whose model is compressed.

- **The hull box** is 1.7 × 2.2 × 5.1 m, from the skids' soles to the top of the windows. It rests with its middle 1.1 m over the ground.
- **Three solids** beside the box add no mass: the engines' housing on the roof, the tail boom, and the fin. The tail reaches 5 m behind the box, so unlike the gunship's it stops against a house and can be shot. The fin is a `Role::Wings` solid, so placing it checks that the tail is clear as well as the cabin.
- **The rotor** is drawn only, 10.2 m across, as the gunship's is drawn only.

## What changed to make it

The client took "helicopter" to mean the gunship. It now asks the data:

- **`Rotorcraft::hover_lever`** is where a kind's lever hovers it. The physics does not read it.
- **The generated vehicle table has a `rotor` row** (`hoverLever`, `leverRate`, `etlSpeed`) for a rotor kind, null for the others. `isRotorcraft` (`client/src/sim/vehicles.ts`) is whether it has one.
- **The collective lever, the pilot camera's shudder, the engine's load and the beat of the blades** read that row or `isRotorcraft`.
- **`RotorModel`** (`client/src/render/rotor.ts`) is the base of both helicopters' models: turning rotors and a searchlight. The presenters look for it rather than for `HeliModel`.
- **Two seat roles**, `airPilot` and `rider`, have hints that mention no weapons and touch controls with no fire button. The pilot's speedometer goes to 280 km/h.

## The model

`tools/vehicles/build_air_ambulance.py` writes `client/public/models/air_ambulance.glb`, about 2,300 triangles:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_air_ambulance.py -- --preview <dir>
```

- **The cabin** is rings of faces from the nose to the boom, with a table saying which are glass, frame or skin. It has an inner skin where it can be seen into, because the game draws only the fronts of faces. `Skin` winds its faces by hand for the same reason: Blender cannot find the outside of an open shell.
- **The markings** are flat quads 12 mm off the skin, with no texture. The word is stroked from a small table of letters. There is no operator's mark or registration.
- **The fan** is drawn turning at 0.55 of the main rotor's speed, where a real one's turns nine times as fast: at that speed its ten blades would strobe.

## Tests

- `crates/sim/src/vehicle/tests.rs`: it rests level on its skids, spins up before it lifts off, holds height on its hover lever, climbs at about 8 m/s, outruns the gunship at about 62 m/s, autorotates to a landing, has a solid tail, and reads the collective. `scripted_ambulance_flight_matches_fixture` and the client's "WASM flies the air ambulance where the native server does" agree on a 480-tick flight.
- `crates/server/src/mechanics/vehicles/tests.rs`: four board and lift off and a fifth stays behind, nobody aboard can fire a gun they carry, and everyone gets out clear of the cabin and tail.
- `crates/shared/src/vehicles.rs`: it is unarmed, and every rotor kind says where it hovers.
- `client/src/__tests__/airambulance.spec.ts`: the model's hubs, seat markers, size, budget and rotors, and the seats' roles, hints and touch buttons. `collective.spec.ts` covers its lever and `headlights.spec.ts` its searchlight.

## Protocol

`PROTOCOL_VERSION` is unchanged: the only new thing on the wire is the kind id.
