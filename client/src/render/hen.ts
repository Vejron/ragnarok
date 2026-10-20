import { Group, Mesh, sphereGeometry, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { Legs } from './legs'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { WALKER_HALF_EXTENTS, WALKER_LEG_LENGTH, WALKER_LEG_SPRING } from '@/sim/consts'
import { CAR_GRAVITY } from '@/sim/vehicle'
import { THRUST_AIR, THRUST_GLIDE, THRUST_LIFT, type Arms } from '@/net/codec'
import { colorFor } from '@/profile'
import type { VehicleModel } from './car'

/** The breeds handed out by vehicle id: a white hen with a red comb, and a silver one with a dark grey comb and wattles. */
const BREEDS = [
  { paint: 0xf2efe6, comb: null },
  { paint: 0xa9adb3, comb: 0x3b3d40 },
] as const
export const HEN_MODEL_URL = `${import.meta.env.BASE_URL}models/hen.glb`
/** Where the driver's head shows, in the hull's frame: in the glass cockpit on the back. */
const HEAD = [0, 1.3, 0.35] as const
/** The walker's legs: thigh and shin, the foot's thickness, and how far the hips sit above the ground. */
const THIGH = 2.1
const SHIN = 2.2
const PAD = 0.3
const STAND = WALKER_LEG_LENGTH - CAR_GRAVITY / WALKER_LEG_SPRING
/** How far the wings swing out from folded, radians: spread to glide, and the two ends of a beat. */
const SPREAD = 1.5
const BEAT_LOW = 0.5
const BEAT_HIGH = 2.3
/** Wing beats a second while flapping. */
const BEAT_RATE = 3.5
/** The head bobs this far forwards and back as it walks, metres, once a stride. */
const BOB = 0.3
/** Hull sway to the planted side while it walks, radians, at the most. */
const SWAY = 0.07
/** How far the tail flicks up as an egg is laid, radians, and how fast it settles, per second. */
const FLICK = 0.7
const FLICK_RETURN = 3

let asset: Node | null = null

/**
 * Parses `hen.glb` (built by `tools/vehicles/build_hen.py`) for every `HenModel` to share. Its materials
 * become flat toon ones to match the rest of the scene (`lamp`, the eyes, is unlit); `paint`, and on a
 * silver hen `comb`, are replaced per hen.
 */
export async function parseHenModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null), { merge: true, keep: ['comb'] })
}

export async function loadHenModel(url = HEN_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseHenModel(await response.arrayBuffer())
}

/**
 * A giant robot hen: a plump armoured body on the walker's two long legs, with wings, a tail and a head
 * that bobs as it walks. The physics is the walker's box on two spring rays; the legs step with the
 * ground they cover and tuck up while it is in the air (`THRUST_AIR`). The wings beat while it leaps
 * or flaps (`THRUST_LIFT`) and spread still while it glides (`THRUST_GLIDE`), and the tail flicks up
 * as each egg is laid. The mesh is `hen.glb`; its nodes are named in `build_hen.py`.
 */
export class HenModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint: Material
  /** The comb and wattles' own material, for a breed that is not red there. */
  private readonly comb: Material | null
  private readonly driverHead = new Mesh(sphereGeometry(0.22, 12, 10), toonMaterial({ color: 0xffffff }))
  private readonly head: Node
  private readonly headRest: number
  private readonly wings: Node[]
  private readonly tail: Node
  private readonly legs: Legs
  private flags = 0
  /** How far the wings are swung out, radians, and where they are in their beat (0..1). */
  private wing = 0
  private beat = 0
  private stride = 0
  private sway = 0
  private flick = 0
  private laid: number | null = null
  /** Wing beats and leaps since the last `takeEvents`, for their sounds. */
  private beats = 0
  private leapt = false
  private eggs = 0

  constructor(id: number) {
    if (!asset) throw new Error('loadHenModel() must finish before hens are created')
    const model = asset.clone()
    this.root.add(model)
    const breed = BREEDS[id % BREEDS.length]!
    this.paint = toonMaterial({ color: breed.paint })
    this.comb = breed.comb === null ? null : toonMaterial({ color: breed.comb })
    model.traverse((node) => {
      if (!(node instanceof Mesh)) return
      if (node.material.name === 'paint') node.material = this.paint
      else if (this.comb && node.material.name === 'comb') node.material = this.comb
    })
    this.head = model.getObjectByName('head')!
    this.headRest = this.head.position.z
    this.wings = ['L', 'R'].map((label) => model.getObjectByName(`wing_${label}`)!)
    this.tail = model.getObjectByName('tail')!
    const joint = (name: string) => ['L', 'R'].map((label) => model.getObjectByName(`${name}_${label}`)!)
    this.legs = new Legs(joint('hip'), joint('knee'), joint('ankle'), {
      thigh: THIGH,
      shin: SHIN,
      pad: PAD,
      stand: STAND,
      hipDrop: WALKER_HALF_EXTENTS[1],
    })
    this.driverHead.position.set(...HEAD)
    this.driverHead.castShadow = true
    this.driverHead.visible = false
    model.getObjectByName('body')!.add(this.driverHead)
  }

  /** The `THRUST_*` flags to draw from: the snapshot's, or the driver's own prediction. */
  setFlags(flags: number) {
    if (flags & THRUST_AIR && !(this.flags & THRUST_AIR) && flags & THRUST_LIFT) this.leapt = true
    this.flags = flags
  }

  /** The footfalls since the last call (landings weigh 1), for the thump that goes with each. */
  takeSteps() {
    return this.legs.takeSteps()
  }

  /** What it did since the last call that makes a sound: wing beats, whether it leapt, and eggs laid. */
  takeEvents() {
    const events = { beats: this.beats, leapt: this.leapt, eggs: this.eggs }
    this.beats = 0
    this.leapt = false
    this.eggs = 0
    return events
  }

  /** Places the hen, steps its legs and works its wings, head and tail. `steer` and `turret` are unused. */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    _turret?: unknown,
    arms?: Arms | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const airborne = (this.flags & THRUST_AIR) !== 0
    const { dt, speed, spin, lean } = this.legs.update(position, rotation, airborne)

    // The wings: beating while it leaps or flaps, spread to glide, folded otherwise.
    let wanted = 0
    if (this.flags & THRUST_LIFT) {
      const before = this.beat
      this.beat = (this.beat + dt * BEAT_RATE) % 1
      if (this.beat < before) this.beats++
      wanted = BEAT_LOW + (BEAT_HIGH - BEAT_LOW) * (0.5 + 0.5 * Math.cos(this.beat * Math.PI * 2))
    } else if (this.flags & THRUST_GLIDE) wanted = SPREAD
    else if (airborne) wanted = 0.4
    this.wing += (wanted - this.wing) * Math.min(1, dt * (this.flags & THRUST_LIFT ? 30 : 6))
    // The left wing swings out to -x, which is a negative turn about the fore-and-aft axis.
    this.wings[0]!.rotation.z = -this.wing
    this.wings[1]!.rotation.z = this.wing

    // The head bobs once a stride on the ground, and stretches out ahead in the air.
    this.stride = (this.stride + speed * dt * 0.6) % 1
    const bob = airborne ? -BOB : Math.sin(this.stride * Math.PI * 2) * BOB * Math.min(1, speed / 3)
    this.head.position.z = this.headRest + bob

    // The hull sways over whichever foot is down, more the faster it goes.
    const target = airborne ? 0 : lean * SWAY * Math.min(1, Math.hypot(speed, Math.abs(spin) * 1.7) / 3)
    this.sway += (target - this.sway) * Math.min(1, dt * 10)
    this.root.quaternion.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), -this.sway))

    // The tail flicks up as an egg is laid.
    const shots = arms?.shots ?? null
    if (shots !== null && this.laid !== null && shots !== this.laid) {
      this.flick = FLICK
      this.eggs++
    }
    this.laid = shots
    this.flick = Math.max(0, this.flick - this.flick * Math.min(1, dt * FLICK_RETURN) - dt * 0.05)
    // Lifting the tail, behind the hull (+z), is a negative turn about x.
    this.tail.rotation.x = -this.flick

    const driver = seats[0] ?? 0
    this.driverHead.visible = driver !== 0
    if (driver !== 0) {
      this.driverHead.material.color.setHex(colorFor(driver))
      this.driverHead.material.needsUpdate()
    }
  }

  dispose() {
    this.paint.dispose()
    this.comb?.dispose()
    this.driverHead.material.dispose()
    this.driverHead.geometry.dispose()
    this.root.removeFromParent()
  }
}
