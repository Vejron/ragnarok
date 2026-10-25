// What streaming a tile in costs, stage by stage, over the 1 m dev world (TILE-STREAMING.md, the
// baseline). Skipped unless asked for, and where the dev world has not been generated
// (`scripts/world-dev.sh`):
//
//   BENCH=1 npx vitest run tiles.bench --silent=false
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { it } from 'vitest'
import type { Mesh } from '@ragnarok/engine'
import { decodeManifest, decodeTile, worldHash, type TileData } from '@/world'
import { ClientSim } from '@/sim/wasm/sim'
import { groundOf, Terrain, type TileGround } from '@/sim/terrain'
import { rasterSurface } from '@/sim/surface'
import { buildingBuffers } from '@/render/buildingmesh'
import { grassMask } from '@/render/grass'
import { TERRAIN_RAMP } from '@/render/groundlight'
import { buildRails } from '@/render/rails'
import { buildRoads } from '@/render/roads'
import { heightSampler } from '@/render/shade'
import { terrainBuffers, TERRAIN_LOD_ERRORS } from '@/render/terrainmesh'
import { buildTrees } from '@/render/trees'
import { buildWater } from '@/render/water'

const root = resolve(process.cwd(), 'public/dev')

it.skipIf(!process.env.BENCH || !existsSync(resolve(root, 'world.msgpack')))('what a tile costs, stage by stage', { timeout: 300_000 }, () => {
  const manifest = new Uint8Array(readFileSync(resolve(root, 'world.msgpack')))
  const world = decodeManifest(manifest)
  const sim = new ClientSim(manifest)
  const tiles = readdirSync(resolve(root, 'tiles'))
    .filter((name) => name.endsWith('.msgpack'))
    .map((name) => {
      const [col, row] = name.replace('.msgpack', '').split('_').map(Number) as [number, number]
      return { col, row, bytes: new Uint8Array(readFileSync(resolve(root, 'tiles', name))) }
    })
  // Every tile's ground loaded first, so roads near an edge sample their neighbours as in the game.
  const terrain = new Terrain(world.halfExtent, world.tileSize)
  const decoded = new Map<number, { tile: TileData; ground: TileGround | null }>()
  for (const { col, row, bytes } of tiles) {
    const tile = decodeTile(bytes)
    const ref = world.tiles.find((t) => t.col === col && t.row === row)!
    const ground = groundOf(world, ref, tile)
    terrain.set(ref, ground)
    decoded.set(row * 65536 + col, { tile, ground })
  }

  const stages: Record<string, number[]> = {}
  const time = <T,>(stage: string, run: () => T): T => {
    const start = performance.now()
    const out = run()
    ;(stages[stage] ??= []).push(performance.now() - start)
    return out
  }
  const vertices = (meshes: Mesh[]) => meshes.reduce((sum, mesh) => sum + mesh.geometry.vertexCount, 0)
  const ribbonVertices: number[] = []

  for (const { col, row, bytes } of tiles) {
    time('hash', () => worldHash(bytes))
    time('decode', () => decodeTile(bytes))
    const { tile, ground } = decoded.get(row * 65536 + col)!
    time('ClientSim.addTile', () => sim.addTile(col, row, bytes))
    time('buildings', () => buildingBuffers(tile, 0, new Set()))
    if (tile.buildings.length > 0) time('buildings, one fallen', () => buildingBuffers(tile, 0, new Set([0])))
    const x0 = -world.halfExtent + col * world.tileSize
    const z0 = -world.halfExtent + row * world.tileSize
    time('surface raster', () => rasterSurface(tile, x0, z0, world.tileSize, ground))
    time('grass mask', () => grassMask(tile, x0, z0, world.tileSize, ground))
    time('trees', () => buildTrees(tile.trees))
    if (!ground) continue
    time('terrain mesh (finest)', () => terrainBuffers(ground, TERRAIN_LOD_ERRORS[0]!))
    const size = (ground.side - 1) * ground.cell
    const height = heightSampler(ground, terrain)
    const border = [ground.x0, ground.z0, ground.x0 + size, ground.z0 + size] as const
    const ribbons = time('roads + rails', () => [
      ...buildRoads(tile.roads, height, ground.cell, border, TERRAIN_RAMP),
      ...buildRails(tile.rails, height, ground.cell, border, TERRAIN_RAMP),
    ])
    ribbonVertices.push(vertices(ribbons))
    time('water', () => buildWater(tile, ground, terrain))
  }

  const summary = (values: number[]) => {
    const sorted = [...values].sort((a, b) => a - b)
    const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))]!
    return { median: at(0.5), p95: at(0.95), max: sorted[sorted.length - 1]! }
  }
  const lines = [`${tiles.length} tiles, ground every ${world.groundCell} m. ms a tile:`, `${'stage'.padEnd(24)}${'median'.padStart(9)}${'p95'.padStart(9)}${'max'.padStart(9)}`]
  for (const [stage, values] of Object.entries(stages)) {
    const { median, p95, max } = summary(values)
    lines.push(`${stage.padEnd(24)}${median.toFixed(2).padStart(9)}${p95.toFixed(2).padStart(9)}${max.toFixed(2).padStart(9)}`)
  }
  const { median, p95, max } = summary(ribbonVertices)
  lines.push(`road + rail vertices a tile: median ${median}, p95 ${p95}, max ${max}`)
  console.log(lines.join('\n'))
})
