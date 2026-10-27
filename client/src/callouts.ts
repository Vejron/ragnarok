import { HIT_DEALT } from './feedback'
import { calloutLines, type ArmorMessage, type CalloutLine } from './sim/armor'

/** A callout stays this long, ms, and fades over the last part of it. */
export const CALLOUT_MS = 1600
const FADE_MS = 500
/** At most this many show at once, the newest at the top. */
const MAX_CALLOUTS = 4
/** The same line again this soon (a machine gun's rounds on the same plate) adds to the count instead. */
const JOIN_MS = 700

export interface CalloutView {
  text: string
  tone: CalloutLine['tone']
  /** How many times it happened in a row: shown as ×N past one. */
  count: number
  alpha: number
}

/** What the player is told about rounds meeting a vehicle's armour: their own, or ones that hit their vehicle. */
export class Callouts {
  private lines: { line: CalloutLine; at: number; count: number }[] = []

  receive(message: ArmorMessage, now: number) {
    for (const line of calloutLines(message, message.role === HIT_DEALT).reverse()) {
      const same = this.lines.find((shown) => shown.line.text === line.text && now - shown.at < JOIN_MS)
      if (same) {
        same.count++
        same.at = now
        continue
      }
      this.lines.unshift({ line, at: now, count: 1 })
    }
    this.lines.sort((a, b) => b.at - a.at)
    this.lines.length = Math.min(this.lines.length, MAX_CALLOUTS)
  }

  clear() {
    this.lines = []
  }

  view(now: number): CalloutView[] {
    this.lines = this.lines.filter((shown) => now - shown.at < CALLOUT_MS)
    return this.lines.map(({ line, at, count }) => ({ ...line, count, alpha: Math.min(1, (CALLOUT_MS - (now - at)) / FADE_MS) }))
  }
}
