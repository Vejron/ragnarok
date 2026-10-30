import { Quaternion, Vector3 } from '@ragnarok/engine'
import { bayOutlet, damageSmoke, type Outlet } from '@/render/vehiclesmoke'
import { GONE, HURT, PART_ENGINE, burning, partState } from '@/sim/armor'
import { vehicleMaxHp } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new Vector3()

/**
 * A plume from a hurt vehicle (`health`): thin steam under 40% health, black smoke and flame as it
 * dies, and a burning column, lighting what is round it, once it is a wreck. An armoured vehicle's
 * parts add to it: a hit engine smokes, a dead one smokes black, and burning fuel burns as a wreck does.
 * It all comes out of the newest holes rounds made (`holes`), or the engine bay if there are none.
 */
export class Smoke implements Presenter<Frame> {
  /** The unspent fraction of a puff, per vehicle, carried between frames. */
  private readonly owed = new Map<number, number>()

  constructor(
    private readonly world: VehicleWorld,
    private readonly vehicles: Vehicles,
    /** The holes in vehicle `id` as drawn now, newest first (`HullMarks.vents`). */
    private readonly holes: (id: number) => Outlet[] = () => [],
  ) {}

  wants(entity: Entity) {
    return entity.health !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const placed = this.vehicles.placed.get(id)
    if (!placed) return
    const { kind, hp, damage } = placed
    const limit = vehicleMaxHp(kind) * 0.4
    const engine = partState(damage, PART_ENGINE)
    const fire = hp === 0 || burning(damage)
    const severity = Math.max(hp < limit ? 1 - hp / limit : -1, fire ? 1 : engine === GONE ? 0.75 : engine === HURT ? 0.35 : -1)
    if (severity < 0) {
      this.owed.delete(id)
      return
    }
    const holes = this.holes(id)
    const outlets = holes.length > 0 ? holes : [bayOutlet(kind, [placed.x, placed.y, placed.z], new Quaternion(...placed.rotation))]
    const owed = damageSmoke(this.world.fx(), outlets, this.vehicles.fx(id)?.velocity ?? STILL, severity, fire, frame.dt, this.owed.get(id) ?? 0)
    this.owed.set(id, owed)
    // A fire lights its surroundings with a flicker, from the hole it burns out of. Each light lives a fraction of a second, so one per frame holds it.
    if (fire) {
      const flicker = 0.7 + 0.3 * Math.sin(frame.now * 0.021 + id * 1.7) * Math.sin(frame.now * 0.0073 + id)
      const { x, y, z } = outlets[0]!
      this.world.lights.flash(x, y + 0.8, z, { color: 0xff7a24, intensity: 2.2 * flicker, radius: 16, life: 0.08 })
    }
  }

  remove(id: number) {
    this.owed.delete(id)
  }
}
