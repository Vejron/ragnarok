import {
  GOLF_CHARGE_TICKS,
  GOLF_CURVE_FRAMES,
  GOLF_MISHIT_POWER,
  GOLF_MISS_FRAMES,
  GOLF_OVERSWING,
  GOLF_PURE_FRAMES,
  GOLF_REST_FRAMES,
  GOLF_RETURN_FRAMES,
} from './consts'

/**
 * The golf swing meter, a mirror of `shared::golf::Meter`: the server steps the same one through the
 * same input frames, so what the golfer sees on the bar is what the swing is scored as, however far
 * away the server is. The parity spec replays `golfMeter` in `tables.generated.json` through it.
 */
export type Stage =
  | { kind: 'idle' }
  | { kind: 'charging'; frames: number }
  | { kind: 'returning'; power: number; frames: number }
  | { kind: 'resting'; frames: number }

/** How the club met the ball: `power` of a full swing, `curve` from -1 (a hook, left) to 1 (a slice, right), doubled in an overswing. */
export interface Strike {
  power: number
  curve: number
  mishit: boolean
}

const f32 = Math.fround

/** The power `frames` frames of held fire charge. */
export const charge = (frames: number) => Math.min(f32(frames / GOLF_CHARGE_TICKS), f32(GOLF_OVERSWING))

/** Frames from letting go at `power` until the marker is on the line. */
export const lineFrames = (power: number) => Math.round(f32(power * GOLF_RETURN_FRAMES))

/** The strike of a swing let go at `power`, pressed again `error` frames after the marker reached the line. */
export function strike(power: number, error: number): Strike {
  const off = Math.abs(error)
  const side = Math.sign(error)
  if (off > GOLF_MISS_FRAMES) return { power: f32(power * f32(GOLF_MISHIT_POWER)), curve: side, mishit: true }
  const curve = f32(Math.min(1, f32(Math.max(0, off - GOLF_PURE_FRAMES) / GOLF_CURVE_FRAMES)) * side) + 0
  return { power, curve: power > 1 ? f32(2 * curve) : curve, mishit: false }
}

export class Meter {
  stage: Stage = { kind: 'idle' }
  /** Whether fire was held in the last frame: every click wants a fresh press. */
  held = false

  /** One input frame, with fire held or not: the strike, if this frame made one. */
  step(fire: boolean): Strike | null {
    const fresh = fire && !this.held
    this.held = fire
    const s = this.stage
    switch (s.kind) {
      case 'idle':
        if (fresh) this.stage = { kind: 'charging', frames: 0 }
        return null
      case 'charging': {
        // Let go, or held to the top of the overswing, where it goes by itself.
        const frames = fire ? s.frames + 1 : s.frames
        const power = charge(frames)
        this.stage = !fire || power >= f32(GOLF_OVERSWING) ? { kind: 'returning', power, frames: 0 } : { kind: 'charging', frames }
        return null
      }
      case 'returning': {
        const frames = s.frames + 1
        const error = frames - lineFrames(s.power)
        if (fresh || error > GOLF_MISS_FRAMES) {
          this.stage = { kind: 'resting', frames: 0 }
          return strike(s.power, error)
        }
        this.stage = { kind: 'returning', power: s.power, frames }
        return null
      }
      case 'resting':
        this.stage = s.frames + 1 >= GOLF_REST_FRAMES ? { kind: 'idle' } : { kind: 'resting', frames: s.frames + 1 }
        return null
    }
  }

  /** The power charged or let go at, and the marker, on the bar where the line is 0 and a full swing 1; null while it is not going. */
  reading(): { power: number; marker: number; returning: boolean } | null {
    const s = this.stage
    if (s.kind === 'charging') return { power: charge(s.frames), marker: charge(s.frames), returning: false }
    if (s.kind === 'returning') return { power: s.power, marker: s.power - s.frames / GOLF_RETURN_FRAMES, returning: true }
    return null
  }

  reset() {
    this.stage = { kind: 'idle' }
  }
}

/** What a strike is called, for the golfer to be told. */
export function called(strike: Strike): string {
  if (strike.mishit) return strike.curve < 0 ? 'Smothered' : 'Topped'
  const curve = strike.curve
  if (curve === 0) return strike.power > 1 ? 'Overswing — pure!' : 'Pure'
  const big = Math.abs(curve) >= 0.5
  if (curve < 0) return big ? 'Hook' : 'Draw'
  return big ? 'Slice' : 'Fade'
}

/** The bar's ends: the far side of the line where a late swing is a mishit, and the top of an overswing. */
export const METER_LOW = -(GOLF_MISS_FRAMES + 1) / GOLF_RETURN_FRAMES
export const METER_HIGH = GOLF_OVERSWING
/** On the bar, how far either side of the line a click is pure, curved, and fully curved. */
export const METER_PURE = (GOLF_PURE_FRAMES + 0.5) / GOLF_RETURN_FRAMES
export const METER_CURVE = (GOLF_PURE_FRAMES + GOLF_CURVE_FRAMES + 0.5) / GOLF_RETURN_FRAMES
export const METER_MISS = (GOLF_MISS_FRAMES + 0.5) / GOLF_RETURN_FRAMES
