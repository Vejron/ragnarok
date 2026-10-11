import { wrapAngle } from '@/net/codec'
import { BUTTON_ALT, BUTTON_CROUCH, BUTTON_FIRE, BUTTON_JUMP, BUTTON_RELOAD, BUTTON_RUN, BUTTON_SLIDE, BUTTON_USE } from './consts'
import { HOTBAR_SLOTS } from './items'

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
  /** The hotbar slot in hand, 0..HOTBAR_SLOTS. */
  slot: number
}

/**
 * Keyboard + pointer-lock mouse, or on a touch device the on-screen controls, which feed it through
 * `setStick`, `lookBy`, `hold` and `tap`. The camera pitch is local only; the yaw is part of the input.
 * Buttons pressed between two ticks are latched so a quick tap is never missed.
 */
export class InputController {
  yaw = 0
  pitch = -0.25
  /** The hotbar slot in hand. Keys 1-5, the wheel and the on-screen slots change it. */
  selectedSlot = 0
  /** `performance.now()` of the last mouse movement, so the camera does not fight the player. */
  private lastLook = -Infinity
  private held = new Set<string>()
  private firing = false
  /** The right mouse button is down: aim, which looks down a scope when the gun has one. */
  private aiming = false
  /** Scales mouse-look; the game turns it down while scoped so a zoomed view is not twitchy. */
  lookScale = 1
  private latched = 0
  private readonly abort = new AbortController()
  /** The primary pointer is a finger: no pointer lock, no keyboard. */
  readonly coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
  /** A touch player has tapped to start, which stands in for taking the pointer lock. */
  private touching = false
  private stick: [number, number, boolean] = [0, 0, false]
  private touchHeld = 0

  constructor(private readonly element: HTMLElement) {
    const options = { signal: this.abort.signal }
    window.addEventListener('keydown', (event) => this.key(event, true), options)
    window.addEventListener('keyup', (event) => this.key(event, false), options)
    window.addEventListener('blur', () => {
      this.held.clear()
      this.touchHeld = 0
      this.stick = [0, 0, false]
      this.firing = false
      this.aiming = false
    }, options)
    element.addEventListener('click', () => {
      if (this.coarse) this.touching = true
      else void element.requestPointerLock?.()
    }, options)
    // The trigger is the left mouse button, once the pointer is captured.
    document.addEventListener('mousedown', (event) => {
      if (document.pointerLockElement !== element) return
      if (event.button === 0) this.firing = true
      if (event.button === 2) this.aiming = true
    }, options)
    document.addEventListener('mouseup', (event) => {
      if (event.button === 0) this.firing = false
      if (event.button === 2) this.aiming = false
    }, options)
    // The wheel cycles the hotbar, one slot per notch.
    document.addEventListener('wheel', (event) => {
      if (document.pointerLockElement !== element || event.deltaY === 0) return
      this.cycleSlot(event.deltaY > 0 ? 1 : -1)
    }, { ...options, passive: true })
    // The right button must not open a menu.
    document.addEventListener('contextmenu', (event) => event.preventDefault(), options)
    document.addEventListener('mousemove', (event) => {
      if (document.pointerLockElement !== element) return
      this.lastLook = performance.now()
      const speed = LOOK_SPEED * this.lookScale
      this.yaw -= event.movementX * speed
      this.pitch = Math.max(MIN_PITCH, Math.min(MAX_PITCH, this.pitch - event.movementY * speed))
    }, options)
  }

  /** Takes hotbar slot `slot` in hand. */
  selectSlot(slot: number) {
    if (Number.isInteger(slot) && slot >= 0 && slot < HOTBAR_SLOTS) this.selectedSlot = slot
  }

  private cycleSlot(by: number) {
    this.selectedSlot = (this.selectedSlot + by + HOTBAR_SLOTS) % HOTBAR_SLOTS
  }

  /** Whether `code` (a `KeyboardEvent.code`) is down right now. */
  isHeld(code: string) {
    return this.held.has(code)
  }

  /** Aim is held, by the right mouse button or on screen. */
  get aimHeld() {
    return this.aiming || (this.touchHeld & BUTTON_ALT) !== 0
  }

  get locked() {
    return this.touching || document.pointerLockElement === this.element
  }

  /** Virtual stick: `x` right and `y` up, each −1..1. `run` is the shift key. */
  setStick(x: number, y: number, run: boolean) {
    this.stick = [x, y, run]
  }

  /** Camera drag in pixels, as the mouse would give. */
  lookBy(dx: number, dy: number) {
    this.lastLook = performance.now()
    const speed = LOOK_SPEED * this.lookScale
    this.yaw -= dx * speed
    this.pitch = Math.max(MIN_PITCH, Math.min(MAX_PITCH, this.pitch - dy * speed))
  }

  /** A button held down on screen, like a key. */
  hold(mask: number, down: boolean) {
    this.touchHeld = down ? this.touchHeld | mask : this.touchHeld & ~mask
    if (down) this.latched |= mask | (mask === BUTTON_CROUCH && this.running() ? BUTTON_SLIDE : 0)
  }

  /** A button pressed once on screen. */
  tap(mask: number) {
    this.latched |= mask
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
    // Tab shows the scoreboard; it must not also move the browser's focus.
    if (event.code === 'Tab') event.preventDefault()
    if (event.repeat) return
    if (down) {
      this.held.add(event.code)
      if (event.code === 'Space') this.latched |= BUTTON_JUMP
      if (event.code === 'KeyE') this.latched |= BUTTON_USE
      if (event.code === 'KeyR') this.latched |= BUTTON_RELOAD
      // Digit1..Digit5 pick a hotbar slot.
      const digit = /^Digit([1-9])$/.exec(event.code)
      if (digit) this.selectSlot(Number(digit[1]) - 1)
      // Crouching while running is a slide.
      if ((event.code === 'KeyC' || event.code === 'ControlLeft') && this.running()) this.latched |= BUTTON_CROUCH | BUTTON_SLIDE
    } else {
      this.held.delete(event.code)
    }
  }

  private running() {
    return this.stick[2] || this.held.has('ShiftLeft') || this.held.has('ShiftRight')
  }

  private axis(negative: string[], positive: string[]) {
    return (positive.some((c) => this.held.has(c)) ? 1 : 0) - (negative.some((c) => this.held.has(c)) ? 1 : 0)
  }

  /** Reads the current intent and clears latched buttons. */
  sample(): InputSample {
    let buttons = this.latched | this.touchHeld
    this.latched = 0
    if (this.stick[2]) buttons |= BUTTON_RUN
    if (this.held.has('Space')) buttons |= BUTTON_JUMP
    if (this.firing || this.held.has('KeyF')) buttons |= BUTTON_FIRE
    if (this.aiming) buttons |= BUTTON_ALT
    if (this.held.has('KeyC') || this.held.has('ControlLeft')) buttons |= BUTTON_CROUCH
    if (this.held.has('ShiftLeft') || this.held.has('ShiftRight')) buttons |= BUTTON_RUN
    return {
      strafe: Math.max(-1, Math.min(1, this.axis(['KeyA', 'ArrowLeft'], ['KeyD', 'ArrowRight']) + this.stick[0])),
      forward: Math.max(-1, Math.min(1, this.axis(['KeyS', 'ArrowDown'], ['KeyW', 'ArrowUp']) + this.stick[1])),
      buttons,
      yaw: this.yaw,
      pitch: this.pitch,
      slot: this.selectedSlot,
    }
  }

  dispose() {
    this.abort.abort()
  }
}
