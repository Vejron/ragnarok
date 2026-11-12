// Turns the race's GPX into `course.json` for a built world: the line in the world's metres, resampled,
// with its kilometre markers, start, finish and where the snowmobiles are parked.
//
//   node scripts/course.ts --gpx ../../worlds/umeloppet/course.gpx --world ../../worlds/umeloppet [--name "Umeloppet 15 km"] [--length 20000]
//
// `--length` gives the course the length the race says it has, metres, when the GPX measures otherwise:
// every distance along it (the readout, the kilometre signs, the profile) is stretched to match, and
// the line itself stays where it is.
//
// Writes `<world>/course.json` unless `--out` says where. Run it again whenever the GPX or the world changes.
import { readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { join, resolve } from 'node:path'
import { parseArgs } from 'node:util'

/** Mirrors `crates/shared/src/world.rs`: metres per degree, and the one latitude longitude is scaled at. */
const METRES_PER_DEG_LAT = 110_540
const METRES_PER_DEG_LON = 111_320
const SCALE_LATITUDE = 63.8225
/** The course line is resampled to a point about this often, metres. */
const SPACING = 5
/** Snowmobiles stand this far to the right of the start line, metres, this far apart. */
const SLED_SIDE = 9
const SLED_GAP = 3.5
const SLED_COUNT = 2

const { values } = parseArgs({
  options: {
    gpx: { type: 'string' },
    world: { type: 'string' },
    out: { type: 'string' },
    name: { type: 'string' },
    length: { type: 'string' },
  },
})
if (!values.gpx || !values.world) {
  console.error('usage: node scripts/course.ts --gpx <file.gpx> --world <world dir> [--out <course.json>] [--name <name>]')
  process.exit(1)
}

// The manifest's frame, read with the client's own msgpack.
const require = createRequire(new URL('../../../client/package.json', import.meta.url))
const { decode } = (await import(require.resolve('@msgpack/msgpack'))) as typeof import('@msgpack/msgpack')
const manifest = decode(readFileSync(join(values.world, 'world.msgpack'))) as [number, [number, number, number], number, number]
const [kind, originLat, originLon] = manifest[1]
if (kind !== 0) throw new Error('only a Swedish world (an equirectangular frame) is supported')
const halfExtent = manifest[2] / 100
const tileSize = manifest[3] / 100
const groundCell = (manifest as unknown as number[])[6]! / 100

/** Ground heights of each tile, metres, row-major with rows along +Z, read as the client's `decodeTile` does. */
const grounds = new Map<string, Float32Array | null>()
const groundOf = (col: number, row: number): Float32Array | null => {
  const key = `${col}_${row}`
  if (!grounds.has(key)) {
    let heights: Float32Array | null = null
    try {
      const tile = decode(readFileSync(join(values.world!, 'tiles', `${key}.msgpack`))) as unknown[]
      const bin = tile[3] as Uint8Array
      // A copy, since Node hands out a view into a shared pool (Buffer.slice copies nothing).
      const cm = new Int32Array(new Uint8Array(bin).buffer, 0, bin.length >> 2)
      if (cm.length > 0) heights = Float32Array.from(cm, (v) => v / 100)
    } catch {
      heights = null
    }
    grounds.set(key, heights)
  }
  return grounds.get(key)!
}

/** The world's ground height at `(x, z)`, metres, from the height model it was built from; null outside its tiles. */
const groundAt = (x: number, z: number): number | null => {
  const col = Math.floor((x + halfExtent) / tileSize)
  const row = Math.floor((z + halfExtent) / tileSize)
  const heights = groundOf(col, row)
  if (!heights) return null
  const side = Math.round(tileSize / groundCell) + 1
  const u = (x + halfExtent - col * tileSize) / groundCell
  const v = (z + halfExtent - row * tileSize) / groundCell
  const i = Math.min(side - 2, Math.max(0, Math.floor(u)))
  const j = Math.min(side - 2, Math.max(0, Math.floor(v)))
  const fu = Math.min(1, Math.max(0, u - i))
  const fv = Math.min(1, Math.max(0, v - j))
  const at = (a: number, b: number) => heights[b * side + a]!
  const top = at(i, j) + (at(i + 1, j) - at(i, j)) * fu
  const bottom = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu
  return top + (bottom - top) * fv
}
const metresPerDegLon = METRES_PER_DEG_LON * Math.cos((SCALE_LATITUDE * Math.PI) / 180)
const project = (lat: number, lon: number): [number, number] => [(lon - originLon) * metresPerDegLon, -(lat - originLat) * METRES_PER_DEG_LAT]

// Track points, else route points, in order, with their elevations where the GPX has them.
const gpx = readFileSync(values.gpx, 'utf8')
const read = (tag: string) =>
  [...gpx.matchAll(new RegExp(`<${tag}\\b([^>]*?)(?:/>|>([\\s\\S]*?)</${tag}>)`, 'g'))].map(([, attributes, body]) => {
    const lat = /\blat\s*=\s*["']([-\d.]+)["']/.exec(attributes!)?.[1]
    const lon = /\blon\s*=\s*["']([-\d.]+)["']/.exec(attributes!)?.[1]
    const ele = /<ele>\s*([-\d.]+)\s*<\/ele>/.exec(body ?? '')?.[1]
    return { at: [Number(lat), Number(lon)] as [number, number], ele: ele === undefined ? null : Number(ele) }
  })
let found = read('trkpt')
if (found.length < 2) found = read('rtept')
if (found.length < 2) throw new Error(`${values.gpx}: no track or route points`)
const latLon = found.map(({ at }) => at)
const elevations = found.every(({ ele }) => ele !== null) ? found.map(({ ele }) => ele!) : null
const name = values.name ?? /<name>([^<]+)<\/name>/.exec(gpx)?.[1]?.trim() ?? 'Umeloppet'

// Into metres, without repeated points, then every SPACING metres along.
const kept = latLon.map(([lat, lon]) => project(lat, lon)).map((p, i) => ({ p, ele: elevations?.[i] ?? 0 }))
  .filter(({ p: [x, z] }, i, all) => i === 0 || Math.hypot(x - all[i - 1]!.p[0], z - all[i - 1]!.p[1]) > 0.01)
const raw = kept.map(({ p }) => p)
const rawAlong = [0]
for (let i = 1; i < raw.length; i++) rawAlong.push(rawAlong[i - 1]! + Math.hypot(raw[i]![0] - raw[i - 1]![0], raw[i]![1] - raw[i - 1]![1]))
const length = rawAlong[rawAlong.length - 1]!
const pointAt = (distance: number): [number, number] => {
  let i = 0
  while (i < raw.length - 2 && rawAlong[i + 1]! < distance) i++
  const t = (distance - rawAlong[i]!) / Math.max(1e-6, rawAlong[i + 1]! - rawAlong[i]!)
  const [ax, az] = raw[i]!
  const [bx, bz] = raw[i + 1]!
  return [ax + (bx - ax) * t, az + (bz - az) * t]
}
/** The GPX's elevation at `distance` along, metres above the sea. */
const heightAt = (distance: number): number => {
  let i = 0
  while (i < raw.length - 2 && rawAlong[i + 1]! < distance) i++
  const t = Math.min(1, Math.max(0, (distance - rawAlong[i]!) / Math.max(1e-6, rawAlong[i + 1]! - rawAlong[i]!)))
  return kept[i]!.ele + (kept[i + 1]!.ele - kept[i]!.ele) * t
}
const steps = Math.max(1, Math.round(length / SPACING))
const along = Array.from({ length: steps + 1 }, (_, i) => (length * i) / steps)
const points = along.map(pointAt)
const round = (v: number) => Math.round(v * 10) / 10

/** The soldier's yaw (0 faces north, -Z) along the course at `distance`. */
const yawAt = (distance: number) => {
  const [ax, az] = pointAt(Math.max(0, distance - SPACING))
  const [bx, bz] = pointAt(Math.min(length, distance + SPACING))
  return Math.atan2(-(bx - ax), -(bz - az))
}
const facing = (distance: number) => {
  const [x, z] = pointAt(distance)
  return { x: round(x), z: round(z), yaw: Math.round(yawAt(distance) * 1000) / 1000 }
}

const start = facing(0)
/** The race's length, metres, and how much longer that is than the line measures. */
const stated = values.length ? Number(values.length) : length
if (!(stated > 0)) throw new Error(`--length ${values.length}: not a length in metres`)
const stretch = stated / length
const km = Array.from({ length: Math.floor(stated / 1000) }, (_, i) => {
  const [x, z] = pointAt(((i + 1) * 1000) / stretch)
  return { x: round(x), z: round(z), km: i + 1 }
}).filter(({ km }) => km * 1000 < stated - 200)

// Side by side to the right of the start, facing along the course.
const right: [number, number] = [Math.cos(start.yaw), -Math.sin(start.yaw)]
const back: [number, number] = [Math.sin(start.yaw), Math.cos(start.yaw)]
const snowmobiles = Array.from({ length: SLED_COUNT }, (_, i) => ({
  x: round(start.x + right[0] * (SLED_SIDE + i * SLED_GAP) + back[0] * 4),
  z: round(start.z + right[1] * (SLED_SIDE + i * SLED_GAP) + back[1] * 4),
  yaw: start.yaw,
}))

// Heights from the world's own ground (the height model), which the skier also feels; the GPX's
// elevations, which a watch's barometer or GPS made up, only where a point lies outside the tiles.
let fromGpx = 0
const heights = points.map(([x, z], i) => {
  const ground = groundAt(x, z)
  if (ground !== null) return round(ground)
  fromGpx++
  return elevations ? round(heightAt(along[i]!)) : 0
})
if (fromGpx > 0) console.warn(`warning: ${fromGpx} points have no ground under them; ${elevations ? "the GPX's elevations" : '0 m'} used there`)

const outside = points.filter(([x, z]) => Math.abs(x) > halfExtent || Math.abs(z) > halfExtent).length
if (outside > 0) console.warn(`warning: ${outside} of ${points.length} points lie outside the world (half extent ${halfExtent} m): paint more tiles`)

const course = {
  name,
  length: Math.round(stated),
  points: points.map(([x, z]) => [round(x), round(z)]),
  along: along.map((d) => round(d * stretch)),
  heights,
  km,
  start,
  finish: facing(length),
  snowmobiles,
}
const out = resolve(values.out ?? join(values.world, 'course.json'))
writeFileSync(out, JSON.stringify(course))
console.log(`${out}: "${name}", ${(stated / 1000).toFixed(2)} km${stretch !== 1 ? ` (measured ${(length / 1000).toFixed(2)} km, stretched ×${stretch.toFixed(3)})` : ''}, ${points.length} points, ${km.length} km markers, heights ${Math.min(...heights)}–${Math.max(...heights)} m from the ground`)
