import { Group, Mesh, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { colorFor } from '@/profile'
import type { VehicleModel } from './car'
import { LOITER_HALF_EXTENTS } from '@/sim/consts'

const [HX, , HZ] = LOITER_HALF_EXTENTS
/** The pusher prop spins this fast with someone at the controls, rad/s, and winds down over about two seconds without. */
const PROP_SPIN = 60
const PROP_SPIN_DOWN = 2
const SKIN = 0x8a8d84
const TRIM = 0x55584f
/** The fuselage's radius, metres. */
const BODY_R = 0.2

// Shared by every loitering drone: parts only ever move, so none of these is disposed.
const FUSELAGE = cylinderGeometry(BODY_R, BODY_R * 0.8, HZ * 1.8, 12)
// Lies along the nose; the wider end forward.
FUSELAGE.rotateX(-Math.PI / 2)
const NOSE = cylinderGeometry(0.02, BODY_R, 0.45, 12)
NOSE.rotateX(-Math.PI / 2)
/** The delta wing as three slabs, narrowing towards the nose: [span, depth, z of its centre]. */
const WING_SLABS: [number, number, number][] = [
  [HX * 2, 0.6, HZ * 0.7],
  [HX * 1.45, 0.6, HZ * 0.36],
  [HX * 0.85, 0.6, 0],
  [BODY_R * 2.6, 0.6, -HZ * 0.34],
]
const WING = WING_SLABS.map(([span, depth]) => boxGeometry(span, 0.05, depth))
const FIN = boxGeometry(0.04, 0.5, 0.55)
const HUB = cylinderGeometry(0.07, 0.09, 0.18, 10)
HUB.rotateX(Math.PI / 2)
const BLADE = boxGeometry(0.9, 0.05, 0.03)
const LIGHT = sphereGeometry(0.06, 8, 6)

/**
 * A loitering drone: a delta wing with a cylinder body, wingtip fins and a pusher prop at the tail,
 * with a light in its pilot's colour on the nose. The prop spins while it is flown and winds down
 * once the link is lost.
 */
export class LoiterModel implements VehicleModel {
  readonly root = new Group()
  private readonly prop = new Group()
  private readonly skin = toonMaterial({ color: SKIN })
  private readonly trim = toonMaterial({ color: TRIM })
  private readonly light = toonMaterial({ color: 0xffffff })
  private spin = 0
  private speed = 0
  private last = performance.now()

  constructor(_id: number) {
    const body = new Mesh(FUSELAGE, this.skin)
    body.castShadow = true
    this.root.add(body)
    const nose = new Mesh(NOSE, this.skin)
    nose.position.set(0, 0, -HZ * 0.9 - 0.22)
    this.root.add(nose)
    WING_SLABS.forEach(([, , z], index) => {
      const slab = new Mesh(WING[index]!, this.skin)
      slab.position.set(0, -0.02, z)
      slab.castShadow = true
      this.root.add(slab)
    })
    for (const side of [-1, 1]) {
      const fin = new Mesh(FIN, this.trim)
      fin.position.set(side * (HX - 0.03), 0.2, HZ * 0.72)
      this.root.add(fin)
    }
    const hub = new Mesh(HUB, this.trim)
    this.prop.add(hub)
    const blade = new Mesh(BLADE, this.trim)
    this.prop.add(blade)
    this.prop.position.set(0, 0, HZ * 0.9 + 0.1)
    this.root.add(this.prop)
    const light = new Mesh(LIGHT, this.light)
    light.position.set(0, 0.12, -HZ * 0.75)
    this.root.add(light)
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
    this.prop.rotation.z = this.spin
    if (pilot !== 0) this.light.color.setHex(colorFor(pilot))
  }

  /** Frees what this drone owns; the shared geometry stays. */
  dispose() {
    this.skin.dispose()
    this.trim.dispose()
    this.light.dispose()
    this.root.removeFromParent()
  }
}
