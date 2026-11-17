import { BUILDING_BARN, BUILDING_BRIDGE, BUILDING_CHURCH, BUILDING_FARMHOUSE, BUILDING_FIRE_STATION, BUILDING_HALL, BUILDING_OUTBUILDING, BUILDING_PUBLIC, BUILDING_SHOP, BUILDING_VILLA, type CollisionMesh } from '@/world'

/*
 * Windows and a door on a building's walls, worked out from its look (`Tile.look`), so a house reads
 * as a house: rows of windows a storey apart along each flat wall, and a door in the middle of the
 * ground row of its longest wall. Each is a white frame with a dark pane on it, a little proud of the
 * wall. A church has one row of tall round-headed windows along its long walls, and its door and a
 * round window in an end. A barn, an outbuilding, a hall, a shop and a public building have the doors
 * and the windows of their `Style`. Only drawn; the same for the same building on every client.
 */

/** A building's triangles in the look: `walls` from `first`, then `roof`. */
export interface LookRange {
  first: number
  walls: number
  roof: number
  /** What it is (`BUILDING_*`); a house when left out. */
  kind?: number
  /** Which way the road it stands by lies (`Building.front`); none when left out. */
  front?: readonly [number, number]
  /** Its footprint (`x, z` pairs, centimetres) and, once worked out, what stands inside it (`insideFor`). */
  footprint?: Int32Array
  inside?: Int32Array
}

type P = [number, number, number]

/** One window or door: its four corners on the wall (counter-clockwise seen from outside), and the wall's outward normal. */
export interface Opening {
  corners: [P, P, P, P]
  normal: P
  door: boolean
  /** `HEAD_*`: how its top is drawn. Square when left out. */
  head?: number
  /** A door standing open, as drawn on a house with rooms (`drawnOpenings`). */
  open?: boolean
}

const STOREY = 2.8
const SILL = 0.9
const WINDOW_HEIGHT = 1.3
const WINDOW_WIDTH = 1.0
/** Gap between window middles along a wall, about, metres. */
const PITCH = 2.8
/** Space kept clear round a window inside its wall, metres. */
const MARGIN = 0.35
const DOOR_WIDTH = 1.0
const DOOR_HEIGHT = 2.1
/**
 * A wall whose foot is this far over the building's lowest is raised (a dormer's front, the step up
 * to a higher part of the roof): its windows are small, set low in it, and it has no door, metres.
 * Mirrors `RAISED` and the `SMALL_*` constants in `windows.rs`.
 */
const RAISED = 2.0
/** Two pieces of wall are one wall when their normals are this much alike and the corners of the one lie within this of the other's plane, metres. Mirror `SAME_WAY` and `SAME_WALL` in `windows.rs`. */
const SAME_WAY = 0.9998
const SAME_WALL = 0.06
const SMALL_SILL = 0.2
const SMALL_HEIGHT = 0.7
const SMALL_WIDTH = 0.8
const SMALL_MARGIN = 0.15
const SMALL_PITCH = 2.0
/**
 * A church's long walls, its door and the round window in each of its ends. Mirror the `CHURCH_*`
 * and `ROSE*` constants in `windows.rs`, where each is described.
 */
const CHURCH_WALL = 8.0
const CHURCH_WIDTH = 1.6
const CHURCH_PITCH = 6.0
const CHURCH_SILL = 2.2
const CHURCH_TOP = 1.2
const CHURCH_HEIGHT = 6.0
const CHURCH_DOOR_WIDTH = 2.0
const CHURCH_DOOR_HEIGHT = 3.2
const ROSE = 2.2
const ROSE_UP = 0.35
const ROSE_CLEAR = 1.0
/**
 * How the walls of a barn, an outbuilding, a hall or a shop are opened. Mirrors `Style` and its
 * constants in `windows.rs`, where each field is described.
 */
interface Style {
  doors: readonly (readonly [number, number])[]
  doored: number
  bays: number
  window: readonly [number, number, number, number]
  walls: readonly [number, number]
  most: number
  storeys: boolean
}
const DOOR = [DOOR_WIDTH, DOOR_HEIGHT] as const
const STYLES: Readonly<Record<number, Style>> = {
  [BUILDING_BARN]: { doors: [[3.2, 3.0], [2.4, 2.4], DOOR], doored: 1, bays: 1, window: [1.5, 0.7, 0.7, 7.0], walls: [0, 2], most: Infinity, storeys: false },
  [BUILDING_OUTBUILDING]: { doors: [[2.4, 2.1], DOOR], doored: 1, bays: 1, window: [1.2, 0.7, 0.7, 2.8], walls: [1, 2], most: 1, storeys: false },
  [BUILDING_HALL]: { doors: [[4.0, 4.0], [3.2, 3.0], DOOR], doored: 2, bays: 1, window: [3.0, 1.0, 2.0, 3.2], walls: [0, Infinity], most: Infinity, storeys: false },
  [BUILDING_SHOP]: { doors: [[1.6, 2.3], DOOR], doored: 1, bays: 1, window: [0.5, 2.0, 2.2, 3.0], walls: [0, 1], most: Infinity, storeys: false },
  [BUILDING_PUBLIC]: { doors: [[1.6, 2.3], DOOR], doored: 1, bays: 1, window: [0.9, 1.6, 1.8, 3.0], walls: [0, Infinity], most: Infinity, storeys: true },
  [BUILDING_FARMHOUSE]: { doors: [DOOR], doored: 1, bays: 1, window: [0.8, 1.5, 0.9, 2.3], walls: [0, Infinity], most: Infinity, storeys: true },
  [BUILDING_VILLA]: { doors: [DOOR], doored: 1, bays: 1, window: [1.0, 1.2, 1.7, 3.6], walls: [0, Infinity], most: Infinity, storeys: true },
  [BUILDING_FIRE_STATION]: { doors: [[3.6, 3.6], [3.0, 3.0], DOOR], doored: 1, bays: 3, window: [0.9, 1.6, 1.8, 3.0], walls: [0, Infinity], most: Infinity, storeys: true },
}
/** Mirror `BAY_GAP`, `FRONT_SHARE` and `FRONT_FACING` in `windows.rs`, where each is described. */
const BAY_GAP = 0.8
const FRONT_SHARE = 0.6
const FRONT_FACING = 63.5
/**
 * How an opening is drawn at its top: square; with a round arch over its width; or as a circle (its
 * hole is a square). Mirror `HEAD_*` in `windows.rs`.
 */
export const HEAD_SQUARE = 0
export const HEAD_ARCH = 1
export const HEAD_ROUND = 2
/** How far frames and panes stand out from the wall, metres, and how much wider a frame is than its pane. */
const FRAME_OUT = 0.025
export const PANE_OUT = 0.04
export const FRAME = 0.09
/** How deep a window's reveal goes into the wall, metres: the wall's thickness (`facade.ts`). */
export const REVEAL = 0.2

/** Linear colours: frames, a door's leaf, and the reveal round a window (the frame's, in shade). */
export const FRAME_RGB = [0.86, 0.84, 0.78] as const
export const DOOR_RGB = [0.11, 0.06, 0.035] as const
export const REVEAL_RGB = [0.7, 0.68, 0.63] as const

interface Plane {
  normal: P
  /** Along the wall, horizontal. */
  u: P
  tris: [number, number][][]
  u0: number
  u1: number
  v0: number
  v1: number
}

/** The windows and door of building `range` of `look`: a house's, or a church's (`range.kind`). */
export function openingsOf(look: CollisionMesh, range: LookRange): Opening[] {
  // A bridge has no windows, no doors and so no inside.
  if (range.kind === BUILDING_BRIDGE) return []
  const { vertices, triangles } = look
  const corner = (v: number): P => [vertices[v * 3]!, vertices[v * 3 + 1]!, vertices[v * 3 + 2]!]
  const planes: (Plane & { d: number })[] = []
  for (let t = range.first; t < range.first + range.walls; t++) {
    // Wound outwards: the look is wound inwards.
    const [a, b, c] = [corner(triangles[t * 3]!), corner(triangles[t * 3 + 2]!), corner(triangles[t * 3 + 1]!)]
    const e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    const f = [c[0] - a[0], c[1] - a[1], c[2] - a[2]]
    let n: P = [e[1]! * f[2]! - e[2]! * f[1]!, e[2]! * f[0]! - e[0]! * f[2]!, e[0]! * f[1]! - e[1]! * f[0]!]
    const l = Math.hypot(...n)
    if (l < 1e-6) continue
    n = [n[0] / l, n[1] / l, n[2] / l]
    // Upright walls only.
    if (Math.abs(n[1]) > 0.1) continue
    const h = Math.hypot(n[0], n[2])
    n = [n[0] / h, 0, n[2] / h]
    // The wall it is a piece of: one facing the same way that all its corners lie in. A wall is often
    // in pieces (a gable end is cut at the ridge) whose corners were rounded apart by a centimetre.
    let plane = planes.find((plane) => plane.normal[0] * n[0] + plane.normal[2] * n[2] > SAME_WAY && [a, b, c].every((p) => Math.abs(plane.normal[0] * p[0] + plane.normal[2] * p[2] - plane.d) < SAME_WALL))
    if (!plane) {
      plane = { normal: n, u: [-n[2], 0, n[0]], tris: [], u0: Infinity, u1: -Infinity, v0: Infinity, v1: -Infinity, d: n[0] * a[0] + n[2] * a[2] }
      planes.push(plane)
    }
    const flat = [a, b, c].map((p) => [p[0] * plane!.u[0] + p[2] * plane!.u[2], p[1]] as [number, number])
    plane.tris.push(flat)
    for (const [u, v] of flat) {
      plane.u0 = Math.min(plane.u0, u)
      plane.u1 = Math.max(plane.u1, u)
      plane.v0 = Math.min(plane.v0, v)
      plane.v1 = Math.max(plane.v1, v)
    }
  }
  const inside = (plane: Plane, u: number, v: number) =>
    plane.tris.some(([a, b, c]) => {
      const s = (p: [number, number], q: [number, number]) => (q[0] - p[0]) * (v - p[1]) - (q[1] - p[1]) * (u - p[0])
      const [x, y, z] = [s(a!, b!), s(b!, c!), s(c!, a!)]
      return (x >= 0 && y >= 0 && z >= 0) || (x <= 0 && y <= 0 && z <= 0)
    })
  const fits = (plane: Plane, u0: number, u1: number, v0: number, v1: number, margin = MARGIN) =>
    [
      [u0 - margin, v0 - margin],
      [u1 + margin, v0 - margin],
      [u1 + margin, v1 + margin],
      [u0 - margin, v1 + margin],
    ].every(([u, v]) => inside(plane, u!, v!))
  let lowest = Infinity
  for (const plane of planes) lowest = Math.min(lowest, plane.v0)
  const sorted = [...planes].sort((a, b) => (a.u1 - a.u0 === b.u1 - b.u0 ? a.d - b.d : b.u1 - b.u0 - (a.u1 - a.u0)))
  type Wall = Plane & { d: number }
  const isRaised = (plane: Wall) => plane.v0 - lowest >= RAISED
  // The front goes first: the wall that faces the road, where one of the longer walls does. A
  // church's front is the end away from its annexe, whatever road passes it.
  const front = range.front ?? [0, 0]
  if (range.kind !== BUILDING_CHURCH && (front[0] !== 0 || front[1] !== 0)) {
    let longest = 0
    for (const plane of sorted) if (!isRaised(plane)) longest = Math.max(longest, plane.u1 - plane.u0)
    let best = -1
    let most = FRONT_FACING
    sorted.forEach((plane, rank) => {
      const facing = plane.normal[0] * front[0] + plane.normal[2] * front[1]
      if (!isRaised(plane) && plane.u1 - plane.u0 >= FRONT_SHARE * longest && facing > most) {
        best = rank
        most = facing
      }
    })
    if (best > 0) sorted.unshift(...sorted.splice(best, 1))
  }
  const style = STYLES[range.kind ?? 0]
  if (style) {
    const [sill, height, width, pitch] = style.window
    const out: Opening[] = []
    // The walls from the ground, longest first: a raised one is left blank.
    sorted.filter((plane) => !isRaised(plane)).forEach((plane, rank) => {
      const length = plane.u1 - plane.u0
      const toWorld = (u: number, v: number): P => [plane.u[0] * u + plane.normal[0] * plane.d, v, plane.u[2] * u + plane.normal[2] * plane.d]
      const push = (u0: number, u1: number, v0: number, v1: number, door: boolean) =>
        out.push({ corners: [toWorld(u0, v0), toWorld(u1, v0), toWorld(u1, v1), toWorld(u0, v1)], normal: plane.normal, door, head: HEAD_SQUARE })
      // The doors, a row about the middle of the wall, as many as it has room for: the row is
      // `[from, to, top]` once it is hung.
      const mid = (plane.u0 + plane.u1) / 2
      let door: [number, number, number] | undefined
      if (rank < style.doored) {
        for (const [w, h] of style.doors) {
          const bays = Math.max(1, Math.min(style.bays, Math.floor((length - 2 * MARGIN + BAY_GAP) / (w + BAY_GAP))))
          for (let i = 0; i < bays; i++) {
            const at = mid + (i - (bays - 1) / 2) * (w + BAY_GAP)
            if (!fits(plane, at - w / 2, at + w / 2, plane.v0 + MARGIN, plane.v0 + h)) continue
            push(at - w / 2, at + w / 2, plane.v0 + 0.02, plane.v0 + h, true)
            door = [door ? door[0] : at - w / 2, at + w / 2, plane.v0 + h]
          }
          if (door) break
        }
      }
      if (rank < style.walls[0] || rank >= style.walls[1] || length < width + 2 * MARGIN) return
      const count = Math.min(Math.max(1, Math.round(length / pitch)), style.most)
      const step = length / count
      for (let storey = 0; plane.v0 + sill + storey * STOREY + height + MARGIN <= plane.v1 && (style.storeys || storey < 1); storey++) {
        const v0 = plane.v0 + sill + storey * STOREY
        for (let i = 0; i < count; i++) {
          const at = plane.u0 + (i + 0.5) * step
          const [u0, u1] = [at - width / 2, at + width / 2]
          // A window keeps clear of the door in its wall.
          const byDoor = door !== undefined && u1 + MARGIN > door[0] && u0 - MARGIN < door[1] && v0 - MARGIN < door[2]
          if (!byDoor && fits(plane, u0, u1, v0, v0 + height)) push(u0, u1, v0, v0 + height, false)
        }
      }
    })
    return out
  }
  // A church's long walls run the way its longest does; the two widest walls across that are its ends.
  const long: P = sorted[0]?.u ?? [1, 0, 0]
  const along = (plane: Wall) => Math.abs(plane.u[0] * long[0] + plane.u[2] * long[2]) > 0.7
  const isLong = (plane: Wall) => along(plane) && plane.u1 - plane.u0 >= CHURCH_WALL
  let ends: Wall[] = []
  let doorEnd: Wall | undefined
  if (range.kind === BUILDING_CHURCH) {
    ends = sorted.filter((plane) => !isRaised(plane) && !along(plane)).slice(0, 2)
    // How far an end is from the lesser walls (an annexe's), summed: the front is the end away from them.
    const far = (e: Wall) => {
      let sum = 0
      for (const p of sorted) {
        if (ends.includes(p) || isRaised(p) || isLong(p)) continue
        const mid = (p.u0 + p.u1) / 2
        const [x, z] = [p.u[0] * mid + p.normal[0] * p.d, p.u[2] * mid + p.normal[2] * p.d]
        sum += Math.abs(e.normal[0] * x + e.normal[2] * z - e.d)
      }
      return sum
    }
    const fronts = [...ends]
    if (fronts.length === 2 && far(fronts[1]!) > far(fronts[0]!) + 0.01) fronts.reverse()
    doorEnd = fronts.find((p) => {
      const mid = (p.u0 + p.u1) / 2
      return fits(p, mid - CHURCH_DOOR_WIDTH / 2, mid + CHURCH_DOOR_WIDTH / 2, p.v0 + MARGIN, p.v0 + CHURCH_DOOR_HEIGHT)
    })
  }
  // A church with no end its door fits in (a hut of a chapel) is given windows as a house is.
  const church = doorEnd !== undefined
  const out: Opening[] = []
  sorted.forEach((plane, rank) => {
    const raised = isRaised(plane)
    const length = plane.u1 - plane.u0
    const toWorld = (u: number, v: number): P => [plane.u[0] * u + plane.normal[0] * plane.d, v, plane.u[2] * u + plane.normal[2] * plane.d]
    const push = (u0: number, u1: number, v0: number, v1: number, door: boolean, head = HEAD_SQUARE) =>
      out.push({ corners: [toWorld(u0, v0), toWorld(u1, v0), toWorld(u1, v1), toWorld(u0, v1)], normal: plane.normal, door, head })
    // The wall of a church's nave standing over its annexe's roof is left blank.
    if (church && raised) return
    if (church && !raised && ends.includes(plane)) {
      // An end of a church: the door in the front one, and a round window over the middle of each.
      const mid = (plane.u0 + plane.u1) / 2
      if (doorEnd === plane) push(mid - CHURCH_DOOR_WIDTH / 2, mid + CHURCH_DOOR_WIDTH / 2, plane.v0 + 0.02, plane.v0 + CHURCH_DOOR_HEIGHT, true)
      const centre = plane.v0 + Math.max(ROSE_UP * (plane.v1 - plane.v0), CHURCH_DOOR_HEIGHT + ROSE_CLEAR + ROSE / 2)
      if (fits(plane, mid - ROSE / 2, mid + ROSE / 2, centre - ROSE / 2, centre + ROSE / 2)) push(mid - ROSE / 2, mid + ROSE / 2, centre - ROSE / 2, centre + ROSE / 2, false, HEAD_ROUND)
      return
    }
    if (church && !raised && isLong(plane)) {
      // A long wall of a church: one row of tall round-headed windows.
      const tall = plane.v1 - plane.v0
      const sill = Math.min(CHURCH_SILL, tall * 0.3)
      const height = Math.min(CHURCH_HEIGHT, tall - sill - Math.min(CHURCH_TOP, tall * 0.15))
      const count = Math.max(1, Math.round(length / CHURCH_PITCH))
      const step = length / count
      for (let i = 0; i < count; i++) {
        const mid = plane.u0 + (i + 0.5) * step
        const [u0, u1, v0] = [mid - CHURCH_WIDTH / 2, mid + CHURCH_WIDTH / 2, plane.v0 + sill]
        if (fits(plane, u0, u1, v0, v0 + height)) push(u0, u1, v0, v0 + height, false, HEAD_ARCH)
      }
      return
    }
    const [sill, height, width, margin, pitch] = raised ? [SMALL_SILL, SMALL_HEIGHT, SMALL_WIDTH, SMALL_MARGIN, SMALL_PITCH] : [SILL, WINDOW_HEIGHT, WINDOW_WIDTH, MARGIN, PITCH]
    if (length < width + 2 * margin) return
    const count = Math.max(1, Math.round(length / pitch))
    const step = length / count
    // Rows a storey apart, as high as the wall reaches (`fits` keeps them off a gable's slopes); the
    // lesser walls of a church (its annexe's) have the one row.
    for (let storey = 0; plane.v0 + sill + storey * STOREY + height + margin <= plane.v1 && !(church && storey >= 1); storey++) {
      const v0 = plane.v0 + sill + storey * STOREY
      for (let i = 0; i < count; i++) {
        const mid = plane.u0 + (i + 0.5) * step
        // The longest wall's door: the middle of its ground row.
        if (!church && !raised && rank === 0 && storey === 0 && i === Math.floor(count / 2) && fits(plane, mid - DOOR_WIDTH / 2, mid + DOOR_WIDTH / 2, plane.v0 + MARGIN, plane.v0 + DOOR_HEIGHT)) {
          push(mid - DOOR_WIDTH / 2, mid + DOOR_WIDTH / 2, plane.v0 + 0.02, plane.v0 + DOOR_HEIGHT, true)
          continue
        }
        const [u0, u1] = [mid - width / 2, mid + width / 2]
        if (fits(plane, u0, u1, v0, v0 + height, margin)) push(u0, u1, v0, v0 + height, false)
      }
    }
  })
  return out
}

const memo = new WeakMap<CollisionMesh, Map<number, readonly Opening[]>>()

/** `openingsOf`, remembered for the look and the building: windows are looked up by their index in it. */
export function openingsFor(look: CollisionMesh, range: LookRange): readonly Opening[] {
  let byBuilding = memo.get(look)
  if (!byBuilding) {
    byBuilding = new Map()
    memo.set(look, byBuilding)
  }
  let openings = byBuilding.get(range.first)
  if (!openings) {
    openings = openingsOf(look, range)
    byBuilding.set(range.first, openings)
  }
  return openings
}

/** How many straight pieces a quarter of a round head is drawn as. */
const ARC_STEPS = 6
/** How far the wall filled in round an arched head stands out, metres: in front of the glass (`GLASS_OUT`), and the frame along the curve in front of that. */
const MASK_OUT = 0.06
const ARC_FRAME_OUT = 0.07

/**
 * The triangles `openingQuads` draws for `openings`: a window's frame ring and reveal, a door's frame
 * and leaf (an open door's frame is three strips, and its leaf has two sides; a gate's leaf is drawn apart); a window with an
 * arched or a round head has the reveal, the wall filled in round the curve with its edges, and the frame along it.
 */
export function openingTriangles(openings: readonly Opening[]): number {
  let n = 0
  for (const o of openings) {
    if (o.door) n += o.open ? (isGate(o) ? 6 : 10) : 4
    else if (o.head === HEAD_ARCH) n += 2 * (4 + 3 + 3 + 4 * ARC_STEPS)
    else if (o.head === HEAD_ROUND) n += 2 * (4 + 4 + 8 * ARC_STEPS)
    else n += 16
  }
  return n
}

/** How far above its foot an open door's leaf starts, metres: on the plinth the ground floor is on (`PLINTH` in `interior.rs`, less a hair). */
const LEAF_FOOT = 0.28
/** How far an open door's top is above a shut one's, metres. Mirrors `DOOR_RISE`. */
const OPEN_RISE = 0.3
/**
 * A door at least this wide, metres, is a gate (a barn's, a hall's, a garage's): mirrors
 * `GATE_WIDTH` in `windows.rs`. On a building with a floor it is opened and shut, and its leaf is
 * drawn apart, where it stands (`gates.ts`): here it is its frame only.
 */
const GATE_WIDTH = 2.2

/** Whether opening `o` is a gate. */
export function isGate(o: Opening): boolean {
  return o.door && Math.hypot(o.corners[1][0] - o.corners[0][0], o.corners[1][2] - o.corners[0][2]) >= GATE_WIDTH
}

/**
 * `openings` as they are drawn on a house with rooms (`furnished`): its door stands open, on the
 * plinth. The windows and a house without rooms are as they were.
 */
export function drawnOpenings(openings: readonly Opening[], furnished: boolean): readonly Opening[] {
  if (!furnished) return openings
  return openings.map((o) => (o.door ? { ...o, open: true } : o))
}

/**
 * Calls `quad` for each quad of `openings` to draw, with the normal it faces: a window's frame, a ring
 * standing a little out from the wall round the hole cut for it, and the reveal going into the wall
 * from the hole's four sides; a door's frame and its leaf, standing out from the wall. The glass is
 * drawn apart (`glass.ts`), and the hole itself is cut out of the wall (`facade.ts`).
 *
 * The hole of a window with an arched or a round head is its rectangle all the same: the wall is
 * filled in over the glass from the curve out to the rectangle (`masonry`, to be drawn as the wall
 * is), and the frame follows the curve.
 */
export function openingQuads(openings: readonly Opening[], quad: (corners: [P, P, P, P], rgb: readonly [number, number, number], normal: P, masonry?: boolean) => void) {
  for (const opening of openings) {
    const { corners, normal, door, open, head } = opening
    const [a, , c] = corners
    // Unit along the wall and up, from the corners.
    const along: P = [corners[1][0] - a[0], 0, corners[1][2] - a[2]]
    const al = Math.hypot(along[0], along[2]) || 1
    const [ux, uz] = [along[0] / al, along[2] / al]
    const grow = (out: number, by: number, bottom = by): [P, P, P, P] => {
      const shift = (p: P, du: number, dv: number): P => [p[0] + ux * du + normal[0] * out, p[1] + dv, p[2] + uz * du + normal[2] * out]
      return [shift(a, -by, -bottom), shift(corners[1], by, -bottom), shift(c, by, by), shift(corners[3], -by, by)]
    }
    if (door && open) {
      // The doorway's frame, up its sides and over its top, and the leaf swung in on its first side.
      const up = c[1] + OPEN_RISE
      const at = (du: number, out: number, y: number): P => [a[0] + ux * du + normal[0] * out, y, a[2] + uz * du + normal[2] * out]
      quad([at(-FRAME, FRAME_OUT, a[1]), at(0, FRAME_OUT, a[1]), at(0, FRAME_OUT, up), at(-FRAME, FRAME_OUT, up)], FRAME_RGB, normal)
      quad([at(al, FRAME_OUT, a[1]), at(al + FRAME, FRAME_OUT, a[1]), at(al + FRAME, FRAME_OUT, up), at(al, FRAME_OUT, up)], FRAME_RGB, normal)
      quad([at(-FRAME, FRAME_OUT, up), at(al + FRAME, FRAME_OUT, up), at(al + FRAME, FRAME_OUT, up + FRAME), at(-FRAME, FRAME_OUT, up + FRAME)], FRAME_RGB, normal)
      // A gate's leaf is drawn apart (`gates.ts`).
      if (isGate(opening)) continue
      const leaf: [P, P, P, P] = [at(0.03, -0.05, a[1] + LEAF_FOOT), at(0.03, -0.05 - al, a[1] + LEAF_FOOT), at(0.03, -0.05 - al, up), at(0.03, -0.05, up)]
      quad(leaf, DOOR_RGB, [ux, 0, uz])
      quad(leaf, DOOR_RGB, [-ux, 0, -uz])
      continue
    }
    if (door) {
      quad(grow(FRAME_OUT, FRAME, 0), FRAME_RGB, normal)
      quad(grow(FRAME_OUT + 0.015, 0), DOOR_RGB, normal)
      continue
    }
    if (head === HEAD_ARCH || head === HEAD_ROUND) {
      // In the wall's plane: `u` along it from the first corner, `v` up from the sill, `out` from the wall.
      const [w, h] = [al, c[1] - a[1]]
      const at = (u: number, v: number, out: number): P => [a[0] + ux * u + normal[0] * out, a[1] + v, a[2] + uz * u + normal[2] * out]
      const r = w / 2
      // The curve's middle: an arch springs from the top of its straight sides, a circle fills its square.
      const cy = head === HEAD_ARCH ? h - r : h / 2
      const arc = (from: number, edge: number) => {
        // Half a circle from the angle `from`, each piece with the wall filled in out to the rectangle's side at `edge`.
        for (let i = 0; i < 2 * ARC_STEPS; i++) {
          const [t0, t1] = [from + (i * Math.PI) / (2 * ARC_STEPS), from + ((i + 1) * Math.PI) / (2 * ARC_STEPS)]
          const on = (t: number, radius: number, out: number) => at(r + Math.cos(t) * radius, cy + Math.sin(t) * radius, out)
          quad([on(t0, r, MASK_OUT), on(t1, r, MASK_OUT), at(r + Math.cos(t1) * r, edge, MASK_OUT), at(r + Math.cos(t0) * r, edge, MASK_OUT)], FRAME_RGB, normal, true)
          quad([on(t0, r, ARC_FRAME_OUT), on(t1, r, ARC_FRAME_OUT), on(t1, r + FRAME, ARC_FRAME_OUT), on(t0, r + FRAME, ARC_FRAME_OUT)], FRAME_RGB, normal)
        }
      }
      arc(0, h)
      // The edges of the filled-in wall, back to the wall behind it: over its top and down its sides.
      const foot = head === HEAD_ROUND ? 0 : cy
      quad([at(0, h, 0), at(w, h, 0), at(w, h, MASK_OUT), at(0, h, MASK_OUT)], FRAME_RGB, [0, 1, 0], true)
      quad([at(0, foot, 0), at(0, h, 0), at(0, h, MASK_OUT), at(0, foot, MASK_OUT)], FRAME_RGB, [-ux, 0, -uz], true)
      quad([at(w, foot, 0), at(w, h, 0), at(w, h, MASK_OUT), at(w, foot, MASK_OUT)], FRAME_RGB, [ux, 0, uz], true)
      if (head === HEAD_ROUND) {
        arc(Math.PI, 0)
        quad([at(0, 0, 0), at(w, 0, 0), at(w, 0, MASK_OUT), at(0, 0, MASK_OUT)], FRAME_RGB, [0, -1, 0], true)
      } else {
        // The straight of the frame: under the sill and up each side to where the arch springs.
        quad([at(-FRAME, -FRAME, FRAME_OUT), at(w + FRAME, -FRAME, FRAME_OUT), at(w, 0, FRAME_OUT), at(0, 0, FRAME_OUT)], FRAME_RGB, normal)
        quad([at(-FRAME, -FRAME, FRAME_OUT), at(0, 0, FRAME_OUT), at(0, cy, FRAME_OUT), at(-FRAME, cy, FRAME_OUT)], FRAME_RGB, normal)
        quad([at(w, 0, FRAME_OUT), at(w + FRAME, -FRAME, FRAME_OUT), at(w + FRAME, cy, FRAME_OUT), at(w, cy, FRAME_OUT)], FRAME_RGB, normal)
      }
    } else {
      const outer = grow(FRAME_OUT, FRAME)
      const inner = grow(FRAME_OUT, 0)
      for (let k = 0; k < 4; k++) quad([outer[k]!, outer[(k + 1) % 4]!, inner[(k + 1) % 4]!, inner[k]!], FRAME_RGB, normal)
    }
    // The reveal: each side of the hole, going into the wall, facing into the opening.
    const back = (p: P): P => [p[0] - normal[0] * REVEAL, p[1], p[2] - normal[2] * REVEAL]
    const faces: P[] = [[0, 1, 0], [-ux, 0, -uz], [0, -1, 0], [ux, 0, uz]]
    for (let k = 0; k < 4; k++) {
      const [p, q] = [corners[k]!, corners[(k + 1) % 4]!]
      quad([p, q, back(q), back(p)], REVEAL_RGB, faces[k]!)
    }
  }
}

/** `corners` ordered so the soup's normal, `(c - b) × (a - b)` of each fan triangle, faces along `normal`. */
export function facing(corners: [P, P, P, P], normal: readonly number[]): [P, P, P, P] {
  const [a, b, c] = corners
  const e = [c[0] - b[0], c[1] - b[1], c[2] - b[2]]
  const f = [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
  const n = [e[1]! * f[2]! - e[2]! * f[1]!, e[2]! * f[0]! - e[0]! * f[2]!, e[0]! * f[1]! - e[1]! * f[0]!]
  return n[0]! * normal[0]! + n[1]! * normal[1]! + n[2]! * normal[2]! >= 0 ? corners : [corners[3], corners[2], corners[1], corners[0]]
}
