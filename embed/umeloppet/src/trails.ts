import { decode, encode } from '@msgpack/msgpack'
import type { TileData, TileRef, WorldData } from '@/world'
import type { TileSink } from '@/stream'
import type { Course } from './course'

/** How wide the course is groomed, metres: as wide as the importer grooms a track (`SKI_TRAIL_WIDTH`). */
const COURSE_WIDTH = 4
/** Where `trees` and `ski` are in a tile file's array (`TileFile` in `crates/shared/src/world.rs`, `decodeTile`). */
const TILE_TREES = 5
const TILE_SKI = 11
/** No trunk stands nearer the course's centre line than this, metres; a rock's edge neither. */
const CLEAR_OF_TREES = 4
/** Rocks among a tile's trees (`TREE_ROCK_BOULDER`, `TREE_ROCK_SLAB`), whose `crown` is their radius. */
const ROCKS = new Set([7, 8])

/**
 * The course as the only groomed ski track, in every tile, as if the importer had found it alone in
 * the map: the skis snap into its grooves and glide faster on it (`ski::SkiMap` in the WASM sim), and it
 * is drawn as the world's tracks are. The world's own tracks (OpenStreetMap's ski ways and, in a world
 * with `groomed_paths`, its forest paths) are taken out, so the race line is the one track in the snow,
 * and the trees and rocks standing on it are cleared.
 * A tile is changed twice over on its way in: its decoded `ski` for the drawing, and its bytes for the
 * sim, which reads the file for itself.
 */
export function groomCourse(course: Course, world: WorldData, sink: TileSink): TileSink {
  const groom = (ref: TileRef, tile: TileData) => {
    const runs = runsIn(course, world, ref)
    const onCourse = clearing(course, world, ref)
    const cleared = tile.trees.filter((tree) => onCourse(tree.x, tree.z, ROCKS.has(tree.kind) ? tree.crown : 0)).length
    if (runs.length === 0 && tile.ski.length === 0 && cleared === 0) return
    tile.ski = runs.map((points) => ({ width: COURSE_WIDTH, points }))
    if (cleared > 0) tile.trees = tile.trees.filter((tree) => !onCourse(tree.x, tree.z, ROCKS.has(tree.kind) ? tree.crown : 0))
    if (tile.bytes) {
      const file = decode(tile.bytes) as unknown[]
      const cm = (v: number) => Math.round(v * 100)
      file[TILE_SKI] = runs.map((points) => [cm(COURSE_WIDTH), points.map(([x, z]) => [cm(x), cm(z)])])
      if (cleared > 0) {
        // [x, z, base, height, crown, kind, solid], centimetres.
        const trees = file[TILE_TREES] as [number, number, number, number, number, number, boolean][]
        file[TILE_TREES] = trees.filter(([x, z, , , crown, kind]) => !onCourse(x / 100, z / 100, ROCKS.has(kind) ? crown / 100 : 0))
      }
      tile.bytes = encode(file)
    }
  }
  // Groomed once, in `prepare` when there is one (it always comes first), else in `attach`.
  const groomed = new WeakSet<TileData>()
  const once = (ref: TileRef, tile: TileData) => {
    if (groomed.has(tile)) return
    groomed.add(tile)
    groom(ref, tile)
  }
  return {
    prepare: sink.prepare && ((ref, tile) => (once(ref, tile), sink.prepare!(ref, tile))),
    attach: (ref, tile) => (once(ref, tile), sink.attach(ref, tile)),
    discard: sink.discard && ((ref) => sink.discard!(ref)),
  }
}

/**
 * The stretches of the course in tile `ref`: each piece of the line goes to the tile its middle is in,
 * and runs of pieces in a row are joined into one track.
 */
function runsIn(course: Course, world: WorldData, ref: TileRef): [number, number][][] {
  const left = -world.halfExtent + ref.col * world.tileSize
  const top = -world.halfExtent + ref.row * world.tileSize
  const inside = ([ax, az]: [number, number], [bx, bz]: [number, number]) => {
    const [x, z] = [(ax + bx) / 2, (az + bz) / 2]
    return x >= left && x < left + world.tileSize && z >= top && z < top + world.tileSize
  }
  const runs: [number, number][][] = []
  let run: [number, number][] | null = null
  const points = course.points
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    if (!inside(a, b)) {
      run = null
      continue
    }
    if (!run) runs.push((run = [a]))
    run.push(b)
  }
  return runs
}

/**
 * Whether something standing at `(x, z)` in tile `ref`, `radius` metres round, is on the course: nearer
 * its centre line than `CLEAR_OF_TREES` plus its radius.
 */
function clearing(course: Course, world: WorldData, ref: TileRef): (x: number, z: number, radius: number) => boolean {
  const margin = CLEAR_OF_TREES + 10
  const left = -world.halfExtent + ref.col * world.tileSize - margin
  const top = -world.halfExtent + ref.row * world.tileSize - margin
  const size = world.tileSize + 2 * margin
  const near = ([x, z]: [number, number]) => x >= left && x < left + size && z >= top && z < top + size
  const segments: [number, number, number, number][] = []
  const points = course.points
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    if (near(a) || near(b)) segments.push([a[0], a[1], b[0], b[1]])
  }
  return (x, z, radius) => {
    const reach = CLEAR_OF_TREES + radius
    for (const [ax, az, bx, bz] of segments) {
      const dx = bx - ax
      const dz = bz - az
      const t = Math.min(1, Math.max(0, ((x - ax) * dx + (z - az) * dz) / Math.max(1e-6, dx * dx + dz * dz)))
      if (Math.hypot(ax + dx * t - x, az + dz * t - z) < reach) return true
    }
    return false
  }
}
