# Audio: engines, the ground, footsteps, launches, water, destruction and the world about you

Each vehicle's engine is made firing by firing on the audio thread, from a description of the real machine. It revs with the driver's throttle, through its own gearbox. Under the engine, the wheels, tracks or hull are heard on the ground they roll over. Soldiers' feet are heard on what they step on. Rockets, bombs and grenades are heard leaving and in flight, rounds crack past and strike, and splashes, breaking glass and falling buildings are heard.

## Where sounds come from

- **Recorded:** gunshots, machine guns, the cannon, explosions, splashes, glass, debris, a building coming down, and rounds striking earth or stone. These are CC0 recordings in `client/public/sfx`, made by `tools/sfx/build_sfx.py` and loaded by `sfx.ts` (`SampleBank`), with the sources in `CREDITS.md`. The tank music (`music.ts`) is recorded too.
- **Synthesised one-shots:** everything else in `vehicleaudio.ts` (`burst` and `ping`: filtered noise and gliding tones). Every recorded sound also has a synthesised stand-in, played until its file has loaded or if it is missing.
- **On the audio thread:** only the engines and the crunch of the ground (`sound/engine.worklet.ts`).

`build_sfx.py` takes each source as an optional flag (`--firearms`, `--explosions`, `--impacts`, `--water`, `--breaking`) and writes only the sounds made from those given. A splash, a shatter or a fall is cut whole from its first onset (`whole`), not at its next transient as a gunshot is. The big splashes and the collapses are slowed and layered over their own low end (`deepen`) for weight. The script needs numpy and soundfile, which a throwaway virtualenv provides.

## The engine

`sound/enginesynth.ts` is the `EngineSynth`, pure numbers with no Web Audio in it:

- **Crank.** The crank turns at `rpm`. Each cylinder fires at its point in the cycle: every other turn for a four-stroke, every turn for a two-stroke.
- **Firing.** A firing is a pressure pulse as strong as the throttle (`load`). It rises in a fraction of a millisecond and dies away over `pulse` ms, with the burn's roughness on it.
  - `imbalance` gives each cylinder a fixed strength of its own, so a V8 burbles below its note.
  - `jitter` varies each firing from the last.
  - `knock` adds a diesel's hard click.
- **Exhaust.** The pulses go down an exhaust pipe `pipe` metres long. The reflection from its open end comes back inverted, so the pipe rings at its odd modes, as strongly as `resonance` lets it. The result is muffled by a low-pass that opens with the revs and the throttle.
- **On top.** The intake's roar comes in time with the strokes, and `mech` adds the valves' tick.
- **Off the throttle.** On the overrun, high in the revs, firings miss and fuel `crackle`s in the pipe.
- **Pistonless layers.** `whine` is a turbo's, a turbine's or an electric motor's, at `whineRatio` times the crank's speed. `roar` is a turbine's exhaust or a rocket.

`kinds/engines.ts` holds the engines as `SynthSpec`s:

| Engine | Kinds | What it is |
|---|---|---|
| `METEOR_V12` | tank | The Centurion's 27-litre petrol V12: deep, smooth, slow to rev, long pipes, crackle off the throttle |
| `DIESEL_AND_TURBINE` | Strv S | The Strv 103's six-cylinder diesel with its gas turbine whining over it |
| `DIESEL_V8` | Strf 90 | A big turbo V8 diesel: lumpy, with the turbo's whistle |
| `DIESEL_SIX` | Pbv 302, AMV (with more turbo) | A six-cylinder truck diesel |
| `BUS_DIESEL` | bus | Lower and softer, with a turbo sigh |
| `DIESEL_FOUR` | technical | A rough four-cylinder diesel that knocks at idle |
| `PETROL_SIX` | jeep | A smooth straight six |
| `PETROL_V8` | sports car | A cross-plane V8: burble, bark, crackles |
| `TWO_STROKE_TWIN` | snowmobile | The "ring-ding" of a tuned twin |
| `TWO_STROKE_SINGLE` | snowracer | A lawnmower's single |
| `PUSHER_TWO_STROKE` | loitering drone | A single with its propeller's buzz |
| `ROTARY_NINE` | triplane | Nine spinning cylinders, no pipes, rattling |
| `RADIAL_SEVEN` | seaplane | A deeper, steadier radial |
| `TURBOSHAFT` | helicopter | A turbine's whine and roar |
| `DRONE_MOTORS` | FPV drone | Four small motors |
| `ROCKET` | guided missile | All roar |
| `ELECTRIC`, `SMALL_ELECTRIC` | Roadster, golf cart | A motor's whine, rising with the speed |
| `SUB_MOTORS` | submarine | A low hum |
| `TWIN_DIESEL_JETS` | Stridsbåt 90 | Two V8 diesels (sixteen cylinders) and the waterjets' roar |

`sound/engine.worklet.ts` runs an `EngineSynth` on the audio thread as the `engine` processor, with `rpm` and `load` as parameters. It also runs a `GritSynth` as the `grit` processor. `sound/worklet.ts` loads them once per context: Vite bundles the worklet through a `?worker&url` import. Until they have loaded, or in a browser without audio worklets, the engine and the crunch are silent and everything else plays.

## The revs

A kind's `sound.engine` (`Motor` in `sim/engine.ts`) has the engine's `synth`, its `idle` and `redline` rpm, how many seconds it takes to `spin` up out of gear, and how its revs follow (`name`):

- **`gears`.** An automatic gearbox with `shifts`.
  - It lifts off for 0.22 s at each upshift, and the revs fall to the next gear's.
  - Pulling away, the clutch slips, so the revs are the throttle's until the car is a third into first gear.
  - With the wheels spinning or in the air, the engine runs free.
  - Held flat at the redline, the limiter cuts it in a stutter.
- **`cvt`.** A belt drive (snowmobile, snowracer). With the throttle open, the revs leap to where the belt grips and hold there as the speed builds. Let go, they fall back to idle.
- **`electric`.** The revs are in step with the wheels, so it is silent at a standstill.
- **`marine`.** A boat's propeller or waterjet (the submarine, the Stridsbåt 90). The throttle sets the revs, and the speed through the water hardly changes them. A hull rocking on the waves would otherwise wobble the note.
- **`rotary`.** A plane: the revs follow the throttle, whose middle is cruise, and the airspeed. The blip switch cuts the load.
- **`rotor`.** A governed turbine. It spools up when the pilot gets in, then holds its speed while the collective changes how hard it works. The blades beat with the rotor's real speed (`Thrust.air`, `SoundInput.rotor`), so a rotor spinning up, drooping, spinning down or autorotating is heard with no engine running.
- **`props`, `flat`.** Drones work harder to climb. A loitering drone and a missile are flat out.

`Revs` follows one engine from frame to frame. `VehicleSounds` keeps one per vehicle in earshot, so a vehicle that comes into the nearest six is heard where its revs are.

The throttle is the driver's. Your own vehicle uses your newest input (`VehicleDriver.throttle`), with no lag. Everyone else's comes on the wire as `Steer.throttle` (protocol 89, [NETWORKING.md](NETWORKING.md)), so the tank revving at a standstill is heard by everyone. A helicopter sends its collective there. Pulling the stick against the way a vehicle rolls is the brake, not the gas (`gasOf`).

## The ground

`sim/groundsound.ts` hears a `SURFACE_*` as a `Ground`:

- `tarmac`.
- `gravel`: dirt is the gravel of tracks and farm roads.
- `grass`, `sand` and `mud` (the marsh).
- `snow`: in a winter world, everything off the ploughed roads.
- `ice`: frozen marsh, and frozen lakes when the world's water is iced.
- `water`.

`VehicleFx.ground` is what the wheels last touched, or `water` while a hull sits in open water. A vehicle afloat counts as on the ground, not flying, so a plane down in the water hears the water and not the wind in its wires. `VehicleFx.submerged` says it is wholly under the surface: a dived submarine hears only a low rush of water past it, with no waves to slap it.

`rolling(ground, speed, slip, tracked)` gives the layers for each ground:

- **Roll.** A low-pass roar.
- **Grit.** The crunch, made by the `grit` processor (`sound/gritsynth.ts`) as short filtered grains at random moments:
  - gravel: many bright grains;
  - snow: squeaky ones;
  - mud: slow, low, wet ones;
  - water: a wave slapping the hull a few times a second, under the wash along it.

  Spinning wheels churn more grains. A tracked vehicle's crunch is heavier and lower.
- **Squeal.** How much of a skid squeals: all of it on tarmac, a scrub on gravel, nothing on ice or in water.
- **Clank.** How a track's clatter carries: ringing on tarmac and ice, dull on soil and snow, nothing afloat.

A tank turning on the spot squeals its tracks. The layers ease from one ground to the next over a tenth of a second or so, so a road's edge does not click.

## Footsteps

The soldier's walk and run clips put each foot down at a quarter and three quarters of their cycle (`gait` in `build_character.py`). `Character` notes each footfall as its cycle passes those points (`takeSteps`). There are none on skis, swimming, in the air, down or getting up.

- **Weight.** A step's weight is 0.25 crouched, 0.5 walking and 1 running. A zombie drags its feet down twice a shamble at 0.7.
- **Footing.** `Surroundings.stepGround` says what the foot lands on: water over the ankles is a wade, a foot well above the terrain is on a floor (a house's storey, a deck), and otherwise it is the ground.
- **The sound.** `VehicleSounds.step` plays the heel's thump and what it lands on:
  - a scuff on tarmac;
  - a crunch of grains on gravel and snow;
  - a swish on grass and sand;
  - a squelch in mud;
  - a click on ice;
  - a splash in water;
  - a hollow knock on a floor.

  Every step is a little different, and kit rattles at a sprint.
- **Range.** Your own steps are heard in your head. Other people's are heard within 40 m, and never take the last five of the fourteen one-shot slots, so gunfire is not crowded out.

## Launches and flight

The server sends no shot for a helicopter's rocket pods, a plane's bomb rack or a thrown grenade, so the client hears them leave when a new projectile first appears (`Projectiles.update`, `VehicleSounds.launch`):

- a rocket lights with a rush and a thump;
- a bomb comes off its rack with a clunk;
- a grenade is a pin's click and the swish of the throw.

A guided missile is a vehicle, and is heard leaving when it is first drawn: a Titan's shoulder tubes, and a submarine's, which also blast.

In flight, the four projectiles nearest the listener within 200 m each have a voice (`FlightVoice`): noise through a band-pass, with Doppler from how fast it closes.

- **A rocket's motor** roars.
- **A shell, a cannon round or a mortar bomb** whistles, loud as it comes and faint once past.
- **A dropped bomb's** whistle falls as it comes down.

Someone else's round that passes within 6 m of the listener cracks once (`Events.drawBullets` tests the segment it flew each frame, since a round covers 10–15 m a frame).

## Water

Every splash, whatever made it, goes through `render/splash.ts`, which tells `FxWorld.heard`. So there is one sound for all of them:

- a vehicle going in (`VehicleFx.splashIn`: the tank off the bridge);
- a soldier jumping in (`Surroundings.wade`);
- rounds into water and blasts on it (`watery`);
- a span of bridge into the river (`BuildingDamage.splashed`).

`VehicleSounds.splash` picks by the splash's size:

- under 0.35 m: a quiet `splash_small` that gives way like a step;
- under 0.8 m: `splash_small`, louder (a soldier);
- bigger: `splash_big` at full level, deeper the bigger it is. Every vehicle going in is at least this big.

A splash is one sharp click over a quieter body of water. Normalised by its peak, as the guns are, the body was barely heard, so `build_sfx.py` normalises splashes, glass, debris and collapses by their loudest 50 ms instead (`loud`), with a soft limit on the click.

## Impacts and destruction

- **Rounds striking.** A round, a pellet or a machine-gun round that strikes something heard within 60 m plays `hit_stone` off a road or anything standing above the ground (a wall, a rock), and `hit_dirt` on open ground (`Events.struck`). Trees are not told apart yet.
- **Glass.** `Glazing` tells `heard` when panes break: one for a round, and one for a blast however many panes went, louder for more.
- **Buildings.** `BuildingDamage.landed` is told when a piece comes down on the ground, and when a section of wall slams down flat after toppling. Small pieces clatter (`debris`). A piece 2 m or more across crashes (`collapse` and a thud). Big landings within 12 m and 0.6 s of each other are one, so a house coming down is a few crashes in a rumble, not one for each piece.
- **Vehicle parts.** `Debris.landed` is told when a part strikes the ground harder than 4 m/s. A shot-off turret thuds, and smaller parts clatter.

Footsteps, impacts, cracks and small splashes never take the last five of the fourteen one-shot slots, so a blast is always heard.

## Far sounds, and the bus

Everything heard in the world goes through one bus (`audioBus` in `audio.ts`): the vehicle sounds, the one-shots, the ambience and the rooster. The bus is a muffle (a low-pass and a level) and one compressor. Music and voice chat bypass it: neither is a sound of the place you are in, and a teammate must be understood from inside a tank.

- **Muffled.** `setMuffle` dulls the world to 1.4 kHz at 70% in a closed seat (a `SeatView` marked `enclosed`: the tanks, the Strf, the Pbv and the AMV, the submarine, the helicopter, the Titan, the bus and the sports car) and to 380 Hz at 40% with the camera under water. The listener's own engine, their hits, hurts and door, and the hiss of their skis go to the bus's `dry` side and are never dulled. A vehicle the listener gets into or out of has its voice made again on the right side.
- **Late and dull.** Every positioned one-shot arrives as late as sound takes from it (343 m/s, no cap) through a low-pass that air closes with distance (`airCut`: 18 kHz close by, 1.6 kHz at 500 m, 350 Hz at the floor). This holds for the synthesised sounds as much as the recorded ones.
- **Carried far.** Guns and machine guns (800 m), small blasts (1.1 km), cannon and vehicles blowing up (1.6 km) carry past the 160 m everything else stops at, on a panner that falls away more slowly. Part of each goes to an echo off the land (a `ConvolverNode` over a made impulse: a slap off the nearest slope, a later one and a darkening roll), more of it the farther the shot (`echoLevel`), so a fight a kilometre off is heard as late, dull rolling cracks. A shot from beyond 160 m leaves six of the fourteen slots free for nearer sounds.

## Ambience

`ambience.ts` is the sound of the world when nobody is shooting. All of it is made from noise and tones, with nothing to load. The game tells it what is about the listener (`Surround`) every frame, and looks round for the costly parts every quarter of a second (`Game.hearSurroundings`):

- the time of day, cloud and rain, from the sky and the weather;
- how wooded it is: standing trees within 30 m, 25 of them making deep forest (`WorldView.treesNear`);
- how near water is, and whether it runs: the water level looked up in rings out to 50 m, and the current where it was found (`Game.nearestWater`). Still water that has frozen over is silent;
- the height above the ground, and whether the soldier is under a roof (the ray up from the head that `roomArm` uses).

What is heard:

| Layer | What it is | When |
|---|---|---|
| Wind | brown noise, its low-pass opening in gusts | always; stronger under cloud, in rain and higher up, less among trees and indoors |
| Leaves | a hiss that brightens in gusts | with the wind, among trees |
| Water | lapping in slow swells, or a steady brighter run for a stream | within 50 m of water |
| Rain | a wide hiss outdoors, a dull drumming on the roof under one | rain |
| Crickets | a made loop of a few chirping crickets in each ear | summer nights, in the open more than in the forest |
| Birds | a falling trill with a flourish, a two-note whistle or a warble, 12 to 55 m off and up in the trees | by day, most at the dawn chorus and again at dusk, more in the forest, few in winter, none in rain |
| Gulls | a few harsh falling cries | by still water by day |
| Owls | a tawny owl's "hoo, hu-hoooo" | at night in the forest |
| Thunder | a crack when near, then a long low roll in swells | each lightning strike, as long after the flash as sound takes |

Each layer runs only while it is heard, and stops its source after five silent seconds, so a quiet scene runs nothing. Under a roof everything outside is dulled to 1.5 kHz.

**Thunder** is the same for every player. Lightning strikes in the weather's slots (`strikeIn` in `render/weather.ts`), and each strike has a distance hashed from its slot (`strikeDistance`, 0.4 to 6 km, most of them far). Its thunder is sent on its way as the flash is seen, and is heard `distance / 343` seconds later. A strike more than two seconds past its thunder (a player who has just joined) is let go.

## Listening without the game

```sh
cd client && ENGINE_WAVS=/some/dir npx vitest run src/__tests__/engine.render.spec.ts --silent=false
```

This writes a WAV for every engine, plus `grounds.wav`. Each engine idles, is blipped twice standing, pulls away flat out through its gears and coasts. Aircraft take off, cruise and blip the switch. `grounds.wav` is a car at 15 m/s over each ground for two seconds. The run prints each file's peak, its loudness second by second and where its energy sits, which is a quick check that nothing is silent, clipped or out of balance.

## Tests

- `enginesynth.spec.ts`:
  - a V12, a two-stroke twin and a diesel four fire at the rates their cylinders and revs give (by autocorrelation);
  - a twin is brighter than a V12;
  - pulling is louder than coasting;
  - a seed repeats exactly;
  - nothing clips or goes NaN from standstill to past the redline;
  - a crackling V8 pops on the overrun;
  - grit is silent without grains and stable at its edges.
- `engine.spec.ts`:
  - `Revs`: idle and blips, four shifts with a lift at each, free revs on spin and in the air, the limiter, the snowmobile's belt, a tank revving slower than a sports car, running down, and the brake;
  - a boat's revs holding steady while the waves rock its speed;
  - the mix: damage, the helicopter's collective, and a tank's tracks squealing and clanking by the ground;
  - water: a hull washing with only an occasional slap, a dived submarine hearing no waves, and a plane afloat hearing the water;
  - every ground sounding different, and winter's snow and ice.
- `vehicleaudio.spec.ts` checks the graph on a fake context:
  - a running engine gets its `engine` and `grit` nodes, which are told to stop when it goes;
  - a kind with no engine gets only the grit.
- `character.spec.ts`: two footfalls a stride, harder running than walking, and none standing, in the air, on skis or swimming.
- `replicate/tests.rs`: `everyone_is_told_how_hard_a_driver_works_the_throttle`.
- `vehicleaudio.spec.ts` (launches and the rest):
  - at most four projectiles heard in flight, each let go when it ends, and none for a grenade or one out of range;
  - splashes, impacts, glass, cracks and launches only within earshot;
  - the small sounds giving way to a blast;
  - a building's crashes merging.
- `presenters.spec.ts`: a new projectile heard leaving once, in flight each frame, and let go when it ends.
- `destruction.spec.ts`: every falling piece heard landing once.
- `glass.spec.ts`: a blast heard once, with the panes it broke.
- `sfx.spec.ts`: every sample `SAMPLES` names is in `public/sfx`.
- `vehicleaudio.spec.ts` (far sounds): a gunshot 500 m off heard late and dull and mostly echo, blasts carried far while crashes and steps stop close by, and far shots leaving slots for near ones.
- `ambience.spec.ts`: birds at dawn more than at noon and none at night, rain and winter quietening them, crickets and owls at night and no crickets in winter, rain drumming on the roof indoors, wind harder in a storm and higher up, leaves only among trees, water and gulls only near still water, only heard beds running, birds singing in a wood at dawn, and thunder following its flash by its distance, once, and only in a storm.

The fake Web Audio context these share is `__tests__/fakeaudio.ts`.

## Not done, or not checked

- Not yet heard in the game. The engines have been rendered to WAVs and measured, and the worklet has rendered in headless Chrome, but levels and tone are first guesses to tune by ear.
- The mech walker's, hen's and Titan's feet keep their own clanks and thuds, with no ground under them.
- The ambience has not been heard in the game: its levels and its birds are first guesses to tune by ear. It has run in headless Chrome. There are no recordings for it yet; `sfx.ts` could take them as it does the guns'.
- Small actions are silent: reloading, an empty click, a weapon switch, a pickup, a parachute opening, gates, and other players getting into vehicles.
- Deaths, gibs and knockdowns make no sound of their own.
- A round striking a tree is heard as earth.
- The new recordings were chosen by their length, loudness and brightness, not by ear.
