import * as THREE from 'three'

/** `PROJECTILE_GRENADE`, `PROJECTILE_SHELL` and `PROJECTILE_CANNON` in codec.ts. */
const GRENADE = 1
const SHELL = 2
/** A mortar bomb, drawn as a shell. */
const MORTAR = 6
const CANNON = 3
const BULLET = 4
const BOMB = 5

/** The projectiles in flight: a glowing missile or a tank shell pointed along its velocity, or a tumbling grenade. */
export class Rockets {
  readonly root = new THREE.Group()
  private readonly body = new THREE.CylinderGeometry(0.09, 0.09, 0.8, 8)
  private readonly material = new THREE.MeshBasicMaterial({ color: 0xff8a2a, fog: false, toneMapped: false })
  /** A tank shell: a fat hot slug with a long trail, big enough to follow at 140 m/s over a distance. */
  private readonly slug = new THREE.CylinderGeometry(0.3, 0.3, 2.4, 10)
  private readonly slugMaterial = new THREE.MeshBasicMaterial({ color: 0xffb020, fog: false, toneMapped: false })
  private readonly trail = new THREE.CylinderGeometry(0.22, 0.02, 16, 8)
  private readonly trailMaterial = new THREE.MeshBasicMaterial({ color: 0xff6a10, transparent: true, opacity: 0.75, fog: false, toneMapped: false, depthWrite: false })
  /** An autocannon round: the same slug and trail, a third the size. */
  private readonly smallSlug = new THREE.CylinderGeometry(0.15, 0.15, 1, 8)
  private readonly smallTrail = new THREE.CylinderGeometry(0.13, 0.01, 6, 6)
  /** A gun's round: a thin white-hot core with a fine orange tail, a streak rather than a rod. */
  private readonly bullet = new THREE.CylinderGeometry(0.014, 0.014, 1.2, 5)
  private readonly bulletTrail = new THREE.CylinderGeometry(0.035, 0.002, 7, 5)
  private readonly bulletMaterial = new THREE.MeshBasicMaterial({ color: 0xfff6dc, fog: false, toneMapped: false })
  private readonly bulletTrailMaterial = new THREE.MeshBasicMaterial({ color: 0xff9a2e, transparent: true, opacity: 0.85, fog: false, toneMapped: false, depthWrite: false })
  private readonly ball = new THREE.SphereGeometry(0.1, 10, 8)
  /** A plane's bomb: a dark finned body that noses over as it falls. */
  private readonly bomb = new THREE.CylinderGeometry(0.075, 0.05, 0.6, 8)
  private readonly fins = new THREE.BoxGeometry(0.2, 0.1, 0.02)
  private readonly bombMaterial = new THREE.MeshToonMaterial({ color: 0x2b2b26 })
  private readonly ballMaterial = new THREE.MeshToonMaterial({ color: 0x4f5d2e })
  private readonly meshes = new Map<number, THREE.Mesh>()
  private readonly up = new THREE.Vector3(0, 1, 0)

  /** Places projectile `id`, creating it on first sight. `velocity` only gives a rocket's or a shell's heading; `flown` is how far a bullet has come, which is as long as its streak may be. */
  place(id: number, kind: number, position: readonly [number, number, number], velocity: readonly [number, number, number], flown = Infinity) {
    let mesh = this.meshes.get(id)
    if (!mesh) {
      if (kind === GRENADE) mesh = new THREE.Mesh(this.ball, this.ballMaterial)
      else if (kind === BOMB) {
        mesh = new THREE.Mesh(this.bomb, this.bombMaterial)
        for (const turn of [0, Math.PI / 2]) {
          const fin = new THREE.Mesh(this.fins, this.bombMaterial)
          fin.position.y = -0.3
          fin.rotation.y = turn
          mesh.add(fin)
        }
      } else if (kind === BULLET) {
        mesh = new THREE.Mesh(this.bullet, this.bulletMaterial)
        const tail = new THREE.Mesh(this.bulletTrail, this.bulletTrailMaterial)
        tail.position.y = -(0.6 + 3.5)
        mesh.add(tail)
      } else if (kind === CANNON) {
        mesh = new THREE.Mesh(this.smallSlug, this.slugMaterial)
        const tail = new THREE.Mesh(this.smallTrail, this.trailMaterial)
        tail.position.y = -(0.4 + 2.5)
        mesh.add(tail)
      } else if (kind === SHELL || kind === MORTAR) {
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
    if (kind === BULLET) {
      // Only as much streak as the round has flown since it left the barrel.
      const tail = mesh.children[0]!
      const length = Math.min(1, Math.max(0.01, flown / 7))
      tail.scale.y = length
      tail.position.y = -(0.6 + 3.5 * length)
    }
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
    this.bulletMaterial.dispose()
    this.bulletTrailMaterial.dispose()
    this.trailMaterial.dispose()
    this.ball.dispose()
    this.ballMaterial.dispose()
    this.bomb.dispose()
    this.fins.dispose()
    this.bombMaterial.dispose()
    this.root.removeFromParent()
  }
}
