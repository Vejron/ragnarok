import { Group, Mesh, type Material, type Node, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, THRUST_LIFT, wrapAngle } from '@/net/codec'
import { TITAN_ANKLE_HEIGHT, TITAN_ELBOW_POLE, TITAN_EYE, TITAN_FIST_REST, TITAN_FOOT_HALF, TITAN_FOREARM_LENGTH, TITAN_GUN_BARRELS, TITAN_GUN_HAND, TITAN_HEAD_DOWN, TITAN_HEAD_PIVOT, TITAN_HEAD_SLEW, TITAN_HEAD_TURN, TITAN_HEAD_UP, TITAN_KNEE_POLE, TITAN_SHIN_LENGTH, TITAN_THIGH_LENGTH, TITAN_UPPER_ARM_LENGTH, TITAN_HALF_EXTENTS, TITAN_RIDE_HEIGHT, TITAN_SHOULDER, TITAN_STEP_LIFT } from '@/sim/consts'
import type { Titan } from '@/entities/units'
import { clamp } from '@/math'
import type { VehicleModel } from './car'

/** Paint colours handed out by vehicle id, so two Titans are told apart. */
/** A Titan's two colours, its armour and the second colour under it: which pair it wears goes by its id. */
const PAINTS = [
  [0xd99a1c, 0x1c3a78],
  [0xb8402a, 0x2c3138],
  [0xd8d4c8, 0x2f5d3a],
  [0x8a9aa8, 0x7a2f2a],
] as const
export const TITAN_MODEL_URL = `${import.meta.env.BASE_URL}models/titan.glb`
/**
 * Upper arm and forearm, thigh and shin, and how far the ankle is above the sole, metres: the server's,
 * whose hit shapes are bent by the same IK (`UPPER`, `FORE`, `THIGH`, `SHIN`, `PAD` in build_titan.py).
 */
const UPPER = TITAN_UPPER_ARM_LENGTH
const FORE = TITAN_FOREARM_LENGTH
const THIGH = TITAN_THIGH_LENGTH
const SHIN = TITAN_SHIN_LENGTH
const PAD = TITAN_ANKLE_HEIGHT
/** A `titan` component's `step` with both feet off the ground (`Step::Airborne` on the server). */
export const STEP_AIRBORNE = 203

/**
 * The foot (0 left) a `titan` component's `step` has in the air, and how far through its step it is,
 * 0..1; null with both on the ground (`Step::from_byte` on the server).
 */
export function swinging(step: number): { foot: number; progress: number } | null {
  if (step >= 1 && step <= 100) return { foot: 0, progress: (step - 1) / 99 }
  if (step >= 101 && step <= 200) return { foot: 1, progress: (step - 101) / 99 }
  return null
}

let asset: Node | null = null

/**
 * Parses `titan.glb` (built by `tools/vehicles/build_titan.py`) for every `TitanModel` to share. Its
 * materials become flat toon ones to match the rest of the scene (`lamp` and `visor` ones are unlit); `paint` and `trim` are
 * replaced per Titan. Its parts are not merged: each limb is placed by itself.
 */
export async function parseTitanModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' || source.name === 'visor' ? unlitMaterial(source) : null))
}

export async function loadTitanModel(url = TITAN_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseTitanModel(await response.arrayBuffer())
}

const DOWN = new Vector3(0, -1, 0)
const scratch = { along: new Vector3(), side: new Vector3(), x: new Vector3(), y: new Vector3(), z: new Vector3(), basis: new Matrix4() }

/**
 * Two-bone IK: where the joint between a bone `a` long from `root` and one `b` long to `target` is,
 * bent towards `pole`. A target out of the bones' reach is brought to the end of it, and `end` is
 * where the second bone then ends.
 */
export function bend(root: Vector3, target: Vector3, a: number, b: number, pole: Vector3, joint: Vector3, end: Vector3) {
  const { along, side } = scratch
  along.subVectors(target, root)
  const reach = Math.min(Math.max(along.length(), Math.abs(a - b) + 0.01), a + b - 0.01)
  if (along.lengthSq() < 1e-8) along.set(0, -1, 0)
  along.normalize()
  // How far along the line the joint is, and how far off it.
  const foot = (a * a - b * b + reach * reach) / (2 * reach)
  const off = Math.sqrt(Math.max(0, a * a - foot * foot))
  side.copy(pole).addScaledVector(along, -pole.dot(along))
  if (side.lengthSq() < 1e-6) side.set(1, 0, 0).addScaledVector(along, -along.x)
  side.normalize()
  joint.copy(root).addScaledVector(along, foot).addScaledVector(side, off)
  end.copy(root).addScaledVector(along, reach)
}

/**
 * Turns `node`, whose part hangs down its own -Y with its front to -Z, to hang from `from` towards `to`
 * with its back as near `back` as that allows.
 */
export function hang(node: Node, from: Vector3, to: Vector3, back: Vector3) {
  const { x, y, z, basis } = scratch
  y.subVectors(from, to)
  if (y.lengthSq() < 1e-8) y.copy(DOWN).negate()
  y.normalize()
  z.copy(back).addScaledVector(y, -back.dot(y))
  if (z.lengthSq() < 1e-6) z.set(0, 1, 0).addScaledVector(y, -y.y)
  z.normalize()
  x.crossVectors(y, z)
  node.position.copy(from)
  node.quaternion.setFromRotationMatrix(basis.makeBasis(x, y, z).elements)
}

/** How fast a round's blow knocks a Titan's fist from where it is held, m/s for a shell through the forearm; how fast the arm swings back, Hz, how much of each swing is lost, and the furthest it is knocked, metres. */
const ARM_KNOCK = 9
const ARM_SWAY_HZ = 1.4
const ARM_SWAY_DAMPING = 0.3
const ARM_KNOCK_MOST = 3
/** How fast a round leaving the gun kicks its fist back along the forearm, m/s. */
const GUN_KICK = 6
/** How quickly the barrels come round to the next one after a round, per second: most of the way in a tenth of a second. */
const BARREL_TURN_RATE = 25

/** How far ahead of the pilot's eye and below it the searchlight shines from, metres. */
const LAMP_AHEAD = 0.9
const LAMP_BELOW = 0.4

/**
 * How a Titan's head is turned from its hull, `[yaw, pitch]` in radians (+ left, + up), to follow a look
 * along `yaw` and `pitch` in the world from a hull turned by `rotation`: within what its neck allows.
 * The server's `titans::turn_head` turns it the same way.
 */
export function headTurn(yaw: number, pitch: number, rotation: readonly [number, number, number, number]): [number, number] {
  const flat = Math.cos(pitch)
  const look = new Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat).applyQuaternion(new Quaternion(...rotation).invert())
  return [clamp(Math.atan2(-look.x, -look.z), -TITAN_HEAD_TURN, TITAN_HEAD_TURN), clamp(Math.asin(clamp(look.y, -1, 1)), -TITAN_HEAD_DOWN, TITAN_HEAD_UP)]
}

/** A head turned by `[yaw, pitch]` from the hull, as a rotation in the hull's frame. */
const headTurned = ([yaw, pitch]: readonly [number, number]) => new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), yaw).multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), pitch))

/**
 * Where the pilot's eye is in the hull's frame when the head is turned by `head` (`headTurn`): `TITAN_EYE`,
 * and `ahead` metres out of the visor from it, carried round the neck with the head.
 */
export function headEye(head: readonly [number, number], ahead = 0): Vector3 {
  const pivot = new Vector3(...TITAN_HEAD_PIVOT)
  return new Vector3(TITAN_EYE[0], TITAN_EYE[1], TITAN_EYE[2] - ahead).sub(pivot).applyQuaternion(headTurned(head)).add(pivot)
}

/**
 * A Titan: its torso on the hull, its head turned to where its pilot looks, and its arms and legs bent by IK to where the server has its fists
 * and feet (`Titan`). Nothing here decides where a limb is: with no word from the server yet the arms
 * hang and the legs stand straight under the hips. The mesh is `titan.glb`; its nodes are named in
 * `build_titan.py`.
 */
export class TitanModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint: Material
  private readonly trim: Material
  private readonly upper: Node[]
  private readonly fore: Node[]
  private readonly fist: Node[]
  private readonly open: Node[]
  private readonly closed: Node[]
  private readonly thigh: Node[]
  private readonly shin: Node[]
  private readonly foot: Node[]
  private readonly head: Node
  private limbs: Titan | null = null
  /** Where its own pilot looks, for the next frame drawn: the head's turn from the hull, and whether it is there at once. */
  private gaze: { turn: readonly [number, number]; snap: boolean } | null = null
  private readonly beam = { at: new Vector3(), direction: new Vector3() }
  /** How the head is turned from the hull as drawn, `[yaw, pitch]` in radians. */
  private readonly facing: [number, number] = [0, 0]
  /** Each fist's middle in the hull frame, as drawn. */
  private readonly fists = [new Vector3(), new Vector3()]
  /** The elbow of the arm that holds the gun (`TITAN_GUN_HAND`), in the hull frame, as drawn. */
  private readonly gunElbow = new Vector3(0, -1, 0)
  /** The rotary cannon's three barrels (`barrels_R`), and how far round they are still to turn, radians about the forearm's line. */
  private readonly barrels: Node | null
  private barrelTurn = 0
  /** Whether each foot was in the air a frame ago, and the way each points, radians about +Y: as it was put down. */
  private readonly stepping = [false, false]
  private readonly pointing = [0, 0]
  private seen = false
  /** Whether it was off the ground a frame ago, how fast it was coming down then, m/s, and when and how high it was. */
  private flying = false
  private fall = 0
  private stamp = 0
  private height = 0
  private slammed: { at: [number, number, number]; speed: number } | null = null
  private steps: { at: [number, number, number]; weight: number }[] = []
  /** How far each fist is knocked from where it is held by rounds striking its arm, metres in the hull frame, and how fast. */
  private readonly knocked = [new Vector3(), new Vector3()]
  private readonly knocking = [new Vector3(), new Vector3()]

  constructor(id: number) {
    if (!asset) throw new Error('loadTitanModel() must finish before Titans are created')
    const model = asset.clone()
    this.root.add(model)
    const [armour, under] = PAINTS[id % PAINTS.length]!
    this.paint = toonMaterial({ color: armour })
    this.trim = toonMaterial({ color: under })
    model.traverse((node) => {
      if (!(node instanceof Mesh)) return
      if (node.material.name === 'paint') node.material = this.paint
      else if (node.material.name === 'trim') node.material = this.trim
    })
    const pair = (name: string) => ['L', 'R'].map((label) => model.getObjectByName(`${name}_${label}`)!)
    this.upper = pair('upper')
    this.fore = pair('fore')
    this.fist = pair('fist')
    this.open = pair('open')
    this.closed = pair('closed')
    this.thigh = pair('thigh')
    this.shin = pair('shin')
    this.foot = pair('foot')
    this.head = model.getObjectByName('head')!
    this.barrels = model.getObjectByName('barrels_R') ?? null
  }

  /**
   * A round struck `node`, part of this model, going along `way` (unit, the hull's frame), with
   * `strength` (1 a shell through): an arm it struck swings back with the blow and comes back.
   */
  recoil(node: Node, way: Vector3, strength: number) {
    for (let at: Node | null = node; at && at !== this.root; at = at.parent) {
      for (let index = 0; index < 2; index++) {
        const parts = [this.upper[index], this.fore[index], this.fist[index]]
        if (!parts.includes(at)) continue
        // A blow on the forearm or the fist swings it further than one by the shoulder.
        const reach = at === this.upper[index] ? 0.5 : 1
        this.knocking[index]!.addScaledVector(way, ARM_KNOCK * strength * reach)
        return
      }
    }
  }

  /** Where the server has its fists and feet, or null before it has said. */
  setLimbs(limbs: Titan | null) {
    this.limbs = limbs
  }

  /**
   * Its own pilot looks so that the head is turned by `turn` (`headTurn`) in the next frame drawn, sooner
   * than the server says so. With `snap` the head is there at once, as it must be for a pilot looking out
   * of it; without, it turns as fast as the server turns it.
   */
  lookTo(turn: readonly [number, number], snap: boolean) {
    this.gaze = { turn, snap }
  }

  /**
   * The searchlight in its visor: from just ahead of the visor, the way the head faces, as of the last
   * `update`. The vectors are reused by the next call.
   */
  searchlight(): { at: Vector3; direction: Vector3 } {
    this.root.updateWorldMatrix(true, true)
    const world = new Matrix4().fromArray(this.head.matrixWorld)
    this.beam.direction.set(0, 0, -1).transformDirection(world)
    // The pilot's eye is at the visor: the lamp is a little ahead of it and below, out of their view.
    this.beam.at.set(TITAN_EYE[0] - TITAN_HEAD_PIVOT[0], TITAN_EYE[1] - TITAN_HEAD_PIVOT[1] - LAMP_BELOW, TITAN_EYE[2] - TITAN_HEAD_PIVOT[2] - LAMP_AHEAD).applyMatrix4(world)
    return this.beam
  }

  /** The middle of fist `hand` (0 left) in the world, as drawn. */
  fistWorld(hand: number): Vector3 {
    this.root.updateWorldMatrix(true, false)
    return this.fists[hand]!.clone().applyMatrix4(new Matrix4().fromArray(this.root.matrixWorld))
  }

  /**
   * The gun in its fist, as drawn: the fist's middle in the world, where its rounds leave from, and the
   * way the forearm points from the elbow, which they go.
   */
  barrel(): { origin: Vector3; direction: Vector3 } {
    this.root.updateWorldMatrix(true, false)
    const world = new Matrix4().fromArray(this.root.matrixWorld)
    const fist = this.fists[TITAN_GUN_HAND]!
    const origin = fist.clone().applyMatrix4(world)
    const direction = fist.clone().sub(this.gunElbow).normalize().applyQuaternion(this.root.quaternion)
    return { origin, direction }
  }

  /** The gun fist kicked back along its forearm by a round leaving, and its barrels turned on to the next. */
  kick() {
    const back = this.gunElbow.clone().sub(this.fists[TITAN_GUN_HAND]!).normalize()
    this.knocking[TITAN_GUN_HAND]!.addScaledVector(back, GUN_KICK)
    this.barrelTurn += (2 * Math.PI) / TITAN_GUN_BARRELS
  }

  /** The footfalls since the last call, for the thud that goes with each. */
  takeSteps() {
    const steps = this.steps
    this.steps = []
    return steps
  }

  /** The landing out of the air since the last call, if there was one: where, and how fast it came down, m/s. */
  takeSlam() {
    const slammed = this.slammed
    this.slammed = null
    return slammed
  }

  /** Where the middle of each sole is in the world, left then right, where it was last placed. */
  soles(): Vector3[] {
    this.root.updateWorldMatrix(true, false)
    const world = new Matrix4().fromArray(this.root.matrixWorld)
    return this.foot.map((foot) => new Vector3(foot.position.x, foot.position.y, foot.position.z).applyMatrix4(world).add(new Vector3(0, -PAD, 0)))
  }

  /** The thrusters firing for `flags` (`THRUST_*`), where it was last placed: one under each foot, blowing down. */
  jets(flags: number): { at: Vector3; direction: Vector3 }[] {
    if (!(flags & THRUST_LIFT)) return []
    this.root.updateWorldMatrix(true, false)
    const world = new Matrix4().fromArray(this.root.matrixWorld)
    return this.foot.map((foot) => ({ at: new Vector3(foot.position.x, foot.position.y, foot.position.z).applyMatrix4(world).add(new Vector3(0, -PAD, 0)), direction: DOWN.clone() }))
  }

  /** Places the Titan and bends its limbs to where they are. `steer`, `seats` and the rest are unused. */
  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const limbs = this.limbs
    const step = swinging(limbs?.step ?? 0)
    // Off the ground, how fast it is coming down is kept for when it lands.
    const now = performance.now()
    const dt = Math.max(0.004, (now - this.stamp) / 1000)
    const flying = limbs?.step === STEP_AIRBORNE
    if (flying && this.flying && dt < 0.25) this.fall = Math.max(0, (this.height - position[1]) / dt)
    if (!flying && this.flying && limbs) {
      this.slammed = { at: [position[0] + (limbs.flx + limbs.frx) / 2, position[1] + (limbs.fly + limbs.fry) / 2, position[2] + (limbs.flz + limbs.frz) / 2], speed: this.fall }
      this.fall = 0
    }
    this.flying = flying
    this.stamp = now
    // The barrels come round to the next one, turning about the forearm's line (the fist's Y).
    if (this.barrels) {
      const step = this.barrelTurn * (1 - Math.exp(-BARREL_TURN_RATE * dt))
      this.barrels.rotation.y = (this.barrels.rotation.y + step) % (2 * Math.PI)
      this.barrelTurn -= step
    }
    this.height = position[1]
    // The head: where the server has turned it, or for its own pilot where they look now.
    const gaze = this.gaze
    this.gaze = null
    const toward = gaze?.turn ?? [limbs?.head_yaw ?? 0, limbs?.head_pitch ?? 0]
    const most = gaze && !gaze.snap ? TITAN_HEAD_SLEW * Math.min(dt, 0.1) : Infinity
    for (const axis of [0, 1] as const) this.facing[axis] += clamp(toward[axis]! - this.facing[axis], -most, most)
    this.head.quaternion.copy(headTurned(this.facing))
    const flags = limbs?.flags ?? 0
    const turn = new Quaternion(...rotation)
    const unturn = turn.clone().invert()
    const [, hy] = TITAN_HALF_EXTENTS
    const back = new Vector3(0, 0, 1)
    const joint = new Vector3()
    const end = new Vector3()
    for (let index = 0; index < 2; index++) {
      const side = index === 0 ? -1 : 1
      // The arm: shoulder to the fist, the elbow out, down and back.
      const shoulder = new Vector3(side * TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2])
      const wanted = limbs
        ? new Vector3(index === 0 ? limbs.lx : limbs.rx, index === 0 ? limbs.ly : limbs.ry, index === 0 ? limbs.lz : limbs.rz)
        : new Vector3(side * TITAN_FIST_REST[0], TITAN_FIST_REST[1], TITAN_FIST_REST[2])
      // Knocked by the rounds striking the arm: a damped spring back to where it is held.
      const knocked = this.knocked[index]!
      const knocking = this.knocking[index]!
      const w = 2 * Math.PI * ARM_SWAY_HZ
      knocking.addScaledVector(knocked, -w * w * Math.min(dt, 0.05)).multiplyScalar(Math.max(0, 1 - 2 * ARM_SWAY_DAMPING * w * Math.min(dt, 0.05)))
      knocked.addScaledVector(knocking, Math.min(dt, 0.05))
      if (knocked.length() > ARM_KNOCK_MOST) knocked.setLength(ARM_KNOCK_MOST)
      wanted.add(knocked)
      bend(shoulder, wanted, UPPER, FORE, new Vector3(side * TITAN_ELBOW_POLE[0], TITAN_ELBOW_POLE[1], TITAN_ELBOW_POLE[2]), joint, end)
      hang(this.upper[index]!, shoulder, joint, back)
      hang(this.fore[index]!, joint, end, back)
      hang(this.fist[index]!, end, end.clone().addScaledVector(end.clone().sub(joint).normalize(), 1), back)
      this.fists[index]!.copy(end)
      if (index === TITAN_GUN_HAND) this.gunElbow.copy(joint)
      const gripping = (flags & (index === 0 ? HANDS_GRIP_LEFT : HANDS_GRIP_RIGHT)) !== 0
      this.open[index]!.visible = !gripping
      this.closed[index]!.visible = gripping

      // The leg: hip to the ankle over where the foot stands, the knee forward.
      const hip = new Vector3(side * TITAN_FOOT_HALF, -hy, 0)
      const sole = limbs
        ? new Vector3(position[0] + (index === 0 ? limbs.flx : limbs.frx), position[1] + (index === 0 ? limbs.fly : limbs.fry), position[2] + (index === 0 ? limbs.flz : limbs.frz))
        : hip.clone().applyQuaternion(turn).add(new Vector3(...position)).setY(position[1] - TITAN_RIDE_HEIGHT)
      // The foot that is stepping is lifted here: the server sends the ground it is over.
      const progress = step?.foot === index ? step.progress : null
      if (progress !== null) sole.y += TITAN_STEP_LIFT * Math.sin(Math.PI * progress)
      const ankle = sole.clone().setY(sole.y + PAD).sub(new Vector3(...position)).applyQuaternion(unturn)
      bend(hip, ankle, THIGH, SHIN, new Vector3(...TITAN_KNEE_POLE), joint, end)
      hang(this.thigh[index]!, hip, joint, back)
      hang(this.shin[index]!, joint, end, back)
      // The foot stays flat. On the ground it points the way it was put down, while the Titan turns over
      // it; in the air it comes round to the way the Titan faces.
      const nose = new Vector3(0, 0, -1).applyQuaternion(turn)
      const facing = Math.atan2(-nose.x, -nose.z)
      if (!this.seen || !limbs) this.pointing[index] = facing
      else if (progress !== null) this.pointing[index]! += wrapAngle(facing - this.pointing[index]!) * progress
      const level = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), this.pointing[index]!)
      this.foot[index]!.position.copy(end)
      this.foot[index]!.quaternion.copy(unturn).multiply(level)

      // A foot that was in the air and is not has come down.
      if (this.stepping[index] && progress === null) this.steps.push({ at: [sole.x, sole.y, sole.z], weight: 1 })
      this.stepping[index] = progress !== null
    }
    this.seen = true
  }

  dispose() {
    this.paint.dispose()
    this.trim.dispose()
    this.root.removeFromParent()
  }
}
