import { Matrix4, type Node, Vector3 } from '@ragnarok/engine'
import type { Turret } from '@/entities/units'
import { AMV_TUBE_X } from '@/sim/consts'
import { AMV_LOB } from '@/sim/lob'
import { CarModel } from './car'
import type { LobGun } from './mortar'

/** How far a tube kicks back along its line when it fires, metres, and how quickly it is back, seconds. */
const RECOIL_STROKE = 0.25
const RECOIL_SECONDS = 0.35

/**
 * The AMV XA-361: an eight-wheeled car with a mortar turret. The turret (`turret`) turns on its ring and
 * the twin tubes (`gun`) elevate in it, both as the server aims them, in the hull frame. The tubes take
 * turns, as the server fires them (`Muzzle::Twin`), so `barrel` can give either one.
 */
export class AmvModel extends CarModel implements LobGun {
  readonly lob = AMV_LOB
  private readonly turret: Node
  private readonly gun: Node
  private firedAt = -Infinity
  private readonly matrix = new Matrix4()

  constructor(id: number) {
    super('amv', id)
    this.turret = this.root.getObjectByName('turret')!
    this.gun = this.root.getObjectByName('gun')!
  }

  /** Places the carrier as any car, then turns the turret and elevates the tubes to the server's aim. */
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
    const kick = since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) : 0
    // Back along the tubes' line from the pivot, whatever their elevation.
    this.gun.position.set(0, -kick * Math.sin(this.gun.rotation.x), kick * Math.cos(this.gun.rotation.x))
  }

  /** A round was just fired: the tubes kick back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /**
   * A tube in world space as of the last `update`: where it pivots and the unit aim direction. `side` is
   * −1 for the left tube, 1 for the right and 0 for the middle between them.
   */
  barrel(side = 0) {
    this.root.updateMatrixWorld(true)
    this.matrix.fromArray(this.gun.matrixWorld)
    const origin = new Vector3(side * AMV_TUBE_X, 0, 0).applyMatrix4(this.matrix)
    const direction = new Vector3(0, 0, -1).transformDirection(this.matrix)
    return { origin, direction }
  }
}
