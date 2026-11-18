import { type Vec3, Quaternion, Vector3 } from '@ragnarok/engine'
import { kindView } from '@/kinds'
import type { KindFx, Roost } from '@/kinds/kind'
import { isTracked } from '@/sim/vehicles'
import { halfExtentsOf } from '@/sim/kinds'
import { groundOf, type Ground } from '@/sim/groundsound'
import { tyreSlip, type WheelLayout } from '@/sim/slip'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_MUD, SURFACE_MUD_DEEP, SURFACE_MUD_FAINT, SURFACE_SAND, SURFACE_WATER, surfaceWetness } from '@/sim/surface'
import { inMarshPool } from './marshnoise'
import type { Particles } from './particles'
import { iced, winter } from './season'
import type { SnowRoost } from './snowroost'
import type { JetSpray } from './jetspray'
import { splash } from './splash'
import { missileExhaust } from './launch'
import type { Skidmarks } from './skidmarks'
import type { Stir } from './waterwaves'

/** What every vehicle's effects draw into and look things up in. */
export interface FxWorld {
  smoke: Particles
  /** Additive particles: sparks and flames. */
  glow: Particles
  skids: Skidmarks
  groundAt(x: number, z: number): number
  surfaceAt(x: number, z: number): number
  /** The water surface height at a point, `NO_WATER` where there is none. */
  waterAt?(x: number, z: number): number
  /** Stirs the water: hulls and bodies pressed into its waves (`WaterWaves`). */
  waves?: Stir
  /** Where the camera is, so far-away vehicles can skip the particles. */
  camera: Vec3
  /** The snow tracks throw up when they dig in: only in a winter world. */
  roost?: SnowRoost | null
  /** The water a waterjet boat throws (`KindFx.jets`). */
  jetSpray?: JetSpray | null
  /** Where effects are heard: every splash (`render/splash.ts`) makes its sound through it. */
  heard?: { splash(at: readonly [number, number, number], size: number, speed: number): void }
}

/** Particles are skipped for vehicles further from the camera than this, metres; marks reach a little further. */
const PARTICLE_RANGE = 120
const MARK_RANGE = 200
/** A wheel more than this above the ground under it is in the air. */
const AIRBORNE = 0.3
/** A hull whose underside is within this of the water surface, metres, rides on it (`skim`). */
const SKIM_DRAFT = 0.3
/** Slower than this, m/s, a hull on the water throws no spray. */
const SKIM_SPEED = 5
/** A hull whose top is this far under the surface, metres, no longer stirs it; it fades out on the way down. */
const DIVED = 0.5
/** Slower than this, m/s, a hull in the water carves no wake. */
const WAKE_SPEED = 1
/** Going into the water slower than this, m/s, throws no splash. */
const ENTRY_SPEED = 1.5
/** Out of the water at least this long, seconds, before going back in splashes again: not every wave a planing hull skips over. */
const ENTRY_DRY = 0.25
/** Faster than this, m/s, a hull ploughing through the water (not planing over it) throws water off its bow. */
const PLOUGH_SPEED = 2.5
/** A waterjet boat's crash stop starts to throw spray at this deceleration, m/s², and throws all of it at `CRASH_STOP_FULL`. */
const CRASH_STOP_FROM = 3
const CRASH_STOP_FULL = 8
/** The speed a waterjet boat's rooster tail is at its biggest, m/s. */
const JET_FULL_SPEED = 22
/** How quickly the smoothed slips and acceleration follow, per second. */
const SMOOTHING = 12

/** Marks and dust by surface: colour and opacity of a mark, colour of the cloud. */
const MARKS: Record<number, { mark: number; alpha: number; cloud: number }> = {
  [SURFACE_ASPHALT]: { mark: 0x161616, alpha: 0.6, cloud: 0xe4e4e4 },
  [SURFACE_DIRT]: { mark: 0x4e3b26, alpha: 0.45, cloud: 0xb39874 },
  [SURFACE_SAND]: { mark: 0x9c8455, alpha: 0.4, cloud: 0xdcc795 },
  [SURFACE_GRASS]: { mark: 0x3f4a22, alpha: 0.38, cloud: 0x8c8a5c },
  // Wet ruts, darker the wetter; a marsh throws no dust.
  [SURFACE_MUD_FAINT]: { mark: 0x33281a, alpha: 0.5, cloud: 0x5e4a32 },
  [SURFACE_MUD]: { mark: 0x2a2116, alpha: 0.58, cloud: 0x5e4a32 },
  [SURFACE_MUD_DEEP]: { mark: 0x221b12, alpha: 0.64, cloud: 0x5e4a32 },
}
/** In a winter world everything off the ploughed roads is under snow: packed, blue-grey ruts and powder for dust. */
const SNOW = { mark: 0x9fb0c2, alpha: 0.5, cloud: 0xf4f8ff }
/** A track's chassis bottom more than this above the ground under it throws no snow, metres. */
const ROOST_AIRBORNE = 0.9
/** How much snow a track throws where the road is ploughed, and off lake ice, against deep snow. */
const ROOST_ON_ROAD = 0.2
const ROOST_ON_ICE = 0.3

const rand = (low: number, high: number) => low + Math.random() * (high - low)

/**
 * The tyre and exhaust effects of one vehicle: skidmarks and smoke when it slides on tarmac, ruts,
 * dust and flung gravel off-road, spray in water, exhaust puffs when it pulls away and a backfire
 * when a sports car lifts off. A helicopter only blows dust up when it is low.
 */
export class VehicleFx {
  private readonly fx: KindFx
  private readonly last: (Vector3 | null)[] = []
  private readonly slips: number[] = []
  private lastPosition: Vector3 | null = null
  private measured = false
  private speed = 0
  private accel = 0
  private peakAccel = 0
  private peak = 0
  private surface = SURFACE_GRASS
  /** What its wheels last touched, as it is heard. */
  private heard: Ground = 'grass'
  /** Whether its bottom sat in open water last frame, and whether all of it was under the surface. */
  private afloat = false
  private dived = false
  /** Its heading last frame, radians, and how fast it turns, rad/s, smoothed. */
  private heading: number | null = null
  private turning = 0
  private down = true
  /** Whether a wheel was in water last frame. */
  private wet = false
  /** The lowest of its wheels' contacts in water this frame, metres up; null with none in water. */
  private wetBottom: number | null = null
  /** The lowest of all its wheels' contacts this frame, touching or not, metres up; null with no wheels. */
  private lowest: number | null = null
  /** Whether its bottom (hull or wheels) was in the water last frame; null before it has been seen. */
  private soaked: boolean | null = null
  /** How long it has been out of the water, seconds. */
  private dryFor = 0
  /** Whether its hull rode on the water surface last frame. */
  private skimming = false
  private airTime = 0
  private fall = 0
  private landed = 0
  private readonly q = new Quaternion()
  private readonly contact = new Vector3()
  private readonly side = new Vector3()
  private readonly forward = new Vector3()
  /** How fast it is going, m/s, as drawn. */
  readonly velocity = new Vector3()
  private readonly steerTurn = new Quaternion()
  private static readonly UP = new Vector3(0, 1, 0)

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

  /** What it is heard rolling on: what its wheels last touched, or the water its hull sits in. */
  get ground(): Ground {
    return this.afloat ? 'water' : this.heard
  }

  /** Whether it is wholly under the water: a dived submarine, a sunk car. */
  get submerged() {
    return this.dived
  }

  /** How fast it turns about the vertical, rad/s, smoothed; + turns left. */
  get yawRate() {
    return this.turning
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

  /** The unspent fraction of a smoke puff behind a missile. */
  private rocketOwed = 0

  constructor(
    private readonly id: number,
    readonly kind: number,
  ) {
    this.fx = kindView(kind).fx
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
      this.heading = null
      this.measured = false
      this.soaked = null
      return
    }
    const here = new Vector3(...position)
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
      const heading = Math.atan2(-this.forward.x, -this.forward.z)
      if (this.heading !== null) {
        let turned = heading - this.heading
        if (turned > Math.PI) turned -= 2 * Math.PI
        if (turned < -Math.PI) turned += 2 * Math.PI
        this.turning += (turned / dt - this.turning) * blend
      }
      this.heading = heading
      this.measured = true
    }
    this.lastPosition = here
    // A missile's motor trails fire and smoke while it burns, seen from further off than anything else's.
    if (this.fx.exhaust === 'rocket') {
      const v = this.velocity
      this.rocketOwed = powered ? missileExhaust(world, [here.x, here.y, here.z], [v.x, v.y, v.z], dt, this.rocketOwed) : 0
      return
    }
    if (distance > MARK_RANGE) {
      this.lift(world)
      this.soaked = null
      return
    }
    const particles = distance < PARTICLE_RANGE
    const layout = this.fx.wheels
    if (!layout) {
      this.wetBottom = null
      this.lowest = null
      this.skim(world, here, particles, dt)
      if (!this.fx.floats && powered && particles) this.downwash(world, here, dt)
      return
    }
    this.wheels(world, layout, steer, particles, dt)
    this.skim(world, here, particles, dt)
    if (powered && particles && world.roost && this.fx.roost) this.roost(world, world.roost, this.fx.roost, here, distance, dt)
    if (powered && particles) {
      if (this.fx.exhaust === 'castor') this.castorOil(world, here, dt)
      else if (this.fx.exhaust === 'pipe') this.exhaust(world, here, dt)
    }
  }

  /** A rotary engine throws out burnt castor oil: a thin blue-grey trail from under the cowling. */
  private castorOil(world: FxWorld, here: Vector3, dt: number) {
    if (Math.random() > dt * 24) return
    const cowl = new Vector3(rand(-0.3, 0.3), -0.3, -2.4).applyQuaternion(this.q).add(here)
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

  private wheels(world: FxWorld, layout: WheelLayout, steer: number, particles: boolean, dt: number) {
    const upright = new Vector3(0, 1, 0).applyQuaternion(this.q).y > 0.5
    let touching = !upright
    const wet: Vector3[] = []
    this.wetBottom = null
    this.lowest = null
    layout.contacts.forEach((local, wheel) => {
      const contact = this.contact.set(local[0], local[1], local[2]).applyQuaternion(this.q).add(this.lastPosition!)
      const steered = wheel < layout.steered
      this.side.set(1, 0, 0)
      if (steered) this.side.applyQuaternion(this.steerTurn.setFromAxisAngle(VehicleFx.UP, -steer * layout.maxSteer * (layout.shares?.[wheel] ?? 1)))
      this.side.applyQuaternion(this.q)
      const before = this.last[wheel]
      const sideways = before ? contact.clone().sub(before).divideScalar(dt).dot(this.side) : 0
      this.last[wheel] = contact.clone()
      // Every wheel of a jeep and a tank is driven; a sports car pushes with its rear pair, and a plane with its propeller.
      const driven = this.fx.drivenFrom !== null && wheel >= this.fx.drivenFrom
      const wanted = tyreSlip(sideways, this.speed, this.accel, driven)
      const slip = (this.slips[wheel] ?? 0) + (wanted - (this.slips[wheel] ?? 0)) * Math.min(1, dt * SMOOTHING)
      this.slips[wheel] = slip

      this.lowest = Math.min(this.lowest ?? Infinity, contact.y)
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
      this.heard = groundOf(surface, winter, iced)
      const moving = Math.abs(this.speed)
      const wetness = surfaceWetness(surface)
      // In winter the marsh is frozen, and the lakes unless the world keeps its open water: ice takes no marks and throws nothing up.
      if ((iced && surface === SURFACE_WATER) || (winter && wetness > 0)) {
        world.skids.lift(track)
        return
      }
      // A marsh's pools splash as water does.
      if (surface === SURFACE_WATER || (wetness > 0 && inMarshPool(contact.x, contact.z, wetness))) {
        world.skids.lift(track)
        wet.push(contact.clone())
        this.wetBottom = Math.min(this.wetBottom ?? Infinity, contact.y)
        if (particles && moving > 3 && Math.random() < dt * moving * 1.2) this.spray(world, contact)
        return
      }
      const soft = surface !== SURFACE_ASPHALT
      const look = winter && soft ? SNOW : (MARKS[surface] ?? MARKS[SURFACE_GRASS]!)
      // Soft ground takes a faint rut from any wheel rolling over it; a tank's tracks always mark it; a marsh
      // takes a deep one from anything.
      const rut = wetness > 0 ? (moving > 0.5 ? 0.6 : 0) : soft && (isTracked(this.kind) ? moving > 0.5 : moving > 8) ? 0.35 : 0
      const strength = Math.max(slip, rut)
      if (strength > 0.12) world.skids.mark(track, contact, this.side, layout.width, look.mark, look.alpha * Math.min(1, strength + 0.2))
      else world.skids.lift(track)
      if (!particles) return

      if (surface === SURFACE_ASPHALT) {
        if (slip > 0.15 && Math.random() < dt * 45 * slip) this.cloud(world, contact, look.cloud, 0.45, [0.5, 3.2], 2.2)
        return
      }
      if (wetness > 0) {
        // Clods of mud off the tyres instead of dust: when they spin, and behind anything quick.
        const fling = slip + Math.max(0, moving - 4) / 12
        if (Math.random() < dt * 40 * fling) this.gravel(world, contact, true)
        return
      }
      // Dust off the tyres when sliding, and a trail behind any quick car off-road.
      const kick = slip + Math.max(0, moving - 6) / 25
      if (Math.random() < dt * 22 * kick) this.cloud(world, contact, look.cloud, surface === SURFACE_GRASS ? 0.28 : 0.4, [0.7, 3.6], 2.0)
      if (surface !== SURFACE_GRASS && !winter && slip > 0.2 && Math.random() < dt * 60 * slip) this.gravel(world, contact)
    })
    this.wet = wet.length > 0
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

  /**
   * Snow off the end of each track: thick when it pulls hard or spins, and a thin veil behind it when it
   * goes quickly. Ploughed roads and lake ice give little; nothing comes off a track in the air.
   */
  private roost(world: FxWorld, snow: SnowRoost, roost: Roost, here: Vector3, distance: number, dt: number) {
    if (new Vector3(0, 1, 0).applyQuaternion(this.q).y < 0.5) return
    const [from, full] = roost.accel
    const pull = Math.min(1, Math.max(0, (this.accel - from) / (full - from)))
    const cruise = Math.min(0.3, Math.max(0, Math.abs(this.speed) - 8) / 30)
    // Far off, less of it: the plume still reads from a distance with a fraction of the particles.
    const near = Math.min(1, 1.5 - distance / PARTICLE_RANGE)
    const strength = Math.max(pull, this.slip * 0.6, cruise) * near
    if (strength < 0.03) return
    const back = this.forward.clone().negate().setY(0).normalize()
    const bottom = -halfExtentsOf(this.kind)[1]
    for (const [x, z] of roost.at) {
      const at = this.contact.set(x, bottom, z).applyQuaternion(this.q).add(here)
      const ground = world.groundAt(at.x, at.z)
      if (at.y - ground > ROOST_AIRBORNE) continue
      at.y = ground
      const surface = world.surfaceAt(at.x, at.z)
      const deep = surface === SURFACE_ASPHALT ? ROOST_ON_ROAD : surface === SURFACE_WATER || surfaceWetness(surface) > 0 ? ROOST_ON_ICE : 1
      snow.throw(at, back, this.velocity, this.speed, strength * deep, roost, dt)
    }
  }

  /** Puff of tyre smoke or dust. */
  private cloud(world: FxWorld, at: Vector3, color: number, alpha: number, size: [number, number], life: number) {
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

  /** A stone, or a clod of `mud`, flung back off a spinning or sliding tyre. */
  private gravel(world: FxWorld, at: Vector3, mud = false) {
    const back = this.forward.clone().multiplyScalar(-rand(2, 5) - Math.abs(this.speed) * 0.15)
    world.smoke.spawn({
      position: [at.x, at.y + 0.1, at.z],
      velocity: [back.x + this.velocity.x * 0.5 + rand(-1.2, 1.2), rand(1.5, 4.5), back.z + this.velocity.z * 0.5 + rand(-1.2, 1.2)],
      life: rand(0.8, 1.6),
      size: mud ? [0.14, 0.1] : [0.09, 0.07],
      color: mud ? (Math.random() < 0.5 ? 0x2b2116 : 0x3c3020) : Math.random() < 0.5 ? 0x3d3022 : 0x6b5a45,
      alpha: 1,
      gravity: 9.8,
      bounce: !mud,
    })
  }

  private spray(world: FxWorld, at: Vector3) {
    // The wheel's contact is the bed; the spray starts at the surface.
    const level = world.waterAt?.(at.x, at.z)
    const top = level !== undefined && Number.isFinite(level) ? level : at.y
    world.smoke.spawn({
      position: [at.x, top + 0.1, at.z],
      velocity: [this.side.x * rand(-2.5, 2.5) + this.velocity.x * 0.3, rand(2, 4.5), this.side.z * rand(-2.5, 2.5) + this.velocity.z * 0.3],
      life: rand(0.5, 0.9),
      size: [0.25, 1.1],
      color: 0xeaf5ff,
      alpha: 0.7,
      gravity: 9.8,
    })
  }

  /**
   * A hull planing over open water: a rooster tail off its back, sheets of spray off its sides and
   * rings spreading behind, and a splash where it comes down onto the surface. Its wheels touch
   * nothing out there, so it counts as on the ground.
   */
  private skim(world: FxWorld, here: Vector3, particles: boolean, dt: number) {
    const level = iced ? undefined : world.waterAt?.(here.x, here.z)
    const [hx, hull, hz] = halfExtentsOf(this.kind)
    // What meets the water: the hull's underside, or a seaplane's pontoons under it, or a car's wheels.
    const hy = this.fx.keel ?? hull
    const meets = this.fx.keel === undefined && this.lowest !== null ? Math.min(here.y - hy, this.lowest) : here.y - hy
    const moving = Math.hypot(this.velocity.x, this.velocity.z)
    const afloat = level !== undefined && Number.isFinite(level)
    // Wheels or tracks on the bed under the water drive through it, not over it.
    const skimming = afloat && this.wetBottom === null && Math.abs(meets - level) < SKIM_DRAFT && moving > SKIM_SPEED
    // A seaplane lying on its pontoons is down, moving or not.
    if (this.fx.keel !== undefined && afloat && here.y - hy < level && new Vector3(0, 1, 0).applyQuaternion(this.q).y > 0.5) {
      this.down = true
      this.airTime = 0
    }
    // Its bottom: the hull's underside, or lower, its wheels or tracks where they are in the water.
    const bottom = Math.min(meets, this.wetBottom ?? Infinity)
    // On dry land it is out of the water, for when it goes in.
    this.splashIn(world, here, afloat ? level : -Infinity, bottom, hx, hz, particles, dt)
    // Anything in the water and moving pushes the water aside, and the waves make its wake of that: a
    // bow wave and a V behind. How deep it sits is how far its bottom is under the surface; a planing
    // hull rides up out of it and pushes less, a crash stop digs the bow in.
    const inWater = afloat && bottom < level + SKIM_DRAFT
    this.afloat = inWater
    this.dived = afloat && here.y + hull < level
    if (inWater && moving > WAKE_SPEED && world.waves && here.y + hull > level - DIVED) {
      const ahead = this.forward
      const sunk = Math.min(1, Math.max(0.1, level - bottom))
      // Only what breaks the surface pushes it aside: a dived submarine leaves it still.
      const breaking = Math.min(1, Math.max(0, (here.y + hull - level + DIVED) / DIVED))
      const stop = this.fx.jets ? this.crashStop() : 0
      world.waves.hull(`vehicle ${this.id}`, {
        x: here.x,
        z: here.z,
        ahead: [ahead.x, ahead.z],
        halfWidth: hx,
        halfLength: hz,
        depth: (sunk * 0.6 + 0.35 * stop) * breaking,
        velocity: [this.velocity.x, this.velocity.z],
        churn: (skimming ? 1.5 : 0.6) + 3 * stop,
      })
    }
    if (inWater && particles && this.fx.jets) this.jets(world, here, level, dt)
    // Ploughing through rather than planing over: water heaps and is thrown off the bow.
    if (inWater && !skimming && !this.fx.floats && particles && moving > PLOUGH_SPEED && bottom < level && Math.random() < dt * moving * 0.5) {
      const bow = this.velocity.clone().setY(0).normalize().multiplyScalar(hz * 0.8).add(here)
      splash(world, { x: bow.x, y: level, z: bow.z }, 0.3 + Math.min(hx, 2) * 0.3, moving)
    }
    if (skimming) {
      this.down = true
      this.airTime = 0
      this.fall = 0
      // Its spray: fans off its back corners, a rooster tail off a snowmobile's track, a mist behind.
      if (particles && !this.fx.jets) {
        const right = new Vector3(1, 0, 0).applyQuaternion(this.q).setY(0).normalize()
        const ahead = this.forward.clone().setY(0).normalize()
        world.jetSpray?.skim({ at: here, right, ahead, velocity: this.velocity, speed: moving, level, halfBeam: hx, halfLength: hz, tail: this.fx.roost ? 1 : 0.3 }, dt)
      }
    }
    this.skimming = skimming
  }

  /**
   * A splash where anything goes into the water: its bottom (`bottom`, metres up) coming under the surface
   * at `level` after being out of it, driving in or dropping in. The faster it comes, the bigger the
   * crown, and the water it pushes aside runs out as a ring as big as its hull (`hx`, `hz`, half its
   * width and length, metres).
   */
  private splashIn(world: FxWorld, here: Vector3, level: number, bottom: number, hx: number, hz: number, particles: boolean, dt: number) {
    const soaked = bottom < level
    const was = this.soaked
    this.soaked = soaked
    if (!soaked) {
      this.dryFor += dt
      return
    }
    const dried = this.dryFor
    this.dryFor = 0
    // First seen already in the water is not going in, and nor is skipping over a wave.
    if (was !== false || dried < ENTRY_DRY) return
    const across = Math.hypot(this.velocity.x, this.velocity.z)
    const hard = Math.max(across, -this.velocity.y)
    if (hard < ENTRY_SPEED) return
    // Driven in, the bow goes in first; dropped in, it lands flat.
    const lead = across > -this.velocity.y ? hz * 0.7 : 0
    const at = across > 0.1 ? this.velocity.clone().setY(0).normalize().multiplyScalar(lead).add(here) : here.clone()
    const size = Math.min(Math.max(hx, 0.6) * 1.2, 0.5 + hard * 0.12)
    if (particles) splash(world, { x: at.x, y: level, z: at.z }, size, hard)
    // The water it shoves aside, as big as the hull, deeper the harder it came.
    world.waves?.splash(here.x, here.z, Math.max(hx, Math.min(hz, hx * 2)), Math.min(1.2, 0.1 + hard * 0.05), 1)
  }

  /** How hard a waterjet boat is crash stopping, 0..1, from its deceleration while it still runs ahead. */
  private crashStop() {
    if (this.speed < 2) return 0
    return Math.min(1, Math.max(0, (-this.accel - CRASH_STOP_FROM) / (CRASH_STOP_FULL - CRASH_STOP_FROM)))
  }

  /**
   * A waterjet boat in the water: its spray (`JetSpray`), the rooster tail and bow sheets under way and
   * the storm of water a crash stop throws; the waves it makes are the wave field's.
   */
  private jets(world: FxWorld, here: Vector3, level: number, dt: number) {
    const [hx, , hz] = halfExtentsOf(this.kind)
    const right = new Vector3(1, 0, 0).applyQuaternion(this.q).setY(0).normalize()
    const ahead = this.forward.clone().setY(0).normalize()
    const stop = this.crashStop()
    world.jetSpray?.throw({ at: here, right, ahead, velocity: this.velocity, speed: this.speed, stop, level, halfBeam: hx, halfLength: hz, fullSpeed: JET_FULL_SPEED }, dt)
  }

  /** Exhaust puffs when the driver pulls away, and a backfire when a sports car lifts off hard. */
  private exhaust(world: FxWorld, here: Vector3, dt: number) {
    const [hx, hy, hz] = halfExtentsOf(this.kind)
    const pipe = new Vector3(hx * 0.45, -hy * 0.5, hz + 0.05).applyQuaternion(this.q).add(here)
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
    if (this.fx.backfire && this.peakAccel > 3 && this.accel < -0.5 && this.speed > 15) {
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
  private downwash(world: FxWorld, here: Vector3, dt: number) {
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
export function crashBurst(world: FxWorld, at: Vector3, paint: number, velocity: Vector3) {
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
export function scrapeSparks(world: FxWorld, at: Vector3, velocity: Vector3) {
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

/** Puffs per foot per second while a Titan's thrusters fire, and how far over the ground they still blow dust out, metres. */
const THRUSTER_RATE = 70
const THRUSTER_WASH = 30

/**
 * The thrusters in a Titan's feet for one frame: a column of flame out of each sole, the smoke it
 * turns to, and the dust it blows out along the ground when the foot is near it.
 */
export function thrusterFlame(world: FxWorld, jets: readonly { at: Vector3; direction: Vector3 }[], velocity: Vector3, dt: number) {
  const count = Math.max(1, Math.round(THRUSTER_RATE * dt))
  for (const { at, direction } of jets) {
    for (let i = 0; i < count; i++) {
      const speed = rand(35, 55)
      const spread = () => rand(-4, 4)
      const from: [number, number, number] = [at.x + rand(-2, 2), at.y, at.z + rand(-2, 2)]
      const out: [number, number, number] = [velocity.x + direction.x * speed + spread(), velocity.y + direction.y * speed + spread(), velocity.z + direction.z * speed + spread()]
      world.glow.spawn({ position: from, velocity: out, life: rand(0.25, 0.5), size: [rand(4.5, 6.5), 1.2], color: 0xffd9a0, colorEnd: 0xff5a14, alpha: 0.9, drag: 1.5, shape: 'puff' })
      if (i % 2 === 0) world.smoke.spawn({ position: from, velocity: [out[0] * 0.6, out[1] * 0.6, out[2] * 0.6], life: rand(1, 1.8), size: [4, rand(10, 15)], color: 0x8a8580, colorEnd: 0xbdb8b0, alpha: 0.3, drag: 2.5, gravity: -1, shape: 'puff' })
    }
    // Near the ground the blast runs out along it.
    const ground = world.groundAt(at.x, at.z)
    if (at.y - ground < THRUSTER_WASH) {
      const angle = rand(0, Math.PI * 2)
      world.smoke.spawn({
        position: [at.x + Math.cos(angle) * 2, ground + 0.4, at.z + Math.sin(angle) * 2],
        velocity: [Math.cos(angle) * rand(14, 24), rand(0.5, 2.5), Math.sin(angle) * rand(14, 24)],
        life: rand(1, 1.8), size: [1.5, rand(6, 10)], color: 0xc9bfa8, alpha: 0.35, drag: 1.8, shape: 'puff',
      })
    }
  }
}

/**
 * The ground thrown up where a Titan comes down hard: a ring of dust running out to `radius` metres
 * from `at`, and a column of it over the spot.
 */
export function slamDust(world: FxWorld, at: readonly [number, number, number], radius: number) {
  const ring = Math.round(60 + radius * 3)
  for (let i = 0; i < ring; i++) {
    const angle = (i / ring) * Math.PI * 2 + rand(-0.05, 0.05)
    const [cos, sin] = [Math.cos(angle), Math.sin(angle)]
    const start = rand(4, 9)
    const [x, z] = [at[0] + cos * start, at[2] + sin * start]
    // Fast enough to reach the edge before the drag has stopped it.
    const speed = radius * rand(1.6, 2.4)
    world.smoke.spawn({
      position: [x, world.groundAt(x, z) + rand(0.3, 1.5), z],
      velocity: [cos * speed, rand(1, 6), sin * speed],
      life: rand(1.6, 2.8), size: [3, rand(9, 15)], color: 0xb8ad96, colorEnd: 0xd6cfbf, alpha: 0.45, drag: 2, gravity: -0.6, shape: 'puff',
    })
  }
  for (let i = 0; i < 40; i++) {
    const angle = rand(0, Math.PI * 2)
    const out = rand(0, 7)
    world.smoke.spawn({
      position: [at[0] + Math.cos(angle) * out, at[1] + rand(0, 2), at[2] + Math.sin(angle) * out],
      velocity: [Math.cos(angle) * rand(2, 8), rand(10, 28), Math.sin(angle) * rand(2, 8)],
      life: rand(1.5, 3), size: [3, rand(8, 14)], color: 0xa89d88, colorEnd: 0xcfc8b8, alpha: 0.5, drag: 1.6, gravity: 2, shape: 'puff',
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
export function coldGas(world: FxWorld, jets: readonly { at: Vector3; direction: Vector3 }[], velocity: Vector3, dt: number) {
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
