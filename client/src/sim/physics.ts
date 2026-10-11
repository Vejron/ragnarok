import RAPIER from '@dimforge/rapier3d-compat'
import type { TileData, WorldData } from '@/world'
import {
  AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, CONTROLLER_OFFSET,
  BUTTON_CROUCH, BUTTON_JUMP, KNOCK_MAX_SPEED, MAX_SLOPE_CLIMB_DEG, SNAP_DISTANCE,
  TANK_TURRET_HALF_HEIGHT, TANK_TURRET_PIVOT, TANK_TURRET_RADIUS,
} from './consts'
import { halfExtentsOf } from './kinds'
import type { TileGround } from './terrain'
import { beginSlide, decayKnock, desiredTranslation, f32, settleVertical } from './movement'
import { fromMrad, KIND_TANK, type InputFrame, type VehicleState } from '@/net/codec'

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
}

/** Moves a character shape through the static world. Abstracted so tests can use a flat floor. */
export interface CharacterMotor {
  /** `snap` pulls a character that was standing back onto a slope it walks down (horizontal pass only). */
  move(position: Vec3, desired: Vec3, snap?: boolean): { translation: Vec3; grounded: boolean }
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
export function stepCharacter(motor: CharacterMotor, state: CharacterState, input: InputFrame, dt: number, speedScale = 1): CharacterState {
  // Jumping off a moving car keeps its speed, like on the server.
  const velocity = state.ride !== 0 ? motor.vehicleVelocity?.(state.ride) : undefined
  const jumped = velocity !== undefined && state.grounded && (input.buttons & BUTTON_JUMP) !== 0
  const carried = beginSlide(jumped ? inherit(state.knock, velocity) : state.knock, state.grounded, input)
  const { vy, desired } = desiredTranslation(state.vy, state.grounded, carried, input, dt, speedScale)
  // Standing and not jumping: stay on the ground over a slope. Mirrors the choice in `move_character`.
  const moved = motor.move(state.position, desired, state.grounded && vy <= 0)
  const translation = moved.translation
  // The tick a jump starts the capsule is still touching the floor; it is not grounded.
  const grounded = moved.grounded && vy <= 0
  const [x, y, z] = state.position
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

type PoseLike = Pick<VehicleState, 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'>

/**
 * A snapshot vehicle as static solids: its hull box, and for a tank the turret cylinder on the
 * ring. Mirrors the colliders `add_car` gives the server's body.
 */
export class VehicleSolids {
  private readonly hull: RAPIER.Collider
  private readonly turret: RAPIER.Collider | null

  constructor(private readonly world: RAPIER.World, kind: number | undefined) {
    const [hx, hy, hz] = halfExtentsOf(kind)
    this.hull = world.createCollider(RAPIER.ColliderDesc.cuboid(hx, hy, hz))
    this.turret = kind === KIND_TANK ? world.createCollider(RAPIER.ColliderDesc.cylinder(TANK_TURRET_HALF_HEIGHT, TANK_TURRET_RADIUS)) : null
  }

  place(pose: PoseLike) {
    this.hull.setTranslation({ x: pose.x, y: pose.y, z: pose.z })
    this.hull.setRotation({ x: pose.qx, y: pose.qy, z: pose.qz, w: pose.qw })
    if (!this.turret) return
    // The ring's offset in the hull frame, rotated into the world.
    const [ox, oy, oz] = TANK_TURRET_PIVOT
    const { qx: x, qy: y, qz: z, qw: w } = pose
    const tx = ox + 2 * (w * (y * oz - z * oy) + y * (x * oy - y * ox) - z * (z * ox - x * oz))
    const ty = oy + 2 * (w * (z * ox - x * oz) + z * (y * oz - z * oy) - x * (x * oy - y * ox))
    const tz = oz + 2 * (w * (x * oy - y * ox) + x * (z * ox - x * oz) - y * (y * oz - z * oy))
    this.turret.setTranslation({ x: pose.x + tx, y: pose.y + ty, z: pose.z + tz })
    this.turret.setRotation({ x, y, z, w })
  }

  remove() {
    this.world.removeCollider(this.hull, false)
    if (this.turret) this.world.removeCollider(this.turret, false)
  }
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
    // One step builds the broad-phase so scene queries see the static colliders.
    world.step()
    return new RapierMotor(world, controller, snapping, body)
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
  setVehicles(vehicles: readonly (Pick<VehicleState, 'id' | 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'> & { kind?: number; vx?: number; vz?: number })[]) {
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

  removeTile(colliders: readonly RAPIER.Collider[]) {
    for (const collider of colliders) this.world.removeCollider(collider, false)
  }

  /** Horizontal then vertical, in two passes, exactly like `Physics::move_character`. */
  move(position: Vec3, desired: Vec3, snap = false) {
    const h = this.pass(snap ? this.snapping : this.controller, position, [desired[0], 0, desired[2]])
    const middle: Vec3 = [f32(position[0] + h.x), f32(position[1] + h.y), f32(position[2] + h.z)]
    const v = this.pass(this.controller, middle, [0, desired[1], 0])
    const grounded = this.controller.computedGrounded()
    return { translation: [f32(h.x + v.x), f32(h.y + v.y), f32(h.z + v.z)] as Vec3, grounded }
  }

  private pass(controller: RAPIER.KinematicCharacterController, position: Vec3, desired: Vec3) {
    this.body.setTranslation({ x: position[0], y: position[1], z: position[2] })
    controller.computeColliderMovement(this.body, { x: desired[0], y: desired[1], z: desired[2] })
    return controller.computedMovement()
  }
}
