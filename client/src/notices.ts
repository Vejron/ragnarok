import { NOTICE_LOST, NOTICE_OBJECTIVE, NOTICE_SAY, NOTICE_WON } from './net/codec'

/** What the scenario has to say, as drawn. */
export interface NoticeView {
  /** What to do now; empty when nothing. */
  objective: string
  /** Recent messages, oldest first. */
  messages: string[]
  /** Just won or lost, and how. */
  outcome: { won: boolean; text: string } | null
}

/** How long a message shows, ms, and how many at once; and how long a win or a loss shows. */
export const MESSAGE_MS = 6000
export const MESSAGES = 3
export const OUTCOME_MS = 8000

/** The objective, the recent messages and the end of a world's scenario, from its `notice` messages. */
export class Notices {
  private objective = ''
  private messages: { text: string; at: number }[] = []
  private outcome: { won: boolean; text: string; at: number } | null = null

  clear() {
    this.objective = ''
    this.messages = []
    this.outcome = null
  }

  receive(kind: number, text: string, now: number) {
    if (kind === NOTICE_SAY) {
      this.messages.push({ text, at: now })
      this.messages = this.messages.slice(-MESSAGES)
    } else if (kind === NOTICE_OBJECTIVE) {
      this.objective = text
    } else if (kind === NOTICE_WON || kind === NOTICE_LOST) {
      this.objective = ''
      this.outcome = { won: kind === NOTICE_WON, text, at: now }
    }
  }

  view(now: number): NoticeView {
    this.messages = this.messages.filter((m) => now - m.at < MESSAGE_MS)
    if (this.outcome && now - this.outcome.at >= OUTCOME_MS) this.outcome = null
    return {
      objective: this.objective,
      messages: this.messages.map((m) => m.text),
      outcome: this.outcome && { won: this.outcome.won, text: this.outcome.text },
    }
  }
}
