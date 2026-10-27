/** Craters kept at once: a ring, the oldest overwritten. */
export const CRATER_CAPACITY = 4096
/** Floats per crater: `x, z, radius, depth` (metres), then birth time (seconds, the renderer's clock) and three unused. */
export const CRATER_FLOATS = 8
/** Side of a cell of the grid over the ground, metres. */
export const CRATER_CELL = 16
/** Buckets the cells hash into (a power of two). */
export const CRATER_BUCKETS = 4096
/** `u32`s a bucket: its count, then that many crater indices. */
export const CRATER_SLOTS = 16
/** How far a crater's marks reach from its middle, in radii (the shader's `CRATER_REACH`). */
export const CRATER_REACH = 1.6
/** Seconds a fresh crater's embers glow, from its birth. */
export const CRATER_EMBER_LIFE = 20
/** Seconds a crater's marks last, from its birth, fading out over the last 30% (the server's `CRATER_FADE_SECONDS`). */
export const CRATER_FADE = 30 * 60

/** The bucket of grid cell `(cx, cz)`, as the shader hashes it. */
export function craterBucket(cx: number, cz: number): number {
  return ((Math.imul(cx, 73856093) ^ Math.imul(cz, 19349663)) >>> 0) & (CRATER_BUCKETS - 1)
}

/**
 * The craters the shaders mark the ground with (SCORCH.md): a ring of `CRATER_CAPACITY` records and a
 * hash grid of the cells their marks reach. Kept by the scene, so it outlives a renderer; the renderer
 * uploads what changed (`takeDirty`), or everything when it has not seen this list before.
 */
export class Craters {
  readonly data = new Float32Array(CRATER_CAPACITY * CRATER_FLOATS)
  readonly grid = new Uint32Array(CRATER_BUCKETS * CRATER_SLOTS)
  /** Bumped by every change; a renderer that has not seen this one uploads it all. */
  version = 0
  private shown = true
  /** The order craters were added in, per slot: lower is older. */
  private readonly order = new Float64Array(CRATER_CAPACITY)
  private added = 0
  /** The order number of the oldest crater not yet expired. */
  private first = 0
  private readonly dirtySlots = new Set<number>()
  private readonly dirtyBuckets = new Set<number>()
  private cleared = false

  /** Whether the ground is marked. Hidden, the craters are still kept (and added), only not drawn. */
  get visible() {
    return this.shown
  }

  set visible(on: boolean) {
    if (on === this.shown) return
    this.shown = on
    this.cleared = true
    this.version++
  }

  /** How many craters are kept. */
  get count() {
    return Math.min(this.added, CRATER_CAPACITY)
  }

  /** Adds a crater at `(x, z)` with `radius` and `depth` (metres), born at `birth` (seconds, `performance.now() / 1000`). */
  add(x: number, z: number, radius: number, depth: number, birth: number) {
    const slot = this.added % CRATER_CAPACITY
    if (this.added >= CRATER_CAPACITY) this.unlist(slot)
    const o = slot * CRATER_FLOATS
    this.data.set([x, z, radius, depth, birth, 0, 0, 0], o)
    this.order[slot] = this.added++
    this.eachBucket(slot, (bucket) => this.list(bucket, slot))
    this.dirtySlots.add(slot)
    this.version++
  }

  /** Forgets every crater. */
  clear() {
    if (this.added === 0) return
    this.data.fill(0)
    this.grid.fill(0)
    this.added = 0
    this.first = 0
    this.dirtySlots.clear()
    this.dirtyBuckets.clear()
    this.cleared = true
    this.version++
  }

  /**
   * Takes the craters born before `before` (seconds, as `add`'s `birth`) out of the grid, oldest first,
   * stopping at the first younger one: craters come in about the order they were born.
   */
  expire(before: number) {
    this.first = Math.max(this.first, this.added - CRATER_CAPACITY)
    for (; this.first < this.added; this.first++) {
      const slot = this.first % CRATER_CAPACITY
      if (this.data[slot * CRATER_FLOATS + 4]! >= before) break
      this.unlist(slot)
      this.version++
    }
  }

  /** The crater slots and buckets changed since the last call (`all` after a `clear`), and forgets them. */
  takeDirty(): { all: boolean; slots: number[]; buckets: number[] } {
    const dirty = { all: this.cleared, slots: [...this.dirtySlots], buckets: [...this.dirtyBuckets] }
    this.dirtySlots.clear()
    this.dirtyBuckets.clear()
    this.cleared = false
    return dirty
  }

  /** The crater indices bucket `bucket` lists. */
  listed(bucket: number): number[] {
    const b = bucket * CRATER_SLOTS
    return Array.from(this.grid.subarray(b + 1, b + 1 + this.grid[b]!))
  }

  private eachBucket(slot: number, visit: (bucket: number) => void) {
    const o = slot * CRATER_FLOATS
    const x = this.data[o]!
    const z = this.data[o + 1]!
    const reach = this.data[o + 2]! * CRATER_REACH
    const seen = new Set<number>()
    for (let cz = Math.floor((z - reach) / CRATER_CELL); cz <= Math.floor((z + reach) / CRATER_CELL); cz++) {
      for (let cx = Math.floor((x - reach) / CRATER_CELL); cx <= Math.floor((x + reach) / CRATER_CELL); cx++) {
        const bucket = craterBucket(cx, cz)
        // Two cells can hash to one bucket: list the crater there once.
        if (!seen.has(bucket)) visit(bucket)
        seen.add(bucket)
      }
    }
  }

  /**
   * Lists `slot` in `bucket`. If it is full, the oldest crater listed there is taken out of every bucket
   * it is in, not only this one, so it goes whole rather than cut off square at this cell's edge.
   */
  private list(bucket: number, slot: number) {
    const b = bucket * CRATER_SLOTS
    if (this.grid[b]! >= CRATER_SLOTS - 1) {
      let oldest = b + 1
      for (let k = b + 2; k <= b + this.grid[b]!; k++) if (this.order[this.grid[k]!]! < this.order[this.grid[oldest]!]!) oldest = k
      this.unlist(this.grid[oldest]!)
    }
    const count = this.grid[b]!
    this.grid[b + 1 + count] = slot
    this.grid[b] = count + 1
    this.dirtyBuckets.add(bucket)
  }

  /** Takes `slot` out of every bucket it is listed in. */
  private unlist(slot: number) {
    this.eachBucket(slot, (bucket) => {
      const b = bucket * CRATER_SLOTS
      const count = this.grid[b]!
      for (let k = 1; k <= count; k++) {
        if (this.grid[b + k] !== slot) continue
        this.grid[b + k] = this.grid[b + count]!
        this.grid[b + count] = 0
        this.grid[b] = count - 1
        this.dirtyBuckets.add(bucket)
        break
      }
    })
  }
}
