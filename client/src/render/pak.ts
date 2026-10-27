import { Group, Mesh, boxGeometry, cylinderGeometry, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import type { VehicleModel } from './car'
import type { Turret } from '@/entities/units'
import { TURRET_STOWED } from '@/net/codec'
import { PAK_AXLES_Z, PAK_BARREL_LENGTH, PAK_GUN_PIVOT, PAK_SEAT_HEAD, PAK_TONGUE_Z, PAK_TRACK_HALF, PAK_WHEEL_RADIUS } from '@/sim/consts'

/** The road wheels' axle (`axles` in `pak.rs`; the second is the trails' spades). */
const AXLE_Z = PAK_AXLES_Z[0]
/** Wheel centres under the hull (the hard point less the suspension's rest). */
const WHEEL_Y = -0.3
/** Where the wheels stand, in the hull frame. */
const GROUND_Y = WHEEL_Y - PAK_WHEEL_RADIUS
/** The barrel runs back on its recoil slide and is pushed home again, seconds and metres. */
const RECOIL_SECONDS = 0.45
const RECOIL_STROKE = 0.55
/** Closing or opening the trails, seconds. */
export const STOW_SECONDS = 1.2
const PAINT = 0x8f7d52
const STEEL = 0x3d4042
const RUBBER = 0x1d1f20

/** Each trail's hinge on the axle carriage, its x mirrored for the right trail. */
const HINGE: [number, number, number] = [0.16, -0.28, AXLE_Z + 0.15]
/** Where the closed trails meet: the towing eye the car's joint holds (`PAK_TONGUE_Z`). */
const EYE: [number, number, number] = [0, 0, PAK_TONGUE_Z]
const TRAIL_LENGTH = Math.hypot(EYE[0] - HINGE[0], EYE[1] - HINGE[1], EYE[2] - HINGE[2])
/** A trail's yaw at its hinge closed, in to the eye, and spread out to fire (the right one's; mirrored for the left). */
const TRAIL_CLOSED = Math.atan2(EYE[0] - HINGE[0], EYE[2] - HINGE[2])
const TRAIL_OPEN = 0.4
/** A trail's pitch closed (up to the hitch) and open (down so the spade bites the ground). */
const TRAIL_RAISED = Math.asin((EYE[1] - HINGE[1]) / TRAIL_LENGTH)
const TRAIL_DOWN = -Math.asin((HINGE[1] - (GROUND_Y + 0.08)) / TRAIL_LENGTH)

// Shared by every Pak: parts only ever move, so none of these is disposed.
const AXLE = cylinderGeometry(0.05, 0.05, 2 * PAK_TRACK_HALF, 8)
AXLE.rotateZ(Math.PI / 2)
const SADDLE = boxGeometry(0.5, 0.22, 0.5)
const TYRE = cylinderGeometry(PAK_WHEEL_RADIUS, PAK_WHEEL_RADIUS, 0.14, 18)
// Turned onto its axle, which runs across the gun, so the wheel spins about X.
TYRE.rotateZ(Math.PI / 2)
const DISC = cylinderGeometry(PAK_WHEEL_RADIUS - 0.07, PAK_WHEEL_RADIUS - 0.07, 0.16, 18)
DISC.rotateZ(Math.PI / 2)
// Six-sided so the hub shows the wheel turning.
const HUB = cylinderGeometry(0.09, 0.09, 0.22, 6)
HUB.rotateZ(Math.PI / 2)
const TRAIL = boxGeometry(0.11, 0.15, TRAIL_LENGTH)
TRAIL.translate(0, 0, TRAIL_LENGTH / 2)
const SPADE = boxGeometry(0.34, 0.3, 0.04)
SPADE.translate(0, -0.15, 0)
const EYE_RING = cylinderGeometry(0.08, 0.08, 0.05, 10)
const APRON = boxGeometry(1.1, 0.32, 0.02)
const PIVOT_BLOCK = boxGeometry(0.34, 0.2, 0.4)
/** The shield: a main plate the barrel passes through, and a second, spaced plate in front of it. */
const SHIELD = boxGeometry(0.62, 0.78, 0.02)
const SHIELD_FRONT = boxGeometry(0.66, 0.72, 0.02)
const SHIELD_TOP = boxGeometry(1.5, 0.1, 0.02)
const CRADLE = boxGeometry(0.2, 0.18, 1.1)
const RECUPERATOR = cylinderGeometry(0.05, 0.05, 1.0, 8)
RECUPERATOR.rotateX(-Math.PI / 2)
const TUBE = cylinderGeometry(0.03, 0.05, PAK_BARREL_LENGTH, 12)
// The barrel's own axis runs along -Z from the trunnions, so it lies along the gun's aim at zero angles.
TUBE.rotateX(-Math.PI / 2)
TUBE.translate(0, 0, -PAK_BARREL_LENGTH / 2)
const BAFFLE = cylinderGeometry(0.07, 0.07, 0.08, 10)
BAFFLE.rotateX(-Math.PI / 2)
const BREECH = boxGeometry(0.2, 0.2, 0.32)
const SIGHT = boxGeometry(0.06, 0.08, 0.26)
const WHEEL_GUARD = boxGeometry(0.06, 0.3, 0.3)

const smooth = (from: number, to: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - from) / (to - from)))
  return t * t * (3 - 2 * t)
}
const lerp = (a: number, b: number, t: number) => a + (b - a) * t

/** One split trail: the yaw at its hinge, the pitch inside it, and the spade folding at its tip. */
interface Trail {
  readonly yaw: Group
  readonly pitch: Group
  readonly spade: Group
  /** +1 for the right trail, -1 for the left. */
  readonly side: number
}

/**
 * The Pak 38 anti-tank gun: two disc wheels on one axle, two split trails back to the spades, and a
 * long 5 cm barrel with a double-baffle muzzle brake behind a spaced shield. The upper carriage turns
 * with the gun's traverse, shield and all, and the cradle lifts with its elevation, so the barrel's
 * line is the server's aim.
 *
 * Hitched (`TURRET_STOWED`) the barrel comes back to the centre line, level, and the trails close and
 * lift into the towing eye, so it travels muzzle-last behind the car. Let go, the trails spread again.
 */
export class PakModel implements VehicleModel {
  readonly root = new Group()
  /** Turns with the traverse: the hull-relative yaw. */
  private readonly mount = new Group()
  /** Lifts with the elevation, about the trunnions. */
  private readonly elevation = new Group()
  /** Runs back along the barrel's line when it fires. */
  private readonly recoiling = new Group()
  private readonly trails: Trail[] = []
  /** Where the gunner's head is, kneeling behind the shield. */
  private readonly seat = new Group()
  private readonly spinners: Node[] = []
  private readonly paint = toonMaterial({ color: PAINT })
  private readonly steel = toonMaterial({ color: STEEL })
  private readonly rubber = toonMaterial({ color: RUBBER })
  private roll = 0
  private placed = false
  /** 0 set up to fire, 1 closed up for travel. */
  private stow = 0
  /** `performance.now()` of the last `update`, for the fold's pace. */
  private updatedAt = 0
  /** `performance.now()` of the last shot, for the barrel's recoil. */
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

    this.seat.position.set(...PAK_SEAT_HEAD)
    this.mount.add(this.seat)
  }

  private part(geometry: ConstructorParameters<typeof Mesh>[0], material: ConstructorParameters<typeof Mesh>[1], parent: Node, x: number, y: number, z: number, shadow = true) {
    const mesh = new Mesh(geometry, material)
    mesh.position.set(x, y, z)
    mesh.castShadow = shadow
    parent.add(mesh)
    return mesh
  }

  /** The axle and its saddle, the road wheels, and the apron hung under the shield. */
  private buildCarriage() {
    this.part(AXLE, this.steel, this.root, 0, WHEEL_Y, AXLE_Z)
    this.part(SADDLE, this.paint, this.root, 0, -0.22, AXLE_Z)
    for (const side of [-1, 1]) {
      const wheel = new Group()
      wheel.name = side < 0 ? 'wheel_L' : 'wheel_R'
      wheel.position.set(side * PAK_TRACK_HALF, WHEEL_Y, AXLE_Z)
      this.part(TYRE, this.rubber, wheel, 0, 0, 0)
      this.part(DISC, this.paint, wheel, 0, 0, 0)
      this.part(HUB, this.steel, wheel, side * 0.03, 0, 0, false)
      this.spinners.push(wheel)
      this.root.add(wheel)
      this.part(WHEEL_GUARD, this.paint, this.root, side * (PAK_TRACK_HALF - 0.14), -0.1, AXLE_Z)
    }
    // The lower shield, between the wheels: it does not traverse.
    const apron = this.part(APRON, this.paint, this.root, 0, -0.42, AXLE_Z - 0.3)
    apron.rotation.x = -0.15
  }

  /** One trail, hinged on the axle carriage, with a spade at its tip; the left one carries the towing eye. */
  private buildTrail(side: number): Trail {
    const yaw = new Group()
    yaw.name = side < 0 ? 'trail_L' : 'trail_R'
    yaw.position.set(side * HINGE[0], HINGE[1], HINGE[2])
    const pitch = new Group()
    yaw.add(pitch)
    this.part(TRAIL, this.paint, pitch, 0, 0, 0)
    const tip = new Group()
    tip.name = side < 0 ? 'tip_L' : 'tip_R'
    tip.position.set(0, 0, TRAIL_LENGTH)
    pitch.add(tip)
    const spade = new Group()
    spade.position.set(0, -0.06, -0.12)
    this.part(SPADE, this.steel, spade, 0, 0, 0)
    tip.add(spade)
    if (side < 0) this.part(EYE_RING, this.steel, tip, -side * 0.06, 0, 0.04)
    this.root.add(yaw)
    return { yaw, pitch, spade, side }
  }

  /** The upper carriage with its shield and sight, and the cradle, recuperator and recoiling barrel it lifts. */
  private buildGun() {
    this.mount.name = 'mount'
    this.mount.position.set(...PAK_GUN_PIVOT)
    this.root.add(this.mount)
    this.part(PIVOT_BLOCK, this.paint, this.mount, 0, -0.3, 0.05)
    // Two plates either side of the barrel's slot, with the top plate across, and the spaced plate ahead.
    for (const side of [-1, 1]) {
      const plate = this.part(SHIELD, this.paint, this.mount, side * 0.43, -0.05, -0.22)
      plate.rotation.set(-0.18, side * -0.12, 0)
      const front = this.part(SHIELD_FRONT, this.paint, this.mount, side * 0.45, -0.08, -0.3)
      front.rotation.set(-0.18, side * -0.12, 0)
    }
    const top = this.part(SHIELD_TOP, this.paint, this.mount, 0, 0.32, -0.15)
    top.rotation.x = -0.18
    this.part(SIGHT, this.steel, this.mount, -0.2, 0.12, 0.2, false)

    this.elevation.name = 'gun'
    this.mount.add(this.elevation)
    this.part(CRADLE, this.paint, this.elevation, 0, -0.04, -0.15)
    this.part(RECUPERATOR, this.steel, this.elevation, 0, -0.13, -0.3)

    this.elevation.add(this.recoiling)
    this.part(TUBE, this.paint, this.recoiling, 0, 0, 0)
    // The double-baffle muzzle brake.
    for (const back of [0.05, 0.17]) this.part(BAFFLE, this.steel, this.recoiling, 0, 0, -PAK_BARREL_LENGTH + back)
    this.part(BREECH, this.steel, this.recoiling, 0, 0, 0.4)
  }

  /**
   * Places the gun and lays it. `turret` is the gun's traverse (hull frame), its elevation and whether
   * it is hitched. Its gunner is drawn kneeling behind the shield by `Game` (`seatFrame`).
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
      // Wheels turn with the ground they roll over: it is towed muzzle-last, so they run backwards then.
      const ahead = this.forward.x * (position[0] - this.last.x) + this.forward.y * (position[1] - this.last.y) + this.forward.z * (position[2] - this.last.z)
      this.roll -= ahead / PAK_WHEEL_RADIUS
      const step = Math.max(0, now - this.updatedAt) / 1000 / STOW_SECONDS
      this.stow = stowed > this.stow ? Math.min(stowed, this.stow + step) : Math.max(stowed, this.stow - step)
    } else {
      // One first seen already hitched (or already set up) is drawn that way, not folding.
      this.stow = stowed
    }
    this.last.set(position[0], position[1], position[2])
    this.updatedAt = now
    this.placed = true
    for (const wheel of this.spinners) wheel.rotation.x = this.roll

    // The barrel comes to the centre line first, then the trails close.
    const gun = smooth(0, 0.4, this.stow)
    const legs = smooth(0.3, 1, this.stow)
    this.mount.rotation.y = lerp(turret?.yaw ?? 0, 0, gun)
    this.elevation.rotation.x = lerp(turret?.pitch ?? 0, 0, gun)
    for (const trail of this.trails) {
      trail.yaw.rotation.y = trail.side * lerp(TRAIL_OPEN, TRAIL_CLOSED, legs)
      // Positive pitch about X tips the +Z tip down; the raised one is a negative turn.
      trail.pitch.rotation.x = lerp(-TRAIL_DOWN, -TRAIL_RAISED, legs)
      // Upright to dig in, or folded flat under the trail.
      trail.spade.rotation.x = lerp(TRAIL_DOWN, Math.PI / 2, legs)
    }

    // The barrel runs back along its own line and is pushed home.
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

  /** A shell was just fired: the barrel runs back. */
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

  /** Frees what this gun owns; the shared geometry stays. */
  dispose() {
    this.paint.dispose()
    this.steel.dispose()
    this.rubber.dispose()
    this.root.removeFromParent()
  }
}
