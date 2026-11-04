import { Quaternion, Vector3 } from '@ragnarok/engine'
import type { Node, PointLights } from '@ragnarok/engine'
import { ARMS_CUT, DAMAGE_WHEELS, KIND_SHIP, KIND_SHIP_GUN } from '@/net/codec'
import type { Debris } from '@/render/debris'
import type { Effects } from '@/render/effects'
import type { Rounds } from '@/render/rounds'
import { HenModel } from '@/render/hen'
import { VehicleFx, type FxWorld } from '@/render/tyres'
import { kindView } from '@/kinds'
import { TankModel } from '@/render/tank'
import { WalkerModel } from '@/render/walker'
import type { VehicleModel } from '@/render/car'
import type { DrivenPose } from '@/sim/driver'
import type { VehiclePose } from '@/sim/motor'
import { SHIP_DECK_Y, SHIP_GUN_HALF_EXTENTS, SHIP_GUN_X, SHIP_GUN_Z, vehicleMaxHp } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'
import type { VehicleSounds } from '@/vehicleaudio'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Arms, Entity, Pintle, Turret } from '../units'
import type { Seating } from './seating'

/** Where a ship's guns stand on her deck, in her frame (`SHIP_GUN_SLOTS`), and which way each faces. */
const GUN_SLOTS = [1, -1].flatMap((side) =>
  SHIP_GUN_Z.map((z) => ({ at: new Vector3(side * SHIP_GUN_X, SHIP_DECK_Y + SHIP_GUN_HALF_EXTENTS[1], z), turn: new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), -side * Math.PI / 2) })),
)
/** A gun further than this from any of a ship's slots, metres, is not on her. */
const SLOT_REACH = 1.5

/** Damage parts of a vehicle that is not hurt. */
const UNHURT: readonly number[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]

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
  turret: Turret | null
  /** The roof gun, on a tank with one. */
  pintle: Pintle | null
  arms: Arms | null
  hp: number
  /** `VehicleState::damage`'s parts; all 0 while unhurt. */
  damage: readonly number[]
  /** `THRUST_*` nozzles firing, as drawn: the driver's own at once, from their prediction. */
  jets: number
  /** Air, stamina or battery left, 0..1, if it has any. */
  air: number | null
}

/** The vehicle the local player drives, which is drawn where it is predicted. */
export interface Driven {
  id: number
  pose: DrivenPose
  /** Its `THRUST_*` nozzles, as predicted. */
  jets: number
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

  constructor(private readonly world: VehicleWorld, private readonly seating: Seating) {}

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

  private draw(id: number, { entity }: Sampled, frame: Frame) {
    const kind = entity.kind!.kind
    let model = this.models.get(id)
    if (!model) {
      model = kindView(kind).model(id)
      this.models.set(id, model)
      this.world.root.add(model.root)
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
    const [x, y, z] = position
    // A hen draws its wings and legs from its flags: the driver's own as predicted, at once.
    if (model instanceof HenModel) model.setFlags(jets)
    model.update(position, [rotation[0], rotation[1], rotation[2], rotation[3]], steer, seats, turret, arms)
    if (model instanceof TankModel) model.setPintle(pintle)
    if (model instanceof WalkerModel || model instanceof HenModel) for (const step of model.takeSteps()) this.world.sounds.footstep(step.at, step.weight)
    if (model instanceof HenModel) {
      const { beats, leapt, eggs } = model.takeEvents()
      if (leapt) this.world.sounds.bawk([x, y + 2, z])
      for (let k = 0; k < Math.min(beats, 2); k++) this.world.sounds.wingBeat([x, y, z])
      if (eggs > 0) this.world.sounds.plop([x, y - 1, z])
    }
    const placed: Placed = {
      kind, x, y, z, yaw: Math.atan2(-forward.x, -forward.z), rotation, steer, seats, turret, pintle, arms,
      hp: entity.health?.hp ?? 0, damage: entity.damage?.parts ?? UNHURT, jets, air: entity.thrust?.air ?? null,
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
        powered,
        hp,
        maxHp: vehicleMaxHp(kind),
        slip: fx.slip,
        surface: fx.surfaceUnder,
        grounded: fx.grounded,
        wheels: hurt ? damage.slice(DAMAGE_WHEELS, DAMAGE_WHEELS + 4) : null,
        cut,
        jets: placed.jets,
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
