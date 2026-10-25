import { Group, type Scene } from '@ragnarok/engine'
import type * as THREE from 'three'
import { OBJ_BUILDING, OBJ_TREE, type Built, type CarvedBuilding, type CraterEntry, type DamagedBuilding, type DestroyedObject } from './net/codec'
import type { Effects } from './render/effects'
import { FallingTrees, type TreeFx, type TreeNoise } from './render/falling'
import { BuildingDamage } from './render/damage'
import { ConstructView } from './render/construct'
import { Collapses } from './render/rubble'
import { disposeTile, rebuildBuildings, remeshTerrain, replaceRibbons, restitchTerrain, shadeTerrain, tileGroup } from './render/world'
import { ribbonsOf } from './render/groundmesh'
import { TileWorker } from './render/tileworker'
import { prepareTile, tileParts, type PreparedTile, type TileInput, type TileParts } from './render/tileprep'
import { buildingBuffers } from './render/buildingmesh'
import { reliefPixels, terrainBuffers, terrainLevel, TERRAIN_LOD_ERRORS, type Pixels } from './render/terrainmesh'
import { buildWater, disposeWater } from './render/water'
import { shellOf } from './sim/carve'
import { CHUNK, Construct, cellAt, chunkKey, chunkOf, cubeCentre, posKey, type Cube, type Pos } from './sim/construct'
import { CELL } from './sim/carve'
import type { SimMotor } from './sim/motor'
import type { Surfaces } from './sim/surface'
import type { Waters } from './sim/waters'
import { CRATER_MAX_DEPTH } from './sim/consts.generated'
import { digCrater, groundOf, type Terrain, type TileGround } from './sim/terrain'
import type { Grass } from './render/grass'
import type { TileData, TileRef, WorldData } from './world'

/** Tiles are keyed by `row * 65536 + col`, as the server names the objects in them. */
export const tileKey = (col: number, row: number) => row * 65536 + col

/** Keeps a tile's building colours stable between reloads and varied across tiles. */
const tileSalt = (ref: TileRef) => ref.col * 31 + ref.row * 17

const sameSet = (a: ReadonlySet<number>, b: ReadonlySet<number>) => a.size === b.size && [...a].every((x) => b.has(x))

/** How far under what the file has a crater can take the ground, whole centimetres. */
const CRATER_CAP = Math.round(CRATER_MAX_DEPTH * 100)

/** Digs `crater` into `ground`, whose tile file has `pristine`: the server's `Ground::dig`. Whether anything moved. */
function digInto(ground: TileGround, pristine: Float32Array, crater: CraterEntry): boolean {
  return digCrater(ground, crater.x, crater.z, crater.radius, crater.depth, (at) => Math.max(Math.round(pristine[at]! * 100) - CRATER_CAP, 0))
}

/** Whether `crater` reaches `ground`'s tile (its edge samples included). */
function reaches(ground: TileGround, crater: CraterEntry): boolean {
  const size = ground.cell * (ground.side - 1) * 100
  const [x0, z0] = [Math.round(ground.x0 * 100), Math.round(ground.z0 * 100)]
  return crater.x + crater.radius > x0 && crater.x - crater.radius < x0 + size && crater.z + crater.radius > z0 && crater.z - crater.radius < z0 + size
}

/** The `(col, row)` of tile `key`. */
const tileOf = (key: number) => ({ col: key % 65536, row: Math.floor(key / 65536) })

/**
 * What the player's world looks like and what is solid in it: the streamed tiles, drawn and made
 * solid in both predicted simulations, and the buildings and trees the server has brought down.
 * The simulations and the terrain arrive after the world loads, so `Game` assigns them.
 */
export class WorldView {
  world: WorldData | null = null
  terrain: Terrain | null = null
  surfaces: Surfaces | null = null
  grass: Grass | null = null
  waters: Waters | null = null
  /** What is solid to the local stickman and the vehicle it drives: the server's physics. */
  motor: SimMotor | null = null

  readonly damage = new BuildingDamage()
  /** The cubes players placed with bricks, as the server has told of them, and their drawing. */
  readonly construct = new Construct()
  readonly cubes = new ConstructView()
  /** Water of every tile, drawn after everything solid. */
  readonly waterRoot = new Group()

  private readonly groups = new Map<number, Group>()
  /** The tiles drawn now, with the salt their colours were picked with. */
  private readonly loaded = new Map<number, { tile: TileData; salt: number }>()
  /** Trees that fell, by tile then index, with the way they fell: replayed onto tiles that stream in later. */
  private readonly fellTrees = new Map<number, Map<number, number>>()
  private readonly fallingTrees = new FallingTrees()
  private readonly goneBuildings = new Map<number, Map<number, number>>()
  private readonly collapses = new Collapses()
  /** Carved buildings, by tile, whose colliders are to be reshaped at the next `update`: a barrage does it once a frame, not once a hit. */
  private readonly reshape = new Map<number, Set<number>>()
  /** Chunks of cubes that changed since the last `update`, to redraw and to give to the physics. */
  private readonly rebuilt = new Set<string>()
  /** Chunks of cubes the physics has not been given yet: all of them until the simulation exists. */
  private readonly unsolid = new Set<string>()
  /** Prepares tiles off the main thread; null where there are no workers, undefined until the world is known. */
  private builder: TileWorker | null | undefined = undefined
  /** Tiles the worker has prepared that are not attached yet, by tile. */
  private readonly prepared = new Map<number, PreparedTile>()
  /** The level of detail each attached tile's terrain is drawn at, for tiles the worker prepared. */
  private readonly levels = new Map<number, number>()
  /** Tiles whose terrain the worker is meshing at another level now. */
  private readonly remeshing = new Set<number>()
  /** Every crater the server has told of, in the order they were dug: replayed onto tiles that stream in later. */
  private readonly craters: CraterEntry[] = []
  /** How many craters each tile's ground had when the worker prepared it. */
  private readonly preparedWith = new Map<number, number>()
  /** Tiles whose dented ground the worker is meshing now, and those dented again meanwhile. */
  private readonly regrounding = new Set<number>()
  private readonly regroundAgain = new Set<number>()
  /** Where the viewer is, `(x, z)`: what picks the tiles' levels of detail. */
  private eye: [number, number] | null = null
  /** Where the levels were last checked from. */
  private detailAt: [number, number] | null = null

  constructor(private readonly scene: Scene) {}

  /** Told when a felled tree cracks and lands, to be heard. */
  set onTreeNoise(listener: ((noise: TreeNoise) => void) | null) {
    this.fallingTrees.onNoise = listener
  }

  /** Advances the falling trees, collapsing buildings and shards. */
  update(dt: number, effects: Effects, camera: THREE.Vector3, fx: TreeFx) {
    this.fallingTrees.update(dt, fx)
    this.collapses.update(dt, effects, fx.smoke)
    this.damage.update(dt, effects, camera, fx.smoke)
    for (const [key, indices] of this.reshape) this.dropBuildingColliders(key, indices)
    this.reshape.clear()
    this.updateCubes(dt, effects)
    this.focus(camera.x, camera.z)
  }

  /** The viewer is at `(x, z)`: tiles that came nearer or went further are meshed at their new level of detail. */
  focus(x: number, z: number) {
    this.eye = [x, z]
    const builder = this.builder
    if (!builder || (this.detailAt && Math.hypot(x - this.detailAt[0], z - this.detailAt[1]) < 8)) return
    this.detailAt = [x, z]
    for (const [key, group] of this.groups) {
      const current = this.levels.get(key)
      if (current === undefined || this.remeshing.has(key)) continue
      const level = terrainLevel(this.tileDistance(key), current)
      if (level === current) continue
      this.remeshing.add(key)
      const { col, row } = tileOf(key)
      builder
        .remesh(col, row, level)
        .then(
          (buffers) => {
            if (this.groups.get(key) !== group) return
            remeshTerrain(group, buffers)
            this.levels.set(key, level)
          },
          () => {},
        )
        .finally(() => {
          this.remeshing.delete(key)
          // The viewer may have moved on meanwhile: look again.
          this.detailAt = null
        })
    }
  }

  /** Horizontal distance from the viewer to tile `key`'s nearest edge, metres. */
  private tileDistance(key: number) {
    if (!this.world || !this.eye) return 0
    const { col, row } = tileOf(key)
    const size = this.world.tileSize
    const x0 = -this.world.halfExtent + col * size
    const z0 = -this.world.halfExtent + row * size
    const [x, z] = this.eye
    return Math.hypot(Math.max(x0 - x, 0, x - (x0 + size)), Math.max(z0 - z, 0, z - (z0 + size)))
  }

  /**
   * The server says cubes were placed, dug out or blasted, and fell. `live` is false for the catch-up on
   * joining, which only places. What changed is redrawn and made solid at the next `update`, once a
   * frame however many messages came.
   */
  built(message: Pick<Built, 'live' | 'placed' | 'removed' | 'fell'>) {
    for (const { x, y, z, color, material } of message.placed) {
      const pos: Pos = [x, y, z]
      const cube = { colour: color, material }
      const predicted = this.predicted.get(posKey(pos))
      this.predicted.delete(posKey(pos))
      // A cube predicted there is now the server's; one of another colour (someone else's) takes its place.
      const there = this.construct.get(pos)
      if (there && predicted?.cube && there.colour === cube.colour && there.material === cube.material) continue
      if (there) this.construct.remove(pos)
      this.construct.place(pos, cube)
      this.touch(pos)
    }
    for (const { x, y, z } of message.removed) {
      const pos: Pos = [x, y, z]
      // A dig predicted there is now the server's: the cube is already gone.
      this.predicted.delete(posKey(pos))
      if (message.live && this.construct.has(pos)) this.dusted.push(cubeCentre(pos))
      if (this.construct.remove(pos) !== null) this.touch(pos)
    }
    const fell: { pos: Pos; cube: Cube }[] = []
    for (const { x, y, z } of message.fell) {
      const pos: Pos = [x, y, z]
      this.predicted.delete(posKey(pos))
      const cube = this.construct.get(pos)
      if (cube === undefined) continue
      fell.push({ pos, cube })
      this.construct.remove(pos)
      this.touch(pos)
    }
    if (message.live) this.cubes.drop(fell, (x, z) => this.terrain?.heightAt(x, z) ?? 0)
  }

  /**
   * What the local player's bricks did on screen before the server said so (`predict`), by `posKey`:
   * a cube put there (`cube`), or the cube dug out of there (`dug`), and when to give up waiting.
   */
  private readonly predicted = new Map<string, { pos: Pos; cube: Cube | null; dug: Cube | null; until: number }>()

  /**
   * The local player's bricks put `cube` at `pos` (or with `cube` null dug the cube there out) on this
   * input tick. It is drawn and solid at once. If the server's `built` has not said the same by `until`
   * (a `performance.now()` time), it is taken back.
   */
  predict(pos: Pos, cube: Cube | null, until: number) {
    const key = posKey(pos)
    if (cube) {
      if (this.construct.place(pos, cube) === null) return
      this.predicted.set(key, { pos, cube, dug: null, until })
    } else {
      const dug = this.construct.get(pos)
      if (!dug) return
      this.construct.remove(pos)
      this.dusted.push(cubeCentre(pos))
      this.predicted.set(key, { pos, cube: null, dug, until })
    }
    this.touch(pos)
  }

  /** Takes back what was predicted and never confirmed by `now`. */
  private expire(now: number) {
    for (const [key, guess] of this.predicted) {
      if (guess.until > now) continue
      this.predicted.delete(key)
      if (guess.cube && this.construct.remove(guess.pos) !== null) this.touch(guess.pos)
      if (guess.dug && this.construct.place(guess.pos, guess.dug) !== null) this.touch(guess.pos)
    }
  }

  /** Cubes dug out or blasted since the last `update`, to be seen going in a little dust. */
  private readonly dusted: [number, number, number][] = []

  /**
   * Notes the chunk of the cube at `pos` as changed: to redraw and to make solid anew. A cube on the
   * chunk's edge changes what its neighbour draws too, so that neighbour is redrawn as well.
   */
  private touch(pos: Pos) {
    const chunk = chunkOf(pos)
    const key = chunkKey(chunk)
    this.rebuilt.add(key)
    this.unsolid.add(key)
    for (let a = 0; a < 3; a++) {
      const local = pos[a]! - chunk[a]! * CHUNK
      const side = local === 0 ? -1 : local === CHUNK - 1 ? 1 : 0
      if (side === 0) continue
      const next = [...chunk] as [number, number, number]
      next[a]! += side
      this.rebuilt.add(chunkKey(next))
    }
  }

  /**
   * A round of this player's of `damage` ended at `(x, y, z)`. If it struck a placed cube, the cube is
   * drawn darker by what it has taken, as a building's wall is, until the server breaks it out.
   */
  private chippedCube(x: number, y: number, z: number, damage: number): boolean {
    // The round stops on the cube's face: its middle is under half a cube further on, the nearest cube round the point.
    let best: Pos | null = null
    let distance = Infinity
    const at = cellAt(x, y, z)
    for (let dy = -1; dy <= 1; dy++) {
      for (let dz = -1; dz <= 1; dz++) {
        for (let dx = -1; dx <= 1; dx++) {
          const pos: Pos = [at[0] + dx, at[1] + dy, at[2] + dz]
          if (!this.construct.has(pos)) continue
          const c = cubeCentre(pos)
          const d = (c[0] - x) ** 2 + (c[1] - y) ** 2 + (c[2] - z) ** 2
          if (d < distance) [best, distance] = [pos, d]
        }
      }
    }
    if (!best || distance > (CELL / 100) ** 2 || !this.construct.dent(best, damage)) return false
    this.rebuilt.add(chunkKey(chunkOf(best)))
    return true
  }

  /** Redraws the chunks of cubes that changed, gives them to the physics once it exists, and moves what is falling. */
  private updateCubes(dt: number, effects: Effects) {
    if (this.predicted.size > 0) this.expire(performance.now())
    for (const key of this.rebuilt) this.cubes.redraw(this.construct, key)
    this.rebuilt.clear()
    if (this.motor && this.unsolid.size > 0) {
      const chunks = [...this.unsolid].flatMap((key) => {
        const data = this.construct.chunk(key)
        return data ? [data] : []
      })
      this.motor.setConstruct(chunks)
      for (const key of this.unsolid) this.construct.forget(key)
      this.unsolid.clear()
    }
    for (const [x, y, z] of this.dusted.splice(0)) effects.dust({ x, y, z }, 1)
    this.cubes.update(dt, effects)
  }

  /**
   * Prepares a streamed-in tile in the worker (`prepareTile`), before `attachTile`, so the frame
   * that attaches it only wraps its arrays in meshes and uploads them.
   */
  async prepareTile(ref: TileRef, tile: TileData) {
    const builder = this.tileWorker()
    if (!builder) return
    const key = tileKey(ref.col, ref.row)
    try {
      this.preparedWith.set(key, this.craters.length)
      this.prepared.set(key, await builder.prepare(ref.col, ref.row, this.tileInput(ref, tile, tileParts(tile), terrainLevel(this.tileDistance(key)))))
    } catch (error) {
      // `attachTile` prepares it on the main thread instead.
      console.warn('tile worker failed', ref, error)
    }
  }

  /**
   * What `prepareTile` makes tile `ref` from: `parts` of `tile` (all of it on the main thread, so its
   * buildings' cells are the ones the client keeps), its terrain at level `level`, and without the
   * buildings that are down or in shards now.
   */
  private tileInput(ref: TileRef, tile: TileData, parts: TileParts, level: number): TileInput {
    const world = this.world
    const ground = this.groundFor(ref, tile)
    const place = world ? { x0: -world.halfExtent + ref.col * world.tileSize, z0: -world.halfExtent + ref.row * world.tileSize, size: world.tileSize } : undefined
    return { tile: parts, ground, salt: tileSalt(ref), level, undrawn: [...this.undrawnBuildings(tileKey(ref.col, ref.row))], ...(place ? { place } : {}) }
  }

  /**
   * The ground of `tile` as it is now: its file's heights, in a copy of their own (the file's stay as
   * they are, which craters are measured from), with every crater so far dug into it.
   */
  private groundFor(ref: TileRef, tile: TileData): TileGround | null {
    const ground = this.world ? groundOf(this.world, ref, tile) : null
    if (!ground) return null
    const own = { ...ground, heights: ground.heights.slice() }
    for (const crater of this.craters) if (reaches(own, crater)) digInto(own, tile.ground, crater)
    return own
  }

  /**
   * The server says shells dug these craters, in this order. Every tile they reach is dented in place
   * (the arrays the terrain, the surfaces and the camera read), given to the physics, and drawn again.
   * Tiles that are not here yet are dented when they arrive. `live` is false for the catch-up on joining.
   */
  cratered(craters: readonly CraterEntry[], live: boolean) {
    const dented = new Set<number>()
    for (const crater of craters) {
      this.craters.push(crater)
      for (const [key, loaded] of this.loaded) {
        const { col, row } = tileOf(key)
        const ground = this.terrain?.get(col, row)
        if (!ground || !reaches(ground, crater) || !digInto(ground, loaded.tile.ground, crater)) continue
        dented.add(key)
      }
      this.grass?.dent(crater.x / 100, crater.z / 100, crater.radius / 100)
      if (live) this.dusted.push([crater.x / 100, (this.terrain?.heightAt(crater.x / 100, crater.z / 100) ?? 0) + 0.5, crater.z / 100])
    }
    for (const key of dented) {
      const { col, row } = tileOf(key)
      const ground = this.terrain?.get(col, row)
      if (!ground) continue
      this.motor?.setTileGround({ col, row }, Int32Array.from(ground.heights, (h) => Math.round(h * 100)))
      this.reground(key)
    }
  }

  /** Draws tile `key`'s terrain, relief and roads again from its ground as it is now. */
  private reground(key: number) {
    const group = this.groups.get(key)
    const loaded = this.loaded.get(key)
    const { col, row } = tileOf(key)
    const ground = this.terrain?.get(col, row)
    if (!group || !loaded || !ground) return
    const level = this.levels.get(key)
    const builder = this.builder
    if (builder && level !== undefined) {
      if (this.regrounding.has(key)) {
        this.regroundAgain.add(key)
        return
      }
      this.regrounding.add(key)
      builder
        .reground(col, row, ground.heights.slice(), level)
        .then(
          (done) => {
            if (this.groups.get(key) !== group) return
            remeshTerrain(group, done.buffers)
            shadeTerrain(group, done.relief)
            replaceRibbons(group, done.ribbons)
          },
          () => {},
        )
        .finally(() => {
          this.regrounding.delete(key)
          if (this.regroundAgain.delete(key)) this.reground(key)
        })
      return
    }
    // No worker, or the main thread prepared this tile: the same, here.
    remeshTerrain(group, terrainBuffers(ground, TERRAIN_LOD_ERRORS[Math.min(Math.max(level ?? 0, 0), TERRAIN_LOD_ERRORS.length - 1)]!))
    shadeTerrain(group, reliefPixels(ground, this.terrain ?? undefined, loaded.tile.detail))
    replaceRibbons(group, ribbonsOf(loaded.tile, ground, this.terrain ?? undefined))
    if (!this.terrain) return
    for (const [dc, dr] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const) {
      const next = this.groups.get(tileKey(col + dc, row + dr))
      if (next) restitchTerrain(next, this.terrain)
    }
  }

  /** A prepared tile that will not be attached after all. */
  discardTile(ref: TileRef) {
    this.preparedWith.delete(tileKey(ref.col, ref.row))
    this.prepared.delete(tileKey(ref.col, ref.row))
    this.builder?.drop(ref.col, ref.row)
  }

  /** Draws a streamed-in tile and makes it solid for both predicted simulations. Returns how to undo it. */
  attachTile(ref: TileRef, tile: TileData) {
    const ground = this.groundFor(ref, tile)
    const salt = tileSalt(ref)
    const key = tileKey(ref.col, ref.row)
    // What the worker prepared, or without one (or if it failed) the same, prepared here and now.
    const fromWorker = this.prepared.get(key)
    this.prepared.delete(key)
    const prepared = fromWorker ?? prepareTile(this.tileInput(ref, tile, tile, 0), this.terrain ?? undefined)
    // Buildings that fell, or are hurt and drawn by `damage` as shards, are not drawn with the tile.
    // More of them may have come down while the worker prepared it.
    const undrawn = this.undrawnBuildings(key)
    // [ghost] temporary: which buildings this tile is attached without, and whether the worker's copy had them
    console.info('[ghost] attach', { col: ref.col, row: ref.row, fromWorker: !!fromWorker, undrawn: [...undrawn], workerUndrawn: prepared.undrawn, carved: [...this.damage.carved(key).keys()], fractured: this.damage.fractured(key) })
    if (!sameSet(new Set(prepared.undrawn), undrawn)) {
      prepared.buildings = buildingBuffers(tile, salt, undrawn)
      prepared.undrawn = [...undrawn]
      console.info('[ghost] attach rebuilt buildings', { col: ref.col, row: ref.row })
    }
    // Buildings that fell before this tile streamed in (or while it was away) settle as rubble.
    const down = new Set(this.goneBuildings.get(key)?.keys())
    const group = tileGroup(tile, ground, prepared)
    this.scene.add(group)
    const water = ground && prepared.draped ? buildWater(tile, ground, this.terrain ?? undefined, prepared.draped.water) : null
    if (water) this.waterRoot.add(water)
    this.terrain?.set(ref, ground)
    this.surfaces?.add(ref, tile, ground, prepared.surface)
    this.grass?.addTile(ref, tile, ground, prepared.grass)
    if (ground) for (const crater of this.craters) if (reaches(ground, crater)) this.grass?.dent(crater.x / 100, crater.z / 100, crater.radius / 100)
    this.waters?.set(ref, ground, tile.water)
    this.groups.set(key, group)
    if (fromWorker?.terrain) this.levels.set(key, prepared.level)
    // Tiles already in place were shaded without this one beside them: close the seams. The worker
    // does that itself for the tiles it prepared (`onShaded`).
    if (this.terrain && ground && !fromWorker) {
      for (const [dc, dr] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const) {
        const next = this.groups.get(tileKey(ref.col + dc, ref.row + dr))
        if (next) restitchTerrain(next, this.terrain)
      }
    }
    if (tile.bytes) this.motor?.addTile(ref, tile.bytes)
    this.loaded.set(key, { tile, salt })
    // The file's ground is flat of craters: the physics is given the dented one, and so is the worker's copy if craters came after it prepared.
    if (ground && this.craters.some((crater) => reaches(ground, crater))) {
      this.motor?.setTileGround(ref, Int32Array.from(ground.heights, (h) => Math.round(h * 100)))
      if (fromWorker && this.preparedWith.get(key) !== this.craters.length) queueMicrotask(() => this.reground(key))
    }
    this.preparedWith.delete(key)
    this.damage.attach(key, tile, salt)
    if (down.size > 0 || this.damage.carved(key).size > 0) {
      this.dropBuildingColliders(key)
      for (const index of down) Collapses.settled(group, tile, index)
    }
    // Trees that fell before this tile streamed in (or while it was away) are already down.
    for (const [index, yaw] of this.fellTrees.get(key) ?? []) this.fellTree(key, index, yaw, false)
    return () => {
      this.loaded.delete(key)
      this.damage.detach(key)
      disposeTile(group)
      if (water) disposeWater(water)
      this.groups.delete(key)
      this.levels.delete(key)
      this.terrain?.delete(ref)
      this.surfaces?.delete(ref)
      this.grass?.removeTile(ref)
      this.waters?.delete(ref)
      this.motor?.removeTile(ref)
      this.builder?.drop(ref.col, ref.row)
    }
  }

  private tileWorker(): TileWorker | null {
    if (this.builder === undefined && this.world) {
      this.builder = TileWorker.create(this.world.halfExtent, this.world.tileSize)
      if (this.builder) this.builder.onShaded = (col, row, relief) => this.reshade(tileKey(col, row), relief)
    }
    return this.builder ?? null
  }

  /** A new relief map from the worker for tile `key`, attached or still waiting to be. */
  private reshade(key: number, relief: Pixels) {
    const group = this.groups.get(key)
    if (group) shadeTerrain(group, relief)
    const waiting = this.prepared.get(key)?.terrain
    if (waiting) waiting.relief = relief
  }

  /** The server says these buildings and trees are gone. `animate` is false for the catch-up on joining. */
  destroyed(objects: readonly DestroyedObject[], animate: boolean) {
    for (const object of objects) {
      const key = tileKey(object.col, object.row)
      if (object.kind === OBJ_TREE) {
        const fell = this.fellTrees.get(key) ?? new Map<number, number>()
        this.fellTrees.set(key, fell)
        if (fell.has(object.index)) continue
        fell.set(object.index, object.yaw)
        this.fellTree(key, object.index, object.yaw, animate, object.force)
      } else if (object.kind === OBJ_BUILDING) {
        const gone = this.goneBuildings.get(key) ?? new Map<number, number>()
        this.goneBuildings.set(key, gone)
        if (gone.has(object.index)) continue
        gone.set(object.index, object.yaw)
        this.damage.fell(key, object.index)
        this.fellBuilding(key, object.index, object.yaw, animate)
      }
    }
  }

  /**
   * The server says blasts took these cells out of standing buildings. The building is redrawn
   * without them, they fly or fall, and the walls stop being solid there in both simulations.
   * `live` is false for the catch-up on joining.
   */
  carved(buildings: readonly CarvedBuilding[], live: boolean) {
    for (const b of buildings) {
      const key = tileKey(b.col, b.row)
      if (this.goneBuildings.get(key)?.has(b.index)) continue
      const first = this.damage.carve(key, b.index, b.blasted, b.fell, live ? b.point : null, live)
      // [ghost] temporary: each carve as it arrives, and whether its tile is drawn right now
      console.info('[ghost] carved', { col: b.col, row: b.row, index: b.index, live, blasted: b.blasted.length, fell: b.fell.length, first, tileLoaded: this.loaded.has(key), groupDrawn: this.groups.has(key) })
      const dirty = this.reshape.get(key) ?? new Set<number>()
      dirty.add(b.index)
      this.reshape.set(key, dirty)
      if (!first) continue
      const group = this.groups.get(key)
      const loaded = this.loaded.get(key)
      if (group && loaded) rebuildBuildings(group, loaded.tile, loaded.salt, this.undrawnBuildings(key))
    }
  }

  /**
   * A round of `damage` from this player ended at `(x, y, z)`. If it struck a placed cube or a standing
   * building, that cube or piece of wall is drawn darker or pushed in by what it has taken, until the server breaks it out.
   * Only the player's own rounds do this, and only on their screen.
   */
  chipped(x: number, y: number, z: number, damage: number) {
    if (this.chippedCube(x, y, z, damage)) return
    const world = this.world
    if (!world) return
    const key = tileKey(Math.floor((x + world.halfExtent) / world.tileSize), Math.floor((z + world.halfExtent) / world.tileSize))
    const loaded = this.loaded.get(key)
    if (!loaded) return
    const { tile } = loaded
    const carved = this.damage.carved(key)
    let best: { index: number; cell: number; distance: number } | null = null
    tile.buildings.forEach((building, index) => {
      if (this.goneBuildings.get(key)?.has(index)) return
      // Only a building the round ended within a metre or two of is worth cutting into cells.
      const xs = building.footprint.map((p) => p[0])
      const zs = building.footprint.map((p) => p[1])
      const margin = 1.6
      if (x < Math.min(...xs) - margin || x > Math.max(...xs) + margin || z < Math.min(...zs) - margin || z > Math.max(...zs) + margin) return
      if (y < building.base - margin || y > building.base + building.height + margin) return
      const shell = shellOf(tile, index)
      const cell = shell?.nearestCell(x, y, z, carved.get(index)) ?? null
      if (!shell || cell === null) return
      const c = shell.centre(cell)
      const distance = (c[0] / 100 - x) ** 2 + (c[1] / 100 - y) ** 2 + (c[2] / 100 - z) ** 2
      if (!best || distance < best.distance) best = { index, cell, distance }
    })
    if (!best) return
    const { index, cell } = best as { index: number; cell: number }
    if (!this.damage.dent(key, index, cell, damage)) return
    const group = this.groups.get(key)
    if (group) rebuildBuildings(group, tile, loaded.salt, this.undrawnBuildings(key))
  }

  /** The server says these standing buildings are hurt. */
  damaged(buildings: readonly DamagedBuilding[]) {
    for (const b of buildings) {
      const key = tileKey(b.col, b.row)
      if (this.goneBuildings.get(key)?.has(b.index)) continue
      this.damage.hurtBuilding(key, b.index, b.stage)
    }
  }

  dispose() {
    this.damage.dispose()
    this.cubes.dispose()
    this.builder?.dispose()
  }

  /**
   * Fells tree number `index` of tile `key` if that tile is loaded: it leaves the tile's drawing
   * and falls, and its trunk stops being solid in both simulations.
   */
  private fellTree(key: number, index: number, yaw: number, animate: boolean, force = 0) {
    const group = this.groups.get(key)
    const loaded = this.loaded.get(key)
    const tree = loaded?.tile.trees[index]
    if (!group || !loaded || !tree) return
    this.fallingTrees.fell(group, tree, index, yaw, animate, force)
    const { col, row } = tileOf(key)
    this.motor?.fellTree(col, row, index)
  }

  /** The buildings of tile `key` its own drawing leaves out: those that fell, and those drawn as shards instead. */
  private undrawnBuildings(key: number): Set<number> {
    const out = new Set(this.goneBuildings.get(key)?.keys())
    for (const index of this.damage.fractured(key)) out.add(index)
    return out
  }

  /** Rebuilds tile `key`'s building colliders in both simulations without the buildings that fell, or just the carved buildings `only`. */
  private dropBuildingColliders(key: number, only?: ReadonlySet<number>) {
    const loaded = this.loaded.get(key)
    if (!loaded || loaded.tile.collision.triangles.length === 0) return
    // Given `only`, just those carved buildings are reshaped; the rest are as they were.
    const down = only ? new Set<number>() : new Set(this.goneBuildings.get(key)?.keys())
    const carved = this.damage.carved(key)
    if (only) for (const index of carved.keys()) if (!only.has(index)) carved.delete(index)
    const { col, row } = tileOf(key)
    this.motor?.setBuildings(col, row, down, carved)
  }

  /**
   * Brings building `index` of tile `key` down, if that tile is loaded: it is redrawn without
   * it, it stops being solid in both simulations, and it collapses in front of the player.
   */
  private fellBuilding(key: number, index: number, yaw: number, animate: boolean) {
    const group = this.groups.get(key)
    const loaded = this.loaded.get(key)
    if (!group || !loaded || !loaded.tile.buildings[index]) return
    rebuildBuildings(group, loaded.tile, loaded.salt, this.undrawnBuildings(key))
    this.dropBuildingColliders(key)
    if (animate) {
      this.collapses.start(group, loaded.tile, index, loaded.salt, yaw, (x, z) => this.terrain?.heightAt(x, z) ?? 0, key * 1_000_003 + index)
    } else {
      Collapses.settled(group, loaded.tile, index)
    }
  }
}
