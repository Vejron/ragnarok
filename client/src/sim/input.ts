import { wrapAngle } from '@/net/codec'
import { BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN, BUTTON_USE } from './consts'

/** Mouse-look sensitivity, radians per pixel. */
const LOOK_SPEED = 0.0025
const MIN_PITCH = -1.2
const MAX_PITCH = 0.6
/** After moving the mouse, the camera stays where it is for this long. */
const LOOK_HOLD_MS = 1500
/** How quickly the camera swings behind a car, per second. */
const FOLLOW_RATE = 2

export interface InputSample {
  strafe: number
  forward: number
  buttons: number
  yaw: number
  /** Look elevation, radians; + is up. */
  pitch: number
}

/**
 * Keyboard + pointer-lock mouse. The camera pitch is local only; the yaw is part of the input.
 * Buttons pressed between two ticks are latched so a quick tap is never missed.
 */
export class InputController {
  yaw = 0
  pitch = -0.25
  /** `performance.now()` of the last mouse movement, so the camera does not fight the player. */
  private lastLook = -Infinity
  private held = new Set<string>()
  private firing = false
  private latched = 0
  private readonly abort = new AbortController()

  constructor(private readonly element: HTMLElement) {
    const options = { signal: this.abort.signal }
    window.addEventListener('keydown', (event) => this.key(event, true), options)
    window.addEventListener('keyup', (event) => this.key(event, false), options)
    window.addEventListener('blur', () => {
      this.held.clear()
      this.firing = false
    }, options)
    element.addEventListener('click', () => void element.requestPointerLock?.(), options)
    // The trigger is the left mouse button, once the pointer is captured.
    document.addEventListener('mousedown', (event) => {
      if (event.button === 0 && document.pointerLockElement === element) this.firing = true
    }, options)
    document.addEventListener('mouseup', (event) => {
      if (event.button === 0) this.firing = false
    }, options)
    document.addEventListener('mousemove', (event) => {
      if (document.pointerLockElement !== element) return
      this.lastLook = performance.now()
      this.yaw -= event.movementX * LOOK_SPEED
      this.pitch = Math.max(MIN_PITCH, Math.min(MAX_PITCH, this.pitch - event.movementY * LOOK_SPEED))
    }, options)
  }

  get locked() {
    return document.pointerLockElement === this.element
  }

  /** Eases the camera yaw towards `target` (e.g. behind a car) unless the mouse moved recently. */
  followYaw(target: number, dtSeconds: number) {
    if (performance.now() - this.lastLook < LOOK_HOLD_MS) return
    this.yaw += wrapAngle(target - this.yaw) * Math.min(1, dtSeconds * FOLLOW_RATE)
  }

  /** Keeps the look elevation within `[min, max]`, e.g. a gun's travel. */
  clampPitch(min: number, max: number) {
    this.pitch = Math.max(min, Math.min(max, this.pitch))
  }

  private key(event: KeyboardEvent, down: boolean) {
    if (event.repeat) return
    if (down) {
      this.held.add(event.code)
      if (event.code === 'Space') this.latched |= BUTTON_JUMP
      if (event.code === 'KeyE') this.latched |= BUTTON_USE
    } else {
      this.held.delete(event.code)
    }
  }

  private axis(negative: string[], positive: string[]) {
    return (positive.some((c) => this.held.has(c)) ? 1 : 0) - (negative.some((c) => this.held.has(c)) ? 1 : 0)
  }

  /** Reads the current intent and clears latched buttons. */
  sample(): InputSample {
    let buttons = this.latched
    this.latched = 0
    if (this.held.has('Space')) buttons |= BUTTON_JUMP
    if (this.firing || this.held.has('KeyF')) buttons |= BUTTON_FIRE
    if (this.held.has('ShiftLeft') || this.held.has('ShiftRight')) buttons |= BUTTON_RUN
    return {
      strafe: this.axis(['KeyA', 'ArrowLeft'], ['KeyD', 'ArrowRight']),
      forward: this.axis(['KeyS', 'ArrowDown'], ['KeyW', 'ArrowUp']),
      buttons,
      yaw: this.yaw,
      pitch: this.pitch,
    }
  }

  dispose() {
    this.abort.abort()
  }
}
