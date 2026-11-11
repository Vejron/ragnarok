# Zombies

At night, hordes of zombies rise in a world's zombie areas and shamble after the living. A zombie is a soldier's body with empty hands and no mind but hunger. Each one can be shot apart limb by limb, as a soldier can:

- It loses arms and legs.
- With no legs it crawls on.
- With no arms it bites instead of clawing.
- Only the head or the torso kills it.

## Setting it up

A world's `scenario.json` turns zombies on with a `zombies` block. It names the scenario's areas they rise in:

```json
{
  "areas": { "woods": { "lat": 63.6572512, "lon": 20.1312598, "radius": 80 } },
  "zombies": { "areas": ["woods"], "max": 30, "horde": 6, "every": 45, "day": false }
}
```

| Field | Default | What |
|---|---|---|
| `areas` | (required) | The areas they rise in, by name. Each horde rises in one of them, picked at random. |
| `max` | 30 | The most zombies up at once. `MAX_ZOMBIES` (96) caps every room whatever this says. |
| `horde` | 6 | How many rise together. |
| `every` | 45 | Seconds between hordes. |
| `day` | false | Whether they rise by day as well. Without it, those still up at dawn fall where they stand, a couple per tick. |

In the map editor, the Scenario panel has a **Zombies** section:
- a switch to turn them on;
- the areas they rise in, picked from the scenario's areas;
- the most up at once, the horde size, seconds between hordes, and whether they rise by day too.

Zombie areas are drawn on the map in green with a dashed edge, labelled 🧟. Renaming an area renames it for the zombies, and an area they use cannot be removed. The editor checks the block as the server does (`client/src/editor/scenario.ts`), and a typo that gets past it still stops the server with a message, as anything else in a scenario does.

"Night" is the same clock the sky and the bots run on (`bots::darkness`, darker than 0.5). A horde rises at a spot in the area on open ground and at least 35 m from anyone living, so nobody sees it climb out of the ground. Its zombies stand within 4 m of that spot. When no spot is clear, that horde is skipped.

## How a zombie behaves

`crates/server/src/mechanics/zombies.rs`. A zombie is a `Player` with a `Zombie` mind, on `TEAM_ZOMBIES`. Like a bot, its input is written each tick, so walking, falling, wounds, knockdown and ragdolls need nothing new.

- **Smell.** About four times a second it picks the nearest living soldier on foot within `ZOMBIE_SMELL_RANGE` (80 m). It needs no line of sight. Someone in a vehicle is out of its reach and of no interest.
- **Walking.** It walks the way `navigation` finds, as bots do (`bots::follow`), and keeps apart from the rest of its horde (`bots::push`). Each zombie has a shambling pace of its own (`ZOMBIE_PACE`). Within `ZOMBIE_LUNGE` (5 m) it lunges at full walking pace. One in `ZOMBIE_RUNNER_ONE_IN` (8) runs.
- **Claw.** Within `ZOMBIE_CLAW_REACH` (1.4 m), facing its prey, it reaches back for `ZOMBIE_CLAW_WINDUP` ticks and then strikes for `ZOMBIE_CLAW_DAMAGE` (22) to the torso. The blow only lands if the prey is still within reach plus 0.4 m, so stepping back as it winds up is not quite enough. After the blow it recovers for `ZOMBIE_CLAW_RECOVER` ticks. With both arms gone it bites for `ZOMBIE_BITE_DAMAGE` (14).
- **Noise.** With nobody to smell, it goes to the nearest gunshot or blast it heard (the same `Noise` the bots and deer hear).
- **Being hurt.** A zombie that is hurt turns on whoever hurt it.
- **Its own side.** Zombies never hurt each other. Bots are on another team, so bots and zombies fight.

The scenario (`crates/server/src/scenario/horde.rs`) keeps a list of its zombies. A dead one lies as long as any body does, and then it leaves, as a squad member that does not respawn leaves. When the scenario starts again, they are all removed.

## On the wire

Nothing new in the format; `PROTOCOL_VERSION` 73 marks the new values.

- `kind` is `KIND_ZOMBIE` (24) instead of `KIND_SOLDIER` for a zombie on foot. Everything else about it is a soldier's: limbs, `struck`, stance and so on.
- Bit 7 of `flags`, the rooster's crowing bit, is `FLAG_CLAWING` for a zombie: it is set from the start of the wind-up to the blow.
- A claw's harm is `WEAPON_CLAW` (34).
- A zombie's profile is on team `TEAM_ZOMBIES` (255) and marked as a bot. The client keeps zombies off the scoreboard and leaves their deaths out of the kill feed. Their kills are shown.

## On the client

- **Look.** `Soldiers` (`entities/presenters/people.ts`) draws a zombie with the soldier's model. `Character.zombie` paints the outfit in rags (`ZOMBIE_RAGS`) and tints the skin and boots grey-green with a copy of the `gear` material. A zombie keeps its rags whatever colour its profile names.
- **Clips.** `tools/character/build_character.py` bakes three clips:
  - `zombie_idle`: hunched and swaying, arms half up.
  - `zombie_walk`: a shamble with the right leg dragged and the arms out in front, paced by `ZOMBIE_WALK_SPEED` (1.2 m/s).
  - `zombie_claw`: the right arm up and back, then raked down across the front.

  `Character.zombieTargets` gives them the weight the stance's idle, walk and run clips would have had. The claw plays out once over `ZOMBIE_CLAW_TIME` each time the flag comes on. Crawling, falling, dying and getting up are the soldier's own clips.
- **Sound.** `ZombieVoices` (`client/src/zombievoices.ts`) makes each zombie groan every 4 to 11 seconds, each at a pitch of its own, and growl as each claw starts. `VehicleSounds.groan` and `claw` synthesise them (a wavering sawtooth through an "uh" formant, with a breathy rasp), placed in the world like every other sound.

## Not done yet

- Zombies do not climb, break doors or go after people in vehicles.
- Only a zombie's own clips are drawn standing and walking. A one-legged zombie hops in the soldier's hop, with the zombie walk's arms.
- The clips have only been looked at in still frames; how they move is for playing to judge.
