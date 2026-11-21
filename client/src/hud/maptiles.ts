/**
 * The world's map as tiles (the importer's `maptiles/`, PHONE.md), the way a web map serves it: level
 * 0 is one square over the whole world, and each level below splits every square in four, down to one
 * per world tile at 2 pixels a metre. A map asks for the few squares it shows, at the level its zoom
 * wants; a square not yet loaded is drawn from the nearest coarser one that is, stretched.
 */

/** How the levels lie (`maptiles/index.json`). */
export interface MapIndex {
  levels: number
  /** Side of a tile, pixels. */
  tile_px: number
  /** Side of a level-0 tile, metres; each level below halves it. */
  top_metres: number
  /** The world point at every level's north-west corner, metres `[x, z]`. */
  origin: [number, number]
  /** The render's hash, which the tiles are asked for by. */
  hash: number
}

/** A loaded tile: anything `drawImage` takes that knows its size. */
export type TileImage = CanvasImageSource & { width: number; height: number; close?: () => void }

/** Fetches one tile's picture, or null where there is none (a square with no world under it). */
export type TileLoader = (url: string) => Promise<TileImage | null>

/** Fetches with `fetch` and decodes off the main thread with `createImageBitmap`. */
const fetchBitmap: TileLoader = async (url) => {
  const response = await fetch(url)
  if (!response.ok) return null
  return createImageBitmap(await response.blob())
}

/** The tiles kept decoded at once, the least recently drawn let go first. */
export const MAX_TILES = 192
/** Requests in flight at once; the rest wait to be asked for again by a later draw. */
const MAX_LOADING = 8

export class MapTiles {
  private index: MapIndex | null = null
  /** By key `z/x/y`: a picture, null for a square that has none, in the order last drawn. */
  private readonly cache = new Map<string, TileImage | null>()
  private readonly loading = new Set<string>()
  /** Called when a tile arrives, so what shows the map draws it again. */
  onTile: () => void = () => {}

  /** Where the world's files are served from (`WORLD_BASE`), once known. */
  private base = '/'

  constructor(private readonly load: TileLoader = fetchBitmap) {}

  /**
   * Fetches the index of the world served from `base`: only known once the world is (in dev, the one
   * the editor activated). Until it comes, and in a world without tiles, nothing is drawn.
   */
  async init(base: string, fetchIndex: () => Promise<MapIndex | null> = () => fetch(`${base}maptiles/index.json`).then((r) => (r.ok ? r.json() as Promise<MapIndex> : null)).catch(() => null)) {
    this.clear()
    this.base = base
    this.index = await fetchIndex()
    if (!this.index) console.warn(`no map tiles at ${base}maptiles/: draw them with osm-import --maptiles`)
    this.onTile()
  }

  get ready(): boolean {
    return this.index !== null
  }

  /** Lets every tile go, as for another world. */
  clear() {
    for (const image of this.cache.values()) image?.close?.()
    this.cache.clear()
    this.loading.clear()
    this.index = null
  }

  /** Side of a tile of level `z`, metres. */
  tileMetres(z: number): number {
    return this.index ? this.index.top_metres / 2 ** z : 0
  }

  /** The coarsest level whose pixels are no bigger than `metresPerPixel`, so the map is sharp, or the finest there is. */
  levelFor(metresPerPixel: number): number {
    const index = this.index
    if (!index) return 0
    const z = Math.ceil(Math.log2(index.top_metres / (index.tile_px * metresPerPixel)) - 1e-6)
    return Math.max(0, Math.min(index.levels - 1, z))
  }

  /**
   * Draws the map at `perMetre` pixels a metre, centred on world (`cx`, `cz`), into a `width` × `height`
   * px rectangle of `g` (in its current transform, whose scale says how many device pixels that is),
   * asking for what is missing.
   */
  draw(g: CanvasRenderingContext2D, cx: number, cz: number, perMetre: number, width: number, height: number) {
    const index = this.index
    if (!index) return
    const device = Math.abs(g.getTransform?.().a ?? 1) || 1
    const z = this.levelFor(1 / (perMetre * device))
    const tileMetres = this.tileMetres(z)
    const [left, top] = [cx - width / 2 / perMetre, cz - height / 2 / perMetre]
    const count = 2 ** z
    const first = (v: number) => Math.max(0, Math.floor(v / tileMetres))
    const last = (v: number) => Math.min(count - 1, Math.floor(v / tileMetres))
    const [x0, x1] = [first(left - index.origin[0]), last(left + width / perMetre - index.origin[0])]
    const [y0, y1] = [first(top - index.origin[1]), last(top + height / perMetre - index.origin[1])]
    g.imageSmoothingEnabled = true
    g.imageSmoothingQuality = 'high'
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        // Whole pixels, a hair over, so no seam shows between neighbours.
        const dx = Math.floor((index.origin[0] + x * tileMetres - left) * perMetre)
        const dy = Math.floor((index.origin[1] + y * tileMetres - top) * perMetre)
        const side = Math.ceil(tileMetres * perMetre) + 1
        this.drawTile(g, z, x, y, dx, dy, side)
      }
    }
    this.trim()
  }

  /** Tile `(z, x, y)` at `(dx, dy)`, `side` px; while it loads, the part of the nearest coarser loaded tile over it. */
  private drawTile(g: CanvasRenderingContext2D, z: number, x: number, y: number, dx: number, dy: number, side: number) {
    const own = this.take(z, x, y)
    if (own) {
      g.drawImage(own, dx, dy, side, side)
      return
    }
    if (own === null) return
    for (let up = 1; up <= z; up++) {
      const parent = this.peek(z - up, x >> up, y >> up)
      if (!parent) continue
      const part = parent.width / 2 ** up
      g.drawImage(parent, (x - ((x >> up) << up)) * part, (y - ((y >> up) << up)) * part, part, part, dx, dy, side, side)
      return
    }
  }

  /** The tile if loaded (marked as just used), null if it has none, or undefined while it is asked for. */
  private take(z: number, x: number, y: number): TileImage | null | undefined {
    const key = `${z}/${x}/${y}`
    if (this.cache.has(key)) {
      const image = this.cache.get(key)!
      this.cache.delete(key)
      this.cache.set(key, image)
      return image
    }
    this.request(key, z, x, y)
    return undefined
  }

  private peek(z: number, x: number, y: number): TileImage | null | undefined {
    return this.cache.get(`${z}/${x}/${y}`)
  }

  private request(key: string, z: number, x: number, y: number) {
    const index = this.index
    if (!index || this.loading.has(key) || this.loading.size >= MAX_LOADING) return
    this.loading.add(key)
    this.load(`${this.base}maptiles/${z}/${x}_${y}.webp?h=${index.hash}`)
      .catch(() => null)
      .then((image) => {
        if (!this.loading.delete(key) || this.index !== index) {
          image?.close?.()
          return
        }
        this.cache.set(key, image)
        this.trim()
        this.onTile()
      })
  }

  /** Lets the least recently drawn tiles go beyond `MAX_TILES`. */
  private trim() {
    while (this.cache.size > MAX_TILES) {
      const [key, image] = this.cache.entries().next().value!
      image?.close?.()
      this.cache.delete(key)
    }
  }

  /** How many tiles are held, for tests and the memory bench. */
  get size(): number {
    return this.cache.size
  }
}
