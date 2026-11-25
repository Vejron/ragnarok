# Character looks

Everyone on foot is drawn from one model, `client/public/models/player.glb`: thirteen bodies on one
skeleton, sharing every clip. The bodies are Quaternius' Animated Men and Animated Women (CC0),
recoloured into the game's looks:

| Look | Bodies (picked by id) | Dressed as |
|---|---|---|
| `soldier` | `body_soldier_m`, `body_soldier_f` | Olive uniform, helmet with goggles, vest with pouches, backpack. The player's colour is on the helmet band and both armbands. |
| `officer` (`KIND_OFFICER`) | `body_officer_m`, `body_officer_f` | Navy, a yellow high-visibility vest with reflective bands, a peaked cap with a chequered band. |
| `medic` (`KIND_MEDIC`) | `body_medic_m`, `body_medic_f` | Yellow-green high-visibility top with a reflective band and green shoulders, green trousers. |
| `civilian` (`KIND_CIVILIAN`, zombies) | `body_civilian_man`, `_sleeves`, `_suit`, `_woman`, `_casual`, `_dress`, `_tanktop` | As the packs dress them, the top tinted per person. A zombie wears one in rags. |

The client picks the look from the entity's `Kind` (`lookOf` in `render/character.ts`): on foot in
`people.ts`, in a seat in `riders.ts`. The local player is a soldier. A person's body, top colour and
skin tone are picked from their id (`pick`), so they look the same every time and to everyone.

## Building it

```sh
python3 tools/character/fetch_packs.py      # once: the packs into .cache/characters/ (git-ignored)
Blender -b -P tools/character/build_character.py [-- --out x.glb --preview DIR --only soldier_m --strip walk_fwd]
```

The build also needs Quaternius' Universal Animation Library 1 and 2, the free "Standard" tier
(CC0). itch.io will not hand them to a script, so download both zips by hand from
[UAL 1](https://quaternius.itch.io/universal-animation-library) and
[UAL 2](https://quaternius.itch.io/universal-animation-library-2) into `.cache/characters/ual/`,
named as itch.io names them. `library.py` reads the clips straight out of the zips.

`build_character.py` runs in three stages:

1. **The source rig.** The game's own rig, with its procedural clips posed in code, as it always
   was (the animate-soldier skill). Its body (`bodies.py`) is not drawn; it settles the lying clips
   on the ground.
2. **The people** (`characters.py`):
   - Each pack model is imported and scaled to 1.72 m, and its bones are renamed to the game's
     (`BONE_MAP`). A foot hangs from its shin and turns at the ankle; the packs hang it from the root.
   - Every body is bound to the men's skeleton. The women's differs by millimetres at the same height.
   - The packs' flat materials become vertex colours in the game's three materials, `outfit`, `gear`
     and `skin`, per the body's paint table in `BODIES`.
   - Uniforms recolour the clothes and add a helmet, vests and caps. These are shaped to the body's
     own cross-sections (`shell`, `section`), so they fit whichever model wears them.
3. **The clips** (`retarget.py`):
   - Every source clip is moved onto the people's skeleton. A bone turns as its source turns away
     from a pose both rigs share (the T-pose: the people stand in it at rest, the source rig is posed
     into it).
   - The hips move as the source's, scaled by leg length (`legs x0.85`).
   - Where the hands hold something, the arms are then solved onto where the source's hands are
     (scaled by arm length), so guns stay in both hands.
   - The Universal Animation Library's hand-keyed clips replace ours where it has one
     (`FROM_LIBRARY`, `library.py`):

     | Ours | The library's | Speed |
     |---|---|---|
     | `idle` | `Idle_Loop` | |
     | `walk_fwd` (the game's 4 m/s walk) | `Jog_Fwd_Loop` | 4.39 m/s |
     | `run_fwd` | `Sprint_Loop` | 6.77 m/s |
     | `crouch_idle`, `crouch_fwd` | `Crouch_Idle_Loop`, `Crouch_Fwd_Loop` | 0.62 m/s, a deep sneak played about three times over at 2 m/s |
     | `zombie_idle`, `zombie_walk` | `Zombie_Idle_Loop`, `Zombie_Walk_Fwd_Loop` | 0.86 m/s |
     | `jump_air` | `Jump_Loop` | |
     | `jump_land` (new: played once when a fall faster than 3 m/s lands standing still; on the move the crouch dip stays) | `Jump_Land` | |
     | `sit` | `Sitting_Idle_Loop`, its hips on ours | |
     | `swim_tread`, `swim_fwd` | `Swim_Idle_Loop`, `Swim_Fwd_Loop` (a front crawl), their heads on ours | 2.38 m a stroke |
     | `getup_back` | `LayToIdle`, played once | |

     The free tier has no strafes, backwards steps, crawl, zombie attack, getting up off the belly or
     diving, so those stay ours.
     With empty hands, and for a zombie, the clip is the library's whole. With something held, the
     library's hips and legs carry the stance's own upper body, turned in the world as the stance's
     clip turns it (`fixed` in `retarget.py`), so the gun stays level and in both hands however far
     the library's trunk leans. Stepping cycles are turned to start where ours do, so they blend in
     step with our strafes.
   - Some are fitted to where the client expects the body (the `fit` in `FROM_LIBRARY`). The
     client puts a seated body by its head and a swimmer by the water line. So the library's
     sitter is moved until its hips sit where ours do, and its swimmers until their heads ride
     where ours do: the head out of the water, 1.3 m over the origin. The build prints how far
     each moved. A seat keeps our upper body even with empty hands (`upright`), since the
     library's sitter leans its head 0.2 m forward of where the seat puts it.
   - A clip the library has and we have not (`jump_land`) takes its held upper body from the
     stance's idle (`upper`).
   - The library's speeds come from its root-motion clips (`Library.travel`), scaled by leg length.
     The build prints them as `SPEED ... (the library's)`.
   - The people's limbs are not the source rig's, so a body the source laid on the ground floats over
     it or sinks into it once retargeted (an armed crawl, propped on the elbows, by 0.3 m). The build
     measures the soldier's posed body and puts these clips back on the ground (`on_the_ground`,
     printed as `GROUND ...`). Each crawl is moved as a whole, so the lowest point of its cycle touches
     the ground; getting up and the slide are raised wherever they would go through it. A new clip
     that lies on the ground goes in that list.
   - The guns are made where the source's hand holds them and moved with the wrist onto the people's
     right hand, with the muzzle bones.

The clips come in five stances: `rifle` (no prefix), `pistol_`, `hold_`, `shoulder_`, and `free_` for
empty hands. With empty hands the arms hang and swing: the library's own arms in its clips, a swing
against the legs in ours. The client plays `free` when nothing is held.

`--preview DIR` renders every body standing and in the clips that bend it furthest (run, crouch, sit,
getting out of a car, crawling, a zombie's walk and claw, treading water), a gun in hand.
`--strip walk_fwd,zombie_walk` renders those clips at eight phases instead, from the side and at
three-quarters, for judging a cycle.
`--preview-source DIR` renders the source rig's own clips, for working on one.

## Numbers the client takes from the build

- `SIT_HEAD` and `STAND_HEAD` come from the build's `HEAD sit` and `HEAD idle`, plus 0.1 m from the
  head bone to the head's middle.
- `SIT_HEAD_BACK`: seated, the head sits 5.7 cm behind the model's origin.
- `SPEED_OF`: `walk_fwd`, `run_fwd` and `crouch_fwd` are the library's, as the build prints them
  (`SPEED ...`). The procedural clips' speeds are the old ones × 0.85.
- `ZOMBIE_WALK_SPEED` is `SPEED zombie_walk`, and `SWIM_STROKE` is `STROKE swim_fwd`.

## Materials

Three, each a character's own copy:

- `outfit`: white shades, tinted per look. That is the player's colour on a soldier's patches, a
  civilian's top from `CIVILIAN_TOPS`, white on a uniform, rags on a zombie.
- `gear`: the final colours, dulled on a zombie.
- `skin`: white, tinted with a tone from `SKIN_TONES`, or grey-green on a zombie.

A hit's flash whitens all three.

## Adding a body or a look

1. Name the model in `fetch_packs.py`, and add a row to `BODIES` in `characters.py`: its file, which
   of its materials are `outfit`, `skin` or a new `gear` colour, and which accessories it wears.
2. Add the body to the look's list in `BODIES_OF` (`render/character.ts`) and in `character.spec.ts`.
3. Rebuild, and look at `--preview DIR --only <body>`.

## After the clips

What moves the bones after the clips (feet on the slope, the left hand on the gun) is in
[ANIMATION.md](ANIMATION.md). It needs the grip bones the build makes (`GRIPS`): `grip`, `grip_pistol`,
`grip_hold` and `grip_shoulder` on the right hand, where each stance's idle lays the left hand.

## Still to come

- **More library clips:** `Jump_Start` (the game's jump leaves the ground at once, so a wind-up
  would lag it), `Driving_Loop` (the client does not know who drives, nor where each car's wheel
  is) and the paid tiers' strafes and crawl.
