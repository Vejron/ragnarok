import RAPIER from '@dimforge/rapier3d-compat'
import {
  BUTTON_JUMP, TICK_DT, JEEP_AXLE_Z, JEEP_HALF_EXTENTS, JEEP_HARD_POINT_Y, JEEP_MAX_SPEED, JEEP_MAX_STEER, JEEP_SUSPENSION_REST, JEEP_TRACK_HALF, JEEP_WHEEL_RADIUS,
  SPORTS_AXLE_Z, SPORTS_HALF_EXTENTS, SPORTS_HARD_POINT_Y, SPORTS_MAX_SPEED, SPORTS_MAX_STEER, SPORTS_SUSPENSION_REST, SPORTS_TRACK_HALF, SPORTS_WHEEL_RADIUS,
  TANK_AXLES_Z, TANK_HALF_EXTENTS, TANK_HARD_POINT_Y, TANK_MAX_SPEED, TANK_SUSPENSION_REST, TANK_TRACK_HALF, TANK_TURN_RATE,
  TANK_WHEEL_RADIUS, HELI_BANK, HELI_CEILING, HELI_CLIMB_SPEED, HELI_HALF_EXTENTS, HELI_LEVEL_GAIN, HELI_RESPONSE, HELI_SINK_SPEED, HELI_TILT,
  HELI_VERT_ACCEL, HELI_VERT_GAIN, HELI_YAW_RATE, BUTTON_RUN, vehicleMaxHp,
  PLANE_AIM_GAIN, PLANE_AIM_MAX_PITCH, PLANE_AOA_GAIN, PLANE_LEVEL_GAIN, PLANE_AXLES_Z, PLANE_BANK_ANGLE, PLANE_CEILING, PLANE_CRUISE_SPEED, PLANE_CRUISE_THROTTLE, PLANE_DRAG,
  PLANE_EDGE_MARGIN, PLANE_GLIDE_PITCH, PLANE_HALF_EXTENTS, PLANE_HARD_POINT_Y, PLANE_IDLE_THROTTLE, PLANE_LEVEL_SPEED, PLANE_MAX_BANK,
  PLANE_MAX_LIFT, PLANE_MAX_STEER, PLANE_PITCH_RATE, PLANE_RESPONSE, PLANE_ROLL_RATE, PLANE_SIDE_GAIN, PLANE_STALL_DROP, PLANE_STALL_SPEED,
  PLANE_SUSPENSION_REST, PLANE_THRUST, PLANE_TRACK_HALF, PLANE_WHEEL_RADIUS, PLANE_YAW_RATE,
} from './consts'
import { addTileCollider, buildStaticColliders, extraSolid, rebuildBuildingCollider, VehicleSolids } from './physics'
import type { TileGround } from './terrain'
import { DAMAGE_WHEELS, fromAxis, fromMrad, KIND_HELI, KIND_PLANE, KIND_SPORTS, KIND_TANK, WHEEL_GONE, type DriveState, type InputFrame, type VehicleState } from '@/net/codec'
import type { TileData, WorldData } from '@/world'
import * as THREE from 'three'

/**
 * Mirrors `crates/server/src/physics/vehicle.rs`: a dynamic body with ray-cast wheels, driven the
 * same way, so the driver's own vehicle can be predicted.
 */

/** Gravity on vehicle bodies. */
export const CAR_GRAVITY = 9.81

/** What holds a vehicle up: its wheels; a rotor, with no wheels; or wheels on the ground and wings in the air. */
type Lift = 'wheels' | 'rotor' | 'wing'

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
  lift: Lift
  linearDamping: number
  /** How far below the box's centre the centre of mass sits, metres; zero leaves the box's own. */
  comDrop: number
}

const JEEP: Spec = {
  halfExtents: JEEP_HALF_EXTENTS,
  density: 200,
  wheelRadius: JEEP_WHEEL_RADIUS,
  trackHalf: JEEP_TRACK_HALF,
  axles: [-JEEP_AXLE_Z, JEEP_AXLE_Z],
  hardPointY: JEEP_HARD_POINT_Y,
  suspensionRest: JEEP_SUSPENSION_REST,
  maxSuspensionForce: 60_000,
  engineForce: 10_000,
  maxSpeed: JEEP_MAX_SPEED,
  steerLock: JEEP_MAX_STEER,
  turnRate: 0,
  lift: 'wheels',
  linearDamping: 0.05,
  comDrop: 0.15,
}

const SPORTS: Spec = {
  halfExtents: SPORTS_HALF_EXTENTS,
  density: 200,
  wheelRadius: SPORTS_WHEEL_RADIUS,
  trackHalf: SPORTS_TRACK_HALF,
  axles: [-SPORTS_AXLE_Z, SPORTS_AXLE_Z],
  hardPointY: SPORTS_HARD_POINT_Y,
  suspensionRest: SPORTS_SUSPENSION_REST,
  maxSuspensionForce: 60_000,
  engineForce: 15_000,
  maxSpeed: SPORTS_MAX_SPEED,
  steerLock: SPORTS_MAX_STEER,
  turnRate: 0,
  lift: 'wheels',
  linearDamping: 0.05,
  comDrop: 0.02,
}

const TANK: Spec = {
  halfExtents: TANK_HALF_EXTENTS,
  density: 1000,
  wheelRadius: TANK_WHEEL_RADIUS,
  trackHalf: TANK_TRACK_HALF,
  axles: TANK_AXLES_Z,
  hardPointY: TANK_HARD_POINT_Y,
  suspensionRest: TANK_SUSPENSION_REST,
  maxSuspensionForce: 800_000,
  engineForce: 88_000,
  maxSpeed: TANK_MAX_SPEED,
  steerLock: 0,
  turnRate: TANK_TURN_RATE,
  lift: 'wheels',
  linearDamping: 0.05,
  comDrop: 0,
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
  lift: 'rotor',
  linearDamping: 0.2,
  comDrop: 0,
}

const PLANE: Spec = {
  halfExtents: PLANE_HALF_EXTENTS,
  density: 90,
  wheelRadius: PLANE_WHEEL_RADIUS,
  trackHalf: PLANE_TRACK_HALF,
  axles: PLANE_AXLES_Z,
  hardPointY: PLANE_HARD_POINT_Y,
  suspensionRest: PLANE_SUSPENSION_REST,
  maxSuspensionForce: 30_000,
  engineForce: 0,
  maxSpeed: 0,
  steerLock: PLANE_MAX_STEER,
  turnRate: 0,
  lift: 'wing',
  linearDamping: 0,
  comDrop: 0,
}

const TURN_ACCEL = 3.0
const BRAKE_PER_KG = 0.045
const COAST_BRAKE_PER_KG = 0.0015
const STEER_RATE = 7.0
const STEER_RETURN = 1.5
const STEER_FADE_SPEED = 20.0
const FULL_POWER = 0.65
const DRIFT_GRIP = 0.05
const DOWNFORCE_PER_KG = 0.0001
const AIR_PITCH = 3.0
const AIR_ROLL = 3.0
const SIDE_GRIP = 1.0
const FRICTION_SLIP = 12.0
const TOE_MAX = 0.12
const WHEEL_GRIP_LOSS = 0.85
const WHEEL_SLIP_LOSS = 0.6
const LOST_WHEEL_RADIUS = 0.05

/** Mirrors `CarCondition`: engine power and the damage of a car's four wheels (255 is gone). */
export interface CarCondition {
  power: number
  wheels: readonly number[]
}

export const NEW_CONDITION: CarCondition = { power: 1, wheels: [0, 0, 0, 0] }

/** Mirrors `CarCondition::of`: full power down to half health, and up to half of it lost below. */
export function conditionOf(kind: number, hp: number, damage: readonly number[]): CarCondition {
  const health = Math.fround(hp / vehicleMaxHp(kind))
  return { power: Math.min(1, Math.fround(0.5 + health)), wheels: damage.slice(DAMAGE_WHEELS, DAMAGE_WHEELS + 4) }
}

export interface CarControls {
  /** -1 (reverse) ..= 1 (forward). */
  throttle: number
  /** -1 (left) ..= 1 (right). */
  steer: number
  handbrake: boolean
  /** -1 (sink) ..= 1 (climb). Only a helicopter has it. */
  lift: number
  /** A pilot is aboard and the rotor or propeller turns. Only a helicopter and a plane have it. */
  powered: boolean
  /** Where a plane's pilot points the mouse, `[yaw, pitch]`: it turns towards it. Null flies straight on. */
  aim?: [number, number] | null
}

export interface CarBodyState {
  position: [number, number, number]
  /** Unit quaternion `[x, y, z, w]`. */
  rotation: [number, number, number, number]
  linvel: [number, number, number]
  angvel: [number, number, number]
}

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value))

/** Mirrors `pull`: share of full engine force at `speed` for a top speed of `max`. */
const pull = (speed: number, max: number) => clamp((max - Math.abs(speed)) / (max * (1 - FULL_POWER)), 0, 1)

/** Mirrors the chassis mass properties of `add_car`: the box's own, in f32 like the server's, with the centre of mass lowered. */
function lowered(desc: RAPIER.ColliderDesc, spec: Spec): RAPIER.ColliderDesc {
  const f = Math.fround
  const [hx, hy, hz] = spec.halfExtents.map(f) as [number, number, number]
  const mass = f(f(f(f(f(spec.density) * 8) * hx) * hy) * hz)
  const third = f(mass / 3)
  const inertia = { x: f(f(f(hy * hy) + f(hz * hz)) * third), y: f(f(f(hx * hx) + f(hz * hz)) * third), z: f(f(f(hx * hx) + f(hy * hy)) * third) }
  return desc.setMassProperties(mass, { x: 0, y: -spec.comDrop, z: 0 }, inertia, { x: 0, y: 0, z: 0, w: 1 })
}

/** One vehicle's body and wheels in a Rapier world. */
export class CarSim {
  private readonly spec: Spec
  private readonly body: RAPIER.RigidBody
  private readonly collider: RAPIER.Collider
  private readonly controller: RAPIER.DynamicRayCastVehicleController | null
  private readonly wheels: number
  /** What a helicopter or a plane was last asked to do, applied in the next `step`. */
  private flight: CarControls = { throttle: 0, steer: 0, handbrake: false, lift: 0, powered: false }
  /** Front wheel angle, radians, + is right. */
  steer = 0
  private turn = 0
  private spin = 0
  /** Throttle and steering of a car, which tilt it in the air. */
  private air: [number, number] = [0, 0]
  /** How worn it is, from the last snapshot. */
  condition: CarCondition = NEW_CONDITION

  /**
   * `bound` is how far the playable square reaches from the centre to each side, metres: a plane turns
   * back near it, as on the server.
   */
  constructor(private readonly world: RAPIER.World, kind: number, position: [number, number, number], yaw: number, private readonly bound = Infinity) {
    this.spec = kind === KIND_TANK ? TANK : kind === KIND_HELI ? HELI : kind === KIND_SPORTS ? SPORTS : kind === KIND_PLANE ? PLANE : JEEP
    const spec = this.spec
    this.body = world.createRigidBody(
      RAPIER.RigidBodyDesc.dynamic()
        .setTranslation(position[0], position[1], position[2])
        .setRotation({ x: 0, y: Math.sin(yaw / 2), z: 0, w: Math.cos(yaw / 2) })
        .setLinearDamping(spec.linearDamping)
        .setAngularDamping(0.8),
    )
    const [hx, hy, hz] = spec.halfExtents
    const chassis = RAPIER.ColliderDesc.cuboid(hx, hy, hz).setFriction(0.4)
    this.collider = world.createCollider(spec.comDrop > 0 ? lowered(chassis, spec) : chassis.setDensity(spec.density), this.body)
    // A tank's turret and a plane's wings are solid but weightless, like the server's.
    const extra = extraSolid(kind)
    if (extra) {
      const [tx, ty, tz] = extra.offset
      world.createCollider(extra.desc.setTranslation(tx, ty, tz).setDensity(0).setFriction(0.4), this.body)
    }
    if (spec.lift === 'rotor') {
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
      controller.setWheelSideFrictionStiffness(i, SIDE_GRIP)
      controller.setWheelFrictionSlip(i, FRICTION_SLIP)
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
    if (spec.lift === 'wing') {
      this.flight = controls
      this.rollPlane(controller, controls, speed, mass, dt)
      return
    }

    const lock = spec.steerLock / (1 + Math.abs(speed) / STEER_FADE_SPEED)
    const steerInput = clamp(controls.steer, -1, 1)
    const target = steerInput * lock
    const returning = Math.abs(target) < Math.abs(this.steer) || target * this.steer < 0
    const step = STEER_RATE * (returning ? STEER_RETURN : 1) * dt
    this.steer += clamp(target - this.steer, -step, step)
    this.turn = steerInput * spec.turnRate
    this.air = [clamp(controls.throttle, -1, 1), steerInput]

    const against = controls.throttle * speed < -1
    let engine = 0
    let brake = 0
    if (controls.throttle === 0) brake = COAST_BRAKE_PER_KG * mass
    else if (against) brake = BRAKE_PER_KG * mass
    else {
      const headroom = controls.throttle > 0 ? pull(speed, spec.maxSpeed) : 0.5
      engine = ((controls.throttle * spec.engineForce) / this.wheels) * headroom * this.condition.power
    }
    const handbrake = controls.handbrake ? BRAKE_PER_KG * mass : 0
    const frontWheels = spec.steerLock > 0 ? 2 : 0
    const drift = controls.handbrake && frontWheels > 0
    for (let i = 0; i < this.wheels; i++) {
      const front = i < frontWheels
      const hurt = frontWheels > 0 ? (this.condition.wheels[i] ?? 0) : 0
      const wear = Math.fround(hurt / WHEEL_GONE)
      const gone = hurt === WHEEL_GONE
      const toe = (i % 2 === 0 ? TOE_MAX : -TOE_MAX) * wear
      controller.setWheelSteering(i, (front ? -this.steer : 0) + toe)
      controller.setWheelEngineForce(i, gone ? 0 : engine)
      controller.setWheelBrake(i, gone ? 0 : front ? brake : Math.max(brake, handbrake))
      controller.setWheelSideFrictionStiffness(i, (drift && !front ? DRIFT_GRIP : SIDE_GRIP) * (1 - WHEEL_GRIP_LOSS * wear))
      controller.setWheelFrictionSlip(i, FRICTION_SLIP * (1 - WHEEL_SLIP_LOSS * wear))
      controller.setWheelRadius(i, gone ? LOST_WHEEL_RADIUS : spec.wheelRadius)
      controller.setWheelSuspensionRestLength(i, gone ? 0 : spec.suspensionRest)
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
    if (this.spec.lift === 'wing') {
      this.body.resetForces(false)
      this.wing(this.flight, grounded, dt)
      this.world.step()
      return
    }
    if (this.spec.steerLock > 0) this.assist(grounded, dt)
    if (this.turn === 0 || !grounded) this.spin = 0
    else {
      this.spin += clamp(this.turn - this.spin, -TURN_ACCEL * dt, TURN_ACCEL * dt)
      const w = this.body.angvel()
      this.body.setAngvel({ x: w.x, y: -this.spin, z: w.z }, true)
    }
    this.world.step()
  }

  /** Mirrors `assist` in `physics/vehicle.rs`: downforce with a wheel down, pitch and roll from the controls in the air. */
  private assist(grounded: boolean, dt: number) {
    const body = this.body
    const q = body.rotation()
    if (grounded) {
      const v = body.linvel()
      const push = DOWNFORCE_PER_KG * body.mass() * (v.x * v.x + v.y * v.y + v.z * v.z) * dt
      const [x, y, z] = rotate(q, 0, -push, 0)
      body.applyImpulse({ x, y, z }, true)
    } else {
      const [throttle, steer] = this.air
      const [x, y, z] = rotate(q, -throttle * AIR_PITCH * dt, 0, -steer * AIR_ROLL * dt)
      const w = body.angvel()
      body.setAngvel({ x: w.x + x, y: w.y + y, z: w.z + z }, true)
    }
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

  /** Mirrors `roll_plane`: no engine on the wheels, the front pair steered by A/D, brakes on Shift or with nobody aboard. */
  private rollPlane(controller: RAPIER.DynamicRayCastVehicleController, controls: CarControls, speed: number, mass: number, dt: number) {
    const wanted = controls.powered ? clamp(controls.steer, -1, 1) : 0
    const lock = this.spec.steerLock / (1 + Math.abs(speed) / STEER_FADE_SPEED)
    const target = wanted * lock
    const returning = Math.abs(target) < Math.abs(this.steer) || target * this.steer < 0
    const step = STEER_RATE * (returning ? STEER_RETURN : 1) * dt
    this.steer += clamp(target - this.steer, -step, step)
    const braking = !controls.powered || controls.lift < -0.5
    const brake = braking ? 0.5 * BRAKE_PER_KG * mass : COAST_BRAKE_PER_KG * mass
    for (let i = 0; i < this.wheels; i++) {
      controller.setWheelSteering(i, i < 2 ? -this.steer : 0)
      controller.setWheelEngineForce(i, 0)
      controller.setWheelBrake(i, brake)
    }
  }

  /** Mirrors `wing` in `physics/vehicle.rs`: propeller, wings, fin, drag, and the throttle and stick that fly it. */
  private wing(controls: CarControls, grounded: boolean, dt: number) {
    const body = this.body
    const q = body.rotation()
    const nose = rotate(q, 0, 0, -1)
    const up = rotate(q, 0, 1, 0)
    const right = rotate(q, 1, 0, 0)
    const lv = body.linvel()
    const v: Vec = [lv.x, lv.y, lv.z]
    const position = body.translation()
    const airspeed = Math.max(0, dot(v, nose))
    const speed = Math.hypot(v[0], v[1], v[2])

    const flying = clamp((airspeed - 0.7 * PLANE_STALL_SPEED) / (0.3 * PLANE_STALL_SPEED), 0, 1)
    const carried = grounded ? 1 : Math.min(1, (airspeed / PLANE_LEVEL_SPEED) ** 2)
    const lift = clamp(flying * (CAR_GRAVITY * carried - PLANE_AOA_GAIN * dot(v, up)), -PLANE_MAX_LIFT, PLANE_MAX_LIFT)
    const side = -PLANE_SIDE_GAIN * dot(v, right) * Math.min(1, airspeed / PLANE_STALL_SPEED)
    const headroom = clamp((PLANE_CEILING - position.y) / 20, 0, 1)
    const thrust = controls.powered ? throttleSetting(controls.lift, grounded) * PLANE_THRUST * this.condition.power * headroom : 0
    const drag = speed * PLANE_DRAG
    const mass = body.mass()
    const accel = [0, 1, 2].map((i) => nose[i]! * thrust + up[i]! * lift + right[i]! * side - v[i]! * drag)
    body.addForce({ x: accel[0]! * mass, y: accel[1]! * mass, z: accel[2]! * mass }, true)

    const stalled: Vec = grounded || speed < 2 ? [0, 0, 0] : scale(cross(nose, [v[0] / speed, v[1] / speed, v[2] / speed]), PLANE_STALL_DROP * (1 - flying))
    const w = body.angvel()
    const current: Vec = [w.x, w.y, w.z]
    let wanted = current
    if (controls.powered || !grounded) {
      const heading = Math.atan2(-nose[0], -nose[2])
      const near = edgeDepth([position.x, position.y, position.z], this.bound)
      const hold = aimDirection([heading, controls.powered ? 0 : PLANE_GLIDE_PITCH], [position.x, position.y, position.z], this.bound)
      const auto = autopilot(q, hold, airspeed)
      let command = auto
      if (controls.powered) {
        const bank = Math.atan2(-right[1], up[1])
        const steer = clamp(controls.steer, -1, 1)
        const pull = clamp(-controls.throttle, -1, 1)
        // With the mouse the plane flies itself towards where it points; the keys take an axis over while held.
        const mouse = controls.aim
          ? autopilot(q, aimDirection([controls.aim[0], clamp(controls.aim[1], -PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)], [position.x, position.y, position.z], this.bound), airspeed)
          : null
        const levelPitch = clamp(-Math.asin(clamp(nose[1], -1, 1)) * PLANE_LEVEL_GAIN, -1, 1)
        const pitch = Math.abs(pull) > 0.05 ? pull : mouse ? mouse[0] : levelPitch
        let roll = Math.abs(steer) > 0.05 ? steer : mouse ? mouse[2] : clamp(-bank * 2, -1, 1)
        roll = clamp(clamp(roll, (-PLANE_MAX_BANK - bank) * 3, (PLANE_MAX_BANK - bank) * 3), -1, 1)
        const yaw = mouse && Math.abs(steer) <= 0.05 ? mouse[1] : clamp(steer * 0.3 + Math.sin(bank) * 0.8, -1, 1)
        const own = [pitch, yaw, roll]
        // The autopilot takes over completely by a third of the way into the edge margin.
        const taken = Math.min(1, near * 3)
        command = own.map((c, i) => c * (1 - taken) + auto[i]! * taken) as Vec
      }
      const authority = Math.min(1.3, (airspeed / PLANE_CRUISE_SPEED) ** 2)
      const turn = rotate(q, command[0] * PLANE_PITCH_RATE * authority, -command[1] * PLANE_YAW_RATE * authority, -command[2] * PLANE_ROLL_RATE * authority)
      wanted = [turn[0] + stalled[0], turn[1] + stalled[1], turn[2] + stalled[2]]
    }
    const response = Math.min(1, PLANE_RESPONSE * dt) * (grounded ? flying : 1)
    body.setAngvel({ x: current[0] + (wanted[0] - current[0]) * response, y: current[1] + (wanted[1] - current[1]) * response, z: current[2] + (wanted[2] - current[2]) * response }, true)
  }

  state(): CarBodyState {
    const t = this.body.translation()
    const r = this.body.rotation()
    const v = this.body.linvel()
    const w = this.body.angvel()
    return { position: [t.x, t.y, t.z], rotation: [r.x, r.y, r.z, r.w], linvel: [v.x, v.y, v.z], angvel: [w.x, w.y, w.z] }
  }
}

type Vec = [number, number, number]
const dot = (a: readonly number[], b: readonly number[]) => a[0]! * b[0]! + a[1]! * b[1]! + a[2]! * b[2]!
const cross = (a: readonly number[], b: readonly number[]): Vec => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!]
const scale = (a: readonly number[], k: number): Vec => [a[0]! * k, a[1]! * k, a[2]! * k]

/** Mirrors `throttle_setting`: cruise in the air with the stick released, idle on the ground. */
export function throttleSetting(throttle: number, grounded: boolean): number {
  const t = clamp(throttle, -1, 1)
  if (t > 0) return PLANE_CRUISE_THROTTLE + t * (1 - PLANE_CRUISE_THROTTLE)
  if (t < 0) return PLANE_CRUISE_THROTTLE + t * (PLANE_CRUISE_THROTTLE - PLANE_IDLE_THROTTLE)
  return grounded ? PLANE_IDLE_THROTTLE : PLANE_CRUISE_THROTTLE
}

/** Mirrors `edge_depth`: how deep into the edge margin a position is, 0 outside it to 1 at the wall. */
function edgeDepth(position: readonly number[], bound: number): number {
  const into = (c: number) => clamp((Math.abs(c) - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN, 0, 1)
  return Math.max(into(position[0]!), into(position[2]!))
}

/** Mirrors `autopilot`: `[pitch, yaw, roll]` that would turn a plane towards the world direction `target`. */
function autopilot(q: { x: number; y: number; z: number; w: number }, target: Vec, airspeed: number): Vec {
  const up = rotate(q, 0, 1, 0)
  const right = rotate(q, 1, 0, 0)
  const local = rotate({ x: -q.x, y: -q.y, z: -q.z, w: q.w }, target[0], target[1], target[2])
  const off = Math.acos(clamp(-local[2], -1, 1))
  const lateral = Math.hypot(local[0], local[1])
  let [across, above] = lateral > 1e-3 ? [local[0] / lateral, local[1] / lateral] : [1, 0]
  const bank = Math.atan2(-right[1], up[1])
  if (off > Math.PI / 2) {
    const lean = Math.abs(across) < 0.5 && Math.abs(bank) > 0.2 ? bank : across
    const side = lean < 0 ? -1 : 1
    ;[across, above] = [side * Math.cos(bank), Math.abs(Math.sin(bank))]
  }
  let pitch = clamp(above * off * PLANE_AIM_GAIN, -1, 1)
  if (pitch > 0) {
    const banked = off > 1.2 ? clamp(Math.abs(bank) / PLANE_MAX_BANK, 0.25, 1) : 1
    const margin = clamp((airspeed - PLANE_STALL_SPEED) / (0.4 * PLANE_STALL_SPEED), 0.2, 1)
    pitch *= banked * margin
  }
  const yaw = clamp(across * off * PLANE_AIM_GAIN, -1, 1)
  const level = clamp(-bank * 2, -1, 1)
  const into = clamp(across * off * 3, -1, 1)
  const blend = Math.min(1, off / PLANE_BANK_ANGLE)
  let roll = level + (into - level) * blend
  roll = clamp(clamp(roll, (-PLANE_MAX_BANK - bank) * 3, (PLANE_MAX_BANK - bank) * 3), -1, 1)
  return [pitch, yaw, roll]
}

/** Mirrors `aim_direction`: where a pilot looking at `[yaw, pitch]` wants to fly, turned back near the edge of the world. */
export function aimDirection([yaw, pitch]: readonly [number, number], position: readonly number[], bound: number): Vec {
  const direction: Vec = [-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)]
  const into = (c: number) => clamp((Math.abs(c) - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN, 0, 1) * Math.sign(c)
  const [x, z] = [into(position[0]!), into(position[2]!)]
  const near = Math.max(Math.abs(x), Math.abs(z))
  if (near === 0) return direction
  const length = Math.hypot(x, z)
  const inward: Vec = [-x / length, 0, -z / length]
  const blended: Vec = [0, 1, 2].map((i) => direction[i]! * (1 - near) + inward[i]! * near) as Vec
  const size = Math.hypot(...blended)
  return size > 1e-3 ? scale(blended, 1 / size) : inward
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
    // Whoever flies a plane turns it towards where they look; only a plane reads it.
    aim: [fromMrad(input.yaw), fromMrad(input.pitch)],
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
  private readonly obstacles = new Map<number, VehicleSolids>()
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

  private constructor(world: RAPIER.World, private readonly bound: number) {
    this.world = world
  }

  static async create(data: WorldData): Promise<VehicleDriver> {
    await RAPIER.init()
    const world = new RAPIER.World({ x: 0, y: -CAR_GRAVITY, z: 0 })
    world.timestep = TICK_DT
    buildStaticColliders(world, data)
    world.step()
    return new VehicleDriver(world, data.halfExtent)
  }

  /** Adds a streamed tile's collision; returns what to pass to `removeTile` when it unloads. */
  addTile(tile: TileData, ground: TileGround | null = null): RAPIER.Collider[] {
    const colliders = addTileCollider(this.world, tile, ground)
    // Stepping with nothing simulated only rebuilds the broad-phase. While a vehicle is predicted
    // the next tick's step does it, and stepping here would advance that vehicle.
    if (colliders.length > 0 && !this.sim) this.world.step()
    return colliders
  }

  /** Colliders already taken out one by one (a felled tree's trunk) are `undefined`. */
  removeTile(colliders: readonly (RAPIER.Collider | undefined)[]) {
    for (const collider of colliders) if (collider) this.world.removeCollider(collider, false)
  }

  /** As `RapierMotor.rebuildBuildings`. */
  rebuildBuildings(old: RAPIER.Collider | undefined, tile: TileData, destroyed: ReadonlySet<number>): RAPIER.Collider | undefined {
    if (old) this.world.removeCollider(old, false)
    const next = rebuildBuildingCollider(this.world, tile, destroyed) ?? undefined
    if (next && !this.sim) this.world.step()
    return next
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
      let solids = this.obstacles.get(vehicle.id)
      if (!solids) {
        solids = new VehicleSolids(this.world, vehicle.kind)
        this.obstacles.set(vehicle.id, solids)
      }
      solids.place(vehicle)
    }
    for (const [id, solids] of this.obstacles) {
      if (!alive.has(id)) {
        solids.remove()
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
      const solids = this.obstacles.get(vehicle.id)
      if (solids) {
        solids.remove()
        this.obstacles.delete(vehicle.id)
      }
      this.sim = new CarSim(this.world, vehicle.kind, [vehicle.x, vehicle.y, vehicle.z], 0, this.bound)
      this.id = vehicle.id
      this.kind = vehicle.kind
    }
    const sim = this.sim
    const before = this.current
    sim.condition = conditionOf(vehicle.kind, vehicle.hp, vehicle.damage)
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
