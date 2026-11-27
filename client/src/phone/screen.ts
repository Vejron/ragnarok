/**
 * The phone's screen (PHONE.md), drawn on the canvas the held phone shows: a status bar, the app in
 * front (the map or the deliveries), and the tabs to switch between them. It is told where on it the
 * pointer goes, in its own pixels (`phone/pointer.ts`): a press and a release in about the same place
 * is a tap on what was drawn there; a press that moves is a drag, which pans the map or scrolls the list.
 */

import type { DeliveryView } from '@/deliveries'
import type { MapMarks } from '@/hud/map'
import type { MapTiles } from '@/hud/maptiles'
import type { PhoneApp } from '@/sim/input'
import { money } from '@/deliveries'
import { deliveryLine } from '@/hud/panels'
import { DealerApp, BLUE, type DealerView } from './dealerapp'
import { JobsApp } from './jobsapp'
import { MapApp } from './mapapp'
import { APP_H, APP_TOP, INK, MUTED, SCREEN_H, SCREEN_W, STATUS_H, TABS_H, TEAL, hit, text, type Region } from './ui'

/** A press that moves less than this, pixels, is a tap. */
const TAP_SLOP = 14

/** What the screen shows. */
export interface PhoneView {
  app: PhoneApp
  tiles: MapTiles
  marks: MapMarks
  delivery: DeliveryView | null
  /** The dealer app's cash and nearest dealer (DEALERS.md). */
  dealer: DealerView
  /** The time of day shown in the status bar. */
  clock: string
}

/** What a tap asks of the game. */
export interface PhoneActions {
  /** A tap on the delivery board or the dealer's list (`PHONE_*`). */
  phone(action: number, job: number): void
  /** Switch to `app`. */
  open(app: PhoneApp): void
}

export class PhoneScreen {
  readonly map = new MapApp()
  readonly jobs = new JobsApp()
  readonly dealer = new DealerApp()
  /** Something changed that the next draw should show. */
  dirty = true
  private regions: Region[] = []
  private press: { x: number; y: number; lastX: number; lastY: number; dragging: boolean } | null = null
  private view: PhoneView | null = null

  constructor(private readonly actions: PhoneActions) {}

  /** Draws the whole screen into `g` (`SCREEN_W` × `SCREEN_H`). */
  draw(g: CanvasRenderingContext2D, view: PhoneView) {
    this.view = view
    this.dirty = false
    const regions: Region[] = []
    g.fillStyle = '#fff'
    g.fillRect(0, 0, SCREEN_W, SCREEN_H)
    if (view.app === 'map') {
      const line = view.delivery && deliveryLine(view.delivery, view.marks.x, view.marks.z)
      this.map.draw(g, regions, view.tiles, view.marks, line ?? null)
    } else if (view.app === 'dealer') {
      this.dealer.draw(g, regions, view.dealer, (action, kind) => this.actions.phone(action, kind))
    } else {
      this.jobs.draw(g, regions, view.delivery, view.marks.x, view.marks.z, (action, job) => this.actions.phone(action, job))
    }
    this.statusBar(g, view)
    this.tabs(g, regions, view)
    this.regions = regions
  }

  private statusBar(g: CanvasRenderingContext2D, view: PhoneView) {
    g.fillStyle = view.app === 'jobs' ? TEAL : view.app === 'dealer' ? BLUE : 'rgba(255, 255, 255, 0.92)'
    g.fillRect(0, 0, SCREEN_W, STATUS_H)
    const ink = view.app === 'map' ? INK : '#fff'
    text(g, view.clock, 40, STATUS_H / 2 + 2, 28, ink, { weight: 800 })
    // The deliveries app has the cash in its own header.
    if (view.delivery && view.app === 'map') text(g, money(view.delivery.cash), SCREEN_W / 2, STATUS_H / 2 + 2, 24, ink, { weight: 700, align: 'center' })
    // Signal bars and a battery, right.
    g.fillStyle = ink
    for (let i = 0; i < 4; i++) g.fillRect(SCREEN_W - 140 + i * 12, STATUS_H / 2 + 10 - (i + 1) * 6, 8, (i + 1) * 6)
    g.strokeStyle = ink
    g.lineWidth = 3
    g.strokeRect(SCREEN_W - 80, STATUS_H / 2 - 10, 44, 22)
    g.fillRect(SCREEN_W - 76, STATUS_H / 2 - 6, 30, 14)
    g.fillRect(SCREEN_W - 34, STATUS_H / 2 - 4, 4, 10)
  }

  private tabs(g: CanvasRenderingContext2D, regions: Region[], view: PhoneView) {
    const top = SCREEN_H - TABS_H
    g.fillStyle = '#fff'
    g.fillRect(0, top, SCREEN_W, TABS_H)
    g.fillStyle = 'rgba(29, 43, 54, 0.12)'
    g.fillRect(0, top, SCREEN_W, 2)
    const apps: [PhoneApp, string][] = [['map', 'Map'], ['jobs', 'Deliveries'], ['dealer', 'Dealer']]
    const w = SCREEN_W / apps.length
    apps.forEach(([app, label], i) => {
      const on = view.app === app
      const color = on ? (app === 'dealer' ? BLUE : TEAL) : MUTED
      const cx = w * i + w / 2
      this.icon(g, app, cx, top + 46, color)
      text(g, label, cx, top + 92, 24, color, { weight: 800, align: 'center' })
      regions.push({ x: w * i, y: top, w, h: TABS_H, act: () => this.actions.open(app) })
    })
    // The home bar.
    g.fillStyle = INK
    g.beginPath()
    g.roundRect(SCREEN_W / 2 - 70, SCREEN_H - 16, 140, 6, 3)
    g.fill()
  }

  /** A tab's little picture: a folded map, a delivery bag, or a car. */
  private icon(g: CanvasRenderingContext2D, app: PhoneApp, x: number, y: number, color: string) {
    g.strokeStyle = color
    g.fillStyle = color
    g.lineWidth = 4
    g.lineJoin = 'round'
    if (app === 'map') {
      g.beginPath()
      g.moveTo(x - 24, y - 16)
      g.lineTo(x - 8, y - 22)
      g.lineTo(x + 8, y - 16)
      g.lineTo(x + 24, y - 22)
      g.lineTo(x + 24, y + 16)
      g.lineTo(x + 8, y + 22)
      g.lineTo(x - 8, y + 16)
      g.lineTo(x - 24, y + 22)
      g.closePath()
      g.stroke()
      g.beginPath()
      g.moveTo(x - 8, y - 22)
      g.lineTo(x - 8, y + 16)
      g.moveTo(x + 8, y - 16)
      g.lineTo(x + 8, y + 22)
      g.stroke()
    } else if (app === 'dealer') {
      // A car side on: its body, the cabin over it, and two wheels.
      g.strokeRect(x - 24, y - 2, 48, 14)
      g.beginPath()
      g.moveTo(x - 14, y - 2)
      g.lineTo(x - 8, y - 16)
      g.lineTo(x + 10, y - 16)
      g.lineTo(x + 16, y - 2)
      g.stroke()
      for (const wx of [x - 13, x + 13]) {
        g.beginPath()
        g.arc(wx, y + 14, 6, 0, Math.PI * 2)
        g.fill()
      }
    } else {
      g.strokeRect(x - 20, y - 16, 40, 36)
      g.fillRect(x - 20, y - 4, 40, 8)
      g.beginPath()
      g.moveTo(x - 10, y - 16)
      g.quadraticCurveTo(x, y - 30, x + 10, y - 16)
      g.stroke()
    }
  }

  /** The pointer went down at `(x, y)`, screen pixels. */
  pointerDown(x: number, y: number) {
    this.press = { x, y, lastX: x, lastY: y, dragging: false }
  }

  /** The pointer moved to `(x, y)` with the button down: past the slop, it drags. */
  pointerMove(x: number, y: number) {
    const press = this.press
    if (!press) return
    if (!press.dragging && Math.hypot(x - press.x, y - press.y) < TAP_SLOP) return
    press.dragging = true
    const [dx, dy] = [x - press.lastX, y - press.lastY]
    press.lastX = x
    press.lastY = y
    const view = this.view
    if (!view || press.y < APP_TOP || press.y >= APP_TOP + APP_H) return
    if (view.app === 'map') this.map.pan(dx, dy, view.marks)
    else if (view.app === 'dealer') this.dealer.scrollBy(-dy)
    else this.jobs.scrollBy(-dy)
    this.dirty = true
  }

  /** The pointer came up at `(x, y)`: a tap if it hardly moved. */
  pointerUp(x: number, y: number) {
    const press = this.press
    this.press = null
    if (!press || press.dragging) return
    const region = hit(this.regions, x, y)
    if (!region) return
    region.act()
    this.dirty = true
  }

  /** The wheel turned over `(x, y)`: `notches` down is positive. It zooms the map or scrolls the list. */
  wheel(notches: number) {
    if (!this.view) return
    if (this.view.app === 'map') this.map.zoom(notches)
    else if (this.view.app === 'dealer') this.dealer.scrollBy(notches * 90)
    else this.jobs.scrollBy(notches * 90)
    this.dirty = true
  }

  /** Whether a drag is under way, so the screen is drawn every frame. */
  get dragging(): boolean {
    return this.press?.dragging ?? false
  }
}
