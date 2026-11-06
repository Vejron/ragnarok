import { Matrix4, type Node, Vector3 } from '@ragnarok/engine'
import type { Turret } from '@/entities/units'
import { CarModel } from './car'

/** How far the gun kicks back along its line when it fires, metres, and how quickly it is back, seconds. */
const RECOIL_STROKE = 0.09
const RECOIL_SECONDS = 0.12

/**
 * The technical: a car like the jeep, with a machine gun on a post in its bed. The mount (`turret`)
 * turns on the post and the gun (`gun`) elevates in it, both as the server aims them, in the hull frame.
 */
export class TechnicalModel extends CarModel {
  private readonly turret: Node
  private readonly gun: Node
  private readonly gunRest: number
  private firedAt = -Infinity
  private readonly pivot = new Vector3()
  private readonly aim = new Vector3()
  private readonly matrix = new Matrix4()

  constructor(id: number) {
    super('technical', id)
    this.turret = this.root.getObjectByName('turret')!
    this.gun = this.root.getObjectByName('gun')!
    this.gunRest = this.gun.position.z
  }

  /** Places the truck as any car, then turns and elevates the gun to the server's aim. */
  override update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    _seats?: readonly number[],
    turret?: Turret | null,
  ) {
    super.update(position, rotation, steer)
    this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    const since = (performance.now() - this.firedAt) / 1000
    this.gun.position.z = this.gunRest + (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) : 0)
  }

  /** A round was just fired: the gun kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The gun in world space as of the last `update`: its pivot and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.gun.matrixWorld))
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }
}
