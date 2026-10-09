import * as THREE from 'three'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS } from '@/sim/consts'

/** Feet sit this far below the capsule centre. */
const FOOT_OFFSET = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS
const LIMB = 0.06

/** Bright, goofy colours, picked by entity id. */
const PALETTE = [0xff5a5f, 0x2ec4b6, 0xffbe0b, 0x8338ec, 0x3a86ff, 0xfb5607, 0x06d6a0, 0xff006e]

function limb(length: number, material: THREE.Material) {
  // Hangs down from its pivot so rotating the pivot swings it like a joint.
  const mesh = new THREE.Mesh(new THREE.CapsuleGeometry(LIMB, length, 4, 8), material)
  mesh.position.y = -length / 2
  mesh.castShadow = true
  const pivot = new THREE.Group()
  pivot.add(mesh)
  return pivot
}

/** A procedural stickman: round head, stick body, swinging arms and legs. */
export class Stickman {
  readonly root = new THREE.Group()
  private readonly armL: THREE.Group
  private readonly armR: THREE.Group
  private readonly legL: THREE.Group
  private readonly legR: THREE.Group
  private phase = 0

  constructor(id: number) {
    const color = PALETTE[id % PALETTE.length]!
    const material = new THREE.MeshToonMaterial({ color: 0x222222 })
    const accent = new THREE.MeshToonMaterial({ color })
    const body = new THREE.Group()
    body.position.y = -FOOT_OFFSET
    this.root.add(body)

    const head = new THREE.Mesh(new THREE.SphereGeometry(0.2, 16, 12), accent)
    head.position.y = 1.5
    head.castShadow = true
    // Two dot eyes on the -Z (forward) side, for goofiness and so facing is readable.
    for (const side of [-1, 1]) {
      const eye = new THREE.Mesh(new THREE.SphereGeometry(0.035, 8, 6), new THREE.MeshBasicMaterial({ color: 0xffffff }))
      eye.position.set(side * 0.08, 0.04, -0.17)
      head.add(eye)
    }
    body.add(head)

    const torso = new THREE.Mesh(new THREE.CapsuleGeometry(LIMB * 1.3, 0.5, 4, 8), material)
    torso.position.y = 1.03
    torso.castShadow = true
    body.add(torso)

    this.armL = limb(0.5, material)
    this.armR = limb(0.5, material)
    this.armL.position.set(-0.12, 1.25, 0)
    this.armR.position.set(0.12, 1.25, 0)
    this.legL = limb(0.62, material)
    this.legR = limb(0.62, material)
    this.legL.position.set(-0.09, 0.72, 0)
    this.legR.position.set(0.09, 0.72, 0)
    body.add(this.armL, this.armR, this.legL, this.legR)
  }

  /** Places the stickman at its capsule centre and animates it from ground speed. */
  update(position: readonly number[], yaw: number, speed: number, grounded: boolean, dt: number) {
    this.root.position.set(position[0]!, position[1]!, position[2]!)
    this.root.rotation.y = yaw
    if (!grounded) {
      // Airborne: arms up, legs tucked, for maximum flailing.
      this.armL.rotation.z = -2.4
      this.armR.rotation.z = 2.4
      this.legL.rotation.x = 0.6
      this.legR.rotation.x = -0.3
      return
    }
    this.phase += dt * speed * 2.2
    const swing = Math.min(1, speed / 4) * 0.8 * Math.sin(this.phase)
    this.legL.rotation.x = swing
    this.legR.rotation.x = -swing
    this.armL.rotation.x = -swing
    this.armR.rotation.x = swing
    this.armL.rotation.z = -0.15
    this.armR.rotation.z = 0.15
  }

  dispose() {
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) object.geometry.dispose()
    })
    this.root.removeFromParent()
  }
}
