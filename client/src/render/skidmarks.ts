import * as THREE from 'three'

/** Seconds a mark takes to fade away. */
export const SKID_LIFE = 15
/** Shortest piece of a mark, metres: a wheel lays a new quad once it has moved this far. */
const MIN_STEP = 0.35
/** A wheel that jumps further than this between marks starts a new strip (a respawn, a teleport). */
const MAX_STEP = 4
/** Height of the mark above the ground it is laid on, metres. */
const LIFT = 0.03

const VERTEX = /* glsl */ `
attribute vec4 tint;
attribute float birth;
uniform float now;
uniform float life;
varying vec4 vTint;
void main() {
  float age = clamp((now - birth) / life, 0.0, 1.0);
  vTint = vec4(tint.rgb, tint.a * (1.0 - age));
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`

const FRAGMENT = /* glsl */ `
varying vec4 vTint;
void main() {
  if (vTint.a < 0.005) discard;
  gl_FragColor = vTint;
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
  readonly root: THREE.Mesh
  private readonly geometry = new THREE.BufferGeometry()
  private readonly material: THREE.ShaderMaterial
  private readonly positions: THREE.BufferAttribute
  private readonly tints: THREE.BufferAttribute
  private readonly births: THREE.BufferAttribute
  private readonly tracks = new Map<string, Track>()
  private readonly color = new THREE.Color()
  private next = 0
  private written = 0
  private time = 0

  constructor(readonly capacity = 4000) {
    this.positions = new THREE.BufferAttribute(new Float32Array(capacity * 4 * 3), 3).setUsage(THREE.DynamicDrawUsage)
    this.tints = new THREE.BufferAttribute(new Float32Array(capacity * 4 * 4), 4).setUsage(THREE.DynamicDrawUsage)
    // Born long ago, so unused quads are fully faded.
    this.births = new THREE.BufferAttribute(new Float32Array(capacity * 4).fill(-1e6), 1).setUsage(THREE.DynamicDrawUsage)
    const index = new Uint32Array(capacity * 6)
    for (let quad = 0; quad < capacity; quad++) {
      const v = quad * 4
      index.set([v, v + 1, v + 2, v + 2, v + 1, v + 3], quad * 6)
    }
    this.geometry.setIndex(new THREE.BufferAttribute(index, 1))
    this.geometry.setAttribute('position', this.positions)
    this.geometry.setAttribute('tint', this.tints)
    this.geometry.setAttribute('birth', this.births)
    this.material = new THREE.ShaderMaterial({
      vertexShader: VERTEX,
      fragmentShader: FRAGMENT,
      uniforms: { now: { value: 0 }, life: { value: SKID_LIFE } },
      transparent: true,
      depthWrite: false,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -4,
      side: THREE.DoubleSide,
    })
    this.root = new THREE.Mesh(this.geometry, this.material)
    this.root.frustumCulled = false
    this.root.renderOrder = 1
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
    for (let k = 0; k < 4; k++) {
      const v = quad * 4 + k
      this.positions.setXYZ(v, corners[k]!.x, corners[k]!.y, corners[k]!.z)
      this.tints.setXYZW(v, this.color.r, this.color.g, this.color.b, alpha)
      this.births.setX(v, this.time)
    }
    for (const attribute of [this.positions, this.tints, this.births]) {
      attribute.addUpdateRange(quad * 4 * attribute.itemSize, 4 * attribute.itemSize)
      attribute.needsUpdate = true
    }
  }

  /** How faded quad `index` is now, 0 fresh to 1 gone, for tests. */
  ageOf(index: number) {
    return Math.min(1, Math.max(0, (this.time - this.births.getX(index * 4)) / SKID_LIFE))
  }

  update(dt: number) {
    this.time += dt
    this.material.uniforms.now!.value = this.time
  }

  dispose() {
    this.geometry.dispose()
    this.material.dispose()
    this.root.removeFromParent()
  }
}
