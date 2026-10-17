import { Geometry, Group, Material, Mesh, cylinderGeometry, ringGeometry, type MaterialParams } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { type Paint, type Ring, ellipsoid, extrude, lathe, limb, surface, torus } from './plague-shapes'
import { PLAGUE_STOMP_RADIUS, PLAGUE_STOMP_WINDUP_TICKS, PLAGUE_STRIKE_INTERVAL_TICKS, PLAGUE_STRIKE_RADIUS, PLAGUE_STRIKE_ROCKETS, PLAGUE_STRIKE_WINDUP_TICKS, TICK_RATE } from '@/sim/consts'
import { FACE_EAR_LEFT, FACE_EAR_RIGHT, FACE_EYE_LEFT, FACE_EYE_RIGHT, FACE_MOUTH, PLAGUE_STOMP, PLAGUE_STRIKE, PLAGUE_WALK } from '@/net/codec'
import { EAR, EYE, FOREARM, HAND_REACH, HEAD, HEAD_RADIUS, HIP_X, HIP_Y, MOUTH, SHIN, SHOULDER_X, SHOULDER_Y, THIGH, UPPER_ARM, type Pose, feet, pose, toWorld } from '@/sim/plague-body'

/** How fast the drawn joints follow the pose the snapshot gives, per second: quick, but smooth over snapshots and mode changes. */
const FOLLOW = 12
/** Blinks come this many seconds apart at most, and take this long. */
const BLINK_EVERY = 5
const BLINK_SECONDS = 0.16
/** How far his pupils move to look at you, metres. */
const GAZE = 0.22

const SUIT = 0x1d2340
const LAPEL = 0x2b3462
const SHIRT = 0xf2f2f2
const TIE = 0xd0161b
const SKIN = 0xf08a2c
const PALE = 0xf6d2a2
const FLUSH = 0xd9662a
const LIP = 0xc4582c
const MOUTH_INSIDE = 0x3a0d0d
const TEETH = 0xf5f0e0
const HAIR = 0xf4d35e
const HAIR_LIGHT = 0xfbe794
const BROW = 0xe6cf72
const EYE_WHITE = 0xf8f8f4
const IRIS = 0x4a7fb5
const PUPIL = 0x111111
const SHOE = 0x161616
const SOLE = 0x2a2a2a
const GOLD = 0xffc83a
const TAPE = 0x9b9b9b
const BANDAGE = 0xf2efe6
const STUMP = 0x8a1a1a

const ease = (from: number, to: number, rate: number, dt: number) => from + (to - from) * Math.min(1, rate * dt)
const near = (a: number, b: number, width: number) => Math.max(0, 1 - Math.abs(a - b) / width)

/** The jacket's outline, bottom to top: open at the front over his shirt, and a belly well ahead of him. */
const TORSO: readonly Ring[] = [
  { y: 15.6, rx: 0 },
  { y: 15.8, rx: 6.2, rz: 5.2 },
  { y: 18, rx: 7.0, rz: 6.2, ahead: 0.6 },
  { y: 21, rx: 7.6, rz: 7.0, ahead: 1.6 },
  { y: 24, rx: 7.4, rz: 6.6, ahead: 1.4 },
  { y: 27, rx: 7.2, rz: 5.6, ahead: 0.5 },
  { y: 29.2, rx: 7.6, rz: 4.6 },
  { y: 30.6, rx: 5.0, rz: 3.6 },
  { y: 31.4, rx: 2.6, rz: 2.4 },
  { y: 31.6, rx: 0 },
]

/** How far forward of his middle the front of the torso is at height `y`, metres. */
function frontAt(y: number) {
  for (let i = 1; i < TORSO.length; i++) {
    const [a, b] = [TORSO[i - 1]!, TORSO[i]!]
    if (y <= b.y) {
      const t = (y - a.y) / Math.max(1e-6, b.y - a.y)
      const front = (r: Ring) => (r.rz ?? r.rx) + (r.ahead ?? 0)
      return front(a) + (front(b) - front(a)) * t
    }
  }
  return 0
}

/** Where the shirt shows: a V that widens from his belly to his collar, edged by the lapels. */
const paintTorso: Paint = (u, _v, [, y]) => {
  const off = Math.abs(u - 0.5)
  if (y < 22 || y > 31.2) return SUIT
  const open = 0.015 + ((y - 22) / 9.2) * 0.075
  if (off < open) return SHIRT
  if (off < open + 0.03) return LAPEL
  return SUIT
}

/** His face's skin: orange, but pale where his goggles sat on the sunbed. */
const paintHead: Paint = (_u, _v, [x, y, z]) => {
  if (z > -1.2) return SKIN
  for (const side of [-1, 1]) {
    const [ex, ey] = [side * EYE[0], EYE[1]]
    if (((x - ex) / 1.35) ** 2 + ((y - ey) / 0.9) ** 2 < 1) return PALE
  }
  return SKIN
}

const paintHair: Paint = (u) => (Math.sin(u * 70) > 0.55 ? HAIR_LIGHT : HAIR)

/** The parts that show what has been shot out of his face, and what they replace. */
interface Feature {
  whole: Group
  lost: Group
}

/**
 * The Orange Plague at his real size, 40 m to the top of his hair: a dark suit open over a white shirt
 * and a red tie that hangs past his belt, an orange face with pale rings round his eyes, pursed lips,
 * a swept yellow quiff and a gold phone in his right hand. He faces -Z at yaw 0, as players do.
 *
 * He is built on the joints of `sim/plague-body.ts`, the same ones the server's hit capsules hang
 * from, and posed from what the snapshot says (`pose`), so where he is drawn is where he is hit. Each
 * eye, ear and his mouth is its own piece, swapped for a bandage, a stump or tape once it is shot out.
 */
export class PlagueModel {
  readonly root = new Group()
  /** Leans and topples, pivoting at the feet. */
  private readonly body = new Group()
  private readonly hips = [new Group(), new Group()] as const
  private readonly knees = [new Group(), new Group()] as const
  private readonly shoulders = [new Group(), new Group()] as const
  private readonly elbows = [new Group(), new Group()] as const
  private readonly head = new Group()
  private readonly mouth = new Group()
  private readonly gaze = [new Group(), new Group()] as const
  private readonly lids = [new Group(), new Group()] as const
  /** Each `FACE_*` bit's piece, by bit number. */
  private readonly features: Feature[] = []
  private readonly materials = new Map<number | string, Material>()
  private drawn: Pose | null = null
  private lastProgress = -1
  private blinkIn = 2
  private blinking = 0
  private face = 0

  constructor() {
    this.root.add(this.body)
    this.buildLegs()
    this.buildTorso()
    this.buildArms()
    this.buildHead()
  }

  private material(color: number, extra: Partial<MaterialParams> = {}, key: number | string = color) {
    let material = this.materials.get(key)
    if (!material) {
      material = toonMaterial({ color, ...extra })
      this.materials.set(key, material)
    }
    return material
  }

  private painted() {
    return this.material(0xffffff, { vertexColors: true }, 'painted')
  }

  /** A mesh of `geometry` in `color` (or painted, with null), casting a shadow, at `x, y, z` in `parent`. */
  private part(parent: Group, geometry: Geometry, color: number | null, x = 0, y = 0, z = 0, shadow = true) {
    const mesh = new Mesh(geometry, color === null ? this.painted() : this.material(color))
    mesh.position.set(x, y, z)
    mesh.castShadow = shadow
    parent.add(mesh)
    return mesh
  }

  private buildLegs() {
    const thigh = limb(THIGH, 2.75, 2.4, 28)
    const shin = limb(SHIN - 1.0, 2.35, 2.05, 28)
    const shoe = ellipsoid(1.55, 1.1, 3.1, 28, undefined, 0, 1, 0.95)
    const sole = ellipsoid(1.6, 0.25, 3.15, 24, undefined, 0.5, 1)
    this.hips.forEach((hip, i) => {
      const side = i === 0 ? -1 : 1
      hip.position.set(side * HIP_X, HIP_Y, 0)
      this.part(hip, thigh, SUIT)
      const knee = this.knees[i]!
      knee.position.set(0, -THIGH, 0)
      this.part(knee, shin, SUIT)
      // His shoes: long, polished and flat underneath; the sole a shade lighter.
      this.part(knee, shoe, SHOE, 0, -SHIN - 0.3, -1.0)
      this.part(knee, sole, SOLE, 0, -SHIN - 0.95, -1.0, false)
      hip.add(knee)
      this.body.add(hip)
    })
  }

  private buildTorso() {
    this.part(this.body, lathe(TORSO, 72, 6, paintTorso), null)
    // The tie follows his front down over the belly, well past where a belt would be.
    const tie = surface(6, 48, (u, v) => {
      const y = 30.7 - v * 13.4
      const half = v > 0.94 ? (1 - v) / 0.06 * 1.0 : 0.45 + v * 0.6
      return [(u - 0.5) * 2 * half, y, -frontAt(y) - 0.18]
    })
    this.part(this.body, tie, TIE).material = this.material(TIE, { side: 'double' }, 'tie')
    this.part(this.body, ellipsoid(0.75, 0.65, 0.45, 20), TIE, 0, 30.9, -frontAt(30.9) - 0.2)
    // A gold pin on his lapel.
    this.part(this.body, ellipsoid(0.32, 0.32, 0.12, 12), GOLD, -3.9, 28.4, -frontAt(28.4) - 0.05, false)
    // Neck and a double chin.
    this.part(this.body, limb(3.2, 2.6, 2.6, 28), SKIN, 0, 33.6, 0)
    this.part(this.body, ellipsoid(2.6, 1.25, 1.9, 28), SKIN, 0, 32.7, -1.9)
  }

  private buildArms() {
    const upper = limb(UPPER_ARM, 2.15, 1.95, 24)
    const fore = limb(FOREARM - 0.7, 1.95, 1.75, 24)
    const cuff = limb(0.9, 1.7, 1.7, 20)
    const palm = ellipsoid(1.3, 1.55, 0.8, 20)
    const finger = limb(1.7, 0.34, 0.3, 10)
    const thumb = limb(1.3, 0.38, 0.32, 10)
    this.shoulders.forEach((shoulder, i) => {
      const side = i === 0 ? -1 : 1
      shoulder.position.set(side * SHOULDER_X, SHOULDER_Y, 0)
      this.part(shoulder, upper, SUIT)
      const elbow = this.elbows[i]!
      elbow.position.set(0, -UPPER_ARM, 0)
      this.part(elbow, fore, SUIT)
      this.part(elbow, cuff, SHIRT, 0, -FOREARM + 0.75, 0)
      this.part(elbow, ellipsoid(0.3, 0.3, 0.3, 10), GOLD, side * 1.55, -FOREARM + 0.4, 0, false)
      const hand = new Group()
      hand.position.set(0, -HAND_REACH + 0.4, 0)
      this.part(hand, palm, SKIN)
      // Short fingers, curled in a little; the thumb on the inside.
      for (let f = 0; f < 4; f++) {
        const mesh = this.part(hand, finger, SKIN, (f - 1.5) * 0.62, -1.2, -0.2, false)
        mesh.rotation.set(0.35 + f * 0.05, 0, 0)
      }
      const t = this.part(hand, thumb, SKIN, -side * 1.0, -0.2, -0.55, false)
      t.rotation.set(0.6, 0, -side * 0.6)
      if (side > 0) {
        const phone = this.part(hand, extrude([[-0.45, -1.4], [0.45, -1.4], [0.45, 1.4], [-0.45, 1.4]], 0.25), GOLD, -0.9, -0.6, -0.75)
        phone.rotation.set(0, 0.4, 0)
      }
      elbow.add(hand)
      shoulder.add(elbow)
      this.body.add(shoulder)
    })
  }

  private buildHead() {
    this.head.position.set(...HEAD)
    this.body.add(this.head)
    // The head: wider at the jowls than at the brow, a little flat in the face.
    const skull = surface(64, 48, (u, v) => {
      const phi = v * Math.PI
      const theta = u * Math.PI * 2
      const jowls = 1 + 0.16 * near(v, 0.7, 0.25)
      const front = Math.cos(theta) < 0 ? 0.97 : 1.03
      return [Math.sin(phi) * Math.sin(theta) * HEAD_RADIUS * jowls, Math.cos(phi) * HEAD_RADIUS * 1.04, Math.sin(phi) * Math.cos(theta) * HEAD_RADIUS * front * (1 + 0.06 * near(v, 0.75, 0.2))]
    }, paintHead, true)
    this.part(this.head, skull, null)
    // The hair: a cap swept back over the crown and a quiff swept forward over the brow.
    this.part(this.head, ellipsoid(4.0, 2.5, 4.35, 48, paintHair, 0, 0.55), null, 0, 1.6, 0.25)
    this.part(this.head, ellipsoid(4.15, 2.1, 3.9, 48, paintHair, 0.15, 0.62), null, 0, 0.9, 0.95)
    const quiff = this.part(this.head, ellipsoid(3.5, 1.35, 1.9, 40, paintHair), null, 0.45, 2.95, -2.75)
    quiff.rotation.set(-0.3, 0, 0.12)
    // Brows, nose.
    for (const side of [-1, 1]) {
      const brow = this.part(this.head, ellipsoid(0.9, 0.24, 0.32, 16), BROW, side * 1.35, 1.75, -3.25, false)
      brow.rotation.set(0, 0, side * 0.18)
    }
    this.part(this.head, ellipsoid(0.7, 1.05, 0.85, 24), SKIN, 0, -0.2, -3.6)
    this.part(this.head, ellipsoid(0.62, 0.55, 0.55, 20), SKIN, 0, -0.75, -4.15)
    for (const side of [-1, 1]) this.buildEye(side)
    for (const side of [-1, 1]) this.buildEar(side)
    this.buildMouth()
  }

  private buildEye(side: number) {
    const i = side < 0 ? 0 : 1
    const whole = new Group()
    whole.position.set(side * EYE[0], EYE[1], EYE[2])
    this.part(whole, ellipsoid(0.75, 0.55, 0.45, 24), EYE_WHITE, 0, 0, 0, false)
    const gaze = this.gaze[i]!
    this.part(gaze, ellipsoid(0.32, 0.32, 0.12, 16), IRIS, 0, 0, -0.38, false)
    this.part(gaze, ellipsoid(0.15, 0.15, 0.08, 12), PUPIL, 0, 0, -0.46, false)
    whole.add(gaze)
    // A heavy upper lid, which blinks, and a bag under the eye.
    const lid = this.lids[i]!
    this.part(lid, ellipsoid(0.84, 0.62, 0.54, 24, undefined, 0, 0.42), SKIN, 0, 0, 0, false)
    lid.rotation.set(0.35, 0, 0)
    whole.add(lid)
    this.part(whole, ellipsoid(0.82, 0.3, 0.42, 20), PALE, 0, -0.5, 0.05, false)
    // Shot out: swollen shut under a cross of plaster.
    const lost = new Group()
    lost.position.copy(whole.position)
    this.part(lost, ellipsoid(0.86, 0.66, 0.56, 24), FLUSH, 0, 0, 0, false)
    for (const tilt of [-0.7, 0.7]) {
      const strip = this.part(lost, extrude([[-1.0, -0.17], [1.0, -0.17], [1.0, 0.17], [-1.0, 0.17]], 0.08), BANDAGE, 0, 0, -0.6, false)
      strip.rotation.set(0, 0, tilt)
    }
    this.addFeature(side < 0 ? FACE_EYE_LEFT : FACE_EYE_RIGHT, whole, lost)
  }

  private buildEar(side: number) {
    const whole = new Group()
    whole.position.set(side * EAR[0], EAR[1], EAR[2])
    whole.rotation.set(0, side * 0.25, 0)
    this.part(whole, ellipsoid(0.38, 1.05, 0.75, 20), SKIN)
    this.part(whole, ellipsoid(0.2, 0.72, 0.48, 16), FLUSH, side * 0.2, 0, 0, false)
    // Shot off: a red stump under a strip of bandage.
    const lost = new Group()
    lost.position.copy(whole.position)
    this.part(lost, ellipsoid(0.26, 0.62, 0.48, 16), STUMP, side * 0.05, 0, 0, false)
    const strip = this.part(lost, extrude([[-0.15, -0.9], [0.15, -0.9], [0.15, 0.9], [-0.15, 0.9]], 1.2), BANDAGE, side * 0.12, 0, 0, false)
    strip.rotation.set(0.2, 0, 0)
    this.addFeature(side < 0 ? FACE_EAR_LEFT : FACE_EAR_RIGHT, whole, lost)
  }

  private buildMouth() {
    const whole = this.mouth
    whole.position.set(...MOUTH)
    // Pursed lips round a dark mouth, teeth showing when it opens.
    const lips = this.part(whole, torus(0.62, 0.3, 32, 14), LIP, 0, 0, -0.22, false)
    lips.rotation.set(Math.PI / 2, 0, 0)
    lips.scale.set(1.45, 1, 1)
    this.part(whole, ellipsoid(0.85, 0.42, 0.18, 20), MOUTH_INSIDE, 0, 0, -0.12, false)
    this.part(whole, extrude([[-0.55, -0.1], [0.55, -0.1], [0.55, 0.1], [-0.55, 0.1]], 0.1), TEETH, 0, 0.22, -0.2, false)
    // Shot shut: taped over, twice.
    const lost = new Group()
    lost.position.copy(whole.position)
    for (const [tilt, z] of [[0.08, -0.62], [-0.35, -0.68]] as const) {
      const tape = this.part(lost, extrude([[-1.5, -0.45], [1.5, -0.45], [1.5, 0.45], [-1.5, 0.45]], 0.1), TAPE, 0, 0, z, false)
      tape.rotation.set(0, 0, tilt)
    }
    this.addFeature(FACE_MOUTH, whole, lost)
  }

  private addFeature(bit: number, whole: Group, lost: Group) {
    lost.visible = false
    this.head.add(whole, lost)
    this.features[Math.log2(bit)] = { whole, lost }
  }

  /**
   * Places and poses him for this frame. `mode` is `PLAGUE_*`, `progress` how far through it he is
   * (as the snapshot says), `face` the `FACE_*` bits shot out, `mouthOpen` (0..1) how wide he is
   * talking, and `look` the point he keeps an eye on, if any. Returns which foot came down this frame,
   * if one did, as a world `[x, z]`, for the dust and the thud.
   */
  update(position: readonly [number, number, number], yaw: number, mode: number, progress: number, toppling: boolean, face: number, mouthOpen: number, look: readonly number[] | null, dt: number): [number, number] | null {
    this.root.position.set(...position)
    this.root.rotation.set(0, yaw, 0)
    const target = pose(mode, progress, toppling)
    const drawn = (this.drawn ??= structuredClone(target))
    const follow = (from: number, to: number) => ease(from, to, FOLLOW, dt)
    drawn.tilt = follow(drawn.tilt, target.tilt)
    drawn.bob = follow(drawn.bob, target.bob)
    for (const i of [0, 1]) {
      drawn.hip[i] = follow(drawn.hip[i]!, target.hip[i]!)
      drawn.knee[i] = follow(drawn.knee[i]!, target.knee[i]!)
      for (const k of [0, 1]) {
        drawn.shoulder[i]![k] = follow(drawn.shoulder[i]![k]!, target.shoulder[i]![k]!)
        drawn.elbow[i]![k] = follow(drawn.elbow[i]![k]!, target.elbow[i]![k]!)
      }
    }
    this.body.position.set(0, drawn.bob, 0)
    this.body.rotation.set(drawn.tilt, 0, 0)
    for (const i of [0, 1] as const) {
      this.hips[i].rotation.set(drawn.hip[i], 0, 0)
      this.knees[i].rotation.set(drawn.knee[i], 0, 0)
      this.shoulders[i].rotation.set(drawn.shoulder[i][0], 0, drawn.shoulder[i][1])
      this.elbows[i].rotation.set(drawn.elbow[i][0], 0, drawn.elbow[i][1])
    }
    this.setFace(face)
    this.mouth.scale.set(1, 0.7 + 1.3 * Math.min(1, Math.max(0, mouthOpen)), 1)
    this.animateEyes(position, yaw, look, toppling, dt)

    // A foot comes down where the walk cycle passes 0 (his left) or one half (his right).
    let landed: [number, number] | null = null
    if (mode === PLAGUE_WALK && !toppling && this.lastProgress >= 0) {
      const [before, now] = [this.lastProgress, progress]
      const left = before > 0.75 && now < 0.25
      const right = before < 0.5 && now >= 0.5 && now - before < 0.25
      if (left || right) {
        const foot = feet(target)[left ? 0 : 1]
        const [x, , z] = toWorld(position, yaw, foot)
        landed = [x, z]
      }
    }
    this.lastProgress = mode === PLAGUE_WALK && !toppling ? progress : -1
    return landed
  }

  private setFace(face: number) {
    if (face === this.face) return
    this.face = face
    this.features.forEach((feature, bit) => {
      const lost = (face & (1 << bit)) !== 0
      feature.whole.visible = !lost
      feature.lost.visible = lost
    })
  }

  /** Blinks now and then, and looks at `look`: his pupils turn towards it while it is in front of him. */
  private animateEyes(position: readonly number[], yaw: number, look: readonly number[] | null, toppling: boolean, dt: number) {
    this.blinkIn -= dt
    if (this.blinkIn <= 0) {
      this.blinking = BLINK_SECONDS
      this.blinkIn = 1 + Math.random() * BLINK_EVERY
    }
    this.blinking = Math.max(0, this.blinking - dt)
    const shut = toppling ? 1 : this.blinking > 0 ? Math.sin((1 - this.blinking / BLINK_SECONDS) * Math.PI) : 0
    for (const lid of this.lids) lid.rotation.set(0.35 + 1.15 * shut, 0, 0)
    let [gx, gy] = [0, 0]
    if (look) {
      const head = toWorld(position, yaw, [0, HEAD[1], 0])
      const [dx, dy, dz] = [look[0]! - head[0], look[1]! - head[1], look[2]! - head[2]]
      // Into his own frame: undo his yaw.
      const [sin, cos] = [Math.sin(yaw), Math.cos(yaw)]
      const [lx, lz] = [cos * dx - sin * dz, sin * dx + cos * dz]
      const length = Math.hypot(lx, dy, lz) || 1
      if (lz < 0) [gx, gy] = [(lx / length) * GAZE, (dy / length) * GAZE * 0.7]
    }
    for (const gaze of this.gaze) gaze.position.set(ease(gaze.position.x, gx, 6, dt), ease(gaze.position.y, gy, 6, dt), 0)
  }

  dispose() {
    this.root.removeFromParent()
    this.root.traverse((node) => {
      if (node instanceof Mesh) node.geometry.dispose()
    })
    for (const material of this.materials.values()) material.dispose()
  }
}

/** Drawn over everything, so a wall in front never hides where the attack lands. */
const ON_TOP = 999
/** How high the airstrike's beam stands over the circle, metres. */
const BEAM_HEIGHT = 120
/** The marks pulse this often, per second. */
const PULSE_RATE = 3
/** After the wind-up the airstrike's mark stays up while the rockets fall, seconds. */
export const STRIKE_LINGER = (PLAGUE_STRIKE_ROCKETS * PLAGUE_STRIKE_INTERVAL_TICKS) / TICK_RATE + 2.6

/**
 * Where his attacks will land, so everyone can get out of the way: a shadow that darkens and a ring
 * that closes in under a raised foot, and a pulsing red circle with a beam of light over an airstrike.
 */
export class PlagueMarks {
  readonly root = new Group()
  private readonly stompMaterial = new Material({ color: 0x000000, lit: false, blend: 'alpha', opacity: 0.4, depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly stompRingMaterial = new Material({ color: 0xff3a1a, lit: false, blend: 'alpha', opacity: 0.9, depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly shadow = new Mesh(ringGeometry(0, PLAGUE_STOMP_RADIUS, 48), this.stompMaterial)
  private readonly closing = new Mesh(ringGeometry(PLAGUE_STOMP_RADIUS * 0.94, PLAGUE_STOMP_RADIUS, 48), this.stompRingMaterial)
  private readonly strikeMaterial = new Material({ color: 0xff2a1a, lit: false, blend: 'alpha', opacity: 0.9, depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly strikeRing = new Mesh(ringGeometry(PLAGUE_STRIKE_RADIUS * 0.92, PLAGUE_STRIKE_RADIUS, 64), this.strikeMaterial)
  private readonly beamMaterial = new Material({ color: 0xff2a1a, lit: false, blend: 'alpha', opacity: 0.25, depthTest: false, depthWrite: false, fog: false, side: 'double' })
  private readonly beam = new Mesh(cylinderGeometry(1.2, 1.2, BEAM_HEIGHT, 10), this.beamMaterial)
  private readonly stomp = new Group()
  private readonly strike = new Group()
  private time = 0

  constructor() {
    for (const ring of [this.shadow, this.closing, this.strikeRing]) {
      ring.rotation.x = -Math.PI / 2
      ring.renderOrder = ON_TOP
      ring.frustumCulled = false
    }
    this.beam.position.y = BEAM_HEIGHT / 2
    this.beam.renderOrder = ON_TOP
    this.beam.frustumCulled = false
    this.stomp.add(this.shadow, this.closing)
    this.strike.add(this.strikeRing, this.beam)
    this.root.add(this.stomp, this.strike)
    this.stomp.visible = false
    this.strike.visible = false
  }

  /**
   * Shows the mark for `mode` (`PLAGUE_*`) at `at`, `windup` (0..1) of the way to landing; `lingering`
   * keeps an airstrike's mark up while its rockets fall. Hides both with `at` null.
   */
  set(at: readonly [number, number, number] | null, mode: number, windup: number, lingering: boolean, dt: number) {
    this.time += dt
    const pulse = 0.5 + 0.5 * Math.sin(this.time * PULSE_RATE * Math.PI * 2)
    const stomping = at !== null && mode === PLAGUE_STOMP
    const striking = at !== null && (mode === PLAGUE_STRIKE || lingering)
    this.stomp.visible = stomping
    this.strike.visible = striking
    if (stomping) {
      this.stomp.position.set(at[0], at[1] + 0.2, at[2])
      this.stompMaterial.opacity = 0.15 + 0.45 * windup
      // The ring closes in from twice the blast to its edge as the foot comes down.
      const scale = 2 - windup
      this.closing.scale.set(scale, scale, 1)
    }
    if (striking) {
      this.strike.position.set(at[0], at[1] + 0.2, at[2])
      this.strikeMaterial.opacity = 0.55 + 0.45 * pulse
      this.beamMaterial.opacity = 0.12 + 0.18 * pulse
    }
  }

  dispose() {
    this.root.removeFromParent()
    this.root.traverse((node) => {
      if (node instanceof Mesh) node.material.dispose()
    })
  }
}

/** How long a wind-up of `mode` lasts, seconds; 0 for anything that is not one. */
export function windupSeconds(mode: number) {
  if (mode === PLAGUE_STOMP) return PLAGUE_STOMP_WINDUP_TICKS / TICK_RATE
  if (mode === PLAGUE_STRIKE) return PLAGUE_STRIKE_WINDUP_TICKS / TICK_RATE
  return 0
}
