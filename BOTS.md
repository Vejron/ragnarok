# Bots

Bots are enemy stickmen run by the server. A bot walks a patrol route. When it sees someone who is not on its team, it goes after them and shoots. When it loses sight of them it goes to where they were last seen, looks round there, and then goes back to its route. A gunshot or a blast it hears, or a hit it takes, sends it to look where that came from. It finds its way round buildings, walls and trees. Two bots can crew a tank, which drives along the roads and shells whoever its gunner sees (see [Tank crews](#tank-crews)). Bots are placed per world in `area.json`, and a world's `scenario.json` can bring on squads of them, set objectives and end in a win or a loss (see [Scenarios](#scenarios)).

All eight stages of the plan are done (see [Roadmap](#roadmap)), with the limits it lists. Tanks are the only vehicles bots use. On the scoreboard and the minimap bots and teams are told apart (see [On the HUD](#on-the-hud)).

## Trying it

```sh
RAGNAROK_BOTS=6 cargo run -p server        # six bots on made-up loops near the player spawns
RAGNAROK_TANKS=2 cargo run -p server       # two tank crews, driving along the nearest roads
```

To see what they are thinking, add `RUST_LOG=info,server::mechanics::bots=debug`. It logs each time a bot spots someone.

## Placing bots

Place bots and draw their routes in the map editor, with the Bot and Route tools (see [EDITOR.md](EDITOR.md)). Pick a bot to set its team and skill, make it stand guard, or let it make up its own loop. The editor saves them in the world's `worlds/<name>/area.json`, next to `spawns`, `vehicles` and `items`, and the file can also be written by hand:

```json
"bots": [
  { "lat": 63.8201, "lon": 20.2934 },
  { "lat": 63.8205, "lon": 20.2950, "team": 2, "skill": 0.9,
    "route": [ { "lat": 63.8205, "lon": 20.2950 }, { "lat": 63.8209, "lon": 20.2950 }, { "lat": 63.8209, "lon": 20.2962 } ] }
]
```

| Field | Default | Meaning |
|---|---|---|
| `lat`, `lon` | required | Where the bot spawns, and respawns after dying. |
| `team` | `1` | Bots on the same team ignore and cannot hurt each other. Bots on different teams fight each other. |
| `skill` | `0.5` | How good a shot it is, from `0` (a raw recruit) to `1` (a marksman). See [Fighting](#fighting). |
| `route` | none | The waypoints it walks round, in order, looping. It finds its own way between them, so a route can go round buildings. With no route it makes up a short loop of open ground within 25 m of home. A route of only its own spot makes it stand guard there. |

A bot placed outside the world's tiles is skipped, and so is any waypoint outside them; the editor only lets them go on painted tiles. Restart the server after saving.

## Scenarios

A world can have a `scenario.json` next to its `area.json`, in `worlds/<name>/`. It has named **areas**, **squads** of bots, and **triggers** that say what happens when. Placed bots in `area.json` are there all the time; a squad is there only once a trigger spawns it. The file is written by hand for now (the editor does not know about it), with positions in latitude and longitude like `area.json`. Restart the server after changing it.

```json
{
  "restart": 20,
  "areas": {
    "farm": { "lat": 63.8420, "lon": 20.3440, "radius": 40 },
    "barn": { "lat": 63.8421, "lon": 20.3437, "radius": 8 }
  },
  "squads": {
    "guards": {
      "team": 2, "skill": 0.4,
      "bots": [
        { "lat": 63.8422, "lon": 20.3436 },
        { "lat": 63.8418, "lon": 20.3445, "route": [ { "lat": 63.8418, "lon": 20.3445 } ] }
      ]
    },
    "raiders": {
      "team": 2, "skill": 0.7, "loadout": ["ak5", "shotgun", "bandage"],
      "bots": [
        { "lat": 63.8439, "lon": 20.3440, "route": [ { "lat": 63.8421, "lon": 20.3437 } ] },
        { "lat": 63.8440, "lon": 20.3446, "route": [ { "lat": 63.8421, "lon": 20.3437 } ] }
      ]
    }
  },
  "triggers": [
    { "when": { "start": true }, "do": [ { "spawn": "guards" }, { "objective": "Take the farm" } ] },
    { "when": { "enter": "farm" }, "do": [ { "say": "They have seen you!" } ] },
    { "name": "taken", "when": { "cleared": "guards" }, "wait": 10,
      "do": [ { "objective": "Hold the barn" }, { "say": "Raiders from the north!" }, { "spawn": "raiders" } ] },
    { "name": "waves", "when": { "done": "taken", "cleared": "raiders" }, "wait": 5, "times": 2,
      "do": [ { "say": "More of them!" }, { "spawn": "raiders" } ] },
    { "when": { "done": "waves", "cleared": "raiders" }, "do": [ { "win": "The farm is yours" } ] },
    { "when": { "enter": "barn", "by": "raiders" }, "do": [ { "lose": "The raiders took the barn" } ] },
    { "when": { "after": 900 }, "do": [ { "lose": "Out of time" } ] }
  ]
}
```

This one has you clear two guards off a farm, then hold its barn against three waves of two raiders. The raiders walk 200 m from the north to the barn. You lose if one of them gets there, or after 15 minutes.

**Areas** are circles: `lat`, `lon` and `radius` in metres.

**Squads.** A squad's bots are written like placed bots (`lat`, `lon` and an optional `route`), and the rest is the squad's:

| Field | Default | Meaning |
|---|---|---|
| `team` | `1` | As for a placed bot. Squads and placed bots on the same team are on the same side. |
| `skill` | `0.5` | As for a placed bot. |
| `loadout` | the standard one | Item names, one a slot, up to five: `revolver`, `ak5`, `grenade`, `bandage`, `sniper`, `golden egg`, `shotgun`, `fpv drone`. Guns come loaded and stacks full. Bots fight only with the guns. |
| `respawn` | `false` | Whether its bots come back at home after dying, with the same loadout. A squad that does not respawn leaves the game once its fallen would have got up (3 s). One that does is cleared only while all of its bots are down at once. |
| `tanks` | none | Tanks, each crewed by two of the squad's bots (see [Tank crews](#tank-crews)). A tank is written like a bot, `lat`, `lon` and an optional `route`, which it drives; with none it drives along the nearest road. Its crew are numbered after the squad's bots, and count towards `cleared` and `enter` with `by` like the rest. |

Each bot is named after its squad: Guards 1, Guards 2 and so on.

**Triggers.** A trigger fires when all the conditions in its `when` come true together. It fires again only after they have stopped being true and come true again, and only `times` times (default `1`; `0` is no limit). After it fires it waits `wait` seconds (default `0`), then does everything in `do`, in order.

| Condition | True while |
|---|---|
| `"start": true` | Always, so it fires as the scenario starts (and starts again). |
| `"after": s` | `s` seconds or more have passed since the start. |
| `"enter": "area"` | A living person is in the area, on foot or in a vehicle. With `"by": "squad"`, one of that squad's bots is. |
| `"cleared": "squad"` | The squad has been spawned and none of it is standing. |
| `"done": "trigger"` | The trigger with that `name` has fired all its `times` (or, with no limit, at least once). |

| Action | What it does |
|---|---|
| `"spawn": "squad"` | Adds the squad's bots, each at its home. Spawning it again while some still stand adds more: waves. |
| `"remove": "squad"` | Takes the squad's bots out of the game. |
| `"say": "text"` | Everyone sees the text for 6 s, under the objective. |
| `"objective": "text"` | Everyone sees the text at the top of the screen until the next objective. Someone joining is told the current one. `""` clears it. |
| `"win": "text"`, `"lose": "text"` | Ends it: everyone sees VICTORY or DEFEAT and the text for 8 s, and the objective is cleared. Anything after it in the same `do` is not done. |

After it ends, the scenario waits `restart` seconds (default `20`), clears its bots away and starts again from the top. People are left as they are: their scores, health and where they stand.

**Mistakes stop the server.** A name that is not defined, an unknown field or action, a position outside the world, an item that does not exist, or a text over 200 characters stops the server at start-up, with a message saying where. A trigger that silently never fired would be harder to find. No `scenario.json` means no scenario.

**On the wire.** The scenario speaks to clients with one reliable message, `notice` (tag 12, `[12, kind, text]`, see NETWORKING.md), so this stage moved `PROTOCOL_VERSION` to 31.

## Tank crews

A tank crew is two bots in one tank, `KIND_TANK`. The driver sits in seat 0 and the gunner in seat 1, and each writes the input a person in that seat would send, so `Vehicles` and `Gunnery` drive the real tank (`mechanics/bots/crew.rs`). A scenario squad brings them on with `tanks`, and `RAGNAROK_TANKS=n` adds `n` at the world's spawn points, from the last back.

- **The route.** A tank drives round its waypoints in straight lines. It does not use the walking grid, which is for bodies a metre wide, so a route should keep to open ground and roads; a tank runs over trees and through walls it can break, as anyone's does. With no route it drives along the nearest road: from the road point nearest it, 300 m out each way and back, with a waypoint at least every 8 m. Roads that meet end to end are one network, and at a junction it keeps to whichever road runs straightest on.
- **Driving.** It steers for the next waypoint in proportion to how far off it is heading, turns on the spot when that is more than 0.7 rad, and eases off the throttle in between. Coming up to a waypoint where the route turns sharply, it slows over the last 25 m: to 3 m/s for a right angle, and to almost nothing to turn right round at the end of a road. It goes on to the next waypoint within 5 m of one.
- **Stuck.** When it has not got 3 m nearer to where it is going in 5 s, it backs out for 2 s and tries again. After three tries it gives that waypoint up for the next. Shoving a parked car along at a crawl counts as stuck.
- **The gunner** looks from just above the turret 10 times a second, all round, out to 120 m, at people on foot and in vehicles other than its own. It does not see through anything solid. It lays the gun where the shell will meet the target: the elevation that lands a shell (140 m/s, falling) on them, ahead of them by how far they move while it flies, less of that at low skill. To that it adds an error of up to 0.04 rad at skill 0 and 0.004 rad at skill 1, three times that on first sight and settling over a second. It takes 1.2 s to react at skill 0, down to 0.4 s at skill 1. It fires once the gun lies within 0.02 rad of where it wants it and the line from the muzzle is clear, as often as the gun reloads (3 s). It never fires at anyone nearer than 12 m, where the burst would reach its own tank.
- **What the driver does about it.** The gunner's state is the crew's. While the gunner sees someone, the driver halts. When the gunner loses them, or hears something, or the tank is hit, the driver drives to where that was, and halts 15 m short. The gunner swings the turret there, and after 20 s with no sight of anyone, or once there, it sweeps the turret round for 6 s. Then they go back to the route, from the waypoint nearest. The leash is the same 150 m from the route as on foot.
- **On foot.** A crewman out of its tank, from respawning or being put out, walks back to it if it is within 60 m, is not a wreck, and its seat is free, and gets in once within reach. It does not do this while fighting someone it can see. Otherwise it fights on foot like any bot. A wreck kills its crew, as it kills anyone inside. A crew that respawns gets up beside where the tank was parked, and is back in when the tank is put back there, 10 s after it was wrecked.
- **A scenario's tanks** go when their crew have gone and no one is in them: at once, or for a wreck, once it has burnt out. So a cleared squad leaves no tank behind for anyone, and `remove` and starting again take its tanks with its bots.

## On the HUD

Every player's `profiles` entry says which team they are on and whether they are a bot (see NETWORKING.md), so `PROTOCOL_VERSION` is 32.

- **Scoreboard** (Tab). When there is more than one side, the board is split into them, each with its kills and deaths between them: yours first, then the rest by kills. People are one side, "People", although they are all against all. Each bot team is "Team N". Bots are tagged BOT, here and in the kill feed.
- **Minimap.** It marks gunfire for 4 s where it came from, as a fading diamond: red for a bot, the shooter's outfit colour for a person, and orange for a rocket, shell or grenade, whose shooter is not sent. It leaves out your own shots and your side's, and a shell or rocket that started within 10 m of you. Gunfire is shown only when the server sent it, so only within the range snapshots cover. Allies (players on your team, other than 0) are shown as teal dots while in view. People are on no team, so for now only a bot would see any. The full map (M) shows the same marks.

The marks are kept by `Blips` (`client/src/blips.ts`), and the board is split by `sides` (`client/src/scoreboard.ts`).

## What a bot does

| Mode | What it does | Ends when |
|---|---|---|
| **Patrol** | Walks to its next waypoint and then the one after, looping. It sees in a 120° cone in front, out to 60 m. | It sees an enemy, hears something or is hit, and switches to Pursue. |
| **Pursue** | While it sees the target it fights them (see [Fighting](#fighting)). Out of sight, it runs to where they were last seen, or to where it heard something (see [Hearing](#hearing)). While alert it sees all round. | It reaches the last-seen spot and finds no one there, or gets stuck (Search). It goes 150 m from its route (Patrol). |
| **Search** | Stands where it lost them and turns slowly, for 6 s. | It sees someone, hears something or is hit (Pursue), or time runs out and it goes back to the nearest waypoint (Patrol). |

- **Sight.** A bot looks 10 times a second, each bot on its own beat. It sees a living person on foot (or a drone pilot's kneeling body) when nothing solid stands between its eye and their chest: terrain, buildings, trees and vehicles all block sight. It picks the nearest enemy it can see. It does not see people inside vehicles.
- **Moving.** It turns at up to 6 rad/s and only walks once it is facing roughly where it is going. It walks on patrol and runs when pursuing. It walks the way it finds (see [Finding the way](#finding-the-way)), one corner at a time. If something the grid does not know about, such as a vehicle, is within 1.2 m ahead, it tries headings up to ±1.8 rad off. When it has not got 0.5 m closer to the next corner in 3 s, it gives up the goal: on patrol it moves on to the next waypoint, and in a pursuit it starts searching.
- **Unarmed.** A bot with no gun runs at the target, stops 3 m away and faces them.
- **Dying.** A bot dies, is scored and respawns like anyone else (after 3 s, at its home). It comes back on patrol, with whatever it was doing forgotten.
- **Teams.** `Player::team` is `TEAM_NONE` (0) for people, who still all fight each other, so people and bots are always enemies. Friendly fire is off for any non-zero team. That covers bullets, blasts and roadkill, since they all go through `combat::damage_hit`.

## Fighting

A bot fights while it can see its target. It starts with the same hotbar as anyone else (revolver, AK5, sniper, grenades, shotgun) and uses only the guns.

- **Gun.** It takes the shotgun under 10 m and keeps it until 15 m, and otherwise the AK5. With neither, it takes the revolver, then the sniper, then any gun. With no one in sight it holds the gun it would use at 30 m.
- **Distance.** It walks towards the target while it fires, until it is close enough for that gun: 5 m with the shotgun, 15 m with the AK5, 12 m with the revolver, 35 m with the sniper. There it stands and aims down the sights (`BUTTON_ALT`: a quarter of the spread, and no spread at all through the sniper's scope). A bot of skill 0.3 or more also sidesteps, changing direction every 0.6 to 1.6 s.
- **Reacting.** From the moment it catches sight of someone, it holds its fire for 0.7 s at skill 0, down to 0.2 s at skill 1. It waits again each time it loses sight of them and sees them again.
- **Aiming.** It aims at the chest, where it thinks the target is by now. It knows only what it saw at its last look, so it extrapolates their speed from the last two looks. It aims ahead by the distance they run while the round flies, at 280 m/s, and above by how far the round drops on the way. A skill-0 bot leads by only 30 per cent of that. A shotgun needs no lead. To all of that it adds an aim error, drawn afresh at every look. The error is up to 0.1 rad at skill 0 and 0.008 rad at skill 1. On first sight it is three times that, and it settles over about a second of watching.
- **Firing.** It fires only once it faces within 0.05 rad of where it means to shoot. It fires the AK5 in bursts of 3 to 6 rounds, then pauses 0.6 s at skill 0, down to 0.25 s at skill 1. It presses again for every round of the other guns, as fast as each gun allows.
- **Reloading.** It reloads when the gun in hand is empty. With no one in sight, it also tops up any gun that is not full.

Against someone running across its front at 4 m/s, 25 m away, over 10 s, a bot does about 100 to 140 damage at skill 0, 300 to 350 at skill 0.5 and 600 to 700 at skill 1. That is a kill in about 8 s, 3 s and 1.5 s.

A bot's shot leaves its eye along its look direction (`combat::fire_gun`). A person's shot goes towards what the over-the-shoulder camera's crosshair is on, and a bot has no camera.

## Hearing

Each tick a bot listens to the noises made since it last listened (`World::noises`). It does not listen while it is fighting someone it can see.

| Noise | Carries | Made in |
|---|---|---|
| A person's gun firing, from where they stand | 80 m | `combat::fire_gun` |
| A vehicle's machine gun firing, from the muzzle | 80 m | `gunnery::machine_gun` |
| A shell, cannon round or mortar bomb being fired, from the muzzle | 150 m | `gunnery::fire` |
| Any blast: a shell, rocket, grenade or bomb bursting, or a wreck going up | 150 m | `combat::blast_at` |

- **Going to look.** It takes the nearest noise it hears and goes to look where it came from, as it goes to where it last saw someone: it runs there alert, so it sees all round, and if it finds no one it searches there for 6 s. Walls do not stop a noise, and the spot it hears is exact.
- **Whose noise.** It ignores only its own. A teammate's gunfire brings it to help, and so does an enemy's.
- **Busy.** Already on its way to look somewhere, it changes its mind only for a noise at least 10 m nearer. In a busy fight the nearest shot jumps from one shooter to another each tick, and every change of goal is a new search for a way.
- **Too far.** It ignores a noise more than 150 m from its route, where it would give up the chase on the way. The leash is measured from the nearest waypoint, not from home, so a squad sent 200 m to attack still fights when it gets there.
- **Being hit.** A hit sends it to look where the hit came from: the attacker, or the centre of a blast (`combat::damage_hit` sets `Bot::hit_from`). So a calm bot shot from behind turns round, and if it can see the attacker it fights back. A hit counts for more than a noise, but not while it is fighting someone it can see.
- `combat::damage_hit` sets `Bot::hit_from` directly. Noises are made through `bots::make_noise`, which keeps nothing when there are no bots, and `Bots` empties the list at the start of each tick.

## Finding the way

`crates/server/src/navigation.rs` keeps a grid of 1 m cells over the world (`World::nav`). Each cell is open or blocked, and an open cell has the height of its floor.

- **A cell's floor** is the topmost static surface under its middle: the ground, a bridge, a deck or a roof. Vehicles are not part of the grid.
- **Blocked.** A cell is blocked when a 1 m box over it, from 0.5 m above its floor to 1.9 m, touches anything static: a wall, a tree, a rock, the world's edge, or ground too steep to climb. Buildings are hollow meshes, so this box is what makes their thin walls block the cells they pass through. An alley needs to be about 2 m wide to be open.
- **Joining.** Two cells side by side join when their floors are within 1 m of each other. So steps and slopes up to about 45° can be walked, a bridge joins the road at its ends, and a roof is an island of its own.
- **Built as needed.** The grid is built 16 × 16 cells at a time, the first time a search looks there. It is never built over the whole world at once, which would be 74 million cells on the big world. When a building is bitten or comes down, `Physics` notes the ground it stood on (`take_changed`), and the chunks there are built again.

**The search.** It is A* over the cells, with 8 neighbours, never cutting the corner of a blocked cell. It is used only when the straight line to the goal is not clear. The way found is cut down to its corners: from each corner, on to the furthest cell still in a clear straight line. A straight line is clear when the cells under the body's middle and both its sides, every 0.5 m, are open and join.
- When someone stands against a wall, their own cell is blocked. The search then starts or ends at the nearest open cell within 2 m whose floor is at their height, and still finishes at their feet.
- Before searching, it floods the ground round each end, up to 400 cells. An end on a small island, such as a roof, a walled yard or the inside of a building, is found out this way, without searching thousands of cells for a way that is not there.

**When a bot looks for a way.** It looks when it has none, when its goal has moved more than 2 m, and every 2 s on the way. Fighting needs no way: it walks straight at someone it can see. All the bots together look at no more than 8,000 cells a tick, and building a chunk counts as 200. One search stops at 6,000 cells (about an 80 m square). A bot whose turn has not come yet, or whose search found nothing, heads straight for its goal, as it did before it could find ways.

## How it works

**A bot is a `Player` with no connection.** Each tick, the game loop gives every player an `InputFrame` (the movement axes, yaw, pitch, buttons and slot), and every mechanic reads only that. `Bots` (`crates/server/src/mechanics/bots.rs`) runs first and writes the bot's `InputFrame` itself. Everything after it handles the bot exactly as it would a person: walking and physics, items, seats, damage, kills, scores, respawning, replication and how it is drawn. Clients see a bot as an ordinary stickman, so bots needed no protocol change and no client change. Only scenarios added one, the `notice` message.

- `Game::add_bot(home, team, route)` makes the player with no outbox. Snapshots and reliable messages go only to outboxes, so nothing is sent to a bot. It gets the starting hotbar, an outfit colour per team, and `spawns = [home]`, so respawning puts it back at home. Every connected client is sent its `Profiles` entry.
- **Caps.** Up to `MAX_BOTS` (64) bots, on top of `MAX_PLAYERS` (64) people. A join counts only the players with a connection, so bots never keep a person out. Placed bots past the cap are skipped, with a warning.
- `World::bots` holds each bot's mind (`Bot`), keyed by player id: its route and next waypoint, its mode, its target and where it last saw them (or heard something), where its last hit came from, its smoothed yaw and pitch, its aim and trigger state, the corners of its way, and a progress counter for noticing when it is stuck.
- `Game::new` adds `WorldFile::placed_bots`, which `main.rs` reads from `area.json` as it does the other placements. `RAGNAROK_BOTS` adds made-up bots on top, at the world's spawn points just past the first six that players start at.
- **Tank crews** (`mechanics/bots/crew.rs`). `Game::add_crew` parks a tank with `vehicles::add_tank`, adds two bots with `Bot::crew` set to it and their seat, and seats them with `vehicles::seat`, which takes a given seat where `BUTTON_USE` takes the first free one. `think` hands a seated crewman to `crew::work`, and sends one on foot to its tank with `crew::board`. `combat::damage_vehicle` sets `Bot::hit_from` for the bots inside. `vehicles::remove_vehicle` takes a scenario's tank away.
- **Scenarios** (`crates/server/src/scenario.rs`). `main.rs` reads `scenario.json` into `Game::scenario`. `Scenario::parse` checks every name once and keeps areas, squads and triggers as indices. `Game::step` ticks it after the mechanics. Squads come on through `Game::add_bot_as` (`add_bot` with a name), and leave through `Game::remove_bot`. A squad's loadout is kept in `Bot::loadout`, which `Combat` gives back on respawn in place of the standard one. `Game::notice` sends a `Notice` to everyone, and a joiner gets the current objective with the catch-up. The client keeps them in `client/src/notices.ts` and draws them in `App.vue`.
- The made-up route uses the rooster's `open_ground` and checks for a clear straight line between waypoints, so a made-up loop never runs through a wall.

### Cost

`full_room_tick_cost` takes `BOTS=n` to add bots to its 64 people, spread over the spawn points among them:

```sh
BOTS=64 cargo test --release -p server -- --ignored --nocapture full_room
```

Measured on the committed fixture city, release build, 64 people:

| Bots | Mechanics per tick (avg / worst) | `Bots` itself | Snapshots per send (every 2 ticks) |
|---|---|---|---|
| 0 | 2.2 / 4.8 ms | — | 0.6 ms |
| 64, before they shot | 3.6 / 5.7 ms | 0.11 ms | 1.9 ms |
| 64, shooting | 3.0 / 6.6 ms | 0.09 ms | 2.0 ms |
| 64, finding their way | 3.3 to 3.7 / 6 to 8 ms | 0.16 ms | 2.0 ms |
| 64, hearing | 3.1 / 6 to 8 ms | 0.2 ms | 1.9 ms |
| 16 | 2.3 / 5.8 ms | 0.05 ms | 0.9 ms |
| 16, and 4 tank crews | 2.6 / 7.4 ms | 0.09 ms | 1.2 ms |

`TANKS=n` adds tank crews to `full_room_tick_cost` as `BOTS=n` adds bots. A crew costs little: the gunner casts one ray per enemy in range at each look, and the driver none. Four crews among 64 people took `Bots` from 0.05 to 0.09 ms, and `Vehicles` from 0.19 to 0.23 ms. In the benchmark the gunners see people all the time, so the tanks stand and fire.

The budget is 16.7 ms a tick, and a full room of 64 people and 64 bots uses about a third of it on a snapshot tick (about 5 ms). Thinking and shooting are cheap: `Combat` goes from 0.04 to 0.07 ms. Most of the cost is moving the bots' bodies (`Movement` goes from 2.0 to 2.6 ms with 64 bots). This is lower than before they shot, because a bot that is fighting stands still.

Finding the way doubles what `Bots` costs, to about 0.16 ms a tick. In the first ticks the grid is built round every bot at once. The 8,000-cell budget spreads that over about three ticks of 3 to 5 ms for `Bots`, where it was one tick of 13 ms before chunk building counted against the budget. After that, `Bots` reaches about 2.5 ms on its worst tick. Hearing at first cost 2.1 ms a tick. In the benchmark every bot hears shots all the time, and each one went to whichever was nearest that tick, so its goal kept moving and it searched for a new way every tick until the budget ran out. Changing its mind only for a noise 10 m nearer brought that back to about 0.2 ms. Before the island check, a search for someone standing on a raised deck searched all of its cells for nothing, at 3 ms each, and four in one tick made 10 ms. Snapshots used to be the problem: with 128 stickmen packed round the spawns, each player's delta is about twice what fits in a packet, and shedding the excess cost a re-encode per stickman shed (8.1 ms per send at 64 bots). It is now worked out in two encodes; see `SERVER-PERFORMANCE.md`, Crowds.

### Tests

`mechanics/bots/tests.rs` builds a flat world, sometimes with a 10 m wall, and steps the real game. The tests check that a bot:

- walks its route round and round, and makes one up when given none;
- spots someone in front of it and closes in;
- does not see behind it or through the wall;
- goes to where it lost them, searches, and goes back to its route;
- leaves its own team alone, attacks a bot of another team, and is still hurt by people;
- has its own cap, and never keeps people out of the room;
- comes back home on patrol after dying;
- runs at someone when it has no gun;
- shoots someone it sees until they die, but not before its reaction time;
- hits a running target at least three times as often at skill 1 as at skill 0;
- leads a moving target, aims above the drop, and does not lead with a shotgun (`aim_point`);
- takes the shotgun up close and the AK5 further out, and stands off at its gun's distance;
- reloads when empty in a fight, and tops up with no one about;
- shoots from its eye along its look;
- walks round a wall that stands across its route, and chases round it to where it last saw someone. With path finding turned off, both of these walk into the wall;
- turns round when shot from behind, and fights back;
- goes to look where it heard a gunshot behind a wall, and searches there;
- hears a blast further off than a gunshot, and ignores its own shots;
- is not drawn off by noises while it fights someone it can see;
- comes running when a teammate fires past the wall;
- still fights at the far end of a route 170 m from home. With the leash measured from home, it gives up and never fires.

With hearing turned off, all of these but the one about not being drawn off fail.

`mechanics/bots/crew/tests.rs` checks that a crew:

- is two bots in their seats of a fresh tank;
- drives its route round and round, and with none drives along the nearest road and keeps to it, turning round at its end;
- makes a road route out both ways and back, on through junctions straight on and not down a side street, and stays put with no roads;
- lays a shell that lands on its mark (`lay`);
- shells someone it sees behind it, and holds still while the gunner fights;
- leaves its own team alone, and does not fire on anyone within 12 m;
- backs out of a wall and gives the waypoint up, and never gets through it;
- gets back in when its wrecked tank is put back;
- fights on foot when its tank is gone.

`tanks_drive_the_fixture_city`, ignored, lets six crews loose on the committed fixture city for two minutes and prints how far each drove. They drove 460 to 1,200 m each, all upright; one stuck behind a parked car turned back.

`scenario/tests.rs` runs scenarios in the real game. A squad spawns at the start with its loadout, skill and names, and the objective reaches everyone, joiners too. A fallen squad bot lies for 3 s and then leaves. Clearing a squad wins, the actions after the win are not done, and after `restart` the squad and the objective come back. Coming into an area fires each time someone comes in, not while they stay, and not for the dead. Waves come one after another with their `wait`, and the last one cleared wins; no fallen bot gets up while it waits to start again. Time running out loses, and so does a squad reaching an area, which people standing in it do not count for. A squad that respawns gets its own loadout back. Mistakes in the file are refused, each with what is wrong, and so is a squad with no bots and no tanks. A squad's tank comes crewed, its crew numbered after its bots, and once it is wrecked and its crew have gone it goes too, after it has burnt out. Removing a squad takes its tank, and starting again brings a fresh one. The example above is parsed too, so it stays right. `notices.spec.ts` checks the client side: the objective stays until the next, messages fade, and a win or a loss shows for a while.

On the HUD, `game.rs` checks that a bot's profile carries its team and the bot flag, to everyone in the room and to a joiner. `scoreboard.spec.ts` checks that the board splits by team with your side first. `blips.spec.ts` checks that gunfire is marked by who fired and fades, that your own shots, your side's and a shell from next to you are left out, that a burst is one mark, and that allies are shown. `profile.spec.ts` checks that people are nobody's allies.

`navigation/tests.rs` checks the grid on its own. It goes straight over open ground and round the end of a wall, through a gap, and up steps onto a deck. A closed room and a roof cannot be reached, and finding that out takes no more than the two island floods. It reaches someone standing against a wall, keeps within its budget, and builds a chunk again once it is forgotten.

## Roadmap

1. ~~Bot players and teams, patrol, sight, pursue, search.~~ Done.
2. ~~**Shooting.**~~ Done (see [Fighting](#fighting)). Bots do not throw grenades yet, or take cover.
3. ~~**Path finding.**~~ Done (see [Finding the way](#finding-the-way)). There are no doors: the inside of a building is never reached. There is also one floor per cell, so the road under a bridge is taken to be the bridge.
4. ~~**Hearing.**~~ Done (see [Hearing](#hearing)). There are no quiet guns, and a bot hears every noise as loudly through a wall. Footsteps and engines make no noise.
5. ~~**Editor tools.**~~ Done (see [EDITOR.md](EDITOR.md)): the Bot and Route tools place bots, draw their routes, and set team and skill. Before this, saving a world in the editor dropped any bots written into `area.json` by hand. Markers still cannot be dragged.
6. ~~**Scenarios.**~~ Done (see [Scenarios](#scenarios)): squads with team, skill and loadout, areas, triggers, waves, objectives, a win or a loss, and a restart. It is all declarative. There is no script language, no editor support, and nothing on the minimap yet. A scenario does not reset people, vehicles or the world when it starts again.
7. ~~**Vehicles.**~~ Done (see [Tank crews](#tank-crews)): a driver and a gunner in a tank, along the roads or a route, with a steering controller over the real tank physics and backing out when stuck. Tanks drive straight between waypoints, with no path finding of their own, so a building across the way is driven into or, if it will not break, given up. A crew does not get out to fight, does not swap seats when one of them dies, and never takes a tank it was not given. Bots use no other vehicles, and the editor cannot place crews yet.
8. ~~**HUD.**~~ Done (see [On the HUD](#on-the-hud)). Bots have no name tags over their heads, and the minimap does not show a scenario's areas or objectives.
