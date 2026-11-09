/**
 * Gyro aim: turning the phone turns the view (GYRO-AIM.md). The sums are kept apart from the input
 * so they can be tested without a phone.
 */

/** How the gyroscope steers the view: never, whenever playing, or only while AIM is held. */
export type GyroMode = 'off' | 'always' | 'aiming'

/** The modes as the settings show them. */
export const GYRO_MODES: { key: GyroMode; label: string; about: string }[] = [
  { key: 'off', label: 'Off', about: 'Drag the right side of the screen to look.' },
  { key: 'aiming', label: 'While aiming', about: 'Turn the phone to fine-tune your aim while AIM is held.' },
  { key: 'always', label: 'Always', about: 'Turning the phone always turns the view, on top of dragging.' },
]

/** View turned per phone turned at sensitivity 1: a little more than one to one, so a small wrist turn covers a doorway. */
export const GYRO_GAIN = 1.5
/** Turn rates slower than this, degrees per second, are a hand holding still and are ignored. */
export const GYRO_DEADZONE = 0.6
/** The longest gap between two motion readings that is integrated, seconds; a stall or a hidden tab counts as no time. */
export const GYRO_MAX_DT = 0.1
/** How quickly the smoothed gravity follows the accelerometer, per reading, 0..1. */
const GRAVITY_FOLLOW = 0.1

/**
 * A `DeviceMotionEvent`'s `rotationRate`, degrees per second. Browsers give `alpha` about the device's
 * x axis, `beta` about y and `gamma` about z (out of the screen), whatever older wording of the spec says.
 */
export interface RotationRate {
  alpha: number | null
  beta: number | null
  gamma: number | null
}

/** A direction in the device's own frame: x to the right and y up the screen in portrait, z out of the screen. */
export type DeviceVector = [number, number, number]

/**
 * The view's turn, radians, over `dt` seconds of `rate`, with the screen rotated `angle` degrees
 * (`screen.orientation.angle`). `up` is which way is up in the device's frame, from gravity, if known.
 *
 * Yaw (+ to the left, as the game's yaw) is the phone's turn about the world's vertical, so a phone
 * held tilted back still turns the way the player turns. Without gravity the screen's up stands in.
 * Pitch (+ up) is the turn about the screen's right-hand edge, so tipping the top of the screen away
 * raises the view.
 */
export function gyroTurn(rate: RotationRate, angle: number, dt: number, up: DeviceVector | null = null): { yaw: number; pitch: number } {
  const step = Math.min(Math.max(dt, 0), GYRO_MAX_DT)
  const theta = (angle * Math.PI) / 180
  const sin = Math.sin(theta)
  const cos = Math.cos(theta)
  // The screen's axes in the device's frame, for the way the page is turned.
  const right: DeviceVector = [cos, -sin, 0]
  const screenUp: DeviceVector = [sin, cos, 0]
  const omega: DeviceVector = [rate.alpha ?? 0, rate.beta ?? 0, rate.gamma ?? 0]
  const vertical = up ? uprightFor(up, screenUp) : screenUp
  const yawRate = dot(omega, vertical)
  const pitchRate = dot(omega, right)
  const rad = Math.PI / 180
  return {
    yaw: Math.abs(yawRate) < GYRO_DEADZONE ? 0 : yawRate * rad * step,
    pitch: Math.abs(pitchRate) < GYRO_DEADZONE ? 0 : pitchRate * rad * step,
  }
}

/**
 * `gravity` made into a unit up vector. Browsers disagree on the accelerometer's sign, so it is
 * turned to face the player's side of the phone: someone playing holds the screen facing them and
 * its top above its bottom, so up never points behind the screen and down its face at once.
 */
function uprightFor(gravity: DeviceVector, screenUp: DeviceVector): DeviceVector {
  const length = Math.hypot(gravity[0], gravity[1], gravity[2])
  if (length < 1e-3) return screenUp
  const sign = dot(gravity, screenUp) + gravity[2] < 0 ? -1 : 1
  return [(gravity[0] * sign) / length, (gravity[1] * sign) / length, (gravity[2] * sign) / length]
}

/** `smoothed` moved a little towards `reading`, so a jolt does not swing which way is up. */
export function followGravity(smoothed: DeviceVector | null, reading: DeviceVector): DeviceVector {
  if (!smoothed) return reading
  return [
    smoothed[0] + (reading[0] - smoothed[0]) * GRAVITY_FOLLOW,
    smoothed[1] + (reading[1] - smoothed[1]) * GRAVITY_FOLLOW,
    smoothed[2] + (reading[2] - smoothed[2]) * GRAVITY_FOLLOW,
  ]
}

const dot = (a: DeviceVector, b: DeviceVector) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
