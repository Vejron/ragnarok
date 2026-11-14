import { Group, Mesh, type Material, type Node, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT } from '@/net/codec'
import { TITAN_FIST_REST, TITAN_FOOT_HALF, TITAN_HALF_EXTENTS, TITAN_RIDE_HEIGHT, TITAN_SHOULDER } from '@/sim/consts'
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
/** A foot moving faster than this, m/s, is in a step; one that was and has slowed below `LANDED` has come down. */
const STEPPING = 3
const LANDED = 0.8

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
  /** Each foot's sole in the world, as drawn, where it was a frame ago, and whether it is in a step. */
  private readonly soles = [new Vector3(), new Vector3()]
  private readonly stepping = [false, false]
  private seen = false
  private stamp = 0
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

  /** Places the Titan and bends its limbs to where they are. `steer`, `seats` and the rest are unused. */
  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const now = performance.now()
    const dt = this.seen ? Math.min(0.1, Math.max(0.001, (now - this.stamp) / 1000)) : 0.016
    this.stamp = now
    const limbs = this.limbs
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
      const ankle = sole.clone().setY(sole.y + PAD).sub(new Vector3(...position)).applyQuaternion(unturn)
      bend(hip, ankle, THIGH, SHIN, new Vector3(0, -0.15, -1), joint, end)
      hang(this.thigh[index]!, hip, joint, back)
      hang(this.shin[index]!, joint, end, back)
      // The foot stays flat on the ground, pointing the way the Titan faces.
      const nose = new Vector3(0, 0, -1).applyQuaternion(turn)
      const level = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.atan2(-nose.x, -nose.z))
      this.foot[index]!.position.copy(end)
      this.foot[index]!.quaternion.copy(unturn).multiply(level)

      // A foot that was swinging and has stopped has come down.
      const speed = this.seen ? sole.distanceTo(this.soles[index]!) / dt : 0
      if (speed > STEPPING) this.stepping[index] = true
      else if (this.stepping[index] && speed < LANDED) {
        this.stepping[index] = false
        this.steps.push({ at: [sole.x, sole.y, sole.z], weight: 1 })
      }
      this.soles[index]!.copy(sole)
    }
    this.seen = true
  }

  dispose() {
    this.paint.dispose()
    this.root.removeFromParent()
  }
}
