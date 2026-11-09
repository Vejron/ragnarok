import { Geometry, Group, Mesh, Vector3 } from '@ragnarok/engine'
import type { TileData } from '@/world'
import { shellOf } from '@/sim/carve'
import type { Debris } from './debris'
import { GLASS_REACH, PANE_VERTICES, glassShards, paneBlasted, paneCentre, paneKey, paneStruck, writePane } from './glass'
import { hash, type P } from './geom'
import { random } from './shards'
import { openingsFor, type Opening } from './windows'
import { openingStands } from './lookfracture'
import { glassMaterial, glassOf } from './world'

/*
 * Which window panes are broken, and breaking them: a round that ends on a pane (`struck`), a blast
 * near enough (`blast`) or a hole blown in the wall beside it (`holed`) collapses that pane's sheet in
 * the tile's glass mesh, leaves jagged teeth in its frame and, when it happens in sight, throws a few
 * shards of glass. All of it follows from what the server sent, so every client breaks the same panes;
 * what is broken is remembered while a tile is away, like a building's holes.
 */

/** Shards thrown from one pane, and from one blast at most. */
const SHARDS_PER_PANE = 7
const SHARDS_PER_BLAST = 40
/** Panes a blast throws shards from (the nearest); the rest break quietly. */
const LOUD_PANES = 6
/** A shard lies a while and then sinks into the ground, from 6 s after it breaks: gone this long after, the last `SHARD_SINK` seconds sinking. */
const SHARD_LIFE = 10
const SHARD_SINK = 4
/** A shard lies this high on the ground, metres. */
const SHARD_REST = 0.02
/** Shard shapes kept for the session. */
const SHAPES = 4

interface LoadedTile {
  tile: TileData
  group: Group
  /** Pane number in the glass mesh by `paneKey`. */
  panes: Map<number, number>
}

export class Glazing {
  /** Broken panes by tile key, building index and opening index, kept across the tile streaming out and in. */
  private readonly broken = new Map<number, Map<number, Set<number>>>()
  /** Of those, the panes whose wall is gone round them: nothing of them is drawn, not a tooth. */
  private readonly gone = new Map<number, Map<number, Set<number>>>()
  private readonly loaded = new Map<number, LoadedTile>()
  private shapes: Geometry[] | null = null

  constructor(private readonly debris: Debris) {}

  /** Tile `key` is drawn in `group` now: its broken panes are broken, and those beside holes already blown (`carved`) with them. */
  attach(key: number, tile: TileData, group: Group, carved: ReadonlyMap<number, ReadonlySet<number>>) {
    const glass = glassOf(group)
    if (!glass) return
    const panes = new Map<number, number>()
    const ids = glass.userData.panes as Uint32Array
    for (let pane = 0; pane < ids.length / 2; pane++) panes.set(paneKey(ids[pane * 2]!, ids[pane * 2 + 1]!), pane)
    this.loaded.set(key, { tile, group, panes })
    for (const [index, cells] of carved) this.holed(key, index, cells, false)
    for (const [index, openings] of this.broken.get(key) ?? []) for (const opening of openings) this.write(key, index, opening)
  }

  detach(key: number) {
    this.loaded.delete(key)
  }

  /** Whether no pane is broken. */
  get untouched() {
    return this.broken.size === 0
  }

  /** Forgets every broken pane: the glass is whole again. Every tile must be detached first. */
  reset() {
    this.broken.clear()
    this.gone.clear()
  }

  dispose() {
    this.loaded.clear()
    this.broken.clear()
    this.gone.clear()
    for (const shape of this.shapes ?? []) shape.dispose()
    this.shapes = null
  }

  /** Broken panes of building `index` of tile `key`, by opening index, for tests. */
  brokenPanes(key: number, index: number): ReadonlySet<number> {
    return this.broken.get(key)?.get(index) ?? new Set()
  }

  /**
   * A round ended at `at`: if that is on a window, true, and if its pane was whole it breaks, its glass
   * flying on the way the round went (in through the wall from outside). A round into a window, whole
   * or broken, is not a round into the wall: nothing else is to be marked.
   */
  struck(at: P): boolean {
    const found = this.paneAt(at)
    if (!found) return false
    const { key, index, opening, o, broken } = found
    if (!broken) this.break(key, index, opening, o, true, at)
    return true
  }

  /** A blast at `at` of `radius` breaks the panes near it (`paneBlasted`), throwing glass from the nearest few. */
  blast(at: P, radius: number) {
    const reach = GLASS_REACH * radius
    const hit: { key: number; index: number; opening: number; o: Opening; d: number }[] = []
    for (const [key, { tile }] of this.loaded) {
      tile.buildings.forEach((building, index) => {
        if (!building.look || !tile.look) return
        if (!this.near(building, at, reach)) return
        openingsFor(tile.look, building.look).forEach((o, opening) => {
          if (o.door || this.isBroken(key, index, opening) || !paneBlasted(o, at, radius)) return
          const c = paneCentre(o)
          hit.push({ key, index, opening, o, d: Math.hypot(c[0] - at[0], c[1] - at[1], c[2] - at[2]) })
        })
      })
    }
    hit.sort((a, b) => a.d - b.d)
    let thrown = 0
    hit.forEach(({ key, index, opening, o }, rank) => {
      const loud = rank < LOUD_PANES && thrown < SHARDS_PER_BLAST
      if (loud) thrown += SHARDS_PER_PANE
      this.break(key, index, opening, o, loud, at)
    })
  }

  /**
   * The cells `removed` are gone from building `index` of tile `key`: the panes within a cell of any
   * of them lose their glass, quietly (the blast is enough), and those whose wall no longer stands
   * round them go altogether, teeth and all, as `lookFracture` leaves out their frames.
   */
  holed(key: number, index: number, removed: ReadonlySet<number>, live: boolean) {
    const loaded = this.loaded.get(key)
    if (!loaded || removed.size === 0) return
    const { tile } = loaded
    const building = tile.buildings[index]
    const shell = building?.look && tile.look ? shellOf(tile, index) : null
    if (!shell || !building?.look || !tile.look) return
    const coords = [...removed].map((id) => shell.coords(id))
    const origin = shell.origin.map((v) => v / 100)
    const cell = shell.cell / 100
    openingsFor(tile.look, building.look).forEach((o, opening) => {
      if (o.door || this.isGone(key, index, opening)) return
      if (!openingStands(shell, removed, o)) return this.remove(key, index, opening)
      if (this.isBroken(key, index, opening)) return
      const lo = [0, 1, 2].map((a) => Math.floor((Math.min(...o.corners.map((p) => p[a]!)) - origin[a]!) / cell) - 1)
      const hi = [0, 1, 2].map((a) => Math.floor((Math.max(...o.corners.map((p) => p[a]!)) - origin[a]!) / cell) + 1)
      const near = coords.some((c) => c.every((v, a) => v >= lo[a]! && v <= hi[a]!))
      if (near) this.break(key, index, opening, o, false, null, live)
    })
  }

  private isBroken(key: number, index: number, opening: number): boolean {
    return this.broken.get(key)?.get(index)?.has(opening) ?? false
  }

  private isGone(key: number, index: number, opening: number): boolean {
    return this.gone.get(key)?.get(index)?.has(opening) ?? false
  }

  /** Pane `opening` went with its wall: broken, if it was not, and drawn as nothing at all. */
  private remove(key: number, index: number, opening: number) {
    for (const map of [this.broken, this.gone]) {
      const buildings = map.get(key) ?? new Map<number, Set<number>>()
      map.set(key, buildings)
      const openings = buildings.get(index) ?? new Set<number>()
      buildings.set(index, openings)
      openings.add(opening)
    }
    this.write(key, index, opening)
  }

  /** Whether `at` is within `reach` of `building`'s footprint box. */
  private near(building: TileData['buildings'][number], at: P, reach: number): boolean {
    const xs = building.footprint.map((p) => p[0])
    const zs = building.footprint.map((p) => p[1])
    return at[0] >= Math.min(...xs) - reach && at[0] <= Math.max(...xs) + reach && at[2] >= Math.min(...zs) - reach && at[2] <= Math.max(...zs) + reach && at[1] >= building.base - reach && at[1] <= building.base + building.height + reach
  }

  /** The window a round ending at `at` went through, the nearest when more than one fits, and whether its pane was broken already. */
  private paneAt(at: P): { key: number; index: number; opening: number; o: Opening; broken: boolean } | null {
    let best: { key: number; index: number; opening: number; o: Opening; broken: boolean; d: number } | null = null
    for (const [key, { tile }] of this.loaded) {
      tile.buildings.forEach((building, index) => {
        if (!building.look || !tile.look || !this.near(building, at, 1.2)) return
        openingsFor(tile.look, building.look).forEach((o, opening) => {
          if (o.door || !paneStruck(o, at)) return
          const c = paneCentre(o)
          const d = Math.hypot(c[0] - at[0], c[1] - at[1], c[2] - at[2])
          if (!best || d < best.d) best = { key, index, opening, o, broken: this.isBroken(key, index, opening), d }
        })
      })
    }
    return best
  }

  /** Breaks pane `opening`: remembered, drawn broken if its tile is drawn, and `loud` throws its glass away from `from`. */
  private break(key: number, index: number, opening: number, o: Opening, loud: boolean, from: P | null, live = true) {
    const buildings = this.broken.get(key) ?? new Map<number, Set<number>>()
    this.broken.set(key, buildings)
    const openings = buildings.get(index) ?? new Set<number>()
    buildings.set(index, openings)
    if (openings.has(opening)) return
    openings.add(opening)
    this.write(key, index, opening)
    if (loud && live) this.shatter(o, from)
  }

  /** Draws pane `opening` of building `index` of tile `key` as broken, or as nothing when it is gone, if the tile is drawn. */
  private write(key: number, index: number, opening: number) {
    const loaded = this.loaded.get(key)
    if (!loaded) return
    const pane = loaded.panes.get(paneKey(index, opening))
    const glass = glassOf(loaded.group)
    const range = loaded.tile.buildings[index]?.look
    if (pane === undefined || !glass || !range || !loaded.tile.look) return
    const o = openingsFor(loaded.tile.look, range)[opening]
    if (!o) return
    const { geometry } = glass
    writePane(geometry.positions, geometry.normals!, pane, o, true, this.isGone(key, index, opening))
    geometry.needsUpdateRange(pane * PANE_VERTICES, PANE_VERTICES)
  }

  /** Throws the glass of pane `o`: in through the window when `from` is outside, out when it is inside. */
  private shatter(o: Opening, from: P | null) {
    const c = paneCentre(o)
    const towards = from && (from[0] - c[0]) * o.normal[0] + (from[1] - c[1]) * o.normal[1] + (from[2] - c[2]) * o.normal[2] > 0 ? -1 : 1
    const next = random(Math.floor(hash(c[0], c[1], c[2], 7) * 1e9))
    const shapes = this.shapesOf()
    glassShards(o, SHARDS_PER_PANE).forEach(({ at, size }, k) => {
      const mesh = new Mesh(shapes[k % shapes.length]!, glassMaterial)
      mesh.position.set(at[0], at[1], at[2])
      mesh.scale.set(size, size, size)
      mesh.rotation.set(next() * 6.28, next() * 6.28, next() * 6.28)
      mesh.castShadow = false
      const speed = 1.5 + next() * 1.5
      const velocity = new Vector3(o.normal[0] * towards * speed + (next() - 0.5) * 0.8, 0.5 + next(), o.normal[2] * towards * speed + (next() - 0.5) * 0.8)
      const spin = new Vector3((next() - 0.5) * 12, (next() - 0.5) * 12, (next() - 0.5) * 12)
      this.debris.throw(mesh, velocity, spin, SHARD_REST, SHARD_LIFE, [], SHARD_SINK)
    })
  }

  /** A few flat jagged triangles about a metre across, scaled down for each shard. */
  private shapesOf(): Geometry[] {
    if (this.shapes) return this.shapes
    this.shapes = []
    for (let k = 0; k < SHAPES; k++) {
      const next = random(1234 + k * 977)
      const corners: P[] = []
      for (let i = 0; i < 3; i++) {
        const angle = (i / 3 + next() * 0.2) * Math.PI * 2
        const r = 0.3 + next() * 0.3
        corners.push([Math.cos(angle) * r, (next() - 0.5) * 0.02, Math.sin(angle) * r])
      }
      const geometry = new Geometry({ positions: Float32Array.from(corners.flat()) })
      geometry.computeNormals()
      this.shapes.push(geometry)
    }
    return this.shapes
  }
}
