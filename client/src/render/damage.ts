import { Color, Geometry, Group, Mesh, Quaternion, Vector3 } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { DAMAGE_STAGES } from '@/sim/consts'
import type { TileData } from '@/world'
import { shellOf, type Shell } from '@/sim/carve'
import type { Effects } from './effects'
import { dustCloud, type Particles } from './particles'
import { Fracture, shardGeometry } from './fracture'
import { lookChunk, lookFracture } from './lookfracture'
import { rubbleSoup, shardSoup, type RubbleMemo } from './shards'
import { buildingColors, buildingSurfaces } from './buildingmesh'
import { lookSlice, type GroundGrid, type LookSource, type LookWorker } from './lookworker'
import { span as timed } from '@/spans'
import { reachOf, type Reach } from './reach'

/** A small deterministic generator, so every client throws the same pieces the same way. */
function random(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** Most pieces thrown from a building at once, and how long they lie where they land, seconds. */
const MAX_FALLING = 48
/** Pieces a building keeps lying about; past that the oldest start sinking at once. */
const KEEP_RESTING = 60
/** A piece lies where it landed this long, then sinks slowly into the ground over `SINK_SECONDS`: gone 30 s after it lands. */
const LIE_SECONDS = 10
const SINK_SECONDS = 20
/** Longest anything flies before it is put on the ground, seconds, so nothing can hang or tumble in the air for ever. */
const MAX_AIR_SECONDS = 8
const GRAVITY = 9.8
/** Cells in a falling section from which it raises a heavy cloud. */
const BIG_FALL = 12
/** A building that was just hit or just lost its hold smoulders this long, easing out, seconds; only this near the camera, metres. */
const SMOULDER_SECONDS = 60
const SMOULDER_RANGE = 160
/** The gap between puffs, seconds, when it burns hottest, and how much longer it grows as it dies away. */
const PUFF_SECONDS = 0.5
const PUFF_STRETCH = 6
/** A point `(x, y, z)`, metres. */
type Mark = readonly [number, number, number]

interface Hurt {
  stage: number
  /** The cells a blast took out of it, by `Shell` number: gone for good. */
  removed: Set<number>
  /** Damage this player's own rounds have done to cells that still stand: they are drawn pushed in. */
  dents: Map<number, number>
  /** Seconds it will still smoulder; 0 when it does not. */
  heat: number
}

/** A bite that just happened: what to throw, and from where. */
interface Burst {
  blasted: number[]
  fell: number[]
  point: Mark | null
}

/** The most cells in one piece thrown from a blast, and in one chunk that falls whole. */
const THROWN_CELLS = 14
const FALLEN_CELLS = 108
/** How hard a broken piece is thrown from the blast, m/s. */
const THROW = 3
/** How fast a piece that lost its hold slides towards the gap, metres a second. */
const LEAN_SPEED = 0.7
/** Seconds from a live loss to the rubble being heaped: about as long as what fell takes to land. */
const RUBBLE_DELAY = 1.1
/** Shard shapes made for each building colour. */
const SHARD_SHAPES = 4
/** Loose shingles thrown off a tiled or shingled roof where it is hit, at most, and how big a plate is (metres). */
const PLATES = 6
const PLATE = [0.22, 0.02, 0.33] as const
/** Roof surfaces that are laid in loose pieces (`Surface` Tiles and Shingles). */
const PLATED = new Set([5, 9])

/** A shard that broke away, on its way down or lying where it fell. */
interface Falling {
  mesh: Mesh
  velocity: Vector3
  spin: Vector3
  age: number
  resting: boolean
  /** Its geometry is one of the kept shard shapes (`shardGeometry`), not to be freed with it. */
  shared?: boolean
  /** A section of wall: it topples over on the ground until it lies flat. */
  section?: boolean
  /** A section toppling, once down: from and to which turn, how long it takes and has taken (seconds), and the corner it turns about (in its own frame). */
  topple?: { from: Quaternion; to: Quaternion; time: number; spent: number; foot: Vector3 }
  /** Seconds it has lain still on the ground. */
  lain?: number
  /** Seconds it has been sinking into the ground: once it has lain `LIE_SECONDS`, or sooner when too many lie about. */
  sinking?: number
  /** How far it sinks, metres: its height as it lies, and a little more. Measured once, as it no longer turns. */
  depth?: number
  /** It has gone into the water, and splashed doing it. */
  wet?: boolean
}

interface Drawn {
  group: Group
  /** The building's walls and roof as cells, once it is hurt, and what is drawn of them. */
  fracture: Fracture | null
  /** What is drawn of the cells still standing, a mesh a brick (`Fracture.bricksNear`). */
  bricks: Map<number, Mesh>
  /**
   * For a building with a look (`Tile.look`): its own walls and roof with the holes cut in them,
   * one mesh, drawn instead of the bricks; and what it is drawn from.
   */
  look: Mesh | null
  seen: { key: number; loaded: Loaded; index: number } | null
  /** A redraw of the look is out with the worker (`BuildingDamage.looks`): the next waits for it. */
  asking: boolean
  /**
   * Its first look is out with the worker and the tile still draws it whole, as it was before it was
   * hit: the tile goes on doing so until the look is back (`onShown`), so it never vanishes meanwhile.
   */
  waiting: boolean
  /** Its tile went or the session was reset: a redraw that comes back now is dropped. */
  released: boolean
  /** The rubble heaped round a building with a look, worked out from the cells that are gone; and seconds until it is next heaped, while what fell is still falling (0 when it is up to date). */
  rubble: Mesh | null
  rubbleDue: number
  /** The mounds of `rubble` by square, so a heap only makes again the squares that changed. */
  mounds: RubbleMemo
  /** The state the bricks in `dirty` are to be redrawn for at the next `update`: a barrage redraws a building once a frame, not once a hit. */
  stale: Hurt | null
  dirty: Set<number>
  falling: Falling[]
  /** Seconds to the next puff of smoke. */
  puff: number
  /** Sections that lost their hold, to be cut out by the look worker with the next redraw and to fall when it shows their hole. */
  sections: PendingSection[]
  /** A heap is out with the look worker; and the state to heap again for once it is back, when more fell meanwhile. */
  heaping: boolean
  heapAgain: Hurt | null
}

/**
 * A section of wall or roof that lost its hold, waiting to be cut out (`lookChunk`) and to fall: its
 * cells, where it turns about (metres), and how it goes, drawn when it broke away.
 */
interface PendingSection {
  cells: number[]
  pivot: Vector3
  velocity: Vector3
  spin: Vector3
}

/** A section's mesh, as the look worker or `lookChunk` made it; with its normals, bounds and reach (and packed) when the worker made it. */
interface SectionMesh {
  positions: Float32Array
  colors: Float32Array
  packed?: ArrayBuffer
  normals?: Float32Array
  bounds?: { x: number; y: number; z: number; radius: number }
  corners?: Float32Array
  thin?: [number, number, number]
}

/** `hex` as linear `[r, g, b]`. */
function rgbOf(hex: number): readonly [number, number, number] {
  const c = Color.hex(hex)
  return [c.r, c.g, c.b] as const
}

/** Ground sampled round a building for its heap, metres apart at the finest, and at most this many samples. */
const GROUND_STEP = 0.5
const GROUND_SAMPLES = 4096
/** How far past its cells a building's heap can reach, metres: a mound's widest, and its plates. */
const HEAP_REACH = 4

interface Loaded {
  tile: TileData
  salt: number
}

/** Dust to be raised where a shard just came away, drained by `update`. */
interface Puff {
  at: Vector3
  size: number
  /** How much longer and slower than a puff it hangs about: 1 for a puff, more for a building coming down. */
  linger?: number
}

const pieceMaterial = toonMaterial({ vertexColors: true, side: 'double' })
// Double-sided: through a hole you see the inside of the far wall and the underside of the roof.
const hurtMaterial = toonMaterial({ vertexColors: true, side: 'double', walls: true })

/** A wall triangle: the three corners and its unit normal (arbitrary side), metres. */
interface Wall {
  corners: Corners
  normal: Vector3
  area: number
}

type Corners = readonly [Vector3, Vector3, Vector3]

/** The side `corners` wind towards: its unit normal and its area. */
function faceOf([a, b, c]: Corners) {
  const across = new Vector3().subVectors(c, b).cross(new Vector3().subVectors(a, b))
  const twice = across.length()
  return { normal: across.divideScalar(twice || 1), area: twice / 2 }
}

/** Building `index`'s own wall triangles in the collision mesh, for one with no cells (`smokeSpot`). */
function wallsOf(tile: TileData, index: number): Wall[] {
  const building = tile.buildings[index]
  if (!building) return []
  const { vertices, triangles } = tile.collision
  const walls: Wall[] = []
  for (let t = building.first; t < building.first + building.walls; t++) {
    const [a, b, c] = [0, 1, 2].map((k) => new Vector3().fromArray(vertices, triangles[t * 3 + k]! * 3)) as [Vector3, Vector3, Vector3]
    // The collision mesh is wound inwards (the renderer reverses it), so the outward normal is the
    // one of the reversed triangle.
    const corners: Corners = [a, c, b]
    const { normal, area } = faceOf(corners)
    if (area < 0.01) continue
    walls.push({ corners, normal, area })
  }
  return walls
}

/**
 * What a hurt building looks like beyond being whole. Just hit or about to fall, it smoulders for a minute. Once cells have been blasted out of it, or this player's rounds have
 * dented some, its walls and roof are redrawn from their cells (`Fracture`) without those that are
 * gone, and the pieces fly and fall. Pieces live in `root` in world coordinates; what a
 * building has suffered is kept, so a tile that streams back in is hurt as it was. Buildings drawn
 * from their cells are to be left out of the tile's own drawing (`fractured`).
 */
export class BuildingDamage {
  readonly root = new Group()
  private readonly hurt = new Map<number, Map<number, Hurt>>()
  private readonly drawn = new Map<number, Map<number, Drawn>>()
  private readonly loaded = new Map<number, Loaded>()
  private readonly puffs: Puff[] = []
  private readonly rand = random(99)
  /**
   * Height of the ground at `(x, z)`, metres, where what falls comes to rest; null where it is not
   * known (no terrain), and then the building's floor is used.
   */
  groundAt: (x: number, z: number) => number | null = () => null
  /** Height of the water's surface at `(x, z)`, metres; null where there is none. A piece falling into it splashes. */
  waterAt: (x: number, z: number) => number | null = () => null
  /** A piece `size` metres across went into the water at `(x, y, z)` (on its surface) at `speed` m/s. */
  splashed: (x: number, y: number, z: number, size: number, speed: number) => void = () => {}
  /** A piece `size` metres across came down on the ground at `(x, y, z)`: a section of wall when it has toppled flat. */
  landed: (x: number, y: number, z: number, size: number) => void = () => {}
  /**
   * Where buildings with a look are redrawn when hurt, off the main thread; null to redraw them here
   * (tests, or no workers). A redraw is then a frame or two late, under the dust.
   */
  looks: LookWorker | null = null
  /**
   * Building `index` of tile `key`, whole when it was hit, is drawn here now that its first look came
   * back from `looks`: the tile is to stop drawing it whole (`fractured` has it from now on).
   */
  onShown: (key: number, index: number) => void = () => {}
  /** Shard shapes by colour and shape (`shardGeometry`), kept for the session. */
  private readonly shards = new Map<number, ReturnType<typeof shardGeometry>>()

  /**
   * Building `index` of tile `key` is now at damage `stage`; one on the point of falling smoulders.
   */
  hurtBuilding(key: number, index: number, stage: number) {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>(), heat: 0 }
    buildings.set(index, state)
    const before = state.stage
    state.stage = Math.max(state.stage, Math.min(stage, DAMAGE_STAGES))
    // Reaching the last stage is what sets it smouldering; the stages it passed through on the way do not.
    if (before < DAMAGE_STAGES && state.stage === DAMAGE_STAGES) state.heat = SMOULDER_SECONDS
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, true, null)
  }

  /**
   * A round of `damage` from this player struck cell `cell` of building `index` of tile `key`, which
   * still stands: it is drawn pushed in by what it has taken. Nothing is replicated: the server
   * breaks the cell out when its health is gone, and then it is simply removed. True when this is
   * the first time the building is drawn as pieces.
   */
  dent(key: number, index: number, cell: number, damage: number): boolean {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>(), heat: 0 }
    buildings.set(index, state)
    const wasWhole = state.removed.size === 0 && state.dents.size === 0
    if (state.removed.has(cell)) return false
    state.dents.set(cell, (state.dents.get(cell) ?? 0) + damage)
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, false, null, [cell])
    if (wasWhole) this.awaitFirst(key, index)
    return wasWhole
  }

  /**
   * A blast took the cells `blasted` out of building `index` of tile `key`, and left `fell` with
   * nothing under them. `live` is false for the catch-up on joining. True when this is the first time
   * the building is drawn as pieces, so the tile has to stop drawing it whole.
   */
  carve(key: number, index: number, blasted: readonly number[], fell: readonly number[], point: Mark | null, live: boolean): boolean {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>(), heat: 0 }
    buildings.set(index, state)
    const wasWhole = state.removed.size === 0 && state.dents.size === 0
    const fresh = { blasted: blasted.filter((c) => !state.removed.has(c)), fell: fell.filter((c) => !state.removed.has(c)), point }
    for (const cell of [...blasted, ...fell]) state.removed.add(cell)
    // Something fresh was lost, now: it smoulders from here. The catch-up on joining shows only what is left, cold.
    if (live && (fresh.blasted.length > 0 || fresh.fell.length > 0)) state.heat = SMOULDER_SECONDS
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, live, fresh, [...fresh.blasted, ...fresh.fell])
    for (const cell of state.removed) state.dents.delete(cell)
    if (wasWhole && state.removed.size > 0) this.awaitFirst(key, index)
    return wasWhole && state.removed.size > 0
  }

  /**
   * The cells `cells` of building `index` of tile `key` stand after all: they were drawn as gone on
   * a prediction the server did not bear out (`WorldView.forgetRams`). The building is redrawn with
   * them; the pieces already thrown lie where they fell.
   */
  mend(key: number, index: number, cells: readonly number[]) {
    const state = this.hurt.get(key)?.get(index)
    if (!state) return
    const back = cells.filter((cell) => state.removed.delete(cell))
    const drawn = this.drawn.get(key)?.get(index)
    if (back.length === 0 || !drawn?.fracture) return
    if (drawn.seen) {
      if (drawn.look) drawn.stale = state
      else this.redraw(drawn, state)
      this.heap(drawn, state)
    } else {
      for (const brick of drawn.fracture.bricksNear(back)) drawn.dirty.add(brick)
      this.redraw(drawn, state)
    }
  }

  /** The cells of each hurt building of tile `key` that are gone, by building index. */
  carved(key: number): Map<number, ReadonlySet<number>> {
    const out = new Map<number, ReadonlySet<number>>()
    for (const [index, state] of this.hurt.get(key) ?? []) if (state.removed.size > 0) out.set(index, state.removed)
    return out
  }

  /** The buildings of tile `key` that are drawn here from their cells and so are not to be drawn whole. */
  fractured(key: number): number[] {
    const out: number[] = []
    const drawn = this.drawn.get(key)
    for (const [index, state] of this.hurt.get(key) ?? []) {
      if (drawn?.get(index)?.waiting) continue
      if (state.removed.size > 0 || state.dents.size > 0) out.push(index)
    }
    return out
  }

  /** Tile `key` is drawn now: show what its buildings have suffered so far. */
  attach(key: number, tile: TileData, salt = 0) {
    const loaded = { tile, salt }
    this.loaded.set(key, loaded)
    for (const [index, state] of this.hurt.get(key) ?? []) this.draw(key, index, loaded, state, false, null)
  }

  /** Tile `key` was unloaded. What was done to its buildings is remembered. */
  detach(key: number) {
    this.loaded.delete(key)
    for (const drawn of this.drawn.get(key)?.values() ?? []) this.release(drawn)
    this.drawn.delete(key)
  }

  /** What building `index` of tile `key` shows, for tests. */
  shown(key: number, index: number): { cells: number } {
    return { cells: this.hurt.get(key)?.get(index)?.removed.size ?? 0 }
  }

  /**
   * Pieces fall and rest, and buildings that were just hit or lost their hold smoulder when `near` (the camera) is close enough to
   * see it, easing out over `SMOULDER_SECONDS`.
   */
  update(dt: number, effects: Effects, near: Vector3, smoke?: Pick<Particles, 'spawn'>) {
    timed('damage.update', () => this.advance(dt, effects, near, smoke))
  }

  /** What `update` does, timed. */
  private advance(dt: number, effects: Effects, near: Vector3, smoke?: Pick<Particles, 'spawn'>) {
    for (const puff of this.puffs.splice(0)) {
      if (smoke) dustCloud(smoke, puff.at, puff.size, puff.linger)
      else effects.dust(puff.at, puff.size)
    }
    for (const [key, buildings] of this.drawn) {
      const loaded = this.loaded.get(key)
      if (!loaded) continue
      for (const [index, drawn] of buildings) {
        if (drawn.stale) this.redraw(drawn, drawn.stale)
        if (drawn.rubbleDue > 0) {
          drawn.rubbleDue -= dt
          const state = this.hurt.get(key)?.get(index)
          if (drawn.rubbleDue <= 0 && state) {
            drawn.rubbleDue = 0
            this.heap(drawn, state)
          }
        }
        this.fall(drawn, loaded.tile.buildings[index]?.base ?? 0, dt)
        const state = this.hurt.get(key)?.get(index)
        if (!state || state.heat <= 0) continue
        state.heat = Math.max(0, state.heat - dt)
        // How hot it still is: 1 at the start, easing to 0 at the end.
        const heat = (state.heat / SMOULDER_SECONDS) ** 2
        drawn.puff -= dt
        if (drawn.puff > 0 || !smoke) continue
        drawn.puff = PUFF_SECONDS * (0.7 + this.rand() * 0.6) * (1 + PUFF_STRETCH * (1 - heat))
        // From the top of the building, where the roof is open or gone.
        const at = this.smokeSpot(loaded.tile, index, state)
        if (!at || at.distanceToSquared(near) > SMOULDER_RANGE * SMOULDER_RANGE) continue
        smolder(smoke, at, heat)
      }
    }
  }

  /** Whether no building has been hurt. */
  get untouched() {
    return this.hurt.size === 0
  }

  /** Forgets every building's hurt: they are whole again. */
  reset() {
    for (const buildings of this.drawn.values()) for (const drawn of buildings.values()) this.release(drawn)
    this.hurt.clear()
    this.drawn.clear()
  }

  dispose() {
    this.reset()
    this.looks?.dispose()
    this.looks = null
    for (const geometry of this.shards.values()) geometry.dispose()
    this.shards.clear()
    this.root.removeFromParent()
    this.loaded.clear()
  }

  /** Brings tile `key`'s building `index` up to `state`; `live` when it is happening now, to be seen. */
  private draw(key: number, index: number, loaded: Loaded, state: Hurt, live: boolean, burst: Burst | null, cells: readonly number[] = []) {
    const buildings = this.drawn.get(key) ?? new Map<number, Drawn>()
    this.drawn.set(key, buildings)
    let drawn = buildings.get(index)
    const building = loaded.tile.buildings[index]
    // Nothing to draw hurt: neither cells nor walls of its own.
    if (!building || (!shellOf(loaded.tile, index) && building.walls === 0)) return
    if (!drawn) {
      drawn = { group: new Group(), fracture: null, bricks: new Map(), look: null, seen: null, asking: false, waiting: false, released: false, rubble: null, rubbleDue: 0, mounds: new Map(), stale: null, dirty: new Set(), falling: [], puff: 0, sections: [], heaping: false, heapAgain: null }
      buildings.set(index, drawn)
      this.root.add(drawn.group)
    }
    if (state.removed.size > 0 || state.dents.size > 0) this.shatter(drawn, key, index, loaded, state, live, burst, cells)
  }

  /** Redraws the building without the cells that are gone and with the ones that were shot pushed in. */
  private shatter(drawn: Drawn, key: number, index: number, loaded: Loaded, state: Hurt, live: boolean, burst: Burst | null, cells: readonly number[]) {
    const building = loaded.tile.buildings[index]!
    // Cubes: through a hole you see the inner faces of the walls round it, so no room needs drawing.
    drawn.fracture ??= timed('damage.fracture', () => new Fracture(loaded.tile, index, loaded.salt))
    const fracture = drawn.fracture
    const shell = fracture.shell
    const looked = !!(loaded.tile.look && building.look && shell)
    if (looked) drawn.seen = { key, loaded, index }
    // Thrown first: the sections that fall whole go to the look worker with the redraw that shows their hole.
    if (live && burst && shell) {
      timed('damage.throw', () => {
        this.throwCells(drawn, fracture, shell, building.base, burst.blasted, burst.point, false, state.removed)
        this.throwCells(drawn, fracture, shell, building.base, burst.fell, burst.point, true, state.removed)
      })
    }
    if (looked) {
      // Its own walls and roof with the holes cut in them, redrawn whole: the first time now, so the tile's
      // own drawing of the building can stop, and after that at the frame's end, so a barrage or a burst of
      // rounds redraws it once a frame, not once a hit.
      if (drawn.look) drawn.stale = state
      else this.redraw(drawn, state)
      // What fell lands about a second later; till then the heap stays as it was. A round that only dented
      // a wall takes nothing out of it, so the heap is left as it is.
      if (live) drawn.rubbleDue = RUBBLE_DELAY
      else if (burst || !drawn.rubble) this.heap(drawn, state)
    } else {
      // The first time it is drawn now, so the tile's own drawing of the building can stop; after that a live hit waits for the frame's end.
      for (const brick of fracture.bricksNear(cells)) drawn.dirty.add(brick)
      if (drawn.bricks.size > 0 && live) drawn.stale = state
      else this.redraw(drawn, state)
    }
  }

  /** Draws the bricks of `drawn` that changed, as `state` has the cells; all of them the first time. */
  private redraw(drawn: Drawn, state: Hurt) {
    const fracture = drawn.fracture
    if (!fracture) return
    drawn.stale = null
    if (drawn.seen) {
      this.redrawLook(drawn, fracture, drawn.seen.loaded, drawn.seen.index, state)
      return
    }
    const first = drawn.bricks.size === 0
    const todo = first ? fracture.allBricks() : drawn.dirty
    for (const key of todo) {
      const built = fracture.buildBrick(key, state.removed, state.dents)
      const old = drawn.bricks.get(key)
      if (!built) {
        if (old) {
          old.geometry.dispose()
          old.removeFromParent()
          drawn.bricks.delete(key)
        }
        continue
      }
      if (old) {
        old.geometry.dispose()
        old.geometry = shardGeometry(built.positions, built.colors)
        continue
      }
      const mesh = new Mesh(shardGeometry(built.positions, built.colors), hurtMaterial)
      mesh.castShadow = true
      mesh.receiveShadow = true
      drawn.group.add(mesh)
      drawn.bricks.set(key, mesh)
    }
    drawn.dirty.clear()
  }

  /**
   * Heaps the rubble of what is gone from a building with a look round its foot: in the look worker
   * when there is one (a frame or two late, under the dust), else here.
   */
  private heap(drawn: Drawn, state: Hurt) {
    const seen = drawn.seen
    const shell = drawn.fracture?.shell
    const looks = this.looks
    if (!seen || !shell || state.removed.size === 0) return
    const { tile, salt } = seen.loaded
    const building = tile.buildings[seen.index]
    if (!building) return
    const { wall, roof } = buildingColors(tile, seen.index, salt)
    if (!looks) {
      timed('damage.heap', () => {
        const ids = [...state.removed].filter((id) => shell.isSolid(id)).sort((a, b) => a - b)
        const centres = ids.map((id) => shell.centre(id).map((v) => v / 100) as unknown as [number, number, number])
        const soup = rubbleSoup(centres, ids.map((id) => shell.isRoof(id)), (x, z) => this.groundUnder(x, z, building.base), rgbOf(wall), rgbOf(roof), drawn.mounds)
        this.showHeap(drawn, shardGeometry(soup.positions, soup.colors))
      })
      return
    }
    // One heap out at a time: what falls meanwhile is heaped by the next, once this one is back.
    if (drawn.heaping) {
      drawn.heapAgain = state
      return
    }
    drawn.heaping = true
    const ask = timed('damage.heapAsk', () => ({ removed: Uint32Array.from(state.removed), ground: this.groundGrid(shell, building.base), wall: rgbOf(wall), roof: rgbOf(roof) }))
    looks.heap(this.lookName(drawn), () => this.lookSource(drawn), ask).then(
      ({ positions, colors, normals, bounds, packed }) => {
        drawn.heaping = false
        if (drawn.released) return
        timed('damage.showHeap', () => this.showHeap(drawn, new Geometry({ positions, colors, normals, bounds, ...(packed ? { packed } : {}) })))
        const again = drawn.heapAgain
        drawn.heapAgain = null
        if (again) this.heap(drawn, again)
      },
      (error: unknown) => {
        drawn.heaping = false
        console.warn('heaping rubble on the main thread:', error)
        this.looks = null
        drawn.heapAgain = null
        if (!drawn.released) this.heap(drawn, state)
      },
    )
  }

  /** Shows `geometry` as `drawn`'s heap of rubble, in place of the one before. */
  private showHeap(drawn: Drawn, geometry: Geometry) {
    if (drawn.rubble) {
      drawn.rubble.geometry.dispose()
      drawn.rubble.geometry = geometry
      return
    }
    const mesh = new Mesh(geometry, pieceMaterial)
    mesh.castShadow = true
    mesh.receiveShadow = true
    drawn.group.add(mesh)
    drawn.rubble = mesh
  }

  /**
   * The ground round `shell`'s cells as far as a heap can reach, for the look worker: as `groundUnder`
   * has it, `floor` where nothing is known, `GROUND_STEP` apart or wider on a big building.
   */
  private groundGrid(shell: Shell, floor: number): GroundGrid {
    const [w, d] = [(shell.dims[0] * shell.cell) / 100 + 2 * HEAP_REACH, (shell.dims[2] * shell.cell) / 100 + 2 * HEAP_REACH]
    const step = Math.max(GROUND_STEP, Math.sqrt((w * d) / GROUND_SAMPLES))
    const [nx, nz] = [Math.ceil(w / step) + 1, Math.ceil(d / step) + 1]
    const [x0, z0] = [shell.origin[0] / 100 - HEAP_REACH, shell.origin[2] / 100 - HEAP_REACH]
    const heights = new Float32Array(nx * nz)
    for (let j = 0; j < nz; j++) for (let i = 0; i < nx; i++) heights[j * nx + i] = this.groundUnder(x0 + i * step, z0 + j * step, floor)
    return { x0, z0, step, nx, nz, heights }
  }

  /** The name `drawn`'s building goes by with the look worker. */
  private lookName(drawn: Drawn): string {
    return `${drawn.seen?.key}:${drawn.seen?.index}`
  }

  /** What the look worker is told of `drawn`'s building the first time: its own look and its cells. */
  private lookSource(drawn: Drawn): LookSource {
    const seen = drawn.seen!
    const look = seen.loaded.tile.look!
    const range = seen.loaded.tile.buildings[seen.index]!.look!
    return { ...lookSlice(look, range), shell: drawn.fracture!.shell!.flags }
  }

  /**
   * Starts section `pending` of `drawn` falling, as `mesh` (from the look worker, or `lookChunk` here):
   * nothing when no wall or roof passed through its cells.
   */
  private launchSection(drawn: Drawn, mesh: SectionMesh | null, pending: PendingSection) {
    if (!mesh || (mesh.positions.length === 0 && !mesh.packed?.byteLength)) return
    const geometry = mesh.normals ? new Geometry({ positions: mesh.positions, colors: mesh.colors, normals: mesh.normals, ...(mesh.bounds ? { bounds: mesh.bounds } : {}), ...(mesh.packed ? { packed: mesh.packed } : {}) }) : shardGeometry(mesh.positions, mesh.colors)
    if (mesh.corners && mesh.thin) reaches.set(geometry, { corners: mesh.corners, thin: mesh.thin })
    const piece = new Mesh(geometry, pieceMaterial)
    piece.position.copy(pending.pivot)
    piece.castShadow = true
    drawn.group.add(piece)
    drawn.falling.push({ mesh: piece, velocity: pending.velocity, spin: pending.spin, age: 0, resting: false, section: true })
  }

  /** Section `pending` of `drawn`'s building cut out here, on the main thread (`lookChunk`). */
  private cutHere(drawn: Drawn, pending: PendingSection): SectionMesh | null {
    const seen = drawn.seen
    const shell = drawn.fracture?.shell
    const range = seen?.loaded.tile.buildings[seen.index]?.look
    if (!seen || !shell || !seen.loaded.tile.look || !range) return null
    const { wall, roof } = buildingColors(seen.loaded.tile, seen.index, seen.loaded.salt)
    return timed('damage.cut', () => lookChunk(seen.loaded.tile.look!, range, shell, new Set(pending.cells), [pending.pivot.x, pending.pivot.y, pending.pivot.z], rgbOf(wall), rgbOf(roof), buildingSurfaces(seen.loaded.tile, seen.index, seen.loaded.salt)))
  }

  /**
   * A building of tile `key` that was whole has just been hit: if its first look went to the worker,
   * the tile keeps drawing it whole until it is back.
   */
  private awaitFirst(key: number, index: number) {
    const drawn = this.drawn.get(key)?.get(index)
    if (drawn && drawn.asking && !drawn.look) drawn.waiting = true
  }

  /** Draws a building with a look from it, the holes in `state` cut in: in the worker when there is one. */
  private redrawLook(drawn: Drawn, fracture: Fracture, loaded: Loaded, index: number, state: Hurt) {
    const { tile, salt } = loaded
    const range = tile.buildings[index]?.look
    const shell = fracture.shell
    const look = tile.look
    if (!look || !range || !shell) return
    const { wall, roof } = buildingColors(tile, index, salt)
    const surfaces = buildingSurfaces(tile, index, salt)
    const looks = this.looks
    if (!looks) {
      const soup = lookFracture(look, range, shell, state.removed, state.dents, rgbOf(wall), rgbOf(roof), surfaces)
      this.showLook(drawn, index, shardGeometry(soup.positions, soup.colors, soup.custom))
      // Sections left waiting when the worker failed are cut out here.
      for (const pending of drawn.sections.splice(0)) this.launchSection(drawn, this.cutHere(drawn, pending), pending)
      return
    }
    // One redraw out at a time: what changes meanwhile is drawn by the next, asked for by `update` once this one is back.
    if (drawn.asking) {
      drawn.stale = state
      return
    }
    drawn.asking = true
    const dents = new Float64Array(state.dents.size * 2)
    let at = 0
    for (const [cell, damage] of state.dents) {
      dents[at++] = cell
      dents[at++] = damage
    }
    const sections = drawn.sections.splice(0)
    const ask = {
      removed: Uint32Array.from(state.removed),
      dents,
      wall: rgbOf(wall),
      roof: rgbOf(roof),
      surfaces,
      sections: sections.map((s) => ({ cells: Uint32Array.from(s.cells), pivot: [s.pivot.x, s.pivot.y, s.pivot.z] as [number, number, number] })),
    }
    looks.draw(this.lookName(drawn), () => this.lookSource(drawn), ask).then(
      ({ positions, colors, custom, normals, bounds, packed, sections: cut }) => {
        drawn.asking = false
        if (drawn.released) return
        timed('damage.showLook', () => {
          this.showLook(drawn, index, new Geometry({ positions, colors, custom, normals, bounds, ...(packed ? { packed } : {}) }))
          // The sections fall now that the hole they leave shows.
          sections.forEach((pending, i) => this.launchSection(drawn, cut[i] ?? null, pending))
        })
      },
      (error: unknown) => {
        drawn.asking = false
        // The worker failed: this and every redraw after it are drawn here, from the next `update`, and its sections cut out with it.
        console.warn('drawing hurt buildings on the main thread:', error)
        this.looks = null
        drawn.sections.unshift(...sections)
        if (!drawn.released) drawn.stale = state
      },
    )
  }

  /** Shows `geometry` as building `index`'s look, in place of the one before; the first time, tells the tile when it waits for it. */
  private showLook(drawn: Drawn, index: number, geometry: Geometry) {
    if (drawn.look) {
      drawn.look.geometry.dispose()
      drawn.look.geometry = geometry
      return
    }
    const mesh = new Mesh(geometry, hurtMaterial)
    mesh.castShadow = true
    mesh.receiveShadow = true
    drawn.group.add(mesh)
    drawn.look = mesh
    if (drawn.waiting && drawn.seen) {
      drawn.waiting = false
      this.onShown(drawn.seen.key, index)
    }
  }

  /**
   * Throws the wall and roof in `cells` out of the building as chunks: blasted ones fly away from
   * `point`, ones that were left hanging (`falls`) drop where they were.
   */
  /** Pieces of `drawn` still falling or toppling: those lying still or sinking take no place. */
  private moving(drawn: Drawn): number {
    let n = 0
    for (const piece of drawn.falling) if (!piece.resting) n++
    return n
  }

  private throwCells(drawn: Drawn, fracture: Fracture, shell: NonNullable<Fracture['shell']>, floor: number, cells: readonly number[], point: Mark | null, falls: boolean, removed: ReadonlySet<number>) {
    // A big swath is thrown in bigger chunks rather than dropped past the limit: pieces still falling from the last round take places too.
    const room = Math.max(MAX_FALLING - this.moving(drawn), 8)
    const most = Math.max(falls ? FALLEN_CELLS : THROWN_CELLS, Math.ceil(cells.length / room))
    const seen = drawn.seen
    for (const group of shell.clusters(cells, most)) {
      if (seen) {
        this.throwShards(drawn, seen, shell, floor, group, point, falls, removed)
        continue
      }
      const chunk = fracture.chunk(new Set(group))
      if (!chunk) continue
      this.puffs.push({ at: chunk.pivot.clone(), size: falls ? 3 + this.rand() * 2 : 1.4 + this.rand() * 1.4 })
      if (this.moving(drawn) >= MAX_FALLING * 2) continue
      const mesh = new Mesh(shardGeometry(chunk.positions, chunk.colors), pieceMaterial)
      mesh.position.copy(chunk.pivot)
      mesh.castShadow = true
      drawn.group.add(mesh)
      const velocity = new Vector3()
      if (!falls && point) {
        velocity.set(chunk.pivot.x - point[0], chunk.pivot.y - point[1], chunk.pivot.z - point[2])
        if (velocity.lengthSq() < 1e-4) velocity.set(0, 1, 0)
        velocity.normalize().multiplyScalar(THROW * (0.5 + this.rand())).y += 1 + this.rand() * 2
      }
      const tumble = falls ? 0.6 : 5
      const spin = new Vector3((this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble)
      // A piece that lost its hold tips over towards the gap, top first, and slides that way a little.
      const lean = falls ? leanOf(shell, group, removed) : null
      if (lean) {
        const rate = 0.3 + this.rand() * 0.3
        spin.x += lean[1] * rate
        spin.z -= lean[0] * rate
        velocity.x += lean[0] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
        velocity.z += lean[1] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
      }
      drawn.falling.push({
        mesh,
        velocity,
        spin,
        age: 0,
        resting: false,
      })
    }
  }

  /**
   * A piece of cells `group` of a building with a look, thrown as broken plates of its wall and roof
   * rather than as cubes: one big slab and a few smaller pieces for a piece that falls, a spray of
   * small ones for a piece that is blasted. They go as the cube chunks go (`fall`).
   */
  private throwShards(drawn: Drawn, seen: NonNullable<Drawn['seen']>, shell: NonNullable<Fracture['shell']>, floor: number, group: readonly number[], point: Mark | null, falls: boolean, removed: ReadonlySet<number>) {
    const pivot = new Vector3()
    let roofs = 0
    for (const id of group) {
      const [x, y, z] = shell.centre(id)
      pivot.x += x / 100 / group.length
      pivot.y += y / 100 / group.length
      pivot.z += z / 100 / group.length
      if (shell.isRoof(id)) roofs++
    }
    this.puffs.push({ at: pivot.clone(), size: falls ? 3 + this.rand() * 2 : 1.4 + this.rand() * 1.4 })
    // A fall throws up a cloud where it lands, too, that hides the heap coming.
    if (falls) this.puffs.push({ at: new Vector3(pivot.x, this.groundUnder(pivot.x, pivot.z, floor) + 0.6, pivot.z), size: 4 + this.rand() * 2 + Math.min(4, group.length / 20) })
    // A big section coming down raises a heavy cloud that rolls out round it and hangs, hiding the heap.
    if (falls && group.length >= BIG_FALL) {
      const reach = 1 + Math.sqrt(group.length) * 0.3
      for (let i = 0; i < 3; i++) {
        const angle = (i / 3 + this.rand() * 0.3) * Math.PI * 2
        const [x, z] = [pivot.x + Math.cos(angle) * reach, pivot.z + Math.sin(angle) * reach]
        this.puffs.push({ at: new Vector3(x, this.groundUnder(x, z, floor) + 0.8, z), size: 5 + this.rand() * 3, linger: 2.6 })
      }
    }
    // What the cells hold, as a volume: a cell is 72 cm a side, but the wall in it is thinner.
    const volume = group.length * 0.1
    const count = falls ? Math.min(5, 2 + Math.floor(group.length / 6)) : Math.min(8, 3 + Math.floor(group.length / 2))
    const lean = falls ? leanOf(shell, group, removed) : null
    let first = 0
    if (falls) {
      // The section of wall or roof that lost its hold falls whole, its edge the hole's edge, and tips
      // over towards the gap. It always falls, however much is already falling: without it its part of
      // the building would just be gone. With a look worker it is cut out there with the next redraw, and
      // falls when that shows its hole; how it goes is drawn now.
      const velocity = new Vector3()
      const spin = new Vector3((this.rand() - 0.5) * 0.4, (this.rand() - 0.5) * 0.2, (this.rand() - 0.5) * 0.4)
      if (lean) {
        const rate = 0.5 + this.rand() * 0.5
        spin.x += lean[1] * rate
        spin.z -= lean[0] * rate
        velocity.x += lean[0] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
        velocity.z += lean[1] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
      }
      const pending: PendingSection = { cells: [...group], pivot: pivot.clone(), velocity, spin }
      if (this.looks) drawn.sections.push(pending)
      else this.launchSection(drawn, this.cutHere(drawn, pending), pending)
      first = 1
    }
    for (let i = first; i < count; i++) {
      if (this.moving(drawn) >= MAX_FALLING * 2) return
      // The first piece of a fall is most of it; the rest are bits.
      const share = falls ? (i === 0 ? 0.7 : 0.3 / Math.max(1, count - 1)) : 1 / count
      // A unit shard holds about 0.12 m³; most bits are small, none bigger than a door.
      const size = Math.min(falls && i === 0 ? 1.8 : 0.8, Math.max(0.15, Math.cbrt((volume * share) / 0.12) * (0.5 + this.rand() * 0.7)))
      const roof = this.rand() * group.length < roofs
      const geometry = this.shardGeometry(seen, roof, Math.floor(this.rand() * SHARD_SHAPES))
      const mesh = new Mesh(geometry, pieceMaterial)
      mesh.position.set(pivot.x + (this.rand() - 0.5) * 0.6, pivot.y + (this.rand() - 0.5) * 0.6, pivot.z + (this.rand() - 0.5) * 0.6)
      mesh.scale.set(size, size, size)
      mesh.rotation.set(this.rand() * 6.28, this.rand() * 6.28, this.rand() * 6.28)
      mesh.castShadow = true
      drawn.group.add(mesh)
      const velocity = new Vector3()
      if (!falls && point) {
        velocity.set(mesh.position.x - point[0], mesh.position.y - point[1], mesh.position.z - point[2])
        if (velocity.lengthSq() < 1e-4) velocity.set(0, 1, 0)
        velocity.normalize().multiplyScalar(THROW * (0.6 + this.rand() * 1.2)).y += 1 + this.rand() * 2.5
      }
      const tumble = falls ? 1.2 : 7
      const spin = new Vector3((this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble)
      if (lean) {
        velocity.x += lean[0] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
        velocity.z += lean[1] * LEAN_SPEED * (0.6 + this.rand() * 0.8)
      }
      drawn.falling.push({ mesh, velocity, spin, age: 0, resting: false, shared: true })
    }
    // A tiled roof sheds loose shingles too: thin plates from the roof cells hit, tumbling fast.
    if (roofs > 0 && PLATED.has(buildingSurfaces(seen.loaded.tile, seen.index, seen.loaded.salt).roof)) {
      const roofCells = group.filter((id) => shell.isRoof(id))
      for (let i = 0; i < Math.min(PLATES, 1 + roofs); i++) {
        if (this.moving(drawn) >= MAX_FALLING * 2) return
        const [cx, cy, cz] = shell.centre(roofCells[Math.floor(this.rand() * roofCells.length)]!)
        const mesh = new Mesh(this.plateGeometry(seen, Math.floor(this.rand() * SHARD_SHAPES)), pieceMaterial)
        mesh.position.set(cx / 100 + (this.rand() - 0.5) * 0.5, cy / 100 + 0.2 + this.rand() * 0.3, cz / 100 + (this.rand() - 0.5) * 0.5)
        const k = 0.8 + this.rand() * 0.4
        mesh.scale.set(PLATE[0] * k, PLATE[1] * k, PLATE[2] * k)
        mesh.rotation.set(this.rand() * 6.28, this.rand() * 6.28, this.rand() * 6.28)
        mesh.castShadow = true
        drawn.group.add(mesh)
        const velocity = new Vector3(0, 2 + this.rand() * 2, 0)
        if (point) {
          const away = new Vector3(mesh.position.x - point[0], 0, mesh.position.z - point[2])
          if (away.lengthSq() > 1e-4) velocity.addScaledVector(away.normalize(), 2 + this.rand() * 2)
        }
        const spin = new Vector3((this.rand() - 0.5) * 10, (this.rand() - 0.5) * 10, (this.rand() - 0.5) * 10)
        drawn.falling.push({ mesh, velocity, spin, age: 0, resting: false, shared: true })
      }
    }
  }

  /** One of the few shapes of a loose shingle in a building's roof colour: a flat plate about a metre across, scaled down when thrown. */
  private plateGeometry(seen: NonNullable<Drawn['seen']>, variant: number) {
    const { roof } = buildingColors(seen.loaded.tile, seen.index, seen.loaded.salt)
    const key = -(roof * SHARD_SHAPES + variant) - 1
    let geometry = this.shards.get(key)
    if (!geometry) {
      const c = Color.hex(roof)
      const soup = shardSoup(roof ^ 0x51a7e ^ (variant * 7919), [c.r, c.g, c.b], 0.06)
      geometry = shardGeometry(soup.positions, soup.colors)
      this.shards.set(key, geometry)
    }
    return geometry
  }

  /** One of the few shard shapes of a building's wall or roof colour, made once and kept. */
  private shardGeometry(seen: NonNullable<Drawn['seen']>, roof: boolean, variant: number) {
    const { wall, roof: top } = buildingColors(seen.loaded.tile, seen.index, seen.loaded.salt)
    const hex = roof ? top : wall
    const key = hex * SHARD_SHAPES + variant
    let geometry = this.shards.get(key)
    if (!geometry) {
      const c = Color.hex(hex)
      const soup = shardSoup(hex ^ (variant * 7919), [c.r, c.g, c.b], roof ? 0.18 : 0.32)
      geometry = shardGeometry(soup.positions, soup.colors)
      this.shards.set(key, geometry)
    }
    return geometry
  }

  /** Where something falling at `(x, z)` near a building whose floor is at `floor` comes to rest. */
  private groundUnder(x: number, z: number, floor: number): number {
    return this.groundAt(x, z) ?? floor
  }

  /**
   * Moves a building's flung pieces: they drop and tumble until their lowest corner meets the ground,
   * a section of wall goes on tipping over there until it lies flat, and then they lie. They stay:
   * after `LIE_SECONDS` each sinks slowly into the ground and is gone (sooner, oldest first, when a
   * building has more than `KEEP_RESTING` lying about).
   */
  private fall(drawn: Drawn, floor: number, dt: number) {
    let lying = 0
    for (const piece of drawn.falling) if (piece.resting && piece.sinking === undefined) lying++
    // The oldest bits go first, and only then sections of wall, the biggest part of the heap.
    for (const sections of [false, true]) {
      for (let i = 0; i < drawn.falling.length && lying > KEEP_RESTING; i++) {
        const piece = drawn.falling[i]!
        if (!piece.resting || piece.sinking !== undefined || !!piece.section !== sections) continue
        piece.sinking = 0
        lying--
      }
    }
    for (let i = drawn.falling.length - 1; i >= 0; i--) {
      const piece = drawn.falling[i]!
      const mesh = piece.mesh
      piece.age += dt
      if (piece.sinking !== undefined) {
        piece.sinking += dt
        piece.depth ??= span(mesh).height + 0.3
        mesh.position.y -= (dt / SINK_SECONDS) * piece.depth
        if (piece.sinking >= SINK_SECONDS) {
          if (!piece.shared) mesh.geometry.dispose()
          mesh.removeFromParent()
          drawn.falling.splice(i, 1)
        }
        continue
      }
      if (piece.resting) {
        piece.lain = (piece.lain ?? 0) + dt
        if (piece.lain >= LIE_SECONDS) piece.sinking = 0
        continue
      }
      const ground = this.groundUnder(mesh.position.x, mesh.position.z, floor)
      if (piece.topple) {
        this.topple(piece, ground, dt)
        continue
      }
      piece.velocity.y -= GRAVITY * dt
      mesh.position.addScaledVector(piece.velocity, dt)
      mesh.rotation.x += piece.spin.x * dt
      mesh.rotation.y += piece.spin.y * dt
      mesh.rotation.z += piece.spin.z * dt
      const { low, wide } = span(mesh)
      if (!piece.wet) {
        // A bridge's deck falling into the river below it.
        const level = this.waterAt(mesh.position.x, mesh.position.z)
        if (level !== null && level > ground && low <= level) {
          piece.wet = true
          this.splashed(mesh.position.x, level, mesh.position.z, Math.max(0.4, wide), -piece.velocity.y)
        }
      }
      if (low > ground && piece.age < MAX_AIR_SECONDS) continue
      // Its lowest corner on the ground, not under it.
      mesh.position.y += ground - low
      // A section of wall standing on its edge topples over onto its broad side; anything else lies where it lands.
      const toppling = piece.section ? toppleOf(mesh) : null
      if (toppling) {
        piece.topple = toppling
        piece.velocity.set(piece.velocity.x * 0.3, 0, piece.velocity.z * 0.3)
        continue
      }
      piece.resting = true
      if (!piece.wet) this.landed(mesh.position.x, ground, mesh.position.z, wide)
    }
  }

  /**
   * Moves a toppling section on: it turns about the corner it stands on, slowly at first and faster
   * as it goes over, as a slab does, and lies still once flat on the ground.
   */
  private topple(piece: Falling, ground: number, dt: number) {
    const mesh = piece.mesh
    const t = piece.topple!
    t.spent = Math.min(t.time, t.spent + dt)
    // Where the corner it turns about is now, in the world, to keep it there.
    const foot = t.foot.clone().applyQuaternion(mesh.quaternion).add(mesh.position)
    const turn = new Quaternion().copy(t.from).slerp(t.to, (t.spent / t.time) ** 2)
    mesh.quaternion.copy(turn)
    const moved = t.foot.clone().applyQuaternion(mesh.quaternion)
    mesh.position.set(foot.x - moved.x + piece.velocity.x * dt, mesh.position.y, foot.z - moved.z + piece.velocity.z * dt)
    piece.velocity.multiplyScalar(Math.max(0, 1 - 4 * dt))
    // Its lowest corner on the ground: the corner it turns about, or what comes down onto it.
    mesh.position.y += this.groundUnder(mesh.position.x, mesh.position.z, ground) - span(mesh).low
    if (t.spent >= t.time) {
      piece.topple = undefined
      piece.resting = true
      // The slab slams down flat.
      this.landed(mesh.position.x, mesh.position.y, mesh.position.z, Math.max(span(mesh).wide, 2))
    }
  }

  private release(drawn: Drawn) {
    drawn.released = true
    for (const mesh of [...drawn.bricks.values(), ...(drawn.look ? [drawn.look] : []), ...(drawn.rubble ? [drawn.rubble] : []), ...drawn.falling.filter((f) => !f.shared).map((f) => f.mesh)]) mesh.geometry.dispose()
    this.root.remove(drawn.group)
  }

  private pointOn(wall: Wall) {
    return pointInTriangle(wall.corners, this.rand(), this.rand())
  }

  /**
   * Where a puff of smoke rises from building `index` of `tile`: the highest of three of its cells still
   * standing, picked at random, at its top; or of three points of its walls where it has no cells.
   * Null when nothing of it stands.
   */
  private smokeSpot(tile: TileData, index: number, state: Hurt): Vector3 | null {
    const shell = shellOf(tile, index)
    if (!shell) {
      const walls = wallsOf(tile, index)
      if (walls.length === 0) return null
      let at = this.pointOn(walls[Math.floor(this.rand() * walls.length)]!)
      for (let i = 0; i < 2; i++) {
        const other = this.pointOn(walls[Math.floor(this.rand() * walls.length)]!)
        if (other.y > at.y) at = other
      }
      return at
    }
    const cells = shell.cells
    if (cells.length === 0) return null
    let best = -1
    for (let i = 0; i < 3; i++) {
      const id = cells[Math.floor(this.rand() * cells.length)]!
      if (state.removed.has(id)) continue
      if (best < 0 || shell.coords(id)[1] > shell.coords(best)[1]) best = id
    }
    if (best < 0) return null
    const [x, y, z] = shell.centre(best)
    return new Vector3(x / 100, (y + shell.cell / 2) / 100, z / 100)
  }
}

/**
 * The way a piece of cells `group` that has lost its hold leans: the horizontal unit vector `[x, z]`
 * from the piece towards the cells gone from round it (within 3 cells), which is what held it.
 * Null when they are all round it or none are.
 */
function leanOf(shell: NonNullable<Fracture['shell']>, group: readonly number[], removed: ReadonlySet<number>): [number, number] | null {
  const own = new Set(group)
  const reach = 3
  let [cx, cz] = [0, 0]
  const low = [Infinity, Infinity, Infinity]
  const high = [-Infinity, -Infinity, -Infinity]
  for (const id of group) {
    const c = shell.coords(id)
    cx += c[0]
    cz += c[2]
    for (let a = 0; a < 3; a++) {
      low[a] = Math.min(low[a]!, c[a]!)
      high[a] = Math.max(high[a]!, c[a]!)
    }
  }
  cx /= group.length
  cz /= group.length
  let [sx, sz] = [0, 0]
  for (const id of removed) {
    if (own.has(id)) continue
    const c = shell.coords(id)
    if ([0, 1, 2].some((a) => c[a]! < low[a]! - reach || c[a]! > high[a]! + reach)) continue
    sx += c[0] - cx
    sz += c[2] - cz
  }
  const length = Math.hypot(sx, sz)
  return length < 1e-3 ? null : [sx / length, sz / length]
}

/** A point of the triangle `[a, b, c]`, uniform for uniform `u` and `v`. */
function pointInTriangle([a, b, c]: Corners, u: number, v: number) {
  if (u + v > 1) [u, v] = [1 - u, 1 - v]
  return a.clone().addScaledVector(b.clone().sub(a), u).addScaledVector(c.clone().sub(a), v)
}

/** A puff of smoke rising from `at`: dark, thick and big while it is hot, thin and grey as it dies. */
function smolder(smoke: Pick<Particles, 'spawn'>, at: Vector3, heat: number) {
  const tone = (grey: number) => (grey << 16) | (grey << 8) | grey
  const rand = (min: number, max: number) => min + Math.random() * (max - min)
  smoke.spawn({
    position: [at.x, at.y, at.z],
    velocity: [rand(-0.4, 0.4), rand(1.2, 2.2), rand(-0.4, 0.4)],
    life: rand(4, 7),
    size: [0.6 + heat * 0.6, 1.8 + heat * 2.4],
    color: tone(0x30 + 0x20 * (1 - heat)),
    colorEnd: tone(0x8a),
    shape: 'puff',
    spin: [-0.4, 0.4],
    alpha: 0.12 + 0.38 * heat,
    gravity: -0.6,
    drag: 0.7,
  })
}

/** A section turned no further than this from lying flat lies where it lands, radians. */
const FLAT_ENOUGH = 0.3

/**
 * How section `mesh`, its lowest corner just on the ground, topples over: from its turn now to the
 * one that lays its broad side (its geometry's thinnest way across) flat, about its lowest corner,
 * taking longer the taller it stands. Null when it lies about flat already.
 */
export function toppleOf(mesh: Mesh): Falling['topple'] | null {
  const { thin, corners } = reach(mesh.geometry)
  const across = new Vector3(thin[0], thin[1], thin[2]).applyQuaternion(mesh.quaternion)
  const up = new Vector3(0, across.y >= 0 ? 1 : -1, 0)
  const angle = Math.acos(Math.min(1, Math.abs(across.y)))
  if (angle < FLAT_ENOUGH) return null
  const from = new Quaternion().copy(mesh.quaternion)
  const to = new Quaternion().setFromUnitVectors(across, up).multiply(from)
  // Its lowest corner, in its own frame.
  const p = corners
  const at = new Vector3()
  let [low, foot] = [Infinity, new Vector3()]
  for (let i = 0; i < p.length; i += 3) {
    at.set(p[i]!, p[i + 1]!, p[i + 2]!).applyQuaternion(mesh.quaternion)
    if (at.y < low) {
      low = at.y
      foot = new Vector3(p[i]!, p[i + 1]!, p[i + 2]!)
    }
  }
  const { height } = span(mesh)
  return { from, to, time: (0.35 + 0.45 * Math.sqrt(height)) * Math.sqrt(angle / (Math.PI / 2)), spent: 0, foot }
}

/** Each piece's geometry's corners and thinnest way, worked out once, or sent with it by the look worker. */
const reaches = new WeakMap<Geometry, Reach>()

/** The `Reach` of `geometry`: kept from the look worker, or worked out the first time it is asked for. */
function reach(geometry: Geometry): Reach {
  let found = reaches.get(geometry)
  if (!found) {
    found = reachOf(geometry.positions)
    reaches.set(geometry, found)
  }
  return found
}

/**
 * Where `mesh`'s corners reach, in world metres: the lowest, how tall it stands, and how wide it is
 * across, from its geometry turned and scaled as it is now. Measured from the corners `reach` keeps,
 * not every vertex, as it is asked every frame of every piece's fall.
 */
export function span(mesh: Mesh): { low: number; height: number; wide: number } {
  const q = mesh.quaternion
  const [qx, qy, qz, qw] = [q.x, q.y, q.z, q.w]
  const [sx, sy, sz] = [mesh.scale.x, mesh.scale.y, mesh.scale.z]
  // The rows of the rotation matrix that give a turned vector's y, x and z.
  const [ry0, ry1, ry2] = [2 * (qx * qy + qw * qz), 1 - 2 * (qx * qx + qz * qz), 2 * (qy * qz - qw * qx)]
  const [rx0, rx1, rx2] = [1 - 2 * (qy * qy + qz * qz), 2 * (qx * qy - qw * qz), 2 * (qx * qz + qw * qy)]
  const [rz0, rz1, rz2] = [2 * (qx * qz - qw * qy), 2 * (qy * qz + qw * qx), 1 - 2 * (qx * qx + qy * qy)]
  const p = reach(mesh.geometry).corners
  let [low, high, x0, x1, z0, z1] = [Infinity, -Infinity, Infinity, -Infinity, Infinity, -Infinity]
  for (let i = 0; i < p.length; i += 3) {
    const x = p[i]! * sx
    const y = p[i + 1]! * sy
    const z = p[i + 2]! * sz
    const wy = ry0 * x + ry1 * y + ry2 * z
    const wx = rx0 * x + rx1 * y + rx2 * z
    const wz = rz0 * x + rz1 * y + rz2 * z
    if (wy < low) low = wy
    if (wy > high) high = wy
    if (wx < x0) x0 = wx
    if (wx > x1) x1 = wx
    if (wz < z0) z0 = wz
    if (wz > z1) z1 = wz
  }
  return { low: mesh.position.y + low, height: high - low, wide: Math.max(x1 - x0, z1 - z0) }
}
