import { decode } from '@msgpack/msgpack'

/** Mirrors `crates/shared/src/world.rs`. Coordinates are metres: +X east, +Z south, +Y up. */

export const WORLD_VERSION = 1

export const ROAD_MAJOR = 0
export const ROAD_MINOR = 1
export const ROAD_PATH = 2

export type Point2 = [number, number]

export interface Building {
  height: number
  footprint: Point2[]
}

export interface Road {
  width: number
  kind: number
  points: Point2[]
}

export interface WorldData {
  origin: [number, number]
  halfExtent: number
  buildings: Building[]
  roads: Road[]
  spawns: Point2[]
  collision: { vertices: Float32Array; triangles: Uint32Array }
}

/** FNV-1a, matching `world_hash` on the server. */
export function worldHash(bytes: Uint8Array): number {
  let hash = 0x811c9dc5
  for (const byte of bytes) hash = Math.imul(hash ^ byte, 0x01000193) >>> 0
  return hash
}

const metres = (cm: number) => cm / 100
const point = ([x, z]: number[]): Point2 => [metres(x!), metres(z!)]

export function decodeWorld(bytes: Uint8Array): WorldData {
  const raw = decode(bytes) as [number, [number, number], number, [number, number[][]][], [number, number, number[][]][], number[][], [number[][], number[][]]]
  const [version, origin, halfExtent, buildings, roads, spawns, [vertices, triangles]] = raw
  if (version !== WORLD_VERSION) throw new Error(`world version ${version}, expected ${WORLD_VERSION}`)
  return {
    origin,
    halfExtent: metres(halfExtent),
    buildings: buildings.map(([height, footprint]) => ({ height: metres(height), footprint: footprint.map(point) })),
    roads: roads.map(([width, kind, points]) => ({ width: metres(width), kind, points: points.map(point) })),
    spawns: spawns.map(point),
    collision: {
      vertices: Float32Array.from(vertices.flat(), metres),
      triangles: Uint32Array.from(triangles.flat()),
    },
  }
}

export async function loadWorld(url = '/world.msgpack'): Promise<{ world: WorldData; hash: number }> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}; run \`cargo run -p osm-import\``)
  const bytes = new Uint8Array(await response.arrayBuffer())
  return { world: decodeWorld(bytes), hash: worldHash(bytes) }
}
