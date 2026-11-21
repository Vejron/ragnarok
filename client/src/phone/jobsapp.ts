/**
 * The phone's deliveries app (DELIVERY.md, PHONE.md): the orders on the board to take, nearest
 * restaurant first, and your own, with what each pays. Tapping a price takes the order; Cancel gives
 * back one not yet fetched. A drag or the wheel scrolls the list.
 */

import { clockOf, houseName, money, shopName, type DeliveryView } from '@/deliveries'
import { JOB_CARRY, JOB_FETCH, PHONE_ACCEPT, PHONE_CANCEL } from '@/sim/consts'
import { APP_H, APP_TOP, INK, MUTED, PAPER, RED, SCREEN_W, TEAL, button, rounded, text, type Region } from './ui'

const HEADER_H = 104
const MINE_H = 150
const OFFER_H = 150
const GAP = 12
const PAD = 16

/** How far `metres` is, for a list. */
const far = (metres: number) => (metres >= 1000 ? `${(metres / 1000).toFixed(1)} km` : `${Math.round(metres)} m`)

export class JobsApp {
  /** How far the list is scrolled down, pixels. */
  scroll = 0
  /** How tall the list was when last drawn, for the scroll's limit. */
  private content = 0

  /** Scrolls by `dy` pixels (down is positive), within the list. */
  scrollBy(dy: number) {
    this.scroll = Math.max(0, Math.min(Math.max(0, this.content - (APP_H - HEADER_H)), this.scroll + dy))
  }

  draw(g: CanvasRenderingContext2D, regions: Region[], d: DeliveryView | null, x: number, z: number, tap: (action: number, job: number) => void) {
    g.fillStyle = PAPER
    g.fillRect(0, APP_TOP, SCREEN_W, APP_H)
    g.fillStyle = TEAL
    g.fillRect(0, APP_TOP, SCREEN_W, HEADER_H)
    text(g, 'Runner', PAD + 8, APP_TOP + HEADER_H / 2, 44, '#fff', { weight: 800 })
    if (!d) {
      text(g, 'No deliveries in this world', SCREEN_W / 2, APP_TOP + APP_H / 2, 28, MUTED, { align: 'center' })
      return
    }
    text(g, money(d.cash), SCREEN_W - PAD - 8, APP_TOP + HEADER_H / 2, 34, '#fff', { weight: 800, align: 'right' })

    const top = APP_TOP + HEADER_H
    g.save()
    g.beginPath()
    g.rect(0, top, SCREEN_W, APP_H - HEADER_H)
    g.clip()
    let y = top + PAD - this.scroll
    const busy = d.mine.some((job) => job.stage === JOB_FETCH || job.stage === JOB_CARRY)
    // A region is only kept where it shows.
    const shown = (r: Region) => r.y >= top && r.y + r.h <= APP_TOP + APP_H
    const local: Region[] = []

    for (const job of d.mine) {
      const carry = job.stage === JOB_CARRY
      const fetch = job.stage === JOB_FETCH
      rounded(g, PAD, y, SCREEN_W - 2 * PAD, MINE_H, 24, carry ? TEAL : fetch ? '#fff' : '#fff')
      if (!carry) {
        g.lineWidth = 4
        g.strokeStyle = fetch ? TEAL : RED
        g.beginPath()
        g.roundRect(PAD + 2, y + 2, SCREEN_W - 2 * PAD - 4, MINE_H - 4, 22)
        g.stroke()
      }
      const ink = carry ? '#fff' : INK
      const title = fetch ? `Pick up at ${shopName(job.id)}` : carry ? `Deliver to ${houseName(job.id)}` : `Lost: ${houseName(job.id)}'s order`
      text(g, title, PAD + 24, y + 40, 30, ink, { weight: 800, max: SCREEN_W - 2 * PAD - 48 })
      const facts = [far(Math.hypot(job.x - x, job.z - z)), money(job.pay), ...(carry ? [clockOf(job.left)] : [])].join('   ')
      text(g, facts, PAD + 24, y + 84, 26, carry && job.left < 0 ? '#ffd166' : ink, { weight: 700 })
      if (fetch) button(g, local, SCREEN_W - PAD - 24 - 150, y + MINE_H - 24 - 52, 150, 52, 'Cancel', () => tap(PHONE_CANCEL, job.id), { fill: '#fff', color: INK, size: 24 })
      if (fetch) {
        g.lineWidth = 3
        g.strokeStyle = INK
        g.beginPath()
        g.roundRect(SCREEN_W - PAD - 24 - 150, y + MINE_H - 24 - 52, 150, 52, 26)
        g.stroke()
      }
      y += MINE_H + GAP
    }

    text(g, 'ORDERS NEARBY', PAD + 8, y + 24, 22, MUTED, { weight: 800 })
    y += 52
    const offers = [...d.offers].sort((a, b) => Math.hypot(a.shopX - x, a.shopZ - z) - Math.hypot(b.shopX - x, b.shopZ - z))
    for (const offer of offers) {
      rounded(g, PAD, y, SCREEN_W - 2 * PAD, OFFER_H, 24, '#fff')
      const textMax = SCREEN_W - 2 * PAD - 24 - 170
      text(g, shopName(offer.id), PAD + 24, y + 36, 30, INK, { weight: 800, max: textMax })
      text(g, `to ${houseName(offer.id)}`, PAD + 24, y + 74, 24, MUTED, { weight: 700, max: textMax })
      const trip = Math.hypot(offer.houseX - offer.shopX, offer.houseZ - offer.shopZ)
      text(g, `${far(Math.hypot(offer.shopX - x, offer.shopZ - z))} away · ${far(trip)} · ${clockOf(offer.par)}`, PAD + 24, y + 112, 22, INK, { weight: 700, max: textMax })
      button(g, local, SCREEN_W - PAD - 24 - 140, y + (OFFER_H - 72) / 2, 140, 72, money(offer.pay), () => tap(PHONE_ACCEPT, offer.id), { enabled: !busy, size: 30 })
      y += OFFER_H + GAP
    }
    if (!offers.length) {
      text(g, 'No orders right now', SCREEN_W / 2, y + 40, 26, MUTED, { align: 'center' })
      y += 80
    }
    g.restore()
    this.content = y + this.scroll - top
    regions.push(...local.filter(shown))
  }
}
