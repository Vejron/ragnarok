# Body parts

A soldier's head, torso and each limb have health of their own. Only the torso or the head running
out kills. The server decides it all (`crates/server/src/mechanics/limbs.rs` and `damage_entity` in
`life.rs`). The movement a lost limb changes runs in the shared sim, so the local player's prediction
agrees with the server. No dice are rolled: the same hit on the same part always does the same thing.

## Health

| part | health | at zero |
|---|---|---|
| torso | `PLAYER_MAX_HP` (100), the soldier's `Health` | dies |
| head | `HEAD_HP` (50) | dies, and the head comes off |
| each arm and leg | `LIMB_HP` (40) | comes off; the soldier lives |

- A round hurts only the part it struck. What it had beyond what that part had left is lost.
- So a sniper round in the leg takes the leg but cannot kill. One in the head kills, as do two revolver
  rounds or five AK5 rounds there.
- A blast hurts the torso, and the limb nearest it as much: a leg when it goes off below the hips, an
  arm otherwise, on its side (`limbs::blasted`).
- Seated players have no limbs to hit (their legs are inside), but their head can still be hit. Creatures
  have only a torso.

## Where a round lands

The hurtbox is still the one capsule, 0.35 m in radius. Where on it a round struck is worked out in
the soldier's own frame (`limb_at`):

| zone | standing | lying down |
|---|---|---|
| head | above `HEAD_ZONE_Y` | the same |
| leg | below `HIP_ZONE_Y`, 0.1 m under the capsule centre (about 0.75 m up) | the back of the body |
| arm | between hip and head, `ARM_ZONE_X` (0.25 m) or more out to the side | out to the side at the front |
| torso | everything else | everything else |

The side is the one the round struck, as the soldier faces. A round through where a limb was strikes the
torso.

## What a lost limb does

| lost | effect |
|---|---|
| one leg | No running, sliding or crouching. Walking is a string of hops (`HOP_SPEED` up, at `HOP_SCALE` of walking speed), and a jump is a small one (`HOP_JUMP_SPEED`). Hopping costs no stamina. |
| both legs | Lies down for good, crawling at `LEGLESS_CRAWL_SCALE` of crawling speed. No jumping. |
| one arm | Guns still fire, but the spread grows by `ONE_ARM_SPREAD`, aiming and the scope no longer steady it, the view sways (`ONE_ARM_SWAY`, carried in the look the client sends) and a reload takes `ONE_ARM_RELOAD` (2) times as long. Losing an arm drops a reload or a bandage half done. |
| both arms | Nothing can be used but a bandage. Driving still works. |

The legs' effects are `legged()` in `crates/sim/src/lib.rs`. The server copies the lost limbs into
`Character::lost` every tick, and the client's prediction reads them from its own `limbs` component.

## Bleeding, bandages and mending

- A stump bleeds, for show only: it drips and pulses red on the HUD, but costs no health.
- A bandage can be started whenever any part is hurt or lost, and with no arms.
- A finished bandage heals the torso, the head and every remaining limb by its 40, stops all bleeding
  and puts one limb back whole, a leg before an arm.
- Carrying the backpack mends the head and limbs as fast as the torso.
- A new life has every part whole.

## On the wire and on screen

- `limbs` (component 7, public, absent while whole): the lost limbs (`LIMB_*`, low four bits) and the
  stumps still bleeding (the same bits above `LIMB_BLEED_SHIFT`).
- `vitals` (component 17, the owner's): `hp` is the torso's, `parts` what the head and each limb have
  left (0 for a limb lost).
- `struck` (component 23, public, absent until the first hit of a life): the hits so far this life, the
  part (`PART_TORSO`, `PART_HEAD` or a `LIMB_*` bit), how much it lost and the bearing it came from.
  Every client makes the soldier flinch there (`Character.flinch`). Protocol 53.
- A lost limb is shrunk away at its root bone (`Character.setLimbs`). With the right arm gone the
  right hand is grown back to size, because the gun is skinned to it: the gun stays in hand at the
  shoulder.
- A limb that has just come off is thrown, bleeding, and the stump sprays (`Gore.sever`). A bleeding
  stump drips.
- Blood stains the ground (`render/bloodstains.ts`, a ring of quads like the skid marks).
  - Where it comes from:
    - About 40 % of the drops thrown by a hit, a severed limb or a death leave a stain where they come
      down, laid when they land.
    - A bleeding stump's drips stain where they land, and it leaves a spot under the soldier with
      every drip, a few a second.
    - A hit on a living soldier sprays out the far side, so the ground behind them is spattered.
    - A body that dies whole leaves a pool that spreads under it once it lies still.
    - A soldier with no legs who is still bleeding leaves a smear as they crawl.
  - How it looks: stains are ragged blots and streaked smears, solid with soft edges and no rim. Their
    colour, wet sheen and drying come from noise over the ground rather than from each stain, so
    stains that overlap run together into one puddle. They dry to brown-black and fade over 2 minutes,
    all in the shader. At most 2000 lie about at once; the oldest are laid over.
- Drops in the air are beads of liquid (the engine's `drop` particle shape): crisp-edged with a glint,
  stretched by speed, and whole until they reach the ground, where they end as their stain appears.
- Callouts:
  - The victim reads "Left leg gone! Bandage it".
  - The shooter reads "Left leg off!" when they hurt that player in the last 1.5 s.
- The HUD shows a small figure above the health bar while any part is hurt: each part fades from white
  through amber to red as its health goes, lost limbs are faint, and bleeding ones pulse red. The health
  bar is the torso's.

## Not done

- One leg has no animation of its own: it hops on the walking clip with that leg gone.
- Bots don't change how they play: an armless bot keeps trying to fire, and none of them bandages.
