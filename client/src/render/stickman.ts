import * as THREE from 'three'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, MAX_PLAYERS } from '@/sim/consts'

/** Feet sit this far below the capsule centre. */
const FOOT_OFFSET = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS
const LIMB = 0.06
const ARM = 0.5
const LEG = 0.62

/** Bright, goofy colours, picked by entity id. */
const PALETTE = [0xff5a5f, 0x2ec4b6, 0xffbe0b, 0x8338ec, 0x3a86ff, 0xfb5607, 0x06d6a0, 0xff006e]

/** The colour of entity `id`'s head, also used for the heads of passengers peeking out of cars. */
export const colorFor = (id: number) => PALETTE[id % PALETTE.length]!

/** Stickmen that can be on screen at once: the room, and a few spare for ones about to be dropped. */
const CAPACITY = MAX_PLAYERS + 8

const HIDDEN = new THREE.Matrix4().makeScale(0, 0, 0)

/**
 * Every stickman in the scene, drawn as five instanced meshes (heads, eyes, torsos, arms, legs):
 * five draw calls however many players there are, where one mesh per body part per player was
 * ten each. A `Stickman` is a slot in these buffers plus the animation state that fills it.
 */
export class Crowd {
  readonly root = new THREE.Group()
  private readonly heads: THREE.InstancedMesh
  private readonly eyes: THREE.InstancedMesh
  private readonly torsos: THREE.InstancedMesh
  private readonly arms: THREE.InstancedMesh
  private readonly legs: THREE.InstancedMesh
  private readonly meshes: THREE.InstancedMesh[]
  private readonly free: number[] = []
  private used = 0

  constructor() {
    const dark = new THREE.MeshToonMaterial({ color: 0x222222 })
    const head = new THREE.MeshToonMaterial({ color: 0xffffff })
    const white = new THREE.MeshBasicMaterial({ color: 0xffffff })
    const limb = (length: number) => new THREE.CapsuleGeometry(LIMB, length, 4, 8)
    this.heads = new THREE.InstancedMesh(new THREE.SphereGeometry(0.2, 16, 12), head, CAPACITY)
    this.eyes = new THREE.InstancedMesh(new THREE.SphereGeometry(0.035, 8, 6), white, CAPACITY * 2)
    this.torsos = new THREE.InstancedMesh(new THREE.CapsuleGeometry(LIMB * 1.3, 0.5, 4, 8), dark, CAPACITY)
    this.arms = new THREE.InstancedMesh(limb(ARM), dark, CAPACITY * 2)
    this.legs = new THREE.InstancedMesh(limb(LEG), dark, CAPACITY * 2)
    this.meshes = [this.heads, this.eyes, this.torsos, this.arms, this.legs]
    for (const mesh of this.meshes) {
      mesh.count = 0
      // Instances move every frame, so the bounds computed once would be wrong at once.
      mesh.frustumCulled = false
      mesh.castShadow = mesh !== this.eyes
      this.root.add(mesh)
    }
  }

  /** How many stickmen are being drawn. */
  get size() {
    return this.used - this.free.length
  }

  /** Reserves a slot and colours its head; -1 when the room for them is used up. */
  allocate(id: number): number {
    const slot = this.free.pop() ?? (this.used < CAPACITY ? this.used++ : -1)
    if (slot < 0) return -1
    this.heads.count = this.used
    this.eyes.count = this.used * 2
    this.torsos.count = this.used
    this.arms.count = this.used * 2
    this.legs.count = this.used * 2
    this.heads.setColorAt(slot, new THREE.Color(colorFor(id)))
    if (this.heads.instanceColor) this.heads.instanceColor.needsUpdate = true
    return slot
  }

  release(slot: number) {
    if (slot < 0) return
    this.hide(slot)
    this.free.push(slot)
  }

  hide(slot: number) {
    if (slot < 0) return
    this.heads.setMatrixAt(slot, HIDDEN)
    this.torsos.setMatrixAt(slot, HIDDEN)
    for (const i of [slot * 2, slot * 2 + 1]) {
      this.eyes.setMatrixAt(i, HIDDEN)
      this.arms.setMatrixAt(i, HIDDEN)
      this.legs.setMatrixAt(i, HIDDEN)
    }
    this.touch()
  }

  /** Writes one slot's matrices. `root` is the capsule centre's transform, `body` the feet's, tipped by `fall`. */
  pose(slot: number, root: THREE.Matrix4, fall: number, arms: Limbs, legs: Limbs) {
    if (slot < 0) return
    const body = M.body.copy(root).multiply(M.part.makeTranslation(0, -FOOT_OFFSET, 0)).multiply(M.part.makeRotationX(fall * (Math.PI / 2)))

    // Head, with its two eyes on the -Z (forward) side.
    const head = M.head.copy(body).multiply(M.part.makeTranslation(0, 1.5, 0))
    this.heads.setMatrixAt(slot, head)
    for (const [i, side] of [[0, -1], [1, 1]] as const) {
      this.eyes.setMatrixAt(slot * 2 + i, M.out.copy(head).multiply(M.part.makeTranslation(side * 0.08, 0.04, -0.17)))
    }
    this.torsos.setMatrixAt(slot, M.out.copy(body).multiply(M.part.makeTranslation(0, 1.03, 0)))
    for (const [i, side] of [[0, -1], [1, 1]] as const) {
      const arm = M.out.copy(body).multiply(M.part.makeTranslation(side * 0.12, 1.25, 0))
      arm.multiply(M.part.makeRotationFromEuler(EULER.set(arms[i]!.x, 0, arms[i]!.z, 'XYZ'))).multiply(M.part.makeTranslation(0, -ARM / 2, 0))
      this.arms.setMatrixAt(slot * 2 + i, arm)
      const leg = M.out.copy(body).multiply(M.part.makeTranslation(side * 0.09, 0.72, 0))
      leg.multiply(M.part.makeRotationX(legs[i]!.x)).multiply(M.part.makeTranslation(0, -LEG / 2, 0))
      this.legs.setMatrixAt(slot * 2 + i, leg)
    }
    this.touch()
  }

  private touch() {
    for (const mesh of this.meshes) mesh.instanceMatrix.needsUpdate = true
  }

  dispose() {
    for (const mesh of this.meshes) {
      mesh.geometry.dispose()
      ;(mesh.material as THREE.Material).dispose()
      mesh.dispose()
    }
    this.root.removeFromParent()
  }
}

/** Rotation of a pair of limbs about X (swing) and Z (out to the side), radians, left then right. */
type Limbs = readonly [{ x: number; z: number }, { x: number; z: number }]

const M = {
  body: new THREE.Matrix4(),
  head: new THREE.Matrix4(),
  part: new THREE.Matrix4(),
  out: new THREE.Matrix4(),
}
const EULER = new THREE.Euler()

/**
 * One stickman: where it is and how its limbs swing, drawn into a slot of the shared `Crowd`.
 */
export class Stickman {
  /** The capsule centre, as last placed. */
  readonly position = new THREE.Vector3()
  private yaw = 0
  private readonly slot: number
  private readonly root = new THREE.Matrix4()
  private readonly arms: [{ x: number; z: number }, { x: number; z: number }] = [{ x: 0, z: -0.15 }, { x: 0, z: 0.15 }]
  private readonly legs: [{ x: number; z: number }, { x: number; z: number }] = [{ x: 0, z: 0 }, { x: 0, z: 0 }]
  private phase = 0
  /** 0 standing, 1 flat on the ground. */
  private fall = 0
  private down = false
  private shown = true

  constructor(private readonly crowd: Crowd, id: number) {
    this.slot = crowd.allocate(id)
    crowd.hide(this.slot)
  }

  /** Whether it is drawn: false while it sits in a car. */
  get visible() {
    return this.shown
  }

  /** Places the stickman at its capsule centre and animates it from ground speed. */
  update(position: readonly number[], yaw: number, speed: number, grounded: boolean, dt: number) {
    this.position.set(position[0]!, position[1]!, position[2]!)
    this.yaw = yaw
    this.fall = Math.max(0, Math.min(1, this.fall + (this.down ? 1 : -1) * dt * 5))
    const [armL, armR] = this.arms
    const [legL, legR] = this.legs
    if (this.down) {
      // Limp.
      armL.x = armR.x = 0
      armL.z = -0.5
      armR.z = 0.5
      legL.x = legR.x = 0
    } else if (!grounded) {
      // Airborne: arms up, legs tucked, for maximum flailing.
      armL.z = -2.4
      armR.z = 2.4
      legL.x = 0.6
      legR.x = -0.3
    } else {
      this.phase += dt * speed * 2.2
      const swing = Math.min(1, speed / 4) * 0.8 * Math.sin(this.phase)
      legL.x = swing
      legR.x = -swing
      armL.x = -swing
      armR.x = swing
      armL.z = -0.15
      armR.z = 0.15
    }
    this.draw()
  }

  private draw() {
    if (!this.shown) return
    // Tips over about the feet, onto its back.
    this.root.makeRotationY(this.yaw).setPosition(this.position)
    this.crowd.pose(this.slot, this.root, this.fall, this.arms, this.legs)
  }

  /** Lying where it fell. */
  set dead(dead: boolean) {
    this.down = dead
  }

  /** Hides the stickman while it sits in a car. */
  set hidden(hidden: boolean) {
    if (this.shown === !hidden) return
    this.shown = !hidden
    if (hidden) this.crowd.hide(this.slot)
    else this.draw()
  }

  dispose() {
    this.crowd.release(this.slot)
  }
}
