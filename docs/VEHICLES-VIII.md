# Vehicles VIII: the tupp

The tupp (Swedish for rooster) is a giant robot rooster of the Hedemora breed, the old landrace from Dalarna. It is the hen's other half: the same legs, wings, stamina and flight, but no eggs. Its head follows the driver's look, and its eyes burn a laser.

## In play

- **One tupp** (`KIND_TUPP` = 16) with 180 hp and one seat. On a world with no editor placements it takes a spawn point after the hen. On a world whose vehicles were placed in the map editor, it appears only where it is placed (**Tupp** in the editor's palette).
- **Moving** is the hen's: W/S and A/D to walk, Space to leap, held to flap and then glide. See [VEHICLES-VII.md](VEHICLES-VII.md).
- **The laser** is held, not tapped. Hold Click or F for 1.25 s while it charges; let go early and the charge is wasted. Then a beam burns for 1.5 s out to 250 m whatever the trigger does, and the head can sweep it. It needs 3 s to cool before it can charge again. The head turns up to about 50° either side of the legs. The beam strikes ten times a second, for 30 to a soldier, 30 to a plane and 14 to a hull, through up to 60 mm of plate. One whole burn does about 200 to a hull, which finishes anything short of a tank.

## The model

`tools/vehicles/build_tupp.py` builds `client/public/models/tupp.glb`, about 5,000 triangles. It keeps the hen's node names, so `TuppModel` can extend `HenModel`, and adds `skull`, under `head`, which turns with the aim. The shapes and the preview renders the hen and the tupp share are `Bird` and `preview` in `tools/vehicles/common.py`.

What makes it a Hedemora rather than any rooster:

- Dark plumage with a gold hackle and saddle, or lavender with a pale hackle. These are the breed's two commonest looks, handed out by vehicle id (`BREEDS` in `render/tupp.ts`).
- A small single comb and small wattles, which is how the breed copes with frost.
- A dark beak and dark legs.
- Feathered "trousers", with feathering down the shins to the toes.

The blue and yellow roundel on each wing is ours, not the breed's.

## The beam

The laser is a `Fire::Beam` with `WEAPON_LASER` (28), on an `Aim::Mount` at `TUPP_HEAD_PIVOT`. `Mount` keeps its ticks of `charge` and of `beam` left, and `work` in `mechanics/gunnery.rs` runs them. Both are cleared each tick before the driver and `needs` are checked, so a beam dies at once if its driver gets out or the tupp falls. The server settles a ray from between the eyes, so a hit lands where the reticle is. If that ray meets no soldier or vehicle, `combat::beam` tries 18 parallel rays around it, out to `TUPP_LASER_RADIUS` (1.5 m), and burns the nearest target they find, one a stroke. Each is stopped by walls as usual, must start where the eyes can see, and may end at most 4 m past the middle ray, so the beam never reaches round a corner.

`Turret`'s flags carry the state to clients as `TURRET_CHARGING` and `TURRET_BEAM`, and `LaserEyes` (`render/lasereyes.ts`) draws it every frame from them, starting and cutting the sounds on each change. A held beam has to be drawn every frame, so `Effects.laserCharge`, `laserBeam` and the rest spawn short-lived particles and streaks frame by frame.

A machine gun's rounds are drawn out to what the client's own ray meets, and that ray does not see vehicles or soldiers. So `machine_gun` always announces a laser's `ShotEvent`, hit or miss, with the ray's real end. `events.ts` keeps that end in `TuppModel.beamEnd` for the beams to run to, and draws the burn there (`Effects.laserBurn`). On the ground it also leaves a scar (`Scorchmarks`, `render/scorchmarks.ts`): quads laid like blood stains, one on each spot of the sweep between two hits, on the ground's slope and over the roads. They start red-hot, are left with glowing veins for a few seconds, and fade over four minutes. The menu's "Scorched craters" look turns them off too. Grass still grows through them.

A vehicle's hitscan `ShotEvent` carries the vehicle as `shooter`, not its gunner, so the client can find the eyes.

The sound is all synthesised in `vehicleaudio.ts`: `laserCharge`, `laserBeam`, `laserSizzle` and `laserFizzle`.

## Camera

The tupp uses the walker's rig (`{ name: 'walker' }`), limited by `TUPP_HEAD_*`.

## Protocol

`PROTOCOL_VERSION` is 58: new `KIND_TUPP` (16), `WEAPON_LASER` (28), `TURRET_CHARGING` and `TURRET_BEAM`. The physics is unchanged, so the tupp needs no checkpoints of its own: it is the hen's `Spec` with the same `HEN_*` numbers.
