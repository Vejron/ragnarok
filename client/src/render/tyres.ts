import type { Vec3 } from '@ragnarok/engine'
import * as THREE from 'three'
import { KIND_HELI, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS } from '@/net/codec'
import { isTracked } from '@/sim/vehicles'
import { halfExtentsOf } from '@/sim/kinds'
import { tyreSlip, wheelLayout, type WheelLayout } from '@/sim/slip'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_SAND, SURFACE_WATER } from '@/sim/surface'
import type { Particles } from './particles'
import type { Skidmarks } from './skidmarks'

/** What every vehicle's effects draw into and look things up in. */
export interface FxWorld {
  smoke: Particles
  /** Additive particles: sparks and flames. */
  glow: Particles
  skids: Skidmarks
  groundAt(x: number, z: number): number
  surfaceAt(x: number, z: number): number
  /** Where the camera is, so far-away vehicles can skip the particles. */
  camera: Vec3
}

/** Particles are skipped for vehicles further from the camera than this, metres; marks reach a little further. */
const PARTICLE_RANGE = 120
const MARK_RANGE = 200
/** A wheel more than this above the ground under it is in the air. */
const AIRBORNE = 0.3
/** How quickly the smoothed slips and acceleration follow, per second. */
const SMOOTHING = 12

/** Marks and dust by surface: colour and opacity of a mark, colour of the cloud. */
const MARKS: Record<number, { mark: number; alpha: number; cloud: number }> = {
  [SURFACE_ASPHALT]: { mark: 0x161616, alpha: 0.6, cloud: 0xe4e4e4 },
  [SURFACE_DIRT]: { mark: 0x4e3b26, alpha: 0.45, cloud: 0xb39874 },
  [SURFACE_SAND]: { mark: 0x9c8455, alpha: 0.4, cloud: 0xdcc795 },
  [SURFACE_GRASS]: { mark: 0x3f4a22, alpha: 0.38, cloud: 0x8c8a5c },
}

const rand = (low: number, high: number) => low + Math.random() * (high - low)

/**
 * The tyre and exhaust effects of one vehicle: skidmarks and smoke when it slides on tarmac, ruts,
 * dust and flung gravel off-road, spray in water, exhaust puffs when it pulls away and a backfire
 * when a sports car lifts off. A helicopter only blows dust up when it is low.
 */
export class VehicleFx {
  private readonly layout: WheelLayout | null
  private readonly last: (THREE.Vector3 | null)[] = []
  private readonly slips: number[] = []
  private lastPosition: THREE.Vector3 | null = null
  private measured = false
  private speed = 0
  private accel = 0
  private peakAccel = 0
  private peak = 0
  private surface = SURFACE_GRASS
  private down = true
  private airTime = 0
  private fall = 0
  private landed = 0
  private readonly q = new THREE.Quaternion()
  private readonly contact = new THREE.Vector3()
  private readonly side = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  /** How fast it is going, m/s, as drawn. */
  readonly velocity = new THREE.Vector3()
  private readonly steerTurn = new THREE.Quaternion()
  private static readonly UP = new THREE.Vector3(0, 1, 0)

  /** Speed along its nose, m/s, signed, smoothed. */
  get forwardSpeed() {
    return this.speed
  }

  /** Acceleration along its nose, m/s², smoothed. */
  get acceleration() {
    return this.accel
  }

  /** The worst of its tyres' slips, 0..1. */
  get slip() {
    return this.slips.reduce((worst, value) => Math.max(worst, value), 0)
  }

  /** The surface under its wheels last time one touched the ground. */
  get surfaceUnder() {
    return this.surface
  }

  /** Whether any wheel touches the ground, or the vehicle lies upturned. */
  get grounded() {
    return this.down
  }

  /** The highest speed it had over the last few tenths of a second, m/s: how hard a crash was. */
  get impactSpeed() {
    return this.peak
  }

  /** The speed of a landing since it was last asked, m/s down, or 0. */
  takeLanding() {
    const fall = this.landed
    this.landed = 0
    return fall
  }

  constructor(
    private readonly id: number,
    readonly kind: number,
  ) {
    this.layout = wheelLayout(kind)
  }

  /**
   * Advances the effects `dt` seconds for the vehicle drawn at `position` / `rotation` with its front
   * wheels at `steer` (fraction of lock). `powered` is whether someone drives it.
   */
  update(world: FxWorld, position: readonly [number, number, number], rotation: readonly [number, number, number, number], steer: number, powered: boolean, dt: number) {
    if (dt <= 0 || dt > 0.1) {
      // After a stall, differences from the last frame mean nothing: start afresh.
      this.lift(world)
      this.lastPosition = null
      this.measured = false
      return
    }
    const here = new THREE.Vector3(...position)
    const distance = world.camera.distanceTo(here)
    this.q.set(...rotation)
    this.forward.set(0, 0, -1).applyQuaternion(this.q)
    if (this.lastPosition) {
      this.velocity.copy(here).sub(this.lastPosition).divideScalar(dt)
      const along = this.velocity.dot(this.forward)
      this.peak = Math.max(Math.hypot(this.velocity.x, this.velocity.z), this.peak * Math.max(0, 1 - dt * 4))
      const blend = Math.min(1, dt * SMOOTHING)
      // The first speed seen is taken as it is: a car that comes into view moving did not just launch.
      if (this.measured) this.accel += ((along - this.speed) / dt - this.accel) * blend
      this.speed = this.measured ? this.speed + (along - this.speed) * blend : along
      this.measured = true
    }
    this.lastPosition = here
    if (distance > MARK_RANGE) {
      this.lift(world)
      return
    }
    const particles = distance < PARTICLE_RANGE
    if (this.kind === KIND_HELI) {
      if (powered && particles) this.downwash(world, here, dt)
      return
    }
    this.wheels(world, steer, particles, dt)
    if (powered && particles) {
      if (this.kind === KIND_PLANE) this.castorOil(world, here, dt)
      // The Roadster is electric: nothing comes out of it but its thrusters' air.
      else if (this.kind !== KIND_ROADSTER) this.exhaust(world, here, dt)
    }
  }

  /** A rotary engine throws out burnt castor oil: a thin blue-grey trail from under the cowling. */
  private castorOil(world: FxWorld, here: THREE.Vector3, dt: number) {
    if (Math.random() > dt * 24) return
    const cowl = new THREE.Vector3(rand(-0.3, 0.3), -0.3, -2.4).applyQuaternion(this.q).add(here)
    world.smoke.spawn({
      position: cowl,
      // Left behind by the plane as it flies on, drifting a little.
      velocity: [this.velocity.x * 0.15 + rand(-0.3, 0.3), rand(-0.1, 0.3), this.velocity.z * 0.15 + rand(-0.3, 0.3)],
      life: rand(0.9, 1.6),
      size: [0.25, 1.4],
      color: 0x8a9096,
      alpha: 0.18,
      gravity: 0,
      drag: 1,
    })
  }

  private wheels(world: FxWorld, steer: number, particles: boolean, dt: number) {
    const layout = this.layout!
    const upright = new THREE.Vector3(0, 1, 0).applyQuaternion(this.q).y > 0.5
    let touching = !upright
    layout.contacts.forEach((local, wheel) => {
      const contact = this.contact.set(local[0], local[1], local[2]).applyQuaternion(this.q).add(this.lastPosition!)
      const steered = wheel < layout.steered
      this.side.set(1, 0, 0)
      if (steered) this.side.applyQuaternion(this.steerTurn.setFromAxisAngle(VehicleFx.UP, -steer * layout.maxSteer))
      this.side.applyQuaternion(this.q)
      const before = this.last[wheel]
      const sideways = before ? contact.clone().sub(before).divideScalar(dt).dot(this.side) : 0
      this.last[wheel] = contact.clone()
      // Every wheel of a jeep and a tank is driven; a sports car pushes with its rear pair, and a plane with its propeller.
      const driven = this.kind !== KIND_PLANE && (this.kind !== KIND_SPORTS || wheel >= 2)
      const wanted = tyreSlip(sideways, this.speed, this.accel, driven)
      const slip = (this.slips[wheel] ?? 0) + (wanted - (this.slips[wheel] ?? 0)) * Math.min(1, dt * SMOOTHING)
      this.slips[wheel] = slip

      const ground = world.groundAt(contact.x, contact.z)
      const track = `${this.id}:${wheel}`
      if (contact.y - ground <= AIRBORNE) touching = true
      if (!upright || contact.y - ground > AIRBORNE) {
        world.skids.lift(track)
        return
      }
      contact.y = Math.max(contact.y, ground)
      const surface = world.surfaceAt(contact.x, contact.z)
      this.surface = surface
      const moving = Math.abs(this.speed)
      if (surface === SURFACE_WATER) {
        world.skids.lift(track)
        if (particles && moving > 3 && Math.random() < dt * moving * 1.2) this.spray(world, contact)
        return
      }
      const look = MARKS[surface] ?? MARKS[SURFACE_GRASS]!
      const soft = surface !== SURFACE_ASPHALT
      // Soft ground takes a faint rut from any wheel rolling over it; a tank's tracks always mark it.
      const rut = soft && (isTracked(this.kind) ? moving > 0.5 : moving > 8) ? 0.35 : 0
      const strength = Math.max(slip, rut)
      if (strength > 0.12) world.skids.mark(track, contact, this.side, layout.width, look.mark, look.alpha * Math.min(1, strength + 0.2))
      else world.skids.lift(track)
      if (!particles) return

      if (surface === SURFACE_ASPHALT) {
        if (slip > 0.15 && Math.random() < dt * 45 * slip) this.cloud(world, contact, look.cloud, 0.45, [0.5, 3.2], 2.2)
        return
      }
      // Dust off the tyres when sliding, and a trail behind any quick car off-road.
      const kick = slip + Math.max(0, moving - 6) / 25
      if (Math.random() < dt * 22 * kick) this.cloud(world, contact, look.cloud, surface === SURFACE_GRASS ? 0.28 : 0.4, [0.7, 3.6], 2.0)
      if (surface !== SURFACE_GRASS && slip > 0.2 && Math.random() < dt * 60 * slip) this.gravel(world, contact)
    })
    // A landing is the first wheel touching after a real drop through the air.
    if (touching) {
      if (!this.down && this.airTime > 0.25 && this.fall > 3) this.landed = this.fall
      this.airTime = 0
      this.fall = 0
    } else {
      this.airTime += dt
      this.fall = Math.max(this.fall, -this.velocity.y)
    }
    this.down = touching
  }

  /** Puff of tyre smoke or dust. */
  private cloud(world: FxWorld, at: THREE.Vector3, color: number, alpha: number, size: [number, number], life: number) {
    world.smoke.spawn({
      position: [at.x + rand(-0.2, 0.2), at.y + 0.15, at.z + rand(-0.2, 0.2)],
      velocity: [this.velocity.x * 0.15 + rand(-0.6, 0.6), rand(0.3, 1.0), this.velocity.z * 0.15 + rand(-0.6, 0.6)],
      life: life * rand(0.7, 1.2),
      size,
      color,
      alpha,
      gravity: -0.3,
      drag: 1.2,
    })
  }

  /** A stone flung back off a spinning or sliding tyre. */
  private gravel(world: FxWorld, at: THREE.Vector3) {
    const back = this.forward.clone().multiplyScalar(-rand(2, 5) - Math.abs(this.speed) * 0.15)
    world.smoke.spawn({
      position: [at.x, at.y + 0.1, at.z],
      velocity: [back.x + this.velocity.x * 0.5 + rand(-1.2, 1.2), rand(1.5, 4.5), back.z + this.velocity.z * 0.5 + rand(-1.2, 1.2)],
      life: rand(0.8, 1.6),
      size: [0.09, 0.07],
      color: Math.random() < 0.5 ? 0x3d3022 : 0x6b5a45,
      alpha: 1,
      gravity: 9.8,
      bounce: true,
    })
  }

  private spray(world: FxWorld, at: THREE.Vector3) {
    world.smoke.spawn({
      position: [at.x, at.y + 0.1, at.z],
      velocity: [this.side.x * rand(-2.5, 2.5) + this.velocity.x * 0.3, rand(2, 4.5), this.side.z * rand(-2.5, 2.5) + this.velocity.z * 0.3],
      life: rand(0.5, 0.9),
      size: [0.25, 1.1],
      color: 0xeaf5ff,
      alpha: 0.7,
      gravity: 9.8,
    })
  }

  /** Exhaust puffs when the driver pulls away, and a backfire when a sports car lifts off hard. */
  private exhaust(world: FxWorld, here: THREE.Vector3, dt: number) {
    const [hx, hy, hz] = halfExtentsOf(this.kind)
    const pipe = new THREE.Vector3(hx * 0.45, -hy * 0.5, hz + 0.05).applyQuaternion(this.q).add(here)
    const tank = isTracked(this.kind)
    const pulling = this.accel > (tank ? 0.8 : 2.0)
    if ((pulling && Math.random() < dt * 30) || Math.random() < dt * 1.5) {
      world.smoke.spawn({
        position: pipe,
        velocity: [-this.forward.x * 1.2 + rand(-0.3, 0.3), rand(0.3, 0.8), -this.forward.z * 1.2 + rand(-0.3, 0.3)],
        life: rand(0.6, 1.1),
        size: tank ? [0.4, 1.6] : [0.2, 0.9],
        color: tank ? 0x2a2a2a : 0x9a9a9a,
        alpha: pulling ? 0.4 : 0.15,
        gravity: -0.5,
        drag: 2,
      })
    }
    this.peakAccel = Math.max(this.accel, this.peakAccel - dt * 6)
    if (this.kind === KIND_SPORTS && this.peakAccel > 3 && this.accel < -0.5 && this.speed > 15) {
      this.peakAccel = 0
      for (let i = 0; i < 6; i++) {
        world.glow.spawn({
          position: pipe,
          velocity: [-this.forward.x * rand(3, 6) + this.velocity.x, rand(-0.2, 0.4), -this.forward.z * rand(3, 6) + this.velocity.z],
          life: rand(0.08, 0.16),
          size: [0.5, 0.15],
          color: i % 2 ? 0xff8a2a : 0xffd27a,
          alpha: 1,
          drag: 4,
        })
      }
    }
  }

  /** A low helicopter's rotor raises a ring of dust below it. */
  private downwash(world: FxWorld, here: THREE.Vector3, dt: number) {
    const ground = world.groundAt(here.x, here.z)
    const height = here.y - ground
    if (height > 14) return
    const strength = 1 - height / 14
    const surface = world.surfaceAt(here.x, here.z)
    const color = surface === SURFACE_WATER ? 0xeaf5ff : (MARKS[surface] ?? MARKS[SURFACE_GRASS]!).cloud
    for (let i = 0; i < 3; i++) {
      if (Math.random() > dt * 40 * strength) continue
      const angle = rand(0, Math.PI * 2)
      const radius = rand(1, 4)
      const out = rand(4, 8) * strength
      world.smoke.spawn({
        position: [here.x + Math.cos(angle) * radius, ground + 0.2, here.z + Math.sin(angle) * radius],
        velocity: [Math.cos(angle) * out, rand(0.2, 0.8), Math.sin(angle) * out],
        life: rand(1.0, 1.8),
        size: [0.8, 3.2],
        color,
        alpha: 0.3 * strength,
        gravity: -0.2,
        drag: 1.4,
      })
    }
  }

  /** Ends every mark this vehicle is laying. */
  lift(world: Pick<FxWorld, 'skids'>) {
    this.last.forEach((_, wheel) => world.skids.lift(`${this.id}:${wheel}`))
    this.last.length = 0
  }
}

/** A crash at `at`: a shower of sparks, chips of `paint` and a puff of dust, carried on with `velocity`. */
export function crashBurst(world: FxWorld, at: THREE.Vector3, paint: number, velocity: THREE.Vector3) {
  for (let i = 0; i < 26; i++) {
    const speed = rand(3, 9)
    const angle = rand(0, Math.PI * 2)
    world.glow.spawn({
      position: at,
      velocity: [Math.cos(angle) * speed + velocity.x * 0.3, rand(1, 6), Math.sin(angle) * speed + velocity.z * 0.3],
      life: rand(0.25, 0.6),
      size: [0.14, 0.04],
      color: i % 3 === 0 ? 0xffffff : i % 3 === 1 ? 0xffd27a : 0xff9a3c,
      alpha: 1,
      gravity: 9.8,
      drag: 0.8,
    })
  }
  for (let i = 0; i < 10; i++) {
    world.smoke.spawn({
      position: at,
      velocity: [rand(-3, 3) + velocity.x * 0.5, rand(1.5, 4.5), rand(-3, 3) + velocity.z * 0.5],
      life: rand(1.5, 3),
      size: [0.12, 0.1],
      color: i % 2 === 0 ? paint : 0x2b2b2b,
      alpha: 1,
      gravity: 9.8,
      bounce: true,
    })
  }
  for (let i = 0; i < 4; i++) {
    world.smoke.spawn({
      position: [at.x + rand(-0.4, 0.4), at.y, at.z + rand(-0.4, 0.4)],
      velocity: [rand(-0.8, 0.8), rand(0.3, 1), rand(-0.8, 0.8)],
      life: rand(1.2, 2),
      size: [0.6, 2.6],
      color: 0xb8b0a0,
      alpha: 0.35,
      gravity: -0.2,
      drag: 1.5,
    })
  }
}

/** Metal dragging along something at `at`: a spray of sparks trailing behind the vehicle. */
export function scrapeSparks(world: FxWorld, at: THREE.Vector3, velocity: THREE.Vector3) {
  for (let i = 0; i < 8; i++) {
    world.glow.spawn({
      position: at,
      velocity: [-velocity.x * rand(0.1, 0.4) + rand(-1.5, 1.5) + velocity.x, rand(0.5, 3), -velocity.z * rand(0.1, 0.4) + rand(-1.5, 1.5) + velocity.z],
      life: rand(0.15, 0.4),
      size: [0.1, 0.03],
      color: i % 2 === 0 ? 0xfff2b0 : 0xffb347,
      alpha: 1,
      gravity: 9.8,
      drag: 1.5,
    })
  }
}

/** Jets per nozzle per second while a Roadster's thrusters fire. */
const JET_RATE = 90

/**
 * A Roadster's cold-gas jets for one frame: white mist blasting out of each nozzle and spreading,
 * carried on with the car's `velocity`. Compressed air does not burn, so there is no flame, only
 * a cold, fast plume that a lift jet also kicks up off the ground below.
 */
export function coldGas(world: FxWorld, jets: readonly { at: THREE.Vector3; direction: THREE.Vector3 }[], velocity: THREE.Vector3, dt: number) {
  const count = Math.max(1, Math.round(JET_RATE * dt))
  for (const { at, direction } of jets) {
    for (let i = 0; i < count; i++) {
      const speed = rand(14, 22)
      world.smoke.spawn({
        position: [at.x + direction.x * 0.1, at.y + direction.y * 0.1, at.z + direction.z * 0.1],
        velocity: [velocity.x + direction.x * speed + rand(-1.5, 1.5), velocity.y + direction.y * speed + rand(-1.5, 1.5), velocity.z + direction.z * speed + rand(-1.5, 1.5)],
        life: rand(0.25, 0.45),
        size: [0.18, rand(1.0, 1.6)],
        color: 0xf4f9ff,
        colorEnd: 0xd8e4ee,
        alpha: 0.55,
        drag: 4,
        gravity: -0.5,
        shape: 'puff',
      })
    }
    // A lift jet near the ground blows a ring of dust out from under the car.
    const ground = world.groundAt(at.x, at.z)
    if (direction.y < -0.5 && at.y - ground < 3) {
      const angle = rand(0, Math.PI * 2)
      world.smoke.spawn({
        position: [at.x, ground + 0.15, at.z],
        velocity: [Math.cos(angle) * rand(5, 9), rand(0.2, 0.8), Math.sin(angle) * rand(5, 9)],
        life: rand(0.5, 0.9),
        size: [0.4, 1.8],
        color: 0xc9bfa8,
        alpha: 0.35,
        drag: 2.5,
        shape: 'puff',
      })
    }
  }
}
