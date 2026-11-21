/**
 * The delivery mode (DELIVERY.md), as the client hears of it: the board of jobs on the phone, the
 * player's own jobs, and who delivered what. The world names no shops or houses, so each job is given
 * a made-up restaurant and household from its id, the same on every client.
 */

import { JOB_CARRY, JOB_FETCH, JOB_LOST } from '@/sim/consts'
import type { JobOffer, JobState } from '@/net/codec'

export type { JobOffer, JobState }

/** One `delivered` message. */
export interface DeliveredNews {
  by: number
  courier: number
  job: number
  pay: number
  cash: number
}

/** Where the map's waypoint points: the shop to fetch at, the house to hand in at, or where a lost parcel lies. */
export interface DeliveryTarget {
  x: number
  z: number
  stage: number
}

/** What the HUD and the phone show of the mode. */
export interface DeliveryView {
  /** The job that matters most: the parcel carried, else the one to fetch, else one lost. */
  job: JobState | null
  target: DeliveryTarget | null
  offers: readonly JobOffer[]
  mine: readonly JobState[]
  /** What the player has earned delivering, in all. */
  cash: number
}

const SHOPS = [
  'Pizzeria Napoli', 'Sushi Zen', 'Kebab Palace', 'Thai Garden', 'The Burger Shack', 'Falafel Corner', 'Taco Loco', 'Noodle Bar',
  'Curry House', 'Dumpling Den', 'Poke Bowl', 'Pasta Fresca', 'Korv & Mos', 'Pho Saigon', 'Wok Express', 'Smash Burger',
]
const FAMILIES = [
  'Andersson', 'Johansson', 'Karlsson', 'Nilsson', 'Eriksson', 'Larsson', 'Olsson', 'Persson', 'Svensson', 'Gustafsson',
  'Pettersson', 'Jonsson', 'Lindberg', 'Lindqvist', 'Berglund', 'Nordström', 'Lundgren', 'Holm', 'Sandberg', 'Forsberg',
]

/** A scramble of `id`, so neighbouring jobs get unrelated names. */
function mix(id: number, salt: number): number {
  let h = Math.imul(id ^ salt, 0x9e3779b1)
  h ^= h >>> 15
  h = Math.imul(h, 0x85ebca6b)
  h ^= h >>> 13
  return h >>> 0
}

/** The restaurant job `id`'s parcel is fetched at. */
export const shopName = (id: number) => SHOPS[mix(id, 0x5150) % SHOPS.length]!

/** The household job `id`'s parcel goes to. */
export const houseName = (id: number) => `the ${FAMILIES[mix(id, 0xfa11) % FAMILIES.length]!}s`

/** Cash as the game shows it. */
export const money = (amount: number) => `${amount} kr`

/** Seconds as m:ss, with a minus once over. */
export const clockOf = (seconds: number) => {
  const s = Math.abs(Math.round(seconds))
  return `${seconds < 0 ? '−' : ''}${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export class DeliveryTracker {
  offers: JobOffer[] = []
  mine: JobState[] = []
  /** Whether the world has deliveries: once one `jobs` message has come. */
  active = false
  /** Jobs handed in, so one gone from the phone is not taken for spoiled. */
  private readonly done = new Set<number>()

  clear() {
    this.offers = []
    this.mine = []
    this.active = false
    this.done.clear()
  }

  /**
   * A `jobs` message. Returns what to say of how the player's own jobs changed, or null; the first
   * one heard says nothing, being how things already stood.
   */
  receive(offers: JobOffer[], mine: JobState[]): string | null {
    const before = this.active ? this.mine : null
    this.offers = offers
    this.mine = mine
    this.active = true
    if (!before) return null
    const was = new Map(before.map((job) => [job.id, job]))
    const news: string[] = []
    for (const job of mine) {
      const old = was.get(job.id)
      if (old?.stage === job.stage) continue
      if (job.stage === JOB_FETCH) news.push(`Order taken: pick it up at ${shopName(job.id)}`)
      else if (job.stage === JOB_CARRY) news.push(old ? `Got it! Deliver to ${houseName(job.id)}` : `You grabbed an order for ${houseName(job.id)}. Deliver it for the cash!`)
      else if (job.stage === JOB_LOST) news.push(old?.stage === JOB_CARRY ? 'You dropped the order!' : 'You lost your order')
    }
    const now = new Set(mine.map((job) => job.id))
    for (const job of before) {
      if (now.has(job.id) || this.done.has(job.id)) continue
      if (job.stage === JOB_LOST) news.push(`The order for ${houseName(job.id)} spoiled`)
      else if (job.stage === JOB_CARRY) news.push('The order is out of your hands')
    }
    return news.length ? news.join(' · ') : null
  }

  /** A `delivered` message. Returns what to say of it, when it concerns the local player. */
  delivered(news: DeliveredNews, localId: number | null, nameOf: (id: number) => string): string | null {
    this.done.add(news.job)
    if (news.by === localId) return news.courier === localId || news.courier === 0 ? `Delivered! +${money(news.pay)}` : `Delivered ${nameOf(news.courier)}'s order! +${money(news.pay)}`
    if (news.courier === localId) return `${nameOf(news.by)} delivered your order and took the ${money(news.pay)}`
    return null
  }

  /** The job that matters most to the player now and where it sends them, with what they have earned (`cash`). */
  view(cash = 0): DeliveryView | null {
    if (!this.active) return null
    const rank = (stage: number) => (stage === JOB_CARRY ? 0 : stage === JOB_FETCH ? 1 : 2)
    const job = [...this.mine].sort((a, b) => rank(a.stage) - rank(b.stage))[0] ?? null
    return { job, target: job ? { x: job.x, z: job.z, stage: job.stage } : null, offers: this.offers, mine: this.mine, cash }
  }
}
