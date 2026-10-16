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
export interface Placements {
  spawns: Spawn[]
  vehicles: PlacedVehicle[]
  items: PlacedItem[]
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
]

export const HEADINGS = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'] as const

/** A compass heading index (0 = north, clockwise, 45° each) as the simulation's yaw. */
export const yawOf = (heading: number): number => (heading === 0 ? 0 : -(heading * Math.PI) / 4)

/** The compass heading index nearest a yaw. */
export const headingOf = (yaw: number): number => (((Math.round((-yaw * 4) / Math.PI) % 8) + 8) % 8)

export const emptyPlacements = (): Placements => ({ spawns: [], vehicles: [], items: [] })

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
