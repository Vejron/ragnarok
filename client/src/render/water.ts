import { Color, Geometry, Group, Mesh, ShaderMaterial, Texture } from '@ragnarok/engine'
import type { TileGround } from '@/sim/terrain'
import { NO_WATER, type Stream, type TileData } from '@/world'
import { heightSampler, type Height, type HeightSource } from './shade'

/**
 * Water: lakes and rivers are a surface mesh per tile at the importer's water level, streams are
 * ribbons draped on the ground. Both share one toy-style shader: a tint that deepens with depth,
 * soft light bands drifting over the surface, a foam line along the shore, and a flow along a
 * stream's length. Depth is a vertex attribute (`level - ground`), not a depth-texture read, so the
 * look needs no extra pass.
 */

/** Shore foam reaches this deep, metres. */
const FOAM_DEPTH = 0.35
/** Ribbon cross-sections along a stream are this far apart, metres. */
const STREAM_STEP = 2
/** Vertices across a stream ribbon (an odd number, so there is a middle one). */
const STREAM_ACROSS = 5
/** How far a stream floats over the ground, metres. */
const STREAM_LIFT = 0.1
/** Depth drawn at the middle of a stream, metres. */
const STREAM_DEPTH = 0.45
/** Along-stream speed of the flow, metres per second. */
const FLOW_SPEED = 1.4

const WGSL = /* wgsl */ `
fn bands(p: vec2f, t: f32) -> f32 {
  let a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
  let b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
  let c = sin((p.x + p.y) * 0.25 + t * 0.4);
  return (a + b + 0.6 * c) / 2.6;
}

// Five crossing swells, long to short. \`fine\` (0 to 1) fades the short ones so far water doesn't shimmer.
fn swell(p: vec2f, t: f32, fine: f32) -> f32 {
  return 0.45 * sin(p.x * 0.9 + p.y * 0.35 + t * 0.9)
       + 0.35 * sin(p.x * -0.4 + p.y * 1.1 - t * 0.7)
       + 0.2 * sin((p.x + p.y) * 1.7 + t * 1.3)
       + fine * 0.14 * sin(p.x * 3.1 - p.y * 2.3 + t * 2.1)
       + fine * 0.1 * sin(p.x * -2.2 - p.y * 3.7 - t * 2.6);
}

// What the water mirrors: a horizon-to-zenith sky with a few soft clouds and the sun's halo.
fn skyAt(r: vec3f, t: f32) -> vec3f {
  let up = clamp(r.y, 0.0, 1.0);
  var c = mix(params.sky, params.sky * vec3f(0.55, 0.75, 1.05), pow(up, 0.55));
  let cp = r.xz / (r.y + 0.3) * 0.9 + vec2f(t * 0.03, 0.0);
  let cloud = smoothstep(0.5, 0.95, 0.5 + 0.3 * sin(cp.x * 1.3 + sin(cp.y * 1.1) * 1.6) + 0.25 * sin(cp.y * 2.1 + sin(cp.x * 1.7) * 1.2));
  c = mix(c, vec3f(1.0), cloud * 0.4 * smoothstep(0.02, 0.3, r.y));
  let s = max(dot(r, frame.sunDir.xyz), 0.0);
  return c + params.sun * (pow(s, 6.0) * 0.12 + pow(s, 200.0) * 1.2);
}

fn caustics(p: vec2f, t: f32) -> f32 {
  let a = abs(sin(p.x * 1.3 + sin(p.y * 0.9 + t * 0.8) * 1.5));
  let b = abs(sin(p.y * 1.1 + sin(p.x * 1.2 - t * 0.6) * 1.5));
  return pow(1.0 - a * b, 6.0);
}

fn surface(s: Surface) -> vec4f {
  let time = frame.fog.w;
  let depth = s.custom.x;
  let flow = s.custom.yzw;
  let world = s.world.xz;
  let stream = flow.z > 0.5;
  var p = world * 0.7;
  if (stream) {
    p = vec2f(flow.x - time * ${FLOW_SPEED.toFixed(2)}, flow.y * 3.0) * 2.2;
  }
  let wave = bands(p, select(time, 0.0, stream));
  let band = smoothstep(0.35, 0.5, wave) * 0.5 + smoothstep(0.6, 0.7, wave) * 0.5;

  let deep = smoothstep(0.0, 2.2, depth);
  var color = mix(params.shallow, params.deep, deep);

  let clear = 1.0 - smoothstep(0.0, 1.4, depth);
  let bedP = select(world * 0.9, p * 0.8, stream);
  let web = caustics(bedP, select(time * 0.8, time, stream));
  color = mix(color, params.bed, clear * 0.45);
  color += params.foam * web * clear * 0.35;
  color = mix(color, params.foam, band * 0.12);

  let e = 0.15;
  let q = select(world * 0.8, p, stream);
  let t = select(time, 0.0, stream);
  let to = frame.cameraPos.xyz - s.world;
  let fine = 1.0 - smoothstep(25.0, 110.0, length(to));
  let h = swell(q, t, fine);
  let grad = vec2f(swell(q + vec2f(e, 0.0), t, fine) - h, swell(q + vec2f(0.0, e), t, fine) - h) / e;
  var n = normalize(vec3f(-grad.x * 0.2, 1.0, -grad.y * 0.2));
  if (!s.front) {
    n.y = -n.y;
  }
  let view = normalize(to);

  // Schlick: little reflection looking straight down, nearly a mirror at a grazing angle.
  let ndv = clamp(abs(dot(n, view)), 0.0, 1.0);
  let fresnel = 0.03 + 0.97 * pow(1.0 - ndv, 5.0);
  let lit = floor(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
  color *= 0.9 + 0.2 * lit;
  var bounce = reflect(-view, n);
  bounce.y = abs(bounce.y);
  var mirror = skyAt(bounce, time);
  // The scene mirrored in the water's plane, drawn from the same view and flipped across (see the renderer).
  // Water at another height than the plane, or without a mirror, keeps the sky.
  let clip = frame.viewProj * vec4f(s.world, 1.0);
  let ndc = clip.xy / max(clip.w, 0.001);
  let ripple = vec2f(n.x, n.z) * 0.05 * fine;
  let ruv = clamp(vec2f(0.5 - ndc.x * 0.5, 0.5 - ndc.y * 0.5) + ripple, vec2f(0.0), vec2f(1.0));
  let seen = textureSampleLevel(reflection, materialSampler, ruv, 0.0);
  let level = params.plane.y * (1.0 - smoothstep(0.2, 0.5, abs(s.world.y - params.plane.x)));
  mirror = mix(mirror, seen.rgb, seen.a * level);
  color = mix(color, mirror, clamp(fresnel * 1.3 + 0.12, 0.0, 0.95));

  let glint = pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 90.0);
  let spark = smoothstep(0.35, 0.6, glint);
  color += params.sun * (spark * 0.8 + pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 24.0) * 0.12);

  let shore = ${FOAM_DEPTH.toFixed(2)} + 0.08 * sin(time * 1.3 + world.x * 0.6 + world.y * 0.45);
  let foam = 1.0 - smoothstep(shore * 0.55, shore, depth);
  let lace = smoothstep(0.1, 0.55, bands(world * 2.1, time * 1.6) * 0.5 + 0.5);
  color = mix(color, params.foam, foam * mix(0.55, 1.0, lace));

  var alpha = smoothstep(0.0, 0.1, depth) * mix(0.62, 0.9, deep);
  alpha = clamp(alpha - clear * 0.12 + fresnel * 0.35 + spark * 0.4, 0.0, 1.0);
  return vec4f(color, alpha);
}
`


/** What the water samples until the renderer's mirror is handed over: transparent, so it shows sky. */
const blank = new Texture({ data: new Uint8Array(4), width: 1, height: 1 }, { srgb: false, wrap: 'clamp', mipmaps: false })

/** One shared material, so every tile draws with the same pipeline and a sky change reaches them all. */
let shared: ShaderMaterial | null = null
function material(): ShaderMaterial {
  shared ??= new ShaderMaterial({
    wgsl: WGSL,
    uniforms: {
      shallow: Color.hex(0x7fd8f5),
      deep: Color.hex(0x2f8fd8),
      foam: Color.hex(0xf4fcff),
      bed: Color.hex(0xd9cf9a),
      sky: Color.hex(0xffffff),
      sun: Color.hex(0xffffff),
      // Height of the mirrored plane, and 1 while there is one.
      plane: [0, 0],
    },
    textures: { reflection: blank },
    blend: 'alpha',
    // Written, so the ink (which reads depth) outlines the surface and not the bed seen through it.
    depthWrite: true,
    // Seen from below too: a swimmer (or anyone on the bed) looks up at it.
    side: 'double',
    // Streams float centimetres over the terrain; a bias keeps their far end from sinking into it.
    depthBias: 12,
    depthBiasSlope: 2,
  })
  return shared
}

/** Has the water sample `texture`, the renderer's mirror image of the scene. */
export function setWaterReflection(texture: Texture) {
  material().textures.reflection = texture
  material().needsUpdate()
}

/** The height the mirror was drawn at this frame, or null for none (the water then reflects the sky alone). */
export function setWaterPlane(height: number | null) {
  const plane = material().uniforms.plane as [number, number]
  const on = height === null ? 0 : 1
  if (plane[1] === on && (height === null || plane[0] === height)) return
  plane[0] = height ?? plane[0]
  plane[1] = on
  material().needsUpdate()
}

/** Points the reflection and glint at the scene's sky and sun colours (sRGB hex). */
export function setWaterLight(sky: number, sun: number) {
  const uniforms = material().uniforms
  ;(uniforms.sky as Color).setHex(sky)
  ;(uniforms.sun as Color).setHex(sun)
  material().needsUpdate()
}

/**
 * The lake and river surface of one tile. Every ground sample is a vertex; only the cells that
 * touch water are triangulated. Depth is `water - ground` at the vertices and interpolates across
 * a cell, so the shoreline (depth 0) falls between a wet and a dry sample, and islands stay dry
 * because their ground is above the level. Null when the tile has no water.
 */
function buildSurface(water: Float32Array, ground: TileGround): Mesh | null {
  const { x0, z0, cell, side, heights } = ground
  if (water.length !== side * side) return null
  const positions = new Float32Array(side * side * 3)
  const depths = new Float32Array(side * side)
  const flows = new Float32Array(side * side * 3)
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const at = row * side + column
      const level = water[at]!
      // A sample with no water is only ever a corner of a cell with some; it takes a neighbour's level below.
      const dry = level === NO_WATER
      positions.set([x0 + column * cell, dry ? heights[at]! : level, z0 + row * cell], at * 3)
      depths[at] = dry ? -1 : level - heights[at]!
    }
  }
  const indices: number[] = []
  const wet = (at: number) => depths[at]! > 0
  for (let row = 0; row < side - 1; row++) {
    for (let column = 0; column < side - 1; column++) {
      const p00 = row * side + column
      const p01 = p00 + 1
      const p10 = p00 + side
      const p11 = p10 + 1
      if (!(wet(p00) || wet(p01) || wet(p10) || wet(p11))) continue
      // Dry corners of a wet cell sit at the cell's water level so the surface is flat across it.
      let level = -Infinity
      for (const p of [p00, p01, p10, p11]) if (water[p] !== NO_WATER) level = Math.max(level, water[p]!)
      for (const p of [p00, p01, p10, p11]) positions[p * 3 + 1] = water[p] === NO_WATER ? level : water[p]!
      indices.push(p00, p10, p01, p10, p11, p01)
    }
  }
  if (indices.length === 0) return null
  const geometry = new Geometry({ positions, custom: surfaceCustom(depths, flows), indices: Uint32Array.from(indices) })
  const mesh = new Mesh(geometry, material())
  mesh.renderOrder = 1
  return mesh
}

/** The vertex attribute the shader reads: depth in x and the stream flow (along, across, is-stream) in yzw. */
function surfaceCustom(depths: Float32Array, flows: Float32Array): Float32Array {
  const out = new Float32Array(depths.length * 4)
  for (let i = 0; i < depths.length; i++) {
    out[i * 4] = depths[i]!
    out[i * 4 + 1] = flows[i * 3]!
    out[i * 4 + 2] = flows[i * 3 + 1]!
    out[i * 4 + 3] = flows[i * 3 + 2]!
  }
  return out
}

/**
 * Streams as ribbons on the ground: cross-sections every `STREAM_STEP` metres, `STREAM_ACROSS`
 * vertices wide, each lifted a hair over the terrain. Depth is deepest in the middle and zero at
 * the banks; `flow` carries the distance along the stream and the offset across it.
 */
function buildStreams(streams: readonly Stream[], height: Height): Mesh | null {
  const positions: number[] = []
  const depths: number[] = []
  const flows: number[] = []
  const indices: number[] = []
  for (const { width, points } of streams) {
    const half = width / 2
    let along = 0
    let previous: number[] | null = null
    for (let i = 0; i + 1 < points.length; i++) {
      const [ax, az] = points[i]!
      const [bx, bz] = points[i + 1]!
      const length = Math.hypot(bx - ax, bz - az)
      if (length < 1e-4) continue
      const nx = -(bz - az) / length
      const nz = (bx - ax) / length
      const parts = Math.max(1, Math.ceil(length / STREAM_STEP))
      // The first cross-section of a segment repeats the last of the one before; skip it there.
      for (let k = previous ? 1 : 0; k <= parts; k++) {
        const x = ax + ((bx - ax) * k) / parts
        const z = az + ((bz - az) * k) / parts
        const section: number[] = []
        for (let j = 0; j < STREAM_ACROSS; j++) {
          const across = (2 * j) / (STREAM_ACROSS - 1) - 1
          const px = x + nx * across * half
          const pz = z + nz * across * half
          section.push(positions.length / 3)
          positions.push(px, height(px, pz) + STREAM_LIFT, pz)
          depths.push(STREAM_DEPTH * (1 - across * across))
          flows.push(along + (length * k) / parts, across * half, 1)
        }
        if (previous) {
          for (let j = 0; j + 1 < STREAM_ACROSS; j++) {
            const [a, b, c, d] = [previous[j]!, previous[j + 1]!, section[j]!, section[j + 1]!]
            // Facing up whichever way the line runs.
            const up = (positions[b * 3 + 2]! - positions[a * 3 + 2]!) * (positions[c * 3]! - positions[a * 3]!) - (positions[b * 3]! - positions[a * 3]!) * (positions[c * 3 + 2]! - positions[a * 3 + 2]!)
            indices.push(...(up > 0 ? [a, b, c, b, d, c] : [a, c, b, b, c, d]))
          }
        }
        previous = section
      }
      along += length
    }
  }
  if (indices.length === 0) return null
  const geometry = new Geometry({
    positions: Float32Array.from(positions),
    custom: surfaceCustom(Float32Array.from(depths), Float32Array.from(flows)),
    indices: Uint32Array.from(indices),
  })
  const mesh = new Mesh(geometry, material())
  mesh.renderOrder = 1
  return mesh
}

/**
 * One tile's water, or null if it has none. It is translucent and added apart from the tile's
 * group so the game can place it in the world apart from the tile.
 */
export function buildWater(tile: Pick<TileData, 'water' | 'streams'>, ground: TileGround, around?: HeightSource): Group | null {
  const meshes = [tile.water.length > 0 ? buildSurface(tile.water, ground) : null, buildStreams(tile.streams, heightSampler(ground, around))].filter((m): m is Mesh => m !== null)
  if (meshes.length === 0) return null
  const group = new Group()
  group.add(...meshes)
  return group
}

/** Frees a tile's water geometry. The material is shared and stays. */
export function disposeWater(group: Group) {
  group.traverse((object) => {
    if (object instanceof Mesh) object.geometry.dispose()
  })
  group.removeFromParent()
}
