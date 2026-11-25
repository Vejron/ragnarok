# Animation: what moves a character besides its clips

A character's pose starts as a blend of baked clips (CHARACTER-LOOKS.md has where they come from).
`Character.update` in `client/src/render/character.ts` blends them by weight. One distance-paced
phase keeps the stepping clips in step. On top of that come the procedural layers that were already
there: the aim's lean, the waist's twist towards the look, breathing, idle glances, weight shifts,
leaning into turns, and the hit springs.

This write-up covers the runtime layer added over all of that (`client/src/render/ik.ts`). It is
what makes the body sit on the world rather than play over it. It runs only for characters within
`LAYER_RANGE` (40 m) of the camera; a scope's zoom counts as near. Further off nobody sees it.

## Order in a frame

1. The animator samples the clips into the bones.
2. **Inertia** eases out any jump in that pose.
3. The procedural layers turn bones: aim, twist, breathing, glances, hit springs.
4. The body is placed at its capsule and leaned.
5. **Sway** nods and rolls the head.
6. **Feet** stand the legs on the ground.
7. **Grip** lays the left hand on the gun.
8. Getting up from a ragdoll eases out of the ragdoll's pose (`easeUp`), and skis are laid on the snow.

A seated character runs steps 1–3 and 7.

## Feet on the ground

`Feet.fit` takes the clip's pose and the terrain (`Crowd.groundAt`, which `game.ts` sets to the loaded
terrain tiles):

- **The hips come down.** The model is lowered by the shortfall of the lower *planted* foot: how far
  the ground under it is below where the clip stands it, at most `REACH` (0.45 m). A foot swinging out
  over lower ground does not pull the hips down.
- **Each leg is solved onto its ground.** The ankle is moved up by how much higher its ground is than
  the clip's flat. Thigh and shin are turned by an analytic two-bone solver (`twoBone`), which keeps
  the knee bending the way it bends already. The swing keeps its shape over the ground, because the
  whole foot path is raised, not only a planted foot.
- **A planted foot tips to the slope**, by up to `TIP_MAX` (0.5 rad), from the terrain's normal under
  it. *Planted* is read from the clip: the ankle within `PLANTED` to `LIFTED` (2.5 to 10 cm) of
  standing flat.
- **A planted foot holds where it came down** until it lifts, so a foot does not skate when a blend
  or a correction moves the body under it. It lets go once held `LOCK_SLIP` (22 cm) from where the
  clip has it. The pull back is capped at that too, so a respawn or a big correction never stretches
  a leg back to where it stood.

**Only the terrain is known.** Floors, decks, bridges and roofs are not, so the layer trusts the
terrain only where it is within `ON_GROUND` (35 cm) of where the clip stands the body. Indoors on an
upper floor, on a boat or on a bridge it fades out, and the feet stand as the clip has them. Where a
tile is not loaded the ground is NaN, which fades it out too.

It is off in the air, lying down, sliding, on skis (`fitSkis` lays the skis instead), swimming,
seated, getting up and climbing out of a grave. It is also off with a leg lost.

## The left hand on the gun

The build puts a bone on the right hand for each stance: `grip`, `grip_pistol`, `grip_hold` and
`grip_shoulder`. Each is exactly where that stance's idle lays the left hand (`GRIPS` in
`build_character.py`). `Grip.hold` solves the left arm onto it, upper arm and forearm by the same
two-bone solver, and gives the hand the bone's rotation.

So the hand stays on the gun whatever pulls the arms apart: a blend between two clips' arms, a
flinch's spring on the upper arm, the aim and the twist. Its weight is the summed weight of the
stance's own clips. The crawls are left out: their left hand leaves the gun (the build prints `GRIP
<stance>: ...` with how far each clip's left hand strays). Free hands, the shared clips (swimming,
skiing with poles, golf, falling) and a lost arm get no grip.

## Jumps in the pose

Most transitions are crossfades, but some clips are snapped in: climbing out of a grave, getting
in or out by a door, and the weights forced while getting up. `Inertia` watches the clips' pose from
frame to frame. When a bone turns further than `SNAP_ANGLE` (0.5 rad, or 14 rad/s), or the root or
hips move further than `SNAP_MOVE` (12 cm), in one frame, it keeps the difference from the pose last
drawn and eases it away with `(1 + kt)·e^(-kt)` (about 0.1 s to half). That way the body moves
through the jump instead of teleporting.

It forgets the pose whenever something else has had the bones (a ragdoll, lying dead), the
character was not drawn, or it was out of range. Coming back is then never taken for a jump.

## Springs

- **The aim follows the look a moment late** (`Follow`, a critically damped spring with a 0.05 s
  half-life). This covers the pitch the spine, chest and neck lean by, and the twist the waist and
  chest take. The gun's line is cosmetic (shots leave from the camera), so this costs no aim.
- **The head sways with the hips** (`Sway`). Speeding up leaves it a little behind and stopping
  nods it on. Each footfall's bob dips it, and a sideways shove rolls it. The spring is a little
  under critical, so it settles with a small overshoot. Accelerations past 25 m/s² (a teleport) are
  cut off.

## What it costs

In Node, 20 armed characters jogging over a slope took about 100 µs a character a frame without the
layer and 3–10 µs more with it, so about 0.1–0.2 ms a frame for a crowd near the camera. Nothing in it
allocates once a character is made.

## Tests

- `ik.spec.ts`: the solver reaches what it can and points at what it cannot. It bends a straight
  chain the way it is told and works through turned parents. `Inertia` eases a snap and leaves smooth
  motion alone. `Follow` does not overshoot.
- `character.spec.ts`, "Character's runtime layer", on the real model:
  - feet on a 0.3 slope end within 2 cm of their ground, and the hips drop;
  - the layer stays off a floor 3 m over the terrain;
  - a planted foot holds when the body shifts 6 cm;
  - the feet let go when the body jumps 5 m;
  - the left hand stays within 5 mm of the grip under aim and a flinch;
  - climbing out of a grave moves the head less than 5 cm in the frame it snaps in.

## Not done

- **Ground other than terrain.** Floors, stairs and decks would need the sim's ray against
  buildings exposed to the client.
- **Springs on gear and hair.** These are skinned to the chest and head, with no bones of their own.
- **Slerp in the animator.** It blends with a normalised weighted sum (nlerp). Between neighbouring
  clips the difference does not show.
