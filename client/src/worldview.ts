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
import { addTileCollider, trunkColliderIndex } from './sim/physics'
import type { Surfaces } from './sim/surface'
import type { Waters } from './sim/waters'
import { groundOf, type Terrain } from './sim/terrain'
import type { Grass } from './render/grass'
import type { VehicleDriver } from './sim/vehicle'
import type { TileData, TileRef, WorldData } from './world'

/** A tile's colliders in the vehicle simulation, with the ones taken out one by one (felled trunks) left `undefined`. */
type TileSolids = (ReturnType<typeof addTileCollider>[number] | undefined)[]

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
  motor: SimMotor | null = null
  driver: VehicleDriver | null = null

  readonly damage = new BuildingDamage()
  /** Water of every tile, drawn after everything solid. */
  readonly waterRoot = new Group()

  private readonly groups = new Map<number, Group>()
  private readonly solids = new Map<number, { tile: TileData; salt: number; drive: TileSolids }>()
  /** Trees that fell, by tile then index, with the way they fell: replayed onto tiles that stream in later. */
  private readonly fellTrees = new Map<number, Map<number, number>>()
  private readonly fallingTrees = new FallingTrees()
  private readonly goneBuildings = new Map<number, Map<number, number>>()
  private readonly collapses = new Collapses()

  constructor(private readonly scene: Scene) {}

  /** Told when a felled tree cracks and lands, to be heard. */
  set onTreeNoise(listener: ((noise: TreeNoise) => void) | null) {
    this.fallingTrees.onNoise = listener
  }

  /** Advances the falling trees, collapsing buildings and shards. */
  update(dt: number, effects: Effects, camera: THREE.Vector3, fx: TreeFx) {
    this.fallingTrees.update(dt, fx)
    this.collapses.update(dt, effects)
    this.damage.update(dt, effects, camera)
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
    const drive: TileSolids = this.driver?.addTile(tile, ground) ?? []
    this.solids.set(key, { tile, salt, drive })
    this.damage.attach(key, tile, salt)
    if (down.size > 0 || this.damage.carved(key).size > 0) {
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
      this.terrain?.delete(ref)
      this.surfaces?.delete(ref)
      this.grass?.removeTile(ref)
      this.waters?.delete(ref)
      this.motor?.removeTile(ref)
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
    const touched = new Set<number>()
    for (const b of buildings) {
      const key = tileKey(b.col, b.row)
      if (this.goneBuildings.get(key)?.has(b.index)) continue
      const first = this.damage.carve(key, b.index, b.blasted, b.fell, live ? b.point : null, live)
      touched.add(key)
      if (!first) continue
      const group = this.groups.get(key)
      const solids = this.solids.get(key)
      if (group && solids) rebuildBuildings(group, solids.tile, solids.salt, this.undrawnBuildings(key))
    }
    for (const key of touched) this.dropBuildingColliders(key)
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
    const solids = this.solids.get(key)
    if (!solids) return
    const { tile } = solids
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
    if (group) rebuildBuildings(group, tile, solids.salt, this.undrawnBuildings(key))
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
    const solids = this.solids.get(key)
    const tree = solids?.tile.trees[index]
    if (!group || !solids || !tree) return
    this.fallingTrees.fell(group, tree, index, yaw, animate, force)
    const { col, row } = tileOf(key)
    this.motor?.fellTree(col, row, index)
    const at = trunkColliderIndex(solids.tile, index)
    if (at < 0) return
    this.driver?.removeTile([solids.drive[at]])
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
    const carved = this.damage.carved(key)
    const { col, row } = tileOf(key)
    this.motor?.setBuildings(col, row, down, carved)
    // The building mesh is `addTile`'s first collider, when the tile has one.
    if (this.driver) solids.drive[0] = this.driver.rebuildBuildings(solids.drive[0], solids.tile, down, carved)
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
