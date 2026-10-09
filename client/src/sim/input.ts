import { BUTTON_JUMP, BUTTON_RUN, BUTTON_USE } from './consts'

/** Mouse-look sensitivity, radians per pixel. */
const LOOK_SPEED = 0.0025
const MIN_PITCH = -1.2
const MAX_PITCH = 0.6

export interface InputSample {
  strafe: number
  forward: number
  buttons: number
  yaw: number
}

/**
 * Keyboard + pointer-lock mouse. The camera pitch is local only; the yaw is part of the input.
 * Buttons pressed between two ticks are latched so a quick tap is never missed.
 */
export class InputController {
  yaw = 0
  pitch = -0.25
  private held = new Set<string>()
  private latched = 0
  private readonly abort = new AbortController()

  constructor(private readonly element: HTMLElement) {
    const options = { signal: this.abort.signal }
    window.addEventListener('keydown', (event) => this.key(event, true), options)
    window.addEventListener('keyup', (event) => this.key(event, false), options)
    window.addEventListener('blur', () => this.held.clear(), options)
    element.addEventListener('click', () => void element.requestPointerLock?.(), options)
    document.addEventListener('mousemove', (event) => {
      if (document.pointerLockElement !== element) return
      this.yaw -= event.movementX * LOOK_SPEED
      this.pitch = Math.max(MIN_PITCH, Math.min(MAX_PITCH, this.pitch - event.movementY * LOOK_SPEED))
    }, options)
  }

  get locked() {
    return document.pointerLockElement === this.element
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
    if (this.held.has('ShiftLeft') || this.held.has('ShiftRight')) buttons |= BUTTON_RUN
    return {
      strafe: this.axis(['KeyA', 'ArrowLeft'], ['KeyD', 'ArrowRight']),
      forward: this.axis(['KeyS', 'ArrowDown'], ['KeyW', 'ArrowUp']),
      buttons,
      yaw: this.yaw,
    }
  }

  dispose() {
    this.abort.abort()
  }
}
