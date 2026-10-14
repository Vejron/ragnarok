import * as THREE from 'three'
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
  readonly root = new THREE.Group()
  private readonly ringMaterial = new THREE.MeshBasicMaterial({ color: 0xff8c2e, transparent: true, opacity: 0.9, depthTest: false, depthWrite: false, fog: false, side: THREE.DoubleSide })
  private readonly ringGeometry = new THREE.RingGeometry(MORTAR_RADIUS * 0.85, MORTAR_RADIUS, 64)
  private readonly ring = new THREE.Mesh(this.ringGeometry, this.ringMaterial)
  private readonly beamMaterial = new THREE.MeshBasicMaterial({ vertexColors: true, transparent: true, depthTest: false, depthWrite: false, fog: false, side: THREE.DoubleSide })
  private readonly beamGeometry = this.gradientBeam()
  private readonly beam = new THREE.Mesh(this.beamGeometry, this.beamMaterial)

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
    const geometry = new THREE.CylinderGeometry(0.6, 0.6, BEAM_HEIGHT, 8, 1, true)
    geometry.translate(0, BEAM_HEIGHT / 2, 0)
    const position = geometry.getAttribute('position')
    const colors = new Float32Array(position.count * 4)
    for (let i = 0; i < position.count; i++) {
      const alpha = 1 - position.getY(i) / BEAM_HEIGHT
      colors.set([1.0, 0.55, 0.18, alpha * alpha * 0.6], i * 4)
    }
    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 4))
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
    this.beam.position.set(impact[0], impact[1], impact[2])
    this.beamMaterial.opacity = 0.85 + 0.15 * pulse
  }

  dispose() {
    this.ringGeometry.dispose()
    this.ringMaterial.dispose()
    this.beamGeometry.dispose()
    this.beamMaterial.dispose()
    this.root.removeFromParent()
  }
}
