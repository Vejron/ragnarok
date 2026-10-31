import { Geometry, Mesh, ShaderMaterial } from '@ragnarok/engine'
import { STAIN_NOISE_WGSL } from './bloodstains'
import { GROUND_LIGHT_WGSL } from './groundlight'

/** Seconds a burn lies on the ground before it has faded away. */
export const SCORCH_LIFE = 240
/** Height of a burn above the ground it is laid on, metres: over the roads' ribbons too, which it burns as well. */
const LIFT = 0.1
/** Metres between the burns of a sweep, for a radius of 1: they overlap into one scar. */
const STEP = 0.4
/** A hit further than this from the last one starts a new scar, metres. */
const MAX_SWEEP = 30

// Per vertex: uv across the burn (-1 to 1), and in `custom` its birth time. Like a stain it draws only
// its shape, and everything else comes from noise over the world, not the burn: overlapping burns read as
// one scar. Soot black at the heart, baked brown towards its edge with a little pale ash. It starts
// red-hot and is left with veins that glow orange (HDR, so they bloom) and go out over a few seconds.
const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}
${STAIN_NOISE_WGSL}

fn surface(s: Surface) -> vec4f {
  let seconds = params.now - s.custom.x;
  let ground = s.world.xz;
  let n = stainNoise(ground * 1.7) * 0.6 + stainNoise(ground * 5.0) * 0.3 + stainNoise(ground * 13.0) * 0.1;
  let r = length(s.uv) + (n - 0.5) * 0.9;
  let cover = 1.0 - smoothstep(0.5, 1.0, r);
  let heart = 1.0 - smoothstep(0.15, 0.7, r);
  let ash = smoothstep(0.7, 0.8, stainNoise(ground * 21.0)) * 0.08;
  let color = mix(vec3f(0.13, 0.085, 0.05), vec3f(0.012, 0.011, 0.01), heart) + ash;
  var rgb = color / PI * groundLight(s.world, vec3f(0.0, 1.0, 0.0), 1.0);
  // Thin veins where broad noise crosses its middle, so they stay lines from a distance.
  let vein = 1.0 - smoothstep(0.0, 0.07, abs(stainNoise(ground * 2.3) - 0.5));
  rgb += vec3f(5.0, 0.8, 0.08) * exp(-seconds / 2.5) * vein * heart + vec3f(3.0, 0.4, 0.03) * exp(-seconds / 0.8) * heart;
  let fade = 1.0 - smoothstep(0.75, 1.0, seconds / params.life);
  return vec4f(rgb, cover * fade);
}
`

const CORNERS = [
  [-1, -1],
  [1, -1],
  [-1, 1],
  [1, 1],
] as const

type Point = { x: number; y: number; z: number }

/**
 * Black burns on the ground where a laser has been, in a ring buffer of `capacity` quads like
 * `Bloodstains`. Each quad is laid on the ground's slope. The look, the cooling and the fading are all in
 * the shader, so nothing is touched per frame and a quad is reused once the ring comes round again.
 */
export class Scorchmarks {
  readonly root: Mesh
  private readonly geometry: Geometry
  private readonly material: ShaderMaterial
  private readonly positions: Float32Array
  private readonly custom: Float32Array
  private next = 0
  private written = 0
  private time = 0

  constructor(private readonly groundAt: (x: number, z: number) => number, readonly capacity = 3000) {
    this.positions = new Float32Array(capacity * 4 * 3)
    this.custom = new Float32Array(capacity * 4 * 4)
    const uvs = new Float32Array(capacity * 4 * 2)
    const tints = new Float32Array(capacity * 4 * 4).fill(1)
    // Born long ago, so unused quads are fully faded.
    for (let v = 0; v < capacity * 4; v++) this.custom[v * 4] = -1e6
    const index = new Uint32Array(capacity * 6)
    for (let quad = 0; quad < capacity; quad++) {
      const v = quad * 4
      index.set([v, v + 1, v + 2, v + 2, v + 1, v + 3], quad * 6)
      CORNERS.forEach((corner, k) => uvs.set(corner, (v + k) * 2))
    }
    this.geometry = new Geometry({ positions: this.positions, uvs, colors: tints, custom: this.custom, indices: index })
    this.material = new ShaderMaterial({
      wgsl: WGSL,
      uniforms: { now: 0, life: SCORCH_LIFE },
      vertexColors: true,
      blend: 'alpha',
      depthWrite: false,
      // Past every ribbon's (`roads.ts`).
      depthBias: 40,
      depthBiasSlope: 2,
      side: 'double',
    })
    this.root = new Mesh(this.geometry, this.material)
    this.root.frustumCulled = false
    this.root.renderOrder = 1
  }

  /** Quads laid so far, for tests. */
  get laid() {
    return this.written
  }

  /** One burn `radius` metres across centred at `x, z`, `lift` metres off the ground (a road's height over it). */
  burn(x: number, z: number, lift: number, radius: number) {
    const turn = Math.random() * Math.PI * 2
    const c = Math.cos(turn) * radius
    const s = Math.sin(turn) * radius
    const quad = this.next
    this.next = (this.next + 1) % this.capacity
    this.written++
    CORNERS.forEach(([u, v], k) => {
      const i = quad * 4 + k
      const cx = x + u * c - v * s
      const cz = z + u * s + v * c
      this.positions.set([cx, this.groundAt(cx, cz) + lift + LIFT, cz], i * 3)
      this.custom[i * 4] = this.time
    })
    this.geometry.needsUpdateRange(quad * 4, 4)
  }

  /** A beam of `radius` metres striking the ground at `to`, having just struck it at `from` (or nowhere): burns the whole way between. */
  sweep(from: Point | null, to: Point, radius: number) {
    const lift = Math.max(0, to.y - this.groundAt(to.x, to.z))
    const dx = from ? to.x - from.x : 0
    const dz = from ? to.z - from.z : 0
    const distance = Math.hypot(dx, dz)
    const steps = distance > MAX_SWEEP ? 0 : Math.floor(distance / (STEP * radius))
    for (let i = steps; i > 0; i--) this.burn(to.x - (dx * i) / (steps + 1), to.z - (dz * i) / (steps + 1), lift, radius * (0.7 + Math.random() * 0.4))
    this.burn(to.x, to.z, lift, radius * 1.2)
  }

  update(dt: number) {
    this.time += dt
    this.material.uniforms.now = this.time
    this.material.needsUpdate()
  }

  dispose() {
    this.geometry.dispose()
    this.material.dispose()
    this.root.removeFromParent()
  }
}
