import { Geometry, Group, Material, Mesh, Quaternion, Vector3, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import type { VehicleModel } from './car'
import { toonMaterial } from './toon'
import { RAFT_HALF_EXTENTS, RAFT_MOTOR_PIVOT, RAFT_MOTOR_SWING, RAFT_TILLER, RAFT_TURN_RATE } from '@/sim/consts'

const [HX, HY, HZ] = RAFT_HALF_EXTENTS
/** The tubes' radius, metres: their tops are the box's top. */
const TUBE = HY - 0.01
/** The side tubes' middles, out from the centre line, and their height. */
const SIDE_X = HX - TUBE
const TUBE_Y = HY - TUBE
/** The bow's half circle: its middle along the hull, and how far its tip kicks up, metres. */
const BOW_Z = -HZ + TUBE + SIDE_X
const KICK = 0.14
/** The tubes run on past the transom to their cone ends here. */
const STERN_Z = HZ - 0.12
/** Round each tube, and along each stretch of its path. */
const SIDES = 12
const BOW_STEPS = 10

/** The rafting colours a raft comes in, chosen by its id: red, yellow, blue and grey. */
const PAINTS = [0xd8432b, 0xf2b134, 0x2f6fb5, 0x4a5056] as const
const RUBBER_DARK = 0x26282b
const COWLING = 0x1b1c1e
const LEG = 0x3a3d40
const TANK = 0xc9302c
const PADDLE = 0xe8e2d0

/** The tubes' path, from the left stern round the bow to the right stern, and its height up the kick. */
function tubePath(): Vector3[] {
  const path: Vector3[] = [new Vector3(-SIDE_X, TUBE_Y, STERN_Z), new Vector3(-SIDE_X, TUBE_Y, 0.3)]
  for (let k = 0; k <= BOW_STEPS; k++) {
    const angle = Math.PI - (Math.PI * k) / BOW_STEPS
    // Up towards the tip, where the bow kicks up to ride over a wave.
    const up = KICK * Math.sin((Math.PI * k) / BOW_STEPS) ** 2
    path.push(new Vector3(SIDE_X * Math.cos(angle), TUBE_Y + up, BOW_Z - SIDE_X * Math.sin(angle)))
  }
  path.push(new Vector3(SIDE_X, TUBE_Y, 0.3), new Vector3(SIDE_X, TUBE_Y, STERN_Z))
  return path
}

/** A tube of `radius` swept along `path`, open at its ends. */
export function tubeGeometry(path: readonly Vector3[], radius: number, sides = SIDES): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const indices: number[] = []
  const up = new Vector3(0, 1, 0)
  path.forEach((point, i) => {
    const ahead = path[Math.min(i + 1, path.length - 1)]!
    const behind = path[Math.max(i - 1, 0)]!
    const along = new Vector3().subVectors(ahead, behind).normalize()
    const across = new Vector3().crossVectors(along, up).normalize()
    const over = new Vector3().crossVectors(across, along).normalize()
    for (let s = 0; s < sides; s++) {
      const angle = (2 * Math.PI * s) / sides
      const n = new Vector3().addScaledVector(across, Math.cos(angle)).addScaledVector(over, Math.sin(angle))
      positions.push(point.x + n.x * radius, point.y + n.y * radius, point.z + n.z * radius)
      normals.push(n.x, n.y, n.z)
    }
  })
  for (let i = 0; i + 1 < path.length; i++) {
    for (let s = 0; s < sides; s++) {
      const a = i * sides + s
      const b = i * sides + ((s + 1) % sides)
      indices.push(a, a + sides, b, b, a + sides, b + sides)
    }
  }
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), indices: new Uint16Array(indices) })
}

const PATH = tubePath()
const TUBES = tubeGeometry(PATH, TUBE)
/** The rubbing strake round the tubes' outside, low down, where they meet the water. */
const STRAKE = tubeGeometry(
  PATH.map((p, i) => {
    const out = i === 0 || i === PATH.length - 1 ? new Vector3(Math.sign(p.x), 0, 0) : new Vector3(p.x, 0, Math.min(0, p.z - BOW_Z)).normalize()
    return new Vector3(p.x + out.x * TUBE * 0.85, p.y - TUBE * 0.45, p.z + out.z * TUBE * 0.85)
  }),
  0.035,
  6,
)
/** The grab line along each side, a hand's breadth over the tube. */
const LINE = cylinderGeometry(0.012, 0.012, 1, 4)
/** A tube's cone end, past the transom. */
const CONE = cylinderGeometry(TUBE, TUBE * 0.55, 0.2, SIDES)
CONE.rotateX(Math.PI / 2)
const CAP = sphereGeometry(TUBE * 0.55, 8, 6)
/** The floor between the tubes, the transom board across the stern, the thwart across the bow. */
const FLOOR = boxGeometry(2 * SIDE_X, 0.08, STERN_Z - BOW_Z + 0.2)
const TRANSOM = boxGeometry(2 * SIDE_X - 0.1, 0.42, 0.05)
const THWART = cylinderGeometry(0.13, 0.13, 2 * SIDE_X, 10)
THWART.rotateZ(Math.PI / 2)
const FUEL = boxGeometry(0.3, 0.2, 0.42)
const PADDLE_SHAFT = cylinderGeometry(0.018, 0.018, 1.3, 5)
PADDLE_SHAFT.rotateX(Math.PI / 2)
const PADDLE_BLADE = boxGeometry(0.18, 0.02, 0.42)
/** The outboard, about its pivot on the transom bracket: its cowling, the leg down into the water, the
 * anti-cavitation plate, the gearcase, the propeller's blades and the tiller. */
const COWL = boxGeometry(0.34, 0.42, 0.52)
const COWL_TOP = sphereGeometry(0.24, 10, 6)
COWL_TOP.scale(0.72, 0.4, 1.1)
const SHAFT = boxGeometry(0.11, 0.62, 0.17)
const PLATE = boxGeometry(0.3, 0.02, 0.34)
const GEARCASE = cylinderGeometry(0.06, 0.05, 0.32, 8)
GEARCASE.rotateX(Math.PI / 2)
const BLADE = boxGeometry(0.06, 0.13, 0.015)
const TILLER = cylinderGeometry(0.022, 0.028, RAFT_TILLER, 6)
const GRIP = cylinderGeometry(0.035, 0.035, 0.14, 6)

/** Where the propeller is on the outboard, below its pivot and behind it, metres. */
const PROP_AT = new Vector3(0, -0.62, 0.28)
/** Where the tiller reaches to, from the cowling's front: forward and over to the driver on the left. */
const TILLER_FROM = new Vector3(0, 0.28, -0.22)
const TILLER_TO = new Vector3(-0.3, 0.42, -0.22 - RAFT_TILLER * 0.92)
/** How quickly the drawn tiller follows the helm, per second; how fast the propeller spins, turns a second at a standstill and for each m/s. */
const HELM_EASE = 6
const PROP_IDLE = 6
const PROP_PER_SPEED = 3

const ease = (from: number, to: number, rate: number) => from + (to - from) * Math.min(1, rate)

/**
 * The raft: an inflatable river-rafting boat for two, its tubes in a rafting colour running round from
 * stern to a kicked-up bow, a rubbing strake low on them and a grab line along their tops, a floor and a
 * thwart across the bow, a fuel tank and two paddles stowed inside, and a big outboard on its transom.
 *
 * The outboard swings on its bracket as its driver puts the tiller over: neither is on the wire, so the
 * helm is read from how fast the raft is turning. Its propeller spins while someone has the tiller.
 */
export class RaftModel implements VehicleModel {
  readonly root = new Group()
  private readonly rubber: Material
  private readonly dark = toonMaterial({ color: RUBBER_DARK })
  private readonly cowling = toonMaterial({ color: COWLING })
  private readonly leg = toonMaterial({ color: LEG })
  private readonly tank = toonMaterial({ color: TANK })
  private readonly paddle = toonMaterial({ color: PADDLE })
  private readonly motor = new Group()
  private readonly prop = new Group()
  private readonly turn = new Quaternion()
  private helm = 0
  private spin = 0
  private lastYaw: number | null = null
  private lastAt = 0
  private lastPosition: [number, number, number] | null = null

  constructor(id: number) {
    this.rubber = toonMaterial({ color: PAINTS[Math.abs(id) % PAINTS.length]! })
    const part = (geometry: Geometry, material: Material, x: number, y: number, z: number, parent: Group = this.root) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }
    part(TUBES, this.rubber, 0, 0, 0)
    part(STRAKE, this.dark, 0, 0, 0)
    for (const side of [-1, 1]) {
      part(CONE, this.rubber, side * SIDE_X, TUBE_Y, STERN_Z + 0.1)
      part(CAP, this.rubber, side * SIDE_X, TUBE_Y, STERN_Z + 0.2)
      // The grab line, from the stern to the bow's shoulder over the tube's outer side.
      const line = part(LINE, this.dark, side * (SIDE_X + TUBE * 0.6), TUBE_Y + TUBE * 0.75, (STERN_Z - 0.2 + BOW_Z) / 2)
      line.scale.set(1, STERN_Z - 0.2 - BOW_Z, 1)
      line.rotation.x = Math.PI / 2
      line.castShadow = false
    }
    part(FLOOR, this.dark, 0, -HY + 0.06, (STERN_Z + BOW_Z) / 2)
    part(TRANSOM, this.dark, 0, -0.02, HZ - 0.2)
    part(THWART, this.rubber, 0, TUBE_Y - 0.05, -0.75)
    part(FUEL, this.tank, 0.3, -HY + 0.2, 1.3)
    // Two paddles stowed along the right tube, inside.
    for (const [y, z] of [[-0.05, 0.2], [0.0, 0.45]] as const) {
      part(PADDLE_SHAFT, this.paddle, SIDE_X - TUBE - 0.08, y, z)
      part(PADDLE_BLADE, this.paddle, SIDE_X - TUBE - 0.08, y, z - 0.75)
    }

    // The outboard, turning on its bracket on the transom.
    this.motor.position.set(...RAFT_MOTOR_PIVOT)
    this.root.add(this.motor)
    part(COWL, this.cowling, 0, 0.38, 0.12, this.motor)
    part(COWL_TOP, this.cowling, 0, 0.6, 0.12, this.motor)
    part(SHAFT, this.leg, 0, -0.2, 0.12, this.motor)
    part(PLATE, this.leg, 0, -0.5, 0.16, this.motor)
    part(GEARCASE, this.leg, 0, PROP_AT.y, 0.14, this.motor)
    this.prop.position.copy(PROP_AT)
    this.motor.add(this.prop)
    for (let k = 0; k < 3; k++) {
      const blade = part(BLADE, this.leg, 0, 0, 0, this.prop)
      blade.rotation.z = (2 * Math.PI * k) / 3
      blade.position.set(0.075 * Math.sin((2 * Math.PI * k) / 3), -0.075 * Math.cos((2 * Math.PI * k) / 3), 0)
      blade.rotation.y = 0.5
    }
    const along = new Vector3().subVectors(TILLER_TO, TILLER_FROM)
    const tiller = part(TILLER, this.cowling, (TILLER_FROM.x + TILLER_TO.x) / 2, (TILLER_FROM.y + TILLER_TO.y) / 2, (TILLER_FROM.z + TILLER_TO.z) / 2, this.motor)
    tiller.quaternion.setFromUnitVectors(new Vector3(0, 1, 0), along.clone().normalize())
    const grip = part(GRIP, this.dark, TILLER_TO.x, TILLER_TO.y, TILLER_TO.z, this.motor)
    grip.quaternion.copy(tiller.quaternion)
  }

  update(position: readonly [number, number, number], rotation: readonly [number, number, number, number], _steer: number, seats: readonly number[]) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const now = performance.now()
    const dt = this.lastAt ? Math.min(0.1, (now - this.lastAt) / 1000) : 0
    this.lastAt = now
    // Its heading, and how fast that turns: the helm the driver holds.
    this.turn.set(...rotation)
    const nose = new Vector3(0, 0, -1).applyQuaternion(this.turn)
    const yaw = Math.atan2(-nose.x, -nose.z)
    let rate = 0
    if (this.lastYaw !== null && dt > 0) {
      let change = yaw - this.lastYaw
      if (change > Math.PI) change -= 2 * Math.PI
      if (change < -Math.PI) change += 2 * Math.PI
      rate = change / dt
    }
    this.lastYaw = yaw
    const manned = (seats[0] ?? 0) !== 0
    // Turning right (the yaw falling) the outboard swings its propeller out to the right, which throws
    // the stern left.
    const wanted = manned ? Math.max(-1, Math.min(1, -rate / RAFT_TURN_RATE)) * RAFT_MOTOR_SWING : 0
    this.helm = ease(this.helm, wanted, dt * HELM_EASE)
    this.motor.rotation.y = this.helm
    // Its propeller spins while someone has the tiller, faster the faster it goes.
    let speed = 0
    if (this.lastPosition && dt > 0) speed = Math.hypot(position[0] - this.lastPosition[0], position[2] - this.lastPosition[2]) / dt
    this.lastPosition = [position[0], position[1], position[2]]
    if (manned) this.spin += (PROP_IDLE + PROP_PER_SPEED * Math.min(speed, 20)) * 2 * Math.PI * dt
    this.prop.rotation.z = this.spin % (2 * Math.PI)
  }

  dispose() {
    for (const material of [this.rubber, this.dark, this.cowling, this.leg, this.tank, this.paddle]) material.dispose()
    this.root.removeFromParent()
  }
}
