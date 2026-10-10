import RAPIER from '@dimforge/rapier3d-compat'
import {
  BUTTON_JUMP, TICK_DT, CAR_AXLE_Z, CAR_HALF_EXTENTS, CAR_HARD_POINT_Y, CAR_MAX_SPEED, CAR_MAX_STEER, CAR_SUSPENSION_REST, CAR_TRACK_HALF, CAR_WHEEL_RADIUS,
  TANK_AXLES_Z, TANK_HALF_EXTENTS, TANK_HARD_POINT_Y, TANK_MAX_SPEED, TANK_SUSPENSION_REST, TANK_TRACK_HALF, TANK_TURN_RATE,
  TANK_WHEEL_RADIUS, HELI_BANK, HELI_CEILING, HELI_CLIMB_SPEED, HELI_HALF_EXTENTS, HELI_LEVEL_GAIN, HELI_RESPONSE, HELI_SINK_SPEED, HELI_TILT,
  HELI_VERT_ACCEL, HELI_VERT_GAIN, HELI_YAW_RATE, BUTTON_RUN,
} from './consts'
import { halfExtentsOf } from './kinds'
import { addTileCollider, buildStaticColliders } from './physics'
import { fromAxis, KIND_HELI, KIND_TANK, type DriveState, type InputFrame, type VehicleState } from '@/net/codec'
import type { TileData, WorldData } from '@/world'
import * as THREE from 'three'

/**
 * Mirrors `crates/server/src/physics/vehicle.rs`: a dynamic body with ray-cast wheels, driven the
 * same way, so the driver's own vehicle can be predicted.
 */

/** Gravity on vehicle bodies. */
export const CAR_GRAVITY = 9.81

interface Spec {
  halfExtents: readonly [number, number, number]
  density: number
  wheelRadius: number
  trackHalf: number
  axles: readonly number[]
  hardPointY: number
  suspensionRest: number
  maxSuspensionForce: number
  engineForce: number
  maxSpeed: number
  steerLock: number
  turnRate: number
  flies: boolean
  linearDamping: number
}

const CAR: Spec = {
  halfExtents: CAR_HALF_EXTENTS,
  density: 200,
  wheelRadius: CAR_WHEEL_RADIUS,
  trackHalf: CAR_TRACK_HALF,
  axles: [-CAR_AXLE_Z, CAR_AXLE_Z],
  hardPointY: CAR_HARD_POINT_Y,
  suspensionRest: CAR_SUSPENSION_REST,
  maxSuspensionForce: 60_000,
  engineForce: 7000,
  maxSpeed: CAR_MAX_SPEED,
  steerLock: CAR_MAX_STEER,
  turnRate: 0,
  flies: false,
  linearDamping: 0.05,
}

const TANK: Spec = {
  halfExtents: TANK_HALF_EXTENTS,
  density: 250,
  wheelRadius: TANK_WHEEL_RADIUS,
  trackHalf: TANK_TRACK_HALF,
  axles: TANK_AXLES_Z,
  hardPointY: TANK_HARD_POINT_Y,
  suspensionRest: TANK_SUSPENSION_REST,
  maxSuspensionForce: 200_000,
  engineForce: 22_000,
  maxSpeed: TANK_MAX_SPEED,
  steerLock: 0,
  turnRate: TANK_TURN_RATE,
  flies: false,
  linearDamping: 0.05,
}

const HELI: Spec = {
  halfExtents: HELI_HALF_EXTENTS,
  density: 300,
  wheelRadius: 0,
  trackHalf: 0,
  axles: [],
  hardPointY: 0,
  suspensionRest: 0,
  maxSuspensionForce: 0,
  engineForce: 0,
  maxSpeed: 0,
  steerLock: 0,
  turnRate: 0,
  flies: true,
  linearDamping: 0.2,
}

const TURN_ACCEL = 3.0
const BRAKE_PER_KG = 0.035
const COAST_BRAKE_PER_KG = 0.0015
const STEER_RATE = 3.0
const STEER_FADE_SPEED = 14.0

export interface CarControls {
  /** -1 (reverse) ..= 1 (forward). */
  throttle: number
  /** -1 (left) ..= 1 (right). */
  steer: number
  handbrake: boolean
  /** -1 (sink) ..= 1 (climb). Only a helicopter has it. */
  lift: number
  /** A pilot is aboard and the rotor turns. Only a helicopter has it. */
  powered: boolean
}

export interface CarBodyState {
  position: [number, number, number]
  /** Unit quaternion `[x, y, z, w]`. */
  rotation: [number, number, number, number]
  linvel: [number, number, number]
  angvel: [number, number, number]
}

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value))

/** One vehicle's body and wheels in a Rapier world. */
export class CarSim {
  private readonly spec: Spec
  private readonly body: RAPIER.RigidBody
  private readonly collider: RAPIER.Collider
  private readonly controller: RAPIER.DynamicRayCastVehicleController | null
  private readonly wheels: number
  /** What a helicopter was last asked to do, applied in the next `step`. */
  private flight: CarControls = { throttle: 0, steer: 0, handbrake: false, lift: 0, powered: false }
  /** Front wheel angle, radians, + is right. */
  steer = 0
  private turn = 0
  private spin = 0

  constructor(private readonly world: RAPIER.World, kind: number, position: [number, number, number], yaw: number) {
    this.spec = kind === KIND_TANK ? TANK : kind === KIND_HELI ? HELI : CAR
    const spec = this.spec
    this.body = world.createRigidBody(
      RAPIER.RigidBodyDesc.dynamic()
        .setTranslation(position[0], position[1], position[2])
        .setRotation({ x: 0, y: Math.sin(yaw / 2), z: 0, w: Math.cos(yaw / 2) })
        .setLinearDamping(spec.linearDamping)
        .setAngularDamping(0.8),
    )
    const [hx, hy, hz] = spec.halfExtents
    this.collider = world.createCollider(RAPIER.ColliderDesc.cuboid(hx, hy, hz).setDensity(spec.density).setFriction(0.4), this.body)
    if (spec.flies) {
      // No wheels: the rotor is a force on the body.
      this.controller = null
      this.wheels = 0
      return
    }
    const controller = world.createVehicleController(this.body)
    this.controller = controller
    controller.setIndexForwardAxis = 2
    for (const z of spec.axles) {
      for (const side of [-1, 1]) {
        controller.addWheel(
          { x: side * spec.trackHalf, y: spec.hardPointY, z },
          { x: 0, y: -1, z: 0 },
          { x: 1, y: 0, z: 0 },
          spec.suspensionRest,
          spec.wheelRadius,
        )
      }
    }
    this.wheels = controller.numWheels()
    for (let i = 0; i < this.wheels; i++) {
      controller.setWheelSuspensionStiffness(i, 25)
      controller.setWheelSuspensionCompression(i, 2.2)
      controller.setWheelSuspensionRelaxation(i, 3)
      controller.setWheelMaxSuspensionTravel(i, 0.3)
      controller.setWheelSideFrictionStiffness(i, 1)
      controller.setWheelFrictionSlip(i, 12)
      controller.setWheelMaxSuspensionForce(i, spec.maxSuspensionForce)
    }
  }

  /** Puts the body in the state the server reported. */
  setState(position: readonly number[], rotation: readonly number[], linvel: readonly number[], angvel: readonly number[], steer: number, spin: number) {
    this.body.setTranslation({ x: position[0]!, y: position[1]!, z: position[2]! }, true)
    this.body.setRotation({ x: rotation[0]!, y: rotation[1]!, z: rotation[2]!, w: rotation[3]! }, true)
    this.body.setLinvel({ x: linvel[0]!, y: linvel[1]!, z: linvel[2]! }, true)
    this.body.setAngvel({ x: angvel[0]!, y: angvel[1]!, z: angvel[2]! }, true)
    this.steer = steer
    this.spin = spin
  }

  /** Steering as a fraction of the lock, for drawing the front wheels. */
  get steerFraction() {
    return this.spec.steerLock > 0 ? clamp(this.steer / this.spec.steerLock, -1, 1) : 0
  }

  /** Removes the body and its wheels from the world. */
  dispose() {
    if (this.controller) this.world.removeVehicleController(this.controller)
    this.world.removeRigidBody(this.body)
  }

  /** Mirrors `Physics::drive_car`. */
  drive(controls: CarControls, dt: number) {
    const spec = this.spec
    const controller = this.controller
    if (!controller) {
      this.flight = controls
      return
    }
    const mass = this.body.mass()
    const r = this.body.rotation()
    // rotation * (0, 0, -1)
    const forward = rotate(r, 0, 0, -1)
    const v = this.body.linvel()
    const speed = forward[0] * v.x + forward[1] * v.y + forward[2] * v.z

    const lock = spec.steerLock / (1 + Math.abs(speed) / STEER_FADE_SPEED)
    const steerInput = clamp(controls.steer, -1, 1)
    const target = steerInput * lock
    const step = STEER_RATE * dt
    this.steer += clamp(target - this.steer, -step, step)
    this.turn = steerInput * spec.turnRate

    const against = controls.throttle * speed < -1
    let engine = 0
    let brake = 0
    if (controls.throttle === 0) brake = COAST_BRAKE_PER_KG * mass
    else if (against) brake = BRAKE_PER_KG * mass
    else {
      const headroom = controls.throttle > 0 ? Math.max(0, 1 - Math.abs(speed) / spec.maxSpeed) : 0.5
      engine = ((controls.throttle * spec.engineForce) / this.wheels) * headroom
    }
    const handbrake = controls.handbrake ? BRAKE_PER_KG * mass : 0
    const frontWheels = spec.steerLock > 0 ? 2 : 0
    for (let i = 0; i < this.wheels; i++) {
      const front = i < frontWheels
      controller.setWheelSteering(i, front ? -this.steer : 0)
      controller.setWheelEngineForce(i, engine)
      controller.setWheelBrake(i, front ? brake : Math.max(brake, handbrake))
    }
  }

  /** Mirrors `Physics::step_cars` for this one vehicle, with the world's own step. */
  step(dt: number) {
    const controller = this.controller
    if (!controller) {
      // Forces persist from step to step until reset.
      this.body.resetForces(false)
      if (this.flight.powered) this.fly(this.flight, dt)
      this.world.step()
      return
    }
    controller.updateVehicle(dt)
    let grounded = false
    for (let i = 0; i < this.wheels; i++) grounded ||= controller.wheelIsInContact(i)
    if (this.turn === 0 || !grounded) this.spin = 0
    else {
      this.spin += clamp(this.turn - this.spin, -TURN_ACCEL * dt, TURN_ACCEL * dt)
      const w = this.body.angvel()
      this.body.setAngvel({ x: w.x, y: -this.spin, z: w.z }, true)
    }
    this.world.step()
  }

  /** Mirrors `fly` in `physics/vehicle.rs`: lean the hull, turn it, and push along its up axis. */
  private fly(controls: CarControls, dt: number) {
    const body = this.body
    const q = body.rotation()
    const up = rotate(q, 0, 1, 0)
    const nose = rotate(q, 0, 0, -1)
    const flatLength = Math.hypot(nose[0], nose[2])
    const heading: [number, number, number] = flatLength > 1e-3 ? [nose[0] / flatLength, 0, nose[2] / flatLength] : [0, 0, -1]
    // heading × (0, 1, 0)
    const right: [number, number, number] = [-heading[2], 0, heading[0]]

    const throttle = clamp(controls.throttle, -1, 1)
    const steer = clamp(controls.steer, -1, 1)
    const lx = heading[0] * throttle * HELI_TILT + right[0] * steer * HELI_BANK
    const ly = 1
    const lz = heading[2] * throttle * HELI_TILT + right[2] * steer * HELI_BANK
    const lean = Math.hypot(lx, ly, lz)
    const [wx, wz] = [lx / lean, lz / lean]
    const wy = ly / lean
    // up × lean
    const turn = [
      (up[1] * wz - up[2] * wy) * HELI_LEVEL_GAIN,
      -steer * HELI_YAW_RATE,
      (up[0] * wy - up[1] * wx) * HELI_LEVEL_GAIN,
    ]
    const w = body.angvel()
    const blend = Math.min(1, HELI_RESPONSE * dt)
    body.setAngvel({ x: w.x + (turn[0]! - w.x) * blend, y: w.y + (turn[1]! - w.y) * blend, z: w.z + (turn[2]! - w.z) * blend }, true)

    const lift = clamp(controls.lift, -1, 1)
    const headroom = clamp((HELI_CEILING - body.translation().y) / 20, -1, 1)
    let target = lift > 0 ? lift * HELI_CLIMB_SPEED * Math.max(0, headroom) : lift * HELI_SINK_SPEED
    if (headroom < 0) target = Math.min(target, headroom * HELI_CLIMB_SPEED)
    const extra = clamp((target - body.linvel().y) * HELI_VERT_GAIN, -HELI_VERT_ACCEL, HELI_VERT_ACCEL)
    const accel = (CAR_GRAVITY + extra) / Math.max(up[1], 0.5)
    const push = accel * body.mass()
    body.addForce({ x: up[0] * push, y: up[1] * push, z: up[2] * push }, true)
  }

  state(): CarBodyState {
    const t = this.body.translation()
    const r = this.body.rotation()
    const v = this.body.linvel()
    const w = this.body.angvel()
    return { position: [t.x, t.y, t.z], rotation: [r.x, r.y, r.z, r.w], linvel: [v.x, v.y, v.z], angvel: [w.x, w.y, w.z] }
  }
}

/** Rotates `(x, y, z)` by the quaternion `q`. */
function rotate(q: { x: number; y: number; z: number; w: number }, x: number, y: number, z: number): [number, number, number] {
  const tx = 2 * (q.y * z - q.z * y)
  const ty = 2 * (q.z * x - q.x * z)
  const tz = 2 * (q.x * y - q.y * x)
  return [x + q.w * tx + (q.y * tz - q.z * ty), y + q.w * ty + (q.z * tx - q.x * tz), z + q.w * tz + (q.x * ty - q.y * tx)]
}


/** What the driver's input asks of the vehicle. Mirrors `Vehicles::on_tick` on the server. */
export function controlsOf(input: InputFrame): CarControls {
  const jump = (input.buttons & BUTTON_JUMP) !== 0
  const run = (input.buttons & BUTTON_RUN) !== 0
  return {
    steer: fromAxis(input.strafe),
    throttle: fromAxis(input.forward),
    handbrake: jump,
    // A helicopter climbs on jump and sinks on run.
    lift: Number(jump) - Number(run),
    powered: true,
  }
}

/** A correction larger than this is a teleport (respawn, unflip), not a misprediction: snap. */
const SNAP_DISTANCE = 3
/** How fast the visual error left by a correction decays, per second. */
const ERROR_DECAY = 10
const MAX_PENDING = 120

type Pose = Pick<VehicleState, 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'>

/** A drawn vehicle: world pose and front wheel steering as a fraction of the lock. */
export interface DrivenPose {
  position: [number, number, number]
  rotation: [number, number, number, number]
  steer: number
}

/**
 * Client-side prediction of the vehicle the local player drives: the same ray-cast vehicle the
 * server simulates, in its own Rapier world of the same static scene. Inputs are applied at once
 * and replayed over each authoritative state, as the stickman's are; what is left of a correction
 * is kept as a visual offset that decays.
 */
export class VehicleDriver {
  private readonly world: RAPIER.World
  private readonly obstacles = new Map<number, RAPIER.Collider>()
  private sim: CarSim | null = null
  private id = 0
  private kind = 0
  private pending: InputFrame[] = []
  private previous: CarBodyState | null = null
  private current: CarBodyState | null = null
  private steer = 0
  private readonly errorPosition = new THREE.Vector3()
  private readonly errorRotation = new THREE.Quaternion()
  /** Size of the last correction, metres. */
  lastCorrection = 0

  private constructor(world: RAPIER.World) {
    this.world = world
  }

  static async create(data: WorldData): Promise<VehicleDriver> {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    buildStaticColliders(world, data)
    world.step()
    return new VehicleDriver(world)
  }

  /** Adds a streamed tile's collision; returns what to pass to `removeTile` when it unloads. */
  addTile(tile: TileData): RAPIER.Collider | null {
    const collider = addTileCollider(this.world, tile)
    // Stepping with nothing simulated only rebuilds the broad-phase. While a vehicle is predicted
    // the next tick's step does it, and stepping here would advance that vehicle.
    if (collider && !this.sim) this.world.step()
    return collider
  }

  removeTile(collider: RAPIER.Collider | null) {
    if (collider) this.world.removeCollider(collider, false)
  }

  /** Whether a vehicle is being predicted, and which. */
  get driving(): number | null {
    return this.sim ? this.id : null
  }

  /** Other vehicles block the predicted one at the poses of the newest snapshot. */
  setObstacles(vehicles: readonly (Pose & { id: number; kind: number })[]) {
    const alive = new Set<number>()
    for (const vehicle of vehicles) {
      if (this.sim && vehicle.id === this.id) continue
      alive.add(vehicle.id)
      let collider = this.obstacles.get(vehicle.id)
      if (!collider) {
        const [hx, hy, hz] = halfExtentsOf(vehicle.kind)
        collider = this.world.createCollider(RAPIER.ColliderDesc.cuboid(hx, hy, hz))
        this.obstacles.set(vehicle.id, collider)
      }
      collider.setTranslation({ x: vehicle.x, y: vehicle.y, z: vehicle.z })
      collider.setRotation({ x: vehicle.qx, y: vehicle.qy, z: vehicle.qz, w: vehicle.qw })
    }
    for (const [id, collider] of this.obstacles) {
      if (!alive.has(id)) {
        this.world.removeCollider(collider, false)
        this.obstacles.delete(id)
      }
    }
  }

  /** Remembers an input the vehicle has not been told of, for a replay once the player drives. */
  record(input: InputFrame) {
    this.pending.push(input)
    if (this.pending.length > MAX_PENDING) this.pending.shift()
  }

  /** Forgets inputs that were never sent, e.g. those made while offline. */
  clearPending() {
    this.pending = []
  }

  acknowledge(ack: number) {
    this.pending = this.pending.filter((input) => input.seq > ack)
  }

  /** Applies one new input to the predicted vehicle, if there is one, and remembers it. */
  apply(input: InputFrame) {
    this.record(input)
    if (!this.sim) return
    this.previous = this.current
    this.current = this.stepWith(input)
  }

  private stepWith(input: InputFrame): CarBodyState {
    this.sim!.drive(controlsOf(input), TICK_DT)
    this.sim!.step(TICK_DT)
    this.steer = this.sim!.steerFraction
    return this.sim!.state()
  }

  /** Rewinds to the server's state as of input `ack` and replays everything newer. */
  reconcile(ack: number, vehicle: VehicleState, drive: DriveState) {
    this.acknowledge(ack)
    if (!this.sim || this.id !== vehicle.id || this.kind !== vehicle.kind) {
      this.leave()
      const collider = this.obstacles.get(vehicle.id)
      if (collider) {
        this.world.removeCollider(collider, false)
        this.obstacles.delete(vehicle.id)
      }
      this.sim = new CarSim(this.world, vehicle.kind, [vehicle.x, vehicle.y, vehicle.z], 0)
      this.id = vehicle.id
      this.kind = vehicle.kind
    }
    const sim = this.sim
    const before = this.current
    sim.setState(
      [vehicle.x, vehicle.y, vehicle.z],
      [vehicle.qx, vehicle.qy, vehicle.qz, vehicle.qw],
      [drive.vx, drive.vy, drive.vz],
      [drive.wx, drive.wy, drive.wz],
      // The server keeps a car's steering in radians, and a tank has none.
      drive.steer,
      drive.spin,
    )
    let state = sim.state()
    let previous = state
    for (const input of this.pending) {
      previous = state
      state = this.stepWith(input)
    }
    this.previous = previous
    this.current = state
    if (!before) {
      this.lastCorrection = 0
      this.errorPosition.set(0, 0, 0)
      this.errorRotation.identity()
      return
    }
    const delta = new THREE.Vector3(...before.position).sub(new THREE.Vector3(...state.position))
    this.lastCorrection = delta.length()
    if (this.lastCorrection > SNAP_DISTANCE) {
      this.errorPosition.set(0, 0, 0)
      this.errorRotation.identity()
      return
    }
    // What was drawn stays where it was, and eases onto the corrected pose.
    this.errorPosition.add(delta)
    const was = new THREE.Quaternion(...before.rotation)
    const now = new THREE.Quaternion(...state.rotation)
    this.errorRotation.multiply(was.multiply(now.invert()))
  }

  /** Stops predicting: the player left the driver's seat. */
  leave() {
    this.sim?.dispose()
    this.sim = null
    this.previous = null
    this.current = null
    this.id = 0
    this.errorPosition.set(0, 0, 0)
    this.errorRotation.identity()
  }

  /** The pose to draw, between the last two ticks, plus the decaying correction. */
  pose(alpha: number, dtSeconds: number): DrivenPose | null {
    const { current } = this
    if (!this.sim || !current) return null
    const previous = this.previous ?? current
    const decay = Math.exp(-ERROR_DECAY * dtSeconds)
    this.errorPosition.multiplyScalar(decay)
    this.errorRotation.slerp(IDENTITY, 1 - decay)
    const position = new THREE.Vector3(...previous.position).lerp(new THREE.Vector3(...current.position), alpha).add(this.errorPosition)
    const rotation = new THREE.Quaternion(...previous.rotation).slerp(new THREE.Quaternion(...current.rotation), alpha)
    rotation.premultiply(this.errorRotation)
    return { position: [position.x, position.y, position.z], rotation: [rotation.x, rotation.y, rotation.z, rotation.w], steer: this.steer }
  }

  dispose() {
    this.leave()
    this.world.free()
  }
}

const IDENTITY = new THREE.Quaternion()
