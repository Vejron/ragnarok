import { Geometry } from '@ragnarok/engine'
import * as THREE from 'three'
import type { TileData } from '@/world'
import { CELL_HP, shellOf, type Shell } from '@/sim/carve'
import { voxelBrick, voxelFaces, type Soup } from './voxels'
import { buildingColors } from './world'

/** How much of its colour a cell right beside a hole loses to the burn, and the next one out. */
const CHAR_NEAR = 0.75
const CHAR_FAR = 0.35
/** How much of its colour a cell that has taken all but its last hit loses. */
const CHAR_DENT = 0.6
/** Soot, as a share of the wall's own colour that is left when fully burnt. */
const SOOT = 0.12
/** How much of their colour thrown pieces keep. */
const THROWN = 0.9

export interface Fractured {
  /** Walls and roof, triangle soup, with a colour per vertex. */
  positions: Float32Array
  colors: Float32Array
}

/** `color` scaled by `f`, as `0xrrggbb`. */
function pack(color: THREE.Color, f: number): number {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v * f * 255)))
  return (c(color.r) << 16) | (c(color.g) << 8) | c(color.b)
}

function arrays(soup: Soup): Fractured {
  return { positions: Float32Array.from(soup.positions), colors: Float32Array.from(soup.colors) }
}

/**
 * A building's cells (`sim/carve.ts`, cut as the server cuts them) drawn as cubes, to be redrawn
 * without the cells a blast took out and with the ones that have been shot darkened. Which cells
 * there are is fixed by the building alone, so every client sees the same house. The server's
 * collision follows the same cells.
 */
export class Fracture {
  /** The building's cells, the same as the server cuts them, or null when it has no walls. */
  readonly shell: Shell | null
  private readonly wallColor: THREE.Color
  private readonly roofColor: THREE.Color

  constructor(tile: TileData, index: number, salt: number) {
    const colors = buildingColors(tile, index, salt)
    this.wallColor = new THREE.Color(colors.wall)
    this.roofColor = new THREE.Color(colors.roof)
    this.shell = shellOf(tile, index)
  }

  private colorOf(id: number): THREE.Color {
    return this.shell?.isRoof(id) ? this.roofColor : this.wallColor
  }

  private readonly levels = new Map<number, number>()
  private readonly burnt = new Set<number>()

  /** Takes the holes in `removed` that are new since the last call into the burn. */
  private absorb(removed: ReadonlySet<number>) {
    const shell = this.shell
    if (!shell || removed.size === 0) return
    // Kept between builds and only added to for the cells gone since the last: a hole is worked out
    // once, not again at every hit on the building.
    if (this.burnt.size > removed.size) {
      this.burnt.clear()
      this.levels.clear()
    }
    const [nx, ny, nz] = shell.dims
    const reach = 2
    for (const id of removed) {
      if (this.burnt.has(id)) continue
      this.burnt.add(id)
      const x = id % nx
      const z = Math.floor(id / nx) % nz
      const y = Math.floor(id / (nx * nz))
      for (let dy = -reach; dy <= reach; dy++) {
        for (let dz = -reach; dz <= reach; dz++) {
          for (let dx = -reach; dx <= reach; dx++) {
            const [ax, ay, az] = [x + dx, y + dy, z + dz]
            if (ax < 0 || ay < 0 || az < 0 || ax >= nx || ay >= ny || az >= nz) continue
            const near = shell.index(ax, ay, az)
            if (removed.has(near) || !shell.isSolid(near)) continue
            const ring = Math.max(Math.abs(dx), Math.abs(dy), Math.abs(dz))
            const level = ring === 1 ? CHAR_NEAR : CHAR_FAR
            if (level > (this.levels.get(near) ?? 0)) this.levels.set(near, level)
          }
        }
      }
    }
  }

  /** How burnt cell `id` is, 0 to 1, ragged by cell. */
  private burnOf(id: number): number {
    const level = this.levels.get(id)
    if (level === undefined) return 0
    const a = Math.sin(id * 91.7 + 3.1) * 43758.5453
    return Math.min(1, level * (0.6 + 0.8 * (a - Math.floor(a))))
  }

  /**
   * How burnt each standing cell is, 0 to 1: the cells touching a hole are black and the ring beyond
   * is half way, ragged by cell. Worked out from the cells that are gone alone, so every client, and a
   * player who joined after the blast, sees the same.
   */
  private burn(removed: ReadonlySet<number>): Map<number, number> {
    this.absorb(removed)
    const out = new Map<number, number>()
    for (const cell of this.levels.keys()) if (!removed.has(cell)) out.set(cell, this.burnOf(cell))
    return out
  }

  /** The bricks (`BRICK` cells a side, numbered from 0) holding a cell, or within the burn's reach of it, for each of `cells`. */
  bricksNear(cells: Iterable<number>): Set<number> {
    const out = new Set<number>()
    const shell = this.shell
    if (!shell) return out
    for (const id of cells) {
      const at = shell.coords(id)
      const lo = at.map((v) => Math.max(0, Math.floor((v - 2) / BRICK)))
      const hi = at.map((v, a) => Math.min(Math.floor((shell.dims[a]! - 1) / BRICK), Math.floor((v + 2) / BRICK)))
      for (let y = lo[1]!; y <= hi[1]!; y++) for (let z = lo[2]!; z <= hi[2]!; z++) for (let x = lo[0]!; x <= hi[0]!; x++) out.add(this.brick(x, y, z))
    }
    return out
  }

  /** Every brick of the building. */
  allBricks(): Set<number> {
    const out = new Set<number>()
    const shell = this.shell
    if (!shell) return out
    const n = shell.dims.map((d) => Math.ceil(d / BRICK))
    for (let y = 0; y < n[1]!; y++) for (let z = 0; z < n[2]!; z++) for (let x = 0; x < n[0]!; x++) out.add(this.brick(x, y, z))
    return out
  }

  private brick(x: number, y: number, z: number): number {
    const shell = this.shell!
    const nx = Math.ceil(shell.dims[0] / BRICK)
    const nz = Math.ceil(shell.dims[2] / BRICK)
    return (y * nz + z) * nx + x
  }

  /**
   * Brick `key` of the building as `build` draws it, in world coordinates; null when nothing stands in it.
   * Call `bricksNear` for the cells that changed first: the holes are taken into the burn here.
   */
  buildBrick(key: number, removed: ReadonlySet<number>, dents: ReadonlyMap<number, number>): Fractured | null {
    const shell = this.shell
    if (!shell) return null
    this.absorb(removed)
    const nx = Math.ceil(shell.dims[0] / BRICK)
    const nz = Math.ceil(shell.dims[2] / BRICK)
    const x = key % nx
    const z = Math.floor(key / nx) % nz
    const y = Math.floor(key / (nx * nz))
    const lo = [x * BRICK, y * BRICK, z * BRICK]
    const hi = [lo[0]! + BRICK, lo[1]! + BRICK, lo[2]! + BRICK]
    const shade = (id: number, extra = 0) => 1 - Math.min(1, this.burnOf(id) + extra) * (1 - SOOT)
    const worn = (id: number) => Math.min(0.95, (dents.get(id) ?? 0) / CELL_HP) * CHAR_DENT
    const soup: Soup = { positions: [], colors: [] }
    voxelBrick(shell, lo, hi, (id) => !removed.has(id), (id) => pack(this.colorOf(id), shade(id, worn(id))), soup)
    return soup.positions.length === 0 ? null : arrays(soup)
  }

  /**
   * The building as it looks without the cells in `removed`, and with those in `dents` (damage a
   * cell has taken, by cell) darkened by how much of its health is spent. A cell keeps its size.
   */
  build(removed: ReadonlySet<number> = new Set(), dents: ReadonlyMap<number, number> = new Map()): Fractured {
    const soup: Soup = { positions: [], colors: [] }
    const shell = this.shell
    if (!shell) return arrays(soup)
    const burnt = this.burn(removed)
    const shade = (id: number, extra = 0) => 1 - Math.min(1, (burnt.get(id) ?? 0) + extra) * (1 - SOOT)
    const standing = (id: number) => shell.isSolid(id) && !removed.has(id)
    const worn = (id: number) => Math.min(0.95, (dents.get(id) ?? 0) / CELL_HP) * CHAR_DENT
    voxelFaces(shell, standing, (id) => pack(this.colorOf(id), shade(id, worn(id))), soup)
    return arrays(soup)
  }

  /**
   * The cells in `cells` on their own, to be thrown: triangle soup around their centre, which is
   * `pivot`. Null when those cells hold nothing.
   */
  chunk(cells: ReadonlySet<number>): (Fractured & { pivot: THREE.Vector3 }) | null {
    const shell = this.shell
    if (!shell) return null
    const pivot = new THREE.Vector3()
    let n = 0
    for (const id of cells) {
      if (!shell.isSolid(id)) continue
      const [x, y, z] = shell.centre(id)
      pivot.x += x / 100
      pivot.y += y / 100
      pivot.z += z / 100
      n++
    }
    if (n === 0) return null
    pivot.divideScalar(n)
    const soup: Soup = { positions: [], colors: [] }
    voxelFaces(shell, (id) => cells.has(id) && shell.isSolid(id), (id) => pack(this.colorOf(id), THROWN), soup, [pivot.x, pivot.y, pivot.z], cells)
    return { ...arrays(soup), pivot }
  }
}

/** A flat-shaded geometry from triangle soup with a colour per vertex. */
export function shardGeometry(positions: Float32Array, colors: Float32Array): Geometry {
  // Not indexed, so every face has its own normal: flat shading.
  const geometry = new Geometry({ positions, colors })
  geometry.computeNormals()
  return geometry
}
