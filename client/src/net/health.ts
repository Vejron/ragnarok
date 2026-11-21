/** No `welcome` this long after the session opened means the server never took us in, ms. */
const WELCOME_WITHIN_MS = 10_000
/** Nothing at all from the server for this long (snapshots come 30 times a second, pongs once) means the session is dead, ms. */
const SILENCE_MS = 5_000
/** A check that comes this much later than the last means the page was not running (a blocked main thread, a tab put away): what was queued meanwhile is read before anything is judged, ms. */
const LATE_CHECK_MS = 2_000
/** How far back corrections and acknowledgements are judged, ms. */
const WINDOW_MS = 4_000
/** Time after the welcome before anything is judged: the catch-up and the first snapshots settle the soldier where the server has them, ms. */
const SETTLE_MS = 3_000
/** A correction bigger than this, metres, is not the few centimetres of an ordinary misprediction. */
const BIG_CORRECTION = 0.5
/** A correction bigger than this, metres, is a snap (`SNAP_DISTANCE` in `predict.ts`). */
const SNAP = 3
/** Reconciles in the window before their corrections are judged: about a second and a half of snapshots. */
const MIN_CORRECTIONS = 40
/** The share of the window's reconciles that must be big for the soldier to be out of step with the server. */
const BIG_SHARE = 0.6
/** Snaps in the window that are not a respawn or a knock but the server and the page disagreeing. */
const MOST_SNAPS = 4
/** An input sent this long ago and still not acknowledged, while snapshots arrive, has not reached the server, ms. */
const ACK_STALL_MS = 3_000
/** Snapshots arriving but none of them rebuilt for this long (their baselines are not held) is a replica that cannot catch up, ms. */
const UNREBUILT_MS = 3_000
/** A resync waits this long after the last one, ms. */
const RESYNC_GAP_MS = 30_000
/** Resyncs within `RESYNC_SPAN_MS` that mean rejoining does not help, and the page should reload. */
const MOST_RESYNCS = 3
const RESYNC_SPAN_MS = 180_000
/** Unacknowledged inputs remembered at most: ten seconds of ticks. */
const MOST_UNACKED = 600

/**
 * Something wrong with the session that rejoining fixes. `silent`: the server has stopped talking, so
 * the session is closed and reconnected. `resync`: it talks, but the page and the server no longer
 * agree (the soldier is corrected all the time, inputs go unacknowledged, snapshots cannot be rebuilt),
 * so the page rejoins afresh, which is what reloading it did. `give-up`: rejoining has not helped, and
 * only reloading the page might.
 */
export interface Trouble {
  kind: 'silent' | 'resync' | 'give-up'
  reason: string
}

/**
 * Watches a session for being dead or out of step with the server (NETWORKING.md, "Healing"). The game
 * tells it what happens, with the time in ms, and asks `check` once a second.
 */
export class ConnectionHealth {
  private connectedAt: number | null = null
  private welcomedAt: number | null = null
  private lastHeard = 0
  private lastCheck: number | null = null
  private lastSnapshot = 0
  private lastRebuilt = 0
  private corrections: { at: number; metres: number }[] = []
  private unacked: { seq: number; at: number }[] = []
  /** When the page last rejoined to resync, kept across sessions. */
  private resyncs: number[] = []
  /** Set after giving up: no more resyncs until the page is reloaded. */
  private gaveUp = false
  /** What the last check made of it, for the HUD. */
  state = 'offline'

  /** The session opened. */
  connected(now: number) {
    this.forget()
    this.connectedAt = now
    this.lastHeard = now
    this.state = 'joining'
  }

  /** The session closed. */
  disconnected() {
    this.forget()
    this.state = 'offline'
  }

  welcomed(now: number) {
    this.welcomedAt = now
    this.lastRebuilt = now
    this.state = 'ok'
  }

  /** Any message from the server. */
  heard(now: number) {
    this.lastHeard = now
  }

  /** An input frame went out. */
  sent(seq: number, now: number) {
    this.unacked.push({ seq, at: now })
    if (this.unacked.length > MOST_UNACKED) this.unacked.shift()
  }

  /** A snapshot arrived; `rebuilt` when its baseline was held, with the newest input it acknowledges. */
  snapshot(now: number, rebuilt: boolean, ack: number) {
    this.lastSnapshot = now
    if (!rebuilt) return
    this.lastRebuilt = now
    let acknowledged = 0
    while (acknowledged < this.unacked.length && this.unacked[acknowledged]!.seq <= ack) acknowledged++
    if (acknowledged > 0) this.unacked.splice(0, acknowledged)
  }

  /** The predicted soldier, or the vehicle they drive, was put right by `metres`. */
  corrected(metres: number, now: number) {
    this.corrections.push({ at: now, metres })
    while (this.corrections.length > 0 && this.corrections[0]!.at < now - WINDOW_MS) this.corrections.shift()
  }

  /** What, if anything, should be done about the session now. */
  check(now: number): Trouble | null {
    const late = this.lastCheck !== null && now - this.lastCheck > LATE_CHECK_MS
    this.lastCheck = now
    if (this.connectedAt === null) return null
    if (late) {
      // The page was not running: give what came meanwhile its chance to be read.
      this.lastHeard = Math.max(this.lastHeard, now - SILENCE_MS / 2)
      return null
    }
    if (this.welcomedAt === null) {
      if (now - this.connectedAt <= WELCOME_WITHIN_MS) return null
      this.state = 'no welcome'
      return { kind: 'silent', reason: 'the server never welcomed us' }
    }
    if (now - this.lastHeard > SILENCE_MS) {
      this.state = 'silent'
      return { kind: 'silent', reason: `nothing from the server for ${Math.round((now - this.lastHeard) / 1000)} s` }
    }
    const reason = now - this.welcomedAt < SETTLE_MS ? null : this.outOfStep(now)
    if (!reason) {
      this.state = 'ok'
      return null
    }
    this.state = reason
    if (this.gaveUp) return null
    this.resyncs = this.resyncs.filter((at) => at > now - RESYNC_SPAN_MS)
    const last = this.resyncs[this.resyncs.length - 1]
    if (last !== undefined && now - last < RESYNC_GAP_MS) return null
    if (this.resyncs.length >= MOST_RESYNCS) {
      this.gaveUp = true
      return { kind: 'give-up', reason }
    }
    this.resyncs.push(now)
    return { kind: 'resync', reason }
  }

  /** Why the page and the server no longer agree, or null while they do. */
  private outOfStep(now: number): string | null {
    const recent = this.corrections.filter(({ at }) => at >= now - WINDOW_MS)
    if (recent.length >= MIN_CORRECTIONS && recent.filter(({ metres }) => metres > BIG_CORRECTION).length >= recent.length * BIG_SHARE) return 'corrected all the time'
    if (recent.filter(({ metres }) => metres > SNAP).length >= MOST_SNAPS) return 'snapped back again and again'
    const oldest = this.unacked[0]
    if (oldest && oldest.at < now - ACK_STALL_MS && now - this.lastRebuilt < 1000) return 'inputs not acknowledged'
    if (now - this.lastSnapshot < 1000 && now - this.lastRebuilt > UNREBUILT_MS) return 'snapshots not rebuilt'
    return null
  }

  private forget() {
    this.connectedAt = null
    this.welcomedAt = null
    this.lastCheck = null
    this.corrections = []
    this.unacked = []
  }
}
