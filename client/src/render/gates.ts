import { Geometry, Mesh, type Group } from '@ragnarok/engine'
import { BUILDING_BARN, type TileData } from '@/world'
import { shellOf } from '@/sim/carve'
import { insideFor } from '@/sim/interior'
import { Surface } from './buildingmesh'
import { openingStands } from './lookfracture'
import { isGate, openingsFor, type Opening } from './windows'
import { buildingMaterial } from './world'

/*
 * The gates of barns, halls and garages (`isGate`), which a soldier opens and shuts with E: the
 * server says which stand open (`Gates`), and here each gate's leaf is a mesh of its own that moves.
 * A hall's and a garage's is an overhead door, of ribbed sections that roll up under the lintel; a
 * barn's is a plank door hung on a rail, which slides aside along the wall. A gate nobody has
 * touched is shut. Which gates are open is remembered while a tile is away, like a building's holes.
 */

/** How long a gate takes to open or shut, seconds. */
const TRAVEL = 1.2
/** How much of an overhead door is left showing under the lintel when it is up, as a share of its height. */
const ROLLED = 0.06
/** How far a barn's door stands out from the wall, and an overhead door in from it, metres. */
const SLIDE_OUT = 0.07
const ROLL_IN = 0.06
/** A leaf reaches this far over its gateway's top, metres: the hole cut for it does (`DOOR_RISE` in `facade.rs`). */
const OVER = 0.3
/** How far a sliding door laps over the wall at each side of its gateway, metres. */
const LAP = 0.12

/** Linear colours: an overhead door's sections, and a barn door's tarred planks. */
const ROLL_RGB = [0.72, 0.74, 0.75] as const
const SLIDE_RGB = [0.2, 0.065, 0.04] as const

interface Leaf {
  mesh: Mesh
  index: number
  opening: number
  o: Opening
  /** Slides aside (a barn's), or rolls up. */
  slides: boolean
  /** 0 shut, 1 open: where it is, and where it is going. */
  at: number
  to: number
}

interface LoadedTile {
  tile: TileData
  leaves: Leaf[]
}

/** The width and height of opening `o`, metres, and the unit along its wall from its first corner. */
function frame(o: Opening): { width: number; height: number; ux: number; uz: number } {
  const [a, b, , d] = o.corners
  const width = Math.hypot(b[0] - a[0], b[2] - a[2]) || 1
  return { width, height: d[1] - a[1], ux: (b[0] - a[0]) / width, uz: (b[2] - a[2]) / width }
}

/**
 * A leaf's geometry about its own origin, the top of its gateway's first corner: back along x from
 * `lap` to `-(width + lap)` (the wall runs along -x when +z is out from it), from `-height` up to 0
 * along y (so it scales towards its top), facing +z and -z, with the pattern `surface`.
 */
function leafGeometry(width: number, height: number, lap: number, rgb: readonly [number, number, number], surface: Surface): Geometry {
  const [x0, x1, y0, y1] = [-(width + lap), lap, -height, 0]
  const corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y0], [x1, y1], [x0, y1]] as const
  const positions = new Float32Array(36)
  const normals = new Float32Array(36)
  const colors = new Float32Array(36)
  const custom = new Float32Array(48)
  for (let side = 0; side < 2; side++) {
    for (let k = 0; k < 6; k++) {
      // The back face is the front one wound the other way.
      const [x, y] = corners[side === 0 ? k : 5 - k]!
      const v = side * 6 + k
      positions.set([x, y, 0], v * 3)
      normals.set([0, 0, side === 0 ? 1 : -1], v * 3)
      colors.set(rgb, v * 3)
      custom[v * 4] = surface
    }
  }
  return new Geometry({ positions, normals, colors, custom })
}

export class GateView {
  /** Open gates by tile key, building index and opening index, kept across the tile streaming out and in. */
  private readonly open = new Map<number, Map<number, Set<number>>>()
  private readonly loaded = new Map<number, LoadedTile>()

  /**
   * Tile `key` is drawn in `group` now: its gates are hung, open or shut as the server last said, and
   * not at all where the wall round one is gone (`carved`).
   */
  attach(key: number, tile: TileData, group: Group, carved: ReadonlyMap<number, ReadonlySet<number>>) {
    const leaves: Leaf[] = []
    tile.buildings.forEach((building, index) => {
      if (!building.look || !tile.look) return
      for (const opening of this.gatesOf(tile, index)) {
        const o = openingsFor(tile.look, building.look)[opening]!
        const slides = building.kind === BUILDING_BARN
        const { width, height, ux, uz } = frame(o)
        const mesh = new Mesh(leafGeometry(width, height + OVER + 0.1, slides ? LAP : 0, slides ? SLIDE_RGB : ROLL_RGB, slides ? Surface.Upright : Surface.Lap), buildingMaterial)
        mesh.castShadow = true
        mesh.receiveShadow = true
        mesh.userData.gate = true
        // Its own z out from the wall, and its x back along it.
        mesh.rotation.set(0, Math.atan2(uz, -ux), 0)
        const at = this.isOpen(key, index, opening) ? 1 : 0
        const leaf: Leaf = { mesh, index, opening, o, slides, at, to: at }
        this.place(leaf)
        group.add(mesh)
        leaves.push(leaf)
      }
    })
    this.loaded.set(key, { tile, leaves })
    for (const [index, cells] of carved) this.holed(key, index, cells)
  }

  detach(key: number) {
    this.loaded.delete(key)
  }

  /**
   * The server says gate `opening` of building `index` of tile `key` is open or shut. With `live` it
   * moves there; without (the catch-up on joining) it is there.
   */
  set(key: number, index: number, opening: number, open: boolean, live: boolean) {
    let byBuilding = this.open.get(key)
    if (!byBuilding) this.open.set(key, (byBuilding = new Map()))
    let gates = byBuilding.get(index)
    if (!gates) byBuilding.set(index, (gates = new Set()))
    if (open) gates.add(opening)
    else gates.delete(opening)
    const leaf = this.loaded.get(key)?.leaves.find((leaf) => leaf.index === index && leaf.opening === opening)
    if (!leaf) return
    leaf.to = open ? 1 : 0
    if (!live) {
      leaf.at = leaf.to
      this.place(leaf)
    }
  }

  /** Whether gate `opening` of building `index` of tile `key` stands open. */
  isOpen(key: number, index: number, opening: number): boolean {
    return this.open.get(key)?.get(index)?.has(opening) ?? false
  }

  /** The buildings of tile `key` with a gate open. */
  opened(key: number): number[] {
    return [...(this.open.get(key) ?? [])].filter(([, gates]) => gates.size > 0).map(([index]) => index)
  }

  /**
   * The gates of building `index` of loaded tile `key` that stand shut, by their number among its
   * openings: what the physics is to take as wall (`SimMotor.setGates`).
   */
  shut(key: number, index: number): number[] {
    const loaded = this.loaded.get(key)
    if (!loaded) return []
    return this.gatesOf(loaded.tile, index).filter((opening) => !this.isOpen(key, index, opening))
  }

  /** The cells `removed` are gone from building `index` of tile `key`: a gate whose wall no longer stands round it goes with it. */
  holed(key: number, index: number, removed: ReadonlySet<number>) {
    const loaded = this.loaded.get(key)
    if (!loaded || removed.size === 0) return
    const shell = shellOf(loaded.tile, index)
    if (!shell) return
    for (const leaf of loaded.leaves) if (leaf.index === index && leaf.mesh.visible && !openingStands(shell, removed, leaf.o)) leaf.mesh.visible = false
  }

  /** Moves the gates that are on their way. */
  update(dt: number) {
    for (const { leaves } of this.loaded.values()) {
      for (const leaf of leaves) {
        if (leaf.at === leaf.to) continue
        const step = dt / TRAVEL
        leaf.at = leaf.to > leaf.at ? Math.min(leaf.to, leaf.at + step) : Math.max(leaf.to, leaf.at - step)
        this.place(leaf)
      }
    }
  }

  /** Whether no gate is open. */
  get untouched() {
    for (const byBuilding of this.open.values()) for (const gates of byBuilding.values()) if (gates.size > 0) return false
    return true
  }

  /** Forgets every open gate: all are shut again. Every tile must be detached first. */
  reset() {
    this.open.clear()
  }

  dispose() {
    this.loaded.clear()
    this.open.clear()
  }

  /** The gates of building `index` of `tile`, by their number among its openings: its wide doors, where it has a floor to drive in on. */
  private gatesOf(tile: TileData, index: number): number[] {
    const building = tile.buildings[index]
    if (!building?.look || !tile.look) return []
    const openings = openingsFor(tile.look, building.look)
    if (!openings.some(isGate) || insideFor(tile.look, building.look).pieces.length === 0) return []
    return openings.flatMap((o, opening) => (isGate(o) ? [opening] : []))
  }

  /** Puts `leaf` where it is when it is `leaf.at` of the way open, eased at both ends. */
  private place(leaf: Leaf) {
    const { o, mesh, slides } = leaf
    const [a] = o.corners
    const { width, height, ux, uz } = frame(o)
    const t = leaf.at * leaf.at * (3 - 2 * leaf.at)
    const out = slides ? SLIDE_OUT : -ROLL_IN
    // A barn's door slides along the wall, its own width and a little more; an overhead door rolls up into its top.
    const along = slides ? t * (width + 2 * LAP) : 0
    mesh.position.set(a[0] + ux * along + o.normal[0] * out, a[1] + height + OVER, a[2] + uz * along + o.normal[2] * out)
    mesh.scale.set(1, slides ? 1 : 1 - (1 - ROLLED) * t, 1)
  }
}
