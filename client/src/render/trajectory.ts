import * as THREE from 'three'
import type { Trajectory } from '@/sim/grenade'

/** Dots a path can have: the fuse in ticks over the sampling stride, with room to spare. */
const MAX_DOTS = 100
/** Drawn over everything, so a wall or a car in front of the arc never hides it. */
const ON_TOP = 999

/**
 * The arc a thrown grenade will take, drawn while one is in hand: bright dots up to where it first
 * lands, a ring there, and fainter dots for the bounce and roll after it.
 */
export class TrajectoryView {
  readonly root = new THREE.Group()
  private readonly dot = new THREE.SphereGeometry(1, 8, 6)
  private readonly ring = new THREE.RingGeometry(0.28, 0.42, 24)
  private readonly flightMaterial = this.material(0xffffff, 0.95)
  private readonly rollMaterial = this.material(0xffffff, 0.35)
  private readonly ringMaterial = this.material(0xffd54a, 1)
  private readonly dots: THREE.Mesh[] = []
  private readonly landing: THREE.Mesh

  constructor() {
    for (let i = 0; i < MAX_DOTS; i++) {
      const mesh = new THREE.Mesh(this.dot, this.flightMaterial)
      mesh.renderOrder = ON_TOP
      mesh.visible = false
      mesh.frustumCulled = false
      this.dots.push(mesh)
      this.root.add(mesh)
    }
    this.landing = new THREE.Mesh(this.ring, this.ringMaterial)
    this.landing.rotation.x = -Math.PI / 2
    this.landing.renderOrder = ON_TOP
    this.landing.frustumCulled = false
    this.landing.visible = false
    this.root.add(this.landing)
    this.root.visible = false
  }

  private material(color: number, opacity: number) {
    return new THREE.MeshBasicMaterial({ color, transparent: true, opacity, depthTest: false, depthWrite: false, fog: false, side: THREE.DoubleSide })
  }

  /** Shows `path`, or hides the view when it is null. */
  set(path: Trajectory | null) {
    this.root.visible = path !== null
    if (!path) return
    const count = Math.min(path.points.length, MAX_DOTS)
    for (let i = 0; i < MAX_DOTS; i++) {
      const mesh = this.dots[i]!
      mesh.visible = i < count
      if (i >= count) continue
      const [x, y, z] = path.points[i]!
      mesh.position.set(x, y, z)
      const flight = i < path.flying
      mesh.material = flight ? this.flightMaterial : this.rollMaterial
      mesh.scale.setScalar(flight ? 0.07 : 0.05)
    }
    this.landing.visible = path.firstImpact !== null
    if (path.firstImpact) this.landing.position.set(path.firstImpact[0], path.firstImpact[1] + 0.05, path.firstImpact[2])
  }

  dispose() {
    this.dot.dispose()
    this.ring.dispose()
    this.flightMaterial.dispose()
    this.rollMaterial.dispose()
    this.ringMaterial.dispose()
    this.root.removeFromParent()
  }
}
