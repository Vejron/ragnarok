import { Group, type Scene } from '@ragnarok/engine'
import type * as THREE from 'three'
import { OBJ_BUILDING, OBJ_TREE, type DamagedBuilding, type DestroyedObject } from './net/codec'
import type { Effects } from './render/effects'
import { FallingTrees } from './render/falling'
import { BuildingDamage } from './render/damage'
import { Collapses } from './render/rubble'
import { buildTile, disposeTile, rebuildBuildings, restitchTerrain } from './render/world'
import { setTreeDetail } from './render/trees'
import { buildWater, disposeWater } from './render/water'
import { RapierMotor, trunkColliderIndex } from './sim/physics'
import type { Surfaces } from './sim/surface'
import { groundOf, type Terrain } from './sim/terrain'
import type { VehicleDriver } from './sim/vehicle'
import type { TileData, TileRef, WorldData } from './world'

/** A tile's colliders in one simulation, with the ones taken out one by one (felled trunks) left `undefined`. */
type TileSolids = (ReturnType<RapierMotor['addTile']>[number] | undefined)[]

/** Tiles are keyed by `row * 65536 + col`, as the server names the objects in them. */
export const tileKey = (col: number, row: number) => row * 65536 + col

/**
 * What the player's world looks like and what is solid in it: the streamed tiles, drawn and made
 * solid in both predicted simulations, and the buildings and trees the server has brought down.
 * The simulations and the terrain arrive after the world loads, so `Game` assigns them.
 */
export class WorldView {
  world: WorldData | null = null
  terrain: Terrain | null = null
  surfaces: Surfaces | null = null
  motor: RapierMotor | null = null
  driver: VehicleDriver | null = null

  readonly damage = new BuildingDamage()
  /** Water of every tile, drawn after everything solid. */
  readonly waterRoot = new Group()

  private readonly groups = new Map<number, Group>()
  /** Each tile's footprint `[x0, z0, x1, z1]`, for the distance its trees are drawn at. */
  private readonly boxes = new Map<number, number[]>()
  private readonly solids = new Map<number, { tile: TileData; salt: number; walk: TileSolids; drive: TileSolids }>()
  /** Trees that fell, by tile then index, with the way they fell: replayed onto tiles that stream in later. */
  private readonly fellTrees = new Map<number, Map<number, number>>()
  private readonly fallingTrees = new FallingTrees()
  private readonly goneBuildings = new Map<number, Map<number, number>>()
  private readonly collapses = new Collapses()

  constructor(private readonly scene: Scene) {}

  /** Advances the falling trees, collapsing buildings and shards. */
  update(dt: number, effects: Effects, camera: THREE.Vector3) {
    this.fallingTrees.update(dt)
    this.collapses.update(dt, effects)
    this.damage.update(dt, effects, camera)
    for (const [key, group] of this.groups) {
      const box = this.boxes.get(key)
      if (box) setTreeDetail(group, box, camera.x, camera.z)
    }
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
    this.groups.set(key, group)
    if (this.world) {
      const x0 = -this.world.halfExtent + ref.col * this.world.tileSize
      const z0 = -this.world.halfExtent + ref.row * this.world.tileSize
      this.boxes.set(key, [x0, z0, x0 + this.world.tileSize, z0 + this.world.tileSize])
    }
    // Tiles already in place were shaded without this one beside them: close the seams.
    if (this.terrain && ground) {
      for (const [dc, dr] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const) {
        const next = this.groups.get(tileKey(ref.col + dc, ref.row + dr))
        if (next) restitchTerrain(next, this.terrain)
      }
    }
    const walk: TileSolids = this.motor?.addTile(tile, ground) ?? []
    const drive: TileSolids = this.driver?.addTile(tile, ground) ?? []
    this.solids.set(key, { tile, salt, walk, drive })
    this.damage.attach(key, tile, salt)
    if (down.size > 0) {
      this.dropBuildingColliders(key)
      for (const index of down) Collapses.settled(group, tile, index)
    }
    // Trees that fell before this tile streamed in (or while it was away) are already down.
    for (const [index, yaw] of this.fellTrees.get(key) ?? []) this.fellTree(key, index, yaw, false)
    return () => {
      this.solids.delete(key)
      this.damage.detach(key)
      disposeTile(group)
      if (water) disposeWater(water)
      this.groups.delete(key)
      this.boxes.delete(key)
      this.terrain?.delete(ref)
      this.surfaces?.delete(ref)
      this.motor?.removeTile(walk)
      this.driver?.removeTile(drive)
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
        this.fellTree(key, object.index, object.yaw, animate)
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

  /** The server says these standing buildings are hurt. */
  damaged(buildings: readonly DamagedBuilding[]) {
    for (const b of buildings) {
      const key = tileKey(b.col, b.row)
      if (this.goneBuildings.get(key)?.has(b.index)) continue
      // The first real damage swaps the whole building in the tile for its shards.
      if (!this.damage.hurtBuilding(key, b.index, b.stage, b.mark)) continue
      const group = this.groups.get(key)
      const solids = this.solids.get(key)
      if (group && solids) rebuildBuildings(group, solids.tile, solids.salt, this.undrawnBuildings(key))
    }
  }

  dispose() {
    this.damage.dispose()
  }

  /**
   * Fells tree number `index` of tile `key` if that tile is loaded: it leaves the tile's drawing
   * and falls, and its trunk stops being solid in both simulations.
   */
  private fellTree(key: number, index: number, yaw: number, animate: boolean) {
    const group = this.groups.get(key)
    const solids = this.solids.get(key)
    const tree = solids?.tile.trees[index]
    if (!group || !solids || !tree) return
    this.fallingTrees.fell(group, tree, index, yaw, animate)
    const at = trunkColliderIndex(solids.tile, index)
    if (at < 0) return
    this.motor?.removeTile([solids.walk[at]])
    this.driver?.removeTile([solids.drive[at]])
    solids.walk[at] = undefined
    solids.drive[at] = undefined
  }

  /** The buildings of tile `key` its own drawing leaves out: those that fell, and those drawn as shards instead. */
  private undrawnBuildings(key: number): Set<number> {
    const out = new Set(this.goneBuildings.get(key)?.keys())
    for (const index of this.damage.fractured(key)) out.add(index)
    return out
  }

  /** Rebuilds tile `key`'s building colliders in both simulations without the buildings that fell. */
  private dropBuildingColliders(key: number) {
    const solids = this.solids.get(key)
    if (!solids || solids.tile.collision.triangles.length === 0) return
    const down = new Set(this.goneBuildings.get(key)?.keys())
    // The building mesh is `addTile`'s first collider, when the tile has one.
    if (this.motor) solids.walk[0] = this.motor.rebuildBuildings(solids.walk[0], solids.tile, down)
    if (this.driver) solids.drive[0] = this.driver.rebuildBuildings(solids.drive[0], solids.tile, down)
  }

  /**
   * Brings building `index` of tile `key` down, if that tile is loaded: it is redrawn without
   * it, it stops being solid in both simulations, and it collapses in front of the player.
   */
  private fellBuilding(key: number, index: number, yaw: number, animate: boolean) {
    const group = this.groups.get(key)
    const solids = this.solids.get(key)
    if (!group || !solids || !solids.tile.buildings[index]) return
    rebuildBuildings(group, solids.tile, solids.salt, this.undrawnBuildings(key))
    this.dropBuildingColliders(key)
    if (animate) {
      this.collapses.start(group, solids.tile, index, solids.salt, yaw, (x, z) => this.terrain?.heightAt(x, z) ?? 0, key * 1_000_003 + index)
    } else {
      Collapses.settled(group, solids.tile, index)
    }
  }
}
