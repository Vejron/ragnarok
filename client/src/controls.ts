import type { GyroMode } from './sim/gyro'

/** How the player likes to steer: the touch controls' options, kept apart from the graphics' `Look`. */
export interface Controls {
  /** When turning the phone turns the view (GYRO-AIM.md). */
  gyro: GyroMode
  /** Gyro sensitivity, a multiple of the standard gain, `GYRO_SENSITIVITY_MIN..MAX`. */
  gyroSensitivity: number
}

/** The slowest gyro sensitivity the settings offer. */
export const GYRO_SENSITIVITY_MIN = 0.25
/** The quickest gyro sensitivity the settings offer. */
export const GYRO_SENSITIVITY_MAX = 3

const KEY = 'ragnarok.controls'
const MODES: GyroMode[] = ['off', 'always', 'aiming']

/** Gyro aim is off until the player asks for it: on iOS it needs a permission, and a tilt nobody expected is a surprise. */
export function defaultControls(): Controls {
  return { gyro: 'off', gyroSensitivity: 1 }
}

export function loadControls(): Controls {
  const controls = defaultControls()
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Controls> | null
    if (saved && MODES.includes(saved.gyro as GyroMode)) controls.gyro = saved.gyro as GyroMode
    if (typeof saved?.gyroSensitivity === 'number' && Number.isFinite(saved.gyroSensitivity)) {
      controls.gyroSensitivity = Math.max(GYRO_SENSITIVITY_MIN, Math.min(GYRO_SENSITIVITY_MAX, saved.gyroSensitivity))
    }
  } catch {
    // No storage, or junk in it: the defaults stand.
  }
  return controls
}

export function saveControls(controls: Controls) {
  try {
    localStorage.setItem(KEY, JSON.stringify(controls))
  } catch {
    // Private mode and the like: the choice just lasts the session.
  }
}
