import * as THREE from 'three'

/** `PROJECTILE_GRENADE`, `PROJECTILE_SHELL` and `PROJECTILE_CANNON` in codec.ts. */
const GRENADE = 1
const SHELL = 2
const CANNON = 3
const BULLET = 4

/** The projectiles in flight: a glowing missile or a tank shell pointed along its velocity, or a tumbling grenade. */
export class Rockets {
  readonly root = new THREE.Group()
  private readonly body = new THREE.CylinderGeometry(0.09, 0.09, 0.8, 8)
  private readonly material = new THREE.MeshBasicMaterial({ color: 0xff8a2a })
  /** A tank shell: a fat hot slug with a long trail, big enough to follow at 140 m/s over a distance. */
  private readonly slug = new THREE.CylinderGeometry(0.3, 0.3, 2.4, 10)
  private readonly slugMaterial = new THREE.MeshBasicMaterial({ color: 0xfff0b0, fog: false })
  private readonly trail = new THREE.CylinderGeometry(0.22, 0.02, 16, 8)
  private readonly trailMaterial = new THREE.MeshBasicMaterial({ color: 0xff9a3c, transparent: true, opacity: 0.55, fog: false, depthWrite: false })
  /** An autocannon round: the same slug and trail, a third the size. */
  private readonly smallSlug = new THREE.CylinderGeometry(0.1, 0.1, 0.8, 8)
  private readonly smallTrail = new THREE.CylinderGeometry(0.07, 0.01, 5, 6)
  /** A sniper round: a thin white-hot streak with a short trail. */
  private readonly bullet = new THREE.CylinderGeometry(0.04, 0.04, 1.2, 6)
  private readonly bulletTrail = new THREE.CylinderGeometry(0.035, 0.005, 9, 6)
  private readonly ball = new THREE.SphereGeometry(0.1, 10, 8)
  private readonly ballMaterial = new THREE.MeshToonMaterial({ color: 0x4f5d2e })
  private readonly meshes = new Map<number, THREE.Mesh>()
  private readonly up = new THREE.Vector3(0, 1, 0)

  /** Places projectile `id`, creating it on first sight. `velocity` only gives a rocket's or a shell's heading. */
  place(id: number, kind: number, position: readonly [number, number, number], velocity: readonly [number, number, number]) {
    let mesh = this.meshes.get(id)
    if (!mesh) {
      if (kind === GRENADE) mesh = new THREE.Mesh(this.ball, this.ballMaterial)
      else if (kind === BULLET) {
        mesh = new THREE.Mesh(this.bullet, this.slugMaterial)
        const tail = new THREE.Mesh(this.bulletTrail, this.trailMaterial)
        tail.position.y = -(0.6 + 4.5)
        mesh.add(tail)
      } else if (kind === CANNON) {
        mesh = new THREE.Mesh(this.smallSlug, this.slugMaterial)
        const tail = new THREE.Mesh(this.smallTrail, this.trailMaterial)
        tail.position.y = -(0.4 + 2.5)
        mesh.add(tail)
      } else if (kind === SHELL) {
        mesh = new THREE.Mesh(this.slug, this.slugMaterial)
        // The trail streams out behind, along the shell's own -Y.
        const tail = new THREE.Mesh(this.trail, this.trailMaterial)
        tail.position.y = -(1.2 + 8)
        mesh.add(tail)
      } else mesh = new THREE.Mesh(this.body, this.material)
      this.meshes.set(id, mesh)
      this.root.add(mesh)
    }
    mesh.position.set(...position)
    if (kind === GRENADE) {
      mesh.rotation.x += 0.25
      return
    }
    const heading = new THREE.Vector3(...velocity)
    if (heading.lengthSq() > 1e-6) mesh.quaternion.setFromUnitVectors(this.up, heading.normalize())
  }

  remove(id: number) {
    const mesh = this.meshes.get(id)
    if (!mesh) return
    mesh.removeFromParent()
    this.meshes.delete(id)
  }

  has(id: number) {
    return this.meshes.has(id)
  }

  dispose() {
    this.meshes.clear()
    this.body.dispose()
    this.material.dispose()
    this.slug.dispose()
    this.slugMaterial.dispose()
    this.trail.dispose()
    this.smallSlug.dispose()
    this.smallTrail.dispose()
    this.bullet.dispose()
    this.bulletTrail.dispose()
    this.trailMaterial.dispose()
    this.ball.dispose()
    this.ballMaterial.dispose()
    this.root.removeFromParent()
  }
}
