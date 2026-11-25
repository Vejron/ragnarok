import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { Craters, InstancedMesh } from '@ragnarok/engine'
import { OBJ_LINE, OBJ_TREE } from '@/net/codec'
import { GROWN_LOG, GROWN_SAPLING, GROWN_WHOLE, GROWN_YOUNG, TREE_SAPLING_SCALE, TREE_YOUNG_SCALE } from '@/sim/consts'
import { Terrain } from '@/sim/terrain'
import { WorldView, tileKey } from '@/worldview'
import { decodeManifest, decodeTile, TREE_BUSH, TREE_FLAG, type TileData, type TileRef } from '@/world'

// The committed synthetic world (`fixtures/world/`).
const read = (path: string) => new Uint8Array(readFileSync(resolve(process.cwd(), '../fixtures/world', path)))

/** A view of the fixture city with every tile decoded; the houses are left out, as in `craters.spec.ts`. */
function city() {
  const world = decodeManifest(read('world.msgpack'))
  const scene = { add: () => {}, craters: new Craters() }
  const view = new WorldView(scene as never)
  view.world = world
  view.terrain = new Terrain(world.halfExtent, world.tileSize)
  const tiles = new Map<TileRef, TileData>()
  for (const ref of world.tiles) tiles.set(ref, { ...decodeTile(read(`tiles/${ref.col}_${ref.row}.msgpack`)), buildings: [] })
  return { view, tiles }
}

/** A tile of the fixture with a tree that is drawn among its instances, and that tree's number. */
function aTree(tiles: Map<TileRef, TileData>) {
  for (const [ref, tile] of tiles) {
    const index = tile.trees.findIndex((tree) => tree.kind !== TREE_BUSH && tree.kind !== TREE_FLAG && tree.solid)
    if (index >= 0) return { ref, tile, index }
  }
  throw new Error('the fixture has no tree')
}

/** How big tree `index` of `ref` is drawn, as a share of its size (the length of its instance's x axis), or null if it is not drawn. */
function drawnSize(view: WorldView, ref: TileRef, index: number, whole: number) {
  const group = (view as unknown as { groups: Map<number, { children: unknown[] }> }).groups.get(tileKey(ref.col, ref.row))!
  for (const child of group.children) {
    const slot = child instanceof InstancedMesh ? (child.userData.treeSlots as Map<number, number> | undefined)?.get(index) : undefined
    if (slot !== undefined) return Math.hypot(...(child as InstancedMesh).matrices.slice(slot * 16, slot * 16 + 3)) / whole
  }
  return null
}

/** How many things a tile group holds besides its instanced meshes: a lying log is one. */
const logs = (view: WorldView, ref: TileRef) => (view as unknown as { groups: Map<number, { children: unknown[] }> }).groups.get(tileKey(ref.col, ref.row))!.children.filter((c) => !(c instanceof InstancedMesh)).length

const felled = (ref: TileRef, index: number, stage = GROWN_LOG) => ({ kind: OBJ_TREE, col: ref.col, row: ref.row, index, yaw: 0, force: 0, stage })
const regrown = (ref: TileRef, index: number, stage: number) => ({ kind: OBJ_TREE, col: ref.col, row: ref.row, index, stage })

describe('regrowth', () => {
  it('takes a felled tree from a log through a sapling and a young tree to whole', () => {
    const { view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    const { ref, index } = aTree(tiles)
    const whole = drawnSize(view, ref, index, 1)!
    const before = logs(view, ref)
    view.destroyed([felled(ref, index)], false)
    expect(drawnSize(view, ref, index, whole)).toBe(0)
    expect(logs(view, ref)).toBe(before + 1)
    view.regrown([regrown(ref, index, GROWN_SAPLING)])
    expect(logs(view, ref)).toBe(before)
    expect(drawnSize(view, ref, index, whole)).toBeCloseTo(TREE_SAPLING_SCALE)
    view.regrown([regrown(ref, index, GROWN_YOUNG)])
    expect(drawnSize(view, ref, index, whole)).toBeCloseTo(TREE_YOUNG_SCALE)
    view.regrown([regrown(ref, index, GROWN_WHOLE)])
    expect(drawnSize(view, ref, index, whole)).toBeCloseTo(1)
    expect(view.pristine).toBe(true)
  })

  it('shows a tile that streams in later each tree at the stage it has grown to', () => {
    const { view, tiles } = city()
    const { ref, tile, index } = aTree(tiles)
    // A joiner's catch-up, before the tile is here.
    view.destroyed([felled(ref, index, GROWN_YOUNG)], false)
    view.attachTile(ref, tile)
    const fresh = city()
    fresh.view.attachTile(ref, tile)
    const whole = drawnSize(fresh.view, ref, index, 1)!
    expect(drawnSize(view, ref, index, whole)).toBeCloseTo(TREE_YOUNG_SCALE)
    expect(logs(view, ref)).toBe(logs(fresh.view, ref))
  })

  it('fells a young tree again, as a smaller log', () => {
    const { view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    const { ref, index } = aTree(tiles)
    const before = logs(view, ref)
    view.destroyed([felled(ref, index)], false)
    view.regrown([regrown(ref, index, GROWN_SAPLING)])
    // A sapling is not felled: the server never says so, and the view would not show it.
    view.regrown([regrown(ref, index, GROWN_YOUNG)])
    view.destroyed([felled(ref, index)], false)
    expect(drawnSize(view, ref, index, 1)).toBe(0)
    expect(logs(view, ref)).toBe(before + 1)
  })

  it('mends a broken piece of fence or wall', () => {
    const { view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    const piece = { kind: OBJ_LINE, col: 0, row: 0, index: 0, yaw: 0, force: 0, stage: GROWN_LOG }
    view.destroyed([piece], false)
    expect(view.pristine).toBe(false)
    view.regrown([{ kind: OBJ_LINE, col: 0, row: 0, index: 0, stage: GROWN_WHOLE }])
    expect(view.pristine).toBe(true)
  })

  it('counts young trees as wood and saplings not', () => {
    const { view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    const { ref, tile, index } = aTree(tiles)
    const at = tile.trees[index]!
    const all = view.treesNear(at.x, at.z, 1)
    view.destroyed([felled(ref, index)], false)
    expect(view.treesNear(at.x, at.z, 1)).toBe(all - 1)
    view.regrown([regrown(ref, index, GROWN_SAPLING)])
    expect(view.treesNear(at.x, at.z, 1)).toBe(all - 1)
    view.regrown([regrown(ref, index, GROWN_YOUNG)])
    expect(view.treesNear(at.x, at.z, 1)).toBe(all)
  })
})
