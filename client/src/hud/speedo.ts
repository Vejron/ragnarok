import type { HudStats } from '@/hud'

/** The dial's sweep: from 225° (bottom left) clockwise through the top to -45° (bottom right), as on a car. */
export const DIAL_START = (-225 * Math.PI) / 180
export const DIAL_SWEEP = (270 * Math.PI) / 180

/** The top of the dial for a seat, km/h: what that kind of thing gets up to, rounded to a tick. */
export function dialMax(seat: HudStats['seat']): number {
  switch (seat) {
    case 'aviator':
      return 400
    case 'pilot':
      return 240
    case 'drone':
    case 'loiter':
      return 160
    case 'missile':
      return 280
    case 'captain':
      return 40
    case 'hen':
    case 'tupp':
    case 'walker':
    case 'mortar':
      return 60
    case null:
      // On skis.
      return 80
    default:
      return 160
  }
}

/** The dial's top for `kmh` on `seat`: its usual top, raised in steps of 40 when outrun. */
export function dialTop(seat: HudStats['seat'], kmh: number): number {
  const usual = dialMax(seat)
  return kmh <= usual ? usual : Math.ceil(kmh / 40) * 40
}

/** The needle's angle (canvas radians: 0 east, clockwise) for `kmh` on a dial to `top`. */
export function needleAngle(kmh: number, top: number): number {
  const t = Math.max(0, Math.min(1, kmh / top))
  return DIAL_START + t * DIAL_SWEEP
}

/** The ticks of a dial to `top`: every `step` km/h, with a number every other one. */
export function dialTicks(top: number): { kmh: number; major: boolean }[] {
  const step = top <= 80 ? 10 : top <= 200 ? 20 : 50
  const ticks: { kmh: number; major: boolean }[] = []
  for (let k = 0, v = 0; v <= top + 1e-6; k++, v += step) ticks.push({ kmh: v, major: k % 2 === 0 })
  return ticks
}

/** How full a bar of `value` out of `max` is, 0..1. */
export const fill = (value: number, max: number) => (max > 0 ? Math.max(0, Math.min(1, value / max)) : 0)
