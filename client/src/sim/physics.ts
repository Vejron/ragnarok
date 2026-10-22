// The static scene and vehicle solids of the client's second Rapier world, the one `VehicleDriver`
// predicts the driver's own vehicle in. The stickman's world is the server's own physics now
// (`motor.ts`); this goes too once vehicle prediction moves there (MIGRATION.md, Phase 1).
import RAPIER from '@dimforge/rapier3d-compat'
import { buildingTriangleRanges, treeKindDef, type TileData, type WorldData } from '@/world'
import { shellOf } from './carve'
import {
  STRV_CASEMATE_CENTRE, STRV_CASEMATE_HALF, TANK_TURRET_HALF_HEIGHT, TANK_TURRET_PIVOT, TANK_TURRET_RADIUS, PLANE_WING_CENTRE, PLANE_WING_HALF_EXTENTS,
  WALKER_FOOT_HALF, WALKER_HALF_EXTENTS, WALKER_LEG_CLEARANCE, WALKER_LEG_HALF_DEPTH, WALKER_LEG_HALF_WIDTH, WALKER_LEG_LENGTH,
} from './consts'
import { halfExtentsOf } from './kinds'
import type { TileGround } from './terrain'
import { DAMAGE_TURRET, KIND_HEN, KIND_PLANE, KIND_STRV, KIND_TANK, KIND_WALKER, TURRET_GONE, type VehicleState } from '@/net/codec'

export type { Vec3 } from './motor'

/** Top of the boundary walls, metres above the lowest ground. Mirrors `BOUNDARY_HEIGHT` on the server. */
const BOUNDARY_HEIGHT = 200

/** Depth of the slab under a terrain, metres below y = 0 (the deepest water bed). Mirrors `BED_FLOOR` on the server. */
const BED_FLOOR = 12

/**
 * The boundary walls as `[x, z, half width, half depth]`: the square's four sides for a world that
 * fills it, else a wall along every edge between a tile of the world and one that is not, merged into
 * runs. Mirrors `boundary_walls` on the server.
 */
export function boundaryWalls(data: Pick<WorldData, 'halfExtent' | 'tileSize' | 'inside'>): [number, number, number, number][] {
  const half = data.halfExtent
  const { inside } = data
  if (!inside || data.tileSize <= 0) return [[half, 0, 0.5, half], [-half, 0, 0.5, half], [0, half, half, 0.5], [0, -half, half, 0.5]]
  const grid = Math.max(1, Math.ceil((2 * half) / data.tileSize))
  const has = (col: number, row: number) => col >= 0 && row >= 0 && col < grid && row < grid && inside[row * grid + col] === true
  const at = (i: number) => -half + i * data.tileSize
  const walls: [number, number, number, number][] = []
  for (const alongX of [true, false]) {
    for (let line = 0; line <= grid; line++) {
      let start = -1
      for (let i = 0; i <= grid; i++) {
        const edge = i < grid && (alongX ? has(i, line - 1) !== has(i, line) : has(line - 1, i) !== has(line, i))
        if (edge && start < 0) {
          start = i
        } else if (!edge && start >= 0) {
          const mid = (at(start) + at(i)) / 2
          const reach = (at(i) - at(start)) / 2
          walls.push(alongX ? [mid, at(line), reach, 0.5] : [at(line), mid, 0.5, reach])
          start = -1
        }
      }
    }
  }
  return walls
}

/**
 * Mirrors `static_colliders` on the server: ground slab, boundary walls, building mesh. The
 * terrain arrives with its tiles (`addTileCollider`).
 */
export function buildStaticColliders(world: RAPIER.World, data: WorldData) {
  const half = data.halfExtent
  const top = data.groundCell > 0 ? -BED_FLOOR : 0
  world.createCollider(RAPIER.ColliderDesc.cuboid(half, 0.5, half).setTranslation(0, top - 0.5, 0))
  const wall = BOUNDARY_HEIGHT / 2
  for (const [x, z, hx, hz] of boundaryWalls(data)) {
    world.createCollider(RAPIER.ColliderDesc.cuboid(hx, wall, hz).setTranslation(x, wall, z))
  }
  if (data.collision.triangles.length > 0) {
    world.createCollider(RAPIER.ColliderDesc.trimesh(data.collision.vertices, data.collision.triangles))
  }
}

/** Rapier packs a collider's groups as memberships in the high 16 bits and what it touches in the low 16. Mirrors `TREE_GROUP` on the server. */
export const TREE_GROUPS = 0x0002ffff
/** A tank's or Strv's solids: members of everything, touching everything but the tree group. */
export const HEAVY_GROUPS = 0xfffffffd

/**
 * One tile's building mesh, tree trunks and terrain as static colliders (none for an empty tile). Tiles come
 * and go as the player moves; the server holds all of them merged, which is the same geometry, so
 * near the player the two agree. The heightfield is laid out as the server's: centred on its
 * square, scaled to its width, heights column-major with rows along Z.
 */
export function addTileCollider(world: RAPIER.World, tile: TileData, ground: TileGround | null = null): RAPIER.Collider[] {
  const colliders: RAPIER.Collider[] = []
  if (tile.collision.triangles.length > 0) {
    const buildings = rebuildBuildingCollider(world, tile, new Set())
    if (buildings) colliders.push(buildings)
  }
  // Trunks, as on the server: an upright cylinder from the foot up to where the crown starts.
  for (const tree of tile.trees) {
    if (!tree.solid) continue
    const half = tree.trunkHeight / 2
    const trunk = RAPIER.ColliderDesc.cylinder(half, tree.trunkRadius).setTranslation(tree.x, tree.base + half, tree.z)
    // A trunk that can be felled is in a group a tank's hull does not touch (see `HEAVY_GROUPS`).
    if (treeKindDef(tree.kind).destructible) trunk.setCollisionGroups(TREE_GROUPS)
    colliders.push(world.createCollider(trunk))
  }
  if (ground) {
    const { side, cell, heights } = ground
    const columnMajor = new Float32Array(side * side)
    for (let row = 0; row < side; row++) for (let column = 0; column < side; column++) columnMajor[row + column * side] = heights[row * side + column]!
    const width = (side - 1) * cell
    const desc = RAPIER.ColliderDesc.heightfield(side - 1, side - 1, columnMajor, { x: width, y: 1, z: width }).setTranslation(
      ground.x0 + width / 2,
      0,
      ground.z0 + width / 2,
    )
    colliders.push(world.createCollider(desc))
  }
  return colliders
}

/**
 * The tile's buildings as one collider: each building as its cubes merged into boxes (`Shell.boxes`,
 * without the cells in `carved` for it) and its floor, or as its own triangles when it has no
 * cubes, then whatever no building claims (bridges), and none of the buildings in `destroyed`.
 * Mirrors `building_shape` and `World::carve` on the server. With no boxes it is a trimesh;
 * otherwise a compound of a cuboid a box and a triangle a remaining triangle, since a compound cannot
 * hold a trimesh. Returns the new collider (the caller removes any old one), or null when nothing
 * is left.
 */
export function rebuildBuildingCollider(
  world: RAPIER.World,
  tile: TileData,
  destroyed: ReadonlySet<number>,
  carved: ReadonlyMap<number, ReadonlySet<number>> = new Map(),
): RAPIER.Collider | null {
  const { triangles, vertices } = tile.collision
  const count = triangles.length / 3
  // 1 for a triangle of the tile's mesh that is left out: a fallen building's, or a wall or roof drawn as cubes.
  const drop = new Uint8Array(count)
  buildingTriangleRanges(tile).forEach(([start, end], index) => {
    if (destroyed.has(index)) drop.fill(1, start, Math.min(end, count))
  })
  const boxes: Int32Array[] = []
  const none = new Set<number>()
  tile.buildings.forEach((building, index) => {
    if (destroyed.has(index)) return
    const shell = shellOf(tile, index)
    if (!shell) return
    drop.fill(1, building.first, Math.min(building.first + building.walls + building.roof, count))
    boxes.push(shell.boxes(carved.get(index) ?? none))
  })
  const kept: number[] = []
  for (let t = 0; t < count; t++) if (!drop[t]) kept.push(triangles[t * 3]!, triangles[t * 3 + 1]!, triangles[t * 3 + 2]!)
  const boxCount = boxes.reduce((n, b) => n + b.length / 6, 0)
  if (boxCount === 0) return kept.length === 0 ? null : world.createCollider(RAPIER.ColliderDesc.trimesh(vertices, Uint32Array.from(kept)))
  const shapes: RAPIER.Shape[] = []
  const positions: RAPIER.Vector[] = []
  const still = { x: 0, y: 0, z: 0, w: 1 }
  for (const list of boxes) {
    for (let i = 0; i < list.length; i += 6) {
      const [x0, y0, z0, x1, y1, z1] = [list[i]!, list[i + 1]!, list[i + 2]!, list[i + 3]!, list[i + 4]!, list[i + 5]!]
      shapes.push(new RAPIER.Cuboid((x1 - x0) / 200, (y1 - y0) / 200, (z1 - z0) / 200))
      positions.push({ x: (x0 + x1) / 200, y: (y0 + y1) / 200, z: (z0 + z1) / 200 })
    }
  }
  const corner = (v: number) => ({ x: vertices[v * 3]!, y: vertices[v * 3 + 1]!, z: vertices[v * 3 + 2]! })
  for (let k = 0; k < kept.length; k += 3) {
    shapes.push(new RAPIER.Triangle(corner(kept[k]!), corner(kept[k + 1]!), corner(kept[k + 2]!)))
    positions.push({ x: 0, y: 0, z: 0 })
  }
  return world.createCollider(RAPIER.ColliderDesc.compound(shapes, positions, shapes.map(() => still)))
}

/**
 * Where in `addTileCollider`'s result tree number `index` of `tile` has its trunk, or -1 if it
 * has none (it is not solid). The result is the building mesh if any, then one per solid tree.
 */
export function trunkColliderIndex(tile: TileData, index: number): number {
  const tree = tile.trees[index]
  if (!tree?.solid) return -1
  let at = tile.collision.triangles.length > 0 ? 1 : 0
  for (let i = 0; i < index; i++) if (tile.trees[i]!.solid) at++
  return at
}

type PoseLike = Pick<VehicleState, 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'> & { damage?: readonly number[] }

/**
 * A snapshot vehicle as static solids: its hull box, and for a tank the turret cylinder on the
 * ring. Mirrors the colliders `add_car` gives the server's body.
 */
export class VehicleSolids {
  private readonly hull: RAPIER.Collider
  /** A tank's turret, a plane's wings or a walker's legs, and where each sits in the hull frame. */
  private readonly extras: { collider: RAPIER.Collider; offset: readonly [number, number, number] }[]

  constructor(private readonly world: RAPIER.World, private readonly kind: number | undefined) {
    const [hx, hy, hz] = halfExtentsOf(kind)
    this.hull = world.createCollider(RAPIER.ColliderDesc.cuboid(hx, hy, hz))
    this.extras = extraSolids(kind).map((extra) => ({ collider: world.createCollider(extra.desc), offset: extra.offset }))
  }

  place(pose: PoseLike) {
    // A tank whose turret was shot off has no turret solid.
    if (this.kind === KIND_TANK) {
      const present = pose.damage?.[DAMAGE_TURRET] !== TURRET_GONE
      for (const { collider } of this.extras) collider.setEnabled(present)
    }
    this.hull.setTranslation({ x: pose.x, y: pose.y, z: pose.z })
    this.hull.setRotation({ x: pose.qx, y: pose.qy, z: pose.qz, w: pose.qw })
    for (const { collider, offset } of this.extras) {
      // The offset in the hull frame, rotated into the world.
      const [ox, oy, oz] = offset
      const { qx: x, qy: y, qz: z, qw: w } = pose
      const tx = ox + 2 * (w * (y * oz - z * oy) + y * (x * oy - y * ox) - z * (z * ox - x * oz))
      const ty = oy + 2 * (w * (z * ox - x * oz) + z * (y * oz - z * oy) - x * (x * oy - y * ox))
      const tz = oz + 2 * (w * (x * oy - y * ox) + x * (z * ox - x * oz) - y * (y * oz - z * oy))
      collider.setTranslation({ x: pose.x + tx, y: pose.y + ty, z: pose.z + tz })
      collider.setRotation({ x, y, z, w })
    }
  }

  remove() {
    this.world.removeCollider(this.hull, false)
    for (const { collider } of this.extras) this.world.removeCollider(collider, false)
  }
}

/** The extra solids of a vehicle kind and their place in the hull frame: a tank's turret, a plane's wings, a walker's legs. */
export function extraSolids(kind: number | undefined): { desc: RAPIER.ColliderDesc; offset: readonly [number, number, number] }[] {
  if (kind === KIND_TANK) return [{ desc: RAPIER.ColliderDesc.cylinder(TANK_TURRET_HALF_HEIGHT, TANK_TURRET_RADIUS), offset: TANK_TURRET_PIVOT }]
  if (kind === KIND_STRV) {
    const [x, y, z] = STRV_CASEMATE_HALF
    return [{ desc: RAPIER.ColliderDesc.cuboid(x, y, z), offset: STRV_CASEMATE_CENTRE }]
  }
  if (kind === KIND_PLANE) {
    const [x, y, z] = PLANE_WING_HALF_EXTENTS
    return [{ desc: RAPIER.ColliderDesc.cuboid(x, y, z), offset: PLANE_WING_CENTRE }]
  }
  if (kind === KIND_WALKER || kind === KIND_HEN) {
    const half = (WALKER_LEG_LENGTH - WALKER_LEG_CLEARANCE) / 2
    const hy = WALKER_HALF_EXTENTS[1]
    return [-1, 1].map((side) => ({
      desc: RAPIER.ColliderDesc.cuboid(WALKER_LEG_HALF_WIDTH, half, WALKER_LEG_HALF_DEPTH),
      offset: [side * WALKER_FOOT_HALF, -hy - half, 0] as const,
    }))
  }
  return []
}

/** Rapier time on this client since the debug readout last reset it (see `render/perf.ts`). */
export const physicsClock = { ms: 0, calls: 0 }

/** Runs `work` and adds its time to `physicsClock`. */
export function timedPhysics<T>(work: () => T): T {
  const started = performance.now()
  try {
    return work()
  } finally {
    physicsClock.ms += performance.now() - started
    physicsClock.calls++
  }
}

/** The same Rapier character controller and static scene the server simulates. */
