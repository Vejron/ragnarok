/** Things placed by hand in the editor, saved in the world's `area.json` as latitude/longitude. */
export interface Spawn {
  lat: number
  lon: number
}
export interface PlacedVehicle {
  kind: number
  lat: number
  lon: number
  /** Radians, as the simulation counts them: 0 faces north (-Z), positive turns left. */
  yaw: number
}
export interface PlacedItem {
  item: number
  count: number
  lat: number
  lon: number
}
/** A bot, as BOTS.md describes it: where it lives, its team and skill, and the waypoints it walks round. */
export interface PlacedBot {
  lat: number
  lon: number
  /** 1 and up: bots on the same team leave each other alone. */
  team: number
  /** 0 (a raw recruit) to 1 (a marksman). */
  skill: number
  /** Walked in order, looping. Empty: it makes up a loop near home. Only its own spot: it stands guard. */
  route: Spawn[]
}
export interface Placements {
  spawns: Spawn[]
  vehicles: PlacedVehicle[]
  items: PlacedItem[]
  bots: PlacedBot[]
}

/** The `KIND_*` ids of crates/shared/src/protocol.rs that are vehicles. */
export const VEHICLE_KINDS: { kind: number; name: string; icon: string }[] = [
  { kind: 1, name: 'Jeep', icon: 'J' },
  { kind: 4, name: 'Sports car', icon: 'S' },
  { kind: 10, name: 'Roadster', icon: 'R' },
  { kind: 2, name: 'Tank', icon: 'T' },
  { kind: 8, name: 'Strv', icon: 'V' },
  { kind: 3, name: 'Helicopter', icon: 'H' },
  { kind: 6, name: 'Plane', icon: 'P' },
  { kind: 7, name: 'Walker', icon: 'W' },
  { kind: 12, name: 'Hen', icon: 'E' },
  { kind: 9, name: 'Mortar', icon: 'M' },
]

/** The `ITEM_*` ids of crates/shared/src/items.rs. */
export const ITEM_KINDS: { item: number; name: string; count: number }[] = [
  { item: 1, name: 'Revolver', count: 1 },
  { item: 2, name: 'AK5', count: 1 },
  { item: 5, name: 'Sniper', count: 1 },
  { item: 7, name: 'Shotgun', count: 1 },
  { item: 3, name: 'Grenade', count: 2 },
  { item: 4, name: 'Bandage', count: 2 },
  { item: 6, name: 'Golden egg', count: 1 },
  { item: 8, name: 'FPV drone', count: 1 },
  { item: 9, name: 'Loitering drone', count: 1 },
  { item: 10, name: 'Flare gun', count: 2 },
  { item: 11, name: 'Bricks', count: 1 },
  { item: 12, name: 'Carl Gustaf', count: 3 },
]

/** The teams a bot can be put on, and the colour each is drawn in. */
export const BOT_TEAMS: { team: number; colour: string }[] = [
  { team: 1, colour: '#7b1fa2' },
  { team: 2, colour: '#00838f' },
  { team: 3, colour: '#ef6c00' },
  { team: 4, colour: '#558b2f' },
]
export const DEFAULT_SKILL = 0.5

export const teamColour = (team: number): string => BOT_TEAMS.find((t) => t.team === team)?.colour ?? '#555'

/** What a bot's route makes it do, in a few words. */
export function routeSummary(bot: PlacedBot): string {
  if (!bot.route.length) return 'makes up its own loop near home'
  if (isGuard(bot)) return 'stands guard at home'
  return `walks ${bot.route.length} waypoint${bot.route.length === 1 ? '' : 's'}`
}

/** Whether the route is just its own spot. */
export const isGuard = (bot: PlacedBot): boolean => bot.route.length === 1 && bot.route[0]!.lat === bot.lat && bot.route[0]!.lon === bot.lon

/**
 * The lines that show a bot's route: from home to the first waypoint (`lead`), and round the
 * waypoints back to the first (`loop`). Either is empty when there is nothing to draw.
 */
export function routeLines(bot: PlacedBot): { lead: [number, number][]; loop: [number, number][] } {
  const points = bot.route.map((p): [number, number] => [p.lat, p.lon])
  const first = points[0]
  if (!first || isGuard(bot)) return { lead: [], loop: [] }
  const atHome = first[0] === bot.lat && first[1] === bot.lon
  return { lead: atHome ? [] : [[bot.lat, bot.lon], first], loop: points.length > 1 ? [...points, first] : [] }
}

export const HEADINGS = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'] as const

/** A compass heading index (0 = north, clockwise, 45° each) as the simulation's yaw. */
export const yawOf = (heading: number): number => (heading === 0 ? 0 : -(heading * Math.PI) / 4)

/** The compass heading index nearest a yaw. */
export const headingOf = (yaw: number): number => (((Math.round((-yaw * 4) / Math.PI) % 8) + 8) % 8)

export const emptyPlacements = (): Placements => ({ spawns: [], vehicles: [], items: [], bots: [] })

/** Index of the point nearest `(x, y)` within `reach`, or -1. */
export function nearest(points: { x: number; y: number }[], x: number, y: number, reach: number): number {
  let best = -1
  let bestDistance = reach
  points.forEach((p, i) => {
    const d = Math.hypot(p.x - x, p.y - y)
    if (d <= bestDistance) {
      best = i
      bestDistance = d
    }
  })
  return best
}
