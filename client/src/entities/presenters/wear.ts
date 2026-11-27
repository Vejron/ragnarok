import { Vector3 } from '@ragnarok/engine'
import { DAMAGE_TURRET, FLAG_WRECKED, TURRET_GONE } from '@/net/codec'
import { vehicleDef } from '@/sim/vehicles'
import { CarModel } from '@/render/car'
import { TankModel } from '@/render/tank'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { Sampled } from '../store'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new Vector3()

/**
 * What a vehicle's hurts look like (`damage`, and `flags` once it is a wreck): a car's dents, cracks, parts coming off and burnt paint,
 * and a tank's turret thrown clear when it is shot off, put back when it is repaired.
 */
export class Wear implements Presenter<Frame> {
  /** Tanks whose turret has already been sent flying, so it is thrown once. */
  private readonly shed = new Set<number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.orientation !== undefined
  }

  update(id: number, sampled: Sampled, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!model || !placed) return
    if (model instanceof CarModel) {
      const velocity = this.vehicles.fx(id)?.velocity ?? STILL
      // Charred only once it has gone up, not while it burns with its health gone (`FLAG_WRECKED`).
      const wrecked = ((sampled.entity.flags?.flags ?? 0) & FLAG_WRECKED) !== 0
      model.wear(placed.damage, wrecked, this.world.debris, velocity)
      model.swingDoors(placed.doors.left, placed.doors.right, placed.doors.gone, frame.dt, this.world.debris, velocity)
    }
    if (model instanceof TankModel && vehicleDef(placed.kind).shedsTurret) {
      if (placed.damage[DAMAGE_TURRET] === TURRET_GONE) this.shedTurret(id)
      else if (this.shed.delete(id)) model.setTurret(true)
    }
  }

  remove(id: number) {
    this.shed.delete(id)
  }

  /** Sends tank `id`'s turret flying, once: a copy of it tumbles off as debris and the tank is left with the bare ring. */
  shedTurret(id: number) {
    const tank = this.vehicles.model(id)
    if (!(tank instanceof TankModel) || this.shed.has(id)) return
    this.shed.add(id)
    const copy = tank.turretCopy()
    tank.setTurret(false)
    if (!copy) return
    const spread = () => (Math.random() - 0.5) * 8
    this.world.debris.throw(copy, new Vector3(spread(), 9 + Math.random() * 3, spread()), new Vector3(spread() / 2, spread() / 3, spread() / 2), 0.5, 25)
  }
}
