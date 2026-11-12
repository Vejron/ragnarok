import { Geometry, Group, Material as Plain, Mesh, Quat, Vec3, boxGeometry, cylinderGeometry, quadGeometry, type Material, type XYZ } from '@ragnarok/engine'
import type { VehicleModel } from './car'
import { toonMaterial } from './toon'
import type { Sub } from '@/entities/units'
import { SUB_MAST_UP } from '@/net/codec'
import { SUB_BOW, SUB_HALF_EXTENTS, SUB_MAST_HEIGHT, SUB_SAIL_HALF, SUB_SAIL_Z, SUB_TUBE_AT } from '@/sim/consts'

const [, HY, HZ] = SUB_HALF_EXTENTS
/** The sail's top, in the hull frame. */
const SAIL_TOP = HY + 2 * SUB_SAIL_HALF[1]
/** How far the periscope stands out of the sail when it is raised, metres, and how quickly it rises or sinks, per second. */
const MAST_OUT = SUB_MAST_HEIGHT - SAIL_TOP
const MAST_RATE = 2.5
/** The screw spins this fast with a captain at the helm, rad/s. */
const SCREW_SPIN = 9
/** How far each blade of the screw is twisted out of its disc, radians. */
const BLADE_PITCH = 0.6
/** A tube's round lid, its radius, metres, and how far it swings up from shut when the tube is empty, radians. */
const LID = 0.32
const LID_OPEN = 1.7
/** How quickly a lid swings, per second. */
const LID_RATE = 4
/** How long a tube goes on smoking after its missile has left, seconds. */
export const TUBE_SMOKES = 3.5

/**
 * The lens's glint, which gives a raised periscope away from afar: its size up close, metres, and the
 * least it shrinks to on screen further off, as a share of its distance (about four pixels at 1080p).
 * Nearer the camera than `GLINT_HIDDEN` metres (the officer's own eye) it is not drawn.
 */
const GLINT_SIZE = 0.35
const GLINT_ANGLE = 0.0045
const GLINT_HIDDEN = 3
/** How bright the glint is seen from behind the lens, and how sharply it brightens towards straight down it, 0..1 and a power. */
const GLINT_BACK = 0.3
const GLINT_SHARP = 3
/**
 * The feather the mast throws where it cuts the water: the foam collar round it, metres across, and
 * the streak trailing behind, metres wide, metres long for each m/s it moves and at most.
 */
const COLLAR = 0.9
const STREAK_WIDTH = 0.7
const STREAK_PER_SPEED = 1.6
const STREAK_MOST = 14
/** Slower than this, m/s, the mast leaves no streak, only its collar. */
const STREAK_SPEED = 0.5

const HULL_GREY = 0x3b4046
const DECK_GREY = 0x23272a
const BELLY_RED = 0x6a2b27
const MARK_WHITE = 0xc9cdc8
const HOLE_DARK = 0x0d0f11

type P = readonly [number, number, number]
const minus = (a: P, b: P): P => [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
const cross = (a: P, b: P): P => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
const dot = (a: P, b: P) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
const unit = (a: P): P => {
  const length = Math.hypot(a[0], a[1], a[2]) || 1
  return [a[0] / length, a[1] / length, a[2] / length]
}
const middle = (points: readonly P[]): P => {
  const sum = [0, 0, 0]
  for (const p of points) for (let axis = 0; axis < 3; axis++) sum[axis]! += p[axis]! / points.length
  return [sum[0]!, sum[1]!, sum[2]!]
}

/** A closed loop of points round a shape, each with the way its surface faces there. */
interface Loop {
  at: P[]
  facing: P[]
}

/** The fixed parts of the boat as one mesh in vertex colours: smooth skins lofted through loops, and flat-faced solids. */
class Built {
  private readonly positions: number[] = []
  private readonly normals: number[] = []
  private readonly colors: number[] = []

  /** A triangle shaded by its corners' own normals, wound to face the way they do. */
  private tri(a: P, b: P, c: P, na: P, nb: P, nc: P, color: number) {
    const face = cross(minus(b, a), minus(c, a))
    if (dot(face, face) < 1e-12) return
    const flip = dot(face, [na[0] + nb[0] + nc[0], na[1] + nb[1] + nc[1], na[2] + nb[2] + nc[2]]) < 0
    const corners = flip ? [a, c, b] : [a, b, c]
    const facings = flip ? [na, nc, nb] : [na, nb, nc]
    const paint = [(color >> 16) & 255, (color >> 8) & 255, color & 255].map((v) => (v / 255) ** 2.2)
    for (let i = 0; i < 3; i++) {
      this.positions.push(...corners[i]!)
      this.normals.push(...facings[i]!)
      this.colors.push(...paint)
    }
  }

  /** A smooth skin through `loops` (all of one length), each strip between two points of a loop in the colour `color` gives it. */
  loft(loops: readonly Loop[], color: (strip: number) => number) {
    for (let ring = 0; ring + 1 < loops.length; ring++) {
      const near = loops[ring]!, far = loops[ring + 1]!
      const count = near.at.length
      for (let k = 0; k < count; k++) {
        const next = (k + 1) % count
        this.tri(near.at[k]!, near.at[next]!, far.at[next]!, near.facing[k]!, near.facing[next]!, far.facing[next]!, color(k))
        this.tri(near.at[k]!, far.at[next]!, far.at[k]!, near.facing[k]!, far.facing[next]!, far.facing[k]!, color(k))
      }
    }
  }

  /** A flat, convex face through `points`, turned to face away from `inside`. */
  face(points: readonly P[], inside: P, color: number) {
    let normal = unit(cross(minus(points[1]!, points[0]!), minus(points[2]!, points[0]!)))
    if (dot(normal, minus(points[0]!, inside)) < 0) normal = [-normal[0], -normal[1], -normal[2]]
    for (let i = 1; i + 1 < points.length; i++) this.tri(points[0]!, points[i]!, points[i + 1]!, normal, normal, normal, color)
  }

  /** A solid between two loops of as many points: its sides in `color`, and its `upper` end in `cap` (the `lower` one too, unless it is `buried`). */
  prism(lower: readonly P[], upper: readonly P[], color: number, cap = color, buried = false) {
    const inside = middle([...lower, ...upper])
    for (let i = 0; i < lower.length; i++) {
      const next = (i + 1) % lower.length
      this.face([lower[i]!, lower[next]!, upper[next]!, upper[i]!], inside, color)
    }
    this.face(upper, inside, cap)
    if (!buried) this.face(lower, inside, cap)
  }

  /** A small flat panel `width` by `height` metres lying on a surface at `at` that faces along `facing`, stood `proud` of it. */
  plate(at: P, facing: P, width: number, height: number, color: number, proud = 0.012) {
    const n = unit(facing)
    const across = unit(cross([0, 1, 0], n))
    const up = cross(n, across)
    const corner = (u: number, v: number): P => [
      at[0] + n[0] * proud + across[0] * u * width / 2 + up[0] * v * height / 2,
      at[1] + n[1] * proud + across[1] * u * width / 2 + up[1] * v * height / 2,
      at[2] + n[2] * proud + across[2] * u * width / 2 + up[2] * v * height / 2,
    ]
    this.face([corner(-1, -1), corner(1, -1), corner(1, 1), corner(-1, 1)], minus(at, n), color)
  }

  geometry() {
    return new Geometry({ positions: new Float32Array(this.positions), normals: new Float32Array(this.normals), colors: new Float32Array(this.colors) })
  }
}

/**
 * The pressure hull: round, `HULL_R` metres in radius along its middle, its axis a little under the
 * box's so that its keel is the box's bottom and the casing on its back comes up to the box's top.
 * Its bow is an ellipse `SUB_BOW` long, as its collision shape tapers, and from `STERN_FROM` it draws
 * in to `TAIL_R` at the screw.
 */
const HULL_R = 1.24
const HULL_Y = HULL_R - HY
const STERN_FROM = 3
const STERN_END = HZ - 0.1
const TAIL_R = 0.16
/** Points round the hull, and the first and last strip (counted from the top) painted as its belly. */
const ROUND = 16
const BELLY = [4, 11] as const

/** The hull's radius at `z`, metres. */
function girth(z: number): number {
  const bow = -HZ + SUB_BOW
  if (z < bow) return HULL_R * Math.sqrt(Math.max(0, 1 - ((bow - z) / SUB_BOW) ** 2))
  if (z > STERN_FROM) return TAIL_R + (HULL_R - TAIL_R) * (1 - Math.min(1, (z - STERN_FROM) / (STERN_END - STERN_FROM)) ** 1.9)
  return HULL_R
}

/** The hull's section at `z`. */
function hullLoop(z: number): Loop {
  const r = girth(z)
  const slope = (girth(z + 0.01) - girth(z - 0.01)) / 0.02
  const at: P[] = [], facing: P[] = []
  for (let k = 0; k < ROUND; k++) {
    const turn = (k / ROUND) * Math.PI * 2
    at.push([r * Math.sin(turn), HULL_Y + r * Math.cos(turn), z])
    facing.push(r < 1e-6 ? [0, 0, -1] : unit([Math.sin(turn), Math.cos(turn), -slope]))
  }
  return { at, facing }
}

/** The casing on the hull's back, bow to stern: where each section is, half its deck's width, and its deck's height, metres. */
const CASING = [[-9.6, 0.1, 1.02], [-8.6, 0.55, 1.24], [-7.4, 0.9, HY], [5.5, 0.9, HY], [6.8, 0.6, 1.2], [7.9, 0.12, 0.86]] as const
/** How far the casing's sides go down into the hull, and how far they flare out by there, metres. */
const CASING_DEEP = 0.75
const CASING_FLARE = 0.1

/** The sail's plan: `SAIL_LENGTH` long from its rounded nose to its sharp tail, the nose `SAIL_NOSE` ahead of `SUB_SAIL_Z`. */
const SAIL_LENGTH = 4.4
const SAIL_NOSE = 2.3
/** The sail's sections from its foot in the casing up to its rounded top: height, share of its plan, and how far its skin leans up there. */
const SAIL_RINGS = [[HY - 0.2, 1, 0], [SAIL_TOP - 0.35, 0.93, 0], [SAIL_TOP - 0.1, 0.86, 0.6], [SAIL_TOP, 0.72, 1.8]] as const

/** Half the sail's width `u` of the way from its nose to its tail: an aerofoil's thickness, `SUB_SAIL_HALF[0]` at its widest. */
const sailHalf = (u: number) => SUB_SAIL_HALF[0] * 10 * (0.2969 * Math.sqrt(u) - 0.126 * u - 0.3516 * u ** 2 + 0.2843 * u ** 3 - 0.1036 * u ** 4)

/** The sail's outline at full size, in its own frame: round from the nose down the starboard side to the tail and back up the port side. */
function sailPlan(): { x: number; z: number; nx: number; nz: number }[] {
  const side = 8
  const points: { x: number; z: number }[] = []
  for (let i = 0; i < side * 2; i++) {
    const j = i <= side ? i : side * 2 - i
    const u = (1 - Math.cos((Math.PI * j) / side)) / 2
    points.push({ x: (i <= side ? 1 : -1) * Math.max(0, sailHalf(u)), z: -SAIL_NOSE + u * SAIL_LENGTH })
  }
  return points.map((p, i) => {
    const before = points[(i + points.length - 1) % points.length]!, after = points[(i + 1) % points.length]!
    // Square to the outline, and out from the middle of the sail.
    let nx = after.z - before.z, nz = -(after.x - before.x)
    if (nx * p.x + nz * (p.z - (SAIL_LENGTH / 2 - SAIL_NOSE)) < 0) [nx, nz] = [-nx, -nz]
    const length = Math.hypot(nx, nz) || 1
    return { ...p, nx: nx / length, nz: nz / length }
  })
}

/** A flat ring of `count` points, `radius` metres round `(x, y, z)`. */
const ring = (x: number, y: number, z: number, radius: number, count: number): P[] =>
  Array.from({ length: count }, (_, i) => [x + radius * Math.sin((i / count) * Math.PI * 2), y, z + radius * Math.cos((i / count) * Math.PI * 2)] as const)

/** Everything on the boat that never moves: hull, casing, sail, planes, tail fins, hatches and the tubes' mouths. */
function fixedParts(): Geometry {
  const built = new Built()

  // The hull, closer sections where it curves; grey above and red under.
  const stations = [-HZ, -HZ + 0.08, -HZ + 0.3, -HZ + 0.7, -HZ + 1.3, -HZ + 2.1, -HZ + SUB_BOW, STERN_FROM, 4.2, 5.4, 6.6, 7.8, 9, 10, STERN_END]
  built.loft(stations.map(hullLoop), (strip) => (strip >= BELLY[0] && strip <= BELLY[1] ? BELLY_RED : HULL_GREY))
  built.face(hullLoop(STERN_END).at, [0, HULL_Y, 0], HOLE_DARK)

  // The casing: a flat walkway on the hull's back, its sides going down into it.
  const section = ([z, half, top]: readonly [number, number, number]): P[] => [
    [-half, top, z], [half, top, z], [half + CASING_FLARE, top - CASING_DEEP, z], [-half - CASING_FLARE, top - CASING_DEEP, z],
  ]
  for (let i = 0; i + 1 < CASING.length; i++) {
    const near = section(CASING[i]!), far = section(CASING[i + 1]!)
    const inside = middle([...near, ...far])
    built.face([near[0]!, near[1]!, far[1]!, far[0]!], inside, DECK_GREY)
    built.face([near[1]!, near[2]!, far[2]!, far[1]!], inside, HULL_GREY)
    built.face([near[3]!, near[0]!, far[0]!, far[3]!], inside, HULL_GREY)
  }
  built.face(section(CASING[0]), [0, 0, 0], HULL_GREY)
  built.face(section(CASING[CASING.length - 1]!), [0, 0, 0], HULL_GREY)
  // A row of limber holes along each side of it, where the sea drains out as it comes up.
  const [, wide] = CASING[2]
  for (let z = CASING[2][0] + 0.6; z < CASING[3][0] - 0.3; z += 0.8) {
    for (const side of [-1, 1]) {
      const down = 0.3
      built.plate([side * (wide + CASING_FLARE * (down / CASING_DEEP)), HY - down, z], [side * CASING_DEEP, CASING_FLARE, 0], 0.42, 0.13, HOLE_DARK)
    }
  }

  // The sail: an aerofoil in plan, drawn in a little to a rounded top.
  const plan = sailPlan()
  const centre = SUB_SAIL_Z + SAIL_LENGTH / 2 - SAIL_NOSE
  const sailLoop = ([y, share, lean]: readonly [number, number, number]): Loop => ({
    at: plan.map((p) => [p.x * share, y, centre + (p.z - (SAIL_LENGTH / 2 - SAIL_NOSE)) * share] as const),
    facing: plan.map((p) => unit([p.nx, lean, p.nz])),
  })
  built.loft(SAIL_RINGS.map(sailLoop), () => HULL_GREY)
  built.face(sailLoop(SAIL_RINGS[SAIL_RINGS.length - 1]!).at, [0, 0, centre], DECK_GREY)
  // The bridge's windows, round its nose.
  const sill = SAIL_TOP - 0.75
  for (const i of [plan.length - 1, 0, 1]) {
    const p = plan[i]!
    built.plate([p.x * 0.94, sill, centre + (p.z - (SAIL_LENGTH / 2 - SAIL_NOSE)) * 0.94], [p.nx, 0, p.nz], 0.2, 0.15, HOLE_DARK, 0.03)
  }

  // The diving planes on the sail: swept back, thinner and shorter at their tips.
  const planeY = HY + SUB_SAIL_HALF[1] * 1.1, planeZ = SUB_SAIL_Z - 0.9
  for (const side of [-1, 1]) {
    const chord = (x: number, from: number, to: number, thick: number): P[] => [
      [side * x, planeY - thick, planeZ + from], [side * x, planeY + thick, planeZ + from], [side * x, planeY + thick, planeZ + to], [side * x, planeY - thick, planeZ + to],
    ]
    built.prism(chord(0.3, -0.5, 0.6, 0.05), chord(1.75, -0.1, 0.5, 0.02), DECK_GREY)
  }

  // The tail: four fins in an X, as Swedish boats wear them, clear of the bed and the quay.
  for (let fin = 0; fin < 4; fin++) {
    const turn = Math.PI / 4 + (fin * Math.PI) / 2
    const sin = Math.sin(turn), cos = Math.cos(turn)
    const chord = (out: number, from: number, to: number, thick: number): P[] =>
      ([[-thick, from], [thick, from], [thick, to], [-thick, to]] as const).map(([across, z]) => [across * cos + out * sin, HULL_Y - across * sin + out * cos, z] as const)
    built.prism(chord(0.25, 8.5, 10.1, 0.06), chord(1.45, 9.3, 10, 0.025), DECK_GREY)
  }

  // Each launch tube: a coaming round a dark mouth in the deck.
  for (const [x, y, z] of SUB_TUBE_AT) {
    built.prism(ring(x, y, z, LID + 0.05, 12), ring(x, y + 0.05, z, LID + 0.03, 12), HULL_GREY, HULL_GREY, true)
    built.face(ring(x, y + 0.055, z, LID - 0.05, 12), [x, y - 1, z], HOLE_DARK)
  }
  // The hatches fore and aft, each in its white rescue ring.
  for (const z of [-7, 4]) {
    built.face(ring(0, HY + 0.006, z, 0.52, 14), [0, 0, z], MARK_WHITE)
    built.face(ring(0, HY + 0.012, z, 0.42, 14), [0, 0, z], DECK_GREY)
    built.prism(ring(0, HY, z, 0.33, 12), ring(0, HY + 0.07, z, 0.3, 12), HULL_GREY, HULL_GREY, true)
  }
  return built.geometry()
}

// Shared by every submarine: parts only ever move, so none of these is disposed.
const FIXED = fixedParts()
/** One blade of the screw, standing out from the hub along Y, and the hub with its pointed cap. */
const BLADES = 5
const BLADE = boxGeometry(0.3, 0.6, 0.04)
const HUB = cylinderGeometry(0.16, 0.16, 0.36, 10)
HUB.rotateX(Math.PI / 2)
const SPINNER = cylinderGeometry(0.16, 0.02, 0.3, 10)
SPINNER.rotateX(Math.PI / 2)
const MAST = cylinderGeometry(0.08, 0.1, 2.6, 8)
const SCOPE_HEAD = boxGeometry(0.2, 0.28, 0.32)
const LENS = boxGeometry(0.14, 0.1, 0.02)
/** What else stands on the sail behind the periscope: the snorkel with its head valve, and a whip aerial. */
const SNORKEL = cylinderGeometry(0.1, 0.12, 0.7, 8)
const SNORKEL_HEAD = boxGeometry(0.26, 0.16, 0.34)
const AERIAL = cylinderGeometry(0.015, 0.03, 1.5, 5)
const LAMP = boxGeometry(0.05, 0.09, 0.14)
const LID_GEOMETRY = cylinderGeometry(LID, LID, 0.05, 12)
const GLINT = quadGeometry(1, 1)
/** A flat disc of foam, and a flat streak a metre long running back from its front edge along +Z. */
const FOAM_DISC = cylinderGeometry(0.5, 0.5, 0.02, 14)
const FOAM_STREAK = boxGeometry(1, 0.02, 1)
FOAM_STREAK.translate(0, 0, 0.5)

/** Where the camera is and the water's surface under the mast, as `setSub` needs them to draw the glint and the feather. */
export interface SubView {
  camera: Readonly<XYZ>
  /** The water's surface height at a point, metres, not finite where there is none. */
  waterAt(x: number, z: number): number
}

const lensAt = new Vec3()
const lensFacing = new Vec3()
const mastAt = new Vec3()
const toCamera = new Vec3()
const facing = new Quat()
const unturn = new Quat()
const FACE = { x: 0, y: 0, z: 1 }

/**
 * A small diesel-electric submarine in the Swedish manner: a round grey hull, red under, with an
 * elliptical bow and a long tapering stern, a flat casing along its back, an aerofoil sail carrying
 * its diving planes, four tail fins in an X and a bronze screw. Its periscope rises out of the sail
 * while its officer is at it (`SUB_MAST_UP`) and turns with their look; behind the sail the round
 * lids of its four launch tubes stand open while a tube is empty and shut again once it is loaded,
 * and a tube that has just fired smokes for a while. The screw turns while its captain is aboard.
 */
export class SubmarineModel implements VehicleModel {
  readonly root = new Group()
  private readonly paint = toonMaterial({ vertexColors: true })
  private readonly deck = toonMaterial({ color: DECK_GREY })
  private readonly bronze = toonMaterial({ color: 0x9a7436 })
  private readonly glass = toonMaterial({ color: 0x9fd4ff })
  private readonly port = new Plain({ color: 0xff3020, emissive: 0xff3020, lit: false })
  private readonly starboard = new Plain({ color: 0x30ff60, emissive: 0x30ff60, lit: false })
  /** The periscope: its mast slides up and down inside the sail and turns about its own axis. */
  readonly periscope = new Group()
  private readonly screw = new Group()
  /** Each tube's lid, hinged at its forward edge. */
  readonly lids: Group[] = []
  /** The tubes that were loaded when it was last drawn, to tell a launch by; `null` before it has been. */
  private loaded: number | null = null
  private launched: number[] = []
  /** How long each tube still smokes from its last launch, seconds. */
  readonly smoking: number[] = SUB_TUBE_AT.map(() => 0)
  /** How far the periscope is out of the sail, 0..1, and each lid is open, 0..1, as drawn. */
  private raised = 0
  private readonly open = SUB_TUBE_AT.map(() => 0)
  private manned = false
  private last = performance.now()
  /** The lens's glint, turned to the camera each frame; unfogged, so it shows as far as the submarine is sent. */
  readonly glint: Mesh
  private readonly shine = new Plain({ color: 0xfff4d8, emissive: 0xfff0c8, lit: false, blend: 'additive', opacity: 1, depthWrite: false, fog: false })
  /** The foam round the mast where it cuts the water, and the streak it trails as the boat moves. */
  readonly collar: Mesh
  readonly streak: Mesh
  private readonly foam = new Plain({ color: 0xf2f8fb, lit: false, blend: 'alpha', opacity: 0.85, depthWrite: false })
  /** Where the mast was last frame, in the world, to tell how fast it moves through the water. */
  private readonly mastWas = new Vec3()
  private mastKnown = false
  private wave = 0

  constructor(_id: number) {
    const part = (geometry: Geometry, material: Material, x: number, y: number, z: number, parent: Group = this.root) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }
    part(FIXED, this.paint, 0, 0, 0)

    // The screw behind the tail: skewed blades round a hub.
    this.screw.position.set(0, HULL_Y, HZ + 0.15)
    this.root.add(this.screw)
    part(HUB, this.bronze, 0, 0, 0, this.screw)
    part(SPINNER, this.bronze, 0, 0, 0.33, this.screw)
    for (let i = 0; i < BLADES; i++) {
      const arm = new Group()
      arm.rotation.z = (i / BLADES) * Math.PI * 2
      this.screw.add(arm)
      part(BLADE, this.bronze, 0, 0.42, 0, arm).rotation.y = BLADE_PITCH
    }

    // The periscope, its head at `SUB_MAST_HEIGHT` when raised, looking along -Z.
    this.periscope.position.set(0, SAIL_TOP - MAST_OUT, SUB_SAIL_Z)
    this.root.add(this.periscope)
    part(MAST, this.deck, 0, MAST_OUT - 1.3, 0, this.periscope)
    part(SCOPE_HEAD, this.deck, 0, MAST_OUT + 0.05, 0, this.periscope)
    part(LENS, this.glass, 0, MAST_OUT + 0.08, -0.17, this.periscope).castShadow = false

    // What stands on the sail for good: the snorkel, a whip aerial, and the running lights on its sides.
    part(SNORKEL, this.deck, 0, SAIL_TOP + 0.3, SUB_SAIL_Z + 0.75)
    part(SNORKEL_HEAD, this.deck, 0, SAIL_TOP + 0.7, SUB_SAIL_Z + 0.78)
    part(AERIAL, this.deck, 0, SAIL_TOP + 0.7, SUB_SAIL_Z + 1.3)
    const lampAt = SUB_SAIL_Z - 1.2
    const lampOut = sailHalf((lampAt - SUB_SAIL_Z + SAIL_NOSE) / SAIL_LENGTH) * 0.93
    part(LAMP, this.port, -lampOut, SAIL_TOP - 0.55, lampAt).castShadow = false
    part(LAMP, this.starboard, lampOut, SAIL_TOP - 0.55, lampAt).castShadow = false

    // The launch tubes' lids, each hinged at its forward edge, shut over the mouth in the deck.
    for (const [x, y, z] of SUB_TUBE_AT) {
      const hinge = new Group()
      hinge.position.set(x, y + 0.08, z - LID)
      this.root.add(hinge)
      part(LID_GEOMETRY, this.deck, 0, 0, LID, hinge)
      this.lids.push(hinge)
    }

    // What gives the raised periscope away: the lens's glint and the feather of foam round the mast.
    this.glint = new Mesh(GLINT, this.shine)
    this.collar = new Mesh(FOAM_DISC, this.foam)
    this.streak = new Mesh(FOAM_STREAK, this.foam)
    for (const mesh of [this.glint, this.collar, this.streak]) {
      mesh.castShadow = false
      mesh.visible = false
      this.root.add(mesh)
    }
  }

  /**
   * Raises and turns the periscope and swings the lids from the snapshot's `sub`, easing over `dt`
   * seconds. Seen from `view`, a raised periscope glints and throws a feather of foam. Called after
   * `update`, so the hull is where it is drawn.
   */
  setSub(sub: Sub | null, dt: number, view: SubView | null = null) {
    const step = Math.min(1, Math.max(0, dt))
    // A tube that was loaded and is not any more has just fired.
    this.smoking.forEach((left, tube) => (this.smoking[tube] = Math.max(0, left - step)))
    if (sub !== null && this.loaded !== null) {
      for (let tube = 0; tube < SUB_TUBE_AT.length; tube++) {
        if ((this.loaded & ~sub.tubes & (1 << tube)) === 0) continue
        this.launched.push(tube)
        this.smoking[tube] = TUBE_SMOKES
      }
    }
    this.loaded = sub?.tubes ?? null
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
      lid.rotation.x = -this.open[tube]! * LID_OPEN
    })
    this.giveAway(view, step)
  }

  /** The tubes that have fired since this was last asked. */
  takeLaunches(): number[] {
    const launched = this.launched
    this.launched = []
    return launched
  }

  /** The mouth of `tube` in the world, as the boat is drawn, and the way it points. */
  tubeMouth(tube: number): { at: XYZ; up: XYZ } {
    const [x, y, z] = SUB_TUBE_AT[tube]!
    const at = mastAt.set(x, y, z).applyQuaternion(this.root.quaternion).add(this.root.position)
    const up = lensFacing.set(0, 1, 0).applyQuaternion(this.root.quaternion)
    return { at: { x: at.x, y: at.y, z: at.z }, up: { x: up.x, y: up.y, z: up.z } }
  }

  /**
   * The raised periscope seen from afar: its lens catches the light, brightest looking straight down
   * it and never smaller on screen than a few pixels, and where its mast cuts the water it wears a
   * collar of foam and trails a white streak as the boat moves.
   */
  private giveAway(view: SubView | null, dt: number) {
    if (this.raised <= 0.01 || view === null) {
      this.glint.visible = this.collar.visible = this.streak.visible = false
      this.mastKnown = false
      return
    }
    const { root } = this
    const yaw = this.periscope.rotation.y
    const sin = Math.sin(yaw), cos = Math.cos(yaw)
    const head = this.periscope.position.y + MAST_OUT + 0.08
    // The lens, in the hull's frame and then the world's, and the way it looks.
    const x = -0.22 * sin, z = SUB_SAIL_Z - 0.22 * cos
    lensAt.set(x, head, z).applyQuaternion(root.quaternion).add(root.position)
    lensFacing.set(-sin, 0, -cos).applyQuaternion(root.quaternion)
    toCamera.set(view.camera.x - lensAt.x, view.camera.y - lensAt.y, view.camera.z - lensAt.z)
    const distance = toCamera.length()
    this.glint.visible = distance > GLINT_HIDDEN
    if (this.glint.visible) {
      toCamera.multiplyScalar(1 / distance)
      const down = Math.max(0, lensFacing.x * toCamera.x + lensFacing.y * toCamera.y + lensFacing.z * toCamera.z)
      this.wave += dt * 7
      const twinkle = 0.85 + 0.15 * Math.sin(this.wave) * Math.sin(this.wave * 2.3)
      this.shine.opacity = this.raised * twinkle * (GLINT_BACK + (1 - GLINT_BACK) * down ** GLINT_SHARP)
      // Turned to the camera in the world, so in the hull's frame its turn undoes the hull's.
      facing.setFromUnitVectors(FACE, toCamera)
      this.glint.quaternion.copy(unturn.copy(root.quaternion).invert().multiply(facing))
      this.glint.position.set(x, head, z)
      this.glint.scale.setScalar(Math.max(GLINT_SIZE, distance * GLINT_ANGLE) * (0.6 + 0.4 * down))
    }

    // The feather, only while the water stands between the sail's top and the head: the mast alone cuts it.
    const mast = mastAt.set(0, 0, SUB_SAIL_Z).applyQuaternion(root.quaternion).add(root.position)
    const waterline = view.waterAt(mast.x, mast.z) - mast.y
    const wet = Number.isFinite(waterline) && waterline > SAIL_TOP && waterline < head
    const speed = this.mastKnown && dt > 0 ? Math.hypot(mast.x - this.mastWas.x, mast.z - this.mastWas.z) / dt : 0
    lensFacing.set(0, 0, -1).applyQuaternion(root.quaternion)
    const ahead = (mast.x - this.mastWas.x) * lensFacing.x + (mast.z - this.mastWas.z) * lensFacing.z >= 0
    this.mastWas.copy(mast)
    this.mastKnown = true
    this.collar.visible = wet
    this.streak.visible = wet && speed > STREAK_SPEED
    if (!wet) return
    // Wider further off, so it still reads as a white fleck at a few hundred metres.
    const spread = Math.max(1, (distance * GLINT_ANGLE) / STREAK_WIDTH)
    this.collar.position.set(0, waterline + 0.02, SUB_SAIL_Z)
    this.collar.scale.set(COLLAR * spread, 1, COLLAR * spread)
    if (this.streak.visible) {
      // It runs back from the mast the way the boat came.
      this.streak.position.set(0, waterline + 0.015, SUB_SAIL_Z)
      this.streak.rotation.y = ahead ? 0 : Math.PI
      this.streak.scale.set(STREAK_WIDTH * spread, 1, Math.min(STREAK_MOST, speed * STREAK_PER_SPEED))
    }
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
    for (const material of [this.paint, this.deck, this.bronze, this.glass, this.port, this.starboard, this.shine, this.foam]) material.dispose()
    this.root.removeFromParent()
  }
}
