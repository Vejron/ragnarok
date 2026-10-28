# Losing limbs

A stickman on foot can lose arms and legs as well as its head. The server decides it all
(`crates/server/src/mechanics/limbs.rs`). The movement it changes runs in the shared sim, so the
local player's prediction agrees with the server. No dice are rolled: the same hit on the same limb
always does the same thing.

## Where a round lands

The hurtbox is still the one capsule, 0.35 m in radius. Where on it a round struck is worked out in
the stickman's own frame (`limb_at`):

| zone | standing | lying down |
|---|---|---|
| head | above `HEAD_ZONE_Y` (as before: 2× damage, a kill takes the head off) | the same |
| leg | below `HIP_ZONE_Y`, 0.1 m under the capsule centre (about 0.75 m up) | the back of the body |
| arm | between hip and head, `ARM_ZONE_X` (0.25 m) or more out to the side | out to the side at the front |
| body | everything else | everything else |

The side is the one the round struck, as the stickman faces. Seated players lose no limbs, and
neither does a creature. A round through where a limb was strikes the body.

## Coming off

- A round in a limb does `LIMB_DAMAGE_MULT` (75 %) of its damage to health, and adds its full damage
  to that limb's wear.
- The limb comes off at a single hit of `SEVER_HIT` (40), or once its wear reaches `SEVER_WEAR` (40).
  That takes one sniper, AT rifle or close shotgun round, four AK5 rounds (12 each) or two revolver
  rounds.
- The round that kills takes the limb too, so the body falls without it.
- A blast that leaves someone alive but does at least `SEVER_HIT` tears off the limb nearest it: a leg
  when it goes off below the hips, an arm otherwise, on its side (`limbs::blasted`).

## What it does

| lost | effect |
|---|---|
| one leg | No running, sliding or crouching. Walking is a string of hops (`HOP_SPEED` up, at `HOP_SCALE` of walking speed), and a jump is a small one (`HOP_JUMP_SPEED`). Hopping costs no stamina. |
| both legs | Lies down for good, crawling at `LEGLESS_CRAWL_SCALE` of crawling speed. No jumping. |
| one arm | Guns still fire, but the spread grows by `ONE_ARM_SPREAD`, aiming and the scope no longer steady it, the view sways (`ONE_ARM_SWAY`, carried in the look the client sends) and a reload takes `ONE_ARM_RELOAD` (2) times as long. Losing an arm drops a reload or a bandage half done. |
| both arms | Nothing can be used but a bandage. Driving still works. |

The legs' effects are `legged()` in `crates/sim/src/lib.rs`. The server copies the lost limbs into
`Character::lost` every tick, and the client's prediction reads them from its own `limbs` component.

## Bleeding and bandages

- A stump bleeds, for show only: it drips and pulses red on the HUD, but costs no health.
- A bandage can be started with full health while a limb is lost, and with no arms.
- A finished bandage heals as before, stops all bleeding and puts one limb back, a leg before an arm.
- A new life has every limb.

## On the wire and on screen

- `limbs` (component 7, public, absent while whole): the lost limbs (`LIMB_*`, low four bits) and the
  stumps still bleeding (the same bits above `LIMB_BLEED_SHIFT`). Protocol 51.
- A lost limb is shrunk away at its root bone (`Character.setLimbs`). With the right arm gone the
  right hand is grown back to size, because the gun is skinned to it: the gun stays in hand at the
  shoulder.
- A limb that has just come off is thrown, bleeding, and the stump sprays (`Gore.sever`). A bleeding
  stump drips.
- Callouts:
  - The victim reads "Left leg gone! Bandage it".
  - The shooter reads "Left leg off!" when they hurt that player in the last 1.5 s.
- The HUD shows a small figure above the health bar while a limb is lost: lost limbs are faint, and
  bleeding ones pulse red.

## Not done

- One leg has no animation of its own: it hops on the walking clip with that leg gone.
- Bots don't change how they play: an armless bot keeps trying to fire, and none of them bandages.
