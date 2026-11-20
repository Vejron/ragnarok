import { NO_WATER, type TileRef } from '@/world'
import type { TileGround } from './terrain'
import { churn, FLOW_STEP, REACH_STEP, shore, wave } from './waves'

/** One tile's water surface heights, laid out like its ground. */
interface TileWater {
  ground: TileGround
  levels: Float32Array
  /** How wide the open water is at each sample, in `REACH_STEP`s (`TileData.reach`); null for none. */
  reach: Uint8Array | null
  /** Which way and how fast the water runs at each sample, `x` then `z` in `FLOW_STEP`s (`TileData.flow`); null where it is still. */
  flow: Int8Array | null
}

/** A boulder just under a river's surface (`Waters.rocksNear`): where it is, how deep its top lies (metres) and the current over it (m/s). */
export interface Rock {
  x: number
  z: number
  depth: number
  flow: [number, number]
}

/** How far from the camera, metres, and in how many directions `planeNear` looks for water. */
const PLANE_RADII = [6, 20, 45, 90, 160]
const PLANE_RAYS = 12
/**
 * Water within this of the mirrored plane, metres, shows the scene in it (the water shader fades it out
 * from 0.2 m to 0.5 m off): `planeNear` counts every level within it as the same water, so a river
 * falling gently past the camera is one surface and not a slice every 10 cm.
 */
const PLANE_BAND = 0.3
/** The level mirrored last frame stays while it has at least this share of the best level's votes. */
const PLANE_KEEP = 0.8

/**
 * The streamed-in tiles' water surface, to put splashes at the surface and not on the carved bed,
 * with or without its waves (`surfaceAt`). The sim has its own copy, which vehicles ride.
 */
export class Waters {
  private readonly tiles = new Map<number, TileWater>()
  /** No waves: the water is ice. */
  calm = false

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
  ) {}

  private key(col: number, row: number) {
    return row * 65536 + col
  }

  set(ref: TileRef, ground: TileGround | null, levels: Float32Array, reach?: Uint8Array, flow?: Int8Array) {
    if (ground && levels.length === ground.heights.length) {
      this.tiles.set(this.key(ref.col, ref.row), { ground, levels, reach: reach && reach.length === levels.length ? reach : null, flow: flow && flow.length === 2 * levels.length ? flow : null })
    }
  }

  /**
   * The current at `(x, z)`, `[x, z]` m/s, between the four samples around it, as the sim has it (`Waters::flow_at`):
   * what carries a swimmer, so what is taken off their speed to pace their strokes. Still where the tile has none.
   */
  flowAt(x: number, z: number): [number, number] {
    const tile = this.tileAt(x, z)
    if (!tile?.flow) return [0, 0]
    const { ground, flow } = tile
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    const out: [number, number] = [0, 0]
    for (let k = 0; k < 2; k++) {
      const at = (i: number, j: number) => flow[2 * (Math.min(j, last) * ground.side + Math.min(i, last)) + k]! * FLOW_STEP
      const near = at(i, j) + (at(i + 1, j) - at(i, j)) * fu
      const far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu
      out[k] = near + (far - near) * fv
    }
    return out
  }

  /**
   * The boulders that lie just under a river's surface within `reach` metres of `(x, z)`: the samples
   * where the water is no deeper than `depth` metres and no deeper than at any sample beside them, and
   * runs at `speed` m/s or more. Each with how deep its top lies and the current over it. None on ice.
   */
  rocksNear(x: number, z: number, reach: number, depth: number, speed: number): Rock[] {
    const out: Rock[] = []
    if (this.calm) return out
    for (const { ground, levels, flow } of this.tiles.values()) {
      if (!flow) continue
      const { x0, z0, cell, side, heights } = ground
      const from = (v: number, v0: number) => Math.max(1, Math.ceil((v - reach - v0) / cell))
      const to = (v: number, v0: number) => Math.min(side - 2, Math.floor((v + reach - v0) / cell))
      const deep = (at: number) => (levels[at]! !== NO_WATER && levels[at]! > heights[at]! ? levels[at]! - heights[at]! : Infinity)
      for (let row = from(z, z0); row <= to(z, z0); row++) {
        for (let column = from(x, x0); column <= to(x, x0); column++) {
          const at = row * side + column
          const top = deep(at)
          if (top > depth) continue
          const flowX = flow[at * 2]! * FLOW_STEP
          const flowZ = flow[at * 2 + 1]! * FLOW_STEP
          if (Math.hypot(flowX, flowZ) < speed) continue
          let lowest = true
          for (const beside of [at - 1, at + 1, at - side, at + side, at - side - 1, at - side + 1, at + side - 1, at + side + 1]) {
            // Of two as shallow, the first counts.
            if (deep(beside) < top || (deep(beside) === top && beside < at)) lowest = false
          }
          if (lowest) out.push({ x: x0 + column * cell, z: z0 + row * cell, depth: top, flow: [flowX, flowZ] })
        }
      }
    }
    return out
  }

  delete(ref: TileRef) {
    this.tiles.delete(this.key(ref.col, ref.row))
  }

  /**
   * The surface height of the water worth mirroring from the camera at `(x, z)` looking along `(dx, dz)`:
   * the level with the most water within `PLANE_BAND` of it among sample points around the camera and
   * ahead, nearer ones counting more. `kept`, the level mirrored last frame, stays while it is nearly as
   * good, so on a river that steps down through rapids the mirror does not flip between two levels as the
   * camera turns. Null when none of them is wet, so a mirrored pass is only drawn where there is water to see.
   */
  planeNear(x: number, z: number, dx: number, dz: number, kept: number | null = null): number | null {
    const levels: number[] = []
    const weights: number[] = []
    for (const radius of PLANE_RADII) {
      for (let i = 0; i < PLANE_RAYS; i++) {
        const angle = (i / PLANE_RAYS) * Math.PI * 2
        const px = Math.cos(angle)
        const pz = Math.sin(angle)
        const level = this.sampleAt(x + px * radius, z + pz * radius)
        if (level === NO_WATER) continue
        // What lies ahead matters more than what is behind.
        levels.push(level)
        weights.push((0.5 + 0.5 * Math.max(0, px * dx + pz * dz)) / radius)
      }
    }
    if (levels.length === 0) return null
    const score = (plane: number) => levels.reduce((sum, level, k) => sum + (Math.abs(level - plane) <= PLANE_BAND ? weights[k]! : 0), 0)
    let best = levels[0]!
    let most = -1
    for (const level of levels) {
      const s = score(level)
      if (s > most) {
        best = level
        most = s
      }
    }
    if (kept !== null && score(kept) >= PLANE_KEEP * most) return kept
    // The middle of the water it wins, so it covers as much of it as it can.
    let sum = 0
    let total = 0
    levels.forEach((level, k) => {
      if (Math.abs(level - best) <= PLANE_BAND) {
        sum += level * weights[k]!
        total += weights[k]!
      }
    })
    return Math.round((sum / total) * 10) / 10
  }

  /**
   * The water surface height at `(x, z)` with its waves, `time` seconds into their loop (`waveTime`), as
   * the sim has it (`Waters::surface`): as big as the water is wide and dying away towards the shore,
   * with the surge of a rapid's standing waves on top. `NO_WATER` where there is none.
   */
  surfaceAt(x: number, z: number, time: number): number {
    const level = this.levelAt(x, z)
    if (level === NO_WATER || this.calm) return level
    const [flowX, flowZ] = this.flowAt(x, z)
    return level + wave(x, z, time, this.reachAt(x, z)).height * shore(this.depthAt(x, z)) + churn(x, z, time, flowX, flowZ).height
  }

  /** How wide the open water is at `(x, z)`, metres, from the nearest ground sample, as the sim has it; 0 where it is not known. */
  private reachAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile?.reach) return 0
    const { ground, reach } = tile
    const column = Math.min(Math.max(Math.round((x - ground.x0) / ground.cell), 0), ground.side - 1)
    const row = Math.min(Math.max(Math.round((z - ground.z0) / ground.cell), 0), ground.side - 1)
    return reach[row * ground.side + column]! * REACH_STEP
  }

  /** How deep the water is at `(x, z)`, metres, between the four samples around it; 0 where it is dry. */
  private depthAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return 0
    const { ground, levels } = tile
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    const at = (i: number, j: number) => {
      const index = Math.min(j, last) * ground.side + Math.min(i, last)
      const level = levels[index]!
      return level !== NO_WATER && level > ground.heights[index]! ? level - ground.heights[index]! : 0
    }
    const near = at(i, j) + (at(i + 1, j) - at(i, j)) * fu
    const far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu
    return near + (far - near) * fv
  }

  private tileAt(x: number, z: number): TileWater | undefined {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const cell = (v: number) => Math.min(Math.max(Math.floor((v + this.halfExtent) / this.tileSize), 0), grid - 1)
    return this.tiles.get(this.key(cell(x), cell(z)))
  }

  /**
   * The water level of the ground sample nearest `(x, z)`, or `NO_WATER` where it is dry: what `planeNear`
   * votes on, so a rapid's ramp counts as the levels above and below it and not as a level of its own.
   */
  private sampleAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return NO_WATER
    const { ground, levels } = tile
    const column = Math.round((x - ground.x0) / ground.cell)
    const row = Math.round((z - ground.z0) / ground.cell)
    if (column < 0 || row < 0 || column >= ground.side || row >= ground.side) return NO_WATER
    const index = row * ground.side + column
    const level = levels[index]!
    return level !== NO_WATER && level > ground.heights[index]! ? level : NO_WATER
  }

  /**
   * The still water surface's height at `(x, z)`, or `NO_WATER` where the nearest sample is dry (or the tile is
   * not loaded). A river's surface falls along it and is shaped by its current, so the height is read between
   * the wet samples of the four around the point, as the sim has it (`Waters::level_at`).
   */
  levelAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return NO_WATER
    const { ground, levels } = tile
    const column = Math.round((x - ground.x0) / ground.cell)
    const row = Math.round((z - ground.z0) / ground.cell)
    if (column < 0 || row < 0 || column >= ground.side || row >= ground.side) return NO_WATER
    // Water at or under the bed is dry land that the importer left a level on.
    const wet = (index: number) => levels[index]! !== NO_WATER && levels[index]! > ground.heights[index]!
    if (!wet(row * ground.side + column)) return NO_WATER
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    let sum = 0
    let weight = 0
    for (const [di, dj, share] of [[0, 0, (1 - fu) * (1 - fv)], [1, 0, fu * (1 - fv)], [0, 1, (1 - fu) * fv], [1, 1, fu * fv]] as const) {
      const index = Math.min(j + dj, last) * ground.side + Math.min(i + di, last)
      if (wet(index)) {
        sum += levels[index]! * share
        weight += share
      }
    }
    return weight > 0 ? sum / weight : levels[row * ground.side + column]!
  }
}
