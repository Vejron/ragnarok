import { Geometry, Group, Material, Mesh, cylinderGeometry, ringGeometry } from '@ragnarok/engine'
import type { Trajectory } from '@/sim/grenade'
import { MORTAR_RADIUS } from '@/sim/consts'

/** Drawn over everything, so a wall or a car in front of the mark never hides it. */
const ON_TOP = 999
/** How high the beam over the impact stands, metres. It fades out towards the top. */
const BEAM_HEIGHT = 60
/** The ring breathes in and out this often, per second, so the mark looks live. */
const PULSE_RATE = 3

/**
 * A field mortar's landing mark: a ring the size of the blast, pulsing on the ground where the bomb
 * would first land, and a beam standing over it that fades out towards the sky.
 */
export class MortarArc {
  readonly root = new Group()
  private readonly ringMaterial = new Material({ color: 0xff8c2e, lit: false, blend: 'alpha', opacity: 0.9, depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly ringGeometry = ringGeometry(MORTAR_RADIUS * 0.85, MORTAR_RADIUS, 64)
  private readonly ring = new Mesh(this.ringGeometry, this.ringMaterial)
  private readonly beamMaterial = new Material({ vertexColors: true, lit: false, blend: 'alpha', depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly beamGeometry = this.gradientBeam()
  private readonly beam = new Mesh(this.beamGeometry, this.beamMaterial)

  constructor() {
    this.ring.rotation.x = -Math.PI / 2
    this.ring.renderOrder = ON_TOP
    this.ring.frustumCulled = false
    this.beam.renderOrder = ON_TOP
    this.beam.frustumCulled = false
    this.root.add(this.ring, this.beam)
    this.root.visible = false
  }

  /** A tall open cylinder whose vertices carry alpha: solid at the ground, clear at the top. */
  private gradientBeam() {
    // Sides only: the tube's end caps (the last two vertex rings' worth of the cylinder) are cut off.
    const tube = cylinderGeometry(0.6, 0.6, BEAM_HEIGHT, 8)
    const sides = 9 * 2
    const index = tube.indices!
    const geometry = new Geometry({
      positions: tube.positions.slice(0, sides * 3),
      indices: index.slice(0, 8 * 6),
    })
    geometry.translate(0, BEAM_HEIGHT / 2, 0)
    const count = geometry.vertexCount
    const colors = new Float32Array(count * 4)
    for (let i = 0; i < count; i++) {
      const alpha = 1 - geometry.positions[i * 3 + 1]! / BEAM_HEIGHT
      colors.set([1.0, 0.55, 0.18, alpha * alpha * 0.6], i * 4)
    }
    geometry.colors = colors
    return geometry
  }

  /** Shows the mark where `path` first lands, or hides it when it is null or never lands. Call every frame to animate. */
  set(path: Trajectory | null) {
    const impact = path?.firstImpact ?? null
    this.root.visible = impact !== null
    if (!impact) return
    const pulse = Math.sin((performance.now() / 1000) * PULSE_RATE * Math.PI * 2)
    this.ring.position.set(impact[0], impact[1] + 0.05, impact[2])
    this.ring.scale.setScalar(1 + 0.05 * pulse)
    this.ringMaterial.opacity = 0.65 + 0.25 * pulse
    this.ringMaterial.needsUpdate()
    this.beam.position.set(impact[0], impact[1], impact[2])
    this.beamMaterial.opacity = 0.85 + 0.15 * pulse
    this.beamMaterial.needsUpdate()
  }

  dispose() {
    this.ringGeometry.dispose()
    this.ringMaterial.dispose()
    this.beamGeometry.dispose()
    this.beamMaterial.dispose()
    this.root.removeFromParent()
  }
}
