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
/** A rocket of the Orange Plague's airstrike: a big glowing missile, nose down. */
const STRIKE = 7

/** The projectiles in flight: a glowing missile or a tank shell pointed along its velocity, or a tumbling grenade. Their trails are `ProjectileTrails`. */
export class Rockets {
  readonly root = new Group()
  private readonly body = cylinderGeometry(0.09, 0.09, 0.8, 8)
  private readonly material = new Material({ color: 0xff8a2a, lit: false, fog: false })
  /** A tank shell: a fat hot slug with a long trail, big enough to follow at 140 m/s over a distance. */
  private readonly slug = cylinderGeometry(0.3, 0.3, 2.4, 10)
  private readonly slugMaterial = new Material({ color: 0xffb020, lit: false, fog: false })
  /** An autocannon round: the same slug, a third the size. */
  private readonly smallSlug = cylinderGeometry(0.15, 0.15, 1, 8)
  /** A gun's round: a thin white-hot core; its streak is a ribbon (`trails.ts`). */
  private readonly bullet = cylinderGeometry(0.014, 0.014, 1.2, 5)
  private readonly bulletMaterial = new Material({ color: 0xfff6dc, lit: false, fog: false })
  private readonly ball = sphereGeometry(0.1, 10, 8)
  /** A plane's bomb: a dark finned body that noses over as it falls. */
  private readonly bomb = cylinderGeometry(0.075, 0.05, 0.6, 8)
  private readonly fins = boxGeometry(0.2, 0.1, 0.02)
  private readonly bombMaterial = toonMaterial({ color: 0x2b2b26 })
  private readonly ballMaterial = toonMaterial({ color: 0x4f5d2e })
  /** An airstrike rocket: big enough to see coming from far off, and hot red so it reads as his. */
  private readonly missile = cylinderGeometry(0.35, 0.35, 3.2, 10)
  private readonly missileMaterial = new Material({ color: 0xff4a2a, lit: false, fog: false })
  private readonly meshes = new Map<number, Mesh>()
  private readonly up = new THREE.Vector3(0, 1, 0)

  /** Places projectile `id`, creating it on first sight. `velocity` only gives a rocket's or a shell's heading. */
  place(id: number, kind: number, position: readonly [number, number, number], velocity: readonly [number, number, number]) {
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
      } else if (kind === CANNON) {
        mesh = new Mesh(this.smallSlug, this.slugMaterial)
      } else if (kind === STRIKE) {
        mesh = new Mesh(this.missile, this.missileMaterial)
      } else if (kind === SHELL || kind === MORTAR) {
        mesh = new Mesh(this.slug, this.slugMaterial)
      } else mesh = new Mesh(this.body, this.material)
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
    this.smallSlug.dispose()
    this.bullet.dispose()
    this.bulletMaterial.dispose()
    this.ball.dispose()
    this.ballMaterial.dispose()
    this.bomb.dispose()
    this.fins.dispose()
    this.bombMaterial.dispose()
    this.root.removeFromParent()
  }
}
