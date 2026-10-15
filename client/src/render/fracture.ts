import { Geometry } from '@ragnarok/engine'
import * as THREE from 'three'
import type { TileData } from '@/world'
import { CELL_HP, shellOf, type Shell, type Tri } from '@/sim/carve'
import { buildingColors } from './world'

/** How far behind the wall the dark inside of the building sits, metres. */
const INSET = 0.4
/** How much of the wall's brightness the inside keeps. */
const INSIDE = 0.6
/** How far a piece of wall that has taken all but its last hit is pushed in, metres, and how far it slips sideways. */
const DENT_DEPTH = 0.22
const DENT_SLIP = 0.09
/** How much of its colour a piece of wall right beside a hole loses to the burn, and the next one out. */
const CHAR_NEAR = 0.75
const CHAR_FAR = 0.35
/** How much of its colour a piece that has taken all but its last hit loses. */
const CHAR_DENT = 0.3
/** Soot, as a share of the wall's own colour that is left when fully burnt. */
const SOOT = 0.12

export interface Fractured {
  /** Walls and roof, triangle soup, with a colour per vertex. */
  positions: Float32Array
  colors: Float32Array
}

/**
 * A building's walls and roof as the small triangles of its cells (`sim/carve.ts`, cut as the
 * server cuts them), to be redrawn without the cells a blast took out and with the ones that have
 * been shot pushed in. Which triangles are which is fixed by the building alone, so every client sees
 * the same house. The server's collision follows the same cells: this is the picture of it.
 */
export class Fracture {
  /** The building's cells, the same as the server cuts them, or null when it has no walls. */
  readonly shell: Shell | null
  /** The small triangles, nine numbers each, wound to face out, and the cell of each. */
  private readonly wall: number[] = []
  private readonly wallCell: number[] = []
  private readonly roof: number[] = []
  private readonly roofCell: number[] = []
  private readonly wallColor: THREE.Color
  private readonly roofColor: THREE.Color
  private readonly middle = new THREE.Vector3()

  constructor(tile: TileData, index: number, salt: number) {
    const building = tile.buildings[index]!
    const colors = buildingColors(tile, index, salt)
    this.wallColor = new THREE.Color(colors.wall)
    this.roofColor = new THREE.Color(colors.roof)
    this.shell = shellOf(tile, index)
    // The collision mesh is wound inwards and drawn reversed (a, c, b), so that is the order kept.
    const outward = (t: Tri) => [t[0] / 100, t[1] / 100, t[2] / 100, t[6] / 100, t[7] / 100, t[8] / 100, t[3] / 100, t[4] / 100, t[5] / 100]
    this.shell?.triangles.forEach(({ tri, cell }, i) => {
      if (i < this.shell!.walls) {
        this.wall.push(...outward(tri))
        this.wallCell.push(cell)
      } else {
        this.roof.push(...outward(tri))
        this.roofCell.push(cell)
      }
    })
    for (const p of building.footprint) this.middle.x += p[0]
    for (const p of building.footprint) this.middle.z += p[1]
    this.middle.divideScalar(Math.max(1, building.footprint.length))
  }

  /**
   * How burnt each standing cell is, 0 to 1: the cells touching a hole are black and the ring beyond
   * is half way, ragged by cell. Worked out from the cells that are gone alone, so every client, and a
   * player who joined after the blast, sees the same.
   */
  private burn(removed: ReadonlySet<number>): Map<number, number> {
    const out = new Map<number, number>()
    const shell = this.shell
    if (!shell || removed.size === 0) return out
    const [nx, ny, nz] = shell.dims
    // Big cells (large buildings) are already a lot of wall: only the first ring.
    const reach = shell.cell <= 200 ? 2 : 1
    for (const id of removed) {
      const x = id % nx
      const z = Math.floor(id / nx) % nz
      const y = Math.floor(id / (nx * nz))
      for (let dy = -reach; dy <= reach; dy++) {
        for (let dz = -reach; dz <= reach; dz++) {
          for (let dx = -reach; dx <= reach; dx++) {
            const [ax, ay, az] = [x + dx, y + dy, z + dz]
            if (ax < 0 || ay < 0 || az < 0 || ax >= nx || ay >= ny || az >= nz) continue
            const near = shell.index(ax, ay, az)
            if (removed.has(near)) continue
            const ring = Math.max(Math.abs(dx), Math.abs(dy), Math.abs(dz))
            const level = ring === 1 ? CHAR_NEAR : CHAR_FAR
            if (level > (out.get(near) ?? 0)) out.set(near, level)
          }
        }
      }
    }
    for (const [cell, level] of out) {
      const a = Math.sin(cell * 91.7 + 3.1) * 43758.5453
      out.set(cell, Math.min(1, level * (0.6 + 0.8 * (a - Math.floor(a)))))
    }
    return out
  }

  /**
   * The building as it looks without the cells in `removed`, and with those in `dents` (damage a
   * cell has taken, by cell) pushed in and slipped by how much of its health is spent.
   */
  build(removed: ReadonlySet<number> = new Set(), dents: ReadonlyMap<number, number> = new Map()): Fractured {
    const positions: number[] = []
    const colors: number[] = []
    const n = new THREE.Vector3()
    const u = new THREE.Vector3()
    const w = new THREE.Vector3()
    const burnt = this.burn(removed)
    const tint = new THREE.Color()
    const push = (source: readonly number[], at: number, cell: number, color: THREE.Color) => {
      const damage = dents.get(cell)
      const burn = Math.min(1, (burnt.get(cell) ?? 0) + (damage ? Math.min(1, damage / CELL_HP) * CHAR_DENT : 0))
      tint.copy(color).multiplyScalar(1 - burn * (1 - SOOT))
      const start = positions.length
      for (let k = 0; k < 9; k += 3) {
        positions.push(source[at + k]!, source[at + k + 1]!, source[at + k + 2]!)
        colors.push(tint.r, tint.g, tint.b)
      }
      if (!damage) return
      const f = Math.min(0.95, damage / CELL_HP)
      u.set(source[at + 3]! - source[at]!, source[at + 4]! - source[at + 1]!, source[at + 5]! - source[at + 2]!)
      w.set(source[at + 6]! - source[at]!, source[at + 7]! - source[at + 1]!, source[at + 8]! - source[at + 2]!)
      n.crossVectors(u, w).normalize()
      // The same slip for every triangle of the cell, and the same on every client.
      const a = Math.sin(cell * 12.9898) * 43758.5453
      const [jx, jz] = [(a - Math.floor(a)) * 2 - 1, (Math.sin(cell * 78.233) * 43758.5453) % 1]
      for (let k = 0; k < 9; k += 3) {
        positions[start + k]! += -n.x * DENT_DEPTH * f + jx * DENT_SLIP * f
        positions[start + k + 1]! += -n.y * DENT_DEPTH * f
        positions[start + k + 2]! += -n.z * DENT_DEPTH * f + jz * DENT_SLIP * f
      }
    }
    this.wallCell.forEach((cell, t) => !removed.has(cell) && push(this.wall, t * 9, cell, this.wallColor))
    this.roofCell.forEach((cell, t) => !removed.has(cell) && push(this.roof, t * 9, cell, this.roofColor))
    return { positions: Float32Array.from(positions), colors: Float32Array.from(colors) }
  }

  /**
   * What is inside the walls: the same walls drawn a little way in, dark, for a hole to show a room
   * and not a void, without the cells that are gone.
   */
  core(removed: ReadonlySet<number> = new Set()): { positions: Float32Array; colors: Float32Array } {
    const positions: number[] = []
    const colors: number[] = []
    const p = new THREE.Vector3()
    const toward = new THREE.Vector3()
    this.wallCell.forEach((cell, t) => {
      if (removed.has(cell)) return
      for (let k = 0; k < 3; k++) {
        p.fromArray(this.wall, t * 9 + k * 3)
        toward.set(this.middle.x - p.x, 0, this.middle.z - p.z)
        const reach = toward.length()
        if (reach > 1e-3) p.addScaledVector(toward.divideScalar(reach), Math.min(INSET, reach))
        positions.push(p.x, p.y, p.z)
        colors.push(this.wallColor.r * INSIDE, this.wallColor.g * INSIDE, this.wallColor.b * INSIDE)
      }
    })
    return { positions: Float32Array.from(positions), colors: Float32Array.from(colors) }
  }

  /**
   * The wall and roof in `cells` on their own, to be thrown: triangle soup around its centre, which
   * is `pivot`. Null when those cells hold nothing.
   */
  chunk(cells: ReadonlySet<number>): { positions: Float32Array; colors: Float32Array; pivot: THREE.Vector3 } | null {
    const pieces: { source: number[]; at: number; color: THREE.Color }[] = []
    this.wallCell.forEach((cell, t) => cells.has(cell) && pieces.push({ source: this.wall, at: t * 9, color: this.wallColor }))
    this.roofCell.forEach((cell, t) => cells.has(cell) && pieces.push({ source: this.roof, at: t * 9, color: this.roofColor }))
    if (pieces.length === 0) return null
    const pivot = new THREE.Vector3()
    for (const { source, at } of pieces) {
      for (let k = 0; k < 3; k++) {
        pivot.x += source[at + k * 3]!
        pivot.y += source[at + k * 3 + 1]!
        pivot.z += source[at + k * 3 + 2]!
      }
    }
    pivot.divideScalar(pieces.length * 3)
    const positions: number[] = []
    const colors: number[] = []
    for (const { source, at, color } of pieces) {
      for (let k = 0; k < 3; k++) {
        positions.push(source[at + k * 3]! - pivot.x, source[at + k * 3 + 1]! - pivot.y, source[at + k * 3 + 2]! - pivot.z)
        colors.push(color.r * 0.9, color.g * 0.9, color.b * 0.9)
      }
    }
    return { positions: Float32Array.from(positions), colors: Float32Array.from(colors), pivot }
  }
}

/** A flat-shaded geometry from triangle soup with a colour per vertex. */
export function shardGeometry(positions: Float32Array, colors: Float32Array): Geometry {
  // Not indexed, so every face has its own normal: flat shading.
  const geometry = new Geometry({ positions, colors })
  geometry.computeNormals()
  return geometry
}
