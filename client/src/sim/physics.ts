import RAPIER from '@dimforge/rapier3d-compat'
import { buildingTriangleRanges, type TileData, type WorldData } from '@/world'
import {
  AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, CONTROLLER_OFFSET,
  BUTTON_CROUCH, BUTTON_JUMP, BUTTON_PRONE, KNOCK_MAX_SPEED, MAX_SLOPE_CLIMB_DEG, PRONE_CENTRE, PRONE_RADIUS, SNAP_DISTANCE, STAND_CENTRE,
  STRV_CASEMATE_CENTRE, STRV_CASEMATE_HALF, TANK_TURRET_HALF_HEIGHT, TANK_TURRET_PIVOT, TANK_TURRET_RADIUS, PLANE_WING_CENTRE, PLANE_WING_HALF_EXTENTS,
  WALKER_FOOT_HALF, WALKER_HALF_EXTENTS, WALKER_LEG_CLEARANCE, WALKER_LEG_HALF_DEPTH, WALKER_LEG_HALF_WIDTH, WALKER_LEG_LENGTH,
} from './consts'
import { halfExtentsOf } from './kinds'
import type { TileGround } from './terrain'
import { beginSlide, decayKnock, desiredTranslation, f32, settleVertical } from './movement'
import { DAMAGE_TURRET, fromMrad, KIND_PLANE, KIND_STRV, KIND_TANK, KIND_WALKER, TURRET_GONE, type InputFrame, type VehicleState } from '@/net/codec'

export type Vec3 = [number, number, number]

/** Mirrors `Character` in `crates/server/src/physics.rs`. Position is the capsule centre. */
export interface CharacterState {
  position: Vec3
  yaw: number
  vy: number
  grounded: boolean
  /** Leftover knockback velocity `[x, z]`, m/s. */
  knock: [number, number]
  /** Id of the vehicle stood on, or 0. The predicted frame is then the one where that vehicle is still. */
  ride: number
  /** Lying down. Mirrors `Character::prone`. */
  prone: boolean
}

/** Moves a character shape through the static world. Abstracted so tests can use a flat floor. */
export interface CharacterMotor {
  /** `snap` pulls a character that was standing back onto a slope it walks down (horizontal pass only). `prone` moves the lying-down body. */
  move(position: Vec3, desired: Vec3, snap?: boolean, prone?: boolean): { translation: Vec3; grounded: boolean }
  /** Whether the standing or lying-down body fits at `position`, among the static world and the cars. Defaults to yes. */
  fits?(position: Vec3, prone: boolean): boolean
  /** Horizontal velocity `[x, z]` of a vehicle, m/s, for characters leaving it. */
  vehicleVelocity?(id: number): readonly [number, number]
}

/** Adds a vehicle's speed to knockback, within the wire's limit. Mirrors `inherit` in movement.rs. */
function inherit(knock: readonly [number, number], velocity: readonly [number, number]): [number, number] {
  let x = f32(knock[0] + velocity[0])
  let z = f32(knock[1] + velocity[1])
  const speed = Math.hypot(x, z)
  if (speed > KNOCK_MAX_SPEED) {
    x = f32((x / speed) * KNOCK_MAX_SPEED)
    z = f32((z / speed) * KNOCK_MAX_SPEED)
  }
  return [x, z]
}

/** One tick of movement. Mirrors `Physics::move_character` on the server. */
/** How far the centre drops between standing and lying down, metres. Mirrors `change_stance` on the server. */
const STANCE_DROP = STAND_CENTRE - PRONE_CENTRE

/** Lies down or gets up, keeping the feet where they are. Mirrors `Physics::change_stance`. */
function changeStance(motor: CharacterMotor, position: Vec3, prone: boolean, wantsProne: boolean): { position: Vec3; prone: boolean } {
  if (wantsProne === prone) return { position, prone }
  const [x, y, z] = position
  const raised: Vec3 = [x, f32(y + (wantsProne ? -STANCE_DROP : STANCE_DROP)), z]
  // Getting up is refused while the standing body would not fit.
  if (wantsProne || motor.fits?.(raised, false) !== false) return { position: raised, prone: wantsProne }
  return { position, prone }
}

export function stepCharacter(motor: CharacterMotor, state: CharacterState, input: InputFrame, dt: number, speedScale = 1): CharacterState {
  const stance = changeStance(motor, state.position, state.prone, (input.buttons & BUTTON_PRONE) !== 0)
  // Jumping off a moving car keeps its speed, like on the server.
  const velocity = state.ride !== 0 ? motor.vehicleVelocity?.(state.ride) : undefined
  const jumped = velocity !== undefined && state.grounded && (input.buttons & BUTTON_JUMP) !== 0
  const carried = beginSlide(jumped ? inherit(state.knock, velocity) : state.knock, state.grounded, input)
  const { vy, desired } = desiredTranslation(state.vy, state.grounded, carried, input, dt, speedScale)
  // Standing and not jumping: stay on the ground over a slope. Mirrors the choice in `move_character`.
  const moved = motor.move(stance.position, desired, state.grounded && vy <= 0, stance.prone)
  const translation = moved.translation
  // The tick a jump starts the capsule is still touching the floor; it is not grounded.
  const grounded = moved.grounded && vy <= 0
  const [x, y, z] = stance.position
  // Walking off the roof into the air does the same. The client cannot tell a step down onto the
  // ground from a step down onto the car, so `ride` stays until the next snapshot says otherwise.
  const left = velocity !== undefined && !grounded && !jumped
  return {
    position: [f32(x + translation[0]), f32(y + translation[1]), f32(z + translation[2])],
    yaw: fromMrad(input.yaw),
    vy: settleVertical(vy, desired[1], translation[1], grounded),
    grounded,
    knock: decayKnock(left ? inherit(carried, velocity) : carried, grounded, (input.buttons & BUTTON_CROUCH) !== 0),
    ride: grounded ? state.ride : 0,
    prone: stance.prone,
  }
}

/** Top of the boundary walls, metres above the lowest ground. Mirrors `BOUNDARY_HEIGHT` on the server. */
const BOUNDARY_HEIGHT = 200

/** Depth of the slab under a terrain, metres below y = 0 (the deepest water bed). Mirrors `BED_FLOOR` on the server. */
const BED_FLOOR = 12

/**
 * Mirrors `static_colliders` on the server: ground slab, boundary walls, building mesh. The
 * terrain arrives with its tiles (`addTileCollider`).
 */
export function buildStaticColliders(world: RAPIER.World, data: WorldData) {
  const half = data.halfExtent
  const top = data.groundCell > 0 ? -BED_FLOOR : 0
  world.createCollider(RAPIER.ColliderDesc.cuboid(half, 0.5, half).setTranslation(0, top - 0.5, 0))
  const wall = BOUNDARY_HEIGHT / 2
  for (const [x, z, hx, hz] of [[half, 0, 0.5, half], [-half, 0, 0.5, half], [0, half, half, 0.5], [0, -half, half, 0.5]] as const) {
    world.createCollider(RAPIER.ColliderDesc.cuboid(hx, wall, hz).setTranslation(x, wall, z))
  }
  if (data.collision.triangles.length > 0) {
    world.createCollider(RAPIER.ColliderDesc.trimesh(data.collision.vertices, data.collision.triangles))
  }
}

/**
 * One tile's building mesh, tree trunks and terrain as static colliders (none for an empty tile). Tiles come
 * and go as the player moves; the server holds all of them merged, which is the same geometry, so
 * near the player the two agree. The heightfield is laid out as the server's: centred on its
 * square, scaled to its width, heights column-major with rows along Z.
 */
export function addTileCollider(world: RAPIER.World, tile: TileData, ground: TileGround | null = null): RAPIER.Collider[] {
  const colliders: RAPIER.Collider[] = []
  if (tile.collision.triangles.length > 0) {
    colliders.push(world.createCollider(RAPIER.ColliderDesc.trimesh(tile.collision.vertices, tile.collision.triangles)))
  }
  // Trunks, as on the server: an upright cylinder from the foot up to where the crown starts.
  for (const tree of tile.trees) {
    if (!tree.solid) continue
    const half = tree.trunkHeight / 2
    colliders.push(
      world.createCollider(RAPIER.ColliderDesc.cylinder(half, tree.trunkRadius).setTranslation(tree.x, tree.base + half, tree.z)),
    )
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
 * The tile's building mesh again, without the buildings in `destroyed`: what `addTileCollider`
 * made for it, minus those triangles, so a collapsed building stops being a wall here as it
 * does on the server. Returns the new collider (the caller removes the old one), or null when
 * nothing is left of the mesh.
 */
export function rebuildBuildingCollider(world: RAPIER.World, tile: TileData, destroyed: ReadonlySet<number>): RAPIER.Collider | null {
  const { triangles, vertices } = tile.collision
  const gone = buildingTriangleRanges(tile).filter((_, index) => destroyed.has(index))
  const kept: number[] = []
  for (let t = 0; t < triangles.length / 3; t++) {
    if (gone.some(([start, end]) => t >= start && t < end)) continue
    kept.push(triangles[t * 3]!, triangles[t * 3 + 1]!, triangles[t * 3 + 2]!)
  }
  if (kept.length === 0) return null
  return world.createCollider(RAPIER.ColliderDesc.trimesh(vertices, Uint32Array.from(kept)))
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
  if (kind === KIND_WALKER) {
    const half = (WALKER_LEG_LENGTH - WALKER_LEG_CLEARANCE) / 2
    const hy = WALKER_HALF_EXTENTS[1]
    return [-1, 1].map((side) => ({
      desc: RAPIER.ColliderDesc.cuboid(WALKER_LEG_HALF_WIDTH, half, WALKER_LEG_HALF_DEPTH),
      offset: [side * WALKER_FOOT_HALF, -hy - half, 0] as const,
    }))
  }
  return []
}

/** The same Rapier character controller and static scene the server simulates. */
export class RapierMotor implements CharacterMotor {
  /** Solid stand-ins for the server's cars, by vehicle id. */
  private readonly cars = new Map<number, VehicleSolids>()
  private readonly carVelocities = new Map<number, readonly [number, number]>()

  private constructor(
    private readonly world: RAPIER.World,
    private readonly controller: RAPIER.KinematicCharacterController,
    private readonly snapping: RAPIER.KinematicCharacterController,
    private readonly body: RAPIER.Collider,
    private readonly proneBody: RAPIER.Collider,
  ) {}

  static async create(data: WorldData): Promise<RapierMotor> {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: 0, z: 0 })
    buildStaticColliders(world, data)
    const controller = RapierMotor.controller(world)
    // Snapping is only on for the horizontal pass of a standing character (see `controller()` on the server).
    const snapping = RapierMotor.controller(world)
    snapping.enableSnapToGround(SNAP_DISTANCE)
    // The character's own shape. The controller excludes it from its own queries.
    const body = world.createCollider(RAPIER.ColliderDesc.capsule(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS).setSensor(true))
    // The same body lying down, a ball resting on the ground.
    const proneBody = world.createCollider(RAPIER.ColliderDesc.ball(PRONE_RADIUS).setSensor(true))
    // One step builds the broad-phase so scene queries see the static colliders.
    world.step()
    return new RapierMotor(world, controller, snapping, body, proneBody)
  }

  private static controller(world: RAPIER.World) {
    const controller = world.createCharacterController(CONTROLLER_OFFSET)
    controller.setSlideEnabled(true)
    controller.enableAutostep(AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, false)
    controller.setMaxSlopeClimbAngle((MAX_SLOPE_CLIMB_DEG * Math.PI) / 180)
    controller.setMinSlopeSlideAngle((MAX_SLOPE_CLIMB_DEG * Math.PI) / 180)
    controller.disableSnapToGround()
    return controller
  }

  /**
   * Makes the cars solid for the character, at the poses of the newest snapshot. The server runs
   * them as dynamic bodies; here they only need to block the predicted stickman like they block
   * the real one. Cars missing from `vehicles` are removed.
   */
  setVehicles(vehicles: readonly (Pick<VehicleState, 'id' | 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'> & { kind?: number; vx?: number; vz?: number; damage?: readonly number[] })[]) {
    const alive = new Set<number>()
    for (const car of vehicles) {
      alive.add(car.id)
      this.carVelocities.set(car.id, [car.vx ?? 0, car.vz ?? 0])
      let solids = this.cars.get(car.id)
      if (!solids) {
        solids = new VehicleSolids(this.world, car.kind)
        this.cars.set(car.id, solids)
      }
      solids.place(car)
    }
    for (const [id, solids] of this.cars) {
      if (!alive.has(id)) {
        solids.remove()
        this.cars.delete(id)
        this.carVelocities.delete(id)
      }
    }
    // Moved colliders reach the broad-phase on the next step.
    if (vehicles.length > 0 || this.cars.size > 0) this.world.step()
  }

  /**
   * How far a ray travels before it meets the world or a car, and the surface normal there; null
   * when it reaches `max` clear. Used to draw where a grenade will go.
   */
  castRay(origin: Vec3, dir: Vec3, max: number): { t: number; normal: Vec3 } | null {
    const ray = new RAPIER.Ray({ x: origin[0], y: origin[1], z: origin[2] }, { x: dir[0], y: dir[1], z: dir[2] })
    const hit = this.world.castRayAndGetNormal(ray, max, true, RAPIER.QueryFilterFlags.EXCLUDE_SENSORS)
    if (!hit) return null
    return { t: hit.timeOfImpact, normal: [hit.normal.x, hit.normal.y, hit.normal.z] }
  }

  vehicleVelocity(id: number) {
    return this.carVelocities.get(id) ?? [0, 0]
  }

  /** Adds a streamed tile's collision; returns what to pass to `removeTile` when it unloads. */
  addTile(tile: TileData, ground: TileGround | null = null): RAPIER.Collider[] {
    const colliders = addTileCollider(this.world, tile, ground)
    // Reach the broad-phase now, so the very next move already collides with it.
    if (colliders.length > 0) this.world.step()
    return colliders
  }

  /** Colliders already taken out one by one (a felled tree's trunk) are `undefined`. */
  removeTile(colliders: readonly (RAPIER.Collider | undefined)[]) {
    for (const collider of colliders) if (collider) this.world.removeCollider(collider, false)
  }

  /**
   * Swaps a tile's building collider (`addTile`'s first, when the tile has a mesh) for one
   * without the buildings in `destroyed`. Returns the new one, or undefined if none is left.
   */
  rebuildBuildings(old: RAPIER.Collider | undefined, tile: TileData, destroyed: ReadonlySet<number>): RAPIER.Collider | undefined {
    if (old) this.world.removeCollider(old, false)
    const next = rebuildBuildingCollider(this.world, tile, destroyed) ?? undefined
    if (next) this.world.step()
    return next
  }

  /** Horizontal then vertical, in two passes, exactly like `Physics::move_character`. */
  move(position: Vec3, desired: Vec3, snap = false, prone = false) {
    const body = prone ? this.proneBody : this.body
    const h = this.pass(body, snap ? this.snapping : this.controller, position, [desired[0], 0, desired[2]])
    const middle: Vec3 = [f32(position[0] + h.x), f32(position[1] + h.y), f32(position[2] + h.z)]
    const v = this.pass(body, this.controller, middle, [0, desired[1], 0])
    const grounded = this.controller.computedGrounded()
    return { translation: [f32(h.x + v.x), f32(h.y + v.y), f32(h.z + v.z)] as Vec3, grounded }
  }

  /** Whether the standing or lying-down body fits at `position`. Mirrors `Physics::fits`. */
  fits(position: Vec3, prone: boolean): boolean {
    const body = prone ? this.proneBody : this.body
    const hit = this.world.intersectionWithShape(
      { x: position[0], y: position[1], z: position[2] },
      { x: 0, y: 0, z: 0, w: 1 },
      body.shape,
      undefined,
      undefined,
      undefined,
      undefined,
      // The character's own bodies are not part of the world it must fit in.
      (collider) => collider !== this.body && collider !== this.proneBody,
    )
    return hit === null
  }

  private pass(body: RAPIER.Collider, controller: RAPIER.KinematicCharacterController, position: Vec3, desired: Vec3) {
    body.setTranslation({ x: position[0], y: position[1], z: position[2] })
    // Both bodies are sensors in the world: the character must not collide with the one it is not using.
    controller.computeColliderMovement(body, { x: desired[0], y: desired[1], z: desired[2] }, undefined, undefined, (collider) => collider !== this.body && collider !== this.proneBody)
    return controller.computedMovement()
  }
}
