# Regrowth

Felled trees grow back and broken pieces of fences and walls are mended, so a long session does not end in a cleared wood. It is the same idea as the craters healing (CRATERS.md, "Healing"): nothing changes where a player could see it, and nothing grows through what stands in the way. Protocol 99.

## Stages

A felled tree goes through `GROWN_*` (`crates/shared/src/consts.rs`), each no sooner than its time after it was felled:

| Stage | From | Drawn | Solid |
|---|---|---|---|
| `GROWN_LOG` (0) | felling | the log lies where it fell | no |
| `GROWN_SAPLING` (1) | `REGROW_SAPLING_SECONDS` (4 min) | the log has rotted; the tree at `TREE_SAPLING_SCALE` (0.25) | no |
| `GROWN_YOUNG` (2) | `REGROW_YOUNG_SECONDS` (9 min) | at `TREE_YOUNG_SCALE` (0.6) | a trunk 0.6 the size; it can be felled again, with 0.6 of the health, and starts over as a log |
| `GROWN_WHOLE` (3) | `REGROW_WHOLE_SECONDS` (15 min) | as it was | as it was; no longer felled |

A broken piece of fence or wall goes from broken straight to whole after `LINE_MEND_SECONDS` (15 min). Buildings never come back.

## When a step is taken

The server looks once every `HEAL_CHECK_TICKS` (`Game::heal`, with the craters) and takes each felled thing that is due at most one stage further (`World::regrow`, `crates/server/src/game/regrowth.rs`):

- **Nobody sees it.** No player with a connection (their soldier, and what they drive or fly) is within `HEAL_CLEAR` (400 m) of it. Bots do not hold it back.
- **Nothing is in the way.** A step that makes it solid (young, whole, a mended piece) waits while a soldier, bots too, a vehicle, any entity with a place, or a placed cube is within `REGROW_CLEARANCE` (1 m) of its trunk or its piece.
- **The ground is whole.** A tree waits while the ground at its foot is dug by a crater, since it stands at the height its file gives; it grows once the crater has healed.

A tree left a long time comes back one stage a check, each out of sight.

## State and wire

- `World::destroyed` is a list of `Felled { object, yaw, since, stage }`; `felled` is the same set to ask of. Whole again, a thing leaves both, and its health is forgotten.
- The physics gets the collider back with `Physics::restore`, built by `sim::tree_collider` (scaled for a young tree) or `fences::piece_collider`. The client's streamed physics finds the tree or piece in its tile (`Physics::regrow`), and a tile that goes takes what grew back on it with it. A restored collider marks the bots' walking grid out of date, as a removed one does.
- `regrown` tells everyone of each step; a joiner's `destroyed` has each entry at its `stage`.
- The client (`WorldView.regrown`, `placeTree`) takes the log away (`FallingTrees.lift`), draws the tree's instance again at its stage's size about its foot (`showTreeInstance`, which `removeTreeInstance` keeps the original matrix for), and gives the prediction its collider (`growTree`, `mendLine`). A tile that streams in later is drawn at each tree's stage. A sapling does not count as wood for the forest ambience (`treesNear`); a young tree does.
