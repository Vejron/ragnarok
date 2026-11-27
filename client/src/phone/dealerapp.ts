/**
 * The phone's dealer app (DEALERS.md, PHONE.md): every vehicle a dealer sells (`DEALER_KINDS`), with
 * its price from your delivery cash. At a dealer, tapping a price buys it, and it stands ready in the
 * carport; away from one, the list says how far the nearest is. A drag or the wheel scrolls the list.
 */

import { money } from '@/deliveries'
import { DEALER_KINDS, DEALER_PRICES, PHONE_ORDER } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'
import { APP_H, APP_TOP, INK, MUTED, PAPER, SCREEN_W, button, rounded, text, type Region } from './ui'

/** The dealers' blue (`render/dealers.ts`). */
export const BLUE = '#1f5fbf'

const HEADER_H = 104
const NOTE_H = 72
const ROW_H = 104
const GAP = 10
const PAD = 16

/** What the dealer app shows: the cash to spend, and how far the nearest dealer is (metres, null in a world with none), and whether it is near enough to buy at. */
export interface DealerView {
  cash: number
  distance: number | null
  near: boolean
}

/** How far `metres` is, for the note. */
const far = (metres: number) => (metres >= 1000 ? `${(metres / 1000).toFixed(1)} km` : `${Math.round(metres)} m`)

export class DealerApp {
  /** How far the list is scrolled down, pixels. */
  scroll = 0
  /** How tall the list was when last drawn, for the scroll's limit. */
  private content = 0

  /** Scrolls by `dy` pixels (down is positive), within the list. */
  scrollBy(dy: number) {
    this.scroll = Math.max(0, Math.min(Math.max(0, this.content - (APP_H - HEADER_H)), this.scroll + dy))
  }

  draw(g: CanvasRenderingContext2D, regions: Region[], view: DealerView, tap: (action: number, kind: number) => void) {
    g.fillStyle = PAPER
    g.fillRect(0, APP_TOP, SCREEN_W, APP_H)
    g.fillStyle = BLUE
    g.fillRect(0, APP_TOP, SCREEN_W, HEADER_H)
    text(g, 'Nord Bil', PAD + 8, APP_TOP + HEADER_H / 2, 44, '#fff', { weight: 800 })
    text(g, money(view.cash), SCREEN_W - PAD - 8, APP_TOP + HEADER_H / 2, 34, '#fff', { weight: 800, align: 'right' })
    if (view.distance === null) {
      text(g, 'No dealers in this world', SCREEN_W / 2, APP_TOP + APP_H / 2, 28, MUTED, { align: 'center' })
      return
    }

    const top = APP_TOP + HEADER_H
    g.save()
    g.beginPath()
    g.rect(0, top, SCREEN_W, APP_H - HEADER_H)
    g.clip()
    let y = top + PAD - this.scroll
    const shown = (r: Region) => r.y >= top && r.y + r.h <= APP_TOP + APP_H
    const local: Region[] = []
    const note = view.near ? 'Ready in the carport when you pay' : `Nearest dealer ${far(view.distance)} away`
    text(g, note, SCREEN_W / 2, y + NOTE_H / 2 - 8, 26, view.near ? INK : MUTED, { weight: 800, align: 'center', max: SCREEN_W - 2 * PAD })
    y += NOTE_H
    for (const [i, kind] of DEALER_KINDS.entries()) {
      const price = DEALER_PRICES[i]!
      rounded(g, PAD, y, SCREEN_W - 2 * PAD, ROW_H, 22, '#fff')
      const textMax = SCREEN_W - 2 * PAD - 24 - 170
      text(g, vehicleDef(kind).name, PAD + 24, y + ROW_H / 2, 30, INK, { weight: 800, max: textMax })
      const afford = view.cash >= price
      button(g, local, SCREEN_W - PAD - 24 - 150, y + (ROW_H - 64) / 2, 150, 64, money(price), () => tap(PHONE_ORDER, kind), { fill: BLUE, enabled: view.near && afford, size: 28 })
      y += ROW_H + GAP
    }
    g.restore()
    this.content = y + this.scroll - top
    regions.push(...local.filter(shown))
  }
}
