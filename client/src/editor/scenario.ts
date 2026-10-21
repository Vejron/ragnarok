/**
 * A scenario as `worlds/<name>/scenario.json` holds it (BOTS.md, SCENARIOS.md), and the checks the
 * server makes when it loads one, so the editor can point at a mistake before it is saved.
 */

export interface Spot {
  lat: number
  lon: number
  /** Walked in order, looping; empty or missing: it makes up its own. */
  route?: { lat: number; lon: number }[]
}

export interface ScenarioArea {
  lat: number
  lon: number
  /** Metres. */
  radius: number
}

export interface ScenarioSquad {
  /** 1 and up: squads on the same team leave each other alone. */
  team: number
  /** 0 to 1. */
  skill: number
  /** Item names, one a slot, at most `MAX_LOADOUT`. Missing: the standard loadout. */
  loadout?: string[]
  respawn?: boolean
  /** Missing in the file counts as none. */
  bots?: Spot[]
  tanks?: Spot[]
}

export interface When {
  start?: boolean
  /** Seconds since the scenario started. */
  after?: number
  enter?: string
  /** With `enter`: a squad, and only its members count. */
  by?: string
  cleared?: string
  done?: string
}

export type Action =
  | { spawn: string }
  | { remove: string }
  | { say: string }
  | { objective: string }
  | { win: string }
  | { lose: string }

export interface Trigger {
  name?: string
  when: When
  /** Seconds between its conditions coming true and its actions. */
  wait?: number
  /** How many times it may fire; 0 is no limit. */
  times?: number
  do: Action[]
}

export interface Scenario {
  /** Seconds from the end until it starts again. */
  restart?: number
  areas: Record<string, ScenarioArea>
  squads: Record<string, ScenarioSquad>
  triggers: Trigger[]
}

export const MAX_LOADOUT = 5
export const MAX_TEXT = 200
/** Item names the server knows for a loadout, as the editor's item list spells them. */
export const LOADOUT_ITEMS = ['Revolver', 'AK5', 'Sniper', 'Shotgun', 'Grenade', 'Bandage', 'Golden egg', 'FPV drone', 'Loitering drone', 'Flare gun']
/** What a name refers to: an area, a squad or a named trigger. */
export type NameKind = 'area' | 'squad' | 'trigger'

export const emptyScenario = (): Scenario => ({ restart: 20, areas: {}, squads: {}, triggers: [] })

/** One problem: where it is (like `squads.tankers.tanks[0]`) and what is wrong. */
export interface Issue {
  path: string
  message: string
}

/** The actions that take a squad's name. */
const SQUAD_ACTIONS = new Set<string>(['spawn', 'remove'])

/** Problems with `scenario`. `inside` says whether a position is in the world. */
export function validateScenario(scenario: Scenario, inside: (lat: number, lon: number) => boolean): Issue[] {
  const issues: Issue[] = []
  const add = (path: string, message: string) => issues.push({ path, message })
  const names = (kind: NameKind): string[] => {
    if (kind === 'area') return Object.keys(scenario.areas)
    if (kind === 'squad') return Object.keys(scenario.squads)
    return scenario.triggers.flatMap((t) => (t.name ? [t.name] : []))
  }
  const placed = (path: string, spot: { lat: number; lon: number }) => {
    if (!inside(spot.lat, spot.lon)) add(path, 'is outside the world')
  }

  if (!((scenario.restart ?? 0) >= 0)) add('restart', 'must not be negative')

  for (const [name, area] of Object.entries(scenario.areas)) {
    const path = `areas.${name}`
    if (!name.trim()) add(path, 'needs a name')
    placed(path, area)
    if (!(area.radius > 0)) add(`${path}.radius`, 'must be more than 0 metres')
  }

  for (const [name, squad] of Object.entries(scenario.squads)) {
    const path = `squads.${name}`
    if (!name.trim()) add(path, 'needs a name')
    if (!Number.isInteger(squad.team) || squad.team < 1) add(`${path}.team`, 'must be 1 or more')
    if (!(squad.skill >= 0 && squad.skill <= 1)) add(`${path}.skill`, 'must be between 0 and 1')
    if (squad.loadout) {
      if (squad.loadout.length > MAX_LOADOUT) add(`${path}.loadout`, `has at most ${MAX_LOADOUT} items`)
      squad.loadout.forEach((item, i) => {
        if (!LOADOUT_ITEMS.some((known) => known.toLowerCase() === item.toLowerCase())) add(`${path}.loadout[${i}]`, `no item called "${item}"`)
      })
    }
    if (!squad.bots?.length && !squad.tanks?.length) add(path, 'needs at least one bot or tank')
    for (const [kind, spots] of [
      ['bots', squad.bots ?? []],
      ['tanks', squad.tanks ?? []],
    ] as const) {
      spots.forEach((spot, i) => {
        placed(`${path}.${kind}[${i}]`, spot)
        spot.route?.forEach((point, j) => placed(`${path}.${kind}[${i}].route[${j}]`, point))
      })
    }
  }

  scenario.triggers.forEach((trigger, n) => {
    const path = trigger.name ? `triggers.${trigger.name}` : `triggers[${n}]`
    if (trigger.name && scenario.triggers.findIndex((t) => t.name === trigger.name) !== n) add(path, 'two triggers have this name')
    const w = trigger.when
    const reference = (kind: NameKind, value: string | undefined, field: string) => {
      if (value !== undefined && !names(kind).includes(value)) add(`${path}.when.${field}`, `there is no ${kind} called "${value}"`)
    }
    if (w.by !== undefined && w.enter === undefined) add(`${path}.when.by`, '"by" goes with "enter"')
    reference('area', w.enter, 'enter')
    reference('squad', w.by, 'by')
    reference('squad', w.cleared, 'cleared')
    reference('trigger', w.done, 'done')
    if (w.after !== undefined && !(w.after >= 0)) add(`${path}.when.after`, 'must not be negative')
    if (!w.start && w.after === undefined && w.enter === undefined && w.cleared === undefined && w.done === undefined) {
      add(path, 'needs a condition: start, after, enter, cleared or done')
    }
    if (trigger.wait !== undefined && !(trigger.wait >= 0)) add(`${path}.wait`, 'must not be negative')
    if (trigger.times !== undefined && !(Number.isInteger(trigger.times) && trigger.times >= 0)) add(`${path}.times`, 'must be a whole number, 0 or more')
    trigger.do.forEach((action, i) => {
      const [key, value] = Object.entries(action)[0] as [string, string]
      const at = `${path}.do[${i}]`
      if (SQUAD_ACTIONS.has(key)) {
        if (!names('squad').includes(value)) add(at, `there is no squad called "${value}"`)
      } else if (value.length > MAX_TEXT) {
        add(at, `is longer than ${MAX_TEXT} characters`)
      }
    })
  })
  return issues
}

/** The triggers that use `name` as a `kind`: described for a message ("trigger cleared", "trigger 3"). */
export function usesName(scenario: Scenario, kind: NameKind, name: string): string[] {
  const label = (t: Trigger, n: number) => `trigger ${t.name ?? n + 1}`
  return scenario.triggers.flatMap((t, n) => {
    const w = t.when
    const uses =
      kind === 'area'
        ? w.enter === name
        : kind === 'squad'
          ? w.by === name || w.cleared === name || t.do.some((a) => ('spawn' in a && a.spawn === name) || ('remove' in a && a.remove === name))
          : w.done === name
    return uses ? [label(t, n)] : []
  })
}

/**
 * The scenario with `kind` named `from` now called `to`, and every trigger that names it changed to
 * match. A `to` that is taken, or an empty one, changes nothing.
 */
export function renameName(scenario: Scenario, kind: NameKind, from: string, to: string): Scenario {
  const taken = kind === 'trigger' ? scenario.triggers.some((t) => t.name === to) : to in (kind === 'area' ? scenario.areas : scenario.squads)
  if (!to.trim() || from === to || taken) return scenario
  const next: Scenario = structuredClone(scenario)
  const swap = <T>(record: Record<string, T>) => {
    const entries = Object.entries(record).map(([key, value]) => [key === from ? to : key, value] as const)
    return Object.fromEntries(entries) as Record<string, T>
  }
  if (kind === 'area') next.areas = swap(next.areas)
  if (kind === 'squad') next.squads = swap(next.squads)
  for (const t of next.triggers) {
    if (kind === 'trigger' && t.name === from) t.name = to
    if (kind === 'area' && t.when.enter === from) t.when.enter = to
    if (kind === 'squad') {
      if (t.when.by === from) t.when.by = to
      if (t.when.cleared === from) t.when.cleared = to
      for (const action of t.do) {
        if ('spawn' in action && action.spawn === from) action.spawn = to
        if ('remove' in action && action.remove === from) action.remove = to
      }
    }
    if (kind === 'trigger' && t.when.done === from) t.when.done = to
  }
  return next
}

/** A bot as the map editor used to place it: always there, with its own team and skill. */
export interface LooseBot {
  lat: number
  lon: number
  team: number
  skill: number
  route: { lat: number; lon: number }[]
}

/**
 * The scenario with `bots` as squads, a squad for each team and skill, and one trigger that spawns
 * them all when the scenario starts. They respawn at home, as placed bots always did.
 */
export function squadsFromBots(scenario: Scenario, bots: LooseBot[]): Scenario {
  if (!bots.length) return scenario
  const groups = new Map<string, LooseBot[]>()
  for (const bot of bots) {
    const key = `${bot.team}/${bot.skill}`
    groups.set(key, [...(groups.get(key) ?? []), bot])
  }
  const skillsOf = (team: number) => new Set(bots.filter((b) => b.team === team).map((b) => b.skill)).size
  const squads = { ...scenario.squads }
  const spawned: string[] = []
  for (const group of groups.values()) {
    const { team, skill } = group[0]!
    const base = skillsOf(team) > 1 ? `placed-team-${team}-skill-${skill}` : `placed-team-${team}`
    let name = base
    for (let n = 2; name in squads; n++) name = `${base}-${n}`
    squads[name] = {
      team,
      skill,
      respawn: true,
      bots: group.map((b) => ({ lat: b.lat, lon: b.lon, ...(b.route.length ? { route: b.route } : {}) })),
      tanks: [],
    }
    spawned.push(name)
  }
  const trigger: Trigger = { when: { start: true }, do: spawned.map((spawn) => ({ spawn })) }
  return { ...scenario, squads, triggers: [...scenario.triggers, trigger] }
}
