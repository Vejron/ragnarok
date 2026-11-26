import { Color, Group, type Geometry } from '@ragnarok/engine'
import type { TileRef } from '@/world'
import { ChunkLayer, FloorBuilder, addShrub, unit, type Shrub } from './forestfloor'
import { random } from './grass'
import { FLOWER_COLORS, GARDEN_FLOWERS, GARDEN_RHUBARB, ITEM_STRIDE, type GardenPlan } from './gardenplan'
import { CraterGrid } from './cratergrid'

/*
 * The gardens round houses (GARDENS.md): rhubarb and the flowers of a bed, where the tile's
 * `gardenPlan` put them. All for show, as the forest's floor is: nothing here is solid and the server
 * knows none of it. Drawn in chunks about the camera by a `ChunkLayer`, the flowers thinning before
 * the rhubarb goes.
 */

/** How far the gardens are drawn: everything within `full` metres, the flowers thinning past it, all of it gone between `from` and `far`. */
export interface GardenReach {
  full: number
  from: number
  far: number
}

export const GARDENS_FULL: GardenReach = { full: 30, from: 60, far: 80 }
export const GARDENS_LITE: GardenReach = { full: 20, from: 40, far: 55 }

/** Side of a chunk, metres. */
const CHUNK = 32

const hex = (color: number) => Color.hex(color)

const RHUBARB_LEAF = hex(0x4c8a2e)
const RHUBARB_LEAF_LIGHT = hex(0x6aa83e)
const RHUBARB_STALK = hex(0xa8343c)
const STEM = hex(0x3e6e2e)
const STEM_LIGHT = hex(0x5c9440)

/** Where a thing stands, for the vertices to shrink into, and how much it sways. */
interface Root {
  x: number
  y: number
  z: number
  rank: number
  sway: number
}

/**
 * A rhubarb plant, `radius` across and `height` high: five to seven leaves, each a broad, crumpled
 * blade held out on a red stalk.
 */
export function addRhubarb(out: FloorBuilder, x: number, y: number, z: number, radius: number, height: number, next: () => number) {
  const root: Root = { x, y: y - 0.03, z, rank: 0, sway: 0.4 }
  const leaves = 5 + Math.floor(next() * 3)
  const turn = next() * Math.PI * 2
  for (let k = 0; k < leaves; k++) {
    const angle = turn + (k / leaves) * Math.PI * 2 + next() * 0.4
    const [ox, oz] = [Math.cos(angle), Math.sin(angle)]
    const reach = radius * (0.45 + 0.25 * next())
    const lift = height * (0.75 + 0.35 * next())
    // The stalk: a thin red strip from the crown to the leaf.
    const [sx, sz] = [-oz * 0.025, ox * 0.025]
    const [ex, ey, ez] = [x + ox * reach, y + lift, z + oz * reach]
    const first = out.vertices
    out.vertex([x - sx, y, z - sz], [0, 1, 0], RHUBARB_STALK, 0.8, root.sway, root.x, root.y, root.z, 0)
    out.vertex([x + sx, y, z + sz], [0, 1, 0], RHUBARB_STALK, 0.8, root.sway, root.x, root.y, root.z, 0)
    out.vertex([ex - sx, ey, ez - sz], [0, 1, 0], RHUBARB_STALK, 1, root.sway, root.x, root.y, root.z, 0)
    out.vertex([ex + sx, ey, ez + sz], [0, 1, 0], RHUBARB_STALK, 1, root.sway, root.x, root.y, root.z, 0)
    out.indices.push(first, first + 2, first + 1, first + 1, first + 2, first + 3)
    // The blade: a fan round a point a little out from the stalk's end, tipped down at its rim, crumpled.
    const size = radius * (0.42 + 0.2 * next())
    const [lx, ly, lz] = [ex + ox * size * 0.55, ey + 0.03, ez + oz * size * 0.55]
    const tilt = 0.35 + 0.2 * next()
    const centre = out.vertex([lx, ly, lz], unit(ox * 0.2, 1, oz * 0.2), RHUBARB_LEAF_LIGHT, 1.05, root.sway, root.x, root.y, root.z, 0)
    const rim = 8
    for (let r = 0; r < rim; r++) {
      const a = angle + (r / rim) * Math.PI * 2
      const [rx, rz] = [Math.cos(a), Math.sin(a)]
      // Longer out from the stalk than across it, and dropping away at the far side.
      const along = rx * ox + rz * oz
      const length = size * (0.85 + 0.25 * along + 0.15 * next())
      const drop = (tilt * (0.6 + 0.4 * along) + (next() - 0.5) * 0.25) * length * 0.6
      out.vertex([lx + rx * length, ly - drop, lz + rz * length], unit(rx * 0.35, 1, rz * 0.35), r % 2 ? RHUBARB_LEAF : RHUBARB_LEAF_LIGHT, 0.85 + 0.2 * next(), root.sway, root.x, root.y, root.z, 0)
    }
    for (let r = 0; r < rim; r++) out.indices.push(centre, centre + 1 + r, centre + 1 + ((r + 1) % rim))
  }
}

/** A clump of a bed's flowers: leafy stems, and over them heads of `color`, a small star a stem. */
export function addFlowers(out: FloorBuilder, x: number, y: number, z: number, radius: number, color: Color, rank: number, next: () => number) {
  const stems: Shrub = { radius: [radius * 0.7, radius], sprigs: [6, 9], length: [radius * 1.6, radius * 2.4], width: 0.45, splay: 0.45, low: STEM, high: STEM_LIGHT, sway: 0.6 }
  addShrub(out, stems, x, y, z, [0, 1, 0], rank, next)
  const root: Root = { x, y: y - 0.02, z, rank, sway: 0.6 }
  const heads = 4 + Math.floor(next() * 4)
  for (let k = 0; k < heads; k++) {
    const angle = next() * Math.PI * 2
    const out01 = Math.sqrt(next()) * radius * 0.8
    const [hx, hy, hz] = [x + Math.cos(angle) * out01, y + radius * (1.7 + next() * 0.9), z + Math.sin(angle) * out01]
    const size = 0.045 + next() * 0.035
    const turn = next() * Math.PI
    const centre = out.vertex([hx, hy + 0.01, hz], [0, 1, 0], color, 1.1, root.sway, root.x, root.y, root.z, rank)
    const petals = 5
    for (let p = 0; p < petals; p++) {
      const a = turn + (p / petals) * Math.PI * 2
      out.vertex([hx + Math.cos(a) * size, hy - 0.01, hz + Math.sin(a) * size], unit(Math.cos(a) * 0.3, 1, Math.sin(a) * 0.3), color, 0.9, root.sway, root.x, root.y, root.z, rank)
    }
    for (let p = 0; p < petals; p++) out.indices.push(centre, centre + 1 + p, centre + 1 + ((p + 1) % petals))
  }
}

const FLOWERS = FLOWER_COLORS.map(hex)

/** The gardens about the camera, over every streamed-in tile's houses. */
export class Gardens {
  readonly root = new Group()
  private readonly layer: ChunkLayer
  private reach: GardenReach = GARDENS_FULL
  private enabled = true
  /** Each tile's items (`GardenPlan.items` runs), by the chunk they stand in. */
  private readonly items = new Map<number, Map<number, number[]>>()
  private readonly craters = new CraterGrid()

  /** `heightAt` is the ground, null where it is not loaded. */
  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    private readonly heightAt: (x: number, z: number) => number | null,
  ) {
    this.layer = new ChunkLayer('gardens', CHUNK, (cx, cz) => this.build(cx, cz), true)
    this.root.add(this.layer.root)
    this.applyReach()
  }

  private applyReach() {
    this.layer.reach(this.reach.full, this.reach.from, this.reach.far)
  }

  private static tileKey(ref: Pick<TileRef, 'col' | 'row'>) {
    return ref.row * 65536 + ref.col
  }

  private static chunkKey(cx: number, cz: number) {
    return (cz + 32768) * 65536 + (cx + 32768)
  }

  /** The gardens of a streamed-in tile, as its `gardenPlan` has them. */
  addTile(ref: Pick<TileRef, 'col' | 'row'>, plan: GardenPlan | null) {
    const key = Gardens.tileKey(ref)
    this.removeTile(ref)
    if (!plan || plan.items.length === 0) return
    const { items } = plan
    for (let i = 0; i < items.length; i += ITEM_STRIDE) {
      const [cx, cz] = [Math.floor(items[i + 1]! / CHUNK), Math.floor(items[i + 2]! / CHUNK)]
      const chunk = Gardens.chunkKey(cx, cz)
      const byTile = this.items.get(chunk) ?? new Map<number, number[]>()
      this.items.set(chunk, byTile)
      const list = byTile.get(key) ?? []
      for (let k = 0; k < ITEM_STRIDE; k++) list.push(items[i + k]!)
      byTile.set(key, list)
      this.layer.staleOver(cx * CHUNK, cz * CHUNK, (cx + 1) * CHUNK, (cz + 1) * CHUNK)
    }
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    const key = Gardens.tileKey(ref)
    for (const [chunk, byTile] of this.items) {
      if (!byTile.delete(key)) continue
      const [cx, cz] = [(chunk % 65536) - 32768, Math.floor(chunk / 65536) - 32768]
      this.layer.staleOver(cx * CHUNK + 1, cz * CHUNK + 1, (cx + 1) * CHUNK - 1, (cz + 1) * CHUNK - 1)
      if (byTile.size === 0) this.items.delete(chunk)
    }
  }

  /** The ground of a tile moved: what grows on it is grown again. */
  regrowTile(ref: Pick<TileRef, 'col' | 'row'>) {
    const x0 = -this.halfExtent + ref.col * this.tileSize, z0 = -this.halfExtent + ref.row * this.tileSize
    this.layer.staleOver(x0, z0, x0 + this.tileSize, z0 + this.tileSize)
  }

  /** A shell dug a crater of `radius` metres at `(x, z)`: what grew in it is gone. */
  dent(x: number, z: number, radius: number) {
    if (!this.craters.add(x, z, radius)) return
    this.layer.staleOver(x - radius, z - radius, x + radius, z + radius)
  }

  /** The ground of a crater `dent` was told of has healed: what grew in it grows again. */
  heal(x: number, z: number, radius: number) {
    if (!this.craters.remove(x, z, radius)) return
    this.layer.staleOver(x - radius, z - radius, x + radius, z + radius)
  }

  /** Which way the wind blows, a unit `[x, z]`. */
  blowing(direction: readonly [number, number]) {
    const wind = this.layer.material.uniforms.wind as number[]
    wind[0] = direction[0]
    wind[1] = direction[1]
    this.layer.material.needsUpdate()
  }

  /** Whether the gardens are drawn: not in a winter world, whose gardens are under the snow. */
  setEnabled(on: boolean) {
    this.enabled = on
    this.root.visible = on
    if (!on) this.layer.clear()
  }

  setReach(reach: GardenReach) {
    if (reach === this.reach) return
    this.reach = reach
    this.layer.clear()
    this.applyReach()
  }

  update(x: number, z: number) {
    if (this.enabled) this.layer.update(x, z, this.reach.far)
  }

  private build(cx: number, cz: number): Geometry | null {
    const byTile = this.items.get(Gardens.chunkKey(cx, cz))
    if (!byTile) return null
    const out = new FloorBuilder()
    for (const list of byTile.values()) {
      for (let i = 0; i < list.length; i += ITEM_STRIDE) {
        const [kind, x, z, radius, size, seed] = [list[i]!, list[i + 1]!, list[i + 2]!, list[i + 3]!, list[i + 4]!, list[i + 5]!]
        if (this.craters.covers(x, z, radius * 0.5)) continue
        const y = this.heightAt(x, z)
        if (y === null) continue
        const next = random(Math.floor(seed * 4294967296))
        if (kind === GARDEN_RHUBARB) addRhubarb(out, x, y, z, radius, size, next)
        else if (kind === GARDEN_FLOWERS) addFlowers(out, x, y, z, radius, FLOWERS[size] ?? FLOWERS[0]!, seed, next)
      }
    }
    return out.vertices > 0 ? out.geometry() : null
  }

  dispose() {
    this.layer.dispose()
    this.root.removeFromParent()
  }
}
