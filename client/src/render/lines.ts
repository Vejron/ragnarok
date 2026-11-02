import { Color, Geometry, Mesh, ShaderMaterial, boxGeometry, cylinderGeometry } from '@ragnarok/engine'
import { heightIn, type TileGround } from '@/sim/terrain'
import { LINE_FENCE, LINE_FENCE_HIGH, LINE_POWER, LINE_POWER_MAJOR, type Line, type Point2 } from '@/world'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * Fences and power lines (`TileData.lines`): posts and poles as one solid mesh a tile, the wires as
 * ribbons that turn to face the eye, and a game fence's netting as panels drawn by the shader. A wire
 * is never drawn thinner than about a pixel: where it would be, it is drawn that wide and as much
 * fainter, so it fades with distance instead of breaking into dashes.
 */

type V3 = [number, number, number]

/** Metres between a wire fence's posts, and a game fence's. */
const FENCE_STEP = 3
const GAME_FENCE_STEP = 4
/** Heights of a wire fence's strands. */
const STRANDS = [0.4, 0.75, 1.1]
/** Top of a game fence's netting, and its foot. */
const NET_TOP = 2
const NET_FOOT = 0.05
/** Sag of a power line's wires, as a share of the span. */
const SAG = 0.018
/** Sections of a sagging wire. */
const WIRE_SECTIONS = 10
/** A wire's radius is its colour's alpha times this, metres. */
const RADIUS_SCALE = 0.02
/** Half the least width a wire is drawn at, pixels. */
const MIN_HALF_PX = 0.7
/** How far a wire swings in the breeze mid-span, metres. */
const SWAY = 0.12

const WOOD = 0x6b5a45
const TARRED = 0x3d3328
const STEEL = 0x8c9294
const PORCELAIN = 0xd8dcd6
const WIRE = 0x6e7376
const CABLE = 0x55595b

/** Points along a line every `step` metres, its ends included. */
export function along(points: readonly Point2[], step: number): Point2[] {
  const out: Point2[] = [points[0]!]
  let carried = 0
  for (let i = 1; i < points.length; i++) {
    const [ax, az] = points[i - 1]!
    const [bx, bz] = points[i]!
    const length = Math.hypot(bx - ax, bz - az)
    let at = step - carried
    for (; at < length; at += step) out.push([ax + ((bx - ax) * at) / length, az + ((bz - az) * at) / length])
    carried = length - (at - step)
  }
  const last = points[points.length - 1]!
  const before = out[out.length - 1]!
  // The end gets a post, unless one stands just short of it.
  if (Math.hypot(last[0] - before[0], last[1] - before[1]) < step * 0.4 && out.length > 1) out.pop()
  out.push(last)
  return out
}

/** A part in `color`, without uvs, so the parts merge. */
function painted(geometry: Geometry, color: number): Geometry {
  geometry.uvs = undefined
  const c = Color.hex(color)
  const colors = new Float32Array(geometry.vertexCount * 3)
  for (let i = 0; i < geometry.vertexCount; i++) colors.set([c.r, c.g, c.b], i * 3)
  geometry.colors = colors
  return geometry
}

/** A post from `y` down into the ground and `height` up. */
function post(x: number, y: number, z: number, height: number, radius: number, color: number, sides = 6): Geometry {
  const sunk = 0.4
  return painted(cylinderGeometry(radius * 0.85, radius, height + sunk, sides).translate(x, y + (height - sunk) / 2, z), color)
}

/** A box `width` across the line at `yaw`, centred at `(x, y, z)`. */
function beam(x: number, y: number, z: number, yaw: number, width: number, height: number, depth: number, color: number): Geometry {
  return painted(boxGeometry(depth, height, width).rotateY(yaw).translate(x, y, z), color)
}

/** Wire ribbons: each vertex on the wire's centre line, pushed out to the side in the shader. */
class Wires {
  positions: number[] = []
  colors: number[] = []
  custom: number[] = []
  indices: number[] = []

  /** A wire from `a` to `b`, sagging `sag` metres mid-span, of `radius` metres. */
  add(a: V3, b: V3, sag: number, radius: number, color: number, sections: number) {
    const c = Color.hex(color)
    const first = this.positions.length / 3
    for (let i = 0; i <= sections; i++) {
      const t = i / sections
      const drop = 4 * sag * t * (1 - t)
      const p: V3 = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t - drop, a[2] + (b[2] - a[2]) * t]
      // The slope of the sagging wire here, and how free it is to swing.
      const tangent: V3 = [b[0] - a[0], b[1] - a[1] - 4 * sag * (1 - 2 * t), b[2] - a[2]]
      const swing = sag > 0 ? 4 * t * (1 - t) : 0
      for (const side of [-1, 1]) {
        this.positions.push(...p)
        this.colors.push(c.r, c.g, c.b, radius / RADIUS_SCALE)
        this.custom.push(...tangent, side * (1 + swing))
      }
      if (i > 0) {
        const k = first + (i - 1) * 2
        this.indices.push(k, k + 1, k + 2, k + 1, k + 3, k + 2)
      }
    }
  }

  geometry(): Geometry | null {
    if (this.indices.length === 0) return null
    const count = this.positions.length / 3
    const normals = new Float32Array(count * 3)
    for (let i = 0; i < count; i++) normals[i * 3 + 1] = 1
    const geometry = new Geometry({
      positions: Float32Array.from(this.positions),
      normals,
      colors: Float32Array.from(this.colors),
      custom: Float32Array.from(this.custom),
      indices: Uint32Array.from(this.indices),
    })
    // Pushed out to a pixel's width in the shader, and swung by the wind: far past the centre lines.
    geometry.bounds = { ...geometry.bounds, radius: geometry.bounds.radius + 3 }
    return geometry
  }
}

/** A game fence's netting: one panel between each pair of posts, `uv` being metres along and up. */
class Netting {
  positions: number[] = []
  normals: number[] = []
  uvs: number[] = []
  indices: number[] = []
  run = 0

  add(a: V3, b: V3) {
    const length = Math.hypot(b[0] - a[0], b[2] - a[2])
    if (length < 1e-3) return
    const n: V3 = [-(b[2] - a[2]) / length, 0, (b[0] - a[0]) / length]
    const k = this.positions.length / 3
    for (const [p, u] of [[a, this.run], [b, this.run + length]] as const) {
      this.positions.push(p[0], p[1] + NET_FOOT, p[2], p[0], p[1] + NET_TOP, p[2])
      this.normals.push(...n, ...n)
      this.uvs.push(u, NET_FOOT, u, NET_TOP)
    }
    this.indices.push(k, k + 2, k + 1, k + 1, k + 2, k + 3)
    this.run += length
  }

  geometry(): Geometry | null {
    if (this.indices.length === 0) return null
    return new Geometry({ positions: Float32Array.from(this.positions), normals: Float32Array.from(this.normals), uvs: Float32Array.from(this.uvs), indices: Uint32Array.from(this.indices) })
  }
}

/**
 * The ribbons' shader. `displace` swings a wire in the breeze, most mid-span, then pushes each vertex
 * out across the wire, square to the eye, by the wire's radius or half a pixel and a bit, whichever is
 * more; the fragment works out the same width and draws the wire as faint as it is narrower than that.
 */
const WIRE_COMMON_WGSL = /* wgsl */ `
fn wireAcross(along: vec3f, at: vec3f) -> vec3f {
  let across = cross(along, frame.cameraPos.xyz - at);
  return across / max(length(across), 1e-5);
}

// Half the width a wire of \`radius\` is drawn at, at \`at\`: never under ${MIN_HALF_PX} px.
fn wireHalf(at: vec3f, across: vec3f, radius: f32) -> f32 {
  let c0 = frame.viewProj * vec4f(at, 1.0);
  let c1 = frame.viewProj * vec4f(at + across * 0.05, 1.0);
  if (c0.w < 0.05 || c1.w < 0.05) {
    return radius;
  }
  let pixels = length((c1.xy / c1.w - c0.xy / c0.w) * frame.viewport.xy) * 0.5 / 0.05;
  return max(radius, ${MIN_HALF_PX.toFixed(2)} / max(pixels, 1e-3));
}
`

const WIRE_VERTEX_WGSL = /* wgsl */ `
${WIRE_COMMON_WGSL}

fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let side = sign(custom.w);
  let swing = abs(custom.w) - 1.0;
  let along = normalize(custom.xyz);
  let aside = normalize(vec3f(-along.z, 0.0, along.x) + vec3f(1e-6, 0.0, 0.0));
  let t = frame.fog.w;
  let gust = sin(t * 0.8 + dot(world.xz, vec2f(0.031, 0.047))) * 0.7 + sin(t * 1.9 + world.x * 0.11) * 0.3;
  let p = world + aside * swing * ${SWAY.toFixed(2)} * gust;
  let across = wireAcross(along, p);
  return p + across * side * wireHalf(p, across, color.a * ${RADIUS_SCALE});
}
`

// \`wireAcross\` and \`wireHalf\` come with the vertex code: the two are one module.
const WIRE_WGSL = /* wgsl */ `
fn surface(s: Surface) -> vec4f {
  let along = normalize(s.custom.xyz);
  let radius = s.color.a * ${RADIUS_SCALE};
  let cover = clamp(radius / wireHalf(s.world, wireAcross(along, s.world), radius), 0.0, 1.0);
  // Round: lit whichever way it faces, unless the sun shines along it.
  let sun = 1.0 - abs(dot(along, frame.sunDir.xyz));
  let shade = sunShadowTap(s.world, vec3f(0.0, 1.0, 0.0));
  let lit = frame.sunColor.rgb * (0.3 + 0.55 * sun) * shade + mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.7);
  return vec4f(s.color.rgb / PI * lit, pow(cover, 0.75));
}
`

/**
 * Wire netting, graduated as a game fence's is: the rows close together at the foot, where small
 * animals would get through, and wider toward the top; uprights every 15 cm. Each wire is as wide as
 * it covers of the pixel, and past where the wires blur together the netting is a faint grey veil.
 */
const NET_WGSL = /* wgsl */ `
fn netLines(coord: f32, width: f32) -> f32 {
  let w = max(fwidth(coord), 1e-4);
  let f = abs(fract(coord + 0.5) - 0.5);
  let sharp = clamp((width * 0.5 - f) / w + 0.5, 0.0, 1.0);
  return mix(sharp, width, smoothstep(0.35, 1.0, w));
}

fn surface(s: Surface) -> vec4f {
  let h = max(s.uv.y, 0.0);
  let spacing = 0.08 * (0.4 + h);
  let rows = netLines(log(1.0 + h / 0.4) / 0.08, 0.004 / spacing);
  let uprights = netLines(s.uv.x / 0.15, 0.004 / 0.15);
  let veil = 0.1 * smoothstep(4.0, 40.0, s.viewDepth);
  let cover = max(max(rows, uprights), veil);
  let shade = sunShadowTap(s.world, vec3f(0.0, 1.0, 0.0));
  let lit = frame.sunColor.rgb * 0.6 * shade + mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.6);
  return vec4f(s.color.rgb / PI * lit, cover);
}
`

let materials: { solid: ReturnType<typeof toonMaterial>; wire: ShaderMaterial; net: ShaderMaterial } | undefined

function shared() {
  materials ??= {
    solid: toonMaterial({ vertexColors: true, snow: winter }),
    wire: new ShaderMaterial({ wgsl: WIRE_WGSL, vertex: WIRE_VERTEX_WGSL, vertexColors: true, blend: 'alpha', side: 'double', shadows: true }),
    net: new ShaderMaterial({ wgsl: NET_WGSL, color: STEEL, blend: 'alpha', side: 'double', shadows: true }),
  }
  return materials
}

/** The direction across a power line at pole `i`: square to the line, halfway between its spans at a bend. */
function crossing(points: readonly Point2[], i: number): number {
  const heading = (a: Point2, b: Point2) => Math.atan2(b[1] - a[1], b[0] - a[0])
  const back = i > 0 ? heading(points[i - 1]!, points[i]!) : null
  const ahead = i + 1 < points.length ? heading(points[i]!, points[i + 1]!) : null
  const mean = back === null ? ahead! : ahead === null ? back : back + Math.atan2(Math.sin(ahead - back), Math.cos(ahead - back)) / 2
  // `rotateY` turns +Z toward +X, and the beam is wide along Z.
  return -mean
}

/** Where on a pole its wires hang, `[across, up]` metres, and their radius. */
const POWER_WIRES: Record<number, { at: [number, number][]; radius: number }[]> = {
  [LINE_POWER]: [{ at: [[-0.95, 9.25], [0, 9.8], [0.95, 9.25]], radius: 0.006 }],
  [LINE_POWER_MAJOR]: [
    { at: [[-5, 18.6], [0, 18.6], [5, 18.6]], radius: 0.014 },
    // The earth wires, over the top.
    { at: [[-4.5, 22.6], [4.5, 22.6]], radius: 0.006 },
  ],
}

/** The parts of one pole or pylon at `(x, y, z)`, its crossarm at `yaw`. */
function pole(kind: number, x: number, y: number, z: number, yaw: number): Geometry[] {
  const side = (across: number): [number, number] => [x + Math.sin(yaw) * across, z + Math.cos(yaw) * across]
  if (kind === LINE_POWER) {
    const parts = [post(x, y, z, 9.5, 0.13, TARRED, 7), beam(x, y + 8.9, z, yaw, 2.3, 0.12, 0.12, STEEL)]
    for (const [across, up] of POWER_WIRES[LINE_POWER]![0]!.at) {
      const [px, pz] = side(across)
      const foot = across === 0 ? 9.5 : 8.96
      parts.push(painted(cylinderGeometry(0.05, 0.07, up - foot, 6).translate(px, y + (foot + up) / 2, pz), PORCELAIN))
    }
    return parts
  }
  // A portal pylon: two legs and a beam, the phases hanging from strings of insulators.
  const parts: Geometry[] = []
  for (const across of [-5.8, 5.8]) {
    const [px, pz] = side(across)
    parts.push(painted(cylinderGeometry(0.35, 0.6, 23.4, 4).translate(px, y + 11.3, pz), STEEL))
  }
  parts.push(beam(x, y + 22, z, yaw, 13, 0.9, 0.9, STEEL))
  for (const [across] of POWER_WIRES[LINE_POWER_MAJOR]![0]!.at) {
    const [px, pz] = side(across)
    parts.push(painted(cylinderGeometry(0.12, 0.12, 3, 6).translate(px, y + 20.05, pz), PORCELAIN))
  }
  return parts
}

/** Ground height under `(x, z)`: the tile's own, or the line's where the world is flat. */
function groundAt(ground: TileGround | null, x: number, z: number, fallback: number): number {
  return ground ? heightIn(ground, x, z) : fallback
}

/**
 * The meshes of a tile's fences and power lines: one of posts and poles, one of wires and one of
 * netting, each only if there is any. The materials are shared by every tile.
 */
export function buildLines(lines: readonly Line[] | undefined, ground: TileGround | null): Mesh[] {
  if (!lines || lines.length === 0) return []
  const parts: Geometry[] = []
  const wires = new Wires()
  const net = new Netting()
  for (const line of lines) {
    const { kind, points, base } = line
    if (points.length < 2) continue
    if (kind === LINE_FENCE || kind === LINE_FENCE_HIGH) {
      const high = kind === LINE_FENCE_HIGH
      const posts = along(points, high ? GAME_FENCE_STEP : FENCE_STEP).map(([x, z]): V3 => [x, groundAt(ground, x, z, base[0] ?? 0), z])
      net.run = 0
      posts.forEach(([x, y, z], i) => {
        parts.push(post(x, y, z, high ? 2.2 : 1.25, high ? 0.07 : 0.05, WOOD))
        const next = posts[i + 1]
        if (!next) return
        if (high) {
          net.add([x, y, z], next)
          wires.add([x, y + NET_TOP + 0.05, z], [next[0], next[1] + NET_TOP + 0.05, next[2]], 0, 0.002, WIRE, 1)
        } else {
          for (const h of STRANDS) wires.add([x, y + h, z], [next[0], next[1] + h, next[2]], 0, 0.0018, WIRE, 1)
        }
      })
      continue
    }
    const hangs = POWER_WIRES[kind]
    if (!hangs) continue
    const yaws = points.map((_, i) => crossing(points, i))
    const at = (i: number, across: number, up: number): V3 => [points[i]![0] + Math.sin(yaws[i]!) * across, base[i]! + up, points[i]![1] + Math.cos(yaws[i]!) * across]
    for (let i = 0; i < points.length; i++) {
      if (i < points.length - 1 || line.end) parts.push(...pole(kind, points[i]![0], base[i]!, points[i]![1], yaws[i]!))
      if (i === 0) continue
      const span = Math.hypot(points[i]![0] - points[i - 1]![0], points[i]![1] - points[i - 1]![1])
      for (const { at: hang, radius } of hangs) {
        for (const [across, up] of hang) wires.add(at(i - 1, across, up), at(i, across, up), span * SAG, radius, CABLE, WIRE_SECTIONS)
      }
    }
  }
  const { solid, wire, net: netting } = shared()
  const meshes: Mesh[] = []
  if (parts.length > 0) {
    const mesh = new Mesh(Geometry.merge(parts), solid)
    mesh.castShadow = true
    mesh.receiveShadow = true
    meshes.push(mesh)
  }
  const wireGeometry = wires.geometry()
  if (wireGeometry) meshes.push(new Mesh(wireGeometry, wire))
  const netGeometry = net.geometry()
  if (netGeometry) meshes.push(new Mesh(netGeometry, netting))
  return meshes
}
