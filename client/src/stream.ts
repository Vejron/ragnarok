import { fetchTile, type TileData, type TileRef, type WorldData } from '@/world'

/** Tiles whose nearest edge is within this many metres of the player are loaded... */
export const LOAD_RADIUS = 450
/** ...and unloaded again beyond this, so walking along a border does not thrash. */
export const UNLOAD_RADIUS = 550
/** Fetches in flight at once. */
const MAX_FETCHES = 4
/** A failed fetch is tried again after this long. */
const RETRY_MS = 5000
/** The game starts without the tiles around it if they take longer than this. */
const PRIME_TIMEOUT_MS = 20000
/** Wanted tiles are recomputed once the player has moved this far, metres. */
const RECHECK_DISTANCE = 12

/** What a loaded tile is attached to. `attach` returns the function that detaches it again. */
export interface TileSink {
  attach(ref: TileRef, tile: TileData): () => void
}

interface Entry {
  ref: TileRef
  state: 'idle' | 'loading' | 'loaded'
  detach: (() => void) | null
  retryAt: number
}

/**
 * Keeps the tiles around the player loaded and the rest out of memory. The world is a grid of
 * tile files (`osm-import`); this fetches the ones within `LOAD_RADIUS`, hands them to the sink
 * (which draws them and gives them collision), and detaches those beyond `UNLOAD_RADIUS`.
 */
export class TileStreamer {
  private readonly entries = new Map<number, Entry>()
  private readonly waiting: Array<() => void> = []
  private x = Infinity
  private z = Infinity
  private checkedAt: [number, number] | null = null
  private inFlight = 0
  private disposed = false

  constructor(
    private readonly world: WorldData,
    private readonly sink: TileSink,
    private readonly fetcher: (ref: TileRef) => Promise<TileData> = fetchTile,
    private readonly now: () => number = () => performance.now(),
  ) {
    for (const ref of world.tiles) this.entries.set(key(ref), { ref, state: 'idle', detach: null, retryAt: 0 })
  }

  /** Tiles attached right now. */
  get loaded() {
    let count = 0
    for (const entry of this.entries.values()) if (entry.state === 'loaded') count++
    return count
  }

  /** Tells the streamer where the player is. Cheap to call every frame. */
  update(x: number, z: number) {
    this.x = x
    this.z = z
    const last = this.checkedAt
    if (last && Math.hypot(x - last[0], z - last[1]) < RECHECK_DISTANCE && this.inFlight === 0 && !this.hasRetries()) return
    this.checkedAt = [x, z]
    this.schedule()
  }

  /** Loads everything in range of `(x, z)` and resolves once it is attached: the first frame needs it. */
  async prime(x: number, z: number, timeoutMs = PRIME_TIMEOUT_MS) {
    this.update(x, z)
    const deadline = this.now() + timeoutMs
    while (!this.disposed && this.pending() > 0 && this.now() < deadline) {
      // Woken when a fetch ends; the timer is for failed tiles, which wait out their retry delay.
      await new Promise<void>((resolve) => {
        this.waiting.push(resolve)
        setTimeout(resolve, RETRY_MS / 4)
      })
      this.schedule()
    }
  }

  dispose() {
    this.disposed = true
    for (const entry of this.entries.values()) {
      entry.detach?.()
      entry.detach = null
    }
    this.waiting.splice(0).forEach((resolve) => resolve())
  }

  /** Loaded tiles are not counted as pending: only wanted ones that are not there yet. */
  private pending() {
    let count = 0
    for (const entry of this.entries.values()) {
      if (entry.state !== 'loaded' && this.distance(entry.ref) <= LOAD_RADIUS) count++
    }
    return count
  }

  private hasRetries() {
    const now = this.now()
    for (const entry of this.entries.values()) {
      if (entry.state === 'idle' && entry.retryAt > 0 && entry.retryAt <= now && this.distance(entry.ref) <= LOAD_RADIUS) return true
    }
    return false
  }

  /** Distance from the player to the nearest edge of a tile, metres. */
  private distance({ col, row }: TileRef) {
    const size = this.world.tileSize
    const x0 = -this.world.halfExtent + col * size
    const z0 = -this.world.halfExtent + row * size
    const dx = Math.max(x0 - this.x, 0, this.x - (x0 + size))
    const dz = Math.max(z0 - this.z, 0, this.z - (z0 + size))
    return Math.hypot(dx, dz)
  }

  private schedule() {
    const now = this.now()
    const wanted: Entry[] = []
    for (const entry of this.entries.values()) {
      const distance = this.distance(entry.ref)
      if (entry.state === 'loaded' && distance > UNLOAD_RADIUS) {
        entry.detach?.()
        entry.detach = null
        entry.state = 'idle'
      } else if (entry.state === 'idle' && distance <= LOAD_RADIUS && entry.retryAt <= now) {
        wanted.push(entry)
      }
    }
    // Nearest first, so the ground under the player arrives before the horizon.
    wanted.sort((a, b) => this.distance(a.ref) - this.distance(b.ref))
    for (const entry of wanted) {
      if (this.inFlight >= MAX_FETCHES) break
      void this.load(entry)
    }
  }

  private async load(entry: Entry) {
    entry.state = 'loading'
    this.inFlight++
    try {
      const tile = await this.fetcher(entry.ref)
      // The player may have walked away while it travelled.
      if (!this.disposed && this.distance(entry.ref) <= UNLOAD_RADIUS) {
        entry.detach = this.sink.attach(entry.ref, tile)
        entry.state = 'loaded'
        entry.retryAt = 0
      } else {
        entry.state = 'idle'
      }
    } catch (error) {
      console.warn('tile failed', entry.ref, error)
      entry.state = 'idle'
      entry.retryAt = this.now() + RETRY_MS
    } finally {
      this.inFlight--
      if (!this.disposed) this.schedule()
      this.waiting.splice(0).forEach((resolve) => resolve())
    }
  }
}

function key({ col, row }: TileRef) {
  return col * 65536 + row
}
