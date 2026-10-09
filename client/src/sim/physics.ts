import RAPIER from '@dimforge/rapier3d-compat'
import type { WorldData } from '@/world'
import {
  AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, CONTROLLER_OFFSET,
  BUTTON_JUMP, CAR_HALF_EXTENTS, KNOCK_MAX_SPEED, MAX_SLOPE_CLIMB_DEG, TANK_HALF_EXTENTS,
} from './consts'
import { decayKnock, desiredTranslation, f32, settleVertical } from './movement'
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
  move(position: Vec3, desired: Vec3): { translation: Vec3; grounded: boolean }
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
export function stepCharacter(motor: CharacterMotor, state: CharacterState, input: InputFrame, dt: number): CharacterState {
  // Jumping off a moving car keeps its speed, like on the server.
  const velocity = state.ride !== 0 ? motor.vehicleVelocity?.(state.ride) : undefined
  const jumped = velocity !== undefined && state.grounded && (input.buttons & BUTTON_JUMP) !== 0
  const carried = jumped ? inherit(state.knock, velocity) : state.knock
  const { vy, desired } = desiredTranslation(state.vy, state.grounded, carried, input, dt)
  const moved = motor.move(state.position, desired)
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
    knock: decayKnock(left ? inherit(carried, velocity) : carried, grounded),
    ride: grounded ? state.ride : 0,
  }
}

const BOUNDARY_HEIGHT = 50

/** Mirrors `static_colliders` on the server: ground slab, boundary walls, building mesh. */
function buildStaticColliders(world: RAPIER.World, data: WorldData) {
  const half = data.halfExtent
  world.createCollider(RAPIER.ColliderDesc.cuboid(half, 0.5, half).setTranslation(0, -0.5, 0))
  const wall = BOUNDARY_HEIGHT / 2
  for (const [x, z, hx, hz] of [[half, 0, 0.5, half], [-half, 0, 0.5, half], [0, half, half, 0.5], [0, -half, half, 0.5]] as const) {
    world.createCollider(RAPIER.ColliderDesc.cuboid(hx, wall, hz).setTranslation(x, wall, z))
  }
  if (data.collision.triangles.length > 0) {
    world.createCollider(RAPIER.ColliderDesc.trimesh(data.collision.vertices, data.collision.triangles))
  }
}

/** The same Rapier character controller and static scene the server simulates. */
export class RapierMotor implements CharacterMotor {
  /** Solid stand-ins for the server's cars, by vehicle id. */
  private readonly cars = new Map<number, RAPIER.Collider>()
  private readonly carVelocities = new Map<number, readonly [number, number]>()

  private constructor(
    private readonly world: RAPIER.World,
    private readonly controller: RAPIER.KinematicCharacterController,
    private readonly body: RAPIER.Collider,
  ) {}

  static async create(data: WorldData): Promise<RapierMotor> {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: 0, z: 0 })
    buildStaticColliders(world, data)
    const controller = world.createCharacterController(CONTROLLER_OFFSET)
    controller.setSlideEnabled(true)
    controller.enableAutostep(AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, false)
    controller.setMaxSlopeClimbAngle((MAX_SLOPE_CLIMB_DEG * Math.PI) / 180)
    controller.setMinSlopeSlideAngle((MAX_SLOPE_CLIMB_DEG * Math.PI) / 180)
    // Snapping is off, as on the server (see `Physics::new`).
    controller.disableSnapToGround()
    // The character's own shape. The controller excludes it from its own queries.
    const body = world.createCollider(RAPIER.ColliderDesc.capsule(CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS).setSensor(true))
    // One step builds the broad-phase so scene queries see the static colliders.
    world.step()
    return new RapierMotor(world, controller, body)
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
      let collider = this.cars.get(car.id)
      if (!collider) {
        const [hx, hy, hz] = car.kind === KIND_TANK ? TANK_HALF_EXTENTS : CAR_HALF_EXTENTS
        collider = this.world.createCollider(RAPIER.ColliderDesc.cuboid(hx, hy, hz))
        this.cars.set(car.id, collider)
      }
      collider.setTranslation({ x: car.x, y: car.y, z: car.z })
      collider.setRotation({ x: car.qx, y: car.qy, z: car.qz, w: car.qw })
    }
    for (const [id, collider] of this.cars) {
      if (!alive.has(id)) {
        this.world.removeCollider(collider, false)
        this.cars.delete(id)
        this.carVelocities.delete(id)
      }
    }
    // Moved colliders reach the broad-phase on the next step.
    if (vehicles.length > 0 || this.cars.size > 0) this.world.step()
  }

  vehicleVelocity(id: number) {
    return this.carVelocities.get(id) ?? [0, 0]
  }

  /** Horizontal then vertical, in two passes, exactly like `Physics::move_character`. */
  move(position: Vec3, desired: Vec3) {
    const h = this.pass(position, [desired[0], 0, desired[2]])
    const middle: Vec3 = [f32(position[0] + h.x), f32(position[1] + h.y), f32(position[2] + h.z)]
    const v = this.pass(middle, [0, desired[1], 0])
    const grounded = this.controller.computedGrounded()
    return { translation: [f32(h.x + v.x), f32(h.y + v.y), f32(h.z + v.z)] as Vec3, grounded }
  }

  private pass(position: Vec3, desired: Vec3) {
    this.body.setTranslation({ x: position[0], y: position[1], z: position[2] })
    this.controller.computeColliderMovement(this.body, { x: desired[0], y: desired[1], z: desired[2] })
    return this.controller.computedMovement()
  }
}
