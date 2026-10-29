# Server performance

How much of the 60 Hz tick the server uses with a full room, how it was measured, and what that
says about where optimisation would and would not pay.

**Short version:** a full room of 64 players, moving and firing, costs about 2 to 3.5 ms of the
16.7 ms tick on the largest world we generate, spread out or not. World size barely matters.
Nothing here needs optimising yet.

## What was measured

An ignored test builds a `Game` from a world file, joins 64 players, puts them on the ground,
and runs 600 ticks of input (everyone running, strafing, turning, and firing about every half
second). It times each mechanic's `on_tick` and the room's snapshot build separately, and prints
the average and the worst tick.

```sh
# the committed city (fixtures/world)
cargo test --release -p server -- --ignored --nocapture full_room

# a generated world: the server's own variable picks it
RAGNAROK_WORLD=client/public/dev/world.msgpack cargo test --release -p server -- --ignored --nocapture full_room
RAGNAROK_WORLD=client/public/world.msgpack     cargo test --release -p server -- --ignored --nocapture full_room

# one player per spawn point, as far apart as the world allows
SPREAD=1 RAGNAROK_WORLD=client/public/world.msgpack cargo test --release -p server -- --ignored --nocapture full_room
```

The test is `full_room_tick_cost` in `crates/server/src/game.rs`. Run it with `--release`:
debug builds are many times slower and say nothing about play.

Players are placed on the world's spawn points, in groups of eight a few metres apart (or one per
spawn point with `SPREAD=1`). With no spawn points they stand on a grid over 500 m.

### What it covers and what it does not

- **Covered:** the mechanics (spawn, pickups, vehicles, gunnery, movement, NPCs, combat), including
  Rapier's character controller against buildings and trees, and building every player's snapshot
  (interest filtering, diffing against the baseline, shedding to the packet limit, encoding).
- **Not covered:** the session tasks (QUIC and TLS, datagram reads and writes, msgpack decoding of
  input), which run on other tasks and scale with players rather than world size; the cost of a
  slow or lossy client; memory. Use `RAGNAROK_SIM_LATENCY_MS`, `RAGNAROK_SIM_JITTER_MS` and
  `RAGNAROK_SIM_LOSS` (see [NETWORKING.md](NETWORKING.md)) with real browsers for those.
- **Players are simulated, not real:** their input is synthetic, so fights are not as clustered as
  a real match might be around an objective. Interest radius is 300 m, so a crowd of 64 in one
  spot would cost more snapshot work than any layout here; see "Unmeasured" below.

## Results

Release build, one machine, so read the ratios rather than the absolute numbers. Tick budget is
16.7 ms.

| World | Half extent | Buildings | Trees |
|---|---|---|---|
| Fixture city | 300 m | 228 | 110 |
| Dev world | 1375 m | 1,992 | 56,543 |
| Big world | 4300 m | 13,633 | 575,093 |

Mechanics, average per tick, and the worst single tick:

| World | Layout | Mechanics avg | Movement avg | Worst tick | Snapshots per call |
|---|---|---|---|---|---|
| Fixture | grid, 70 m apart | 2.0 ms | 1.9 ms | 8.0 ms | 0.66 ms |
| Dev | grouped | 1.3 ms | 1.26 ms | 1.9 ms | 0.82 ms |
| Dev | spread | 1.9 ms | 1.87 ms | 2.6 ms | 0.91 ms |
| Big | grouped | 1.8 ms | 1.68 ms | 7.6 ms | 0.94 ms |
| Big | spread | 2.5 ms | 2.29 ms | 7.9 ms | 0.97 ms |

A snapshot call builds the snapshots for the whole room and runs every second tick, so the
per-tick cost of a full room on the big world, spread out, is about 2.5 + 1.0 / 2 = 3.0 ms.

Other mechanics, per tick, on the big world: vehicles 0.12 to 0.14 ms, combat about 0.013 ms,
everything else under 0.002 ms. Movement is over 90 per cent of the total in every run.

## What the numbers say

1. **Movement is the cost, and it is cheap.** About 30 to 36 microseconds per player per tick: two
   `move_shape` calls (a horizontal and a vertical pass) through the character controller against
   the static colliders. Everything else is lost in it.
2. **World size barely matters.** From 228 to 13,633 buildings the mechanics cost goes from 2.0 ms
   to 1.8 ms, and the dev world is the cheapest of the three. The fixture used a different layout
   (a 70 m grid, not spawn points), so the rows are not strictly comparable; the dev and big
   worlds are. Rapier's broad phase and the world being cut into per-building colliders keep
   queries local.
3. **Spreading players out costs 35 to 50 per cent more movement.** Each character touches
   different colliders and nothing stays in cache. It is still under 2.5 ms.
4. **Snapshots are 0.7 to 1 ms for the room**, which is 10 to 15 microseconds per player. Interest
   filtering (300 m, with 40 m of hysteresis) keeps each player's lists short, and an unchanged
   entity costs nothing on the wire, so the diff is mostly empty.
5. **Worst ticks reach about 8 ms** on the large worlds. That is half the budget and happens
   rarely. It is likely the first tick after the room fills, when every player gets a full
   snapshot rather than a delta, and it has not been isolated.

## Things that look expensive and are not

The audit of the server flagged these. Measured, none of them matter at this size:

- **Per-player snapshot assembly is O(players x world).** At 64 players it is under 1 ms for the
  room. A spatial grid would add code and gain almost nothing until the room is several times
  larger.
- ~~**The shed loop re-encodes after every removal.**~~ This one did matter, once bots crowded
  the room (see [Crowds](#crowds)).
- **32 scenes kept per player as baselines.** This is memory, not time, and was not measured; it is
  bounded by 64 players x 32 scenes of what is in range.

## One trap

A character buried in the ground or inside geometry is expensive. The controller depenetrates
every tick, and in an early run of this test, with players spawned at y = 0 under a hilly world,
movement cost five times as much (10 ms instead of 1.9 ms). Nothing in play should produce that
(spawns are open ground, and falls land on terrain), but a bug that leaves players stuck in
a collider would show up as a server-wide tick spike, not just a stuck player. If a tick time
alarm is ever added, that is the first thing to check.

## Unmeasured

These are the cases most likely to move the numbers, in order of how much I would worry:

1. **Everyone in one spot.** All 64 inside each other's 300 m: every snapshot lists everyone, every
   entity changes every tick. The size cap and the shed loop start to matter. The test can do it by
   setting the spawn list to one point.
2. **A real match.** Grenades, rockets, tanks and planes in flight add projectiles, vehicle
   physics (a tank or plane is dynamic Rapier bodies stepped every tick) and destruction (a
   collapsing building rebuilds its colliders). The test only has the placed vehicles standing
   still.
3. **A slow client.** A full outbox now disconnects the player (the outbox holds 64 messages,
   shared by snapshots and reliable messages); how often that fires under real load is unknown.
4. **The network tasks.** Datagram encode and send for 64 sessions at 30 Hz is about 2,000
   packets a second.

## If it ever needs to go faster

In order of expected return:

1. Narrow the character controller's query filter (skip sensors, groups that cannot collide) and
   reuse the query pipeline across players within a tick. Movement is over 90 per cent of the
   cost.
2. Stop rebuilding snapshots for players whose view has not changed: a player standing still with
   nothing moving near them produces an empty delta, and the whole scan could be skipped by
   comparing a per-region version number.
3. A spatial grid for interest, replacing the linear scan of entities and vehicles per player.
4. Only then, parallelise snapshot building (the replicas are independent per player).

None of these is worth doing before the unmeasured cases above show a problem.

## Bullets

Guns (AK5, revolver, sniper) fire real, flying bullets. Measured with the same full-room test, every
player firing an AK5 ten times a second (about 120 bullets in the air at once), fixture city:

| How bullets reach clients | Snapshots per call, whole room |
|---|---|
| Hitscan (before) | about 1.0 ms |
| Listed as projectiles in every snapshot, every tick of flight | 45 to 75 ms (3 to 4 ticks of budget) |
| Announced once as a spawn, flown by the clients (built) | about 2.2 ms |

Stepping a bullet is cheap, one segment cast and one player test a tick; the cost was the wire. The
shed loop also now drops effects in bulk, by the excess over the packet limit, instead of one
re-encode per item. A synthetic room with every player firing full-auto is the worst case.

## Crowds

With bots (BOTS.md) a room holds 64 people and 64 bots, and the benchmark (`BOTS=64`) packs all 128
soldiers around the spawn points, running. Almost every one of them changes in every snapshot, so a
player's delta is about twice what fits in 1200 bytes, and the farthest half of it has to be shed.
The shed loop dropped one entity at a time and re-encoded the whole snapshot after each, so dozens
of encodes per player per snapshot:

| Shedding entities and vehicles | Snapshots per call, 64 people + 64 bots | 64 people, no bots |
|---|---|---|
| One re-encode per thing shed (before) | 8.1 ms | 0.85 ms |
| Halving to find how many fit | 3.3 ms | 0.86 ms |
| Size worked out from each item's size, then one encode (built) | 1.9 ms | 0.60 ms |

Each list is a MessagePack array, a header (1 byte below 16 items, 3 below 65,536) and then its
items back to back, so the size with the nearest `k` kept is the size with the lists empty plus the
kept items' sizes plus the header growth. That is exact, and a `debug_assert` checks it against
the real encode. Sorting the delta by distance also looked each distance up in a `HashMap` at
every comparison; it now looks each up once (`sort_by_cached_key`). Effects still shed first, in bulk.
