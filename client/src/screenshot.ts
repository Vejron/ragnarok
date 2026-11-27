import type { Building, Point2, TileRef } from '@/world'

/*
 * Debug screenshots (F2). The frame is drawn and read back in the same task, so the picture is the
 * frame just drawn, and what it shows is written into the PNG itself as a JSON `iTXt` chunk under
 * `SHOT_KEYWORD`: the world, the camera (position, direction, field of view and its view and
 * projection matrices), the player, and what the crosshair ray hits, with the building there. In
 * dev the shot is saved under `shots/` at the repo root by Vite (`vite.config.ts`); otherwise it is
 * downloaded. `node client/scripts/shot-info.mjs <png>` prints what a shot says.
 */

/** The keyword of the PNG text chunk a shot's JSON is in. */
export const SHOT_KEYWORD = 'ragnarok'
/** How far the crosshair ray reaches, metres. */
const LOOK_RANGE = 1000
/** A building is the one looked at when the ray's hit lies in its footprint or this close to it, metres. */
const NEAR_BUILDING = 2

type Vec3 = [number, number, number]

/** A building of a loaded tile, as the shot names it. */
export interface TiledBuilding {
  ref: TileRef
  index: number
  building: Building
}

/** What a shot is taken of: the game's parts it reads. */
export interface ShotSource {
  canvas: HTMLCanvasElement
  camera: {
    position: { x: number; y: number; z: number }
    fov: number
    aspect: number
    view: Float32Array
    projection: Float32Array
    getWorldDirection<T extends { x: number; y: number; z: number }>(out: T): T
    updateMatrices(): void
  }
  /** Where the world is served from (`WORLD_BASE`), and its manifest's hash. */
  world: () => { base: string; hash: number }
  /** The time of day: 0 midnight, 0.5 noon. */
  phase: () => number
  player: () => Vec3 | null
  castRay: (origin: Vec3, dir: Vec3, max: number) => { t: number } | null
  buildings: () => Iterable<TiledBuilding>
  /** Tells the player where the shot went. */
  say: (text: string) => void
}

/** The building `p` is under (inside its footprint and in none of its yards), else the one with the nearest wall within `NEAR_BUILDING`. */
export function buildingAt(p: Point2, buildings: Iterable<TiledBuilding>): TiledBuilding | null {
  let best: TiledBuilding | null = null
  let nearest = NEAR_BUILDING
  for (const b of buildings) {
    const ring = b.building.footprint
    const yards = b.building.holes
    if (inside(p, ring) && !yards.some((yard) => inside(p, yard))) return b
    const d = Math.min(distanceTo(p, ring), ...yards.map((yard) => distanceTo(p, yard)))
    if (d <= nearest) {
      nearest = d
      best = b
    }
  }
  return best
}

function inside([x, z]: Point2, ring: readonly Point2[]): boolean {
  let hit = false
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [a, b] = [ring[i]!, ring[j]!]
    if (a[1] > z !== b[1] > z && x < ((b[0] - a[0]) * (z - a[1])) / (b[1] - a[1]) + a[0]) hit = !hit
  }
  return hit
}

function distanceTo([x, z]: Point2, ring: readonly Point2[]): number {
  let best = Infinity
  for (let i = 0; i < ring.length; i++) {
    const [a, b] = [ring[i]!, ring[(i + 1) % ring.length]!]
    const [dx, dz] = [b[0] - a[0], b[1] - a[1]]
    const length = dx * dx + dz * dz
    const t = length > 0 ? Math.max(0, Math.min(1, ((x - a[0]) * dx + (z - a[1]) * dz) / length)) : 0
    best = Math.min(best, Math.hypot(x - a[0] - dx * t, z - a[1] - dz * t))
  }
  return best
}

const round = (v: number, places = 3) => Math.round(v * 10 ** places) / 10 ** places

/** What `source` shows now, as a shot's JSON says it. */
export function describe(source: ShotSource, now = new Date()) {
  const { camera } = source
  camera.updateMatrices()
  const at: Vec3 = [camera.position.x, camera.position.y, camera.position.z]
  const d = camera.getWorldDirection({ x: 0, y: 0, z: 0 })
  const dir: Vec3 = [d.x, d.y, d.z]
  const hit = source.castRay(at, dir, LOOK_RANGE)
  const point: Vec3 | null = hit ? [at[0] + dir[0] * hit.t, at[1] + dir[1] * hit.t, at[2] + dir[2] * hit.t] : null
  const found = point ? buildingAt([point[0], point[2]], source.buildings()) : null
  const world = source.world()
  return {
    taken: now.toISOString(),
    world: world.base,
    worldHash: world.hash,
    phase: round(source.phase(), 4),
    camera: {
      position: at.map((v) => round(v)),
      /** The unit vector the camera looks along; x east, y up, z south. */
      direction: dir.map((v) => round(v, 5)),
      /** Compass heading (0 north, 90 east) and pitch (up positive), degrees. */
      heading: round(((Math.atan2(dir[0], -dir[2]) * 180) / Math.PI + 360) % 360, 2),
      pitch: round((Math.asin(Math.max(-1, Math.min(1, dir[1]))) * 180) / Math.PI, 2),
      fov: camera.fov,
      aspect: round(camera.aspect, 5),
      /** Column-major 4×4: world to camera, and camera to clip space (reversed depth). */
      view: Array.from(camera.view, (v) => round(v, 6)),
      projection: Array.from(camera.projection, (v) => round(v, 6)),
    },
    player: source.player()?.map((v) => round(v)) ?? null,
    looking: point
      ? {
          point: point.map((v) => round(v)),
          distance: round(hit!.t),
          building: found
            ? {
                tile: [found.ref.col, found.ref.row],
                index: found.index,
                centre: centreOf(found.building.footprint).map((v) => round(v)),
                footprint: found.building.footprint.map(([x, z]) => [round(x, 2), round(z, 2)]),
                holes: found.building.holes.map((hole) => hole.map(([x, z]) => [round(x, 2), round(z, 2)])),
                base: round(found.building.base, 2),
                height: round(found.building.height, 2),
                roofKind: found.building.roofKind,
                kind: found.building.kind,
              }
            : null,
        }
      : null,
  }
}

function centreOf(ring: readonly Point2[]): Point2 {
  let [x, z] = [0, 0]
  for (const p of ring) {
    x += p[0] / ring.length
    z += p[1] / ring.length
  }
  return [x, z]
}

/** The F2 key: takes a shot of the next frame drawn. */
export class Screenshots {
  private wanted = false
  private readonly onKey = (event: KeyboardEvent) => {
    if (event.code !== 'F2' || event.repeat) return
    event.preventDefault()
    this.wanted = true
  }

  constructor(private readonly source: ShotSource) {
    window.addEventListener('keydown', this.onKey)
  }

  /** Call right after the frame is drawn: reads it back while it is still there. */
  afterRender() {
    if (!this.wanted) return
    this.wanted = false
    const info = describe(this.source)
    // `toBlob` takes its copy now, in this task, before the frame is handed on and cleared.
    this.source.canvas.toBlob((blob) => {
      if (blob) void this.save(blob, info)
    }, 'image/png')
  }

  private async save(blob: Blob, info: ReturnType<typeof describe>) {
    const png = withText(new Uint8Array(await blob.arrayBuffer()), SHOT_KEYWORD, JSON.stringify(info))
    const world = info.world.split('/').filter(Boolean).pop() ?? 'world'
    const [x, , z] = info.camera.position
    const stamp = info.taken.replace(/[-:]/g, '').replace('T', '-').slice(0, 15)
    const name = `${world}-${Math.round(x!)}_${Math.round(z!)}-${stamp}.png`
    if (import.meta.env.DEV) {
      try {
        const response = await fetch(`/__shots?name=${encodeURIComponent(name)}`, { method: 'POST', body: png })
        if (response.ok) {
          this.source.say(`Screenshot saved: shots/${name}`)
          return
        }
      } catch {
        // No dev server to take it: downloaded below.
      }
    }
    const link = document.createElement('a')
    link.href = URL.createObjectURL(new Blob([png], { type: 'image/png' }))
    link.download = name
    link.click()
    setTimeout(() => URL.revokeObjectURL(link.href), 10_000)
    this.source.say(`Screenshot downloaded: ${name}`)
  }

  dispose() {
    window.removeEventListener('keydown', this.onKey)
  }
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    table[n] = c >>> 0
  }
  return table
})()

/** PNG's CRC-32 of `bytes`. */
export function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff]! ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

/** The length of the PNG signature, and of the IHDR chunk that must come first. */
const SIGNATURE = 8
const IHDR = 25

/** `png` with an uncompressed international text chunk (`iTXt`) holding `text` under `keyword`, just after its header. */
export function withText(png: Uint8Array, keyword: string, text: string): Uint8Array<ArrayBuffer> {
  const encoder = new TextEncoder()
  // keyword, 0, compression flag 0, method 0, empty language tag, 0, empty translated keyword, 0, text.
  const data = new Uint8Array([...encoder.encode(keyword), 0, 0, 0, 0, 0, ...encoder.encode(text)])
  const chunk = new Uint8Array(12 + data.length)
  const view = new DataView(chunk.buffer)
  view.setUint32(0, data.length)
  chunk.set(encoder.encode('iTXt'), 4)
  chunk.set(data, 8)
  view.setUint32(8 + data.length, crc32(chunk.subarray(4, 8 + data.length)))
  const at = SIGNATURE + IHDR
  const out = new Uint8Array(png.length + chunk.length)
  out.set(png.subarray(0, at))
  out.set(chunk, at)
  out.set(png.subarray(at), at + chunk.length)
  return out
}

/** The text of `png`'s uncompressed `iTXt` chunk under `keyword`, or null. */
export function textOf(png: Uint8Array, keyword: string): string | null {
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength)
  const decoder = new TextDecoder()
  for (let at = SIGNATURE; at + 12 <= png.length; ) {
    const length = view.getUint32(at)
    const type = decoder.decode(png.subarray(at + 4, at + 8))
    if (type === 'iTXt') {
      const data = png.subarray(at + 8, at + 8 + length)
      const end = data.indexOf(0)
      if (decoder.decode(data.subarray(0, end)) === keyword && data[end + 1] === 0) {
        // Past the flag and method, then the language tag and translated keyword, each ended by a 0.
        let i = end + 3
        i = data.indexOf(0, i) + 1
        i = data.indexOf(0, i) + 1
        return decoder.decode(data.subarray(i))
      }
    }
    if (type === 'IEND') break
    at += 12 + length
  }
  return null
}
