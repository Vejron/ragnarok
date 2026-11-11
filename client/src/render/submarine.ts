import { Group, Mesh, boxGeometry, cylinderGeometry, sphereGeometry, type Geometry, type Material } from '@ragnarok/engine'
import type { VehicleModel } from './car'
import { toonMaterial } from './toon'
import type { Sub } from '@/entities/units'
import { SUB_MAST_UP } from '@/net/codec'
import { SUB_HALF_EXTENTS, SUB_MAST_HEIGHT, SUB_SAIL_HALF, SUB_SAIL_Z, SUB_TUBE_AT } from '@/sim/consts'

const [, HY, HZ] = SUB_HALF_EXTENTS
/** The pressure hull's radius, metres: the box's half height, so it fills it top to bottom. */
const R = HY
/** The sail's top, in the hull frame. */
const SAIL_TOP = HY + 2 * SUB_SAIL_HALF[1]
/** How far the periscope stands out of the sail when it is raised, metres, and how quickly it rises or sinks, per second. */
const MAST_OUT = SUB_MAST_HEIGHT - SAIL_TOP
const MAST_RATE = 2.5
/** The screw spins this fast with a captain at the helm, rad/s. */
const SCREW_SPIN = 9
/** A tube's lid, a side, metres, and how far it swings up from shut when the tube is empty, radians. */
const LID = 0.7
const LID_OPEN = 1.7
/** How quickly a lid swings, per second. */
const LID_RATE = 4

const HULL_GREY = 0x3b4046
const DECK_GREY = 0x2c3034
const BELLY_RED = 0x5e2a28

// Shared by every submarine: parts only ever move, so none of these is disposed.
/** The hull's middle, a cylinder along Z, its rounded bow and its tapering tail. */
const BODY_LENGTH = 2 * HZ - 2 * R - 3.2
const BODY = cylinderGeometry(R, R, BODY_LENGTH, 18)
BODY.rotateX(Math.PI / 2)
const BOW = sphereGeometry(R, 18, 10)
const TAIL = cylinderGeometry(R, 0.25, 4.4, 18)
TAIL.rotateX(Math.PI / 2)
/** A flat walkway along the top, and a dark stripe along the bottom. */
const DECK = boxGeometry(1.3, 0.12, BODY_LENGTH + 1.5)
const KEEL = boxGeometry(0.5, 0.25, BODY_LENGTH)
const SAIL = boxGeometry(2 * SUB_SAIL_HALF[0], 2 * SUB_SAIL_HALF[1], 2 * SUB_SAIL_HALF[2])
const SAIL_CAP = cylinderGeometry(SUB_SAIL_HALF[0], SUB_SAIL_HALF[0], 2 * SUB_SAIL_HALF[1], 12)
const PLANE = boxGeometry(3.2, 0.08, 0.7)
const FIN = boxGeometry(0.08, 2.0, 0.9)
const STERN_PLANE = boxGeometry(3.4, 0.08, 0.9)
const BLADE = boxGeometry(0.18, 1.2, 0.05)
const HUB = cylinderGeometry(0.18, 0.12, 0.4, 10)
HUB.rotateX(Math.PI / 2)
const MAST = cylinderGeometry(0.08, 0.1, 2.6, 8)
const SCOPE_HEAD = boxGeometry(0.2, 0.28, 0.32)
const LENS = boxGeometry(0.14, 0.1, 0.02)
const LID_GEOMETRY = boxGeometry(LID, 0.06, LID)
const MOUTH = cylinderGeometry(0.3, 0.3, 0.02, 12)

/**
 * A small diesel-electric submarine: a grey cylinder of a hull with a rounded bow and a tapering tail, a
 * walkway along its top, a sail with diving planes forward of the middle, cruciform tail fins and a
 * screw. Its periscope rises out of the sail while its officer is at it (`SUB_MAST_UP`) and turns with
 * their look; behind the sail the lids of its four launch tubes stand open while a tube is empty and
 * shut again once it is loaded. The screw turns while its captain is aboard.
 */
export class SubmarineModel implements VehicleModel {
  readonly root = new Group()
  private readonly hull = toonMaterial({ color: HULL_GREY })
  private readonly deck = toonMaterial({ color: DECK_GREY })
  private readonly belly = toonMaterial({ color: BELLY_RED })
  private readonly glass = toonMaterial({ color: 0x9fd4ff })
  private readonly dark = toonMaterial({ color: 0x0d0f11 })
  /** The periscope: its mast slides up and down inside the sail and turns about its own axis. */
  readonly periscope = new Group()
  private readonly screw = new Group()
  /** Each tube's lid, hinged at its forward edge. */
  readonly lids: Group[] = []
  /** How far the periscope is out of the sail, 0..1, and each lid is open, 0..1, as drawn. */
  private raised = 0
  private readonly open = SUB_TUBE_AT.map(() => 0)
  private manned = false
  private last = performance.now()

  constructor(_id: number) {
    const part = (geometry: Geometry, material: Material, x: number, y: number, z: number, parent: Group = this.root) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }
    const front = -HZ + R
    part(BODY, this.hull, 0, 0, front + BODY_LENGTH / 2)
    part(BOW, this.hull, 0, 0, front)
    part(TAIL, this.hull, 0, 0, front + BODY_LENGTH + 2.2)
    part(DECK, this.deck, 0, R - 0.05, front + BODY_LENGTH / 2 - 0.3)
    part(KEEL, this.belly, 0, -R + 0.05, front + BODY_LENGTH / 2)

    // The sail, rounded at its front and back, with its planes.
    part(SAIL, this.hull, 0, HY + SUB_SAIL_HALF[1], SUB_SAIL_Z)
    for (const end of [-1, 1]) part(SAIL_CAP, this.hull, 0, HY + SUB_SAIL_HALF[1], SUB_SAIL_Z + end * SUB_SAIL_HALF[2])
    part(PLANE, this.deck, 0, HY + SUB_SAIL_HALF[1] * 1.2, SUB_SAIL_Z - SUB_SAIL_HALF[2] * 0.4)

    // The tail's fins and planes, and the screw behind them.
    const tail = HZ - 1.2
    part(FIN, this.deck, 0, 0, tail)
    part(STERN_PLANE, this.deck, 0, 0, tail)
    this.screw.position.set(0, 0, HZ + 0.15)
    this.root.add(this.screw)
    part(HUB, this.dark, 0, 0, 0, this.screw)
    for (const turn of [0, Math.PI / 2, Math.PI, (3 * Math.PI) / 2]) {
      const blade = part(BLADE, this.dark, 0, 0, 0, this.screw)
      blade.rotation.z = turn
      blade.position.set(Math.sin(turn) * 0.55, Math.cos(turn) * 0.55, 0)
    }

    // The periscope, its head at `SUB_MAST_HEIGHT` when raised, looking along -Z.
    this.periscope.position.set(0, SAIL_TOP - MAST_OUT, SUB_SAIL_Z)
    this.root.add(this.periscope)
    part(MAST, this.deck, 0, MAST_OUT - 1.3, 0, this.periscope)
    part(SCOPE_HEAD, this.deck, 0, MAST_OUT + 0.05, 0, this.periscope)
    part(LENS, this.glass, 0, MAST_OUT + 0.08, -0.17, this.periscope).castShadow = false

    // The launch tubes: a dark mouth in the deck under each lid.
    for (const [x, y, z] of SUB_TUBE_AT) {
      part(MOUTH, this.dark, x, y + 0.01, z).castShadow = false
      const hinge = new Group()
      hinge.position.set(x, y + 0.04, z - LID / 2)
      this.root.add(hinge)
      part(LID_GEOMETRY, this.deck, 0, 0, LID / 2, hinge)
      this.lids.push(hinge)
    }
  }

  /** Raises and turns the periscope and swings the lids from the snapshot's `sub`, easing over `dt` seconds. */
  setSub(sub: Sub | null, dt: number) {
    const step = Math.min(1, Math.max(0, dt))
    const up = sub !== null && (sub.flags & SUB_MAST_UP) !== 0
    this.raised = up ? Math.min(1, this.raised + (MAST_RATE / MAST_OUT) * step) : Math.max(0, this.raised - (MAST_RATE / MAST_OUT) * step)
    this.periscope.position.y = SAIL_TOP - MAST_OUT + this.raised * MAST_OUT
    this.periscope.visible = this.raised > 0.01
    if (sub) this.periscope.rotation.y = sub.scope
    this.lids.forEach((lid, tube) => {
      const empty = sub !== null && (sub.tubes & (1 << tube)) === 0
      const was = this.open[tube]!
      this.open[tube] = empty ? Math.min(1, was + LID_RATE * step) : Math.max(0, was - LID_RATE * step)
      // Swung up and over towards the bow.
      lid.rotation.x = this.open[tube]! * LID_OPEN
    })
  }

  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number], _steer: number, seats: readonly number[]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    this.manned = (seats[0] ?? 0) !== 0
    const now = performance.now()
    const dt = Math.min(0.1, (now - this.last) / 1000)
    this.last = now
    if (this.manned) this.screw.rotation.z += SCREW_SPIN * dt
  }

  /** Frees what this submarine owns; the shared geometry stays. */
  dispose() {
    for (const material of [this.hull, this.deck, this.belly, this.glass, this.dark]) material.dispose()
    this.root.removeFromParent()
  }
}
