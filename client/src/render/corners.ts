import { normalOf, type P, type Poly } from './geom'

/*
 * Where a building's walls end, so the shader can draw a board over each corner of a boarded house
 * (`wallsLook` in the engine's walls.wgsl.ts). A wall is cut into many faces by its windows, and a
 * face knows nothing of the wall it is part of, so the faces in one plane are gathered into the
 * straight runs of wall they make up; each vertex then carries how far it is from its run's two ends.
 */

/** A straight run of wall: the level direction along it, and where along that it starts and ends, metres. */
export interface Run {
  tx: number
  tz: number
  from: number
  to: number
}

/** A face leaning more than this out of upright (its normal's y) is no wall. */
const UPRIGHT = 0.05
/** Two faces are in one plane when their normals agree to this (about 2°) and they lie this near it, metres. */
const SAME_WAY = 0.9995
const SAME_PLANE = 0.03
/** Faces in one plane are one run when no more than this lies between them along it, metres. */
const JOINED = 0.02
/** Two runs meet where their ends are this near, metres; and they make no corner when their normals agree to this (a turn under about 30°: a round silo's facets). */
const MEET = 0.06
const NO_CORNER = 0.866
/** An end that is no corner is put this far off, metres, so no board is drawn near it. */
const FAR = 1000

interface Plane {
  nx: number
  nz: number
  /** How far the plane is from the origin along its normal, metres. */
  d: number
  spans: [number, number][]
  runs: Run[]
}

/** A wall face's level unit normal, or null for a face that is not upright. */
function level(normal: P): [number, number] | null {
  const length = Math.hypot(normal[0], normal[2])
  if (Math.abs(normal[1]) > UPRIGHT * Math.hypot(length, normal[1]) || length === 0) return null
  return [normal[0] / length, normal[2] / length]
}

/** The straight runs of wall that the faces `walls` of one building make up. */
export class WallRuns {
  private readonly planes: Plane[] = []

  constructor(walls: readonly Poly[]) {
    for (const poly of walls) {
      const plane = this.plane(normalOf(poly), poly[0]!, true)
      if (!plane) continue
      let [lo, hi] = [Infinity, -Infinity]
      for (const p of poly) {
        const t = -p[0] * plane.nz + p[2] * plane.nx
        lo = Math.min(lo, t)
        hi = Math.max(hi, t)
      }
      plane.spans.push([lo, hi])
    }
    for (const plane of this.planes) {
      plane.spans.sort((a, b) => a[0] - b[0])
      for (const [lo, hi] of plane.spans) {
        const last = plane.runs[plane.runs.length - 1]
        if (last && lo <= last.to + JOINED) last.to = Math.max(last.to, hi)
        else plane.runs.push({ tx: -plane.nz, tz: plane.nx, from: lo, to: hi })
      }
    }
    // An end where the wall only bends a little is no corner.
    const ends = this.planes.flatMap((plane) => plane.runs.flatMap((run) => [run.from, run.to].map((t, end) => ({ plane, run, end, x: plane.nx * plane.d + run.tx * t, z: plane.nz * plane.d + run.tz * t }))))
    const soft = ends.filter((a) => ends.some((b) => b.plane !== a.plane && Math.hypot(a.x - b.x, a.z - b.z) < MEET && a.plane.nx * b.plane.nx + a.plane.nz * b.plane.nz > NO_CORNER))
    for (const { run, end } of soft) {
      if (end === 0) run.from -= FAR
      else run.to += FAR
    }
  }

  /** The plane a face with `normal` through `p` lies in; `make` it if there is none yet. Null for a face that is not upright. */
  private plane(normal: P, p: P, make: boolean): Plane | null {
    const n = level(normal)
    if (!n) return null
    const d = p[0] * n[0] + p[2] * n[1]
    for (const plane of this.planes) {
      if (plane.nx * n[0] + plane.nz * n[1] > SAME_WAY && Math.abs(p[0] * plane.nx + p[2] * plane.nz - plane.d) < SAME_PLANE) return plane
    }
    if (!make) return null
    const plane: Plane = { nx: n[0], nz: n[1], d, spans: [], runs: [] }
    this.planes.push(plane)
    return plane
  }

  /** The run of wall that a face with `normal` through `p` is part of, or null when it is part of none, or of one with no corner at either end. */
  of(normal: P, p: P): Run | null {
    const plane = this.plane(normal, p, false)
    if (!plane) return null
    const t = -p[0] * plane.nz + p[2] * plane.nx
    const run = plane.runs.find((r) => t >= r.from - MEET && t <= r.to + MEET)
    return run && run.to - run.from < FAR * 2 ? run : null
  }
}

/** How far `(x, z)` on `run` is from its two ends, metres: what a wall's vertex carries as its uv. */
export function endsAt(run: Run, x: number, z: number): [number, number] {
  const t = x * run.tx + z * run.tz
  return [Math.max(0, t - run.from), Math.max(0, run.to - t)]
}
