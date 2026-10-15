import { Geometry, Material, Mesh, type Ramp } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { ROAD_MAJOR, ROAD_MINOR, ROAD_PATH, type Road } from '@/world'
import { normalAt, toneAt, type Height } from './shade'

/** `[casing, fill]` per road kind; paths have no casing. */
const ROAD_COLORS: Record<number, [number | null, number]> = {
  [ROAD_MAJOR]: [0xc9a86a, 0xfff1c9],
  [ROAD_MINOR]: [0xbdbdbd, 0xffffff],
  [ROAD_PATH]: [null, 0xd8b98a],
}
/** How much wider than the road its casing is, metres. */
const CASING = 0.8
/** Along-road spacing of cross-sections, so a ribbon follows the ground's bumps, metres. */
const SPACING = 1
/** Largest gap between vertices across a road, and between a disc's rings, metres. */
const ACROSS = 1
/** Sides of the disc that rounds a joint or an end. */
const DISC = 12
/** Paths first, so the wider roads are drawn over them at junctions. */
const RANK: Record<number, number> = { [ROAD_PATH]: 0, [ROAD_MINOR]: 1, [ROAD_MAJOR]: 2 }

const materials = new Map<string, Material>()
function layerMaterial(color: number, offset: number, ramp: Ramp) {
  const key = `${color}:${offset}`
  let found = materials.get(key)
  if (!found) {
    // Each layer pulls further towards the camera, so overlapping roads settle in a fixed order and
    // clear the terrain at any distance; the lift alone covers the near field, where depth is fine.
    // A flat quad between samples cuts the terrain's kinks, so the lift must exceed that chord error.
    found = toonMaterial({ color, ramp, vertexColors: true, depthBias: offset, depthBiasSlope: 2 })
    materials.set(key, found)
  }
  return found
}

class Layer {
  readonly positions: number[] = []
  readonly normals: number[] = []
  readonly colors: number[] = []
  readonly indices: number[] = []

  constructor(
    private readonly height: Height,
    private readonly lift: number,
    private readonly step: number,
  ) {}

  vertex(x: number, z: number) {
    const normal = normalAt(this.height, x, z, this.step)
    const tone = toneAt(this.height, x, z, normal)
    this.positions.push(x, this.height(x, z) + this.lift, z)
    this.normals.push(...normal)
    this.colors.push(tone, tone, tone)
    return this.positions.length / 3 - 1
  }

  /** A triangle, wound so it faces up whichever way round its corners were given. */
  triangle(a: number, b: number, c: number) {
    const p = this.positions
    const up = (p[b * 3 + 2]! - p[a * 3 + 2]!) * (p[c * 3]! - p[a * 3]!) - (p[b * 3]! - p[a * 3]!) * (p[c * 3 + 2]! - p[a * 3 + 2]!)
    this.indices.push(...(up > 0 ? [a, b, c] : [a, c, b]))
  }

  /** A round patch of rings no more than `ACROSS` apart, so it keeps to the ground out to its rim. */
  disc(x: number, z: number, radius: number) {
    const rings = Math.max(1, Math.ceil(radius / ACROSS))
    const centre = this.vertex(x, z)
    let inner: number[] = []
    for (let k = 1; k <= rings; k++) {
      const r = (radius * k) / rings
      const ring = Array.from({ length: DISC }, (_, i) => {
        const angle = (i / DISC) * Math.PI * 2
        return this.vertex(x + Math.cos(angle) * r, z + Math.sin(angle) * r)
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
    const across = Math.max(2, Math.ceil((2 * half) / ACROSS))
    let previous: number[] | null = null
    for (let k = 0; k <= parts; k++) {
      const x = ax + ((bx - ax) * k) / parts
      const z = az + ((bz - az) * k) / parts
      const section: number[] = []
      for (let j = 0; j <= across; j++) {
        const side = ((2 * j) / across - 1) * half
        section.push(this.vertex(x + px * side, z + pz * side))
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

  mesh(material: Material): Mesh | null {
    if (this.indices.length === 0) return null
    const geometry = new Geometry({
      positions: Float32Array.from(this.positions),
      normals: Float32Array.from(this.normals),
      colors: Float32Array.from(this.colors),
      indices: Uint32Array.from(this.indices),
    })
    const mesh = new Mesh(geometry, material)
    mesh.receiveShadow = true
    return mesh
  }
}

/**
 * Roads as ribbons laid on the terrain, so their edges are as sharp as the screen is, however
 * close the camera gets (a texture would show its texels). One mesh per kind and layer: all the
 * casings, then all the fills, so a junction reads as one surface rather than a stack of outlines.
 * `border` is the tile's `[x0, z0, x1, z1]`; a road ending on it gets no round cap, since its
 * neighbour tile carries on from there.
 */
export function buildRoads(roads: readonly Road[], height: Height, step: number, border: readonly [number, number, number, number], ramp: Ramp): Mesh[] {
  const meshes: Mesh[] = []
  for (const pass of [0, 1] as const) {
    for (const kind of [ROAD_PATH, ROAD_MINOR, ROAD_MAJOR]) {
      const colors = ROAD_COLORS[kind]!
      const color = colors[pass]
      if (color === null || color === undefined) continue
      const rank = RANK[kind]!
      const layer = new Layer(height, (pass === 0 ? 0.05 : 0.07) + rank * 0.005, step)
      for (const road of roads) {
        if (road.kind !== kind) continue
        const half = (road.width + (pass === 0 ? CASING : 0)) / 2
        const { points } = road
        for (let i = 0; i + 1 < points.length; i++) layer.segment(points[i]![0], points[i]![1], points[i + 1]![0], points[i + 1]![1], half)
        for (const [x, z] of points) {
          const onBorder = Math.abs(x - border[0]) < 0.01 || Math.abs(x - border[2]) < 0.01 || Math.abs(z - border[1]) < 0.01 || Math.abs(z - border[3]) < 0.01
          if (!onBorder) layer.disc(x, z, half)
        }
      }
      const mesh = layer.mesh(layerMaterial(color, (pass === 0 ? 8 : 20) + rank * 2, ramp))
      if (mesh) meshes.push(mesh)
    }
  }
  return meshes
}
