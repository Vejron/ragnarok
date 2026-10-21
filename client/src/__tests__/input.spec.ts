import { afterEach, describe, expect, it } from 'vitest'
import { InputController } from '@/sim/input'
import { BUTTON_ALT, BUTTON_CROUCH, BUTTON_FIRE, BUTTON_JUMP, BUTTON_PRONE, BUTTON_RELOAD, BUTTON_USE } from '@/sim/consts'

const press = (code: string) => window.dispatchEvent(new KeyboardEvent('keydown', { code }))
const release = (code: string) => window.dispatchEvent(new KeyboardEvent('keyup', { code }))

describe('InputController hotbar', () => {
  let input: InputController | null = null
  const make = () => (input = new InputController(document.createElement('div')))
  afterEach(() => input?.dispose())

  it('starts on the first slot and follows the number keys', () => {
    const i = make()
    expect(i.sample().slot).toBe(0)
    press('Digit3')
    expect(i.sample().slot).toBe(2)
    press('Digit5')
    expect(i.sample().slot).toBe(4)
    press('Digit1')
    expect(i.sample().slot).toBe(0)
  })

  it('ignores keys with no slot and slots out of range', () => {
    const i = make()
    press('Digit9')
    press('Digit0')
    expect(i.sample().slot).toBe(0)
    i.selectSlot(7)
    i.selectSlot(-1)
    i.selectSlot(1.5)
    expect(i.sample().slot).toBe(0)
    i.selectSlot(3)
    expect(i.sample().slot).toBe(3)
  })

  it('toggles prone with Z, and gets up on jump or crouch', () => {
    const i = make()
    press('KeyZ')
    release('KeyZ')
    expect(i.sample().buttons & BUTTON_PRONE).toBeTruthy()
    expect(i.sample().buttons & BUTTON_PRONE).toBeTruthy()
    press('KeyZ')
    release('KeyZ')
    expect(i.sample().buttons & BUTTON_PRONE).toBe(0)
    press('KeyZ')
    release('KeyZ')
    press('Space')
    release('Space')
    expect(i.sample().buttons & BUTTON_PRONE).toBe(0)
    press('KeyZ')
    release('KeyZ')
    press('KeyC')
    const crouched = i.sample().buttons
    expect(crouched & BUTTON_PRONE).toBe(0)
    expect(crouched & BUTTON_CROUCH).toBeTruthy()
    release('KeyC')
    press('KeyZ')
    release('KeyZ')
    i.clearProne()
    expect(i.sample().buttons & BUTTON_PRONE).toBe(0)
  })

  it('toggles prone from the on-screen button', () => {
    const i = make()
    i.tap(BUTTON_PRONE)
    expect(i.prone).toBe(true)
    expect(i.sample().buttons & BUTTON_PRONE).toBeTruthy()
    i.tap(BUTTON_PRONE)
    expect(i.prone).toBe(false)
    i.tap(BUTTON_PRONE)
    i.hold(BUTTON_JUMP, true)
    expect(i.prone).toBe(false)
  })

  it('sends reload and use for a single frame, and fire for as long as it is held', () => {
    const i = make()
    press('KeyR')
    press('KeyE')
    const first = i.sample().buttons
    expect(first & BUTTON_RELOAD).toBeTruthy()
    expect(first & BUTTON_USE).toBeTruthy()
    expect(i.sample().buttons & (BUTTON_RELOAD | BUTTON_USE)).toBe(0)
    press('KeyF')
    expect(i.sample().buttons & BUTTON_FIRE).toBeTruthy()
    expect(i.sample().buttons & BUTTON_FIRE).toBeTruthy()
    release('KeyF')
    expect(i.sample().buttons & BUTTON_FIRE).toBe(0)
  })
})

describe('InputController aim', () => {
  it('holds aim while the right button is down, only with the pointer captured', () => {
    const element = document.createElement('div')
    const i = new InputController(element)
    const mouse = (type: string, button: number) => document.dispatchEvent(new MouseEvent(type, { button }))
    mouse('mousedown', 2)
    expect(i.sample().buttons & BUTTON_ALT).toBe(0)
    Object.defineProperty(document, 'pointerLockElement', { value: element, configurable: true })
    mouse('mousedown', 2)
    expect(i.aimHeld).toBe(true)
    expect(i.sample().buttons & BUTTON_ALT).toBeTruthy()
    expect(i.sample().buttons & BUTTON_ALT).toBeTruthy()
    mouse('mouseup', 2)
    expect(i.aimHeld).toBe(false)
    expect(i.sample().buttons & BUTTON_ALT).toBe(0)
    Object.defineProperty(document, 'pointerLockElement', { value: null, configurable: true })
    i.dispose()
  })

  it('turns the camera more slowly while scoped', () => {
    const i = new InputController(document.createElement('div'))
    i.yaw = 0
    i.lookBy(100, 0)
    const full = i.yaw
    i.yaw = 0
    i.lookScale = 0.3
    i.lookBy(100, 0)
    expect(i.yaw / full).toBeCloseTo(0.3)
    i.dispose()
  })
})
