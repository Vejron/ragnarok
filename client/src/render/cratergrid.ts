/**
 * The craters a layer of the ground's cover keeps bare, found by where they are. A long fight digs
 * thousands, and every blade, stone and plant laid asks whether it stands in one: each crater is
 * listed under the cells its bowl reaches, so the asking looks at the few craters about the spot and
 * costs the same however many there are in all.
 */

/** A crater's bowl on the ground plane: its centre and radius, metres. */
export interface Bowl {
  x: number
  z: number
  radius: number
}

/** Side of a cell, metres: a few bowls wide, so a bowl is listed under a cell or two. */
const CELL = 16

const cellOf = (v: number) => Math.floor(v / CELL)
const cellKey = (cx: number, cz: number) => (cz + 32768) * 65536 + (cx + 32768)

export class CraterGrid {
  /** The bowls reaching each cell, by `cellKey`. */
  private readonly cells = new Map<number, Bowl[]>()
  /** Every bowl held, by `x,z,radius`, so telling of one twice adds nothing. */
  private readonly held = new Map<string, Bowl>()

  /** How many bowls are held. */
  get size() {
    return this.held.size
  }

  /** Takes the bowl of `radius` metres at `(x, z)`. False when it was held already. */
  add(x: number, z: number, radius: number): boolean {
    const key = `${x},${z},${radius}`
    if (this.held.has(key)) return false
    const bowl = { x, z, radius }
    this.held.set(key, bowl)
    for (let cz = cellOf(z - radius); cz <= cellOf(z + radius); cz++) {
      for (let cx = cellOf(x - radius); cx <= cellOf(x + radius); cx++) {
        const at = cellKey(cx, cz)
        const list = this.cells.get(at)
        if (list) list.push(bowl)
        else this.cells.set(at, [bowl])
      }
    }
    return true
  }

  /** Lets go of the bowl `add` was given. False when it was not held. */
  remove(x: number, z: number, radius: number): boolean {
    const key = `${x},${z},${radius}`
    const bowl = this.held.get(key)
    if (!bowl) return false
    this.held.delete(key)
    for (let cz = cellOf(z - radius); cz <= cellOf(z + radius); cz++) {
      for (let cx = cellOf(x - radius); cx <= cellOf(x + radius); cx++) {
        const at = cellKey(cx, cz)
        const list = this.cells.get(at)!
        list.splice(list.indexOf(bowl), 1)
        if (list.length === 0) this.cells.delete(at)
      }
    }
    return true
  }

  /** Whether `(x, z)` is less than `margin` metres from a bowl, or inside one. */
  covers(x: number, z: number, margin = 0): boolean {
    if (this.held.size === 0) return false
    // A bowl that near has a point within the margin, and is listed under that point's cell.
    const [cx0, cx1, cz0, cz1] = [cellOf(x - margin), cellOf(x + margin), cellOf(z - margin), cellOf(z + margin)]
    for (let cz = cz0; cz <= cz1; cz++) {
      for (let cx = cx0; cx <= cx1; cx++) {
        const list = this.cells.get(cellKey(cx, cz))
        if (!list) continue
        for (const c of list) if (Math.hypot(x - c.x, z - c.z) < c.radius + margin) return true
      }
    }
    return false
  }

  /** The bowls reaching the rectangle from `(x0, z0)` to `(x1, z1)`, each once. */
  within(x0: number, z0: number, x1: number, z1: number): Bowl[] {
    const found = new Set<Bowl>()
    for (let cz = cellOf(z0); cz <= cellOf(z1); cz++) {
      for (let cx = cellOf(x0); cx <= cellOf(x1); cx++) {
        for (const c of this.cells.get(cellKey(cx, cz)) ?? []) {
          if (c.x + c.radius >= x0 && c.x - c.radius <= x1 && c.z + c.radius >= z0 && c.z - c.radius <= z1) found.add(c)
        }
      }
    }
    return [...found]
  }

  clear() {
    this.cells.clear()
    this.held.clear()
  }
}
