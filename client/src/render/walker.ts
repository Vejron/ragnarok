import { Group, Mesh, sphereGeometry, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { WALKER_FOOT_HALF, WALKER_GUN_BARREL_DROP, WALKER_GUN_BARREL_X, WALKER_HALF_EXTENTS, WALKER_LEG_LENGTH, WALKER_LEG_SPRING } from '@/sim/consts'
import { CAR_GRAVITY } from '@/sim/vehicle'
import type { Turret } from '@/net/codec'
import { colorFor } from './character'
import type { VehicleModel } from './car'

/** Paint colours handed out by vehicle id, so neighbouring walkers are told apart. */
const PAINTS = [0x7d8a8f, 0x8f8a7d, 0x6e7a73, 0x857c72] as const
export const WALKER_MODEL_URL = `${import.meta.env.BASE_URL}models/walker.glb`
/** Where the driver's head shows, in the upper body's frame: out of the roof hatch. */
const HEAD = [0.25, 1.4, 0.3] as const
/** The furthest a foot lands ahead of its hip, metres: at a run it plants this far ahead and slides this far back. */
const STEP = 1.7
/** A foot lifts for a new step when it has slid this far from where a fresh step would land, at the least. */
const SETTLE = 0.3
/** How far the guns kick back when they fire, metres, and how fast they return, per second. */
const RECOIL = 0.35
const RECOIL_RETURN = 6
/** Hull sway to the planted side while it walks, radians, at the most. */
const SWAY = 0.06
/** How high a foot is lifted at the middle of its swing, metres. */
const LIFT = 0.6
/** Thigh and shin, metres, and the foot pad's thickness. The ankle hangs this far above the sole. */
const THIGH = 2.1
const SHIN = 2.2
/** The foot's thickness in the model (`PAD` in build_walker.py): the ankle hangs this far above the sole. */
const PAD = 0.3
/** How far the hips sit above the ground when it stands: the leg's reach less its spring's sag. */
const STAND = WALKER_LEG_LENGTH - CAR_GRAVITY / WALKER_LEG_SPRING

let asset: Node | null = null

/**
 * Parses `walker.glb` (built by `tools/vehicles/build_walker.py`) for every `WalkerModel` to share. Its
 * materials become flat toon ones to match the rest of the scene (`lamp` ones are unlit); `paint` is
 * replaced per walker.
 */
export async function parseWalkerModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null))
}

export async function loadWalkerModel(url = WALKER_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseWalkerModel(await response.arrayBuffer())
}

/**
 * An AT-ST in silhouette: an armoured cab with chin guns, on two long bird-like legs that step with the
 * ground they cover. The cab (the upper body) turns on the hips with the driver's aim, within a
 * turret's travel, and the guns lift in it. The legs are drawn only: the physics is one box
 * (the cab) held up by two spring rays, with a solid box on each leg, so they animate from the ground
 * speed. The mesh is `walker.glb`; its nodes are named in `build_walker.py`.
 */
export class WalkerModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint: Material
  private readonly head = new Mesh(sphereGeometry(0.22, 12, 10), toonMaterial({ color: 0xffffff }))
  /** The upper body, which turns with the aim, and the guns in it, which lift. */
  private readonly torso: Node
  private readonly gun: Node
  private readonly gunRest: number
  private kick = 0
  /** Hip pivots, left then right, each holding the thigh, then a knee pivot holding the shin, then the ankle. */
  private readonly hips: Node[] = []
  private readonly knees: Node[] = []
  private readonly ankles: Node[] = []
  private stamp = 0
  private readonly last = new THREE.Vector3()
  private yaw = 0
  private seen = false
  /** Each foot: planted at a fixed world spot, or swinging to a new one. */
  private readonly feet = [0, 1].map(() => ({ planted: true, x: 0, z: 0, from: 0, to: 0, time: 0, duration: 0.3 }))
  /** Smoothed sideways sway of the hull, + is toward the right foot. */
  private sway = 0
  /** Feet that came down since the last `takeSteps`: where, and how hard (0..1, by speed). */
  private steps: { at: [number, number, number]; weight: number }[] = []

  constructor(id: number) {
    if (!asset) throw new Error('loadWalkerModel() must finish before walkers are created')
    const model = asset.clone()
    this.root.add(model)
    this.paint = toonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = this.paint
    })
    this.torso = model.getObjectByName('torso')!
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z
    for (const label of ['L', 'R']) {
      this.hips.push(model.getObjectByName(`hip_${label}`)!)
      this.knees.push(model.getObjectByName(`knee_${label}`)!)
      this.ankles.push(model.getObjectByName(`ankle_${label}`)!)
    }
    this.head.position.set(...HEAD)
    this.head.castShadow = true
    this.head.visible = false
    this.torso.add(this.head)
  }

  /** Where the guns are and which way they point, in the world, for the muzzle flash and the tracer. */
  barrel(side = 0) {
    this.gun.updateWorldMatrix(true, false)
    // `side` -1 or 1 is one of the two barrels, which hang under the chin; 0 is the pivot between them.
    const matrix = new THREE.Matrix4().fromArray(this.gun.matrixWorld)
    const origin = new THREE.Vector3(side * WALKER_GUN_BARREL_X, side === 0 ? 0 : -WALKER_GUN_BARREL_DROP, 0).applyMatrix4(matrix)
    const direction = new THREE.Vector3(0, 0, -1).transformDirection(matrix)
    return { origin, direction }
  }

  /** The footfalls since the last call, for the thump that goes with each. */
  takeSteps() {
    const steps = this.steps
    this.steps = []
    return steps
  }

  /** The guns kick back as they fire. */
  recoil() {
    this.kick = RECOIL
  }

  /** Places the walker and swings its legs by how far it has moved since the last call. `steer` is unused. */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)

    // The upper body and guns follow the aim the server reports; the legs keep their own heading.
    this.torso.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0

    const now = performance.now()
    const dt = this.seen ? Math.min(0.1, Math.max(0.001, (now - this.stamp) / 1000)) : 0.016
    this.stamp = now
    const here = new THREE.Vector3(position[0], 0, position[2])
    // Nose pitch: leaning forward dips it. The legs undo it, so the feet stay flat on the ground.
    const nose = new THREE.Vector3(0, 0, -1).applyQuaternion(this.root.quaternion)
    const pitch = Math.asin(Math.max(-1, Math.min(1, nose.y)))
    const heading = new THREE.Vector3(nose.x, 0, nose.z)
    if (heading.lengthSq() < 1e-6) heading.set(0, 0, -1)
    heading.normalize()
    // Right of the heading, in the ground plane.
    const right = new THREE.Vector3(-heading.z, 0, heading.x)
    const yaw = Math.atan2(-heading.x, -heading.z)
    const velocity = here.clone().sub(this.last).divideScalar(dt)
    let spin = 0
    if (this.seen) {
      spin = yaw - this.yaw
      spin = Math.atan2(Math.sin(spin), Math.cos(spin)) / dt
    }
    const speed = velocity.length()
    // A jump in position (a respawn) puts the feet back under the hull.
    const restart = !this.seen || speed > 40
    this.last.copy(here)
    this.yaw = yaw
    this.seen = true
    const along = velocity.dot(heading)

    let lean = 0
    this.hips.forEach((hip, index) => {
      const foot = this.feet[index]!
      const side = index === 0 ? -1 : 1
      const hipX = position[0] + right.x * side * WALKER_FOOT_HALF
      const hipZ = position[2] + right.z * side * WALKER_FOOT_HALF
      if (restart) {
        foot.planted = true
        foot.x = hipX
        foot.z = hipZ
      }
      // How fast this hip moves along the heading: the hull's own speed, and the turn swinging it round.
      const hipRate = along - side * WALKER_FOOT_HALF * spin
      const direction = Math.sign(hipRate)
      const wanted = direction * STEP * Math.min(1, Math.abs(hipRate) / 6)
      const moving = Math.abs(hipRate) > 0.15
      // Where the foot is, ahead of the hip along the heading.
      let reach = (foot.x - hipX) * heading.x + (foot.z - hipZ) * heading.z
      let lift = 0

      if (foot.planted) {
        const other = this.feet[1 - index]!
        const behind = moving ? reach * direction < -Math.max(SETTLE, Math.abs(wanted)) : Math.abs(reach) > SETTLE
        const stretched = Math.abs(reach) > STEP * 1.4
        if (behind && (other.planted || stretched)) {
          foot.planted = false
          foot.from = reach
          foot.to = moving ? wanted : 0
          foot.time = 0
          // Quick enough to be down before the other foot has run out of room.
          foot.duration = Math.min(0.4, Math.max(0.15, (1.6 * STEP) / Math.max(1, Math.abs(hipRate))))
        }
      }
      if (!foot.planted) {
        foot.time += dt
        const s = Math.min(1, foot.time / foot.duration)
        reach = foot.from + (foot.to - foot.from) * (s * s * (3 - 2 * s))
        lift = Math.sin(Math.PI * s) * LIFT
        lean -= side * Math.sin(Math.PI * s)
        if (s >= 1) {
          foot.planted = true
          foot.x = hipX + heading.x * reach
          foot.z = hipZ + heading.z * reach
          lift = 0
          if (!restart) this.steps.push({ at: [foot.x, position[1] - WALKER_HALF_EXTENTS[1] - STAND, foot.z], weight: Math.min(1, 0.45 + speed / 8) })
        }
      }

      // Two-bone IK in the leg's plane: the ankle goes `reach` ahead of the hip and `STAND - PAD - lift`
      // below it. The hull's tilt moves the hip, so the target is turned back by the pitch.
      const drop = STAND - PAD - lift
      const distance = Math.min(Math.hypot(reach, drop), THIGH + SHIN - 0.01)
      const toTarget = Math.atan2(reach, drop)
      const kneeInside = Math.acos(Math.max(-1, Math.min(1, (THIGH * THIGH + SHIN * SHIN - distance * distance) / (2 * THIGH * SHIN))))
      const thighOffset = Math.acos(Math.max(-1, Math.min(1, (THIGH * THIGH + distance * distance - SHIN * SHIN) / (2 * THIGH * distance))))
      // The knee points back, like a bird's: the thigh goes behind the line to the foot and the shin forward of it.
      const thigh = toTarget - thighOffset
      const bend = Math.PI - kneeInside
      // A positive rotation about x swings a hanging leg towards -z, the way the nose points.
      hip.rotation.x = thigh - pitch
      this.knees[index]!.rotation.x = bend
      this.ankles[index]!.rotation.x = -(hip.rotation.x + this.knees[index]!.rotation.x) - pitch
    })

    // The hull sways over whichever foot is down, more the faster it goes.
    const target = Math.max(-1, Math.min(1, lean)) * SWAY * Math.min(1, Math.hypot(speed, Math.abs(spin) * WALKER_FOOT_HALF) / 3)
    this.sway += (target - this.sway) * Math.min(1, dt * 10)
    // Leaning to the right moves the top of the hull to +x, which is a negative turn about the nose axis.
    this.root.quaternion.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), -this.sway))

    this.kick = Math.max(0, this.kick - this.kick * Math.min(1, dt * RECOIL_RETURN) - dt * 0.2)
    this.gun.position.z = this.gunRest + this.kick

    const driver = seats[0] ?? 0
    this.head.visible = driver !== 0
    if (driver !== 0) {
      this.head.material.color.setHex(colorFor(driver))
      this.head.material.needsUpdate()
    }
  }

  dispose() {
    this.paint.dispose()
    this.head.material.dispose()
    this.head.geometry.dispose()
    this.root.removeFromParent()
  }
}
