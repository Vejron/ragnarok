# Ghosts

A killed player no longer drops back in by parachute. After the three seconds lying dead (`RESPAWN_TICKS`)
they rise out of the body as a **ghost**: a churning shadow with a violet rim and two dim eyes, which
flies anywhere and touches nothing. When they choose, and where nobody is waiting to be ambushed, it
takes shape again as the soldier, with everything they carried.

The parachute drop is kept for coming into the game (joining, which is also how a player changes their
entry points: on the settings page) and for a ghost that gives itself up.

Bots, zombies and bus drivers do not become ghosts: they come back at a spawn point as before.

## Flying

| Key | Does |
|---|---|
| W/A/S/D | fly where you look (up and down too) and sideways |
| Space / C or Ctrl | rise / sink |
| E (use) | take shape here, once allowed |
| G (chute) | give up the ghost: drop by parachute at your next entry point, with your loadout afresh |

A ghost flies at `GHOST_SPEED` (20 m/s), rises and sinks at `GHOST_CLIMB` (10 m/s), and eases to the
speed it wants at `GHOST_ACCEL` (60 m/s²). It goes through buildings, trees, vehicles and water, but is
kept between `GHOST_FLOOR` (0.6 m) and `GHOST_CEILING` (150 m) over the bare ground under it
(`Physics::ground_under`, which ignores buildings, trees and placed cubes), and inside the playable square.

It is still dead the whole time: nothing can hurt it, and it can pick up, board, use or fire nothing.
Every mechanic that does those already asks whether the player is alive. The server's interest centre
follows it, so it sees the world around wherever it flies.

## Taking shape, and why it cannot be used to ambush

- **Not straight away.** A ghost may take shape only after `GHOST_MIN_TICKS` (5 s) as one.
- **Keep-out.** Not within `GHOST_KEEPOUT` (40 m, along the ground) of a living enemy: any living
  player, bot or zombie not on its team. Bus drivers do not count, and nor do the dead.
- **Somewhere to stand.** The ground under it must pass `spawn::usable`: inside the world, dry, and at
  least a metre clear of every building.
- **Slowly, and seen.** Taking shape lasts `GHOST_FORM_TICKS` (2 s). The ghost drops to the ground at once
  and everyone around sees a dark swirl closing in on the spot and rising into a column. It is still
  dead meanwhile, so it can neither act nor be shot. An enemy who walks up during the swirl does not stop
  it: the swirl is the warning.
- **Seen while flying.** Every player sees every ghost (within the usual 300 m interest radius).

A ghost still one after `GHOST_TICKS` (30 s) takes shape where it is if it may, and drops at an entry
point if it may not.

What comes back: full health, every limb, and the hotbar as it was at death (ammunition and all).
A drop at an entry point gives the player's own loadout afresh instead, so a ghost chooses between
coming back here with what it had and coming back there with a fresh kit.

## On the wire

- `STANCE_GHOST` (8) and `STANCE_FORM` (9) in `flags.stance`. The dead bit stays set; the client draws a
  ghostly stance as a cloud (`entities/presenters/ghosts.ts`, `render/ghost.ts`), never as a body.
- The owner's `vitals` carry `ghost` (whole seconds left as one, 0 for none) and `block`
  (`GHOST_BLOCK_*`: none, too soon, enemy near, no ground), which the HUD shows; the cloud's eyes turn
  red for the owner while blocked.
- `sim::Character::ghost` is predicted like any other field (`CHARACTER_LEN` 19). The client rebuilds it
  from `STANCE_GHOST` when reconciling, and steps its prediction while a ghost although the player is dead.
  The flight itself is `shared::ghost::ghost_velocity`, run by both sides.

## Where it is

| Path | What |
|---|---|
| `crates/server/src/mechanics/ghosts.rs` | `Ghost` component, rising, the keep-out, taking shape, the drop |
| `crates/server/src/mechanics/combat.rs` | a due player rises as a ghost (`ghosts::haunts`) instead of `spawn::place` |
| `crates/shared/src/ghost.rs` | `ghost_velocity` |
| `crates/sim/src/lib.rs` | `Physics::fly_ghost`, `ground_under` |
| `client/src/render/ghost.ts` | the cloud and the swirl, in smoke and glow particles |
| `client/src/entities/presenters/ghosts.ts` | who is a ghost and where to draw it |
| `client/src/hud/panels.ts` | the ghost's title, its time and keys |
