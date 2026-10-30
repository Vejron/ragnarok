import { Group, Mesh, Matrix4, Quaternion, Vector3, type Node } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import type { VehicleModel } from './car'

/**
 * What a gun on a wheeled carriage is built of, whichever gun: a mount that traverses, a cradle that
 * elevates on it, a barrel that recoils in that, the gunner's place, and the wheels and paint.
 */
export abstract class TowedGun implements Pick<VehicleModel, 'root'> {
  readonly root = new Group()
  /** Turns with the traverse: the hull-relative yaw. */
  protected readonly mount = new Group()
  /** Lifts with the elevation, about the trunnions. */
  protected readonly elevation = new Group()
  /** Runs back along the barrel's line when it fires. */
  protected readonly recoiling = new Group()
  /** Where the gunner's head is. */
  protected readonly seat = new Group()
  protected readonly spinners: Node[] = []
  protected readonly paint: ReturnType<typeof toonMaterial>
  protected readonly steel: ReturnType<typeof toonMaterial>
  protected readonly rubber: ReturnType<typeof toonMaterial>
  protected roll = 0
  protected placed = false
  /** 0 set up to fire, 1 folded for travel. */
  protected stow = 0
  /** `performance.now()` of the last `update`, for the fold's pace. */
  protected updatedAt = 0
  /** `performance.now()` of the last shot, for the barrel's recoil. */
  protected firedAt = -Infinity
  protected readonly last = new Vector3()
  protected readonly forward = new Vector3()
  private readonly pivot = new Vector3()
  private readonly aim = new Vector3()
  private readonly matrix = new Matrix4()

  constructor(paint: number, steel: number, rubber: number) {
    this.paint = toonMaterial({ color: paint })
    this.steel = toonMaterial({ color: steel })
    this.rubber = toonMaterial({ color: rubber })
  }

  protected part(geometry: ConstructorParameters<typeof Mesh>[0], material: ConstructorParameters<typeof Mesh>[1], parent: Node, x: number, y: number, z: number, shadow = true) {
    const mesh = new Mesh(geometry, material)
    mesh.position.set(x, y, z)
    mesh.castShadow = shadow
    parent.add(mesh)
    return mesh
  }

  /** The gunner's head point in the world and the mount's turn, as of the last `update`. */
  seatFrame(head: Vector3, rotation: Quaternion) {
    this.root.updateMatrixWorld(true)
    this.seat.getWorldPosition(head)
    const q = this.mount.getWorldQuaternion(new Quaternion())
    rotation.set(q.x, q.y, q.z, q.w)
  }

  /** A round was just fired: the barrel kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The barrel's root in world space as of the last `update`, and its unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.elevation.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.elevation.matrixWorld))
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Frees what this gun owns; the shared geometry stays. */
  dispose() {
    this.paint.dispose()
    this.steel.dispose()
    this.rubber.dispose()
    this.root.removeFromParent()
  }
}
