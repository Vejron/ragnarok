import { FLAG_SIREN, KIND_AMBULANCE, KIND_POLICE_CAR } from '@/net/codec'
import { CarModel } from '@/render/car'
import { dayPhase, nightAt } from '@/render/daynight'
import { TICK_RATE } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** The blue a beacon throws on what is round it. */
const COLOR = 0x3f7dff
/** Strength at full dark (`PointLights` intensity). */
const INTENSITY = 4
/** The share of its strength a beacon has by day, so one flashing at noon still tints the roof under it. */
const BY_DAY = 0.3
/** How far a beacon's light reaches, metres. */
const REACH = 18
/** How far over the middle of its lens a beacon's light is held, metres, so it falls on the roof and the road and not inside the lens. */
const LIFT = 0.15
/** How long each lens is alight before the other takes over, milliseconds: the pair goes round twice a second. */
export const BEACON_SIDE_MS = 250

/** The kinds with a light bar and a siren. */
const KINDS = new Set([KIND_POLICE_CAR, KIND_AMBULANCE])

/**
 * A police car's or an ambulance's light bar and siren, on while the server says so (`FLAG_SIREN`) and
 * the vehicle is not a wreck. The bar's two lenses (`beacon_L` and `beacon_R` in the model) take turns,
 * `BEACON_SIDE_MS` each by the frame's clock: the lens whose turn it is is drawn alight, and a blue
 * point light is held over it, at full strength in the dark and `BY_DAY` of it by day. Each vehicle
 * starts on the side its id gives it, so two side by side do not flash as one. The siren is heard from
 * where the vehicle is (`VehicleSounds.siren`).
 *
 * Runs after the vehicles are placed, since the lenses are where the model is drawn.
 */
export class Beacons implements Presenter<Frame> {
  /** The vehicles whose bars are flashing: one that stops has its lenses put out once. */
  private readonly flashing = new Set<number>()
  private night = 0

  constructor(private readonly world: Pick<VehicleWorld, 'lights' | 'sounds'>, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.orientation !== undefined && entity.kind !== undefined && KINDS.has(entity.kind.kind)
  }

  begin(frame: Frame) {
    this.night = nightAt(dayPhase(frame.tick, TICK_RATE))
    this.world.sounds.beginSirens()
  }

  update(id: number, { entity, velocity }: Sampled, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!(model instanceof CarModel) || !model.hasBeacons || !placed) return
    const on = ((entity.flags?.flags ?? 0) & FLAG_SIREN) !== 0 && placed.hp > 0
    if (!on) {
      if (this.flashing.delete(id)) model.flash(null)
      return
    }
    this.flashing.add(id)
    const side = (Math.floor(frame.now / BEACON_SIDE_MS) + id) % 2 === 0 ? 0 : 1
    const at = model.flash(side)!
    this.world.lights.hold(at.x, at.y + LIFT, at.z, { color: COLOR, intensity: INTENSITY * (BY_DAY + (1 - BY_DAY) * this.night), radius: REACH })
    this.world.sounds.siren(id, [placed.x, placed.y, placed.z], [velocity[0], 0, velocity[1]])
  }

  end() {
    this.world.sounds.endSirens()
  }

  // Its model goes with the vehicle (`Vehicles`), lenses and all.
  remove(id: number) {
    this.flashing.delete(id)
    this.world.sounds.dropSiren(id)
  }
}
