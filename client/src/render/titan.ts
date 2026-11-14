import { Group, Mesh, type Material, type Node, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, THRUST_LIFT, wrapAngle } from '@/net/codec'
import { TITAN_FIST_REST, TITAN_FOOT_HALF, TITAN_HALF_EXTENTS, TITAN_RIDE_HEIGHT, TITAN_SHOULDER, TITAN_STEP_LIFT } from '@/sim/consts'
import type { Titan } from '@/entities/units'
import type { VehicleModel } from './car'

/** Paint colours handed out by vehicle id, so two Titans are told apart. */
const PAINTS = [0x7f8890, 0x8a8478, 0x6f7c80, 0x86807c] as const
export const TITAN_MODEL_URL = `${import.meta.env.BASE_URL}models/titan.glb`
/** Upper arm and forearm, shoulder to elbow and elbow to the fist's middle, metres (`UPPER`, `FORE` in build_titan.py). */
const UPPER = 12
const FORE = 12
/** Thigh and shin, hip to knee and knee to ankle, metres, and how far the ankle is above the sole (`THIGH`, `SHIN`, `PAD`). */
const THIGH = 13.5
const SHIN = 13.5
const PAD = 1.5
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
 * materials become flat toon ones to match the rest of the scene (`lamp` ones are unlit); `paint` is
 * replaced per Titan. Its parts are not merged: each limb is placed by itself.
 */
export async function parseTitanModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null))
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

/**
 * A Titan: its torso on the hull, and its arms and legs bent by IK to where the server has its fists
 * and feet (`Titan`). Nothing here decides where a limb is: with no word from the server yet the arms
 * hang and the legs stand straight under the hips. The mesh is `titan.glb`; its nodes are named in
 * `build_titan.py`.
 */
export class TitanModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint: Material
  private readonly upper: Node[]
  private readonly fore: Node[]
  private readonly fist: Node[]
  private readonly open: Node[]
  private readonly closed: Node[]
  private readonly thigh: Node[]
  private readonly shin: Node[]
  private readonly foot: Node[]
  private limbs: Titan | null = null
  /** Each fist's middle in the hull frame, as drawn. */
  private readonly fists = [new Vector3(), new Vector3()]
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

  constructor(id: number) {
    if (!asset) throw new Error('loadTitanModel() must finish before Titans are created')
    const model = asset.clone()
    this.root.add(model)
    this.paint = toonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = this.paint
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
  }

  /** Where the server has its fists and feet, or null before it has said. */
  setLimbs(limbs: Titan | null) {
    this.limbs = limbs
  }

  /** The middle of fist `hand` (0 left) in the world, as drawn. */
  fistWorld(hand: number): Vector3 {
    this.root.updateWorldMatrix(true, false)
    return this.fists[hand]!.clone().applyMatrix4(new Matrix4().fromArray(this.root.matrixWorld))
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
    this.height = position[1]
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
      bend(shoulder, wanted, UPPER, FORE, new Vector3(side * 0.7, -0.5, 0.6), joint, end)
      hang(this.upper[index]!, shoulder, joint, back)
      hang(this.fore[index]!, joint, end, back)
      hang(this.fist[index]!, end, end.clone().addScaledVector(end.clone().sub(joint).normalize(), 1), back)
      this.fists[index]!.copy(end)
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
      bend(hip, ankle, THIGH, SHIN, new Vector3(0, -0.15, -1), joint, end)
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
    this.root.removeFromParent()
  }
}
