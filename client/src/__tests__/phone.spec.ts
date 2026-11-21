import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { InputController } from '@/sim/input'
import { BUTTON_FIRE } from '@/sim/consts'

const press = (code: string) => {
  window.dispatchEvent(new KeyboardEvent('keydown', { code }))
  window.dispatchEvent(new KeyboardEvent('keyup', { code }))
}

describe('the held phone', () => {
  let input: InputController | null = null
  let element: HTMLElement
  let holder: Element | null = null

  beforeEach(() => {
    holder = null
    element = document.createElement('div')
    Object.defineProperty(document, 'pointerLockElement', { configurable: true, get: () => holder })
    const change = () => document.dispatchEvent(new Event('pointerlockchange'))
    element.requestPointerLock = vi.fn(() => {
      holder = element
      change()
      return Promise.resolve()
    }) as typeof element.requestPointerLock
    document.exitPointerLock = vi.fn(() => {
      holder = null
      change()
    })
    input = new InputController(element)
    void element.requestPointerLock()
  })
  afterEach(() => input?.dispose())

  it('M brings it up on the map and T on the deliveries, freeing the mouse without pausing', () => {
    press('KeyM')
    expect(input!.phone).toBe('map')
    expect(document.exitPointerLock).toHaveBeenCalled()
    expect(input!.locked).toBe(true)
    press('KeyT')
    expect(input!.phone).toBe('jobs')
  })

  it('the same key again puts it away and takes the mouse back', () => {
    press('KeyT')
    press('KeyT')
    expect(input!.phone).toBeNull()
    expect(holder).toBe(element)
  })

  it('the keys still walk while it is up, and nothing fires', () => {
    press('KeyM')
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyW' }))
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyF' }))
    const sample = input!.sample()
    expect(sample.forward).toBeGreaterThan(0)
    expect(sample.buttons & BUTTON_FIRE).toBe(0)
  })

  it('Esc puts it away and pauses', () => {
    press('KeyM')
    press('Escape')
    expect(input!.phone).toBeNull()
    expect(input!.locked).toBe(false)
  })

  it('does not come up from the pause screen', () => {
    document.exitPointerLock()
    press('KeyM')
    expect(input!.phone).toBeNull()
  })

  it('goes away when the game takes the mouse back', () => {
    press('KeyM')
    void element.requestPointerLock()
    expect(input!.phone).toBeNull()
  })

  it('is dropped without taking the mouse back, as when its holder dies', () => {
    press('KeyM')
    input!.dropPhone()
    expect(input!.phone).toBeNull()
    expect(holder).toBeNull()
  })
})
