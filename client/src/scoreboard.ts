/** One line of the kill feed, as drawn. */
export interface FeedEntry {
  /** 0 when the world did it. */
  killer: number
  victim: number
  weapon: number
  /** Whether the local player is the killer or the victim. */
  you: 'killer' | 'victim' | null
}

export interface ScoreRow {
  id: number
  kills: number
  deaths: number
  /** Seconds carrying the backpack (BACKPACK.md). */
  held: number
  you: boolean
}

/** One side's rows on the scoreboard, with what they have done between them. */
export interface Side {
  /** 0 for people, who are all against all. */
  team: number
  kills: number
  deaths: number
  held: number
  rows: ScoreRow[]
}

/**
 * `rows` (best first) split by team: the local player's side first, then the rest by kills. A
 * room of people alone is one side.
 */
export function sides(rows: readonly ScoreRow[], teamOf: (id: number) => number): Side[] {
  const byTeam = new Map<number, Side>()
  for (const row of rows) {
    const team = teamOf(row.id)
    let side = byTeam.get(team)
    if (!side) byTeam.set(team, (side = { team, kills: 0, deaths: 0, held: 0, rows: [] }))
    side.kills += row.kills
    side.deaths += row.deaths
    side.held += row.held
    side.rows.push(row)
  }
  const yours = (side: Side) => side.rows.some((row) => row.you)
  return [...byTeam.values()].sort((a, b) => Number(yours(b)) - Number(yours(a)) || b.kills - a.kills || a.team - b.team)
}

/** How long a kill stays in the feed, ms, and how many show at once. */
export const FEED_MS = 6000
export const FEED_LENGTH = 5

/**
 * Totals and the recent kills. The server sends the new totals with every kill, so nothing is
 * counted here.
 */
export class Scoreboard {
  private readonly totals = new Map<number, { kills: number; deaths: number; held: number }>()
  private feed: { killer: number; victim: number; weapon: number; at: number }[] = []

  clear() {
    this.totals.clear()
    this.feed = []
  }

  /** `scores` message: everyone's totals, sent once to a player who has just joined. */
  replace(scores: readonly { id: number; kills: number; deaths: number; held: number }[]) {
    this.totals.clear()
    for (const { id, kills, deaths, held } of scores) this.totals.set(id, { kills, deaths, held })
  }

  /** `backpack` message: its carrier's seconds with it, in all. */
  held(id: number, seconds: number) {
    this.entry(id).held = seconds
  }

  forget(id: number) {
    this.totals.delete(id)
  }

  kills(id: number | null): number {
    return id === null ? 0 : (this.totals.get(id)?.kills ?? 0)
  }

  deaths(id: number | null): number {
    return id === null ? 0 : (this.totals.get(id)?.deaths ?? 0)
  }

  /** `kill` message. `killer` is 0 when the world did it. */
  kill(kill: { killer: number; victim: number; weapon: number; killerKills: number; victimDeaths: number }, now: number) {
    if (kill.killer !== 0) this.entry(kill.killer).kills = kill.killerKills
    this.entry(kill.victim).deaths = kill.victimDeaths
    this.feed.push({ killer: kill.killer, victim: kill.victim, weapon: kill.weapon, at: now })
    if (this.feed.length > FEED_LENGTH) this.feed.shift()
  }

  /** The kills still on screen at `now`, marked from `localId`'s point of view. */
  recent(now: number, localId: number | null): FeedEntry[] {
    this.feed = this.feed.filter((entry) => now - entry.at < FEED_MS)
    return this.feed.map(({ killer, victim, weapon }) => ({
      killer, victim, weapon,
      you: killer === localId ? 'killer' : victim === localId ? 'victim' : null,
    }))
  }

  /** Everyone known (scored, in view, or you), best first. */
  rows(localId: number | null, inView: Iterable<number>): ScoreRow[] {
    const ids = new Set<number>(this.totals.keys())
    if (localId !== null) ids.add(localId)
    for (const id of inView) ids.add(id)
    return [...ids]
      .map((id) => ({ id, kills: this.kills(id), deaths: this.deaths(id), held: this.totals.get(id)?.held ?? 0, you: id === localId }))
      .sort((a, b) => b.kills - a.kills || a.deaths - b.deaths || a.id - b.id)
  }

  private entry(id: number) {
    let total = this.totals.get(id)
    if (!total) this.totals.set(id, (total = { kills: 0, deaths: 0, held: 0 }))
    return total
  }
}
