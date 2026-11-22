import { ROAD_FAINT, ROAD_MAJOR, ROAD_MINOR, ROAD_PATH, ROAD_PAVED_PATH, ROAD_TRACK, type Point2, type Road } from '@/world'
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
  // One asphalt for every paved road, and a bridge's deck (`DECK_ASPHALT`): no change of tone where two meet.
  [ROAD_MAJOR]: [0x9a9486, 0x6e6e70],
  [ROAD_MINOR]: [0x9a9486, 0x6e6e70],
  [ROAD_PATH]: [null, 0xd8b98a],
  // A path wide enough to drive: the same worn dirt.
  [ROAD_TRACK]: [null, 0xd8b98a],
  [ROAD_FAINT]: [null, 0xa9a274],
  // A paved footway or cycleway: asphalt, with no shoulder.
  [ROAD_PAVED_PATH]: [null, 0x6e6e70],
}
/** Ploughed: packed snow with a little grit in it. */
const WINTER_ROAD_COLORS: Record<number, [number | null, number | null]> = {
  [ROAD_MAJOR]: [0xc6ccd4, 0xd8dde3],
  [ROAD_MINOR]: [0xd5dae1, 0xe4e8ed],
  // Paths lie under the snow: where one is skied, its ski track is what shows.
  [ROAD_PATH]: [null, null],
  [ROAD_TRACK]: [null, null],
  [ROAD_FAINT]: [null, null],
  [ROAD_PAVED_PATH]: [null, null],
}
/** How much wider than the road its casing is, metres. */
const CASING = 1.2
/**
 * How far in under the asphalt a paved road's shoulder reaches, metres: past the fill's crumbled edge
 * (`asphalt.wgsl`), and no farther, so the two never lie over each other where the ground is bumpy.
 */
const SHOULDER_UNDER = 0.25
/** How far a ribbon reaching a tile border runs on past it, metres, under its neighbour's: no seam shows. */
const BORDER_OVERLAP = 0.3
/**
 * What `custom.w` of an asphalt vertex carries its road's paint in (`asphalt.wgsl`): the half width
 * plus this times `profile`, so one number is the same at every vertex of a road and passes whole.
 */
export const PROFILE_SCALE = 32
/** The kinds drawn as asphalt in summer, a road's with a gravel shoulder for a casing. */
const PAVED = new Set([ROAD_MAJOR, ROAD_MINOR, ROAD_PAVED_PATH])
/** How raggedly a paved road's gravel shoulder breaks up into the verge (`Material.worn`), 0 to 1. */
const SHOULDER_RAGGED = 0.1
/** Along-road spacing of cross-sections, so a ribbon follows the ground's bumps, metres. */
const SPACING = 1
/** Largest gap between vertices across a road, and between a disc's rings, metres. */
const ACROSS = 1
/** Sides of the disc that rounds a joint or an end. */
const DISC = 12
/**
 * Paths first, so the wider roads are drawn over them at junctions. Each kind has a rank of its own:
 * two at one rank lie at one depth, and flicker through each other where they cross.
 */
const RANK: Record<number, number> = { [ROAD_FAINT]: 0, [ROAD_PATH]: 1, [ROAD_TRACK]: 2, [ROAD_PAVED_PATH]: 3, [ROAD_MINOR]: 4, [ROAD_MAJOR]: 5 }
/**
 * The sharpest bend whose two strips meet in a mitre, on the line that halves it, radians: sharper,
 * and the mitre's point would reach far out, so the strips end square under a round patch instead.
 */
const MITRE = Math.PI / 3
/** Over how far a road narrows to a narrower one it ends on, metres. */
const TAPER = 8
const KINDS = [ROAD_FAINT, ROAD_PATH, ROAD_TRACK, ROAD_PAVED_PATH, ROAD_MINOR, ROAD_MAJOR]
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

/**
 * Where a disc at a road's joint lies along the road: its `along` (metres from the way's start) and the
 * unit direction of the road there, so its vertices carry on the frame of the strips either side.
 */
interface Frame {
  along: number
  tx: number
  tz: number
}
const NO_FRAME: Frame = { along: 0, tx: 1, tz: 0 }

/** A road's paint packed in one small whole number for its asphalt (`asphalt.wgsl`): `marks` in seven bits, then `lanes` and `forward`, three bits each. */
export function paintProfile(road: Road): number {
  return (road.marks & 127) | (Math.min(road.lanes, 7) << 7) | (Math.min(road.forward, 7) << 10)
}

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
  /** The paint of the road being laid, packed (`paintProfile`), for an asphalt layer's vertices. */
  profile = 0

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
    /**
     * A surface the layer must not lie under (a shoulder under the asphalt): a triangle whose middle is
     * deeper inside it than `SHOULDER_UNDER` is left out, so the two never fight over which is on top.
     */
    private readonly under: Reach | null = null,
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
  vertex(x: number, z: number, lift = this.lift, across = 0, half = 0, along = 0, lateral = 0) {
    const normal = this.shade.normal(x, z)
    const y = this.shade.height(x, z)
    const tone = toneOf(normal, y, this.shade.mean(x, z))
    this.positions.push(x, y + lift, z)
    this.normals.push(normal[0], normal[1], normal[2])
    this.colors.push(tone, tone, tone)
    if (this.look.kind !== 'plain') {
      // An edge lying inside another strip, at a junction, is the middle of the road, not its edge.
      const inside = this.reach && half > 0 ? Math.max(0, Math.min(1, 1 - this.reach.depth(x, z) / half)) : Math.abs(across)
      // Asphalt has no ragged edge and reads its tone from the vertex colour: it carries where it is
      // along the road and across it instead, for its paint and patches.
      if (this.look.kind === 'asphalt') this.custom.push(inside, along, lateral, half + PROFILE_SCALE * this.profile)
      else this.custom.push(inside, this.ragged ?? 0, tone, half)
      const [x0, z0, x1, z1] = this.border
      this.uvs.push((x - x0) / (x1 - x0), (z - z0) / (z1 - z0))
    }
    return this.positions.length / 3 - 1
  }

  /** A triangle, wound so it faces up whichever way round its corners were given. */
  triangle(a: number, b: number, c: number) {
    const p = this.positions.data
    if (this.under && this.under.depth((p[a * 3]! + p[b * 3]! + p[c * 3]!) / 3, (p[a * 3 + 2]! + p[b * 3 + 2]! + p[c * 3 + 2]!) / 3) > SHOULDER_UNDER) return
    const up = (p[b * 3 + 2]! - p[a * 3 + 2]!) * (p[c * 3]! - p[a * 3]!) - (p[b * 3]! - p[a * 3]!) * (p[c * 3 + 2]! - p[a * 3 + 2]!)
    if (up > 0) this.indices.push(a, b, c)
    else this.indices.push(a, c, b)
  }

  /**
   * A round patch of rings no more than `ACROSS` apart, so it keeps to the ground out to its rim.
   * With a `reach`, its edges fade as the strips either side of it do, rather than in a circle of their own.
   */
  disc(x: number, z: number, radius: number, frame: Frame = NO_FRAME, hole = 0) {
    const rings = Math.max(1, Math.ceil((radius - hole) / ACROSS))
    const at = (px: number, pz: number, across: number) => {
      const [dx, dz] = [px - x, pz - z]
      return this.vertex(px, pz, this.lift, across, radius, frame.along + dx * frame.tx + dz * frame.tz, -dx * frame.tz + dz * frame.tx)
    }
    const centre = hole > 0 ? -1 : at(x, z, 0)
    let inner: number[] = hole > 0 ? Array.from({ length: DISC }, (_, i) => at(x + Math.cos((i / DISC) * Math.PI * 2) * hole, z + Math.sin((i / DISC) * Math.PI * 2) * hole, hole / radius)) : []
    for (let k = 1; k <= rings; k++) {
      const r = hole + ((radius - hole) * k) / rings
      const ring = Array.from({ length: DISC }, (_, i) => {
        const angle = (i / DISC) * Math.PI * 2
        return at(x + Math.cos(angle) * r, z + Math.sin(angle) * r, r / radius)
      })
      for (let i = 0; i < DISC; i++) {
        const next = (i + 1) % DISC
        if (k === 1 && hole === 0) this.triangle(centre, ring[i]!, ring[next]!)
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
   * wider casing to show through the fill (or the terrain through both). `startCut` and `endCut` slant
   * its ends for a mitre: how far along the road the end moves for each metre to the right across it.
   * `startHalf` and `endHalf` are the half widths it narrows to at its ends, where it meets a narrower road.
   */
  segment(ax: number, az: number, bx: number, bz: number, half: number, along = 0, hole = 0, startCut = 0, endCut = 0, startHalf = half, endHalf = half) {
    const length = Math.hypot(bx - ax, bz - az)
    if (length < 1e-4) return
    // Right of the way it runs, x east and z south: where `lateral` is positive.
    const px = -(bz - az) / length
    const pz = (bx - ax) / length
    const [tx, tz] = [(bx - ax) / length, (bz - az) / length]
    const parts = Math.max(1, Math.ceil(length / SPACING))
    // With a `hole`, two bands either side of the middle, from `hole` out to `half`.
    const sides: [number, number][] = hole > 0 ? [[-1, -hole / half], [hole / half, 1]] : [[-1, 1]]
    for (const [from, to] of sides) {
      const across = Math.max(hole > 0 ? 1 : 2, Math.ceil(((to - from) * half * (this.wanders ? 1 + WIDTH_JITTER : 1)) / ACROSS))
      let previous: number[] | null = null
      for (let k = 0; k <= parts; k++) {
        const x = ax + ((bx - ax) * k) / parts
        const z = az + ((bz - az) * k) / parts
        // Drawn in towards a narrower road it meets at either end, over `TAPER` metres or half its length.
        const run = Math.min(TAPER, length / 2)
        const [fromStart, fromEnd] = [(length * k) / parts, length - (length * k) / parts]
        const tapered = fromStart < run ? startHalf + ((half - startHalf) * fromStart) / run : fromEnd < run ? endHalf + ((half - endHalf) * fromEnd) / run : half
        const width = this.wanders ? wornHalf(half, x, z) : tapered
        const section: number[] = []
        for (let j = 0; j <= across; j++) {
          const t = from + ((to - from) * j) / across
          // Sheared towards each end's cut: how far along the road a point this far across it is moved.
          const lateral = t * width
          const shear = lateral * (startCut + ((endCut - startCut) * k) / parts)
          section.push(this.vertex(x + px * lateral + tx * shear, z + pz * lateral + tz * shear, this.lift, t, width, along + (length * k) / parts + shear, lateral))
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

/** Whether `(x, z)` lies on the tile's `border` (`[x0, z0, x1, z1]`), where a road carries on in the next tile. */
function onBorder([x, z]: Point2, border: readonly [number, number, number, number]): boolean {
  return Math.abs(x - border[0]) < 0.01 || Math.abs(x - border[2]) < 0.01 || Math.abs(z - border[1]) < 0.01 || Math.abs(z - border[3]) < 0.01
}

/**
 * `points` with an end on the tile's border moved `BORDER_OVERLAP` on past it, so the ribbon runs a
 * little under its neighbour tile's: the two pieces' cut ends need not line up to the millimetre.
 */
function pastBorders(points: readonly Point2[], border: readonly [number, number, number, number]): Point2[] {
  const out = points.slice()
  const extend = (end: number, next: number) => {
    const [[x, z], [nx, nz]] = [out[end]!, out[next]!]
    const length = Math.hypot(x - nx, z - nz)
    if (length > 1e-6 && onBorder([x, z], border)) out[end] = [x + ((x - nx) / length) * BORDER_OVERLAP, z + ((z - nz) / length) * BORDER_OVERLAP]
  }
  if (out.length > 1) {
    extend(0, 1)
    extend(out.length - 1, out.length - 2)
  }
  return out
}

/**
 * Whether `road` is the paving between two close carriageways, or of a fork (`barrier.rs` in the
 * importer): a main road with no lanes. It fills what the roads round it leave, as wide as it is
 * to its ends, whatever it meets there.
 */
function paving(road: Road): boolean {
  return road.kind === ROAD_MAJOR && road.lanes === 0
}

/**
 * The widest half width of the other roads of `road`'s surface (paved, or its own worn kind) that
 * have a point at `(x, z)`, where `road` ends, or null when it ends alone there.
 */
function meeting(roads: readonly Road[], road: Road, x: number, z: number, pass: 0 | 1): number | null {
  let widest: number | null = null
  for (const other of roads) {
    if (other === road || (PAVED.has(road.kind) ? !PAVED.has(other.kind) : other.kind !== road.kind)) continue
    if (other.points.some(([px, pz]) => Math.abs(px - x) < 0.05 && Math.abs(pz - z) < 0.05)) widest = Math.max(widest ?? 0, halfWidth(other, pass))
  }
  return widest
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
      // A paved road's shoulder is a band along each edge, not a sheet under the whole road, and none of it
      // lies under any road's asphalt: not at a joint's disc, nor where another road crosses.
      const shoulder = pass === 0 && PAVED.has(kind)
      const asphalt = shoulder ? new Reach(roads.filter((road) => PAVED.has(road.kind)).map((road) => ({ points: road.points, half: halfWidth(road, 1) })), false) : null
      const layer = new Layer((pass === 0 ? 0.05 : 0.07) + rank * 0.005, shade, look, border, reach, asphalt)
      for (const road of roads) {
        if (road.kind !== kind) continue
        const half = halfWidth(road, pass)
        const hole = shoulder ? road.width / 2 - SHOULDER_UNDER : 0
        const points = pastBorders(road.points, border)
        layer.profile = paintProfile(road)
        // A first point moved back past a border starts that much before the way's own `along`.
        let along = road.along - Math.hypot(points[0]![0] - road.points[0]![0], points[0]![1] - road.points[0]![1])
        // A bend is mitred: the strips either side of it are cut on the line that halves it, so they
        // meet edge to edge with nothing lying over anything. `cuts[i]` is that slant at point `i`
        // (`Layer.segment`), or null where the bend is too sharp, or the strips too short, for one.
        const heading = (i: number): [number, number] => {
          const [[ax, az], [bx, bz]] = [points[i]!, points[i + 1]!]
          const length = Math.hypot(bx - ax, bz - az) || 1
          return [(bx - ax) / length, (bz - az) / length]
        }
        const span = (i: number) => Math.hypot(points[i + 1]![0] - points[i]![0], points[i + 1]![1] - points[i]![1])
        const reachAcross = half * (RAGGED[kind] === undefined ? 1 : 1 + WIDTH_JITTER)
        const cuts: (number | null)[] = points.map((_, i) => {
          if (i === 0 || i === points.length - 1) return 0
          const [[ax, az], [bx, bz]] = [heading(i - 1), heading(i)]
          // Positive where the road turns right, x east and z south.
          const turn = Math.atan2(ax * bz - az * bx, ax * bx + az * bz)
          const cut = Math.tan(turn / 2)
          return Math.abs(turn) <= MITRE && reachAcross * Math.abs(cut) <= Math.min(span(i - 1), span(i)) / 2 ? cut : null
        })
        // Where an end meets a narrower road, the half width it narrows to there: else its own.
        const ends = [0, points.length - 1].map((i) => {
          if (onBorder(road.points[i]!, border) || RAGGED[kind] !== undefined || paving(road)) return half
          const met = meeting(roads, road, points[i]![0], points[i]![1], pass)
          return met !== null && met < half ? met : half
        }) as [number, number]
        const arcs = [along]
        for (let i = 0; i + 1 < points.length; i++) {
          const [[ax, az], [bx, bz]] = [points[i]!, points[i + 1]!]
          // The inside of a right turn ends early and starts late.
          layer.segment(ax, az, bx, bz, half, along, hole, cuts[i] ?? 0, -(cuts[i + 1] ?? 0), i === 0 ? ends[0] : half, i + 2 === points.length ? ends[1] : half)
          along += Math.hypot(bx - ax, bz - az)
          arcs.push(along)
        }
        points.forEach(([x, z], i) => {
          if (onBorder(road.points[i]!, border)) return
          const end = i === 0 || i === points.length - 1
          // A round patch at a bend too sharp to mitre, and at an end: a cap where the road stops, and
          // where it meets another, no wider than the narrower of the two, so that a wide road ending
          // on a narrow one stops square and does not bulge out past it.
          if (!end && cuts[i] !== null) return
          let radius = RAGGED[kind] === undefined ? half : wornHalf(half, x, z)
          if (end && !paving(road)) {
            const met = meeting(roads, road, x, z, pass)
            if (met !== null) radius = Math.min(radius, met)
          }
          if (radius <= hole) return
          const [ax, az] = points[Math.max(0, i - 1)]!
          const [bx, bz] = points[Math.min(points.length - 1, i + 1)]!
          const length = Math.hypot(bx - ax, bz - az) || 1
          const frame = { along: arcs[i]!, tx: (bx - ax) / length, tz: (bz - az) / length }
          layer.disc(x, z, radius, frame, Math.min(hole, radius))
        })
      }
      const ribbon = layer.ribbon(color, (pass === 0 ? 8 : 20) + rank * 2)
      if (ribbon) out.push(ribbon)
    }
  }
  return out
}
