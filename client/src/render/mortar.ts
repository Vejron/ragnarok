import { Group, Mesh, boxGeometry, cylinderGeometry, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import type { VehicleModel } from './car'
import type { Turret } from '@/entities/units'
import { TURRET_STOWED } from '@/net/codec'
import { MORTAR_BARREL_LENGTH, MORTAR_GUN_PIVOT, MORTAR_SEAT_HEAD, MORTAR_TRACK_HALF, MORTAR_WHEEL_RADIUS } from '@/sim/consts'

/** Fore and aft of each axle, metres (`axles` in `spec.rs`). */
const AXLE_Z = 0.9
/** Wheel centres under the hull (the hard point less the suspension's rest). */
const WHEEL_Y = -0.55
/** Where the wheels stand, in the hull frame. */
const GROUND_Y = WHEEL_Y - MORTAR_WHEEL_RADIUS
/** The barrel slides back and eases home over this long, seconds and metres. */
const RECOIL_SECONDS = 0.6
const RECOIL_STROKE = 0.5
/** A full fold from firing to travel, or back, seconds. */
export const STOW_SECONDS = 1.8
const PAINT = 0x56653a
const STEEL = 0x3e4244
const RUBBER = 0x1d1f20

/** Each trail's hinge on the carriage, its x mirrored for the right trail. */
const HINGE: [number, number, number] = [0.3, -0.3, -0.3]
/** Where the closed trails meet: at the tow end, by the tongue anchor the car's joint holds (`tongue_anchor`). */
const TIP: [number, number, number] = [0, -0.1, -1.75]
const TRAIL_LENGTH = Math.hypot(TIP[0] - HINGE[0], TIP[1] - HINGE[1], TIP[2] - HINGE[2])
/** A trail's yaw at its hinge closed (the left one's; mirrored for the right), and how far out it opens. */
const TRAIL_CLOSED = Math.atan2(HINGE[0] - TIP[0], -(TIP[2] - HINGE[2]))
const TRAIL_OPEN = -0.28
/** A trail's pitch closed (up to the hitch) and open (down so the spade bites the ground). */
const TRAIL_RAISED = Math.asin((TIP[1] - HINGE[1]) / TRAIL_LENGTH)
const TRAIL_DOWN = -Math.asin((HINGE[1] - (GROUND_Y + 0.05)) / TRAIL_LENGTH)
/** The firing pad's height down on the ground and drawn up under the carriage. */
const PAD_DOWN = GROUND_Y + 0.04
const PAD_UP = -0.42
/** Where the barrel lies in its travel lock: swung round over the rear, level. */
const TRAVEL_YAW = Math.PI
const TRAVEL_PITCH = 0

// Shared by every mortar: parts only ever move, so none of these is disposed.
const CARRIAGE = boxGeometry(1.1, 0.4, 2.3)
const BEAM = boxGeometry(0.1, 0.14, 1.9)
const BOGIE_MOUNT = boxGeometry(0.3, 0.3, 0.35)
const FENDER = boxGeometry(0.38, 0.04, 2.1)
const TYRE = cylinderGeometry(MORTAR_WHEEL_RADIUS, MORTAR_WHEEL_RADIUS, 0.3, 16)
// Turned onto its axle, which runs across the mortar, so the wheel spins about X.
TYRE.rotateZ(Math.PI / 2)
// Six-sided so the hub shows the wheel turning.
const HUB = cylinderGeometry(0.2, 0.2, 0.32, 6)
HUB.rotateZ(Math.PI / 2)
const LOCK_POST = boxGeometry(0.14, 0.62, 0.14)
const LOCK_JAW = boxGeometry(0.06, 0.22, 0.16)
const PAD = cylinderGeometry(0.42, 0.46, 0.08, 14)
const JACK = cylinderGeometry(0.08, 0.08, 0.5, 8)
JACK.translate(0, 0.29, 0)
const TRAIL = boxGeometry(0.16, 0.2, TRAIL_LENGTH)
TRAIL.translate(0, 0, -TRAIL_LENGTH / 2)
const SPADE = boxGeometry(0.46, 0.42, 0.05)
SPADE.translate(0, -0.21, 0)
const LUNETTE = cylinderGeometry(0.12, 0.12, 0.06, 12)
const TURNTABLE = cylinderGeometry(0.45, 0.5, 0.12, 16)
const UPPER = boxGeometry(0.8, 0.25, 0.9)
const CHEEK = boxGeometry(0.06, 0.55, 0.5)
const APU = boxGeometry(0.34, 0.36, 0.55)
const EXHAUST = cylinderGeometry(0.035, 0.035, 0.3, 6)
const CRADLE = boxGeometry(0.32, 0.28, 1.3)
const RECUPERATOR = cylinderGeometry(0.06, 0.06, 1.1, 8)
RECUPERATOR.rotateX(-Math.PI / 2)
const TUBE = cylinderGeometry(0.075, 0.11, MORTAR_BARREL_LENGTH, 12)
// The barrel's own axis runs along -Z from the mount, so it lies along the gun's aim at zero angles.
TUBE.rotateX(-Math.PI / 2)
TUBE.translate(0, 0, -MORTAR_BARREL_LENGTH / 2)
const EXTRACTOR = cylinderGeometry(0.115, 0.115, 0.32, 12)
EXTRACTOR.rotateX(-Math.PI / 2)
const BRAKE = boxGeometry(0.28, 0.18, 0.34)
const BREECH = boxGeometry(0.3, 0.3, 0.4)

const WHEEL_SPOTS: [string, number, number][] = [
  ['FL', -MORTAR_TRACK_HALF, -AXLE_Z],
  ['FR', MORTAR_TRACK_HALF, -AXLE_Z],
  ['RL', -MORTAR_TRACK_HALF, AXLE_Z],
  ['RR', MORTAR_TRACK_HALF, AXLE_Z],
]

const smooth = (from: number, to: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - from) / (to - from)))
  return t * t * (3 - 2 * t)
}
const lerp = (a: number, b: number, t: number) => a + (b - a) * t
const wrap = (angle: number) => Math.atan2(Math.sin(angle), Math.cos(angle))

/** One split trail: the yaw at its hinge, the pitch inside it, and the spade folding at its tip. */
interface Trail {
  readonly yaw: Group
  readonly pitch: Group
  readonly spade: Group
  /** +1 for the right trail, -1 for the left. */
  readonly side: number
}

/**
 * The field gun, drawn after the Swedish Haubits 77: a low carriage on tandem wheels, two split trails
 * forward to the tow end, and a long barrel in a cradle on a turning upper carriage with its own power
 * unit. The mount turns with the gun's traverse and the cradle lifts with its elevation, so the barrel's
 * line is the server's aim.
 *
 * Hitched (`TURRET_STOWED`) it folds for travel: the barrel swings round onto the lock over the rear,
 * then the trails close into a tow bar at the hitch and the firing pad lifts. Let go, it unfolds the same
 * way backwards. The fold is only drawn: the server fires only an unhitched gun.
 */
export class MortarModel implements VehicleModel {
  readonly root = new Group()
  /** Turns with the traverse: the hull-relative yaw. */
  private readonly mount = new Group()
  /** Lifts with the elevation, about the mount's trunnions. */
  private readonly elevation = new Group()
  /** Slides back along the barrel's line when it fires. */
  private readonly recoiling = new Group()
  private readonly pad = new Group()
  private readonly trails: Trail[] = []
  /** Where the gunner's head is, riding the mount. */
  private readonly seat = new Group()
  private readonly spinners: Node[] = []
  private readonly paint = toonMaterial({ color: PAINT })
  private readonly steel = toonMaterial({ color: STEEL })
  private readonly rubber = toonMaterial({ color: RUBBER })
  private roll = 0
  private placed = false
  /** 0 set up to fire, 1 folded for travel. */
  private stow = 0
  /** `performance.now()` of the last `update`, for the fold's pace. */
  private updatedAt = 0
  /** `performance.now()` of the last shot, for the barrel's recoil stroke. */
  private firedAt = -Infinity
  private readonly last = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly pivot = new THREE.Vector3()
  private readonly aim = new THREE.Vector3()
  private readonly matrix = new THREE.Matrix4()

  constructor(_id: number) {
    this.buildCarriage()
    for (const side of [-1, 1]) this.trails.push(this.buildTrail(side))
    this.buildGun()

    this.seat.position.set(...MORTAR_SEAT_HEAD)
    this.mount.add(this.seat)
  }

  private part(geometry: ConstructorParameters<typeof Mesh>[0], material: ConstructorParameters<typeof Mesh>[1], parent: Node, x: number, y: number, z: number, shadow = true) {
    const mesh = new Mesh(geometry, material)
    mesh.position.set(x, y, z)
    mesh.castShadow = shadow
    parent.add(mesh)
    return mesh
  }

  /** The lower carriage: the body, the tandem bogies and their wheels, the travel lock and the firing pad. */
  private buildCarriage() {
    this.part(CARRIAGE, this.paint, this.root, 0, -0.25, 0)
    for (const side of [-1, 1]) {
      this.part(BEAM, this.steel, this.root, side * 0.64, WHEEL_Y, 0)
      this.part(BOGIE_MOUNT, this.steel, this.root, side * 0.55, -0.4, 0)
      this.part(FENDER, this.paint, this.root, side * MORTAR_TRACK_HALF, -0.03, 0)
    }
    for (const [name, x, z] of WHEEL_SPOTS) {
      const wheel = new Group()
      wheel.name = `wheel_${name}`
      wheel.position.set(x, WHEEL_Y, z)
      this.part(TYRE, this.rubber, wheel, 0, 0, 0)
      this.part(HUB, this.steel, wheel, 0, 0, 0, false)
      this.spinners.push(wheel)
      this.root.add(wheel)
    }

    // The travel lock the barrel rests in over the rear, its jaws either side of the tube.
    this.part(LOCK_POST, this.steel, this.root, 0, 0.26, 1.3)
    for (const side of [-1, 1]) this.part(LOCK_JAW, this.steel, this.root, side * 0.15, MORTAR_GUN_PIVOT[1], 1.3)

    this.pad.name = 'pad'
    this.part(PAD, this.steel, this.pad, 0, 0, 0)
    this.part(JACK, this.steel, this.pad, 0, 0, 0, false)
    this.root.add(this.pad)
  }

  /** One trail, hinged under the carriage, with a spade at its tip; the left one carries the lunette. */
  private buildTrail(side: number): Trail {
    const yaw = new Group()
    yaw.name = side < 0 ? 'trail_L' : 'trail_R'
    yaw.position.set(side * HINGE[0], HINGE[1], HINGE[2])
    const pitch = new Group()
    yaw.add(pitch)
    this.part(TRAIL, this.paint, pitch, 0, 0, 0)
    const tip = new Group()
    tip.name = side < 0 ? 'tip_L' : 'tip_R'
    tip.position.set(0, 0, -TRAIL_LENGTH)
    pitch.add(tip)
    const spade = new Group()
    spade.position.set(0, -0.08, 0.08)
    this.part(SPADE, this.steel, spade, 0, 0, 0)
    tip.add(spade)
    if (side < 0) this.part(LUNETTE, this.steel, tip, -side * 0.08, 0.02, -0.06)
    this.root.add(yaw)
    return { yaw, pitch, spade, side }
  }

  /** The upper carriage on its turntable, and the cradle, recuperators and recoiling barrel it lifts. */
  private buildGun() {
    this.mount.name = 'mount'
    this.mount.position.set(...MORTAR_GUN_PIVOT)
    this.root.add(this.mount)
    this.part(TURNTABLE, this.steel, this.mount, 0, -0.69, 0)
    this.part(UPPER, this.paint, this.mount, 0, -0.55, 0.1)
    for (const side of [-1, 1]) this.part(CHEEK, this.paint, this.mount, side * 0.3, -0.22, 0)
    // The power unit that lets an FH77 drive itself into position, on the left of the upper carriage.
    this.part(APU, this.paint, this.mount, -0.62, -0.45, 0.25)
    this.part(EXHAUST, this.steel, this.mount, -0.62, -0.15, 0.45, false)

    this.elevation.name = 'gun'
    this.mount.add(this.elevation)
    this.part(CRADLE, this.paint, this.elevation, 0, 0, -0.35)
    for (const y of [0.2, -0.2]) this.part(RECUPERATOR, this.steel, this.elevation, 0, y, -0.55)

    this.elevation.add(this.recoiling)
    this.part(TUBE, this.paint, this.recoiling, 0, 0, 0)
    this.part(EXTRACTOR, this.paint, this.recoiling, 0, 0, -0.6 * MORTAR_BARREL_LENGTH)
    this.part(BRAKE, this.steel, this.recoiling, 0, 0, -MORTAR_BARREL_LENGTH + 0.15)
    this.part(BREECH, this.steel, this.recoiling, 0, 0, 0.25)
  }

  /**
   * Places the mortar and aims it. `turret` is the mount's yaw (hull frame), the elevation and whether
   * the gun is hitched. Its gunner is drawn sitting on the mount by `Game` (`seatFrame`).
   */
  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    _seats: readonly number[],
    turret?: Turret | null,
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.forward.set(0, 0, -1).applyQuaternion(this.root.quaternion)
    const now = performance.now()
    const stowed = turret && (turret.flags & TURRET_STOWED) !== 0 ? 1 : 0
    if (this.placed) {
      // Wheels turn with the ground they roll over.
      const ahead = this.forward.x * (position[0] - this.last.x) + this.forward.y * (position[1] - this.last.y) + this.forward.z * (position[2] - this.last.z)
      this.roll -= ahead / MORTAR_WHEEL_RADIUS
      const step = Math.max(0, now - this.updatedAt) / 1000 / STOW_SECONDS
      this.stow = stowed > this.stow ? Math.min(stowed, this.stow + step) : Math.max(stowed, this.stow - step)
    } else {
      // One first seen already hitched (or already set up) is drawn that way, not unfolding.
      this.stow = stowed
    }
    this.last.set(position[0], position[1], position[2])
    this.updatedAt = now
    this.placed = true
    for (const wheel of this.spinners) wheel.rotation.x = this.roll

    // Folding, the gun goes first and the trails follow; unfolding runs the same way back.
    const gun = smooth(0, 0.55, this.stow)
    const legs = smooth(0.45, 1, this.stow)
    const yaw = turret?.yaw ?? 0
    const pitch = turret?.pitch ?? 0
    this.mount.rotation.y = yaw + wrap(TRAVEL_YAW - yaw) * gun
    this.elevation.rotation.x = lerp(pitch, TRAVEL_PITCH, gun)
    for (const trail of this.trails) {
      trail.yaw.rotation.y = trail.side * lerp(TRAIL_OPEN, TRAIL_CLOSED, legs)
      trail.pitch.rotation.x = lerp(TRAIL_DOWN, TRAIL_RAISED, legs)
      // Upright to dig in, or folded back flat under the trail.
      trail.spade.rotation.x = lerp(-TRAIL_DOWN, -Math.PI / 2, legs)
    }
    this.pad.position.y = lerp(PAD_DOWN, PAD_UP, legs)

    // The barrel slides back along its own line and eases home.
    const since = (now - this.firedAt) / 1000
    this.recoiling.position.z = since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) ** 2 : 0
  }

  /** The gunner's head point in the world and the mount's turn, as of the last `update`. */
  seatFrame(head: THREE.Vector3, rotation: THREE.Quaternion) {
    this.root.updateMatrixWorld(true)
    this.seat.getWorldPosition(head)
    const q = this.mount.getWorldQuaternion(new THREE.Quaternion())
    rotation.set(q.x, q.y, q.z, q.w)
  }

  /** A bomb was just fired: the barrel kicks back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The barrel's root in world space as of the last `update`, and its unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.elevation.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.elevation.matrixWorld))
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /** Frees what this mortar owns; the shared geometry stays. */
  dispose() {
    this.paint.dispose()
    this.steel.dispose()
    this.rubber.dispose()
    this.root.removeFromParent()
  }
}
