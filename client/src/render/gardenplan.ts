import { BUILDING_CABIN, BUILDING_FARMHOUSE, BUILDING_FLATS, BUILDING_PLAIN, BUILDING_VILLA, COVER_GARDEN, COVER_PARK, type Building, type Point2, type TileData } from '@/world'
import { SURFACE_CELL, SURFACE_GRASS, type TileSurface } from '@/sim/surface'
import { WOOD_STRIDE } from './forest'
import { coverIds } from './fray'

/*
 * What grows in a house's garden (GARDENS.md): a flower bed along its sunniest wall and a rhubarb patch
 * out at the back. Worked out from the tile alone, with the
 * rest of the tile (`tileprep.ts`), so the beds can be painted into the ground and kept clear of grass,
 * and the main thread is handed where everything stands (`gardens.ts` draws it). Nothing here
 * touches the scene or the DOM, so it runs in the tile worker.
 */

/** What a `GardenPlan` item is. */
export const GARDEN_RHUBARB = 0
export const GARDEN_FLOWERS = 1

/** Numbers an item of `GardenPlan.items`: kind, x, z, radius (metres), height (metres) or for flowers an index into `FLOWER_COLORS`, and a seed 0 to 1. */
export const ITEM_STRIDE = 6
/** Numbers a bed of `GardenPlan.beds`: its four corners, x and z each. */
export const BED_STRIDE = 8

/** One tile's gardens. */
export interface GardenPlan {
  items: Float32Array
  beds: Float32Array
}

/** The flower colours of a bed, sRGB: a bed has two or three of them. */
export const FLOWER_COLORS = [0xe8508a, 0xf4f0e4, 0xf2c93a, 0x9a62c8, 0xd8343a, 0xf08a3c, 0xf2a6c4] as const

/** How likely each kind of house is to have a flower bed and a rhubarb patch. Kinds not here have neither. */
const CHANCES: Record<number, readonly [number, number]> = {
  // An old wooden house keeps the garden its grandparents planted.
  [BUILDING_FARMHOUSE]: [0.6, 0.65],
  [BUILDING_PLAIN]: [0.6, 0.3],
  [BUILDING_VILLA]: [0.75, 0.25],
  [BUILDING_CABIN]: [0.15, 0.3],
  [BUILDING_FLATS]: [0.3, 0],
}
/** A building with more floor than this is no house to garden round, square metres. */
const MAX_AREA = 700
/** A house with a grave within this of its middle stands in a churchyard, metres. */
const CHURCHYARD = 25
/** A bed's depth out from the wall, metres; it starts this far out, past the drip line... */
const BED_DEPTH = 0.75
const BED_OUT = 0.15
/** ...and stops this far short of each corner. */
const BED_INSET = 0.6
/** A wall shorter than this has no bed, metres. */
const BED_MIN = 3
/** Flower clumps along a bed, one every this many metres in each of its two rows. */
const FLOWER_STEP = 0.45

/** A number 0 to 1 from `a`, `b` and `salt`, the same in every worker. */
function hash(a: number, b: number, salt: number): number {
  let h = Math.imul(Math.round(a * 16) | 0, 0x27d4eb2d) ^ Math.imul(Math.round(b * 16) | 0, 0x165667b1) ^ Math.imul(salt, 0x9e3779b1)
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b)
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35)
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296
}

/** The distance from `(x, z)` to the segment `a`–`b`. */
function toSegment(x: number, z: number, [ax, az]: Point2, [bx, bz]: Point2): number {
  const dx = bx - ax, dz = bz - az
  const length2 = dx * dx + dz * dz
  const t = length2 > 0 ? Math.min(1, Math.max(0, ((x - ax) * dx + (z - az) * dz) / length2)) : 0
  return Math.hypot(x - ax - t * dx, z - az - t * dz)
}

function inside(ring: readonly Point2[], x: number, z: number): boolean {
  let odd = false
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [ax, az] = ring[i]!, [bx, bz] = ring[j]!
    if (az > z !== bz > z && x < ax + ((z - az) / (bz - az)) * (bx - ax)) odd = !odd
  }
  return odd
}

type Plot = Pick<Building, 'footprint' | 'kind'> & { front?: readonly [number, number] }

/**
 * The gardens of a tile's houses (those whose middle is on it). `surface` is the tile's surface grid:
 * a garden grows only on grass, in a garden, a park or the plain ground nothing covers, clear of every
 * building and of the trees in `woods` (`woodsOf`). A house with any of `graves` (x and z each) near
 * stands in a churchyard, and has none.
 */
export function gardenPlan(tile: Pick<TileData, 'cover'> & { buildings: readonly Plot[] }, surface: TileSurface, woods: Float32Array, graves: Float32Array = new Float32Array(0)): GardenPlan {
  const { x0, z0, side } = surface
  const size = side * SURFACE_CELL
  const ids = coverIds(tile, x0, z0, size, side)
  const items: number[] = []
  const beds: number[] = []
  const boxes = tile.buildings.map(({ footprint }) => {
    let [minX, minZ, maxX, maxZ] = [Infinity, Infinity, -Infinity, -Infinity]
    for (const [x, z] of footprint) [minX, minZ, maxX, maxZ] = [Math.min(minX, x), Math.min(minZ, z), Math.max(maxX, x), Math.max(maxZ, z)]
    return [minX, minZ, maxX, maxZ] as const
  })
  /** Whether a thing of `radius` can stand at `(x, z)`, `room` metres clear of every building. */
  const open = (x: number, z: number, radius: number, room: number) => {
    const column = Math.floor((x - x0) / SURFACE_CELL), row = Math.floor((z - z0) / SURFACE_CELL)
    if (column < 0 || row < 0 || column >= side || row >= side) return false
    const k = row * side + column
    const cover = ids[k]!
    if (surface.cells[k] !== SURFACE_GRASS || (cover !== 255 && cover !== COVER_GARDEN && cover !== COVER_PARK)) return false
    const reach = radius + room
    for (let b = 0; b < tile.buildings.length; b++) {
      const [minX, minZ, maxX, maxZ] = boxes[b]!
      if (x < minX - reach || x > maxX + reach || z < minZ - reach || z > maxZ + reach) continue
      const ring = tile.buildings[b]!.footprint
      if (inside(ring, x, z)) return false
      for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) if (toSegment(x, z, ring[j]!, ring[i]!) < reach) return false
    }
    for (let i = 0; i < woods.length; i += WOOD_STRIDE) {
      if (Math.hypot(x - woods[i]!, z - woods[i + 1]!) < radius + 0.6 + woods[i + 2]! * 0.3) return false
    }
    return true
  }

  for (const house of tile.buildings) {
    const chances = CHANCES[house.kind ?? BUILDING_PLAIN]
    const ring = house.footprint
    if (!chances || ring.length < 3) continue
    // Which way round the outline runs, so each wall's outward side is known.
    let twice = 0, cx = 0, cz = 0
    for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
      twice += ring[j]![0] * ring[i]![1] - ring[i]![0] * ring[j]![1]
      cx += ring[i]![0]
      cz += ring[i]![1]
    }
    ;[cx, cz] = [cx / ring.length, cz / ring.length]
    if (Math.abs(twice) / 2 > MAX_AREA || x0 > cx || cx >= x0 + size || z0 > cz || cz >= z0 + size) continue
    let churchyard = false
    for (let i = 0; i < graves.length && !churchyard; i += 2) churchyard = Math.hypot(graves[i]! - cx, graves[i + 1]! - cz) < CHURCHYARD
    if (churchyard) continue
    const turn = twice > 0 ? 1 : -1
    const roll = (salt: number) => hash(cx, cz, salt)
    const [fx, fz] = house.front && (house.front[0] !== 0 || house.front[1] !== 0) ? house.front : [0, 0]
    const fl = Math.hypot(fx, fz) || 1
    // Toward the road the garden is open; its back is away from the road, or the north without one.
    const [backX, backZ] = fx === 0 && fz === 0 ? [0, -1] : [-fx / fl, -fz / fl]

    const walls = ring.map((a, i) => {
      const b = ring[(i + 1) % ring.length]!
      const [dx, dz] = [b[0] - a[0], b[1] - a[1]]
      const length = Math.hypot(dx, dz)
      return { a, b, length, along: [dx / length, dz / length] as const, out: [(turn * dz) / length, (-turn * dx) / length] as const }
    })
    const front = fx === 0 && fz === 0 ? null : walls.reduce((best, wall) => (wall.out[0] * fx + wall.out[1] * fz > best.out[0] * fx + best.out[1] * fz ? wall : best))

    // A bed along the wall that faces the sun most squarely (+z is south), but not the door's.
    if (roll(1) < chances[0]) {
      const sunny = walls.filter((wall) => wall !== front && wall.length >= BED_MIN && wall.out[1] > -0.2).sort((p, q) => q.out[1] - p.out[1])[0]
      if (sunny) {
        const { a, along, out, length } = sunny
        const span = Math.min(length - 2 * BED_INSET, 4 + roll(2) * 8)
        const start = BED_INSET + roll(3) * (length - 2 * BED_INSET - span)
        const at = (t: number, depth: number): Point2 => [a[0] + along[0] * t + out[0] * depth, a[1] + along[1] * t + out[1] * depth]
        const corners = [at(start, BED_OUT), at(start + span, BED_OUT), at(start + span, BED_OUT + BED_DEPTH), at(start, BED_OUT + BED_DEPTH)]
        // The lawn beyond it must be open ground: a bed against a wall on a road is no bed.
        const [mx, mz] = at(start + span / 2, BED_OUT + BED_DEPTH + 1.5)
        if (open(mx, mz, 0.2, 0.5)) {
          beds.push(...corners.flat())
          const colors = [Math.floor(roll(4) * FLOWER_COLORS.length), Math.floor(roll(5) * FLOWER_COLORS.length), Math.floor(roll(6) * FLOWER_COLORS.length)]
          for (const [row, depth] of [[0, BED_OUT + BED_DEPTH * 0.3], [1, BED_OUT + BED_DEPTH * 0.72]] as const) {
            for (let t = start + FLOWER_STEP * (0.5 + row * 0.5); t < start + span - 0.15; t += FLOWER_STEP) {
              const [x, z] = at(t + (hash(t, depth, 7) - 0.5) * 0.15, depth)
              // Bigger, and so taller, at the back of the bed against the wall.
              const seed = hash(x, z, 8)
              items.push(GARDEN_FLOWERS, x, z, (row === 0 ? 0.2 : 0.15) + seed * 0.07, colors[Math.floor(hash(x, z, 9) * 3)]!, seed)
            }
          }
        }
      }
    }

    // Out at the back, away from the road, a rhubarb patch a few metres from the wall.
    const backWall = walls.reduce((best, wall) => (wall.out[0] * backX + wall.out[1] * backZ > best.out[0] * backX + best.out[1] * backZ ? wall : best))
    const backAt = (t: number, depth: number): Point2 => [backWall.a[0] + backWall.along[0] * t + backWall.out[0] * depth, backWall.a[1] + backWall.along[1] * t + backWall.out[1] * depth]
    if (roll(30) < chances[1]) {
      const plants = 1 + Math.floor(roll(31) * 3)
      const [px, pz] = backAt(roll(32) * backWall.length, 3 + roll(33) * 3)
      for (let k = 0; k < plants; k++) {
        const angle = roll(34) * 6.283 + k * 2.1
        const [x, z] = [px + Math.cos(angle) * 1.2 * Math.min(k, 1), pz + Math.sin(angle) * 1.2 * Math.min(k, 1)]
        const radius = 0.5 + hash(x, z, 35) * 0.25
        if (open(x, z, radius, 0.8)) items.push(GARDEN_RHUBARB, x, z, radius, 0.5 + hash(x, z, 36) * 0.2, hash(x, z, 37))
      }
    }
  }
  return { items: Float32Array.from(items), beds: Float32Array.from(beds) }
}

/** A plan with nothing in it. */
export const NO_GARDENS: GardenPlan = { items: new Float32Array(0), beds: new Float32Array(0) }
