import { Vector3 } from '@ragnarok/engine'
import { CarModel } from '@/render/car'
import { coldGas } from '@/render/tyres'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new Vector3()

/** A car's thrusters (`thrust`): cold gas from each nozzle firing, the driver's own as predicted. */
export class Jets implements Presenter<Frame> {
  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.thrust !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const car = this.vehicles.model(id)
    const jets = this.vehicles.placed.get(id)?.jets ?? 0
    if (car instanceof CarModel && jets) coldGas(this.world.fx(), car.jets(jets), this.vehicles.fx(id)?.velocity ?? STILL, frame.dt)
  }

  remove() {}
}
