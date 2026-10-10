import { afterEach, describe, expect, it } from 'vitest'
import { InputController } from '@/sim/input'
import { BUTTON_FIRE, BUTTON_RELOAD, BUTTON_USE } from '@/sim/consts'

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
