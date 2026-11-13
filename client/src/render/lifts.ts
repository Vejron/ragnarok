import { Geometry, Group, InstancedMesh, Mesh, Quaternion, Vector3, boxGeometry, cylinderGeometry, mat4 } from '@ragnarok/engine'
import { LIFT_BOARD_HEIGHT, LIFT_BOARD_RADIUS, LIFT_CHAIR_DROP, LIFT_SEAT_PITCH } from '@/sim/consts'
import { LiftPath, type Chair } from '@/sim/lifts'
import type { Lift } from '@/world'
import { ClientSim } from '@/sim/wasm/sim'
import { lineMaterials, painted, Wires } from './lines'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * Ski lifts (`WorldData.lifts`), every one drawn as a chair lift: its pylons and stations as the
 * physics stands them (`shared::lifts::structure`, through the WASM's `liftParts`, so what is drawn
 * is what is hit), the cable loop, and the chairs on it where the server's clock has them
 * (`sim/lifts.ts`). The pylons, stations and cable are one mesh a lift, built once; the chairs are
 * one instanced mesh for all lifts, placed every frame. LIFTS.md.
 */

const STEEL = 0x7d8488
const DARK_STEEL = 0x50565a
/** The colour of each `LOOK_*` of `crates/shared/src/lifts.rs`, in its order: steel, dark steel, concrete, roof. */
const LOOKS = [STEEL, DARK_STEEL, 0xa7a49c, 0x3c4a5a]
const SEAT = 0x24467a
const CABLE = 0x3b3f42
/** The cable's radius, metres. */
const CABLE_RADIUS = 0.022
/** Chairs further than this from the eye are not drawn, metres. */
const CHAIR_REACH = 1200
/**
 * Height of a sitting soldier's head over the seat, metres: the `sit` clip's head (`SIT_HEAD` over the
 * model's feet) with its hips at the seat.
 */
const HEAD_OVER_SEAT = 0.8

/** One chair, its seat's middle at the origin, facing -Z, hanging from the cable `LIFT_CHAIR_DROP` above. */
function chairGeometry(seats: number): Geometry {
  const width = seats * LIFT_SEAT_PITCH + 0.1
  const half = width / 2
  const parts: Geometry[] = [
    // The seat and the back, padded.
    painted(boxGeometry(width, 0.07, 0.5).translate(0, -0.035, -0.08), SEAT),
    painted(boxGeometry(width, 0.5, 0.06).translate(0, 0.3, 0.2), SEAT),
    // The frame under them, the hanger up to the grip on the cable, and the grip.
    painted(boxGeometry(width + 0.04, 0.04, 0.04).translate(0, -0.09, 0.2), DARK_STEEL),
    painted(boxGeometry(0.06, 0.06, 0.35).translate(0, 0.62, 0.38), STEEL),
    painted(cylinderGeometry(0.035, 0.035, LIFT_CHAIR_DROP - 0.6, 6).translate(0, 0.3 + LIFT_CHAIR_DROP / 2, 0.55), STEEL),
    painted(boxGeometry(0.06, 0.06, 0.4).translate(0, -0.09, 0.4), STEEL),
    painted(cylinderGeometry(0.035, 0.035, 0.7, 6).translate(0, 0.25, 0.55), STEEL),
    painted(boxGeometry(0.16, 0.22, 0.3).translate(0, LIFT_CHAIR_DROP + 0.02, 0.55), DARK_STEEL),
  ]
  // Armrests at the ends.
  for (const x of [-half, half]) {
    parts.push(painted(boxGeometry(0.04, 0.3, 0.04).translate(x, 0.15, -0.25), STEEL))
    parts.push(painted(boxGeometry(0.04, 0.04, 0.45).translate(x, 0.3, -0.03), STEEL))
  }
  return Geometry.merge(parts)
}

/**
 * The pylons and stations of `lift` as the physics stands them: each part of `liftParts` (nine numbers:
 * shape, middle, turn, size, look) as a box or a post in its look's colour.
 */
function structure(lift: Lift): Geometry[] {
  const cm = (metres: number) => Math.round(metres * 100)
  const flat = ClientSim.liftParts(new Int32Array(lift.points.flatMap(([x, z]) => [cm(x), cm(z)])), new Int32Array(lift.base.map(cm)), new Int32Array(lift.cable.map(cm)))
  const parts: Geometry[] = []
  for (let i = 0; i + 9 <= flat.length; i += 9) {
    const [shape, x, y, z, turn, a, b, c, look] = flat.slice(i, i + 9) as unknown as number[]
    const color = LOOKS[look!] ?? STEEL
    const geometry = shape === 0 ? boxGeometry(a! * 2, b! * 2, c! * 2).rotateY(turn!) : cylinderGeometry(a!, a!, b! * 2, a! > 1 ? 24 : 8)
    parts.push(painted(geometry.translate(x!, y!, z!), color))
  }
  return parts
}

/** The world's ski lifts, drawn, and where on them their riders sit. */
export class LiftView {
  readonly root = new Group()
  readonly paths: LiftPath[]
  private readonly chairs: InstancedMesh | null = null
  private readonly matrix = mat4.create()
  private readonly chair: Chair = { x: 0, y: 0, z: 0, yaw: 0, along: 0, stretch: 0 }

  constructor(readonly lifts: readonly Lift[]) {
    this.paths = lifts.map((lift) => new LiftPath(lift))
    if (lifts.length === 0) return
    const { solid, wire } = lineMaterials()
    for (const [index, lift] of lifts.entries()) {
      const path = this.paths[index]!
      if (path.chairs === 0) continue
      const parts = structure(lift)
      if (parts.length > 0) this.root.add(new Mesh(Geometry.merge(parts), solid))
      const wires = new Wires()
      const p = path.points
      const corners = p.length / 3
      for (let i = 0; i < corners; i++) {
        const j = (i + 1) % corners
        wires.add([p[i * 3]!, p[i * 3 + 1]!, p[i * 3 + 2]!], [p[j * 3]!, p[j * 3 + 1]!, p[j * 3 + 2]!], 0, CABLE_RADIUS, CABLE, 1)
      }
      const geometry = wires.geometry()
      if (geometry) this.root.add(new Mesh(geometry, wire))
    }
    // Every lift's chairs as one mesh, with as many seats as the widest lift's.
    const seats = Math.max(...this.paths.map((path) => path.seats))
    const capacity = this.paths.reduce((sum, path) => sum + path.chairs, 0)
    if (capacity === 0) return
    this.chairs = new InstancedMesh(chairGeometry(seats), toonMaterial({ vertexColors: true, snow: winter }), capacity)
    this.chairs.count = 0
    this.root.add(this.chairs)
  }

  /** Places every chair near the eye at server tick `tick` (fractional). */
  update(tick: number, eye: { x: number; z: number }) {
    const mesh = this.chairs
    if (!mesh) return
    let count = 0
    for (const path of this.paths) {
      for (let index = 0; index < path.chairs; index++) {
        const chair = path.chair(index, tick, this.chair)
        if (Math.hypot(chair.x - eye.x, chair.z - eye.z) > CHAIR_REACH) continue
        const half = chair.yaw / 2
        mat4.compose(this.matrix, chair.x, chair.y, chair.z, 0, Math.sin(half), 0, Math.cos(half), 1, 1, 1)
        mesh.setMatrixAt(count++, this.matrix)
      }
    }
    mesh.count = count
    mesh.needsUpdate()
  }

  /**
   * Where a rider of chair `chair` of lift `lift`, in place `seat`, sits at `tick`: their head into
   * `head` and the chair's turn into `turn`; their capsule centre is `seat` under the head, as the
   * server has it. False for a lift or chair there is not.
   */
  rider(lift: number, chair: number, seat: number, tick: number, head: Vector3, turn: Quaternion): boolean {
    const path = this.paths[lift]
    if (!path || chair >= path.chairs) return false
    const pose = path.chair(chair, tick, this.chair)
    const [x, y, z] = path.place(pose, seat)
    head.set(x, y + HEAD_OVER_SEAT, z)
    turn.set(0, Math.sin(pose.yaw / 2), 0, Math.cos(pose.yaw / 2))
    return true
  }

  /** Where the seat of a rider is at `tick`, `[x, y, z]` metres, or null. */
  seatOf(lift: number, chair: number, seat: number, tick: number): [number, number, number] | null {
    const path = this.paths[lift]
    if (!path || chair >= path.chairs) return null
    return path.place(path.chair(chair, tick, this.chair), seat)
  }

  /**
   * Whether a soldier standing at `position` (capsule centre) on ground `ground` can sit on a chair
   * now, as the server judges it (`chair_for`), but for a chair being full.
   */
  boardable(position: readonly number[], ground: (x: number, z: number) => number, standCentre: number, tick: number): boolean {
    const feet = position[1]! - standCentre
    for (const path of this.paths) {
      for (let index = 0; index < path.chairs; index++) {
        const chair = path.chair(index, tick, this.chair)
        if (Math.hypot(chair.x - position[0]!, chair.z - position[2]!) > LIFT_BOARD_RADIUS + 1) continue
        if (chair.y - ground(chair.x, chair.z) > LIFT_BOARD_HEIGHT || Math.abs(chair.y - feet) > LIFT_BOARD_HEIGHT) continue
        for (let seat = 0; seat < path.seats; seat++) {
          const [x, , z] = path.place(chair, seat)
          if (Math.hypot(x - position[0]!, z - position[2]!) <= LIFT_BOARD_RADIUS) return true
        }
      }
    }
    return false
  }
}
