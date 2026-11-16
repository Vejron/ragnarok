import { Quaternion, Vector3 } from '@ragnarok/engine'
import { DAMAGE_FRONT, FLAG_LAMP, KIND_HEN, KIND_LOITER, KIND_MISSILE, KIND_MORTAR, KIND_PAK, KIND_SHIP_GUN } from '@/net/codec'
import { CarModel } from '@/render/car'
import { HeliModel } from '@/render/heli'
import { TitanModel } from '@/render/titan'
import { LIGHTS_BREAK } from '@/render/carwear'
import { dayPhase, nightAt } from '@/render/daynight'
import { TICK_RATE } from '@/sim/consts'
import { halfExtentsOf } from '@/sim/kinds'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** Warm white, as old halogen lamps. */
const COLOR = 0xfff1d6
/** Strength at full dark (`PointLights` intensity). */
const INTENSITY = 5
/** How far a vehicle's headlights reach, metres. */
const REACH = 40
/** The headlights' half-angle, radians, and the share of it that softens toward the edge. */
const ANGLE = 0.5
const PENUMBRA = 0.5
/** A helicopter's searchlight: twice the headlights' reach, in a narrower beam. */
const SEARCH_REACH = 2 * REACH
const SEARCH_ANGLE = 0.25
const SEARCH_PENUMBRA = 0.4
/** A Titan's searchlight: cold white, from fifty metres up, so far-reaching, narrow and strong. */
const TITAN_COLOR = 0xeef4ff
const TITAN_INTENSITY = 9
const TITAN_REACH = 320
const TITAN_ANGLE = 0.17
const TITAN_PENUMBRA = 0.35
/** The share of its strength a lamp has by day, so one switched on at noon is seen to be on. */
const BY_DAY = 0.15
/** Seconds the lights take to come on or go off. */
const FADE = 0.25
/**
 * Where a vehicle without lamps of its own in its model shines from: the middle of the front of its hull,
 * this share of its half-height above its centre, just ahead of it.
 */
const HULL_LAMP_RISE = 0.3
const HULL_LAMP_AHEAD = 0.1
/** How far the beam is dipped below the nose: a rise over a run of 1. */
const HULL_LAMP_DIP = 0.08

/** What has no lamps: towed and fixed guns, munitions in flight and the hen. */
const UNLIT = new Set([KIND_MORTAR, KIND_PAK, KIND_SHIP_GUN, KIND_MISSILE, KIND_LOITER, KIND_HEN])

/**
 * A vehicle's lamps, which its driver switches (L, `FLAG_LAMP`), at full strength in the dark and
 * `BY_DAY` of it by day (`nightAt`), fading in and out over `FADE`:
 * - a car's headlights, from the front lamps in its model along the nose, until the lamps are smashed;
 * - a helicopter's searchlight, under the nose gun and turning with it;
 * - a Titan's searchlight, from its visor the way its head faces;
 * - anything else's headlights, from the middle of the front of its hull along the nose.
 *
 * They go out when the vehicle is wrecked, and when its driver leaves (the server's to say).
 */
export class Headlights implements Presenter<Frame> {
  /** How far on each vehicle's lights are, 0 to 1; one whose lights are off is not kept. */
  private readonly level = new Map<number, number>()
  private night = 0
  private readonly beam = { at: new Vector3(), direction: new Vector3() }
  private readonly turn = new Quaternion()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.orientation !== undefined && entity.kind !== undefined && !UNLIT.has(entity.kind.kind)
  }

  begin(frame: Frame) {
    this.night = nightAt(dayPhase(frame.tick, TICK_RATE))
  }

  update(id: number, sampled: Sampled, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!model || !placed) return
    const car = model instanceof CarModel
    const switched = ((sampled.entity.flags?.flags ?? 0) & FLAG_LAMP) !== 0
    const on = switched && placed.hp > 0 && !(car && (placed.damage[DAMAGE_FRONT] ?? 0) >= LIGHTS_BREAK)
    const target = on ? BY_DAY + (1 - BY_DAY) * this.night : 0
    const last = this.level.get(id) ?? 0
    const step = frame.dt / FADE
    const level = target > last ? Math.min(target, last + step) : Math.max(target, last - step)
    if (level <= 0) {
      this.level.delete(id)
      return
    }
    this.level.set(id, level)
    if (model instanceof TitanModel) {
      const { at, direction } = model.searchlight()
      this.world.lights.hold(at.x, at.y, at.z, { color: TITAN_COLOR, intensity: TITAN_INTENSITY * level, direction, radius: TITAN_REACH, angle: TITAN_ANGLE, penumbra: TITAN_PENUMBRA })
      return
    }
    if (model instanceof HeliModel) {
      const { at, direction } = model.searchlight()
      this.world.lights.hold(at.x, at.y, at.z, { color: COLOR, intensity: INTENSITY * level, direction, radius: SEARCH_REACH, angle: SEARCH_ANGLE, penumbra: SEARCH_PENUMBRA })
      return
    }
    const { at, direction } = (car ? model.headlight() : null) ?? this.hullLamp(placed.kind, placed)
    this.world.lights.hold(at.x, at.y, at.z, { color: COLOR, intensity: INTENSITY * level, direction, radius: REACH, angle: ANGLE, penumbra: PENUMBRA })
  }

  remove(id: number) {
    this.level.delete(id)
  }

  /** The beam of a vehicle of `kind` placed at `placed` from the front of its hull. The vectors are reused by the next call. */
  private hullLamp(kind: number, placed: { x: number; y: number; z: number; rotation: readonly [number, number, number, number] }) {
    const [, halfY, halfZ] = halfExtentsOf(kind)
    this.turn.set(...placed.rotation)
    this.beam.at.set(0, HULL_LAMP_RISE * halfY, -halfZ - HULL_LAMP_AHEAD).applyQuaternion(this.turn)
    this.beam.at.x += placed.x
    this.beam.at.y += placed.y
    this.beam.at.z += placed.z
    this.beam.direction.set(0, -HULL_LAMP_DIP, -1).normalize().applyQuaternion(this.turn)
    return this.beam
  }
}
