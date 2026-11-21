/** What the phone's apps draw with (PHONE.md): its size, colours, text and the places a tap lands. */

import { FONT } from '@/hud/paint'

/** The phone's screen, pixels: portrait, as the canvas its texture is drawn on. */
export const SCREEN_W = 540
export const SCREEN_H = 1080
/** The status bar along the top and the app tabs along the bottom, pixels. */
export const STATUS_H = 64
export const TABS_H = 132
/** Where an app draws: between the two. */
export const APP_TOP = STATUS_H
export const APP_H = SCREEN_H - STATUS_H - TABS_H

export const INK = '#1d2b36'
export const PAPER = '#f4f1e8'
/** The courier's colour (`render/parcel.ts`), the delivery app's. */
export const TEAL = '#14a38f'
export const RED = '#d64545'
export const MUTED = 'rgba(29, 43, 54, 0.6)'

/** A place a tap lands on, in screen pixels, and what it does. */
export interface Region {
  x: number
  y: number
  w: number
  h: number
  act: () => void
}

/** The topmost region at `(x, y)`, if any: the last drawn wins. */
export function hit(regions: readonly Region[], x: number, y: number): Region | null {
  for (let i = regions.length - 1; i >= 0; i--) {
    const r = regions[i]!
    if (x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r
  }
  return null
}

export function font(size: number, weight = 700) {
  return `${weight} ${size}px ${FONT}`
}

/** `text` at `(x, y)` (its middle line), cut short with an ellipsis to fit `max` pixels. */
export function text(g: CanvasRenderingContext2D, value: string, x: number, y: number, size: number, color: string, { weight = 700, align = 'left' as CanvasTextAlign, max = Infinity } = {}) {
  g.font = font(size, weight)
  g.fillStyle = color
  g.textAlign = align
  g.textBaseline = 'middle'
  let shown = value
  if (g.measureText(shown).width > max) {
    while (shown.length > 1 && g.measureText(`${shown}…`).width > max) shown = shown.slice(0, -1)
    shown = `${shown}…`
  }
  g.fillText(shown, x, y)
}

export function rounded(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number, fill: string) {
  g.beginPath()
  g.roundRect(x, y, w, h, r)
  g.fillStyle = fill
  g.fill()
}

/** A pill button with `label`, its region added to `regions`. */
export function button(g: CanvasRenderingContext2D, regions: Region[], x: number, y: number, w: number, h: number, label: string, act: () => void, { fill = TEAL, color = '#fff', size = 28, enabled = true } = {}) {
  rounded(g, x, y, w, h, h / 2, enabled ? fill : '#b9c2c0')
  text(g, label, x + w / 2, y + h / 2 + 1, size, color, { weight: 800, align: 'center', max: w - 16 })
  if (enabled) regions.push({ x, y, w, h, act })
}
