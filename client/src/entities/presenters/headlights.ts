import { DAMAGE_FRONT } from '@/net/codec'
import { CarModel } from '@/render/car'
import { HeliModel } from '@/render/heli'
import { TitanModel } from '@/render/titan'
import { LIGHTS_BREAK } from '@/render/carwear'
import { dayPhase, nightAt } from '@/render/daynight'
import { TICK_RATE } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** Warm white, as old halogen lamps. */
const COLOR = 0xfff1d6
/** Strength at full dark (`PointLights` intensity). */
const INTENSITY = 5
/** How far a car's headlights reach, metres. */
const REACH = 40
/** The headlights' half-angle, radians, and the share of it that softens toward the edge. */
const ANGLE = 0.5
const PENUMBRA = 0.5
/** A helicopter's searchlight: twice the headlights' reach, in a narrower beam. */
const SEARCH_REACH = 2 * REACH
const SEARCH_ANGLE = 0.25
const SEARCH_PENUMBRA = 0.4
/** A Titan's searchlight: cold white, from fifty metres up, so far-reaching, narrow and strong; and the share of its strength it has by day. */
const TITAN_COLOR = 0xeef4ff
const TITAN_INTENSITY = 9
const TITAN_REACH = 320
const TITAN_ANGLE = 0.17
const TITAN_PENUMBRA = 0.35
const TITAN_BY_DAY = 0.15
/** Seconds the lights take to come on or go off. */
const FADE = 0.25

/**
 * Lamps after dark, coming on with the dusk (`nightAt`) and fading in and out over `FADE`:
 * - the jeep's, sports car's and Roadster's headlights, from the front lamps along the nose while someone
 *   is in the driver's seat, until the lamps are smashed or the car is wrecked;
 * - a helicopter's searchlight, under the nose gun and turning with it, while anyone is aboard.
 *
 * A Titan's searchlight is its pilot's to switch (L), day or night: from its visor, the way its head faces.
 */
export class Headlights implements Presenter<Frame> {
  /** How far on each car's lights are, 0 to 1; a car whose lights are off is not kept. */
  private readonly level = new Map<number, number>()
  private night = 0

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.orientation !== undefined && entity.kind !== undefined
  }

  begin(frame: Frame) {
    this.night = nightAt(dayPhase(frame.tick, TICK_RATE))
  }

  update(id: number, _: unknown, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    const car = model instanceof CarModel
    const titan = model instanceof TitanModel
    if (!(car || titan || model instanceof HeliModel) || !placed) return
    const on = placed.hp > 0 && (titan ? model.searchlight() !== null : car ? placed.seats[0] !== 0 && (placed.damage[DAMAGE_FRONT] ?? 0) < LIGHTS_BREAK : placed.seats.some((seat) => seat !== 0))
    const target = !on ? 0 : titan ? TITAN_BY_DAY + (1 - TITAN_BY_DAY) * this.night : this.night
    const last = this.level.get(id) ?? 0
    const step = frame.dt / FADE
    const level = target > last ? Math.min(target, last + step) : Math.max(target, last - step)
    if (level <= 0) {
      this.level.delete(id)
      return
    }
    this.level.set(id, level)
    const beam = car ? model.headlight() : model.searchlight()
    if (!beam) return
    const { at, direction } = beam
    if (titan) {
      this.world.lights.hold(at.x, at.y, at.z, { color: TITAN_COLOR, intensity: TITAN_INTENSITY * level, direction, radius: TITAN_REACH, angle: TITAN_ANGLE, penumbra: TITAN_PENUMBRA })
      return
    }
    this.world.lights.hold(at.x, at.y, at.z, {
      color: COLOR, intensity: INTENSITY * level, direction,
      ...(car ? { radius: REACH, angle: ANGLE, penumbra: PENUMBRA } : { radius: SEARCH_REACH, angle: SEARCH_ANGLE, penumbra: SEARCH_PENUMBRA }),
    })
  }

  remove(id: number) {
    this.level.delete(id)
  }
}
