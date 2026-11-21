import { TREE_BIRCH, TREE_BROADLEAF, TREE_COLUMNAR, TREE_LOWWIDE, TREE_ORNAMENTAL, TREE_PINE, TREE_SPRUCE, type Tree } from '@/world'

/*
 * Which forest stands where on a tile, from the trees themselves. The importer has already given every
 * tree its species from Naturvårdsverket's layers (`crates/osm-import/src/species.rs`), so the stand
 * about a spot is the crowns about it: pine, spruce or broadleaf. The forest's floor follows it
 * (FOREST-FLOOR.md): what is painted (`paintCover`), what grass grows (`grassMask`) and what stands
 * in it (`forestfloor.ts`). Nothing here touches the scene or the DOM, so it runs in the tile worker.
 */

/** A forest's floor, by the trees over it. `FLOOR_MIXED` is a wood with too few trees near to say. */
export const FLOOR_MIXED = 0
/** Pine heath: dry, open ground of lingonberry, heather and reindeer lichen. */
export const FLOOR_HEATH = 1
/** Spruce: deep shade over a carpet of moss and bilberry. */
export const FLOOR_MOSS = 2
/** Birch and the other broadleaves: grass, ferns and last year's leaves. */
export const FLOOR_LEAF = 3
/** The floors a stand can have, in the order of `Forest.shares`. */
export const STAND_FLOORS = [FLOOR_HEATH, FLOOR_MOSS, FLOOR_LEAF] as const

/** Side of a cell of the forest's grid, metres: a stand is a few crowns across. */
export const FOREST_CELL = 8
/** Crown cover in a cell and its neighbours, square metres of crown radius squared, at which the stand is surely what its trees say. */
const FULL_COVER = 24

/** Which stand (index into `STAND_FLOORS`) each archetype counts toward; a rock, a bush or a flag toward none. */
const STAND_OF: Record<number, number> = {
  [TREE_PINE]: 0,
  [TREE_SPRUCE]: 1,
  [TREE_BIRCH]: 2,
  [TREE_BROADLEAF]: 2,
  [TREE_ORNAMENTAL]: 2,
  [TREE_COLUMNAR]: 2,
  [TREE_LOWWIDE]: 2,
}

/** The stand (index into `STAND_FLOORS`) a tree of `kind` counts toward, or undefined for what is no tree. */
export const standOf = (kind: number): number | undefined => STAND_OF[kind]

/** Numbers a tree in `woodsOf`: x, z, crown radius (metres) and `TREE_*`. */
export const WOOD_STRIDE = 4

/**
 * A tile's trees as the forest's floor needs them, `WOOD_STRIDE` numbers a tree: small enough to send
 * to the tile worker, which has no other use for the trees. Rocks, bushes and flags are left out.
 */
export function woodsOf(trees: readonly Pick<Tree, 'x' | 'z' | 'crown' | 'kind'>[]): Float32Array {
  const kept = trees.filter((tree) => STAND_OF[tree.kind] !== undefined)
  const out = new Float32Array(kept.length * WOOD_STRIDE)
  kept.forEach((tree, i) => out.set([tree.x, tree.z, tree.crown, tree.kind], i * WOOD_STRIDE))
  return out
}

/** The stands of one tile, on a grid of `FOREST_CELL` cells from its `(x0, z0)` corner. */
export interface Forest {
  x0: number
  z0: number
  /** Cells a side. */
  n: number
  /**
   * For each cell, how much of the ground is each of `STAND_FLOORS`, 0 to 1: the stand's shares of the
   * crown cover, scaled down where the trees are few. What is left to 1 is `FLOOR_MIXED`.
   */
  shares: Float32Array
}

const forests = new WeakMap<Float32Array, Forest>()

/**
 * The stands of the tile at `(x0, z0)`, `size` metres a side, from its `woods` (`woodsOf`). Each
 * tree counts by the area of its crown, in its own cell and the eight about it. Worked out once a
 * tile and kept.
 */
export function forestOf(woods: Float32Array, x0: number, z0: number, size: number): Forest {
  const made = forests.get(woods)
  if (made) return made
  const n = Math.max(1, Math.ceil(size / FOREST_CELL))
  const cover = new Float32Array(n * n * 3)
  for (let i = 0; i < woods.length; i += WOOD_STRIDE) {
    const stand = STAND_OF[woods[i + 3]!]
    if (stand === undefined) continue
    const column = Math.floor((woods[i]! - x0) / FOREST_CELL), row = Math.floor((woods[i + 1]! - z0) / FOREST_CELL)
    if (column < 0 || row < 0 || column >= n || row >= n) continue
    cover[(row * n + column) * 3 + stand]! += woods[i + 2]! ** 2
  }
  const shares = new Float32Array(n * n * 3)
  const sum = [0, 0, 0]
  for (let row = 0; row < n; row++) {
    for (let column = 0; column < n; column++) {
      sum.fill(0)
      for (let dr = -1; dr <= 1; dr++) {
        for (let dc = -1; dc <= 1; dc++) {
          const r = row + dr, c = column + dc
          if (r < 0 || c < 0 || r >= n || c >= n) continue
          // The cell's own trees count double: the stand is theirs first.
          const weight = dr === 0 && dc === 0 ? 2 : 1
          for (let k = 0; k < 3; k++) sum[k]! += cover[(r * n + c) * 3 + k]! * weight
        }
      }
      const total = sum[0]! + sum[1]! + sum[2]!
      if (total <= 0) continue
      const sure = Math.min(1, total / FULL_COVER)
      for (let k = 0; k < 3; k++) shares[(row * n + column) * 3 + k] = (sum[k]! / total) * sure
    }
  }
  const forest = { x0, z0, n, shares }
  forests.set(woods, forest)
  return forest
}

/** The shares of `STAND_FLOORS` at `(x, z)`, between the cells' middles, into `out`. */
export function standAt(forest: Forest, x: number, z: number, out: [number, number, number]): [number, number, number] {
  const { n, shares } = forest
  const u = Math.min(n - 1, Math.max(0, (x - forest.x0) / FOREST_CELL - 0.5)), v = Math.min(n - 1, Math.max(0, (z - forest.z0) / FOREST_CELL - 0.5))
  const c = Math.min(n - 2, Math.floor(u)), r = Math.min(n - 2, Math.floor(v))
  if (n < 2) return (out[0] = shares[0]!), (out[1] = shares[1]!), (out[2] = shares[2]!), out
  const fu = u - c, fv = v - r
  for (let k = 0; k < 3; k++) {
    const at = (dc: number, dr: number) => shares[((r + dr) * n + c + dc) * 3 + k]!
    out[k] = (at(0, 0) * (1 - fu) + at(1, 0) * fu) * (1 - fv) + (at(0, 1) * (1 - fu) + at(1, 1) * fu) * fv
  }
  return out
}

const scratch: [number, number, number] = [0, 0, 0]

/**
 * The floor (`FLOOR_*`) at `(x, z)`: one of the stands there, drawn by `pick` (0 to 1) against their
 * shares, so a mixed stand's floor is a patchwork of its trees' and the line between two stands is not
 * a line.
 */
export function floorAt(forest: Forest, x: number, z: number, pick: number): number {
  const shares = standAt(forest, x, z, scratch)
  let left = pick
  for (let k = 0; k < 3; k++) if ((left -= shares[k]!) < 0) return STAND_FLOORS[k]!
  return FLOOR_MIXED
}
