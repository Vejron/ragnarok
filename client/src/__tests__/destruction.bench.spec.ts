// What shelling one big house costs the client's main thread, cut by cut. The cuts come from the
// server's `demolition_cost` bench, which writes them out; skipped unless asked for:
//
//   DEMOLITION_OUT=/tmp/cuts.json RAGNAROK_WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture demolition_cost
//   BENCH=1 BENCH_WORLD=../worlds/home BENCH_CUTS=/tmp/cuts.json npx vitest run destruction.bench --silent=false
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { decodeManifest, decodeTile } from '@/world'
import { ClientSim } from '@/sim/wasm/sim'
import { BuildingDamage } from '@/render/damage'
import { Glazing } from '@/render/glazing'
import { Debris } from '@/render/debris'
import { buildTile } from '@/render/world'
import { lookFracture } from '@/render/lookfracture'
import { shellOf } from '@/sim/carve'
import { buildingColors, buildingSurfaces } from '@/render/buildingmesh'
import { Color } from '@ragnarok/engine'
import type { LookWorker } from '@/render/lookworker'

const root = resolve(process.cwd(), process.env.BENCH_WORLD ?? 'public/dev')
const cutsFile = process.env.BENCH_CUTS ?? ''

interface Cut {
  col: number
  row: number
  index: number
  point: [number, number, number]
  blasted: number[]
  fell: number[]
}

it.skipIf(!process.env.BENCH || !existsSync(cutsFile) || !existsSync(resolve(root, 'world.msgpack')))('what shelling a big house costs the main thread', { timeout: 300_000 }, () => {
  const cuts = JSON.parse(readFileSync(cutsFile, 'utf8')) as Cut[]
  const { col, row, index } = cuts[0]!
  const manifest = new Uint8Array(readFileSync(resolve(root, 'world.msgpack')))
  const world = decodeManifest(manifest)
  const bytes = new Uint8Array(readFileSync(resolve(root, 'tiles', `${col}_${row}.msgpack`)))
  const tile = decodeTile(bytes)
  const sim = new ClientSim(manifest)
  sim.addTile(col, row, bytes)
  const key = row * 65536 + col
  const group = buildTile(tile, 0)
  const damage = new BuildingDamage()
  // The look worker is off the main thread: a stand-in that never answers, so only the main thread's share is timed here.
  damage.looks = { draw: () => new Promise(() => {}), dispose: () => {} } as unknown as LookWorker
  damage.attach(key, tile)
  const glazing = new Glazing(new Debris())
  glazing.attach(key, tile, group, new Map())
  console.log(`world ${world.halfExtent * 2} m, building ${index} of tile ${col},${row}: ${tile.buildings[index]?.look ? 'with a look' : 'no look'}`)

  const stages: Record<string, number[]> = {}
  const time = <T,>(stage: string, run: () => T): T => {
    const start = performance.now()
    const out = run()
    ;(stages[stage] ??= []).push(performance.now() - start)
    return out
  }
  const removed = new Set<number>()
  const smoke = { spawn: () => {} }
  const effects = { dust: () => {} } as never
  const near = new Vector3(0, 0, 0)
  for (const cut of cuts) {
    const point = cut.point.map((v) => v / 100) as [number, number, number]
    time('BuildingDamage.carve', () => damage.carve(key, index, cut.blasted, cut.fell, point, true))
    for (const cell of [...cut.blasted, ...cut.fell]) removed.add(cell)
    time('Glazing.holed', () => glazing.holed(key, index, removed, true))
    time('collider (WASM carveBuilding)', () => {
      sim.carveBuilding(col, row, index, Uint32Array.from(removed))
      sim.refresh()
    })
    // A frame's worth of falling, and the heap once it is due.
    time('BuildingDamage.update (frame)', () => damage.update(1 / 60, effects, near, smoke))
    time('BuildingDamage.update (heap due)', () => damage.update(1.2, effects, near, smoke))
  }
  // What the look worker spends on one redraw, at the end.
  const shell = shellOf(tile, index)!
  const range = tile.buildings[index]!.look!
  const rgb = (hex: number) => {
    const c = Color.hex(hex)
    return [c.r, c.g, c.b] as const
  }
  const { wall, roof } = buildingColors(tile, index, 0)
  for (let i = 0; i < 3; i++) time('lookFracture (worker)', () => lookFracture(tile.look!, range, shell, removed, new Map(), rgb(wall), rgb(roof), buildingSurfaces(tile, index, 0)))

  const summary = (values: number[]) => {
    const sorted = [...values].sort((a, b) => a - b)
    const total = values.reduce((a, b) => a + b, 0)
    return `avg ${(total / values.length).toFixed(2)} ms, median ${sorted[sorted.length >> 1]!.toFixed(2)}, worst ${sorted[sorted.length - 1]!.toFixed(2)} (${values.length} calls)`
  }
  for (const [stage, values] of Object.entries(stages)) console.log(`${stage.padEnd(36)} ${summary(values)}`)
})
