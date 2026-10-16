# Vehicles V: the field mortar

A towed heavy mortar: a two-axle gun carriage with a steel tongue, one gunner, and a bomb that lobs high and bursts where it lands. A car tows it through a real joint. To fire it you stop, unhitch, get out and climb on. [VEHICLES-III.md](VEHICLES-III.md) covers the cars it is towed by, and [VEHICLES-IV.md](VEHICLES-IV.md) the plane; the mortar is built on the same vehicle spec and gunnery code as both.

## What you get

- **One mortar**, parked on a road-side spot after the planes have their runways, never on one (see [Placement](#placement)). It has 150 hp and about 900 kg, which a jeep can pull and a sports car struggles with.
- **Hitching.** Stop a car and press **H** with its driver in seat 0: a free mortar within 2.5 m of the car's rear tow point is hitched to it. Press **H** again to let it go. Hitching needs the car at 1.5 m/s or less, and the mortar must be empty. A towed mortar cannot be boarded and cannot fire.
- **Towing.** The mortar follows the car through a Rapier spherical joint, so it swings on corners and pulls on the car's rear. A wrecked car, or a respawn, lets it go.
- **Aiming.** The mouse sets the bearing: the mount follows the look at 3 rad/s, so the barrel stays on the crosshair. **W** and **S** walk a **range dial** from 50 m to 245 m (60 m/s, starting at 120 m). The barrel's elevation is the lob that puts the bomb on that range, so the barrel always points where the bomb will go.
- **Firing.** Click or **F**. A bomb leaves at 70 m/s and falls at `GRAVITY` 20, so the range runs from about 42 m (nearly vertical) to 245 m (45°). The mortar is shoved back 3 m/s and its 3.2 m barrel kicks back for 0.6 s. Reload is 7 s.
- **Blast.** 11 m radius, 260 damage at the centre, 30 knock. Wider and harder than a tank shell, and it bursts on the ground, not in the air: the bomb's lifetime is 10 s.
- **Landing marker.** A ring the size of the blast sits on the ground where the bomb would first land, pulsing, with an orange beam over it that is solid at the ground and fades out towards the sky. The HUD shows the dialled range in metres.

## Why the aim is a range dial

The first version mapped the mouse's vertical look onto the 45°–85° elevation. The look could only sweep about 35°, the targets it reached were a sliver of the view, and every shot had to be found by trial. The range dial replaces that: the mouse picks the bearing, the dial picks the distance, and the elevation follows from the physics. The landing marker is the feedback for placing a shot.

The lob solve is `lob_pitch` in `mechanics/gunnery.rs`. For a range `r` on level ground, the high angle is `θ = ½(π − asin(g·r / v²))`, clamped to the mortar's 45°–85° travel. It ignores the height of the ground at the target, so a shot into a valley lands a little short, and one onto a hill a little long.

## How it works

### Hitching

`hitch` in `mechanics/vehicles.rs` runs on **H**. It needs a seated driver in seat 0 of a car with wheels that is not wrecked and is at or below `HITCH_MAX_SPEED`. If the car is already towing, it unhitches. Otherwise it takes the nearest free, unwrecked, empty mortar within `HITCH_REACH`.

`physics/vehicle.rs` makes the joint. The car's tow anchor is at the middle of its rear face, `TOW_BACK` (0.25 m) behind it; the mortar's tongue is the same distance in front of its own front face. `hitch` builds a `SphericalJointBuilder` with `contacts_enabled(false)` and stores its handle on the mortar's `Car`. `unhitch`, `detach` and `towed_by` manage it; `reset_car` detaches first. The mortar's own `Spec` (`physics/vehicle/spec.rs`) is unpowered, with two axles so it stands on four wheels, and its `Lift::Wheels` handbrake holds it while parked.

### Gunnery

`work_mortar` in `mechanics/gunnery.rs` runs each tick for a `KIND_MORTAR` (dispatched from `Gunnery::on_tick`):

1. The mount's yaw slews towards the look, relative to the hull, at `MORTAR_TRAVERSE_SLEW`.
2. `BUTTON_RANGE_UP` and `BUTTON_RANGE_DOWN` move `gun.range` by `MORTAR_RANGE_RATE` × dt, clamped to `[MORTAR_RANGE_MIN, MORTAR_RANGE_MAX]`. A gun not yet dialled starts at `MORTAR_RANGE_START`.
3. `gun.pitch` is set directly from `lob_pitch(gun.range)`. It is not slewed, so the barrel the client draws is the one the bomb leaves from.
4. On **F** with the cooldown at zero, and not towed, `fire` launches a `PROJECTILE_MORTAR` from the tube and `kick_car` shoves the mortar back.

A bomb is a `Rocket` in `step_shells`, the same falling path as a tank shell. `combat.rs` must skip it in the rocket-motor branch: the first version did not, the motor drove it straight up, and it never came down. The regression test `a_mortar_bomb_arcs_up_and_lands_on_the_range_it_was_dialled_to` fires at the default range and checks the landing, and `the_range_dial_walks_the_lob_and_the_elevation_follows_it` checks the dial's limits and the elevation at each end.

### Placement

`MORTAR_COUNT` is 1. `mechanics/vehicles.rs` places it after the planes, on a spawn no plane or car took, skipping any spot on a plane's runway, and facing along the road. A mortar that does not fit simply is not placed. The fixture world has one spare spawn, so the count test asserts `mortars <= MORTAR_COUNT`.

## Wire

Protocol **26**:

- `KIND_MORTAR` (9), one seat, 150 hp. `MAX_VEHICLES` is 14.
- `PROJECTILE_MORTAR` (6), `WEAPON_MORTAR` (18), for the falling bomb and its blast.
- `BUTTON_HITCH` (`1 << 9`), `BUTTON_RANGE_UP` (`1 << 10`), `BUTTON_RANGE_DOWN` (`1 << 11`).
- A mortar's `Turret` is its mount: `yaw` is the traverse, `pitch` the elevation the barrel is at, `range` (decimetres) the dialled range, which the HUD reads.
- Protocol **30** added `Turret::flags` as a fifth field: `TURRET_STOWED` (bit 0) while the mortar is hitched, so clients draw it folded for travel.
- The byte fixtures in `protocol.rs` and `codec.spec.ts` were regenerated, and `fixtures/parity/gameplay.json` gained the mortar's row. [NETWORKING.md](NETWORKING.md) has the fields.

## Client

| File | What it does |
|---|---|
| `render/mortar.ts` | `MortarModel`: hand-built from primitives (no glTF), after the Haubits 77. A low carriage on tandem wheels that roll with the ground, two split trails forward to the tow end with spades, a firing pad, an upper carriage with its power unit that turns with the traverse, and a cradle that lifts the long barrel (recuperators, fume extractor, muzzle brake, breech). A gunner's head when someone works it, and a 0.6 s recoil on each shot. On `turret.stowed` it folds over `STOW_SECONDS` (1.8 s): the barrel swings onto the travel lock over the rear, then the trails close into the tow bar and the pad lifts. Unhitched, it unfolds the same way backwards. A mortar first seen hitched is drawn folded straight away. |
| `render/mortar-arc.ts` | `MortarArc`: the landing marker (ring and gradient beam), pulsing with `performance.now()`. |
| `sim/grenade.ts` | `dropTrajectory` takes the TTL, so the preview stops when the bomb would. |
| `game.ts` | The gunner's camera (behind the mount, looking where the mouse points), the `mortarPath` preview from the tube's real pose, `watchGun` for the recoil, the blast on `WEAPON_MORTAR`, prompts, the range in the HUD. |
| `sim/input.ts` | **H** latches `BUTTON_HITCH`; **W** and **S** hold the range buttons. |
| `sim/vehicle.ts`, `sim/physics.ts` | The mortar's spec and solid, so the driver's prediction treats a towed mortar as a static solid. |
| `App.vue` | The mortar seat's hint (bearing, range, fire), the driver's hint for **H**. |

## Tests

`cargo test -p server` and `cd client && npm test`.

- `physics/vehicle/tests.rs`: `a_hitched_mortar_is_pulled_along_behind_the_jeep_until_it_is_let_go`.
- `mechanics/gunnery/tests.rs`: the bomb lands at its dialled range and bursts on the ground; the dial clamps and the elevation follows.
- `mechanics/vehicles/tests.rs`: `cars_are_placed_along_the_road` counts the mortar within `MORTAR_COUNT`. The mortar settles and stays put with the other parked cars.
- `shared` parity: the gameplay fixture and the byte fixtures, regenerated with `UPDATE_PARITY=1 cargo test -p shared parity`.

## Not done, or not checked

- **Rubber-banding when towing.** The driver's client predicts the car, but the mortar is a static solid at its snapshot pose, so the joint's pull on the car is not predicted and reconciliation pulls it back. A fix needs the trailer and its joint in the driver's local simulation.
- Not driven in a browser: the feel of the range dial, the pulse and beam of the marker, and whether the joint stays stable in play are untested.
- No sound of its own: it is silent on its own (no engine), and its blast reuses the tank shell's blast profile rather than a dedicated mortar sound.
- No touch button for **H** or for the range dial.
- The range solve ignores the height of the ground at the target (see [Why the aim is a range dial](#why-the-aim-is-a-range-dial)).
