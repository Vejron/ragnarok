# Gunsmoke: a shell leaving the barrel

A gun that fires shells used to give one flash, one lick of flame and four grey puffs that were gone
in under two seconds, the same for a tank's cannon as for an autocannon. Now a shot is a blast sized
to the gun: fire along the bore, a bank of smoke that rolls out and hangs, jets from the muzzle
brake, burning grains that arc away and bounce, dust off the ground, and a barrel that goes on
smoking for seconds after. Smoke everywhere in the game churns, leans with the wind and is lit by
the flashes and fires near it.

Nothing here is on the wire or in the Rust: it is all drawn from the shot counter the client
already had.

## The shot (`Effects.gunfire`, `client/src/render/effects.ts`)

`gunfire(muzzle, direction, profile, fired, scene)` draws, in order:

1. **The flash**: a glare at the muzzle. This is all a gun further than 200 m from the camera gets.
2. **Fire**: puffs of flame along the bore that cool to a dark ember red.
3. **The blast's smoke**: puffs blown out ahead, the first of them furthest and fastest (up to
   14 m/s), with heavy drag, so they stall a few metres out and swell into a bank. They leave
   already a third of their full width, and they `hold` their opacity (see below).
4. **Brake jets** (a cannon only): a flash, a flame and smoke thrown out level from both sides of
   the muzzle.
5. **Sparks**: burning grains of propellant in a cone ahead, stretched along their flight, falling
   and bouncing off the ground. One in five is an ember instead, slow and carried about by the air.
6. **Dust**: when the muzzle is within 4 m of the ground, a cloud lifted off it under the blast.

The light on the world (`scene.lights.flash`) is thrown by whoever fires, from the profile's `light`.

`fired` is how far the shot counter moved since the last frame. A rotary cannon firing faster than
the frame rate used to show one shot; now the smoke and sparks of up to three are drawn.

**Budget**, as an explosion's: past 80 m a shot has half its smoke and sparks, past 200 m none, and
all of it is scaled by 0.65 when more than 800 smoke particles are alive and by 0.4 past 1500.

## The profiles (`client/src/render/gunfire.ts`)

A `MuzzleProfile` is plain data: the flash's width, the flame's reach, how many puffs and how wide
and dark, how long they hang, the brake's puffs, the sparks, the dust, and how the barrel smokes
afterwards. `guns.ts` picks one by the gun's model (`cannonOf`).

| Profile | Guns | Smoke puffs | Hangs | Sparks | Barrel smokes |
|---|---|---|---|---|---|
| `CANNON` | tank, Strv S, Pak | 18, and 4 from each side of the brake | 5.5 s | 22 | 5 s |
| `AUTOCANNON` | Strf, helicopter | 4 | 2 s | 6 | 3 s, building over a burst |
| `ROTARY` | Titan | 5 | 2.5 s | 10 | 4 s, building over a burst |
| `MORTAR` | mortar | 10, up out of the tube | 3.5 s | 8 | 3.5 s |
| `BLACK_POWDER` | ship's gun | 18, dark, 6.5 m wide | 7 s | 14 | 6 s |
| `LAUNCHER` | Carl Gustaf (its backblast is drawn apart) | 4 | 1.8 s | 4 | no |
| `BEAM` | walker | none: the flash alone | | | no |

The old `muzzleFlash` and `blackPowder` are gone; black powder is a profile like the rest, and its
smoke now drifts with the game's wind instead of a boosted one of its own.

## The barrel afterwards (`Effects.barrelSmoke`, `Turrets` in `presenters/guns.ts`)

`Turrets` keeps a heat per gun, 0 to 1. A shot adds the profile's `heat` (1 for a cannon, 0.2 to 0.3
for the guns that fire in bursts, so their barrels smoke more the longer they fire), and it runs
down to nothing over the profile's `linger` seconds. While there is any, `barrelSmoke` lays wisps at
the muzzle each frame, **where the muzzle is that frame**, so the smoke follows a gun that
traverses: thick and quick with a lick of flame while the barrel is near its hottest, thinning to a
curl. Not drawn past 120 m.

## Small arms

A rifle shot used to be one glare half a metre across, the same for a revolver as for the sniper,
and no smoke. Now a soldier's gun and a vehicle's machine gun go off through `gunfire` like a cannon,
with profiles of their own, and the flash is a shaped flare for every gun.

| Profile | Guns | Smoke puffs | Hangs | Barrel smokes |
|---|---|---|---|---|
| `PISTOL` | revolver, flare gun | 2, 0.8 m | 1.2 s | no |
| `RIFLE` | AK5 (and any hand gun not named) | 2, 0.9 m | 1.5 s | 2.5 s, only after a long burst (heat 0.08 a round) |
| `SNIPER` | sniper, AT rifle | 5, 1.4 m, dust when fired low | 2.2 s | 2 s |
| `SHOTGUN` | shotgun | 4, 1.2 m, a long tongue of fire | 1.6 s | 1.5 s |
| `MACHINE_GUN` | a vehicle's MGs: technical, planes, roof guns, Stridsbåt | 1 a round | 1.4 s | no |

`smallArmsOf(item)` picks a hand gun's profile. The smoke is pale haze (smokeless powder), not a
cannon's grey.

**The flare** (`Effects.muzzleFlare`, the start of every `gunfire`): a white-hot core glare inside a
warm one, a cone of fire along the bore (a wide tongue and a hot core, both `FlashEffects` streaks
that shoot a little further out as they go), and a star of four petals round it at a new roll every
shot, each a thin streak over a short wide one so it tapers. All of it is gone in 0.04 to 0.06 s
(a cannon's outer glare in 0.12 s).

**One shot, drawn once.** `Events.soldierFired` is the one path: the flash and smoke, the light, and
heat in the barrel. The local player's shot goes off when they fire it (`Game.fireFeedback`), so the
server's bullet or hitscan event draws nothing more for it at the muzzle; everyone else's goes off
when their bullet is first drawn (`bulletLeft`) or their hitscan shot arrives. `Events.smokeBarrels`
lays the wisps at each soldier's muzzle every frame while their barrel is hot.

**Down the scope** (`gunfire(…, scoped)`): no flare, which would fill the eye, a third of the smoke
from just ahead of and below the eye, and no light.

**Recoil** (`Recoil` on an `ItemDef` in `crates/shared/src/items.rs`; `client/src/render/recoil.ts`).
Each shot lifts the shooter's aim `rise` radians and turns it `left` radians to the left, give or
take `jitter`:

| Gun | Up a shot | Left a shot | Give or take |
|---|---|---|---|
| AK5 | 0.004 rad | 0.0012 | 0.0008 |
| Revolver | 0.014 | 0.003 | 0.002 |
| Sniper | 0.035 | 0 | 0.008 |

The climb goes into the aim itself (`GunClimb`, handed to `InputController.climb` over about
0.075 s, so the aim rises rather than jumps), and the server is sent the raised aim: a burst walks
up and left unless held down. A 30-round AK5 burst left alone climbs about 7°. The AK5's bloom
was lowered for it, from 0.005 a shot up to 0.05 to 0.0035 up to 0.035.

Down a scope, four fifths of the kick only throws the view (`ScopeRecoil`: up in 0.05 s, a
critically damped settle back over about half a second), and a fifth (`CLIMB_SHARE`) goes into the
aim, so the next shot needs a small pull back down. A light camera jolt goes with it.

Gone: `Effects.muzzle` (the glare), the glare `Effects.tracer` drew at the muzzle, and
`Effects.blast` as a shotgun's shot; it is the Carl Gustaf's `backblast` now and nothing else: a
4.5 m cone of fire with a white-hot core, fourteen fireballs rolling back in it, burning grains, and
a bank of smoke left 1 to 3.5 m behind the venturi.

## Shells in flight (`client/src/render/trails.ts`)

- A tank's, a Pak's or a mortar's shell leaves a wake that lasts 3 s (it was 1.6 s), wider and more
  opaque, laid in 1.5 m steps so it is no more segments than before.
- An autocannon's and a rotary cannon's rounds, which had only their hot streak, leave a thin wisp
  for 0.9 s. The rotary's rounds used the shell's long wake before; at its rate of fire they now take
  the autocannon's short one, laid in 2.5 m steps.
- Smoke wakes `wander` (they kink and spread as they age) and `drift` (the wind carries them off).

## What the engine gained (`engine/src/fx/`)

All of it is per particle, in the compute pass, or per vertex. The one thing added per pixel is the
sun's shading on a puff, a square root and a dot product.

- **`turbulence`** (m/s): the air's churning carries the particle about. The flow is two sizes of
  eddy, each with no divergence, so smoke rolls and folds without bunching, and it drifts with time.
  Particles without it skip the work.
- **`wind`** (0 to 1) and `ParticleSystem.setWind`: drag is against air that moves with the wind, so
  smoke blown out of a barrel stalls and then leans off downwind. The client sets the wind from the
  game's own (`sim/wind.ts`, the one that fills sails) at `WIND_SPEED` 1.2 m/s. The four copies of a
  constant `WIND` in the effect files are gone.
- **`hold`**: opacity is there within a tenth of a second, stays for the first half of the
  particle's life and thins over the second, where a plain particle fades from the moment it is
  born. This is what makes smoke hang.
- **Lit smoke.** The compute pass reads the frame's point and spot lights through the same grid the
  scene shaders use, and stores the light on each alpha puff. A muzzle flash lights its own smoke,
  a blast the smoke round it, a headlamp the smoke in its beam, and a burning wreck the underside
  of its plume. The two buffers bring the compute stage to the eight storage buffers a default
  device allows; the vertex stage, which was already full, gains none.
- **Sun on a puff.** By day a puff is shaded as a lumpy ball: bright toward the sun, darker away,
  with a bright rim when the sun is behind it.
- **Ribbons**: `wander` in the vertex shader, `drift` on the CPU as the points age (`Ribbons.setWind`).
- **Ink under smoke.** The outlines are drawn from depth in the output pass, and smoke writes no
  depth, so the lines of whatever stood behind a bank of smoke showed through it. The output pass
  now reads the half-resolution target's coverage where it is about to draw a line, and fades the
  line by it.

## Cost

Measured with `gpu-bench.mjs` (whole frames to the GPU's finish, each variant alternately with the
plain frame), quality `high`, 2268×1258:

| | forest | night | water |
|---|---|---|---|
| plain frame | 3.23 ms | 3.16 ms | 2.73 ms |
| `gunsmoke`: four cannons firing 14 m ahead and one at the camera's own muzzle, 0.6 s on | +0.15 | +0.15 | +0.15 |
| 200 puffs 3 to 5 m wide across the view, before these changes | +0.11 | +0.10 | |
| the same, after (sun shading, lights, the larger particle) | +0.13 | +0.12 | +0.13 |

The old shot cost 0.01 ms because there was nearly nothing in it. The plain frame did not move with
the ink's new read.

```sh
node client/scripts/gpu-bench.mjs --views forest,night --parts gunsmoke --no-shots
node client/scripts/gpu-bench.mjs --views forest,night --parts gunsmoke --shoot --scale 1   # pictures
```

## Not done

- **Heat haze** at the muzzle: it needs a copy of the scene's colour every frame.
- **Sorted smoke.** Puffs draw in no fixed order. Light and dark smoke mixed thickly could show it;
  the pictures so far do not.
- **Smoke lit at night by its own albedo.** A puff is darkened by the ground light when it is born,
  so a lamp or a flash lights it as grey whatever its colour.
