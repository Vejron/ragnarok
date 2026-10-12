import * as THREE from 'three'

/** How one particle moves and looks over its life. */
export interface ParticleSpec {
  position: THREE.Vector3 | readonly [number, number, number]
  velocity: readonly [number, number, number]
  /** Seconds. */
  life: number
  /** Diameter at birth and at death, metres. */
  size: [number, number]
  color: number
  /** Opacity at birth; it fades to nothing by the end. */
  alpha: number
  /** Metres per second² pulling it down; negative floats it up. */
  gravity?: number
  /** Share of its speed lost per second. */
  drag?: number
  /** Bounces off the ground (a stone) instead of passing through it. */
  bounce?: boolean
}

const VERTEX = /* glsl */ `
attribute vec3 offset;
attribute vec4 tint;
attribute float size;
varying vec4 vTint;
varying vec2 vUv;
void main() {
  vTint = tint;
  vUv = uv;
  // A billboard: the quad is laid out in view space around the particle's centre.
  vec4 centre = modelViewMatrix * vec4(offset, 1.0);
  centre.xy += position.xy * size;
  gl_Position = projectionMatrix * centre;
}
`

const FRAGMENT = /* glsl */ `
varying vec4 vTint;
varying vec2 vUv;
void main() {
  float edge = smoothstep(0.5, 0.15, length(vUv - 0.5));
  float alpha = vTint.a * edge;
  if (alpha < 0.01) discard;
  gl_FragColor = vec4(vTint.rgb, alpha);
}
`

/**
 * A pool of soft round billboards in world space: tyre smoke, dust, gravel, spray and sparks. One
 * draw call for all of them. Particles are simulated on the CPU (a few thousand at most); when the
 * pool is full a new one replaces an old one.
 */
export class Particles {
  readonly root: THREE.Mesh
  private readonly geometry = new THREE.InstancedBufferGeometry()
  private readonly material: THREE.ShaderMaterial
  private readonly offsets: THREE.InstancedBufferAttribute
  private readonly tints: THREE.InstancedBufferAttribute
  private readonly sizes: THREE.InstancedBufferAttribute
  private count = 0
  // Per particle: position (3), velocity (3), age, life, size from, size to, colour (3), alpha, gravity, drag, bounce.
  private static readonly STRIDE = 17
  private readonly state: Float32Array
  private readonly color = new THREE.Color()

  /** Ground height under a point, for particles that bounce. */
  groundAt: (x: number, z: number) => number = () => 0

  constructor(
    readonly capacity = 3000,
    additive = false,
  ) {
    const quad = new THREE.PlaneGeometry(1, 1)
    this.geometry.index = quad.index
    this.geometry.setAttribute('position', quad.getAttribute('position'))
    this.geometry.setAttribute('uv', quad.getAttribute('uv'))
    this.offsets = new THREE.InstancedBufferAttribute(new Float32Array(capacity * 3), 3).setUsage(THREE.DynamicDrawUsage)
    this.tints = new THREE.InstancedBufferAttribute(new Float32Array(capacity * 4), 4).setUsage(THREE.DynamicDrawUsage)
    this.sizes = new THREE.InstancedBufferAttribute(new Float32Array(capacity), 1).setUsage(THREE.DynamicDrawUsage)
    this.geometry.setAttribute('offset', this.offsets)
    this.geometry.setAttribute('tint', this.tints)
    this.geometry.setAttribute('size', this.sizes)
    this.geometry.instanceCount = 0
    this.material = new THREE.ShaderMaterial({
      vertexShader: VERTEX,
      fragmentShader: FRAGMENT,
      transparent: true,
      depthWrite: false,
      blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending,
    })
    this.root = new THREE.Mesh(this.geometry, this.material)
    // The particles are anywhere in the world, so the mesh's own bounds mean nothing.
    this.root.frustumCulled = false
    this.root.renderOrder = 2
    this.state = new Float32Array(capacity * Particles.STRIDE)
  }

  /** How many particles are alive. */
  get alive() {
    return this.count
  }

  spawn(spec: ParticleSpec) {
    if (this.count === this.capacity) this.remove(0)
    const s = this.state
    const at = this.count++ * Particles.STRIDE
    const p = spec.position
    const [x, y, z] = p instanceof THREE.Vector3 ? [p.x, p.y, p.z] : p
    this.color.setHex(spec.color)
    s[at] = x; s[at + 1] = y; s[at + 2] = z
    s[at + 3] = spec.velocity[0]; s[at + 4] = spec.velocity[1]; s[at + 5] = spec.velocity[2]
    s[at + 6] = 0; s[at + 7] = spec.life
    s[at + 8] = spec.size[0]; s[at + 9] = spec.size[1]
    s[at + 10] = this.color.r; s[at + 11] = this.color.g; s[at + 12] = this.color.b
    s[at + 13] = spec.alpha
    s[at + 14] = spec.gravity ?? 0
    s[at + 15] = spec.drag ?? 0
    s[at + 16] = spec.bounce ? 1 : 0
  }

  /** Moves the last particle into slot `index`. */
  private remove(index: number) {
    const last = --this.count
    if (index !== last) this.state.copyWithin(index * Particles.STRIDE, last * Particles.STRIDE, (last + 1) * Particles.STRIDE)
  }

  update(dt: number) {
    const s = this.state
    for (let i = this.count - 1; i >= 0; i--) {
      const at = i * Particles.STRIDE
      const age = s[at + 6]! + dt
      if (age >= s[at + 7]!) {
        this.remove(i)
        continue
      }
      s[at + 6] = age
      const keep = Math.max(0, 1 - s[at + 15]! * dt)
      let vx = s[at + 3]! * keep
      let vy = s[at + 4]! * keep - s[at + 14]! * dt
      let vz = s[at + 5]! * keep
      const x = s[at]! + vx * dt
      let y = s[at + 1]! + vy * dt
      const z = s[at + 2]! + vz * dt
      if (s[at + 16]) {
        const ground = this.groundAt(x, z) + 0.02
        if (y < ground) {
          y = ground
          vy = Math.abs(vy) * 0.3
          vx *= 0.5
          vz *= 0.5
        }
      }
      s[at] = x; s[at + 1] = y; s[at + 2] = z
      s[at + 3] = vx; s[at + 4] = vy; s[at + 5] = vz
    }
    const offsets = this.offsets.array as Float32Array
    const tints = this.tints.array as Float32Array
    const sizes = this.sizes.array as Float32Array
    for (let i = 0; i < this.count; i++) {
      const at = i * Particles.STRIDE
      const t = s[at + 6]! / s[at + 7]!
      offsets[i * 3] = s[at]!
      offsets[i * 3 + 1] = s[at + 1]!
      offsets[i * 3 + 2] = s[at + 2]!
      sizes[i] = s[at + 8]! + (s[at + 9]! - s[at + 8]!) * t
      tints[i * 4] = s[at + 10]!
      tints[i * 4 + 1] = s[at + 11]!
      tints[i * 4 + 2] = s[at + 12]!
      // Fades in over the first tenth, out over the rest.
      tints[i * 4 + 3] = s[at + 13]! * Math.min(1, t * 10) * (1 - t)
    }
    this.geometry.instanceCount = this.count
    this.offsets.needsUpdate = true
    this.tints.needsUpdate = true
    this.sizes.needsUpdate = true
  }

  /** Where particle `index` is, for tests. */
  positionOf(index: number): [number, number, number] {
    const at = index * Particles.STRIDE
    return [this.state[at]!, this.state[at + 1]!, this.state[at + 2]!]
  }

  dispose() {
    this.geometry.dispose()
    this.material.dispose()
    this.root.removeFromParent()
  }
}
