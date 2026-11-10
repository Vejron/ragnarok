---
name: animate-soldier
description: Add or rework an animation clip of the soldier in Ragnarok: pose it in tools/character/build_character.py, check it from three views, rebuild player.glb and blend it in the client. Use when the user asks for a new movement animation (swimming, climbing, vaulting and so on) or says an existing one looks wrong, stiff or mechanical.
---

# Animating the soldier

The soldier's clips are not drawn by hand. `tools/character/build_character.py` poses the rig in code for each frame and bakes the result into `client/public/models/player.glb`. The client (`client/src/render/character.ts`) blends the clips by name.

The swimming clips (`swim_tread`, `swim_fwd`, `dive_idle`, `dive_fwd`, written up in `docs/SWIMMING.md`) are the newest and the best template. They took three rounds of corrections from the user, and every one came from something below.

## 1. Look before you pose

Render the clip from the side, from above and from the front, and look at every view before calling it done.

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b -P tools/character/build_character.py -- --out /tmp/x.glb --preview DIR --only swim
$B -b -P tools/character/sheet.py -- DIR swim_fwd      # one sheet: a row a view, a column a frame
```

- **One view hides mistakes.** Arms and legs that cross the body's middle look right from the side. Only the top and front views show them.
- **Draw the reference the clip is about.** The swimming previews draw the waterline as a slab, which is how arms waving in the air were finally seen. A climb wants the wall, a vault the obstacle.
- **`--only PREFIX`** keeps the run to the shots whose names start with it. Add your clip's shots to `preview()` at the bottom of the script.
- **PIL and ImageMagick are not installed.** `sheet.py` tiles the shots with Blender's own numpy.

## 2. How the rig bites

- **`bake` sets the carry pose first.** Every frame starts from the rest pose with the arms holding a gun (`set_arms_carry`). A clip with empty hands must set every arm bone itself, or the carry pose shows through.
- **Bones turn relative to their parent.** The arms hang off the chest, so an arched or leaning trunk carries them with it. If a hand must be at a place in the world (under the water, on a rung), take the trunk's lean back out, as `stroke_arm` does with the `lean` that `swim_trunk` returns.
- **Sideways rotation flips overhead.** The `pose_rot` turn that spreads a hanging arm or leg outward pulls it inward once the limb points up or forward. Do not reason in Euler angles for a limb that leaves its rest direction: say where it points instead (`stroke_arm` takes directions in the root's frame and works for both sides).
- **Tip the body about a point.** `pitch_about(angle, pivot)` turns the root about a height on the body and not about the feet, so a flat swimmer stays where the capsule is.
- **Axes:** x is the soldier's left, -y is forward, z is up.

## 3. Make it move like a body

- **Never ease to a stop at a key.** Keys joined by ease-in, ease-out curves stop dead at each one, and the clip reads as a robot. `keyed(t, keys)` runs one smooth periodic spline through all the keys; give it the poses of the stroke and their phases.
- **Offset the two sides.** `SWIM_SIDES` runs the right side a little behind the left.
- **Let things trail.** Wrists lag the forearm, the trunk rolls, rises and surges with the effort, and the head counters the chest.
- **Keep the cycle slow enough.** A cycle that fits the speed on paper can still look frantic. The crawl and the swim strokes are both paced below what the limbs would strictly cover.

## 4. The client

- **Blend:** give the clip a share of the `targets` map in `Character.update` (or in its own function, as `swimTargets` and `skiTargets`), and take that share out of the others so the weights still sum to one.
- **Pace:** a clip the feet drive goes in `SPEED_OF` with the ground speed at which its feet stop sliding. A clip that covers distance some other way advances a phase by distance, as `SWIM_STROKE` (metres a cycle) does.
- **Switch off what fights it:** the aim lean, the landing dip and the held item, where they make no sense.
- **Tests:** add the clip's name to the list in `client/src/__tests__/character.spec.ts`, and a test that the right clip carries the weight in the new state.
- **Who sees it:** other players get the state from a stance or flag (`people.ts`), and your own soldier from the predicted state (`poseLocal` in `game.ts`). Wire both.

## 5. Finish

1. Rebuild the model without `--out`, so it lands in `client/public/models/player.glb`.
2. Run `npx vitest run src/__tests__/character.spec.ts` and `npm run type-check` in `client/`.
3. Add the clip to the script's docstring and to the feature's write-up in `docs/`.
4. Tell the user it has only been seen in still frames. Motion is judged in the game, so ask them to look.
