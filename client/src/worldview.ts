import { Group, type Scene } from '@ragnarok/engine'
import type * as THREE from 'three'
import { OBJ_BUILDING, OBJ_TREE, type CarvedBuilding, type DamagedBuilding, type DestroyedObject } from './net/codec'
import type { Effects } from './render/effects'
import { FallingTrees, type TreeFx, type TreeNoise } from './render/falling'
import { BuildingDamage } from './render/damage'
import { Collapses } from './render/rubble'
import { buildTile, disposeTile, rebuildBuildings, restitchTerrain } from './render/world'
import { buildWater, disposeWater } from './render/water'
import { shellOf } from './sim/carve'
import type { SimMotor } from './sim/motor'
import type { Surfaces } from './sim/surface'
import type { Waters } from './sim/waters'
import { groundOf, type Terrain } from './sim/terrain'
import type { Grass } from './render/grass'
import type { TileData, TileRef, WorldData } from './world'

/** Tiles are keyed by `row * 65536 + col`, as the server names the objects in them. */
export const tileKey = (col: number, row: number) => row * 65536 + col

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

  /** How bright the day is, 1 at noon and 0 at midnight: smoke is not lit, so what draws it is dimmed by this. */
  daylight = 1

  constructor(private readonly scene: Scene) {}

  /** Told when a felled tree cracks and lands, to be heard. */
  set onTreeNoise(listener: ((noise: TreeNoise) => void) | null) {
    this.fallingTrees.onNoise = listener
  }

  /** Advances the falling trees, collapsing buildings and shards. */
  update(dt: number, effects: Effects, camera: THREE.Vector3, fx: TreeFx) {
    this.fallingTrees.update(dt, fx)
    this.collapses.update(dt, effects)
    this.damage.update(dt, effects, camera, fx.smoke, this.daylight)
    for (const [key, indices] of this.reshape) this.dropBuildingColliders(key, indices)
    this.reshape.clear()
  }

  /** Draws a streamed-in tile and makes it solid for both predicted simulations. Returns how to undo it. */
  attachTile(ref: TileRef, tile: TileData) {
    const ground = this.world ? groundOf(this.world, ref, tile) : null
    const salt = ref.col * 31 + ref.row * 17
    const key = tileKey(ref.col, ref.row)
    // Buildings that fell before this tile streamed in (or while it was away) are not drawn.
    const down = new Set(this.goneBuildings.get(key)?.keys())
    // Buildings that are hurt are drawn by `damage`, as shards, not with the tile.
    const group = buildTile(tile, salt, ground, this.terrain ?? undefined, this.undrawnBuildings(key))
    this.scene.add(group)
    const water = ground ? buildWater(tile, ground, this.terrain ?? undefined) : null
    if (water) this.waterRoot.add(water)
    this.terrain?.set(ref, ground)
    this.surfaces?.add(ref, tile, ground)
    this.grass?.addTile(ref, tile, ground)
    this.waters?.set(ref, ground, tile.water)
    this.groups.set(key, group)
    // Tiles already in place were shaded without this one beside them: close the seams.
    if (this.terrain && ground) {
      for (const [dc, dr] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const) {
        const next = this.groups.get(tileKey(ref.col + dc, ref.row + dr))
        if (next) restitchTerrain(next, this.terrain)
      }
    }
    if (tile.bytes) this.motor?.addTile(ref, tile.bytes)
    this.loaded.set(key, { tile, salt })
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
      this.terrain?.delete(ref)
      this.surfaces?.delete(ref)
      this.grass?.removeTile(ref)
      this.waters?.delete(ref)
      this.motor?.removeTile(ref)
    }
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
   * A round of `damage` from this player ended at `(x, y, z)`. If it struck a standing building,
   * the piece of wall there is drawn pushed in by what it has taken, until the server breaks it out.
   * Only the player's own rounds do this, and only on their screen.
   */
  chipped(x: number, y: number, z: number, damage: number) {
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
