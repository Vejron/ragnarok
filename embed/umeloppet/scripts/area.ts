// Paints a winter world's `area.json` around the race's GPX: every 256 m tile of the importer's grid
// within `--margin` metres of the course, and a spawn at the start. Build the world from it as any other
// (`cargo run -p osm-import -- --area <dir>/area.json --out <dir>`), then run `scripts/course.ts`.
//
//   node scripts/area.ts --gpx ../../worlds/umeloppet/course.gpx --out ../../worlds/umeloppet/area.json [--margin 400]
//
// It will not overwrite an `area.json` (which may hold placements made in the editor) without `--force`.
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { parseArgs } from 'node:util'

/** Mirrors `crates/osm-import/src/grid.rs` and `crates/shared/src/world.rs`: Sweden's grid. */
const ORIGIN: [number, number] = [63.8225, 20.235]
const TILE = 256
const METRES_PER_DEG_LAT = 110_540
const METRES_PER_DEG_LON = 111_320
const SCALE_LATITUDE = 63.8225
/** The importer's largest world, tiles a side (`MAX_SIDE`). */
const MAX_SIDE = 120

const { values } = parseArgs({
  options: {
    gpx: { type: 'string' },
    out: { type: 'string' },
    margin: { type: 'string', default: '400' },
    force: { type: 'boolean', default: false },
  },
})
if (!values.gpx || !values.out) {
  console.error('usage: node scripts/area.ts --gpx <file.gpx> --out <world dir>/area.json [--margin <metres>] [--force]')
  process.exit(1)
}
if (existsSync(values.out) && !values.force) {
  console.error(`${values.out} exists; pass --force to replace it`)
  process.exit(1)
}
const margin = Number(values.margin)

const metresPerDegLon = METRES_PER_DEG_LON * Math.cos((SCALE_LATITUDE * Math.PI) / 180)
const project = (lat: number, lon: number): [number, number] => [(lon - ORIGIN[1]) * metresPerDegLon, -(lat - ORIGIN[0]) * METRES_PER_DEG_LAT]

const gpx = readFileSync(values.gpx, 'utf8')
const read = (tag: string) =>
  [...gpx.matchAll(new RegExp(`<${tag}\\b([^>]*)>`, 'g'))].map(([, attributes]) => [
    Number(/\blat\s*=\s*["']([-\d.]+)["']/.exec(attributes!)?.[1]),
    Number(/\blon\s*=\s*["']([-\d.]+)["']/.exec(attributes!)?.[1]),
  ] as [number, number])
let latLon = read('trkpt')
if (latLon.length < 2) latLon = read('rtept')
if (latLon.length < 2) throw new Error(`${values.gpx}: no track or route points`)

// Every tile whose square comes within `margin` of the line, found by walking the line in short steps.
const tiles = new Set<string>()
const reach = Math.ceil(margin / TILE)
const points = latLon.map(([lat, lon]) => project(lat, lon))
for (let i = 0; i < points.length; i++) {
  const [ax, az] = points[i]!
  const [bx, bz] = points[Math.min(points.length - 1, i + 1)]!
  const steps = Math.max(1, Math.ceil(Math.hypot(bx - ax, bz - az) / 20))
  for (let s = 0; s <= steps; s++) {
    const x = ax + ((bx - ax) * s) / steps
    const z = az + ((bz - az) * s) / steps
    const [gx, gy] = [Math.floor(x / TILE), Math.floor(z / TILE)]
    for (let dy = -reach; dy <= reach; dy++) {
      for (let dx = -reach; dx <= reach; dx++) {
        // The nearest point of tile (gx+dx, gy+dy) to (x, z).
        const nx = Math.min(Math.max(x, (gx + dx) * TILE), (gx + dx + 1) * TILE)
        const nz = Math.min(Math.max(z, (gy + dy) * TILE), (gy + dy + 1) * TILE)
        if (Math.hypot(nx - x, nz - z) <= margin) tiles.add(`${gx + dx},${gy + dy}`)
      }
    }
  }
}
const list = [...tiles].map((key) => key.split(',').map(Number) as [number, number]).sort((a, b) => a[0] - b[0] || a[1] - b[1])
const side = Math.max(...list.map(([gx]) => gx)) - Math.min(...list.map(([gx]) => gx)) + 1
const tall = Math.max(...list.map(([, gy]) => gy)) - Math.min(...list.map(([, gy]) => gy)) + 1
if (Math.max(side, tall) > MAX_SIDE) throw new Error(`the course spans ${side} x ${tall} tiles; the importer takes at most ${MAX_SIDE} a side`)

const [lat, lon] = latLon[0]!
const area = {
  theme: 'winter',
  groomed_paths: false,
  cell: 1.0,
  tiles: list.map(([gx, gy]) => ({ gx, gy })),
  spawns: [{ lat, lon }],
  vehicles: [],
  items: [],
  bots: [],
}
writeFileSync(values.out, JSON.stringify(area))
console.log(`${values.out}: ${list.length} tiles (${((list.length * TILE * TILE) / 1e6).toFixed(1)} km²) in a ${side} x ${tall} square`)
