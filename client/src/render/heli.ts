import * as THREE from 'three'
import { HELI_HALF_EXTENTS, HELI_SEATS } from '@/sim/consts'
import type { Turret } from '@/net/codec'
import { colorFor } from './stickman'
import type { VehicleModel } from './car'

const [HALF_X, HALF_Y, HALF_Z] = HELI_HALF_EXTENTS
/** Main rotor speed with the pilot aboard, rad/s, and how fast it spins up or down, rad/s². */
const ROTOR_SPEED = 28
const ROTOR_ACCEL = 14
const TAIL_ROTOR_RATIO = 1.6
/** Each seat's occupant in the hull frame: pilot on the right, passenger on the left, as in a real cockpit. */
const SEATS: readonly [number, number, number][] = [
  [0.4, 0.2, -1.2],
  [-0.4, 0.2, -1.2],
]
const BODY_COLORS = [0xd62828, 0x2a9d8f, 0x3a86ff, 0xf4a261, 0x8338ec, 0x90be6d]

/**
 * A toy helicopter: a cabin under a glass bubble, a tail boom with a fin and a small rotor, skids,
 * and a main rotor that spins up while someone sits in the pilot's seat. The cabin box is the
 * physics hull; the rotors are bigger than it, as a cartoon's are.
 */
export class HeliModel implements VehicleModel {
  readonly root = new THREE.Group()
  private readonly rotor = new THREE.Group()
  private readonly tailRotor = new THREE.Group()
  private readonly heads: THREE.Mesh[] = []
  private rotorSpeed = 0
  private lastUpdate = performance.now()

  constructor(id: number) {
    const paint = new THREE.MeshToonMaterial({ color: BODY_COLORS[id % BODY_COLORS.length]! })
    const dark = new THREE.MeshToonMaterial({ color: 0x2b2b2b })
    const glass = new THREE.MeshToonMaterial({ color: 0xbfe9ff, transparent: true, opacity: 0.45 })
    const part = (parent: THREE.Object3D, geometry: THREE.BufferGeometry, material: THREE.Material, x: number, y: number, z: number) => {
      const mesh = new THREE.Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }

    // Cabin: a solid lower half and a glass bubble above it, over the front two thirds of the hull.
    const cabinLength = HALF_Z * 2 * 0.62
    const cabinZ = -HALF_Z + cabinLength / 2
    part(this.root, new THREE.BoxGeometry(HALF_X * 2, HALF_Y, cabinLength), paint, 0, -HALF_Y / 2, cabinZ)
    part(this.root, new THREE.BoxGeometry(HALF_X * 2 - 0.1, HALF_Y, cabinLength - 0.1), glass, 0, HALF_Y / 2, cabinZ)
    part(this.root, new THREE.BoxGeometry(HALF_X * 2 - 0.1, 0.12, cabinLength - 0.1), paint, 0, HALF_Y, cabinZ)

    // Tail boom, fin and tail rotor.
    const boomLength = HALF_Z * 2 - cabinLength + 0.2
    const boomZ = HALF_Z - boomLength / 2
    part(this.root, new THREE.BoxGeometry(0.4, 0.4, boomLength), paint, 0, 0.0, boomZ)
    part(this.root, new THREE.BoxGeometry(0.12, 0.9, 0.7), paint, 0, 0.45, HALF_Z - 0.2)
    this.tailRotor.position.set(0.28, 0.5, HALF_Z - 0.2)
    part(this.tailRotor, new THREE.BoxGeometry(0.06, 1.1, 0.14), dark, 0, 0, 0)
    this.root.add(this.tailRotor)

    // Skids on two struts each.
    for (const side of [-1, 1]) {
      part(this.root, new THREE.BoxGeometry(0.12, 0.1, HALF_Z * 1.7), dark, side * (HALF_X - 0.1), -HALF_Y - 0.05, -0.2)
      for (const z of [-1.1, 0.5]) {
        part(this.root, new THREE.BoxGeometry(0.1, 0.35, 0.1), dark, side * (HALF_X - 0.1), -HALF_Y + 0.12, z)
      }
    }

    // Main rotor: a mast and two long blades that turn about it.
    part(this.root, new THREE.CylinderGeometry(0.1, 0.12, 0.35, 8), dark, 0, HALF_Y + 0.2, cabinZ + 0.2)
    this.rotor.position.set(0, HALF_Y + 0.4, cabinZ + 0.2)
    part(this.rotor, new THREE.BoxGeometry(6.4, 0.06, 0.35), dark, 0, 0, 0)
    this.root.add(this.rotor)

    const headGeometry = new THREE.SphereGeometry(0.2, 12, 10)
    for (let seat = 0; seat < HELI_SEATS; seat++) {
      const head = new THREE.Mesh(headGeometry, new THREE.MeshToonMaterial({ color: 0xffffff }))
      head.position.set(...SEATS[seat]!)
      head.castShadow = true
      head.visible = false
      this.root.add(head)
      this.heads.push(head)
    }
  }

  /**
   * Places the helicopter and animates it. The rotor spins up while seat 0 is taken. `steer` and
   * `turret` are unused: its lean and turn are in the orientation itself.
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
    _turret?: Turret | null,
  ) {
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.lastUpdate) / 1000)
    this.lastUpdate = now
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const target = (seats[0] ?? 0) !== 0 ? ROTOR_SPEED : 0
    const step = ROTOR_ACCEL * dt
    this.rotorSpeed += Math.max(-step, Math.min(step, target - this.rotorSpeed))
    this.rotor.rotation.y += this.rotorSpeed * dt
    this.tailRotor.rotation.x += this.rotorSpeed * TAIL_ROTOR_RATIO * dt
    this.heads.forEach((head, seat) => {
      const occupant = seats[seat] ?? 0
      head.visible = occupant !== 0
      if (occupant !== 0) (head.material as THREE.MeshToonMaterial).color.setHex(colorFor(occupant))
    })
  }

  dispose() {
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) object.geometry.dispose()
    })
    this.root.removeFromParent()
  }
}
