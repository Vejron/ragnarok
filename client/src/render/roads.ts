import { ROAD_FAINT, ROAD_GRAVEL, ROAD_MAJOR, ROAD_MINOR, ROAD_PATH, ROAD_TRACK, type Point2, type Road } from '@/world'
import { winter } from './season'
import { toneOf, type GroundShade } from './shade'
import { value } from './marshnoise'

/*
 * Roads as ribbons draped on the ground, as plain arrays: built in the tile worker
 * (`tile.worker.ts`) or, without one, on the main thread, and made meshes in `world.ts`
 * (`ribbonMeshes`). Nothing here touches the scene, so it loads in a worker.
 */

/**
 * `[casing, fill]` per road kind; paths have no casing. A paved road's casing is its gravel shoulder,
 * and its fill is asphalt (`PAVED`).
 */
const ROAD_COLORS: Record<number, [number | null, number]> = {
  [ROAD_MAJOR]: [0x9a9486, 0x6a6a6c],
  [ROAD_MINOR]: [0x9a9486, 0x6e6e70],
  [ROAD_PATH]: [null, 0xd8b98a],
  [ROAD_GRAVEL]: [null, 0xc9bca4],
  // A path wide enough to drive: the same worn dirt.
  [ROAD_TRACK]: [null, 0xd8b98a],
  [ROAD_FAINT]: [null, 0xa9a274],
}
/** Ploughed: packed snow with a little grit in it. */
const WINTER_ROAD_COLORS: Record<number, [number | null, number | null]> = {
  [ROAD_MAJOR]: [0xc6ccd4, 0xd8dde3],
  [ROAD_MINOR]: [0xd5dae1, 0xe4e8ed],
  // Paths lie under the snow: where one is skied, its ski track is what shows.
  [ROAD_PATH]: [null, null],
  [ROAD_GRAVEL]: [0xd5dae1, 0xe4e8ed],
  [ROAD_TRACK]: [null, null],
  [ROAD_FAINT]: [null, null],
}
/** How much wider than the road its casing is, metres. */
const CASING = 1.2
/** The kinds drawn as asphalt in summer, with a gravel shoulder for a casing. */
const PAVED = new Set([ROAD_MAJOR, ROAD_MINOR])
/** How raggedly a paved road's gravel shoulder breaks up into the verge (`Material.worn`), 0 to 1. */
const SHOULDER_RAGGED = 0.1
/** Along-road spacing of cross-sections, so a ribbon follows the ground's bumps, metres. */
const SPACING = 1
/** Largest gap between vertices across a road, and between a disc's rings, metres. */
const ACROSS = 1
/** Sides of the disc that rounds a joint or an end. */
const DISC = 12
/** Paths first, so the wider roads are drawn over them at junctions. */
const RANK: Record<number, number> = { [ROAD_FAINT]: 0, [ROAD_PATH]: 0, [ROAD_TRACK]: 0, [ROAD_GRAVEL]: 1, [ROAD_MINOR]: 1, [ROAD_MAJOR]: 2 }
const KINDS = [ROAD_FAINT, ROAD_PATH, ROAD_TRACK, ROAD_GRAVEL, ROAD_MINOR, ROAD_MAJOR]
/** How raggedly a worn kind's edges break up into the ground beside it (`Material.worn`), 0 to 1. */
const RAGGED: Record<number, number> = { [ROAD_FAINT]: 1, [ROAD_PATH]: 0.6, [ROAD_TRACK]: 0.45 }
/** How much a worn path's width strays either way, as a share of it, and over how many metres it changes. */
const WIDTH_JITTER = 0.3
const WIDTH_SPAN = 9

/** A worn path's half width `half` where it passes `(x, z)`: wider and narrower by turns, the same in every tile. */
function wornHalf(half: number, x: number, z: number): number {
  return half * (1 + WIDTH_JITTER * (2 * value(x / WIDTH_SPAN + 71, z / WIDTH_SPAN + 23) - 1))
}

/** How far `(px, pz)` is from the segment `a -> b`, metres. */
function toSegment(px: number, pz: number, ax: number, az: number, bx: number, bz: number): number {
  const [dx, dz] = [bx - ax, bz - az]
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (pz - az) * dz) / (dx * dx + dz * dz || 1)))
  return Math.hypot(px - ax - dx * t, pz - az - dz * t)
}

/** Side of the cells `Reach` sorts its strips into, metres. */
const REACH_CELL = 16

/**
 * How deep inside the surface some ribbons make together a point is: the most, over their strips, of
 * a strip's half width less how far the point is from its centre line, metres. Where ribbons overlap
 * at a junction, one strip's edge lying inside another is deep inside the whole, so it is drawn as the
 * middle of the road rather than as an edge that shows across the other.
 */
export class Reach {
  /** Each strip: its ends and half width, five numbers. */
  private readonly strips: number[] = []
  /** Which strips come near each cell of a `REACH_CELL` grid, by `cellKey`. */
  private readonly cells = new Map<number, number[]>()

  constructor(
    ribbons: readonly { points: readonly Point2[]; half: number }[],
    /** Whether the ribbons' widths stray along them as trodden paths' do (`wornHalf`). */
    private readonly wanders: boolean,
  ) {
    for (const { points, half } of ribbons) {
      const ends = points.length === 1 ? [points[0]!, points[0]!] : points
      for (let i = 0; i + 1 < ends.length; i++) {
        const [[ax, az], [bx, bz]] = [ends[i]!, ends[i + 1]!]
        const strip = this.strips.length / 5
        this.strips.push(ax, az, bx, bz, half)
        const reach = half * (wanders ? 1 + WIDTH_JITTER : 1)
        for (let cx = Math.floor((Math.min(ax, bx) - reach) / REACH_CELL); cx <= Math.floor((Math.max(ax, bx) + reach) / REACH_CELL); cx++) {
          for (let cz = Math.floor((Math.min(az, bz) - reach) / REACH_CELL); cz <= Math.floor((Math.max(az, bz) + reach) / REACH_CELL); cz++) {
            const key = cellKey(cx, cz)
            const list = this.cells.get(key)
            if (list) list.push(strip)
            else this.cells.set(key, [strip])
          }
        }
      }
    }
  }

  /** How deep inside the surface `(x, z)` is, metres; negative outside it. */
  depth(x: number, z: number): number {
    let deepest = -Infinity
    for (const strip of this.cells.get(cellKey(Math.floor(x / REACH_CELL), Math.floor(z / REACH_CELL))) ?? []) {
      const s = strip * 5
      const half = this.wanders ? wornHalf(this.strips[s + 4]!, x, z) : this.strips[s + 4]!
      deepest = Math.max(deepest, half - toSegment(x, z, this.strips[s]!, this.strips[s + 1]!, this.strips[s + 2]!, this.strips[s + 3]!))
    }
    return deepest
  }
}

/** A `Reach` cell's key: its column and row packed into one number. */
function cellKey(cx: number, cz: number): number {
  return (cx + 32768) * 65536 + (cz + 32768)
}

/** How a road kind's casing (`pass` 0) or fill (1) is drawn: a worn path, asphalt and its shoulder, or whole. */
function lookOf(kind: number, pass: 0 | 1): Look {
  const ragged = RAGGED[kind]
  if (ragged !== undefined) return { kind: 'worn', ragged, wanders: true }
  if (winter || !PAVED.has(kind)) return PLAIN
  return pass === 0 ? { kind: 'worn', ragged: SHOULDER_RAGGED, wanders: false } : { kind: 'asphalt' }
}

/** Half the width of a road's casing (`pass` 0) or fill (1), metres. */
function halfWidth(road: Road, pass: 0 | 1): number {
  return (road.width + (pass === 0 ? CASING : 0)) / 2
}

/**
 * How a layer is drawn: whole, as a worn path whose edges break up into the ground (how raggedly, 0
 * to 1, `Material.worn`, and whether its width wanders as a trodden path's does), or as asphalt
 * (`Material.asphalt`).
 */
export type Look = { kind: 'plain' } | { kind: 'worn'; ragged: number; wanders: boolean } | { kind: 'asphalt' }

/** A layer drawn whole, in its colour and the terrain's tone. */
export const PLAIN: Look = { kind: 'plain' }

/**
 * One layer's triangles: corners in metres, a normal and a grey tone a vertex, and indices. A worn or
 * asphalt layer also has `custom` (`Layer.custom`) and `uvs`, where each vertex is on its tile's cover
 * map, so its edges can fade into the ground's own colour (`Material.ground`).
 */
export interface RibbonBuffers {
  positions: Float32Array
  normals: Float32Array
  colors: Float32Array
  custom?: Float32Array
  uvs?: Float32Array
  indices: Uint32Array
}

/**
 * One layer of ribbons and what it is drawn with: its `color`, its `look`, and `offset`, the depth
 * bias that keeps overlapping layers in a fixed order (`layerMaterial`).
 */
export interface Ribbon {
  buffers: RibbonBuffers
  color: number
  look: Look['kind']
  offset: number
}

/** Numbers appended three at a time to a typed array that doubles when full. */
class Triples<A extends Float32Array | Uint32Array> {
  length = 0

  constructor(public data: A) {}

  push(a: number, b: number, c: number) {
    if (this.length + 3 > this.data.length) {
      const grown = new (this.data.constructor as new (length: number) => A)(this.data.length * 2)
      grown.set(this.data)
      this.data = grown
    }
    this.data[this.length] = a
    this.data[this.length + 1] = b
    this.data[this.length + 2] = c
    this.length += 3
  }

  /** What was appended, in an array of its own size. */
  array(): A {
    return this.data.slice(0, this.length) as A
  }
}

/** One material's worth of ground-hugging triangles, toned like the terrain under them. */
export class Layer {
  private readonly positions = new Triples(new Float32Array(3 * 1024))
  private readonly normals = new Triples(new Float32Array(3 * 1024))
  private readonly colors = new Triples(new Float32Array(3 * 1024))
  /**
   * A worn or asphalt layer's `custom`, four a vertex: where it is across the ribbon (-1 to 1), how
   * ragged the edges are (0 for asphalt), the terrain's tone there, and the ribbon's half width, metres.
   */
  private readonly custom: number[] = []
  /** A worn or asphalt layer's place on its tile's cover map, two a vertex. */
  private readonly uvs: number[] = []
  private readonly indices = new Triples(new Uint32Array(3 * 1024))

  constructor(
    private readonly lift: number,
    /** The tile's shading, shared by its layers (`GroundShade`). */
    private readonly shade: GroundShade,
    /** How the layer is drawn. */
    private readonly look: Look = PLAIN,
    /** The tile's `[x0, z0, x1, z1]`, which its cover map spans, for a worn or asphalt layer's `uvs`. */
    private readonly border: readonly [number, number, number, number] = [0, 0, 1, 1],
    /**
     * The surface the layer's ribbons make together, for a worn or asphalt layer: where it has one,
     * a vertex is as far across as how deep into the whole surface it is says, not its own strip.
     */
    private readonly reach: Reach | null = null,
  ) {}

  /** A worn layer's: how raggedly its edges break up, or null for any other layer. */
  private get ragged(): number | null {
    return this.look.kind === 'worn' ? this.look.ragged : null
  }

  /** Whether the layer's width strays either way along it, as a trodden path's does (`wornHalf`). */
  private get wanders(): boolean {
    return this.look.kind === 'worn' && this.look.wanders
  }

  /**
   * A vertex on the ground at `(x, z)`, raised by `lift` (the layer's own unless given). `across` is
   * where it is across the ribbon, -1 to 1, and `half` the ribbon's half width there, metres, for a
   * worn or asphalt layer's edges.
   */
  vertex(x: number, z: number, lift = this.lift, across = 0, half = 0) {
    const normal = this.shade.normal(x, z)
    const y = this.shade.height(x, z)
    const tone = toneOf(normal, y, this.shade.mean(x, z), x, z)
    this.positions.push(x, y + lift, z)
    this.normals.push(normal[0], normal[1], normal[2])
    this.colors.push(tone, tone, tone)
    if (this.look.kind !== 'plain') {
      // An edge lying inside another strip, at a junction, is the middle of the road, not its edge.
      const inside = this.reach && half > 0 ? Math.max(0, Math.min(1, 1 - this.reach.depth(x, z) / half)) : Math.abs(across)
      this.custom.push(inside, this.ragged ?? 0, tone, half)
      const [x0, z0, x1, z1] = this.border
      this.uvs.push((x - x0) / (x1 - x0), (z - z0) / (z1 - z0))
    }
    return this.positions.length / 3 - 1
  }

  /** A triangle, wound so it faces up whichever way round its corners were given. */
  triangle(a: number, b: number, c: number) {
    const p = this.positions.data
    const up = (p[b * 3 + 2]! - p[a * 3 + 2]!) * (p[c * 3]! - p[a * 3]!) - (p[b * 3]! - p[a * 3]!) * (p[c * 3 + 2]! - p[a * 3 + 2]!)
    if (up > 0) this.indices.push(a, b, c)
    else this.indices.push(a, c, b)
  }

  /**
   * A round patch of rings no more than `ACROSS` apart, so it keeps to the ground out to its rim.
   * With a `reach`, its edges fade as the strips either side of it do, rather than in a circle of their own.
   */
  disc(x: number, z: number, radius: number) {
    const rings = Math.max(1, Math.ceil(radius / ACROSS))
    const centre = this.vertex(x, z, this.lift, 0, radius)
    let inner: number[] = []
    for (let k = 1; k <= rings; k++) {
      const r = (radius * k) / rings
      const ring = Array.from({ length: DISC }, (_, i) => {
        const angle = (i / DISC) * Math.PI * 2
        return this.vertex(x + Math.cos(angle) * r, z + Math.sin(angle) * r, this.lift, k / rings, radius)
      })
      for (let i = 0; i < DISC; i++) {
        const next = (i + 1) % DISC
        if (k === 1) this.triangle(centre, ring[i]!, ring[next]!)
        else {
          this.triangle(inner[i]!, ring[i]!, ring[next]!)
          this.triangle(inner[i]!, ring[next]!, inner[next]!)
        }
      }
      inner = ring
    }
  }

  /**
   * A strip along a -> b, `half` either side. It is cut across as well as along, because a flat quad
   * the width of the road cuts the ground's crown or ditch by centimetres, which is enough for the
   * wider casing to show through the fill (or the terrain through both).
   */
  segment(ax: number, az: number, bx: number, bz: number, half: number) {
    const length = Math.hypot(bx - ax, bz - az)
    if (length < 1e-4) return
    const px = -(bz - az) / length
    const pz = (bx - ax) / length
    const parts = Math.max(1, Math.ceil(length / SPACING))
    const across = Math.max(2, Math.ceil((2 * half * (this.wanders ? 1 + WIDTH_JITTER : 1)) / ACROSS))
    let previous: number[] | null = null
    for (let k = 0; k <= parts; k++) {
      const x = ax + ((bx - ax) * k) / parts
      const z = az + ((bz - az) * k) / parts
      const width = this.wanders ? wornHalf(half, x, z) : half
      const section: number[] = []
      for (let j = 0; j <= across; j++) {
        const t = (2 * j) / across - 1
        section.push(this.vertex(x + px * t * width, z + pz * t * width, this.lift, t, width))
      }
      if (previous) {
        for (let j = 0; j < across; j++) {
          this.triangle(previous[j]!, previous[j + 1]!, section[j]!)
          this.triangle(previous[j + 1]!, section[j + 1]!, section[j]!)
        }
      }
      previous = section
    }
  }

  /** The layer as a ribbon drawn in `color` with depth bias `offset`, or null when it has no triangles. */
  ribbon(color: number, offset: number): Ribbon | null {
    if (this.indices.length === 0) return null
    const buffers: RibbonBuffers = { positions: this.positions.array(), normals: this.normals.array(), colors: this.colors.array(), indices: this.indices.array() }
    if (this.look.kind !== 'plain') {
      buffers.custom = new Float32Array(this.custom)
      buffers.uvs = new Float32Array(this.uvs)
    }
    return { buffers, color, look: this.look.kind, offset }
  }
}

/**
 * Roads as ribbons laid on the terrain, so their edges are as sharp as the screen is, however
 * close the camera gets (a texture would show its texels). One ribbon per kind and layer: all the
 * casings, then all the fills, so a junction reads as one surface rather than a stack of outlines.
 * `border` is the tile's `[x0, z0, x1, z1]`; a road ending on it gets no round cap, since its
 * neighbour tile carries on from there. `shade` is the tile's (`GroundShade`).
 */
export function roadRibbons(roads: readonly Road[], shade: GroundShade, border: readonly [number, number, number, number]): Ribbon[] {
  const out: Ribbon[] = []
  for (const pass of [0, 1] as const) {
    for (const kind of KINDS) {
      const colors = (winter ? WINTER_ROAD_COLORS : ROAD_COLORS)[kind]!
      const color = colors[pass]
      if (color === null || color === undefined) continue
      const rank = RANK[kind]!
      const look = lookOf(kind, pass)
      // Paved roads make one surface whatever their class; a worn path's kind its own.
      const together = look.kind === 'plain' ? [] : roads.filter((road) => (PAVED.has(kind) ? PAVED.has(road.kind) : road.kind === kind))
      const reach = together.length > 0 ? new Reach(together.map((road) => ({ points: road.points, half: halfWidth(road, pass) })), RAGGED[kind] !== undefined) : null
      const layer = new Layer((pass === 0 ? 0.05 : 0.07) + rank * 0.005, shade, look, border, reach)
      for (const road of roads) {
        if (road.kind !== kind) continue
        const half = halfWidth(road, pass)
        const { points } = road
        for (let i = 0; i + 1 < points.length; i++) layer.segment(points[i]![0], points[i]![1], points[i + 1]![0], points[i + 1]![1], half)
        for (const [x, z] of points) {
          const onBorder = Math.abs(x - border[0]) < 0.01 || Math.abs(x - border[2]) < 0.01 || Math.abs(z - border[1]) < 0.01 || Math.abs(z - border[3]) < 0.01
          if (!onBorder) layer.disc(x, z, RAGGED[kind] === undefined ? half : wornHalf(half, x, z))
        }
      }
      const ribbon = layer.ribbon(color, (pass === 0 ? 8 : 20) + rank * 2)
      if (ribbon) out.push(ribbon)
    }
  }
  return out
}
