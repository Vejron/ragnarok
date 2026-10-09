import * as THREE from 'three'
import { encodeInput, encodePing, KIND_STICKMAN, KIND_TANK, toAxis, toMrad, type EntityState, type InputFrame, type ServerMessage, type Turret, type VehicleState } from './net/codec'
import { blendEntity, blendVehicle, Interpolator } from './net/interp'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport, type ConnectionStatus } from './net/transport'
import {
  ENTER_RADIUS, EXPLOSION_RADIUS, INPUT_REDUNDANCY, PROTOCOL_VERSION, SHELL_COOLDOWN_TICKS, SHELL_RANGE, STANDING_Y, TANK_BARREL_LENGTH,
  TANK_BARREL_MAX, TANK_BARREL_MIN, TICK_MS,
} from './sim/consts'
import { InputController } from './sim/input'
import { RapierMotor } from './sim/physics'
import { Predictor } from './sim/predict'
import type { Vec3 } from './sim/physics'
import { CarModel, type VehicleModel } from './render/car'
import { Effects } from './render/effects'
import { DRIVING_DISTANCE, ON_FOOT_DISTANCE, Scene, TANK_DISTANCE } from './render/scene'
import { TankModel } from './render/tank'
import { Stickman } from './render/stickman'
import { buildWorld } from './render/world'
import { loadWorld } from './world'

export interface HudStats {
  status: ConnectionStatus
  rtt: number | null
  players: number
  interpDelayMs: number
  correction: number
  pending: number
  locked: boolean
  warning: string | null
  /** A hint for the key that does something right now, or null. */
  prompt: string | null
  /** Set while in a vehicle: the seat's role. */
  seat: 'driver' | 'passenger' | 'gunner' | null
  /** For a gunner: how loaded the gun is, 0 (just fired) to 1 (ready). */
  reload: number
}

interface CarPlacement {
  kind: number
  turret: Turret | null
  x: number
  y: number
  z: number
  /** Heading of the nose, stickman convention. */
  yaw: number
  seats: readonly number[]
}

/** Players sitting in cars, which the stickman list does not include. */
const riders = (cars: ReadonlyMap<number, CarPlacement>) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
const RECONNECT_MS = 3000

/**
 * The client game loop. Fixed 60 Hz ticks sample input, predict the local player and send inputs;
 * every animation frame renders the predicted local player and interpolated remote players.
 */
export class Game {
  private readonly scene: Scene
  private readonly input: InputController
  private readonly transport: Transport
  private readonly clock = new ServerClock()
  private readonly rtt = new RttEstimator()
  private readonly interp = new Interpolator<EntityState>(blendEntity)
  private readonly vehicleInterp = new Interpolator<VehicleState>(blendVehicle)
  private readonly avatars = new Map<number, Stickman>()
  private readonly cars = new Map<number, VehicleModel>()
  private readonly effects = new Effects()
  /** The last shot counter seen for each gun, so a change is a new shell. */
  private readonly shots = new Map<number, number>()
  private lastShotAt = -Infinity
  /** The vehicle seat the server says the local player sits in; null when on foot. */
  private seat: { vehicle: number; kind: number; index: number } | null = null
  private wasGunner = false
  private predictor: Predictor | null = null
  private motor: RapierMotor | null = null
  /** Vehicle poses of the newest snapshot, which the predicted stickman's frame is fixed to while riding. */
  private readonly refPoses = new Map<number, VehicleState>()
  private localId: number | null = null
  private worldHash = 0
  private seq = 0
  private recent: InputFrame[] = []
  private accumulator = 0
  private lastFrame = 0
  private frameHandle = 0
  private reconnectTimer = 0
  private pingTimer = 0
  private disposed = false
  /** Set when reconnecting cannot help (protocol mismatch); only a reload can. */
  private halted = false
  private warning: string | null = null

  constructor(container: HTMLElement, private readonly onStats: (stats: HudStats) => void) {
    this.scene = new Scene(container)
    this.input = new InputController(this.scene.canvas)
    this.scene.scene.add(this.effects.root)
    this.transport = new Transport(
      (message) => this.handle(message),
      (status) => {
        if (status === 'OFFLINE') this.onDisconnect()
      },
    )
    // Dev-only handle for poking at the game from the console or a test driver.
    if (import.meta.env.DEV) (window as unknown as { __game?: Game }).__game = this
  }

  async start() {
    const { world, hash } = await loadWorld()
    this.worldHash = hash
    this.scene.scene.add(buildWorld(world))
    const motor = await RapierMotor.create(world)
    this.motor = motor
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    this.predictor = new Predictor(motor, { position: [x, STANDING_Y, z], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0 })
    this.avatar(0)
    if (this.disposed) return
    this.lastFrame = performance.now()
    this.frameHandle = requestAnimationFrame((now) => this.frame(now))
    // A timer rather than the frame loop, so a background tab (no animation frames) still pings
    // and the server does not drop it as idle.
    this.pingTimer = window.setInterval(() => {
      if (this.localId !== null) this.transport.send(encodePing(this.rtt.next(performance.now())))
    }, PING_INTERVAL_MS)
    void this.connect()
  }

  private async connect() {
    if (this.disposed) return
    try {
      await this.transport.connect()
    } catch (error) {
      console.warn('connect failed', error)
      this.transport.close()
    }
  }

  private onDisconnect() {
    this.localId = null
    this.seat = null
    this.interp.clear()
    this.vehicleInterp.clear()
    this.motor?.setVehicles([])
    this.clock.reset()
    for (const [id, avatar] of this.avatars) {
      if (id !== 0) {
        avatar.dispose()
        this.avatars.delete(id)
      }
    }
    if (!this.disposed && !this.halted) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = window.setTimeout(() => void this.connect(), RECONNECT_MS)
    }
  }

  private handle(message: ServerMessage) {
    const now = performance.now()
    switch (message.type) {
      case 'welcome':
        if (message.version !== PROTOCOL_VERSION) {
          this.warning = `Server speaks protocol ${message.version}, this page ${PROTOCOL_VERSION}. Reload.`
          this.halted = true
          this.transport.close()
          return
        }
        if (message.worldHash !== this.worldHash) this.warning = 'Server world differs from this page’s world.msgpack. Reload.'
        this.localId = message.id
        // Recreate the local stickman in this player's colour.
        this.avatars.get(0)?.dispose()
        this.avatars.delete(0)
        // Inputs made before joining were never sent; they must not be replayed on server state.
        this.predictor?.clearPending()
        this.recent = []
        break
      case 'snapshot': {
        if (this.localId === null) return
        this.clock.observe(message.tick, now)
        const remote = message.entities.filter((e) => e.id !== this.localId)
        this.interp.push(message.tick, remote)
        this.vehicleInterp.push(message.tick, message.vehicles)
        // Cars are solid to the predicted stickman, at the pose of the newest snapshot.
        this.refPoses.clear()
        this.motor?.setVehicles(
          message.vehicles.map((vehicle) => {
            this.refPoses.set(vehicle.id, vehicle)
            const [vx, vz] = this.vehicleInterp.latestVelocity(vehicle.id) ?? [0, 0]
            return { ...vehicle, vx, vz }
          }),
        )
        // Seated players are not listed as entities; whatever track they left behind is stale.
        const mount = message.vehicles.find((vehicle) => vehicle.seats.includes(this.localId!))
        this.seat = mount ? { vehicle: mount.id, kind: mount.kind, index: mount.seats.indexOf(this.localId!) } : null
        for (const vehicle of message.vehicles) for (const rider of vehicle.seats) if (rider !== 0) this.interp.remove(rider)
        const own = message.entities.find((e) => e.id === this.localId)
        if (this.seat) {
          // The stickman is parked inside the car. Inputs still go out; the server just applies them to the car.
          this.predictor?.acknowledge(message.ack)
        } else if (own && this.predictor) {
          this.predictor.reconcile(message.ack, { position: [own.x, own.y, own.z], yaw: own.yaw, vy: own.vy, grounded: own.grounded, knock: [own.knockX, own.knockZ], ride: message.ride })
        }
        break
      }
      case 'pong':
        this.rtt.pong(message.nonce, now)
        break
      case 'leave':
        this.interp.remove(message.id)
        break
    }
  }

  private tick() {
    const sample = this.input.sample()
    const frame: InputFrame = {
      seq: ++this.seq,
      strafe: toAxis(sample.strafe),
      forward: toAxis(sample.forward),
      yaw: toMrad(sample.yaw),
      buttons: sample.buttons,
      pitch: toMrad(sample.pitch),
    }
    // In a car the stickman does not walk; the frame is kept for replay in case it steps out.
    if (this.seat) this.predictor!.record(frame)
    else this.predictor!.apply(frame)
    if (this.localId === null) return
    this.recent.push(frame)
    if (this.recent.length > INPUT_REDUNDANCY) this.recent.shift()
    this.transport.send(encodeInput(this.recent))
  }

  private frame(now: number) {
    if (this.disposed) return
    this.frameHandle = requestAnimationFrame((t) => this.frame(t))
    const dt = Math.min(0.25, (now - this.lastFrame) / 1000)
    this.lastFrame = now

    this.accumulator += dt * 1000
    let ticks = 0
    while (this.accumulator >= TICK_MS && ticks < MAX_TICKS_PER_FRAME) {
      this.tick()
      this.accumulator -= TICK_MS
      ticks++
    }
    if (ticks === MAX_TICKS_PER_FRAME) this.accumulator = 0

    const predictor = this.predictor!
    const renderTick = this.clock.tickAt(now) - this.clock.interpDelay
    const seated = this.seat !== null
    const cars = this.updateCars(renderTick)

    const position = this.ridden(predictor.renderPosition(this.accumulator / TICK_MS, dt), predictor.state.ride, renderTick)
    const local = predictor.state
    const localSpeed = Math.hypot(local.position[0] - predictor.previous.position[0], local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)
    const me = this.avatar(0)
    me.hidden = seated
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    me.update(position, this.input.yaw, localSpeed, local.grounded, dt)
    const ride = seated ? cars.get(this.seat!.vehicle) : undefined
    const gunner = ride !== undefined && ride.kind === KIND_TANK && this.seat!.index === 1
    const turret = gunner ? this.cars.get(this.seat!.vehicle) : undefined
    if (gunner && turret instanceof TankModel) {
      if (!this.wasGunner && ride.turret) {
        // Take the seat looking where the barrel points.
        this.input.yaw = ride.yaw + ride.turret.yaw
        this.input.pitch = ride.turret.pitch
      }
      // The camera sits behind and above the turret, looking along the barrel's line.
      this.input.clampPitch(TANK_BARREL_MIN, TANK_BARREL_MAX)
      const { origin } = turret.barrel()
      this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch)
    } else if (ride) {
      this.input.followYaw(ride.yaw, dt)
      this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, ride.kind === KIND_TANK ? TANK_DISTANCE : DRIVING_DISTANCE)
    } else {
      this.scene.follow(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE)
    }

    this.wasGunner = gunner
    this.effects.update(dt)

    const seen = new Set<number>([0])
    for (const id of this.interp.ids()) {
      const state = this.interp.sample(id, renderTick)
      if (!state || state.kind !== KIND_STICKMAN) continue
      seen.add(id)
      this.avatar(id).update([state.x, state.y, state.z], state.yaw, state.speed, state.grounded, dt)
    }
    for (const [id, avatar] of this.avatars) {
      if (!seen.has(id)) {
        avatar.dispose()
        this.avatars.delete(id)
      }
    }

    this.scene.render()
    this.onStats({
      status: this.transport.status,
      rtt: this.rtt.rtt,
      players: this.localId === null ? 1 : seen.size + riders(cars) - (seated ? 1 : 0),
      interpDelayMs: (this.clock.interpDelay * TICK_MS),
      correction: predictor.lastCorrection,
      pending: predictor.pendingCount,
      locked: this.input.locked,
      warning: this.warning,
      prompt: this.prompt(position, cars),
      seat: !this.seat ? null : this.seat.index === 0 ? 'driver' : this.seat.kind === KIND_TANK ? 'gunner' : 'passenger',
      reload: Math.min(1, (now - this.lastShotAt) / (SHELL_COOLDOWN_TICKS * TICK_MS)),
    })
  }

  /**
   * While standing on a car, the stickman is predicted as if the car stood still where the newest
   * snapshot left it. This carries that position along to wherever the car is drawn now, so the
   * stickman stays glued to the roof and still moves on it.
   */
  private ridden(position: Vec3, ride: number, renderTick: number): Vec3 {
    const ref = this.refPoses.get(ride)
    const now = ride === 0 ? null : this.vehicleInterp.sample(ride, renderTick)
    if (!ref || !now) return position
    const local = new THREE.Vector3(position[0] - ref.x, position[1] - ref.y, position[2] - ref.z)
    local.applyQuaternion(new THREE.Quaternion(ref.qx, ref.qy, ref.qz, ref.qw).invert())
    local.applyQuaternion(new THREE.Quaternion(now.qx, now.qy, now.qz, now.qw))
    return [now.x + local.x, now.y + local.y, now.z + local.z]
  }

  /** Draws every vehicle at the render tick and returns where they are, by id. */
  private updateCars(renderTick: number) {
    const placed = new Map<number, CarPlacement>()
    for (const id of this.vehicleInterp.ids()) {
      const state = this.vehicleInterp.sample(id, renderTick)
      if (!state) continue
      let car = this.cars.get(id)
      if (!car) {
        car = state.kind === KIND_TANK ? new TankModel(id) : new CarModel(id)
        this.cars.set(id, car)
        this.scene.scene.add(car.root)
      }
      const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(new THREE.Quaternion(state.qx, state.qy, state.qz, state.qw))
      const yaw = Math.atan2(-forward.x, -forward.z)
      car.update([state.x, state.y, state.z], [state.qx, state.qy, state.qz, state.qw], state.steer, state.seats, state.turret)
      placed.set(id, { kind: state.kind, turret: state.turret, x: state.x, y: state.y, z: state.z, yaw, seats: state.seats })
      if (car instanceof TankModel && state.turret) this.watchGun(id, car, state.turret, state.seats)
    }
    for (const [id, car] of this.cars) {
      if (!placed.has(id)) {
        car.dispose()
        this.cars.delete(id)
        this.shots.delete(id)
      }
    }
    return placed
  }

  /** A change in the shot counter is a shell: flash, tracer and burst, from where the barrel is drawn. */
  private watchGun(id: number, tank: TankModel, turret: Turret, seats: readonly number[]) {
    const before = this.shots.get(id)
    this.shots.set(id, turret.shots)
    // The first sight of a gun is not a shot, nor is a counter that has not moved.
    if (before === undefined || before === turret.shots) return
    const { origin, direction } = tank.barrel()
    tank.recoil()
    const muzzle = origin.clone().addScaledVector(direction, TANK_BARREL_LENGTH)
    this.effects.shell(muzzle, direction, Math.max(0.5, turret.range - TANK_BARREL_LENGTH), turret.range < SHELL_RANGE - 1, EXPLOSION_RADIUS)
    if (seats[1] === this.localId) this.lastShotAt = performance.now()
  }

  /** The key hint for what E does here. */
  private prompt(position: readonly number[], cars: ReadonlyMap<number, CarPlacement>): string | null {
    if (this.localId === null) return null
    if (this.seat) return 'E: get out'
    let best: { distance: number; driverSeatFree: boolean; tank: boolean } | null = null
    for (const car of cars.values()) {
      const distance = Math.hypot(car.x - position[0]!, car.y - position[1]!, car.z - position[2]!)
      const hasFreeSeat = car.seats.includes(0)
      if (hasFreeSeat && distance <= ENTER_RADIUS && (!best || distance < best.distance)) best = { distance, driverSeatFree: car.seats[0] === 0, tank: car.kind === KIND_TANK }
    }
    if (!best) return null
    if (best.tank) return best.driverSeatFree ? 'E: drive the tank' : 'E: man the gun'
    return best.driverSeatFree ? 'E: drive' : 'E: ride along'
  }

  /** The stickman for an entity; id 0 is the local player. */
  private avatar(id: number) {
    let avatar = this.avatars.get(id)
    if (!avatar) {
      avatar = new Stickman(id === 0 ? (this.localId ?? 0) : id)
      this.avatars.set(id, avatar)
      this.scene.scene.add(avatar.root)
    }
    return avatar
  }

  dispose() {
    this.disposed = true
    cancelAnimationFrame(this.frameHandle)
    clearTimeout(this.reconnectTimer)
    clearInterval(this.pingTimer)
    this.transport.close()
    this.input.dispose()
    this.effects.dispose()
    this.scene.dispose()
  }
}
