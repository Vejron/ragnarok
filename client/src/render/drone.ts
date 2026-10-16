import { Group, Mesh, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { colorFor } from '@/profile'
import type { VehicleModel } from './car'
import { DRONE_HALF_EXTENTS } from '@/sim/consts'

const [HX, HY, HZ] = DRONE_HALF_EXTENTS
/** Props spin this fast with someone at the controls, rad/s, and wind down over about a second without. */
const PROP_SPIN = 90
const PROP_SPIN_DOWN = 1.2
const FRAME = 0x2b2d30
const CHARGE = 0x6b6f3a

// Shared by every drone: parts only ever move, so none of these is disposed.
const BODY = boxGeometry(HX * 0.9, HY * 1.6, HZ * 1.2)
const CHARGE_PACK = cylinderGeometry(0.045, 0.045, HZ * 1.6, 10)
// Lies along the drone's nose, under the body.
CHARGE_PACK.rotateX(Math.PI / 2)
const ARM = boxGeometry(0.03, 0.02, Math.hypot(HX, HZ) * 2)
const MOTOR = cylinderGeometry(0.025, 0.025, 0.04, 8)
const PROP = boxGeometry(0.2, 0.004, 0.02)
const CAMERA = sphereGeometry(0.025, 8, 6)

/** Motor spots in the hull frame: the corners of the airframe. */
const MOTORS: [number, number][] = [
  [-HX, -HZ],
  [HX, -HZ],
  [-HX, HZ],
  [HX, HZ],
]

/**
 * An FPV drone: a quad frame with a charge slung under it and a camera at the nose, lit in its pilot's
 * colour. Its props spin while it is flown and wind down once the link is lost.
 */
export class DroneModel implements VehicleModel {
  readonly root = new Group()
  private readonly props: Mesh[] = []
  private readonly frame = toonMaterial({ color: FRAME })
  private readonly charge = toonMaterial({ color: CHARGE })
  private readonly light = toonMaterial({ color: 0xffffff })
  private spin = 0
  private speed = 0
  private last = performance.now()

  constructor(_id: number) {
    const body = new Mesh(BODY, this.frame)
    body.castShadow = true
    this.root.add(body)
    const pack = new Mesh(CHARGE_PACK, this.charge)
    pack.position.set(0, -HY * 1.2, 0)
    this.root.add(pack)
    for (const angle of [Math.PI / 4, -Math.PI / 4]) {
      const arm = new Mesh(ARM, this.frame)
      arm.rotation.y = angle
      this.root.add(arm)
    }
    for (const [x, z] of MOTORS) {
      const motor = new Mesh(MOTOR, this.frame)
      motor.position.set(x, 0.02, z)
      this.root.add(motor)
      const prop = new Mesh(PROP, this.frame)
      prop.position.set(x, 0.045, z)
      this.props.push(prop)
      this.root.add(prop)
    }
    const camera = new Mesh(CAMERA, this.light)
    camera.position.set(0, 0.01, -HZ * 0.65)
    this.root.add(camera)
  }

  /** Places the drone. `seats[0]` is its pilot, or 0 once the link is lost. */
  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number], _steer: number, seats: readonly number[]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.last) / 1000)
    this.last = now
    const pilot = seats[0] ?? 0
    this.speed = pilot !== 0 ? PROP_SPIN : Math.max(0, this.speed - (PROP_SPIN / PROP_SPIN_DOWN) * dt)
    this.spin += this.speed * dt
    this.props.forEach((prop, index) => {
      // Diagonal pairs turn opposite ways, as on a real quad.
      prop.rotation.y = index === 0 || index === 3 ? this.spin : -this.spin
    })
    if (pilot !== 0) this.light.color.setHex(colorFor(pilot))
  }

  /** Frees what this drone owns; the shared geometry stays. */
  dispose() {
    this.frame.dispose()
    this.charge.dispose()
    this.light.dispose()
    this.root.removeFromParent()
  }
}
