/**
 * The arcs drawn ahead of what the local player is about to throw, fire or drop: a grenade in hand, a
 * field mortar's bomb, or a plane's bomb, where each would land if let go now.
 */

import { Matrix4, Vector3 } from '@ragnarok/engine'
import type { VehicleModel } from '@/render/car'
import { MortarModel } from '@/render/mortar'
import type { MortarArc } from '@/render/mortar-arc'
import { PlaneModel } from '@/render/plane'
import type { TrajectoryView } from '@/render/trajectory'
import type { VehicleFx } from '@/render/tyres'
import { MORTAR_BARREL_LENGTH, MORTAR_SHELL_SPEED, MORTAR_SHELL_TTL_TICKS, PLANE_BOMB_MIN_HEIGHT, PLANE_BOMB_PIVOT } from '@/sim/consts'
import { dropTrajectory, throwTrajectory } from '@/sim/grenade'
import type { SimMotor } from '@/sim/motor'
import type { SeatView } from '@/seats'

/** What the arcs are worked out from this frame. */
export interface Aim {
  motor: SimMotor | null
  /** The seat's arc, when seated. */
  arc: SeatView['arc'] | null
  /** The vehicle sat in: its model, its motion, and the bombs on its rack. */
  vehicle: { model: VehicleModel | undefined; motion: VehicleFx | undefined; bombs: number } | null
  /** A grenade in hand, free to throw: where from, and the look. */
  grenade: { eye: [number, number, number]; yaw: number; pitch: number } | null
  groundAt: (x: number, z: number) => number
}

export class Arcs {
  constructor(private readonly trajectory: TrajectoryView, private readonly mortarArc: MortarArc) {}

  update({ motor, arc, vehicle, grenade, groundAt }: Aim) {
    const castRay = (origin: [number, number, number], dir: [number, number, number], max: number) => motor!.castRay(origin, dir, max)
    this.mortarArc.set(motor && arc === 'mortar' && vehicle ? this.mortarPath(vehicle.model, castRay) : null)
    if (motor && arc === 'mortar') this.trajectory.set(null)
    else if (motor && arc === 'bombs' && vehicle) this.trajectory.set(this.bombPath(vehicle, castRay, groundAt))
    else if (motor && grenade) this.trajectory.set(throwTrajectory(castRay, grenade.eye, grenade.yaw, grenade.pitch))
    else this.trajectory.set(null)
  }

  /** For a field mortar's gunner: where a bomb fired now would land, from the tube's muzzle at the mortar's muzzle speed. */
  private mortarPath(mount: VehicleModel | undefined, castRay: Parameters<typeof dropTrajectory>[0]) {
    if (!(mount instanceof MortarModel)) return null
    const { origin, direction } = mount.barrel()
    const start = origin.clone().addScaledVector(direction, MORTAR_BARREL_LENGTH)
    const velocity = direction.clone().multiplyScalar(MORTAR_SHELL_SPEED)
    // Only where it first lands matters: the bomb flies its whole lifetime, as the real one does.
    return dropTrajectory(castRay, [start.x, start.y, start.z], [velocity.x, velocity.y, velocity.z], 3, MORTAR_SHELL_TTL_TICKS)
  }

  /** For a plane's pilot with bombs on the rack and room under them: where one let go now would land. */
  private bombPath(vehicle: NonNullable<Aim['vehicle']>, castRay: Parameters<typeof dropTrajectory>[0], groundAt: Aim['groundAt']) {
    const { model: plane, motion, bombs } = vehicle
    if (!(plane instanceof PlaneModel) || !motion || bombs === 0) return null
    plane.root.updateMatrixWorld(true)
    const start = new Vector3(...PLANE_BOMB_PIVOT).applyMatrix4(new Matrix4().fromArray(plane.root.matrixWorld))
    if (start.y - groundAt(start.x, start.z) < PLANE_BOMB_MIN_HEIGHT) return null
    const velocity = motion.velocity
    return dropTrajectory(castRay, [start.x, start.y, start.z], [velocity.x, velocity.y, velocity.z])
  }
}
