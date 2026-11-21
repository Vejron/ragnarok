/**
 * The mouse (or a finger) on the held phone (PHONE.md). While the phone is up the pointer is free;
 * where it falls on the page is turned into the phone's own pixels through where its screen is drawn
 * (`HeldPhoneView.corners`), and handed to the screen as a press, a drag or the wheel. A press off the
 * phone puts it away and takes the pointer back to play.
 */

import type { InputController } from '@/sim/input'
import type { ScreenCorners } from '@/render/heldphone'
import { quadToUnit } from './homography'
import type { PhoneScreen } from './screen'
import { SCREEN_H, SCREEN_W } from './ui'

/** Where on the phone's screen, in its pixels, page fraction `at` falls; null off it. */
export function onScreen(corners: ScreenCorners | null, at: [number, number]): [number, number] | null {
  if (!corners) return null
  const uv = quadToUnit(corners, at)
  if (!uv || uv[0] < 0 || uv[0] > 1 || uv[1] < 0 || uv[1] > 1) return null
  return [uv[0] * SCREEN_W, uv[1] * SCREEN_H]
}

export class PhonePointer {
  private readonly abort = new AbortController()
  private down = false

  constructor(element: HTMLElement, input: InputController, corners: () => ScreenCorners | null, screen: PhoneScreen) {
    const options = { signal: this.abort.signal }
    const where = (event: PointerEvent | WheelEvent) => {
      const rect = element.getBoundingClientRect()
      return onScreen(corners(), [(event.clientX - rect.left) / rect.width, (event.clientY - rect.top) / rect.height])
    }
    const free = () => input.phone !== null && document.pointerLockElement !== element
    element.addEventListener('pointerdown', (event) => {
      if (!free() || event.button !== 0) return
      const at = where(event)
      if (!at) {
        // Off the phone: back to play. The phone may still be swinging up, so only once it is there.
        if (corners()) input.putPhoneAway()
        return
      }
      this.down = true
      element.setPointerCapture?.(event.pointerId)
      screen.pointerDown(at[0], at[1])
    }, options)
    element.addEventListener('pointermove', (event) => {
      if (!this.down || !free()) return
      const rect = element.getBoundingClientRect()
      const uv = corners() && quadToUnit(corners()!, [(event.clientX - rect.left) / rect.width, (event.clientY - rect.top) / rect.height])
      // A drag keeps going past the screen's edge.
      if (uv) screen.pointerMove(uv[0] * SCREEN_W, uv[1] * SCREEN_H)
    }, options)
    const up = (event: PointerEvent) => {
      if (!this.down) return
      this.down = false
      const at = where(event)
      if (at) screen.pointerUp(at[0], at[1])
      else screen.pointerUp(-1, -1)
    }
    element.addEventListener('pointerup', up, options)
    element.addEventListener('pointercancel', up, options)
    element.addEventListener('wheel', (event) => {
      if (!free() || event.deltaY === 0 || !where(event)) return
      screen.wheel(event.deltaY > 0 ? 1 : -1)
    }, { ...options, passive: true })
  }

  dispose() {
    this.abort.abort()
  }
}
