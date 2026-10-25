import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { Mesh } from '@ragnarok/engine'
import { Grass, chunkBlades } from '@/render/grass'
import { Terrain } from '@/sim/terrain'
import { WorldView } from '@/worldview'
import { decodeManifest, decodeTile, type TileData, type TileRef } from '@/world'

// The committed synthetic world (`fixtures/world/`).
const read = (path: string) => new Uint8Array(readFileSync(resolve(process.cwd(), '../fixtures/world', path)))

/** A view of the fixture city with every tile attached, and the groups the scene was given. */
function city() {
  const world = decodeManifest(read('world.msgpack'))
  const groups: unknown[] = []
  const view = new WorldView({ add: (group: unknown) => groups.push(group) } as never)
  view.world = world
  view.terrain = new Terrain(world.halfExtent, world.tileSize)
  const tiles = new Map<TileRef, TileData>()
  for (const ref of world.tiles) tiles.set(ref, decodeTile(read(`tiles/${ref.col}_${ref.row}.msgpack`)))
  return { world, view, tiles }
}

/** A crater centred on `(x, z)` metres, in the server's whole centimetres. */
const crater = (x: number, z: number, radius = 600, depth = 150) => ({ x: Math.round(x * 100), z: Math.round(z * 100), radius, depth })

describe('craters', () => {
  it('dent the ground of the tiles they reach, and leave the file alone', () => {
    const { view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    // On the seam between two tiles, so both have to move together.
    const [x, z] = [-50, 20]
    const before = view.terrain!.heightAt(x, z)
    const fileHeights = new Map([...tiles.values()].map((tile) => [tile, tile.ground.slice()]))
    view.cratered([crater(x, z)], true)
    const after = view.terrain!.heightAt(x, z)
    expect(before - after).toBeGreaterThan(0.3)
    // Both sides of the seam agree.
    expect(view.terrain!.heightAt(x - 0.01, z)).toBeCloseTo(view.terrain!.heightAt(x + 0.01, z), 1)
    // Far away it is as it was; the tile files are untouched (craters are measured from them).
    for (const [tile, heights] of fileHeights) expect(tile.ground).toEqual(heights)
  })

  it('are dug into a tile that streams in after them, as if it had been here', () => {
    const early = city()
    for (const [ref, tile] of early.tiles) early.view.attachTile(ref, tile)
    const [x, z] = [10, 10]
    early.view.cratered([crater(x, z), crater(x + 3, z, 400, 80)], true)

    const late = city()
    late.view.cratered([crater(x, z), crater(x + 3, z, 400, 80)], false)
    for (const [ref, tile] of late.tiles) late.view.attachTile(ref, tile)
    for (const [dx, dz] of [[0, 0], [3, 0], [-2, 4], [6, -3]] as const) {
      expect(late.view.terrain!.heightAt(x + dx, z + dz)).toBe(early.view.terrain!.heightAt(x + dx, z + dz))
    }
  })

  it('redraw the terrain mesh of a tile they reach', () => {
    const { world, view, tiles } = city()
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    const [x, z] = [10, 10]
    const ref = world.tiles.find((r) => r.col === Math.floor((x + world.halfExtent) / world.tileSize) && r.row === Math.floor((z + world.halfExtent) / world.tileSize))!
    const group = (view as unknown as { groups: Map<number, { children: unknown[] }> }).groups.get(ref.row * 65536 + ref.col)!
    const mesh = () => group.children.find((child) => child instanceof Mesh && child.userData.ground) as Mesh
    /** Vertices of the terrain mesh below `y`. */
    const below = (y: number) => {
      let count = 0
      for (let i = 1; i < mesh().geometry.positions.length; i += 3) if (mesh().geometry.positions[i]! < y) count++
      return count
    }
    const top = view.terrain!.heightAt(x, z)
    const before = below(top - 2)
    view.cratered([crater(x, z, 600, 300)], true)
    // The bowl is in the mesh, and in the ground the physics is given.
    expect(below(top - 2)).toBeGreaterThan(before)
    expect(view.terrain!.heightAt(x, z)).toBeCloseTo(top - 3, 1)
  })

  it('clear the grass from the bowl, and grow the rest on the new ground', () => {
    const { world, view, tiles } = city()
    const terrain = view.terrain!
    const grass = new Grass(world.halfExtent, world.tileSize, (x, z) => terrain.sample(x, z))
    view.grass = grass
    for (const [ref, tile] of tiles) view.attachTile(ref, tile)
    // Somewhere the mask has grass: scan the open ground for a point that grows it.
    let spot: [number, number] | null = null
    for (let x = -100; x < 100 && !spot; x += 7) for (let z = -100; z < 100 && !spot; z += 7) if (grass.growsAt(x, z) !== 0) spot = [x, z]
    expect(spot, 'the fixture has grass somewhere').not.toBeNull()
    const [x, z] = spot!
    view.cratered([crater(x, z, 600, 150)], true)
    expect(grass.growsAt(x, z)).toBe(0)
    expect(grass.growsAt(x + 3, z)).toBe(0)
    // Blades grown now stand on the dented ground, and none grow in the bowl.
    const blades = chunkBlades(Math.floor(x / 16), Math.floor(z / 16), 12, 1, (bx, bz) => grass.growsAt(bx, bz), (bx, bz) => terrain.sample(bx, bz))
    expect(blades.every((b) => Math.hypot(b.x - x, b.z - z) >= 4.8)).toBe(true)
    for (const b of blades) expect(b.y).toBeCloseTo(terrain.heightAt(b.x, b.z), 5)
  })
})
