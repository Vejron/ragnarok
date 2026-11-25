# Swimming

Deep water floats a soldier off their feet. They swim at the surface with their head out, dive by
looking down, and can hold their breath for 30 s. After that they drown, unless they come up.

## When a soldier swims

- `Character::swimming` is set in `Physics::move_character_scaled` (`crates/sim/src/lib.rs`) by
  `shared::swim::swims`. It is the same step on the server and in the client's prediction.
- It reads the water over the soldier's feet (`Physics::water_over_feet`), waves and all:
  - more than `SWIM_DEPTH` (1.3 m) starts swimming;
  - less than `SWIM_LEAVE_DEPTH` (1.1 m) stops it.
  - The gap keeps it from flickering on a bank. Walking out up a shelving shore happens by itself.
- **Waves:** depths, the float height and whether the eyes are under all go by the surface with its
  waves (`Physics::surface_at`, `shared::waves`, on the tick the step is: `set_tick`).
  - A floating swimmer is lifted with the swell's rise and bobs with it.
  - The waves die away towards the shore, so wading in and out is as before.
  - The client's prediction sets the sim's tick to its estimate of the server's before each step on
    foot, and keeps that tick with the input. A replay after a snapshot steps each input on its own tick
    again, so it floats on the same waves as the first time (`Predictor.reconcile`).
  - **Drawn on the water's clock.** The water is drawn at the server's tick now, but a swimmer is drawn
    as of an earlier tick: the local one blended between the last two steps, everyone else at the render
    tick, about 100 ms behind. On a swell rising at over 1 m/s that put the water visibly up or down the
    body. So a swimmer is drawn carried by how much the water under them has risen since the tick they
    were stepped at (`Game.rewave`), and bobs exactly with the water as drawn.
- A swimmer floats with their capsule centre `SWIM_FLOAT` (0.45 m) under the surface. That puts the eye
  (`EYE_HEIGHT` over the centre) just out of the water, and leaves exactly `SWIM_DEPTH` over the feet.
- Frozen (winter) water is not water to the sim, so nobody swims in it.

## Moving

`Physics::swim` with `shared::swim::swim_velocity`. Gravity is off.

| Input | At the surface | Under (more than `SWIM_UNDER`, 0.9 m, below the float height) |
|---|---|---|
| W/A/S/D | swim flat at `SWIM_SPEED` (2 m/s) | forward follows the look, pitch and all |
| Shift | `SWIM_FAST_SPEED` (3.2 m/s), spending stamina as running does | the same |
| Space | nothing: the water holds you at the float height | kick up at `SWIM_RISE` (1.6 m/s) |
| C | sink at `SWIM_SINK` (1.4 m/s) | the same |
| Nothing | the water springs you to the float height (`SWIM_BUOYANCY`) | drift up at `SWIM_DRIFT` (0.5 m/s) |

- **Diving:** at the surface, swimming forward while looking down more than `SWIM_DIVE_PITCH` (52°,
  near your own feet) takes you under, and so does C. That is far past where the camera looks while
  swimming along, so only a swimmer who means it goes under.
  - A dive goes down at least `SWIM_DUCK` (1.5 m/s) until it is `SWIM_UNDER` down.
  - Short of that, a swimmer who stops diving is floated back up.
  - So nobody hangs just under the surface with their eyes going in and out of it, which would keep
    refilling their breath.
- **Vertical speed:** it comes round to what is wanted at `SWIM_ACCEL` plus `SWIM_DRAG` for every m/s
  it is off. A fall into deep water is braked hard; a stroke changes gently.
- **Collisions:** the bed, rocks and hulls still block, through the same two controller passes as
  walking.
- **Climbing out:** in water, and on foot within `WADE_REACH` (1 m) of it, the controller climbs steps
  of `SWIM_CLIMB` (0.6 m) and slopes up to `SWIM_CLIMB_DEG` (75°), so a swimmer can get up a steep bank.
  A swimmer standing on a bank above the float height is not pulled back down.
- **What a swimmer cannot do:** lie down, open a canopy (an open one closes on landing in the water) or
  ski. Knockback dies out at `SWIM_KNOCK_KEEP` a tick, and a knockdown wears off as on the ground.
- **No fall damage:** there was none on land either.

## Breath and drowning

`crates/server/src/mechanics/breath.rs`, which is server only.

- **Under the water:** while the eyes are under the surface, waves and all (`Physics::submerged`: the standing
  eye, or the prone one, so lying face down in a deep enough puddle counts), `Player::breath` counts
  down from `BREATH_FULL` (1800 ticks, 30 s).
- **Getting air back:** with the eyes out, it comes back `BREATH_REFILL` a tick (empty to full in 5 s).
- **Drowning:** with no air left, `DROWN_DAMAGE` (15 hp) comes off the torso every `DROWN_EVERY` ticks
  (1 s), so full health lasts 10 s.
  - The kill has no killer and the weapon `WEAPON_DROWN` (25). The kill feed reads
    "<victim> drowned".
- **Reset:** breath is refilled when dead and while seated in a vehicle.
- **On the wire:** `Vitals::breath` (0..255 of `BREATH_FULL`) is sent to the owner for the HUD's
  BREATH gauge. The gauge shows while the air is not full, and reads DROWNING once it is out.

## The gun is put away

While swimming:
- the server refuses all item use (`use_item` in `combat.rs`);
- the client does not predict a shot, aim or scope;
- `flags` carries no held item, so nobody sees one.

The hotbar selection is kept, and the gun is back in hand on land.

## On the wire

- `STANCE_SWIM` (5) is swimming with the head out. `STANCE_DIVE` (6) is with the eyes under. Both take
  precedence over every other stance.
- A swimmer is never shown crouching.
- The client's reconcile rebuilds `Character::swimming` from these stances. The WASM character
  (`CHARACTER_LEN`) carries it as its 18th number.
- Protocol 72.

## Animation

Four clips, shared by every stance since nothing is held. At the surface they are Quaternius'
Universal Animation Library's (CHARACTER-LOOKS.md), moved so the head rides where the build's own
clips have it, out of the water 1.3 m over the origin. Under the water they are the build's own
(`tools/character/build_character.py`):

| Clip | What it is |
|---|---|
| `swim_tread` | the library's `Swim_Idle_Loop`: upright with the head and shoulders out, arms sculling, legs kicking |
| `swim_fwd` | the library's `Swim_Fwd_Loop`: a front crawl along the surface, the head breaking it |
| `dive_idle` | flat under the water about `DIVE_PIVOT`, near the capsule centre, sculling slowly |
| `dive_fwd` | an underwater breaststroke: a pull to the thighs, a frog kick and a long glide |

How the build poses its own (it still poses a surface pair, a tread and a breaststroke, as the
place the library's are fitted to):
- **Arms:** `stroke_arm` takes directions in the root's frame and takes the chest's lean back out, so
  an arched back does not lift the arms out of the water.
- **Curves:** `keyed` runs a smooth spline through the stroke's keys, so nothing stops dead at one.
- **Life:** the right side runs a little behind the left, the wrists trail the arms, and the trunk
  rolls, surges and rises with the stroke.

The client blends them in `Character.swimTargets` (`client/src/render/character.ts`):
- **Which clip:** `swim` 1 (surface) or 2 (under) picks the pair, and speed picks still or stroking.
- **Stroke pace:** the strokes advance with the water covered, `SWIM_STROKE` (2.38 m, the front
  crawl's own, `STROKE swim_fwd` from the build) a cycle.
- **Tilt:** a diver tips about the middle of the body towards where it swims, up to `DIVE_TILT_MAX`.
- **What switches off:** the aim lean and the landing dip. The twist is kept at the surface only.
- **Previews:** `Blender -b -P tools/character/build_character.py -- --preview DIR --only soldier_m
  --strip swim_tread,swim_fwd,dive_fwd` draws each clip through its cycle with the surface line, on the
  people; `--preview-source DIR --only swim` (or `--only dive`) draws the source rig's own.

## Effects

- **Above:** a swimmer pushes a small wake into the wave field (`Surroundings.wade`, [WATER-WAVES.md](WATER-WAVES.md)). The old rings drawn as flat particles on the still level are gone: they lay at the wrong height on a swell, and the wave field's own rings replace them.
- **Below:** a body whose centre is more than `DIVE_DEPTH` (1.2 m) under leaves the surface still.
- **Camera:** the underwater fog was already there (`scene.setUnderwater`).

## Not done

- **Bots:** they have no idea of water. One that walks into deep water swims by the same physics, but
  does not plan for it.
- **No bubbles, no muffled sound.**
- **No ladders:** there is no way up a jetty's sheer side higher than `SWIM_CLIMB`.
