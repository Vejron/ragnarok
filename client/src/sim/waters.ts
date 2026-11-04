import { NO_WATER, type TileRef } from '@/world'
import type { TileGround } from './terrain'
import { shore, wave } from './waves'

/** One tile's water surface heights, laid out like its ground. */
interface TileWater {
  ground: TileGround
  levels: Float32Array
}

/** How far from the camera, metres, and in how many directions `planeNear` looks for water. */
const PLANE_RADII = [6, 20, 45, 90, 160]
const PLANE_RAYS = 12

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

  set(ref: TileRef, ground: TileGround | null, levels: Float32Array) {
    if (ground && levels.length === ground.heights.length) this.tiles.set(this.key(ref.col, ref.row), { ground, levels })
  }

  delete(ref: TileRef) {
    this.tiles.delete(this.key(ref.col, ref.row))
  }

  /**
   * The surface height of the water worth mirroring from the camera at `(x, z)` looking along `(dx, dz)`:
   * the commonest level among sample points around it and ahead, nearer ones counting more. Null when none
   * of them is wet, so a mirrored pass is only drawn where there is water to see.
   */
  planeNear(x: number, z: number, dx: number, dz: number): number | null {
    const votes = new Map<number, number>()
    for (const radius of PLANE_RADII) {
      for (let i = 0; i < PLANE_RAYS; i++) {
        const angle = (i / PLANE_RAYS) * Math.PI * 2
        const px = Math.cos(angle)
        const pz = Math.sin(angle)
        const level = this.levelAt(x + px * radius, z + pz * radius)
        if (level === NO_WATER) continue
        // What lies ahead matters more than what is behind.
        const ahead = 0.5 + 0.5 * Math.max(0, px * dx + pz * dz)
        const key = Math.round(level * 10) / 10
        votes.set(key, (votes.get(key) ?? 0) + ahead / radius)
      }
    }
    let best: number | null = null
    let most = 0
    for (const [level, weight] of votes) {
      if (weight > most) {
        best = level
        most = weight
      }
    }
    return best
  }

  /**
   * The water surface height at `(x, z)` with its waves, `time` seconds into their loop (`waveTime`), as
   * the water draws it and the sim has it: they die away towards the shore. `NO_WATER` where there is none.
   */
  surfaceAt(x: number, z: number, time: number): number {
    const level = this.levelAt(x, z)
    if (level === NO_WATER || this.calm) return level
    return level + wave(x, z, time).height * shore(this.depthAt(x, z))
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

  /** The water surface height at `(x, z)`, or `NO_WATER` where there is none (or the tile is not loaded). */
  levelAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return NO_WATER
    const { ground, levels } = tile
    const column = Math.round((x - ground.x0) / ground.cell)
    const row = Math.round((z - ground.z0) / ground.cell)
    if (column < 0 || row < 0 || column >= ground.side || row >= ground.side) return NO_WATER
    const index = row * ground.side + column
    const level = levels[index]!
    // Water at or under the bed is dry land that the importer left a level on.
    return level !== NO_WATER && level > ground.heights[index]! ? level : NO_WATER
  }
}
