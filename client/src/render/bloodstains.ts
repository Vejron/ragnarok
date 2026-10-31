import { Color, Geometry, Mesh, ShaderMaterial, type Vector3 } from '@ragnarok/engine'
import { GROUND_LIGHT_WGSL } from './groundlight'

/** Seconds a stain lies on the ground before it has faded away. */
export const STAIN_LIFE = 120
/** Height of a stain above the ground it is laid on, metres. */
const LIFT = 0.025
/** Shortest piece of a smear, metres. */
const MIN_STEP = 0.25
/** A smear that jumps further than this starts a new strip. */
const MAX_STEP = 3
/** Fresh blood; the shader dries it to brown-black. */
const FRESH = 0x6a0404
const KIND_SMEAR = 0
const KIND_BLOT = 1

/** Value noise over the ground, for what is drawn on it. */
export const STAIN_NOISE_WGSL = /* wgsl */ `
fn stainHash(p: vec2f) -> f32 {
  var q = fract(vec3f(p.x, p.y, p.x) * 0.1031);
  q += dot(q, q.yzx + 33.33);
  return fract((q.x + q.y) * q.z);
}

fn stainNoise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(stainHash(i), stainHash(i + vec2f(1.0, 0.0)), u.x), mix(stainHash(i + vec2f(0.0, 1.0)), stainHash(i + vec2f(1.0, 1.0)), u.x), u.y);
}
`

// Per vertex: uv across the stain (-1 to 1; along a smear `v` is metres travelled), and in `custom`
// its birth time, a seed for its ragged edge, its kind, and how long it takes to spread, seconds.
// A stain draws only its shape: solid inside with a soft edge and no rim, and its colour, sheen and
// drying come from noise over the world, not the stain. So stains that overlap read as one puddle that
// runs together rather than as a pile of outlined discs. It dries over the first half of its life and
// fades over the last third.
const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

${STAIN_NOISE_WGSL}

fn surface(s: Surface) -> vec4f {
  let seconds = params.now - s.custom.x;
  let age = clamp(seconds / params.life, 0.0, 1.0);
  let seed = s.custom.y;
  var cover = 0.0;
  if (s.custom.z < 0.5) {
    // A smear: ragged along both edges, thinner in streaks where it was dragged.
    let edge = 0.6 + 0.4 * stainNoise(vec2f(s.uv.y * 2.5 + seed * 13.0, seed * 7.0));
    cover = 1.0 - smoothstep(edge - 0.35, edge, abs(s.uv.x));
    cover *= 0.6 + 0.4 * stainNoise(vec2f(s.uv.x * 4.0 + seed * 5.0, s.uv.y * 0.5));
  } else {
    // A blot: a lumpy puddle that spreads to its full size, and a few specks thrown out round it.
    let grow = mix(0.3, 1.0, sqrt(clamp(seconds / max(s.custom.w, 0.001), 0.0, 1.0)));
    let angle = atan2(s.uv.y, s.uv.x);
    let r = length(s.uv);
    let wobble = 0.1 * sin(angle * 3.0 + seed * 6.3) + 0.06 * sin(angle * 7.0 + seed * 2.1) + 0.14 * (stainNoise(vec2f(angle * 2.0 + 20.0, seed * 9.0)) - 0.5);
    let edge = (0.66 + wobble) * grow;
    cover = 1.0 - smoothstep(edge * 0.55, edge, r);
    let cell = s.uv * 4.0 + vec2f(seed * 17.0);
    let speck = step(0.85, stainHash(floor(cell))) * (1.0 - smoothstep(0.05, 0.16, length(fract(cell) - 0.5)));
    cover = max(cover, speck * step(edge, r) * step(r, grow) * 0.8);
  }
  // Colour and drying vary over the ground, so neighbouring stains share them.
  let ground = s.world.xz;
  let n = stainNoise(ground * 3.0);
  let fine = stainNoise(ground * 11.0);
  let dry = clamp(smoothstep(0.0, 0.5, age) + (n - 0.5) * 0.3, 0.0, 1.0);
  let dried = vec3f(0.11, 0.025, 0.015);
  var color = mix(params.fresh, dried, dry) * (0.85 + 0.3 * fine);
  let lit = groundLight(s.world, vec3f(0.0, 1.0, 0.0), 1.0);
  var rgb = color / PI * lit;
  // A wet sheen toward the sun, broken up a little, gone as it dries.
  let toEye = normalize(frame.cameraPos.xyz - s.world);
  let halfway = normalize(toEye + frame.sunDir.xyz);
  let gloss = pow(max(halfway.y, 0.0), 80.0) * (1.0 - dry) * (0.5 + 0.5 * n);
  rgb += frame.sunColor.rgb * gloss * 0.3;
  let fade = 1.0 - smoothstep(0.66, 1.0, age);
  return vec4f(rgb, cover * fade * s.color.a);
}
`

type Point = readonly [number, number, number]

interface Smear {
  /** Where its last piece ended, either side; none before its first piece. */
  left: Point | null
  right: Point | null
  centre: Vector3
  along: number
  seed: number
}

/**
 * Blood on the ground: round blots where drops and pools land, and smeared strips where a body is
 * dragged, in a ring buffer of `capacity` quads like `Skidmarks`. The shape, spreading, drying and
 * fading are all in the shader, so nothing is touched per frame and a quad is reused once the ring
 * comes round again.
 */
export class Bloodstains {
  readonly root: Mesh
  private readonly geometry: Geometry
  private readonly material: ShaderMaterial
  private readonly positions: Float32Array
  private readonly uvs: Float32Array
  private readonly tints: Float32Array
  private readonly custom: Float32Array
  private readonly smears = new Map<string, Smear>()
  private next = 0
  private written = 0
  private time = 0

  constructor(readonly capacity = 2000) {
    this.positions = new Float32Array(capacity * 4 * 3)
    this.uvs = new Float32Array(capacity * 4 * 2)
    this.tints = new Float32Array(capacity * 4 * 4)
    this.custom = new Float32Array(capacity * 4 * 4)
    // Born long ago, so unused quads are fully faded.
    for (let v = 0; v < capacity * 4; v++) this.custom[v * 4] = -1e6
    const index = new Uint32Array(capacity * 6)
    for (let quad = 0; quad < capacity; quad++) {
      const v = quad * 4
      index.set([v, v + 1, v + 2, v + 2, v + 1, v + 3], quad * 6)
    }
    this.geometry = new Geometry({ positions: this.positions, uvs: this.uvs, colors: this.tints, custom: this.custom, indices: index })
    this.material = new ShaderMaterial({
      wgsl: WGSL,
      uniforms: { now: 0, life: STAIN_LIFE, fresh: new Color().setHex(FRESH) },
      vertexColors: true,
      blend: 'alpha',
      depthWrite: false,
      depthBias: 4,
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

  /**
   * A round blot `radius` metres across centred at `x, y, z` (`y` the ground), reaching its full
   * size over `spread` seconds: a drop with none, a pool with a few.
   */
  blot(x: number, y: number, z: number, radius: number, spread = 0, alpha = 1) {
    const seed = Math.random()
    // Turned at random, so the same lumps are never lined up.
    const turn = seed * Math.PI * 2
    const c = Math.cos(turn) * radius
    const s = Math.sin(turn) * radius
    const corners = [
      [-1, -1],
      [1, -1],
      [-1, 1],
      [1, 1],
    ] as const
    const quad = this.take()
    corners.forEach(([u, v], k) => {
      const i = quad * 4 + k
      this.positions.set([x + u * c - v * s, y + LIFT, z + u * s + v * c], i * 3)
      this.uvs.set([u, v], i * 2)
      this.tints.set([1, 1, 1, alpha], i * 4)
      this.custom.set([this.time, seed, KIND_BLOT, spread], i * 4)
    })
    this.geometry.needsUpdateRange(quad * 4, 4)
  }

  /**
   * Smear `track` is being dragged over the ground at `centre` (on the ground), `width` metres across.
   * Joins on to the smear's last piece if it has one; it runs the way it was dragged.
   */
  smear(track: string, centre: Vector3, width: number, alpha = 1) {
    const last = this.smears.get(track)
    if (!last) {
      this.smears.set(track, { left: null, right: null, centre: centre.clone(), along: 0, seed: Math.random() })
      return
    }
    const dx = centre.x - last.centre.x
    const dz = centre.z - last.centre.z
    const step = Math.hypot(dx, dz)
    if (step < MIN_STEP) return
    if (step > MAX_STEP) {
      this.smears.delete(track)
      return
    }
    // Across the way it moved.
    const sx = (-dz / step) * (width / 2)
    const sz = (dx / step) * (width / 2)
    const end = (c: Vector3, sign: number) => [c.x + sx * sign, c.y + LIFT, c.z + sz * sign] as const
    const from = [last.left ?? end(last.centre, -1), last.right ?? end(last.centre, 1)]
    const to = [end(centre, -1), end(centre, 1)]
    const along = last.along + step
    const quad = this.take()
    const corners = [from[0]!, from[1]!, to[0]!, to[1]!]
    const vs = [last.along, last.along, along, along]
    corners.forEach((p, k) => {
      const i = quad * 4 + k
      this.positions.set(p, i * 3)
      this.uvs.set([k % 2 ? 1 : -1, vs[k]!], i * 2)
      this.tints.set([1, 1, 1, alpha], i * 4)
      this.custom.set([this.time, last.seed, KIND_SMEAR, 0], i * 4)
    })
    this.geometry.needsUpdateRange(quad * 4, 4)
    this.smears.set(track, { left: to[0]!, right: to[1]!, centre: centre.clone(), along, seed: last.seed })
  }

  /** Smear `track` stopped; its next piece starts a new strip. */
  lift(track: string) {
    this.smears.delete(track)
  }

  private take() {
    const quad = this.next
    this.next = (this.next + 1) % this.capacity
    this.written++
    return quad
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
