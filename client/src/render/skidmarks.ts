import { Color, Geometry, Mesh, ShaderMaterial } from '@ragnarok/engine'
import type * as THREE from 'three'
import { GROUND_LIGHT_WGSL } from './groundlight'

/** Seconds a mark takes to fade away. */
export const SKID_LIFE = 15
/** Shortest piece of a mark, metres: a wheel lays a new quad once it has moved this far. */
const MIN_STEP = 0.35
/** A wheel that jumps further than this between marks starts a new strip (a respawn, a teleport). */
const MAX_STEP = 4
/** Height of the mark above the ground it is laid on, metres. */
const LIFT = 0.03

// Each vertex carries its tint (colour and alpha) as a vertex colour and its birth time in `custom.x`.
// Lit as the flat ground under it is, so a mark darkens with the road at night and shows in headlights.
const WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn surface(s: Surface) -> vec4f {
  let age = clamp((params.now - s.custom.x) / params.life, 0.0, 1.0);
  let lit = groundLight(s.world, vec3f(0.0, 1.0, 0.0), 1.0);
  return vec4f(s.color.rgb / PI * lit, s.color.a * (1.0 - age));
}
`

interface Track {
  left: THREE.Vector3
  right: THREE.Vector3
  centre: THREE.Vector3
}

/**
 * Tyre marks on the ground: strips of quads, one strip per wheel, in a ring buffer of `capacity`
 * quads. Each mark fades out over `SKID_LIFE` seconds (in the shader, so nothing is touched per
 * frame) and its quad is reused once the ring comes round again.
 */
export class Skidmarks {
  readonly root: Mesh
  private readonly geometry: Geometry
  private readonly material: ShaderMaterial
  private readonly positions: Float32Array
  private readonly tints: Float32Array
  private readonly births: Float32Array
  private readonly tracks = new Map<string, Track>()
  private readonly color = new Color()
  private next = 0
  private written = 0
  private time = 0

  constructor(readonly capacity = 4000) {
    this.positions = new Float32Array(capacity * 4 * 3)
    this.tints = new Float32Array(capacity * 4 * 4)
    // Born long ago, so unused quads are fully faded.
    this.births = new Float32Array(capacity * 4).fill(-1e6)
    const index = new Uint32Array(capacity * 6)
    for (let quad = 0; quad < capacity; quad++) {
      const v = quad * 4
      index.set([v, v + 1, v + 2, v + 2, v + 1, v + 3], quad * 6)
    }
    // The geometry's custom attribute is four floats per vertex; the birth time is its x.
    this.geometry = new Geometry({ positions: this.positions, colors: this.tints, custom: new Float32Array(capacity * 4 * 4), indices: index })
    this.material = new ShaderMaterial({
      wgsl: WGSL,
      uniforms: { now: 0, life: SKID_LIFE },
      vertexColors: true,
      blend: 'alpha',
      depthWrite: false,
      // Lifted off the ground it is laid on, in depth as well as in height.
      depthBias: 4,
      depthBiasSlope: 2,
      side: 'double',
    })
    this.root = new Mesh(this.geometry, this.material)
    this.root.frustumCulled = false
    this.root.renderOrder = 1
    for (let v = 0; v < capacity * 4; v++) this.geometry.custom![v * 4] = this.births[v]!
  }

  /** Quads laid so far, for tests; at most `capacity` are visible at once. */
  get laid() {
    return this.written
  }

  /**
   * Wheel `track` is marking the ground at `centre`, `width` metres across along `side` (a unit
   * vector), with `color` at `alpha`. Joins on to the wheel's last mark if it has one.
   */
  mark(track: string, centre: THREE.Vector3, side: THREE.Vector3, width: number, color: number, alpha: number) {
    const left = centre.clone().addScaledVector(side, -width / 2)
    const right = centre.clone().addScaledVector(side, width / 2)
    left.y += LIFT
    right.y += LIFT
    const last = this.tracks.get(track)
    if (last) {
      const step = last.centre.distanceTo(centre)
      if (step < MIN_STEP) return
      if (step <= MAX_STEP) this.quad(last.left, last.right, left, right, color, alpha)
    }
    this.tracks.set(track, { left, right, centre: centre.clone() })
  }

  /** Wheel `track` stopped marking; its next mark starts a new strip. */
  lift(track: string) {
    this.tracks.delete(track)
  }

  private quad(a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3, d: THREE.Vector3, color: number, alpha: number) {
    const quad = this.next
    this.next = (this.next + 1) % this.capacity
    this.written++
    this.color.setHex(color)
    const corners = [a, b, c, d]
    const custom = this.geometry.custom!
    for (let k = 0; k < 4; k++) {
      const v = quad * 4 + k
      this.positions.set([corners[k]!.x, corners[k]!.y, corners[k]!.z], v * 3)
      this.tints.set([this.color.r, this.color.g, this.color.b, alpha], v * 4)
      this.births[v] = this.time
      custom[v * 4] = this.time
    }
    // Only this quad's four vertices go to the GPU.
    this.geometry.needsUpdateRange(quad * 4, 4)
  }

  /** How faded quad `index` is now, 0 fresh to 1 gone, for tests. */
  ageOf(index: number) {
    return Math.min(1, Math.max(0, (this.time - this.births[index * 4]!) / SKID_LIFE))
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
