# Writing a scenario

A scenario is a small story told in a world: enemies appear, you get an objective, and you win or lose. A scenario is a `scenario.json` next to the world's `area.json`. You can write it by hand, or edit it in the map editor's **Scenario** mode (see [EDITOR.md](EDITOR.md)). This page is a guide to the file and its parts. [BOTS.md](BOTS.md#scenarios) has the full reference for every field, condition and action.

## 1. Pick the world and the places

- The file goes in the active world's folder, `worlds/<name>/scenario.json`. The active world is in `worlds/active.json` (the editor's **use** button sets it), or whatever `RAGNAROK_WORLD` points at. A scenario belongs to one world: positions are real latitude and longitude, so it does not carry over to another.
- The easiest way to get coordinates is the map editor. In Scenario mode, place areas, bots, tanks and their routes on the map and the editor writes their latitude and longitude for you. To write the file by hand, place a Spawn on the map where you want something, read its latitude and longitude out of `area.json`, and copy them. Then delete the placeholder if it should not be there all the time.
- Everything must be inside the painted tiles. A position outside the world stops the server at start-up with a message saying where.

## 2. Write the pieces

A scenario has three parts:

1. **`areas`**: named circles (`lat`, `lon`, `radius` in metres). They are places things happen: a meadow to hold, a shed to guard.
2. **`squads`**: named groups of bots (team, skill, loadout, `respawn`, and `tanks`). A squad is not in the game until a trigger spawns it.
3. **`triggers`**: "when this is true, do that". This is where the story is.

Start small. This one is the whole of a first scenario:

```json
{
  "areas": { "yard": { "lat": 63.8400, "lon": 20.3150, "radius": 30 } },
  "squads": { "guards": { "team": 2, "bots": [ { "lat": 63.8401, "lon": 20.3151 } ] } },
  "triggers": [
    { "when": { "start": true }, "do": [ { "spawn": "guards" }, { "objective": "Clear the yard" } ] },
    { "when": { "cleared": "guards" }, "do": [ { "win": "The yard is clear" } ] }
  ]
}
```

A worked, larger one is in `worlds/ersboda-alidhem/scenario.json` (Red Dawn): paratroopers land on the fields west of Mariehem, and the player runs for a hideout in the woods, ambushes a tank convoy on the highway, then raids the invaders' headquarters. It uses waves, delayed triggers, `done` chains, a hunting patrol and a time limit. Its routes follow the roads in 60 m steps, because a bot only searches for its way within about 80 m.

## 3. Patterns that work

- **Intro, then escalation.** Start with `spawn` plus an `objective`. A trigger on `cleared` of the first squad then says something and spawns the next after a `wait`.
- **Waves.** Give one trigger a `name`, and `times: 3`. A second trigger on `done` of that name and `cleared` of the squad finishes it. Spawning a squad again while some still stand adds more bots.
- **Defend a place.** Make an area, and lose when the enemy gets in: `{ "enter": "shed", "by": "raiders" }`. People in the area do not count when `by` is given.
- **Time limit.** `{ "after": 900 }` with a `lose`.
- **Tanks.** Put `tanks` on a squad. A tank is written like a bot, and two of the squad's bots crew it. With no `route` it drives along the nearest road; with one it follows the points in order. A tank only drives in straight lines between waypoints, so give it a route that goes round buildings.
- **Sides.** Squads on the same `team` fight together. Squads on different teams fight each other as well as you, which is a fun twist and a surprise if it is not meant.
- **Hard or easy.** `skill` runs 0 to 1: reaction time and aim error scale with it. 0.3 is a pushover, 0.8 is not.

## 4. Try it

1. Save the file and **restart the server**. It is read at start-up, and a mistake stops the server with the place named: an undefined name, an unknown field or action, an item that does not exist, a text over 200 characters, or a position outside the world. Fix what it says and start again.
2. Join the game. The objective shows at the top of the screen, and notices appear under it.
3. The scenario starts from the top again `restart` seconds after it ends (20 by default). People keep their health, scores and position, so move back to the start yourself.
   While no one is connected the whole room is paused: bots, scenario clock and physics hold still until someone joins, so `start` triggers fire with the first joiner, and `after` counts only time with someone there.
4. To test one part, make it the first: put `"start": true` on the trigger you are working on, or lower `wait` and `after`.

## 5. What it cannot do (yet)

- The map editor checks names, positions and limits before saving, but the server still has the final say when it starts.
- The minimap does not show a scenario's areas or objectives.
- A restart does not reset vehicles, items or the world, only the scenario's own squads and tanks.
- There is no script language: triggers are declarative, with no variables or counting beyond `times`.
- Bots use no vehicles other than tanks, and crews never get out.
