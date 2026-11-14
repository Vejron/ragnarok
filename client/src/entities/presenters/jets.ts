import { Vector3 } from '@ragnarok/engine'
import { CarModel } from '@/render/car'
import { coldGas, thrusterFlame } from '@/render/tyres'
import { TitanModel } from '@/render/titan'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new Vector3()

/** A vehicle's thrusters (`thrust`): cold gas from each of a car's nozzles firing, flame from a Titan's feet; the driver's own as predicted. */
export class Jets implements Presenter<Frame> {
  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.thrust !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const car = this.vehicles.model(id)
    const jets = this.vehicles.placed.get(id)?.jets ?? 0
    if (car instanceof CarModel && jets) coldGas(this.world.fx(), car.jets(jets), this.vehicles.fx(id)?.velocity ?? STILL, frame.dt)
    // A Titan's burn, and light what is under them.
    if (car instanceof TitanModel && jets) {
      const flames = car.jets(jets)
      thrusterFlame(this.world.fx(), flames, this.vehicles.fx(id)?.velocity ?? STILL, frame.dt)
      for (const { at } of flames) this.world.lights.hold(at.x, at.y - 4, at.z, { color: 0xff8a3a, intensity: 5, radius: 45 })
    }
  }

  remove() {}
}
