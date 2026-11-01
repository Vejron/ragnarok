# Winter and skiing

A world is a winter one when its `area.json` says `"theme": "winter"`. The importer copies that into `Manifest::theme`, so the client and server both know. Kulla (`worlds/kulla`) is the first.

## The world

- **Snow** is drawn in the shader (`MAT_SNOW`): what faces up turns white, so steep ground and walls stay bare. It covers terrain and buildings. Trees have their own version in `render/trees.ts`, broken into clumps by noise.
- **Ice** is not a sim feature. `osm_import::winter::freeze` raises the ground to 3 cm under the water surface, so the lake is flat ground to physics, and the client still draws the surface, in its frozen colours (`freezeWater`).
- **Ski tracks** are `Tile::ski`: `piste:type=nordic` ways and `route=ski` relations, from a query of their own (`osm_ski.rs`). `"groomed_paths": true` in `area.json` grooms forest paths and tracks too, for places OSM maps few ski tracks. Kulla needs it.
- `render/season.ts` holds the one flag everything picks colours by. Both the main thread and the tile worker set it before the first tile.

## Skiing (`crates/shared/src/ski.rs`)

Skis are a switch, like the headlamp, and not an item: K (`BUTTON_SKIS`) puts them on or takes them off, nearly still (`SKI_CLIP_SPEED`), in a winter world only. `Movement` does it, so the key does nothing elsewhere. They stay on when you change weapon. Worn skis replicate through `carry`, now a mask of `CARRY_*`.

**Guns and poles.** A gun in hand (`fills_hands`: guns, launchers, the flare gun) leaves no hand for the poles, so they are stowed and only the legs push: a skate kick at `SKI_KICK_POWER`, a third of the poles, with nothing added by run. The herringbone still climbs. X slings what is in hand (the input's `slot` is `SLOT_NONE`) and the poles come back; X again or picking a slot brings it back. The tick's `armed` goes through `step_character` beside `speed_scale`, from the same hotbar lookup on both sides.

On skis, all speed is in `Character::knock`, so ragdolls launch at the skier's speed. `ski_step` adds gravity along the slope, friction (lower on a track), drag and the push of the poles, and turns the velocity towards the skis as far as they grip. The constants are `SKI_*` in `consts.rs`.

| Input | On skis |
|---|---|
| K | Skis on or off |
| X | Sling the gun or take it back: poles or gun |
| W | Push with the poles; with a gun in hand a weak skate kick |
| Shift + W | Pole hard: a strong extra push that fades away at `SKI_BOOST_TOP` (43 km/h), for stamina. Nothing extra with a gun in hand |
| S | Snowplow: brakes, and holds a tighter turn |
| Crouch | Tuck: half the drag, no pushing |
| A / D | Turn the skis 45° from the look |
| Aim | Coast straight, no steering, no push |
| Jump | A smaller jump |

**Climbing.** Steeper than 12° along the skis a push turns into a herringbone (the Swedish *saxning*): each step gets the skier up at a steady pace, slower the steeper it is, up to 30° at a walk and 42° with run held, which costs stamina. Past that, or winded, nothing holds and they slip back. On screen the skis splay into a V and lie on the hill, while the body stays up over them.

**Falls** are found inside the sim step, so they are predicted, and do no damage: skis too far across the direction of travel for 0.2 s at speed (speed × angle over `SKI_EDGE`), a landing harder than `SKI_LANDING`, a landing with the skis over 30° off the direction of travel, or being stopped short by something.

**In the air.** A skier leaves the ground when it falls away faster than the legs can follow: the climb rate at their speed drops by `SKI_CREST_DROP` over a look `SKI_CREST_LOOK` ahead, felt by a second ray from `SKI_CREST_SPEED` up. So a bump throws the fast and not the slow, with the climb it gave them, and a jump on the way up a crest adds to it. Gravity on a skier in the air is the lighter `SKI_AIR_GRAVITY`, and drag still acts, less in a tuck.

**The tuck** (crouch) halves drag and gives up the push: it pays only once the speed is up.

**Grooves.** A track has two (`SKI_GROOVE_OFFSET` either side of its centre line), and they hold skis: while both where the skis point and how they are going lie within 28° of one, the velocity follows it, round its bends too, a pull draws the skier on to its line, no edge can catch, and it is the fastest going there is. Looking further off or strafing lifts them out, and crossing at an angle never drops them in. Being in a groove is worked out each tick from position and velocity, so there is nothing to send. While aiming, only how they are going counts.

The slope comes from a ray straight down, as `terrain_below` does it. It is nudged off the skier (`SKI_RAY_NUDGE`), because one exactly on a ground grid line can miss the heightfield.

## On screen

Clips `ski_*` (poles) and, per stance, `armed_*` (the stance's grip on the same legs, chest kept up so the gun stays level) are baked in `tools/character/build_character.py`, and `Character` picks the gait from speed, slope and crouch, so nothing extra is sent. Skis and poles are meshes under the foot and hand bones (`render/skigear.ts`), the skis turned each frame to lie flat. A pole keeps its tip on the snow through the push, the hand coming down over it tipping it back, then swings back off the snow and round to the plant (`poleLift`). Grooves and spray are in `render/skitracks.ts`, client-only. Snowfall has its own particle pool (`render/snowfall.ts`).

With a gun the legs and skis go the way of travel and the torso carries the gun to the look, up to `SKI_TWIST`; past that the legs turn too. Other players' poles or gun follow from the held item, which is already sent.

## Headlamp

L switches it, for every soldier in every world: the server keeps it (`Player::lamp`) and shows it in `carry` (`CARRY_LAMP`). `Character::lampBeam` reads the lamp's place and direction off the head bone each frame, so it goes with every nod, stride and tumble. It is a spot light through `PointLights`, like a car's headlights, so it lights what it falls on and casts no shadows. It does little by day and comes up at dusk (`nightAt`). Riders wear it but do not cast its light.
