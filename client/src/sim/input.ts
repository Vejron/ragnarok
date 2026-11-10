import { wrapAngle } from '@/net/codec'
import { BUTTON_ALT, BUTTON_AMMO, BUTTON_CHUTE, BUTTON_CROUCH, BUTTON_FIRE, BUTTON_HITCH, BUTTON_JUMP, BUTTON_LAMP, BUTTON_PRONE, BUTTON_RANGE_DOWN, BUTTON_RANGE_UP, BUTTON_RELOAD, BUTTON_RUN, BUTTON_SKIS, BUTTON_SLIDE, BUTTON_USE } from './consts'
import { followGravity, GYRO_GAIN, gyroTurn, type DeviceVector, type GyroMode } from './gyro'
import { HOTBAR_SLOTS, SLOT_NONE } from './items'

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
  /** The gun is slung (X): the hands are free, and `selectedSlot` waits to come back. */
  holstered = false
  /** `performance.now()` of the last mouse movement, so the camera does not fight the player. */
  private lastLook = -Infinity
  private held = new Set<string>()
  private firing = false
  /** The right mouse button is down: aim, which looks down a scope when the gun has one. */
  private aiming = false
  /** Scales mouse-look; the game turns it down while scoped so a zoomed view is not twitchy. */
  lookScale = 1
  /**
   * While a field mortar's gunner lays it from above (aim held), the tube's elevation sent in place of the
   * look's pitch, radians. The look's pitch is held within what a view can tilt, so it cannot carry it.
   */
  layPitch: number | null = null
  private latched = 0
  private readonly abort = new AbortController()
  /** The primary pointer is a finger: no pointer lock, no keyboard. */
  readonly coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
  /** A touch player has tapped to start, which stands in for taking the pointer lock. */
  private touching = false
  private stick: [number, number, boolean] = [0, 0, false]
  private touchHeld = 0
  /** Lying down: Z toggles it, and jumping or crouching gets up. */
  private proneLatched = false
  /** The paper map is held up to read: M toggles it; firing or aiming puts it away. */
  mapUp = false
  /** Wheel notches turned with the map up, not yet taken (`takeMapWheel`): down zooms out. */
  private mapWheel = 0
  /** F3 was pressed since last asked (`takeStatsToggle`): show or hide the HUD's network numbers. */
  private statsToggle = false
  /** The click that put the map away fires nothing: its buttons are ignored until let go. */
  private swallow = false
  /** When turning the phone turns the view (GYRO-AIM.md). Off until the player picks a mode. */
  gyroMode: GyroMode = 'off'
  /** The player's gyro sensitivity, a multiple of `GYRO_GAIN`. */
  gyroSensitivity = 1
  /** `timeStamp` of the last motion reading that turned the view, ms; null when the next one starts afresh. */
  private lastMotion: number | null = null
  /** Which way is up in the phone's frame, smoothed from the accelerometer, if it reports gravity. */
  private gravity: DeviceVector | null = null

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
      this.lastMotion = null
    }, options)
    element.addEventListener('click', () => {
      if (this.coarse) {
        this.touching = true
        // The tap is the gesture iOS needs before it hands over the gyroscope, once per page load.
        if (this.gyroMode !== 'off') {
          void this.enableMotion().then((granted) => {
            if (!granted) this.gyroMode = 'off'
          })
        }
      } else void element.requestPointerLock?.()
    }, options)
    window.addEventListener('devicemotion', (event) => this.motion(event), options)
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
    // The wheel cycles the hotbar, one slot per notch; with the map up it zooms the map.
    document.addEventListener('wheel', (event) => {
      if (document.pointerLockElement !== element || event.deltaY === 0) return
      if (this.mapUp) this.mapWheel += event.deltaY > 0 ? 1 : -1
      else this.cycleSlot(event.deltaY > 0 ? 1 : -1)
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
    if (Number.isInteger(slot) && slot >= 0 && slot < HOTBAR_SLOTS) {
      this.selectedSlot = slot
      this.holstered = false
    }
  }

  private cycleSlot(by: number) {
    this.selectedSlot = (this.selectedSlot + by + HOTBAR_SLOTS) % HOTBAR_SLOTS
    this.holstered = false
  }

  /** Slings what is in hand, or takes it back. */
  holster() {
    this.holstered = !this.holstered
  }

  /** What the hands hold: a hotbar slot, or `SLOT_NONE`. */
  get heldSlot() {
    return this.holstered ? SLOT_NONE : this.selectedSlot
  }

  /** Whether `code` (a `KeyboardEvent.code`) is down right now. */
  isHeld(code: string) {
    return this.held.has(code)
  }

  /** Push-to-talk held on screen; V on a keyboard. */
  touchTalk = false

  /** Talking: V held, or the touch button. */
  get talkHeld() {
    return this.touchTalk || this.held.has('KeyV')
  }

  /** Lets go of every button held on screen: for when the touch controls change or go away under a finger. */
  releaseTouch() {
    this.touchHeld = 0
    this.touchTalk = false
    this.stick = [0, 0, false]
  }

  /** Aim is held, by the right mouse button or on screen. */
  get aimHeld() {
    return !this.swallow && (this.aiming || (this.touchHeld & BUTTON_ALT) !== 0)
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

  /** A turn of the view in radians, + yaw to the left and + pitch up, scaled as the mouse is while aiming. */
  turnBy(yaw: number, pitch: number) {
    this.lastLook = performance.now()
    this.yaw += yaw * this.lookScale
    this.pitch = Math.max(MIN_PITCH, Math.min(MAX_PITCH, this.pitch + pitch * this.lookScale))
  }

  /** Whether the gyroscope steers the view right now, by the mode and what the player is doing. */
  get gyroActive() {
    if (this.gyroMode === 'off' || !this.locked || this.mapUp) return false
    return this.gyroMode === 'always' || this.aimHeld
  }

  /**
   * Asks for the motion sensors where the browser wants asking (iOS), which must be from a tap or click.
   * Resolves false if the player said no; elsewhere there is nothing to ask and it resolves true.
   */
  async enableMotion(): Promise<boolean> {
    const ask = (globalThis.DeviceMotionEvent as { requestPermission?: () => Promise<'granted' | 'denied'> } | undefined)?.requestPermission
    if (typeof ask !== 'function') return true
    try {
      return (await ask.call(DeviceMotionEvent)) === 'granted'
    } catch {
      // Not from a gesture, or no sensors at all.
      return false
    }
  }

  /** One reading of the motion sensors: turns the view by how far the phone turned since the last. */
  private motion(event: DeviceMotionEvent) {
    const g = event.accelerationIncludingGravity
    if (g && g.x !== null && g.y !== null && g.z !== null) this.gravity = followGravity(this.gravity, [g.x, g.y, g.z])
    const rate = event.rotationRate
    if (!this.gyroActive || !rate) {
      this.lastMotion = null
      return
    }
    const last = this.lastMotion
    this.lastMotion = event.timeStamp
    if (last === null) return
    const angle = screen.orientation?.angle ?? (window as { orientation?: number }).orientation ?? 0
    const turn = gyroTurn(rate, angle, (event.timeStamp - last) / 1000, this.gravity)
    if (turn.yaw === 0 && turn.pitch === 0) return
    const gain = GYRO_GAIN * this.gyroSensitivity
    this.turnBy(turn.yaw * gain, turn.pitch * gain)
  }

  /** A button held down on screen, like a key. */
  hold(mask: number, down: boolean) {
    this.touchHeld = down ? this.touchHeld | mask : this.touchHeld & ~mask
    if (down && mask & (BUTTON_JUMP | BUTTON_CROUCH)) this.proneLatched = false
    if (down) this.latched |= mask | (mask === BUTTON_CROUCH && this.running() ? BUTTON_SLIDE : 0)
  }

  /** A button pressed once on screen. Prone is a toggle, as Z is. */
  tap(mask: number) {
    if (mask === BUTTON_PRONE) this.proneLatched = !this.proneLatched
    else {
      if (mask & (BUTTON_JUMP | BUTTON_CROUCH)) this.proneLatched = false
      this.latched |= mask
    }
  }

  /** Whether the player has asked to lie down. */
  get prone() {
    return this.proneLatched
  }

  /** Gets up: dying or taking a seat should not leave the player lying down afterwards. */
  clearProne() {
    this.proneLatched = false
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
    if (event.code === 'Tab' || event.code === 'F3') event.preventDefault()
    if (event.repeat) return
    if (down) {
      this.held.add(event.code)
      if (event.code === 'Space') this.latched |= BUTTON_JUMP
      // Z lies down and gets up again; jumping or crouching also gets up.
      if (event.code === 'KeyZ') this.proneLatched = !this.proneLatched
      if (event.code === 'Space' || event.code === 'KeyC' || event.code === 'ControlLeft') this.proneLatched = false
      if (event.code === 'KeyE') this.latched |= BUTTON_USE
      if (event.code === 'KeyH') this.latched |= BUTTON_HITCH
      if (event.code === 'KeyG') this.latched |= BUTTON_CHUTE
      if (event.code === 'KeyL') this.latched |= BUTTON_LAMP
      if (event.code === 'KeyK') this.latched |= BUTTON_SKIS
      if (event.code === 'KeyX') this.holster()
      if (event.code === 'KeyM') this.mapUp = !this.mapUp
      if (event.code === 'F3') this.statsToggle = true
      if (event.code === 'KeyR') this.latched |= BUTTON_RELOAD
      // Q loads a gun's other kind of round: a tank's high explosive, or back to armour-piercing.
      if (event.code === 'KeyQ') this.latched |= BUTTON_AMMO
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

  /** Whether F3 was pressed since last asked. */
  takeStatsToggle() {
    const toggled = this.statsToggle
    this.statsToggle = false
    return toggled
  }

  /** The wheel notches turned over the map since last asked. */
  takeMapWheel() {
    const notches = this.mapWheel
    this.mapWheel = 0
    return notches
  }

  /** Puts the map away, as firing or aiming does. */
  lowerMap() {
    this.mapUp = false
  }

  /** Reads the current intent and clears latched buttons. */
  sample(): InputSample {
    let buttons = this.latched | this.touchHeld
    this.latched = 0
    if (this.stick[2]) buttons |= BUTTON_RUN
    if (this.held.has('Space')) buttons |= BUTTON_JUMP
    if (this.firing || this.held.has('KeyF')) buttons |= BUTTON_FIRE
    if (this.aiming) buttons |= BUTTON_ALT
    // Firing or aiming with the map up puts it away, and that press does nothing else.
    const trigger = buttons & (BUTTON_FIRE | BUTTON_ALT)
    if (this.mapUp && trigger) {
      this.mapUp = false
      this.swallow = true
    }
    if (this.swallow) {
      if (trigger) buttons &= ~(BUTTON_FIRE | BUTTON_ALT)
      else this.swallow = false
    }
    if (this.held.has('KeyC') || this.held.has('ControlLeft')) buttons |= BUTTON_CROUCH
    if (this.proneLatched) buttons |= BUTTON_PRONE
    // W and S (or the stick pushed up or down) walk a mortar's range dial; nobody on foot or driving needs them for anything else here.
    if (this.held.has('KeyW') || this.stick[1] > 0.5) buttons |= BUTTON_RANGE_UP
    if (this.held.has('KeyS') || this.stick[1] < -0.5) buttons |= BUTTON_RANGE_DOWN
    if (this.held.has('ShiftLeft') || this.held.has('ShiftRight')) buttons |= BUTTON_RUN
    return {
      strafe: Math.max(-1, Math.min(1, this.axis(['KeyA', 'ArrowLeft'], ['KeyD', 'ArrowRight']) + this.stick[0])),
      forward: Math.max(-1, Math.min(1, this.axis(['KeyS', 'ArrowDown'], ['KeyW', 'ArrowUp']) + this.stick[1])),
      buttons,
      yaw: this.yaw,
      pitch: this.layPitch ?? this.pitch,
      slot: this.heldSlot,
    }
  }

  dispose() {
    this.abort.abort()
  }
}
