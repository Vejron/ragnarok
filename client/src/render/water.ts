import * as THREE from 'three'
import type { TileGround } from '@/sim/terrain'
import { NO_WATER, type Stream, type TileData } from '@/world'
import { heightSampler, type Height, type HeightSource } from './shade'

/**
 * Water: lakes and rivers are a surface mesh per tile at the importer's water level, streams are
 * ribbons draped on the ground. Both share one toy-style shader: a tint that deepens with depth,
 * soft light bands drifting over the surface, a foam line along the shore, and a flow along a
 * stream's length. Depth is a vertex attribute (`level - ground`), not a depth-texture read, so the
 * look needs no extra pass and stays out of the way of the AO pass.
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

const uniforms = {
  uTime: { value: 0 },
  uShallow: { value: new THREE.Color(0x7fd8f5) },
  uDeep: { value: new THREE.Color(0x2f8fd8) },
  uFoam: { value: new THREE.Color(0xf4fcff) },
  uBed: { value: new THREE.Color(0xd9cf9a) },
  uSky: { value: new THREE.Color(0xffffff) },
  uSun: { value: new THREE.Color(0xffffff) },
  uSunDir: { value: new THREE.Vector3(70, 80, 55).normalize() },
}

/** Points the reflection and glint at the scene's sky and sun. `direction` is towards the sun. */
export function setWaterLight(sky: THREE.ColorRepresentation, sun: THREE.ColorRepresentation, direction: THREE.Vector3) {
  uniforms.uSky.value.set(sky)
  uniforms.uSun.value.set(sun)
  uniforms.uSunDir.value.copy(direction).normalize()
}

/** Advances the ripples and flow. `seconds` is any steadily increasing clock. */
export function updateWater(seconds: number) {
  uniforms.uTime.value = seconds
}

const vertexShader = /* glsl */ `
  attribute float depth;
  attribute vec3 flow;
  varying float vDepth;
  varying vec3 vFlow;
  varying vec2 vWorld;
  varying float vSurfaceY;
  #include <fog_pars_vertex>
  void main() {
    vDepth = depth;
    vFlow = flow;
    vec4 world = modelMatrix * vec4(position, 1.0);
    vWorld = world.xz;
    vSurfaceY = world.y;
    vec4 mvPosition = viewMatrix * world;
    gl_Position = projectionMatrix * mvPosition;
    #include <fog_vertex>
  }
`

const fragmentShader = /* glsl */ `
  uniform float uTime;
  uniform vec3 uShallow;
  uniform vec3 uDeep;
  uniform vec3 uFoam;
  uniform vec3 uBed;
  uniform vec3 uSky;
  uniform vec3 uSun;
  uniform vec3 uSunDir;
  varying float vDepth;
  varying vec3 vFlow;
  varying vec2 vWorld;
  varying float vSurfaceY;
  #include <fog_pars_fragment>

  // Two crossing wave trains, stepped into three soft bands so the surface reads like toy plastic.
  float bands(vec2 p, float t) {
    float a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
    float b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
    float c = sin((p.x + p.y) * 0.25 + t * 0.4);
    return (a + b + 0.6 * c) / 2.6;
  }

  // Surface height from three crossing wave trains; its gradient is the ripple normal.
  float swell(vec2 p, float t) {
    return 0.5 * sin(p.x * 0.9 + p.y * 0.35 + t * 0.9)
         + 0.35 * sin(p.x * -0.4 + p.y * 1.1 - t * 0.7)
         + 0.15 * sin((p.x + p.y) * 1.7 + t * 1.3);
  }

  // Bright web of lines, the light a rippled surface focuses on the bed.
  float caustics(vec2 p, float t) {
    float a = abs(sin(p.x * 1.3 + sin(p.y * 0.9 + t * 0.8) * 1.5));
    float b = abs(sin(p.y * 1.1 + sin(p.x * 1.2 - t * 0.6) * 1.5));
    return pow(1.0 - a * b, 6.0);
  }

  void main() {
    // Streams scroll along their own length; lakes drift over the world.
    bool stream = vFlow.z > 0.5;
    vec2 p = stream ? vec2(vFlow.x - uTime * ${FLOW_SPEED.toFixed(2)}, vFlow.y * 3.0) * 2.2 : vWorld * 0.7;
    float wave = bands(p, stream ? 0.0 : uTime);
    float band = smoothstep(0.35, 0.5, wave) * 0.5 + smoothstep(0.6, 0.7, wave) * 0.5;

    float deep = smoothstep(0.0, 2.2, vDepth);
    vec3 color = mix(uShallow, uDeep, deep);

    // Where it is shallow the bed shows through, lit by a moving web of caustics.
    float clear = 1.0 - smoothstep(0.0, 1.4, vDepth);
    vec2 bedP = stream ? p * 0.8 : vWorld * 0.9;
    float web = caustics(bedP, stream ? uTime : uTime * 0.8);
    color = mix(color, uBed, clear * 0.45);
    color += uFoam * web * clear * 0.35;
    color = mix(color, uFoam, band * 0.12);

    // Ripple normal from the swell's gradient, flipped for a viewer under the surface.
    const float e = 0.15;
    vec2 q = stream ? p : vWorld * 0.8;
    float t = stream ? 0.0 : uTime;
    float h = swell(q, t);
    vec2 grad = vec2(swell(q + vec2(e, 0.0), t) - h, swell(q + vec2(0.0, e), t) - h) / e;
    vec3 n = normalize(vec3(-grad.x * 0.12, 1.0, -grad.y * 0.12));
    if (!gl_FrontFacing) n.y = -n.y;
    vec3 view = normalize(vec3(cameraPosition.x - vWorld.x, cameraPosition.y - vSurfaceY, cameraPosition.z - vWorld.y));

    // Sky creeps in at grazing angles; lit ripples step into soft bands.
    float fresnel = pow(1.0 - clamp(abs(dot(n, view)), 0.0, 1.0), 3.0);
    float lit = floor(clamp(dot(n, uSunDir) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
    color *= 0.9 + 0.2 * lit;
    color = mix(color, uSky, fresnel * 0.55);

    // A hard sparkle where a ripple mirrors the sun.
    float glint = pow(max(dot(reflect(-uSunDir, n), view), 0.0), 90.0);
    float spark = smoothstep(0.35, 0.6, glint);
    color += uSun * spark * 0.8;

    // A foam line at the shore that breathes in and out.
    float shore = ${FOAM_DEPTH.toFixed(2)} + 0.08 * sin(uTime * 1.3 + vWorld.x * 0.6 + vWorld.y * 0.45);
    float foam = 1.0 - smoothstep(shore * 0.55, shore, vDepth);
    float lace = smoothstep(0.1, 0.55, bands(vWorld * 2.1, uTime * 1.6) * 0.5 + 0.5);
    color = mix(color, uFoam, foam * mix(0.55, 1.0, lace));

    // Clear at the very edge, so the shoreline is a soft line and not a polygon edge.
    float alpha = smoothstep(0.0, 0.1, vDepth) * mix(0.62, 0.9, deep);
    alpha = clamp(alpha - clear * 0.12 + fresnel * 0.15 + spark * 0.4, 0.0, 1.0);
    gl_FragColor = vec4(color, alpha);
    #include <tonemapping_fragment>
    #include <colorspace_fragment>
    #include <fog_fragment>
  }
`

let shared: THREE.ShaderMaterial | null = null
function material(): THREE.ShaderMaterial {
  shared ??= new THREE.ShaderMaterial({
    uniforms: THREE.UniformsUtils.merge([THREE.UniformsLib.fog, uniforms]),
    vertexShader,
    fragmentShader,
    transparent: true,
    depthWrite: false,
    // Seen from below too: a swimmer (or anyone on the bed) looks up at it.
    side: THREE.DoubleSide,
    fog: true,
    // Streams float centimetres over the terrain; a bias keeps their far end from sinking into it.
    polygonOffset: true,
    polygonOffsetFactor: -2,
    polygonOffsetUnits: -12,
  })
  // `merge` copies the values, so the shared ones are re-pointed to keep every tile in step.
  for (const name of Object.keys(uniforms) as (keyof typeof uniforms)[]) shared.uniforms[name] = uniforms[name]
  return shared
}

/**
 * The lake and river surface of one tile. Every ground sample is a vertex; only the cells that
 * touch water are triangulated. Depth is `water - ground` at the vertices and interpolates across
 * a cell, so the shoreline (depth 0) falls between a wet and a dry sample, and islands stay dry
 * because their ground is above the level. Null when the tile has no water.
 */
function buildSurface(water: Float32Array, ground: TileGround): THREE.Mesh | null {
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
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('depth', new THREE.BufferAttribute(depths, 1))
  geometry.setAttribute('flow', new THREE.BufferAttribute(flows, 3))
  geometry.setIndex(indices)
  geometry.computeBoundingSphere()
  const mesh = new THREE.Mesh(geometry, material())
  mesh.renderOrder = 1
  return mesh
}

/**
 * Streams as ribbons on the ground: cross-sections every `STREAM_STEP` metres, `STREAM_ACROSS`
 * vertices wide, each lifted a hair over the terrain. Depth is deepest in the middle and zero at
 * the banks; `flow` carries the distance along the stream and the offset across it.
 */
function buildStreams(streams: readonly Stream[], height: Height): THREE.Mesh | null {
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
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('depth', new THREE.Float32BufferAttribute(depths, 1))
  geometry.setAttribute('flow', new THREE.Float32BufferAttribute(flows, 3))
  geometry.setIndex(indices)
  geometry.computeBoundingSphere()
  const mesh = new THREE.Mesh(geometry, material())
  mesh.renderOrder = 1
  return mesh
}

/**
 * One tile's water, or null if it has none. It is translucent and added apart from the tile's
 * group so the game can keep it out of the AO pass (`Scene.excludeFromAO`) in one place.
 */
export function buildWater(tile: Pick<TileData, 'water' | 'streams'>, ground: TileGround, around?: HeightSource): THREE.Group | null {
  const meshes = [tile.water.length > 0 ? buildSurface(tile.water, ground) : null, buildStreams(tile.streams, heightSampler(ground, around))].filter((m): m is THREE.Mesh => m !== null)
  if (meshes.length === 0) return null
  const group = new THREE.Group()
  group.add(...meshes)
  return group
}

/** Frees a tile's water geometry. The material is shared and stays. */
export function disposeWater(group: THREE.Group) {
  group.traverse((object) => {
    if (object instanceof THREE.Mesh) object.geometry.dispose()
  })
  group.removeFromParent()
}
