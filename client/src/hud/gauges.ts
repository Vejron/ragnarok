import type { Texture } from '@ragnarok/engine'
import type { GolfMeterView, HudStats } from '@/hud'
import { HOTBAR_ICONS } from '@/hotbarIcons'
import { LIMB_ARM_L, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_R } from '@/net/codec'
import { HEN_EGGS } from '@/sim/consts'
import { METER_CURVE, METER_HIGH, METER_LOW, METER_MISS, METER_PURE } from '@/sim/golf'
import { HOTBAR_SLOTS, itemDef, type ItemSlot } from '@/sim/items'
import { AMBER, CanvasSprite, INK, INK_CSS, RED, TEAL, TEXT, canvas2d, hudTexture, uiScale, type Painter } from './paint'
import { dialTicks, dialTop, fill, needleAngle } from './speedo'

/** How much wider than the hotbar the golf meter is, and the least it is across, CSS pixels. */
const GOLF_METER_WIDEN = 2.2
const GOLF_METER_MIN = 560

/** Fills for the gauge bars, left to right (a single colour is flat). */
const FILLS = {
  heat: ['#f3c94b', '#ff5a5f'],
  air: ['#9fd8ff', '#e8f6ff'],
  stamina: ['#8fd16a', '#d9f5a8'],
  breath: ['#5fb7d9', '#c9eefc'],
  white: ['#ffffff'],
} as const
type Fill = keyof typeof FILLS

export const SLOT = 32
/** A touch screen's slots are big enough for a thumb. */
export const TOUCH_SLOT = 44
const SLOT_GAP = 4

type Rect = { x: number; y: number; w: number; h: number }

/** Where the hotbar's slots are on a view `width` × `height` layout pixels. */
export function hotbarRects(width: number, height: number, touch: boolean): Rect[] {
  const slot = touch ? TOUCH_SLOT : SLOT
  const total = HOTBAR_SLOTS * slot + (HOTBAR_SLOTS - 1) * SLOT_GAP
  const left = (width - total) / 2
  const top = height - (touch ? 10 : 12) - slot
  return Array.from({ length: HOTBAR_SLOTS }, (_, k) => ({ x: left + k * (slot + SLOT_GAP), y: top, w: slot, h: slot }))
}

/** `hotbarRects` in CSS pixels, as the HUD draws them on a view `width` × `height` CSS pixels: for taps on a touch screen. */
export function hotbarRectsCss(width: number, height: number, touch: boolean): Rect[] {
  const ui = uiScale(width, height)
  return hotbarRects(width / ui, height / ui, touch).map((r) => ({ x: r.x * ui, y: r.y * ui, w: r.w * ui, h: r.h * ui }))
}

/** `ammo` for a gun, the stack size for a thing you use up. */
export function slotDetail(slot: ItemSlot): string {
  const item = itemDef(slot.item)
  if (!item) return ''
  if (item.kind === 'gun') return `${slot.ammo}/${item.mag}`
  return slot.count > 1 ? `×${slot.count}` : ''
}

/** What a body part with `left` of its health (0..1) is drawn in: white when whole, through amber, to red. */
const partColor = (left: number) => {
  const t = Math.max(0, Math.min(1, left))
  return t >= 0.999 ? 'rgba(255, 255, 255, 0.85)' : `hsl(${50 * t}, 95%, ${60 + 25 * t * t}%)`
}

/** The body map's limbs, `LIMB_*` bit by bit, as lines from the shoulders or hips, in its 40×64 box. */
const LIMB_PARTS = [
  { bit: LIMB_ARM_L, from: [20, 18], to: [7, 32] },
  { bit: LIMB_ARM_R, from: [20, 18], to: [33, 32] },
  { bit: LIMB_LEG_L, from: [20, 36], to: [10, 60] },
  { bit: LIMB_LEG_R, from: [20, 36], to: [30, 60] },
] as const

/**
 * The bottom of the screen: health, the body map, the hotbar, and in the bottom-right corner the
 * gauges (heat, air, stamina, battery, eggs and bombs), the vehicle's health and the speedometer.
 */
export class Gauges {
  private readonly bars = new Map<string, Texture>()
  private readonly icons = new Map<number, Texture | null>()
  private readonly body = new CanvasSprite(30, 48)

  /** A rounded bar `w` × `h` in `fill`, made once, for showing part of. */
  private barTexture(kind: Fill, w: number, h: number, scale: number): Texture {
    const key = `${kind}|${w}|${h}|${scale}`
    let texture = this.bars.get(key)
    if (texture) return texture
    const [canvas, g] = canvas2d(w * scale, h * scale)
    if (g) {
      g.scale(scale, scale)
      const stops = FILLS[kind]
      const gradient = g.createLinearGradient(0, 0, w, 0)
      stops.forEach((stop, k) => gradient.addColorStop(stops.length === 1 ? 0 : k / (stops.length - 1), stop))
      g.fillStyle = gradient
      g.beginPath()
      g.roundRect(0, 0, w, h, h / 2)
      g.fill()
    }
    texture = hudTexture(canvas)
    this.bars.set(key, texture)
    return texture
  }

  /** A pill-shaped bar at (`x`, `y`) filled `amount` (0..1) with `kind`, tinted `tint`, with `text` over it. */
  bar(p: Painter, x: number, y: number, w: number, h: number, amount: number, kind: Fill, tint: number, text: string, textSize = TEXT.xs) {
    p.pill(x, y, w, h, 0xffffff, 0.55, 2)
    const f = Math.max(0, Math.min(1, amount))
    if (f > 0) p.image(this.barTexture(kind, w, h, p.scale), x, y, w * f, h, 1, tint, [0, 0, f, 1])
    p.label(text, x + w / 2, y + h / 2, { size: textSize, weight: 800, color: INK_CSS }, 'center')
  }

  draw(p: Painter, s: HudStats, now: number) {
    if (s.hands) {
      this.health(p, s)
      if (s.limbs) this.bodyMap(p, s.limbs, now)
      this.hotbar(p, s)
    }
    this.corner(p, s)
  }

  private health(p: Painter, s: HudStats) {
    const x = 16
    const y = p.height - 16 - 22
    const low = s.hp <= s.maxHp / 4
    if (s.hurt > 0.3) p.pill(x - 4, y - 4, 228, 30, RED, 0.35)
    p.pill(x, y, 220, 22, 0xffffff, 0.55, 2, s.hurt > 0.3 ? RED : 0x000000, s.hurt > 0.3 ? 1 : 0.25)
    const f = fill(s.hp, s.maxHp)
    if (f > 0) p.image(this.barTexture('white', 220, 22, p.scale), x, y, 220 * f, 22, 1, low ? RED : TEAL, [0, 0, f, 1])
    p.label(String(s.hp), x + 110, y + 11, { size: 13, weight: 800, color: INK_CSS }, 'center')
  }

  private bodyMap(p: Painter, limbs: NonNullable<HudStats['limbs']>, now: number) {
    // The lost limbs still bleeding pulse; ten steps of the pulse are plenty.
    const phase = limbs.bleeding ? Math.round((0.5 - 0.5 * Math.cos(((now / 900) % 1) * Math.PI * 2)) * 10) : 0
    const key = `${limbs.lost}|${limbs.bleeding}|${limbs.head.toFixed(2)}|${limbs.torso.toFixed(2)}|${limbs.parts.map((v) => v.toFixed(2)).join(',')}|${phase}`
    this.body.update(key, (g) => {
      g.scale(30 / 40, 48 / 64)
      g.lineWidth = 4
      g.lineCap = 'round'
      g.strokeStyle = partColor(limbs.head)
      g.beginPath()
      g.arc(20, 8, 6, 0, Math.PI * 2)
      g.stroke()
      g.strokeStyle = partColor(limbs.torso)
      g.beginPath()
      g.moveTo(20, 14)
      g.lineTo(20, 36)
      g.stroke()
      LIMB_PARTS.forEach((part, i) => {
        const lost = (limbs.lost & part.bit) !== 0
        const bleeding = (limbs.bleeding & part.bit) !== 0
        g.globalAlpha = lost && bleeding ? 0.35 + 0.065 * phase : 1
        g.strokeStyle = lost ? (bleeding ? '#ff5a5f' : 'rgba(255, 255, 255, 0.15)') : partColor(limbs.parts[i] ?? 1)
        g.beginPath()
        g.moveTo(part.from[0], part.from[1])
        g.lineTo(part.to[0], part.to[1])
        g.stroke()
      })
      g.globalAlpha = 1
    }, p.scale)
    p.sprite(this.body, 16, p.height - 46 - 48)
  }

  /** Item `id`'s icon, rasterised from its SVG once it has loaded; null until then or when it has none. */
  private icon(id: number): Texture | null {
    if (this.icons.has(id)) return this.icons.get(id)!
    this.icons.set(id, null)
    const svg = HOTBAR_ICONS[id]
    if (!svg) return null
    const image = new Image()
    image.onload = () => {
      const [canvas, g] = canvas2d(48, 48)
      if (!g) return
      g.drawImage(image, 0, 0, 48, 48)
      this.icons.set(id, hudTexture(canvas, true))
    }
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="48" height="48" style="color:${INK_CSS}">${svg}</svg>`)}`
    return null
  }

  /**
   * The golf swing meter, `w` wide at `[x, y]`: the power filling it while fire is held, a tick where it
   * was let go, and the marker running back to the line near its left end. Around the line the zones:
   * pure (white), a draw or fade (green), a hook or slice (amber), and a mishit (red); past a full swing,
   * the overswing.
   */
  private golfMeter(p: Painter, golf: GolfMeterView, x: number, y: number, w: number) {
    const h = 16
    const at = (v: number) => x + ((v - METER_LOW) / (METER_HIGH - METER_LOW)) * w
    const span = (from: number, to: number, color: number, alpha: number) => {
      const [a, b] = [at(Math.max(METER_LOW, from)), at(Math.min(METER_HIGH, to))]
      p.rect(a, y, b - a, h, color, alpha)
    }
    p.box(x, y, w, h, 3, 0x000000, 0.35, 1, 0xffffff, 0.5)
    span(1, METER_HIGH, 0xe63946, 0.45)
    span(-METER_MISS, METER_MISS, 0xe63946, 0.55)
    span(-METER_CURVE, METER_CURVE, 0xffbe0b, 0.8)
    span(-(METER_PURE + METER_CURVE) / 2, (METER_PURE + METER_CURVE) / 2, 0x6ad04a, 0.85)
    span(-METER_PURE, METER_PURE, 0xffffff, 0.95)
    for (const v of [0.25, 0.5, 0.75, 1]) p.rect(at(v) - 0.5, y + h - 4, 1, 4, 0xffffff, 0.6)
    const reading = golf.reading
    if (reading) {
      const top = at(Math.max(0, reading.power))
      if (!reading.returning) p.rect(at(0), y + 2, top - at(0), h - 4, 0xffbe0b, 0.9)
      else {
        p.rect(top - 1, y - 3, 2, h + 6, 0xffbe0b, 1)
        const m = at(Math.max(METER_LOW, reading.marker))
        p.rect(m - 1.5, y - 4, 3, h + 8, 0xffffff, 1)
        p.rect(m - 0.5, y - 4, 1, h + 8, 0x000000, 0.6)
      }
    }
    if (golf.call) p.label(golf.call, x + w / 2, y - 14, { size: 15, weight: 800, color: '#ffffff', shadow: true }, 'center', 0.95)
  }

  private hotbar(p: Painter, s: HudStats) {
    const rects = hotbarRects(p.width, p.height, s.touch)
    const first = rects[0]!
    const last = rects[rects.length - 1]!
    if (s.golf) {
      // Wider than the hotbar, for a finer read of the line; centred over it, within the screen.
      const bar = last.x + last.w - first.x
      const w = Math.min(p.width - 32, Math.max(bar * GOLF_METER_WIDEN, GOLF_METER_MIN))
      this.golfMeter(p, s.golf, first.x + bar / 2 - w / 2, first.y - 28, w)
    }
    else if (s.progress > 0) {
      const w = last.x + last.w - first.x
      p.pill(first.x, first.y - 8, w, 5, 0xffffff, 0.55, 1)
      p.round(first.x, first.y - 8, Math.max(5, w * s.progress), 5, 2.5, 0xffbe0b)
    }
    rects.forEach((r, k) => {
      const slot = s.hotbar[k]
      const item = slot ? itemDef(slot.item) : undefined
      const selected = s.selected === k
      const empty = !slot || slot.item === 0
      const dry = slot && item?.kind === 'gun' && slot.ammo === 0
      const fade = empty ? 0.6 : 1
      const y = r.y - (selected ? 2 : 0)
      if (selected) {
        p.box(r.x - 2, y - 2, r.w + 4, r.h + 4, 8, 0xffbe0b, 1, 1, 0x000000, 0.25)
        p.box(r.x, y, r.w, r.h, 6, 0xffffff, 0.92)
      } else p.box(r.x, y, r.w, r.h, 6, 0xffffff, 0.55 * fade, 1, 0x000000, 0.25 * fade)
      const icon = slot ? this.icon(slot.item) : null
      const inset = r.w / 8
      if (icon) p.image(icon, r.x + inset, y + inset, r.w - 2 * inset, r.h - 2 * inset, (dry ? 0.45 : 1) * fade)
      p.label(String(k + 1), r.x + 3, y + 6, { size: TEXT.xs, weight: 800, color: INK_CSS }, 'left', 0.7 * fade)
      if (slot) p.label(slotDetail(slot), r.x + r.w - 2, y + r.h - 6, { size: TEXT.xs, weight: 800, color: dry ? '#b3261e' : INK_CSS }, 'right', fade)
    })
  }

  /**
   * The bottom-right column, from the bottom up: bars, then the vehicle's health, then the speedometer.
   * A touch screen's buttons are in that corner, so there the column hangs from the top right (where the
   * minimap would be), from the top down, the speedometer first.
   */
  private corner(p: Painter, s: HudStats) {
    const right = p.width - 16
    const W = 160
    const H = 18
    const rows: { h: number; draw: (top: number) => void }[] = []
    const row = (draw: (top: number) => void, h = H) => rows.push({ h, draw })
    const gauge = (amount: number, kind: Fill, alarm: boolean, text: string) =>
      row((top) => this.bar(p, right - W, top, W, H, amount, alarm ? 'white' : kind, alarm ? RED : 0xffffff, text))
    const pips = (count: number, left: number, egg: boolean) =>
      row((top) => {
        const w = egg ? 14 : 10
        for (let k = 0; k < count; k++) {
          const x = right - count * (w + 5) + 5 + k * (w + 5)
          const alpha = k < left ? 1 : 0.2
          if (egg) {
            p.layer.centred(x + 7, top + 11, 18, 22, 0x000000, 0.45 * alpha, p.disc)
            p.layer.centred(x + 7, top + 11, 14, 18, 0xf6efdc, alpha, p.disc)
          } else p.box(x, top, 10, 22, 4, 0x2b2b26, alpha, 2, 0xffffff, 0.7 * alpha)
        }
      }, 22)

    if (s.heat !== null) {
      if (s.bombs !== null) pips(4, s.bombs, false)
      gauge(s.heat, 'heat', s.jammed, s.jammed ? 'JAMMED' : 'GUNS')
    }
    if (s.air !== null) {
      if (s.eggs !== null) pips(HEN_EGGS, s.eggs, true)
      const wings = s.seat === 'hen' || s.seat === 'tupp'
      const label = s.seat === 'helm' ? (s.air < 0.15 ? 'IN IRONS' : 'WIND') : wings ? (s.air < 0.15 ? 'GLIDE' : 'WINGS') : s.seat === 'sled' ? (s.air < 0.15 ? 'BOOST LOW' : 'BOOST') : s.air < 0.15 ? 'AIR LOW' : 'AIR'
      gauge(s.air, 'air', s.air < 0.15, label)
    }
    if (s.breath !== null) gauge(s.breath, 'breath', s.breath <= 0, s.breath <= 0 ? 'DROWNING' : 'BREATH')
    if (s.stamina && (s.stamina.left < 1 || s.stamina.winded)) gauge(s.stamina.left, 'stamina', s.stamina.winded, s.stamina.winded ? 'WINDED' : 'STAMINA')
    if (s.drone) {
      gauge(s.drone.signal, 'air', s.drone.signal < 0.2, s.drone.signal < 0.2 ? 'SIGNAL WEAK' : 'SIGNAL')
      gauge(s.drone.battery, 'air', s.drone.battery < 0.2, s.drone.battery < 0.2 ? 'BATTERY LOW' : 'BATTERY')
    }
    if (s.vehicle) {
      const f = fill(s.vehicle.hp, s.vehicle.maxHp)
      row((top) => this.bar(p, right - W, top, W, H, f, 'white', f > 0.5 ? TEAL : f > 0.25 ? AMBER : RED, `${s.vehicle!.name} ${Math.ceil(f * 100)}%`))
    }
    if (s.speed !== null) row((top) => this.speedometer(p, right - 60, top + 60, 60, s.seat, s.speed!), 120)
    if (s.touch) {
      let top = 16
      for (const { h, draw } of rows.reverse()) {
        draw(top)
        top += h + 6
      }
    } else {
      let bottom = p.height - 16
      for (const { h, draw } of rows) {
        draw(bottom - h)
        bottom -= h + 6
      }
    }
  }

  /** A round dial of radius `r` centred (`cx`, `cy`): ticks, numbers, a needle and the speed written below. */
  private speedometer(p: Painter, cx: number, cy: number, r: number, seat: HudStats['seat'], kmh: number) {
    const top = dialTop(seat, kmh)
    p.dot(cx, cy, r + 2, 0xffffff, 0.9)
    p.dot(cx, cy, r, INK, 0.72)
    for (const tick of dialTicks(top)) {
      const a = needleAngle(tick.kmh, top)
      const [c, s] = [Math.cos(a), Math.sin(a)]
      const inner = r - (tick.major ? 10 : 6)
      p.line(cx + c * inner, cy + s * inner, cx + c * (r - 3), cy + s * (r - 3), tick.major ? 2 : 1.5, 0xffffff, tick.major ? 0.95 : 0.6)
      if (tick.major) p.label(String(tick.kmh), cx + c * (r - 19), cy + s * (r - 19), { size: TEXT.xs, weight: 700, color: '#fff' }, 'center', 0.9)
    }
    const a = needleAngle(kmh, top)
    const reach = r - 8
    p.line(cx - Math.cos(a) * 8, cy - Math.sin(a) * 8, cx + Math.cos(a) * reach, cy + Math.sin(a) * reach, 3, RED)
    p.dot(cx, cy, 5, 0xffffff)
    p.dot(cx, cy, 3, RED)
    p.label(String(Math.round(kmh)), cx, cy + r * 0.42, { size: 20, weight: 800, color: '#fff' }, 'center')
    p.label('km/h', cx, cy + r * 0.72, { size: TEXT.xs, weight: 700, color: '#fff' }, 'center', 0.85)
  }

  dispose() {
    for (const texture of this.bars.values()) texture.dispose()
    for (const texture of this.icons.values()) texture?.dispose()
    this.body.dispose()
  }
}
