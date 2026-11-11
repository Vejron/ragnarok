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

"Night" is the same clock the sky and the bots run on (`bots::darkness`, darker than 0.5). A horde rises at a spot in the area on open ground and at least 35 m from anyone living. Its zombies come up within 4 m of that spot. When no spot is clear, that horde is skipped.

## Rising from the grave

A horde does not come up all at once. Its first zombie comes up straight away, and the rest one by one over the next `ZOMBIE_RISE_STAGGER` (10 s), each at a random moment of its own (`scenario/horde.rs` keeps the graves still to open in `Progress::graves`). Graves still to open count towards `max`, and at dawn or when the scenario ends they are filled in.

Each zombie then takes `ZOMBIE_RISE_TICKS` (6 s) to climb out (`Zombie::rising`). Meanwhile it stands where it is: it smells, hears and claws at nothing, and does not walk. It can be shot all the same, and one hurt while climbing still turns on whoever hurt it once it is out. Its body is a soldier's standing capsule from the start, so a shot at the air where its chest will be hits it.

The snapshot shows it in `STANCE_RISE` (7) for as long as it climbs. The client plays `zombie_rise` over the same 6 s from when it first sees that stance, and drops into the zombie's own clips when the stance ends. The clip carries the body down into the ground and the ground hides what is still buried: the right hand punches up through the soil and claws at the air, then the left, the head breaks the surface, it plants both hands and hauls itself out, slips back, heaves again, gets a knee up onto the ground and stands, shaking with the effort the whole way. No contact shadow or splashes are drawn while it climbs.

## How a zombie behaves

`crates/server/src/mechanics/zombies.rs`. A zombie is a `Player` with a `Zombie` mind, on `TEAM_ZOMBIES`. Like a bot, its input is written each tick, so walking, falling, wounds, knockdown and ragdolls need nothing new.

- **Smell.** About four times a second it picks the nearest living soldier on foot within `ZOMBIE_SMELL_RANGE` (80 m). It needs no line of sight. Someone in a vehicle is out of its reach and of no interest.
- **Walking.** It walks the way `navigation` finds, as bots do (`bots::follow`), and keeps apart from the rest of its horde (`bots::push`). To a goal further off than `SEARCH_WITHIN` (60 m) it looks for no way and heads straight for it, round what is in front of it: a search reaches about 40 m, so one for a blast heard 150 m off found nothing at full cost, for every zombie that heard it. On `bussjobanan`, 20 zombies drawn to a blast a second cost a debug server 7.3 ms a tick with one tick in five late (worst 80 ms), and 1.5 ms with none late since (`horde_tick_cost` in `zombies/tests.rs` measures it). Each zombie has a shambling pace of its own (`ZOMBIE_PACE`). Within `ZOMBIE_LUNGE` (5 m) it lunges at full walking pace. One in `ZOMBIE_RUNNER_ONE_IN` (8) runs.
- **Claw.** Within `ZOMBIE_CLAW_REACH` (1.4 m), facing its prey, it reaches back for `ZOMBIE_CLAW_WINDUP` ticks and then strikes for `ZOMBIE_CLAW_DAMAGE` (22) to the torso. The blow only lands if the prey is still within reach plus 0.4 m, so stepping back as it winds up is not quite enough. After the blow it recovers for `ZOMBIE_CLAW_RECOVER` ticks. With both arms gone it bites for `ZOMBIE_BITE_DAMAGE` (14).
- **Noise.** With nobody to smell, it goes to the nearest gunshot or blast it heard (the same `Noise` the bots and deer hear).
- **Being hurt.** A zombie that is hurt turns on whoever hurt it.
- **Its own side.** Zombies never hurt each other. Bots are on another team, so bots and zombies fight.

The scenario (`crates/server/src/scenario/horde.rs`) keeps a list of its zombies. A dead one lies as long as any body does, and then it leaves, as a squad member that does not respawn leaves. When the scenario starts again, they are all removed.

## On the wire

Nothing new in the format; `PROTOCOL_VERSION` 73 marks the new values, and 74 the `STANCE_RISE` stance.

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
  - `zombie_rise`: once, out of the grave (above). Its preview shots draw the ground as a slab (`--only zombie_rise`).

  `Character.zombieTargets` gives them the weight the stance's idle, walk and run clips would have had. The claw plays out once over `ZOMBIE_CLAW_TIME` each time the flag comes on. Crawling, falling, dying and getting up are the soldier's own clips.
- **Sound.** `ZombieVoices` (`client/src/zombievoices.ts`) makes each zombie groan every 4 to 11 seconds, each at a pitch of its own, and growl as each claw starts. `VehicleSounds.groan` and `claw` synthesise them (a wavering sawtooth through an "uh" formant, with a breathy rasp), placed in the world like every other sound.

## Not done yet

- Zombies do not climb, break doors or go after people in vehicles.
- Only a zombie's own clips are drawn standing and walking. A one-legged zombie hops in the soldier's hop, with the zombie walk's arms.
- The clips have only been looked at in still frames; how they move is for playing to judge.
- A client that first sees a zombie halfway out of the ground plays the climb from the start, and it snaps up when the server says it is out.
- No soil is thrown up as it climbs.
