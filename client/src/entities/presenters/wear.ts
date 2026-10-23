import * as THREE from 'three'
import { DAMAGE_TURRET, TURRET_GONE } from '@/net/codec'
import { vehicleDef } from '@/sim/vehicles'
import { CarModel } from '@/render/car'
import { TankModel } from '@/render/tank'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new THREE.Vector3()

/**
 * What a vehicle's hurts look like (`damage` and `health`): a car's dents, cracks and parts coming off,
 * and a tank's turret thrown clear when it is shot off, put back when it is repaired.
 */
export class Wear implements Presenter<Frame> {
  /** Tanks whose turret has already been sent flying, so it is thrown once. */
  private readonly shed = new Set<number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.orientation !== undefined
  }

  update(id: number) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!model || !placed) return
    if (model instanceof CarModel) model.wear(placed.damage, placed.hp, this.world.debris, this.vehicles.fx(id)?.velocity ?? STILL)
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
    this.world.debris.throw(copy, new THREE.Vector3(spread(), 9 + Math.random() * 3, spread()), new THREE.Vector3(spread() / 2, spread() / 3, spread() / 2), 0.5, 25)
  }
}
