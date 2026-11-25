import { Quaternion, Vector3 } from '@ragnarok/engine'
import type { Node, PointLights } from '@ragnarok/engine'
import { ARMS_CUT, DAMAGE_WHEELS, KIND_MISSILE, KIND_SHIP, KIND_SHIP_GUN, KIND_TITAN, THRUST_LIFT } from '@/net/codec'
import type { Debris } from '@/render/debris'
import type { Effects } from '@/render/effects'
import type { Rounds } from '@/render/rounds'
import type { Flinch } from '@/render/flinch'
import { HenModel } from '@/render/hen'
import { RotorModel } from '@/render/rotor'
import { VehicleFx, slamDust, type FxWorld } from '@/render/tyres'
import { kindView } from '@/kinds'
import { missileLaunch, tubeSmoke } from '@/render/launch'
import { splash } from '@/render/splash'
import { SubmarineModel, TUBE_SMOKES, type SubView } from '@/render/submarine'
import { TankModel } from '@/render/tank'
import { TitanModel } from '@/render/titan'
import { WalkerModel } from '@/render/walker'
import type { VehicleModel } from '@/render/car'
import type { DrivenPose } from '@/sim/driver'
import type { VehiclePose } from '@/sim/motor'
import { SHIP_DECK_Y, SHIP_GUN_HALF_EXTENTS, SHIP_GUN_X, SHIP_GUN_Z, TITAN_RIDE_HEIGHT, TITAN_SLAM_FULL, TITAN_SLAM_RADIUS, TITAN_SLAM_RADIUS_FULL, TITAN_SLAM_SPEED, vehicleMaxHp } from '@/sim/consts'
import { isRotorcraft, vehicleDef } from '@/sim/vehicles'
import type { VehicleSounds } from '@/vehicleaudio'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Arms, Entity, Pintle, Sub, Turret } from '../units'
import type { Seating } from './seating'

/** Where a ship's guns stand on her deck, in her frame (`SHIP_GUN_SLOTS`), and which way each faces. */
const GUN_SLOTS = [1, -1].flatMap((side) =>
  SHIP_GUN_Z.map((z) => ({ at: new Vector3(side * SHIP_GUN_X, SHIP_DECK_Y + SHIP_GUN_HALF_EXTENTS[1], z), turn: new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), -side * Math.PI / 2) })),
)
/** A gun further than this from any of a ship's slots, metres, is not on her. */
const SLOT_REACH = 1.5

/** How hard a Titan's footfall is heard: as a landing at this many m/s. */
const TITAN_FOOTFALL = 12
/** How far round a Titan's sole is in the water, metres: about half its 6.4 by 8.2 m. */
const TITAN_SOLE = 3.5
/** How fast a Titan's foot comes down into the water, m/s, for the height of its splash. */
const TITAN_STOMP = 12
/** How far round a walker's foot and a hen's are in the water, metres. */
const WALKER_FOOT = 0.7
const HEN_FOOT = 0.4

/** Damage parts of a vehicle that is not hurt. */
const UNHURT: readonly number[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
/** Doors all shut and on. */
const SHUT = { open: 0, gone: 0 }

/** A vehicle as drawn this frame: where (predicted, for the one the local player drives), and its parts. */
export interface Placed {
  kind: number
  x: number
  y: number
  z: number
  /** Heading of the nose, soldier convention. */
  yaw: number
  rotation: readonly [number, number, number, number]
  steer: number
  /** The player in each seat, 0 for an empty one (`Seating`). */
  seats: readonly number[]
  /** A car of the traffic with its driver at the wheel: boarding it drags them out. */
  manned?: boolean
  turret: Turret | null
  /** The roof gun, on a tank with one. */
  pintle: Pintle | null
  arms: Arms | null
  /** A submarine's periscope and tubes. */
  sub: Sub | null
  hp: number
  /** `VehicleState::damage`'s parts; all 0 while unhurt. */
  damage: readonly number[]
  /** Its doors standing open and come off, by their index (`Doors`, DOORS.md). */
  doors: { open: number; gone: number }
  /** `THRUST_*` nozzles firing, as drawn: the driver's own at once, from their prediction. */
  jets: number
  /** Air, stamina or battery left, 0..1, if it has any. */
  air: number | null
  /** How hard its driver works the engine, -1..1 (`Steer.throttle`). */
  throttle: number
}

/** The vehicle the local player drives, which is drawn where it is predicted. */
export interface Driven {
  id: number
  pose: DrivenPose
  /** Its `THRUST_*` nozzles, as predicted. */
  jets: number
  /** How hard its driver works the engine, -1..1, from their newest input. */
  throttle: number
}

/** What vehicles are drawn into, and the local player's part in them. */
export interface VehicleWorld {
  root: Node
  sounds: VehicleSounds
  effects: Effects
  /** Machine gun rounds in flight. */
  rounds: Rounds
  debris: Debris
  lights: PointLights
  fx: () => FxWorld
  /** How far a ray goes before it meets something, if it does within `max`. */
  castRay: (origin: [number, number, number], direction: [number, number, number], max: number) => { t: number } | null
  /** The vehicle the local player drives this frame, if any. */
  driven: () => Driven | null
  /** The vehicle the local player sits in, if any. */
  seated: () => number | null
  /** Puts the ears where the camera is, before the engines are heard. */
  listen: () => void
  /** The local player's gun went off. */
  ownShot: () => void
  /** Something heavy came down at `at`: shakes the view by up to `strength` if it is within a few `radius` of it. */
  shake?: (at: readonly [number, number, number], radius: number, strength: number) => void
  /** Vehicles rocking from the rounds that strike them, as drawn. */
  flinch?: Flinch
}

/**
 * Every vehicle: its model at its pose, its tyres' marks, smoke and spray, its engine's sound, and the
 * steps of anything that walks. What it carries is drawn by the presenters for those components
 * (`guns.ts`, `wear.ts`, `smoke.ts`, `jets.ts`), which run after this one and find the model and its
 * motion here.
 */
export class Vehicles implements Presenter<Frame> {
  /** This frame's vehicles, by id. */
  readonly placed = new Map<number, Placed>()
  private readonly models = new Map<number, VehicleModel>()
  private readonly motion = new Map<number, VehicleFx>()
  /** Which ship each of her guns stands on, and at which slot, once found. */
  private readonly decks = new Map<number, { ship: number; slot: number }>()
  /** Ships' guns waiting to be drawn this frame, once their ships are. */
  private readonly guns: [number, Sampled][] = []

  /** Where each Titan's soles were last frame, for how fast they swing through the water. */
  private readonly soles = new Map<number, Vector3[]>()

  constructor(private readonly world: VehicleWorld, private readonly seating: Seating) {}

  /**
   * A foot coming down at `at` (its sole, metres) into water, if there is any over it: a splash of a
   * foot `size` metres round coming down at `speed` m/s, and the ring it pushes out.
   */
  private stepIn(at: readonly [number, number, number], size: number, speed: number) {
    const fx = this.world.fx()
    const level = fx.waterAt?.(at[0], at[2])
    if (level === undefined || !Number.isFinite(level) || level <= at[1]) return
    splash(fx, { x: at[0], y: level, z: at[2] }, size, speed)
    // How deep it plunges, up to its own size: a Titan's foot shoves a wall of water out.
    fx.waves?.splash(at[0], at[2], size, Math.min(size * 0.4, level - at[1], 1.5), 1)
  }

  /**
   * A Titan's feet wading: each sole under the surface is a hull pushed through the water as it swings,
   * so a stride through a lake shoves a bow wave ahead of the foot and lifts the water as it comes out.
   */
  private wade(id: number, model: TitanModel, dt: number) {
    const fx = this.world.fx()
    const soles = model.soles()
    const last = this.soles.get(id)
    this.soles.set(id, soles)
    if (!fx.waves || !last || dt <= 0) return
    soles.forEach((sole, foot) => {
      const level = fx.waterAt?.(sole.x, sole.z)
      if (level === undefined || !Number.isFinite(level) || level <= sole.y) return
      const before = last[foot]!
      const velocity = [(sole.x - before.x) / dt, (sole.z - before.z) / dt] as const
      fx.waves!.hull(`titan ${id} foot ${foot}`, {
        x: sole.x, z: sole.z, ahead: [velocity[0], velocity[1]], halfWidth: TITAN_SOLE, halfLength: TITAN_SOLE,
        depth: Math.min(1.5, level - sole.y), velocity, churn: 1,
      })
    })
  }

  /** Vehicle `id`'s model, while it is drawn. */
  model(id: number): VehicleModel | undefined {
    return this.models.get(id)
  }

  /** How vehicle `id` moves as drawn, and what its wheels touch. */
  fx(id: number): VehicleFx | undefined {
    return this.motion.get(id)
  }

  wants(entity: Entity) {
    return entity.orientation !== undefined && entity.place !== undefined && entity.kind !== undefined
  }

  begin() {
    this.placed.clear()
    this.world.listen()
    this.world.sounds.begin()
  }

  update(id: number, sampled: Sampled, frame: Frame) {
    // A ship's gun is drawn on her deck as she is drawn: after her, in `end`.
    if (sampled.entity.kind!.kind === KIND_SHIP_GUN) this.guns.push([id, sampled])
    else this.draw(id, sampled, frame)
  }

  /**
   * Where gun `id`, drawn from its snapshots at `position`, stands on its ship as she is drawn this
   * frame: her pose and its slot on her deck. Her own pose runs ahead of her guns' snapshots wherever
   * she is predicted (her helmsman's), and theirs are interpolated apart from hers, so drawn from their
   * own they would lag behind their ports at speed.
   */
  private onDeck(id: number, position: readonly [number, number, number]): { position: [number, number, number]; rotation: readonly [number, number, number, number] } | null {
    let deck = this.decks.get(id)
    if (!deck || this.placed.get(deck.ship)?.kind !== KIND_SHIP) {
      deck = undefined
      const at = new Vector3(...position)
      for (const [ship, placed] of this.placed) {
        if (placed.kind !== KIND_SHIP) continue
        const local = at.clone().sub(new Vector3(placed.x, placed.y, placed.z)).applyQuaternion(new Quaternion(...placed.rotation).invert())
        const slot = GUN_SLOTS.findIndex((slot) => slot.at.distanceTo(local) < SLOT_REACH)
        if (slot >= 0) deck = { ship, slot }
      }
      if (!deck) return null
      this.decks.set(id, deck)
    }
    const ship = this.placed.get(deck.ship)!
    const turn = new Quaternion(...ship.rotation)
    const { at, turn: facing } = GUN_SLOTS[deck.slot]!
    const spot = at.clone().applyQuaternion(turn)
    const q = turn.multiply(facing)
    return { position: [ship.x + spot.x, ship.y + spot.y, ship.z + spot.z], rotation: [q.x, q.y, q.z, q.w] }
  }

  /** What a submarine's raised periscope is seen from, to glint and throw its feather (`SubmarineModel.setSub`). */
  private subView(): SubView {
    const fx = this.world.fx()
    return { camera: fx.camera, waterAt: (x, z) => fx.waterAt?.(x, z) ?? Number.NaN }
  }

  /**
   * A submarine's tubes as they fire: the fire and smoke of each launch, its light and its roar, and
   * the smoke a tube goes on giving off afterwards. A tube under the water breaks the surface over it.
   */
  private launches(model: SubmarineModel, dt: number) {
    const fx = this.world.fx()
    const over = ({ x, y, z }: { x: number; y: number; z: number }) => {
      const level = fx.waterAt?.(x, z) ?? Number.NaN
      return level > y ? level : null
    }
    for (const tube of model.takeLaunches()) {
      const { at, up } = model.tubeMouth(tube)
      const surface = over(at)
      missileLaunch(fx, at, up, surface)
      const y = surface ?? at.y
      this.world.lights.flash(at.x, y + 1.5, at.z, { color: 0xffa040, intensity: 7, radius: 34, life: 0.5 })
      this.world.sounds.blast([at.x, y, at.z], 'small')
    }
    model.smoking.forEach((left, tube) => {
      if (left <= 0) return
      const { at, up } = model.tubeMouth(tube)
      if (over(at) === null) tubeSmoke(fx, at, up, left / TUBE_SMOKES, dt)
    })
  }

  private draw(id: number, { entity }: Sampled, frame: Frame) {
    const kind = entity.kind!.kind
    let model = this.models.get(id)
    if (!model) {
      model = kindView(kind).model(id)
      this.models.set(id, model)
      this.world.root.add(model.root)
      // A guided missile is a vehicle of its own, born at its launch: a Titan's shoulder tubes send no
      // shot, so it is heard leaving when it first appears (a submarine's tube also blasts, `launches`).
      if (kind === KIND_MISSILE) this.world.sounds.launch([entity.place!.x, entity.place!.y, entity.place!.z], 'rocket')
    }
    const driven = this.world.driven()
    const mine = driven !== null && driven.id === id ? driven : null
    const place = entity.place!
    const q = entity.orientation!
    const deck = kind === KIND_SHIP_GUN ? this.onDeck(id, [place.x, place.y, place.z]) : null
    const position: [number, number, number] = deck ? deck.position : mine ? [...mine.pose.position] : [place.x, place.y, place.z]
    const rotation = deck ? deck.rotation : mine ? mine.pose.rotation : ([q.qx, q.qy, q.qz, q.qw] as const)
    const steer = mine ? mine.pose.steer : (entity.steer?.steer ?? 0)
    const forward = new Vector3(0, 0, -1).applyQuaternion(new Quaternion(...rotation))
    const seats = this.seating.of(id, kind)
    const turret = entity.turret ?? null
    const pintle = entity.pintle ?? null
    const arms = entity.arms ?? null
    const jets = mine ? mine.jets : (entity.thrust?.flags ?? 0)
    const throttle = mine ? mine.throttle : (entity.steer?.throttle ?? 0)
    const [x, y, z] = position
    // A hen draws its wings and legs from its flags: the driver's own as predicted, at once.
    if (model instanceof HenModel) model.setFlags(jets)
    // A helicopter's rotor turns as fast as the physics has it turning: spinning up, drooping or autorotating.
    if (model instanceof RotorModel) model.setRotor(entity.thrust?.air ?? null)
    // A Titan's limbs are where the server has them.
    if (model instanceof TitanModel) model.setLimbs(entity.titan ?? null)
    // Rocked by the rounds striking it, about the ground under it; a Titan's legs bend to keep its feet
    // where they stand. Not the Titan its own pilot looks out of: the visor would swing through the view.
    const drawnAt: [number, number, number] = [x, y, z]
    const drawnTurn: [number, number, number, number] = [rotation[0], rotation[1], rotation[2], rotation[3]]
    if (!(mine && kind === KIND_TITAN)) this.world.flinch?.pose(id, drawnAt, drawnTurn, kind === KIND_TITAN ? TITAN_RIDE_HEIGHT : vehicleDef(kind).halfExtents[1], frame.dt)
    model.update(drawnAt, drawnTurn, steer, seats, turret, arms)
    if (model instanceof TankModel) model.setPintle(pintle)
    if (model instanceof SubmarineModel) {
      model.setSub(entity.sub ?? null, frame.dt, this.subView())
      this.launches(model, frame.dt)
    }
    // A missile's motor lights what it flies over while it burns.
    if (kind === KIND_MISSILE && (seats[0] ?? 0) !== 0) this.world.lights.hold(x, y, z, { color: 0xff9a40, intensity: 3.5, radius: 22 })
    if (model instanceof WalkerModel || model instanceof HenModel) {
      for (const step of model.takeSteps()) {
        this.world.sounds.footstep(step.at, step.weight)
        this.stepIn(step.at, model instanceof WalkerModel ? WALKER_FOOT : HEN_FOOT, 2 + 3 * step.weight)
      }
    }
    // A Titan's foot comes down like something dropped from a height.
    if (model instanceof TitanModel) {
      for (const step of model.takeSteps()) {
        this.world.sounds.thud(step.at, TITAN_FOOTFALL)
        this.stepIn(step.at, TITAN_SOLE, TITAN_STOMP)
      }
      this.wade(id, model, frame.dt)
      // Come down hard out of the air, the ground goes up around it and everything near shakes.
      const slam = model.takeSlam()
      // Come down in water, both feet throw it up and out.
      if (slam) this.stepIn(slam.at, TITAN_SOLE * 2, slam.speed)
      if (slam && slam.speed >= TITAN_SLAM_SPEED) {
        const power = Math.min(1, (slam.speed - TITAN_SLAM_SPEED) / (TITAN_SLAM_FULL - TITAN_SLAM_SPEED))
        const radius = TITAN_SLAM_RADIUS + (TITAN_SLAM_RADIUS_FULL - TITAN_SLAM_RADIUS) * power
        this.world.sounds.thud(slam.at, 3 * slam.speed)
        this.world.sounds.explosion(slam.at)
        slamDust(this.world.fx(), slam.at, radius)
        this.world.shake?.(slam.at, radius, 0.3 + 0.5 * power)
      } else if (slam) this.world.sounds.thud(slam.at, 2 * TITAN_FOOTFALL)
    }
    if (model instanceof HenModel) {
      const { beats, leapt, eggs } = model.takeEvents()
      if (leapt) this.world.sounds.bawk([x, y + 2, z])
      for (let k = 0; k < Math.min(beats, 2); k++) this.world.sounds.wingBeat([x, y, z])
      if (eggs > 0) this.world.sounds.plop([x, y - 1, z])
    }
    const placed: Placed = {
      kind, x, y, z, yaw: Math.atan2(-forward.x, -forward.z), rotation, steer, seats, manned: this.seating.manned(id), turret, pintle, arms, sub: entity.sub ?? null,
      hp: entity.health?.hp ?? 0, damage: entity.damage?.parts ?? UNHURT, doors: entity.doors ?? SHUT, jets, air: entity.thrust?.air ?? null, throttle,
    }
    this.placed.set(id, placed)
    this.tyres(id, placed, frame.dt, entity.arms ? (entity.arms.flags & ARMS_CUT) !== 0 : false)
  }

  end(frame: Frame) {
    for (const [id, sampled] of this.guns) this.draw(id, sampled, frame)
    this.guns.length = 0
    this.world.sounds.end(frame.dt)
  }

  remove(id: number) {
    this.decks.delete(id)
    this.models.get(id)?.dispose()
    this.models.delete(id)
    this.motion.get(id)?.lift(this.world.fx())
    this.motion.delete(id)
    this.soles.delete(id)
    this.world.sounds.drop(id)
  }

  /** Skidmarks, tyre smoke, dust and exhaust for a vehicle drawn at its pose, and its engine heard. */
  private tyres(id: number, placed: Placed, dt: number, cut: boolean) {
    const { kind, seats, hp, damage } = placed
    let fx = this.motion.get(id)
    if (!fx || fx.kind !== kind) {
      fx = new VehicleFx(id, kind)
      this.motion.set(id, fx)
    }
    const position: [number, number, number] = [placed.x, placed.y, placed.z]
    // A towed gun has no engine, whoever sits at it.
    const powered = (seats[0] ?? 0) !== 0 && hp > 0 && !vehicleDef(kind).towed
    fx.update(this.world.fx(), position, placed.rotation, placed.steer, powered, dt)
    // The driver's own car is drawn from its prediction, so what is heard follows the wheel at once.
    const landing = fx.takeLanding()
    if (landing > 0) this.world.sounds.thud(position, landing)
    const hurt = damage.some((wear) => wear > 0)
    this.world.sounds.engine(
      id,
      {
        kind,
        speed: fx.forwardSpeed,
        accel: fx.acceleration,
        climb: fx.velocity.y,
        throttle: placed.throttle,
        powered,
        hp,
        maxHp: vehicleMaxHp(kind),
        slip: fx.slip,
        ground: fx.ground,
        // Afloat is not flying: a plane down in the water hears the water, not the wind in its wires.
        grounded: fx.grounded || fx.ground === 'water',
        submerged: fx.submerged,
        yawRate: fx.yawRate,
        wheels: hurt ? damage.slice(DAMAGE_WHEELS, DAMAGE_WHEELS + 4) : null,
        cut,
        // A Titan in the air with its thrusters off is silent.
        jets: kind === KIND_TITAN ? placed.jets & THRUST_LIFT : placed.jets,
        rotor: isRotorcraft(kind) ? (placed.air ?? 0) : undefined,
      },
      position,
      [fx.velocity.x, fx.velocity.y, fx.velocity.z],
      this.world.seated() === id,
    )
  }
}

/** Vehicle `id` as the newest snapshot has it, for the prediction of the soldier and of its driver. */
export function poseOf(id: number, entity: Entity): VehiclePose {
  const { x, y, z } = entity.place!
  const { qx, qy, qz, qw } = entity.orientation!
  return { id, kind: entity.kind?.kind ?? 0, x, y, z, qx, qy, qz, qw, hp: entity.health?.hp ?? 0, damage: entity.damage?.parts ?? UNHURT, air: entity.thrust?.air ?? null }
}
