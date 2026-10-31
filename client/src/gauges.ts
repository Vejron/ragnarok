/** What a plane's crew sees: the gauges on the HUD, and the pilot's two markers on the screen. */

import { Vector3 } from '@ragnarok/engine'
import type { Arms, Pintle } from '@/entities/units'
import type { HudStats } from '@/hud'
import { ARMS_JAMMED, ARMS_REAR_JAMMED, PINTLE_JAMMED } from '@/net/codec'
import type { PlaneModel } from '@/render/plane'
import type { VehicleFx } from '@/render/tyres'
import { PLANE_STALL_SPEED } from '@/sim/consts'

/** A sight's position on screen, kept off its very edges. */
const clamp01 = (value: number) => Math.min(0.94, Math.max(0.06, value))

/**
 * The gauges of a seat with a plane's `arms`: the pilot's guns' heat, the bombs and the stall warning,
 * or the rear gunner's heat.
 */
export function cockpit(arms: Arms | null, pilot: boolean, motion: VehicleFx | undefined): Pick<HudStats, 'heat' | 'jammed' | 'bombs' | 'stall'> {
  if (!arms) return { heat: null, jammed: false, bombs: null, stall: false }
  if (!pilot) return { heat: arms.rear_heat, jammed: (arms.flags & ARMS_REAR_JAMMED) !== 0, bombs: null, stall: false }
  const stall = motion !== undefined && !motion.grounded && motion.forwardSpeed < PLANE_STALL_SPEED * 1.1
  return { heat: arms.heat, jammed: (arms.flags & ARMS_JAMMED) !== 0, bombs: arms.bombs, stall }
}

/** What a tank's machine gunner sees on the HUD: their gun's heat and whether it has jammed. */
export function roofGun(pintle: Pintle | null): Pick<HudStats, 'heat' | 'jammed' | 'bombs' | 'stall'> {
  return { heat: pintle?.heat ?? null, jammed: ((pintle?.flags ?? 0) & PINTLE_JAMMED) !== 0, bombs: null, stall: false }
}

/**
 * Where the pilot's two markers are on the screen, as fractions of its width and height: the nose,
 * which is where the guns point a little way out, and the aim, the heading the mouse asks for (`yaw`,
 * `pitch`). Given every frame, apart from the HUD stats, which are throttled, so they move smoothly.
 */
export function pilotSights(camera: { project(point: Vector3): unknown }, plane: PlaneModel, yaw: number, pitch: number): { nose: [number, number] | null; aim: [number, number] | null } {
  const toScreen = (point: Vector3): [number, number] | null => {
    // Kept on the screen: the view may be a way behind the aim.
    if (!camera.project(point)) return null
    return [clamp01((point.x + 1) / 2), clamp01((1 - point.y) / 2)]
  }
  const { origin, direction } = plane.forwardGun(0)
  const flat = Math.cos(pitch)
  const wanted = new Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat)
  return {
    nose: toScreen(origin.clone().addScaledVector(direction, 150)),
    aim: toScreen(new Vector3().copy(plane.root.position).addScaledVector(wanted, 150)),
  }
}
