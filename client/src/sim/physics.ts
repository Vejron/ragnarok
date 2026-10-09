import RAPIER from '@dimforge/rapier3d-compat'
import type { WorldData } from '@/world'
import {
  AUTOSTEP_HEIGHT, AUTOSTEP_MIN_WIDTH, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, CONTROLLER_OFFSET,
  MAX_SLOPE_CLIMB_DEG,
} from './consts'
import { desiredTranslation, f32, settleVertical } from './movement'
import { fromMrad, type InputFrame } from '@/net/codec'

export type Vec3 = [number, number, number]

/** Mirrors `Character` in `crates/server/src/physics.rs`. Position is the capsule centre. */
export interface CharacterState {
  position: Vec3
  yaw: number
  vy: number
  grounded: boolean
}

/** Moves a character shape through the static world. Abstracted so tests can use a flat floor. */
export interface CharacterMotor {
  move(position: Vec3, desired: Vec3): { translation: Vec3; grounded: boolean }
}

/** One tick of movement. Mirrors `Physics::move_character` on the server. */
export function stepCharacter(motor: CharacterMotor, state: CharacterState, input: InputFrame, dt: number): CharacterState {
  const { vy, desired } = desiredTranslation(state.vy, state.grounded, input, dt)
  const moved = motor.move(state.position, desired)
  const translation = moved.translation
  // The tick a jump starts the capsule is still touching the floor; it is not grounded.
  const grounded = moved.grounded && vy <= 0
  const [x, y, z] = state.position
  return {
    position: [f32(x + translation[0]), f32(y + translation[1]), f32(z + translation[2])],
    yaw: fromMrad(input.yaw),
    vy: settleVertical(vy, desired[1], translation[1], grounded),
    grounded,
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
