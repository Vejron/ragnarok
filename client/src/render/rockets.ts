import { Group, Material, Mesh, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'

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
  readonly root = new Group()
  private readonly body = cylinderGeometry(0.09, 0.09, 0.8, 8)
  private readonly material = new Material({ color: 0xff8a2a, lit: false, fog: false })
  /** A tank shell: a fat hot slug with a long trail, big enough to follow at 140 m/s over a distance. */
  private readonly slug = cylinderGeometry(0.3, 0.3, 2.4, 10)
  private readonly slugMaterial = new Material({ color: 0xffb020, lit: false, fog: false })
  private readonly trail = cylinderGeometry(0.22, 0.02, 16, 8)
  private readonly trailMaterial = new Material({ color: 0xff6a10, lit: false, blend: 'alpha', opacity: 0.75, fog: false, depthWrite: false })
  /** An autocannon round: the same slug and trail, a third the size. */
  private readonly smallSlug = cylinderGeometry(0.15, 0.15, 1, 8)
  private readonly smallTrail = cylinderGeometry(0.13, 0.01, 6, 6)
  /** A gun's round: a thin white-hot core with a fine orange tail, a streak rather than a rod. */
  private readonly bullet = cylinderGeometry(0.014, 0.014, 1.2, 5)
  private readonly bulletTrail = cylinderGeometry(0.035, 0.002, 7, 5)
  private readonly bulletMaterial = new Material({ color: 0xfff6dc, lit: false, fog: false })
  private readonly bulletTrailMaterial = new Material({ color: 0xff9a2e, lit: false, blend: 'alpha', opacity: 0.85, fog: false, depthWrite: false })
  private readonly ball = sphereGeometry(0.1, 10, 8)
  /** A plane's bomb: a dark finned body that noses over as it falls. */
  private readonly bomb = cylinderGeometry(0.075, 0.05, 0.6, 8)
  private readonly fins = boxGeometry(0.2, 0.1, 0.02)
  private readonly bombMaterial = toonMaterial({ color: 0x2b2b26 })
  private readonly ballMaterial = toonMaterial({ color: 0x4f5d2e })
  private readonly meshes = new Map<number, Mesh>()
  private readonly up = new THREE.Vector3(0, 1, 0)

  /** Places projectile `id`, creating it on first sight. `velocity` only gives a rocket's or a shell's heading; `flown` is how far a bullet has come, which is as long as its streak may be. */
  place(id: number, kind: number, position: readonly [number, number, number], velocity: readonly [number, number, number], flown = Infinity) {
    let mesh = this.meshes.get(id)
    if (!mesh) {
      if (kind === GRENADE) mesh = new Mesh(this.ball, this.ballMaterial)
      else if (kind === BOMB) {
        mesh = new Mesh(this.bomb, this.bombMaterial)
        for (const turn of [0, Math.PI / 2]) {
          const fin = new Mesh(this.fins, this.bombMaterial)
          fin.position.y = -0.3
          fin.rotation.y = turn
          mesh.add(fin)
        }
      } else if (kind === BULLET) {
        mesh = new Mesh(this.bullet, this.bulletMaterial)
        const tail = new Mesh(this.bulletTrail, this.bulletTrailMaterial)
        tail.position.y = -(0.6 + 3.5)
        mesh.add(tail)
      } else if (kind === CANNON) {
        mesh = new Mesh(this.smallSlug, this.slugMaterial)
        const tail = new Mesh(this.smallTrail, this.trailMaterial)
        tail.position.y = -(0.4 + 2.5)
        mesh.add(tail)
      } else if (kind === SHELL || kind === MORTAR) {
        mesh = new Mesh(this.slug, this.slugMaterial)
        // The trail streams out behind, along the shell's own -Y.
        const tail = new Mesh(this.trail, this.trailMaterial)
        tail.position.y = -(1.2 + 8)
        mesh.add(tail)
      } else mesh = new Mesh(this.body, this.material)
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
