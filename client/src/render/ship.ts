import { Geometry, Group, Material, Mesh, Quaternion, Vector3, boxGeometry, cylinderGeometry, sphereGeometry } from '@ragnarok/engine'
import type { VehicleModel } from './car'
import { toonMaterial } from './toon'
import { SHIP_BOW, SHIP_BULWARK_HEIGHT, SHIP_BULWARK_THICK, SHIP_DECK_Y, SHIP_GUN_HALF_EXTENTS, SHIP_GUN_PIVOT, SHIP_GUN_Z, SHIP_HALF_EXTENTS, SHIP_HELM, SHIP_MAST_HEIGHT, SHIP_MAST_RADIUS, SHIP_MAST_Z } from '@/sim/consts'
import { boom, pointOfSail } from '@/sim/wind'

/**
 * The wind every ship's sail is drawn in this frame, which way it blows (a unit `[x, z]`): `Game` sets
 * it from the server's tick (`sim/wind`), as it sets the waves' clock.
 */
export const seaWind: { blowing: readonly [number, number] } = { blowing: [1, 0] }

const [HX, HY, HZ] = SHIP_HALF_EXTENTS
/** Her depth, length and mast against the brig the shapes below were first drawn for, which was 2.2 m deep, 18 m long and had a 12 m mast. */
const S = HY / 1.1
const L = HZ / 9
const M = SHIP_MAST_HEIGHT / 12
/** The top of the bulwarks, where the sides end, in the hull frame. */
const RAIL_Y = SHIP_DECK_Y + SHIP_BULWARK_HEIGHT
/** The stem's half width, as a share of the beam's, as the sim's hull has it. */
const STEM = 0.08
const STATIONS = 25
/** Down the side from the rail: each row's height, how far out it is as a share of the half beam there, and the band below it. */
const ROWS: readonly { y: number; out: number; color: number }[] = [
  { y: RAIL_Y, out: 1, color: 0xc49a3c },
  { y: SHIP_DECK_Y + 0.3, out: 1, color: 0x2b1d14 },
  { y: SHIP_DECK_Y - 0.1, out: 1, color: 0x4a3222 },
  { y: 0.2 * S, out: 0.97, color: 0x4a3222 },
  { y: -0.4 * S, out: 0.9, color: 0x7a3626 },
  { y: -0.85 * S, out: 0.68, color: 0x7a3626 },
  { y: -HY, out: 0, color: 0x7a3626 },
]
const DECK = 0xa57c4f
const INSIDE = 0x6e4b2e
const RAIL = 0x3b2a1c
const SPAR = 0x5b4028
const CANVAS = 0xe9e0c9
const ROPE = 0x4b3f2e
/** The yard, how high over the deck it hangs and how long it is; the sail under it, how deep and wide. */
const YARD_Y = SHIP_DECK_Y + 0.85 * SHIP_MAST_HEIGHT
const YARD_LENGTH = 0.77 * SHIP_MAST_HEIGHT
const SAIL_WIDTH = 0.68 * SHIP_MAST_HEIGHT
const SAIL_DROP = 0.57 * SHIP_MAST_HEIGHT
const SAIL_COLUMNS = 8
const SAIL_ROWS = 6
/** How deep the sail's belly is, metres, hanging slack and full. */
const BELLY_SLACK = 0.25 * M
const BELLY_FULL = 1.6 * M
/** How quickly the sail fills or slackens, and is let out or braced round, per second. */
const SAIL_EASE = 1.5

/** The half beam at `z` along the hull: tapering in over the bow to the stem, as the sim's does. */
const beam = (z: number) => {
  const fromStem = (z + HZ) / SHIP_BOW
  return fromStem >= 1 ? HX * (1 - 0.06 * Math.max(0, (z - (HZ - 3 * L)) / (3 * L))) : HX * (STEM + (1 - STEM) * fromStem)
}
/** How far the rail rises over its height amidships at `z`: a sheer up to the bow and the stern. */
const sheer = (z: number) => 0.45 * S * Math.max(0, (-z - (HZ - SHIP_BOW - L)) / (SHIP_BOW + L)) ** 2 + 0.3 * S * Math.max(0, (z - (HZ - 3 * L)) / (3 * L)) ** 2

/** Flat-shaded triangles in vertex colours, each quad turned to face along `out`. */
class Faces {
  readonly positions: number[] = []
  readonly normals: number[] = []
  readonly colors: number[] = []

  quad(a: Vector3, b: Vector3, c: Vector3, d: Vector3, out: Vector3, color: number) {
    const normal = new Vector3().subVectors(b, a).cross(new Vector3().subVectors(c, a))
    if (normal.lengthSq() < 1e-10) normal.subVectors(c, a).cross(new Vector3().subVectors(d, a))
    const corners = normal.dot(out) < 0 ? [a, d, c, b] : [a, b, c, d]
    if (normal.dot(out) < 0) normal.negate()
    normal.normalize()
    const [r, g, bl] = [((color >> 16) & 255) / 255, ((color >> 8) & 255) / 255, (color & 255) / 255].map((v) => v ** 2.2)
    for (const index of [0, 1, 2, 0, 2, 3]) {
      const p = corners[index]!
      this.positions.push(p.x, p.y, p.z)
      this.normals.push(normal.x, normal.y, normal.z)
      this.colors.push(r!, g!, bl!)
    }
  }

  geometry() {
    return new Geometry({ positions: new Float32Array(this.positions), normals: new Float32Array(this.normals), colors: new Float32Array(this.colors) })
  }
}

/** The hull, from the rail down to the keel, its deck, the inside of its bulwarks and the rail's cap, the transom and the stem. */
function hullGeometry(): Geometry {
  const faces = new Faces()
  const z = (i: number) => HZ - (2 * HZ * i) / (STATIONS - 1)
  // Each side's rows at each station, outside; the rail's top rises with the sheer.
  const side = (i: number, s: number) =>
    ROWS.map((row, k) => new Vector3(s * beam(z(i)) * row.out, k === 0 ? row.y + sheer(z(i)) : row.y, z(i)))
  for (const s of [-1, 1]) {
    for (let i = 0; i + 1 < STATIONS; i++) {
      const [here, next] = [side(i, s), side(i + 1, s)]
      for (let k = 0; k + 1 < ROWS.length; k++) {
        const out = new Vector3(s, (here[k]!.y + here[k + 1]!.y) / 2 < -0.6 ? -1 : 0, 0)
        faces.quad(here[k]!, next[k]!, next[k + 1]!, here[k + 1]!, out, ROWS[k]!.color)
      }
      // The inside of the bulwark, and the cap along its top.
      const inset = (p: Vector3, y: number) => new Vector3(p.x - s * SHIP_BULWARK_THICK * 2, y, p.z)
      const [top, nextTop] = [here[0]!, next[0]!]
      faces.quad(inset(top, SHIP_DECK_Y), inset(nextTop, SHIP_DECK_Y), inset(nextTop, nextTop.y), inset(top, top.y), new Vector3(-s, 0, 0), INSIDE)
      faces.quad(top, nextTop, inset(nextTop, nextTop.y), inset(top, top.y), new Vector3(0, 1, 0), RAIL)
    }
  }
  // The deck, plank by plank down its length.
  for (let i = 0; i + 1 < STATIONS; i++) {
    const [a, b] = [beam(z(i)) - SHIP_BULWARK_THICK * 2, beam(z(i + 1)) - SHIP_BULWARK_THICK * 2]
    faces.quad(new Vector3(-a, SHIP_DECK_Y, z(i)), new Vector3(a, SHIP_DECK_Y, z(i)), new Vector3(b, SHIP_DECK_Y, z(i + 1)), new Vector3(-b, SHIP_DECK_Y, z(i + 1)), new Vector3(0, 1, 0), i % 2 ? DECK : 0x9a7348)
  }
  // The transom across the stern, and the narrow stem.
  for (const [i, out] of [[0, 1], [STATIONS - 1, -1]] as const) {
    const [left, right] = [side(i, -1), side(i, 1)]
    for (let k = 0; k + 1 < ROWS.length; k++) faces.quad(left[k]!, right[k]!, right[k + 1]!, left[k + 1]!, new Vector3(0, 0, out), out > 0 ? 0x3e2a1c : ROWS[k]!.color)
  }
  return faces.geometry()
}

const HULL = hullGeometry()
const MAST = cylinderGeometry(SHIP_MAST_RADIUS * 0.7, SHIP_MAST_RADIUS, SHIP_MAST_HEIGHT, 10)
const TOPMAST = cylinderGeometry(0.08 * M, 0.12 * M, 2.4 * M, 8)
const TOP = cylinderGeometry(0.7 * M, 0.6 * M, 0.18, 10)
const YARD = cylinderGeometry(0.1, 0.1, YARD_LENGTH, 8)
YARD.rotateZ(Math.PI / 2)
const FURLED = cylinderGeometry(0.28, 0.28, SAIL_WIDTH, 8)
FURLED.rotateZ(Math.PI / 2)
const BOWSPRIT = cylinderGeometry(0.1 * M, 0.16 * M, 6 * L, 8)
BOWSPRIT.rotateX(Math.PI / 2)
const WHEEL = cylinderGeometry(0.35, 0.35, 0.05, 16)
WHEEL.rotateX(Math.PI / 2)
const HUB = cylinderGeometry(0.1, 0.1, 0.2, 8)
HUB.rotateX(Math.PI / 2)
const SPOKE = boxGeometry(0.9, 0.04, 0.04)
/** A gun's port through the bulwark, dark, with the barrel run out of it. */
const PORT = boxGeometry(0.03, 0.34, 0.4)
/** How high over the deck a gun's barrel goes through its port. */
const PORT_Y = SHIP_DECK_Y + SHIP_GUN_HALF_EXTENTS[1] + SHIP_GUN_PIVOT[1]
const PEDESTAL = boxGeometry(0.3, 0.7, 0.3)
const LANTERN = boxGeometry(0.3, 0.45, 0.3)
const SKULL = sphereGeometry(0.17, 10, 8)
SKULL.scale(1, 1, 0.25)
const BONE = boxGeometry(0.55, 0.06, 0.03)
const LINE = cylinderGeometry(0.025, 0.025, 1, 4)

/** A flat grid of `columns` × `rows` quads, to be bent each frame: a sail or a flag. */
function cloth(columns: number, rows: number): Geometry {
  const count = (columns + 1) * (rows + 1)
  const indices: number[] = []
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < columns; i++) {
      const a = j * (columns + 1) + i
      indices.push(a, a + columns + 1, a + 1, a + 1, a + columns + 1, a + columns + 2)
    }
  }
  return new Geometry({ positions: new Float32Array(count * 3), normals: new Float32Array(count * 3), indices: new Uint16Array(indices), bounds: { x: 0, y: 0, z: 0, radius: 8 } })
}

const ease = (from: number, to: number, rate: number) => from + (to - from) * Math.min(1, rate)

/**
 * A pirate brig: a dark hull with an ochre wale and a red bottom, tapering to its stem and rising at
 * bow and stern, a plank deck inside its bulwarks, one mast with a square sail on its yard, a
 * bowsprit, the helm at the stern and a black flag at the masthead. Her six guns are vehicles of their
 * own, drawn where they stand on her deck.
 *
 * The sail is furled while nobody has the helm. Set, it is braced round to the wind (`sim/wind`'s
 * boom) and bellies out as far as the wind fills it, or is taken aback into the wind; the flag streams
 * downwind. Neither is on the wire: both are drawn from the server's wind at the frame's tick.
 */
export class ShipModel implements VehicleModel {
  readonly root = new Group()
  private readonly hull = toonMaterial({ vertexColors: true })
  private readonly wood = toonMaterial({ color: SPAR })
  private readonly canvas = toonMaterial({ color: CANVAS, side: 'double' })
  private readonly black = toonMaterial({ color: 0x141414, side: 'double' })
  private readonly bone = toonMaterial({ color: 0xf2efe6 })
  private readonly rope = toonMaterial({ color: ROPE })
  private readonly lamp = new Material({ color: 0xffcf6a, emissive: 0xffb340, lit: false })
  private readonly yard = new Group()
  private readonly sail = cloth(SAIL_COLUMNS, SAIL_ROWS)
  private readonly sailMesh: Mesh
  private readonly furled: Mesh
  private readonly flagPole = new Group()
  private readonly flag = cloth(6, 3)
  private readonly turn = new Quaternion()
  private readonly local = new Vector3()
  /** The boom's swing and the sail's fill (negative taken aback), as drawn. */
  private swing = 0
  private fill = 0
  private set = 0
  private lastAt = 0

  constructor(_id: number) {
    const hull = new Mesh(HULL, this.hull)
    hull.castShadow = true
    this.root.add(hull)
    const part = (geometry: Geometry, material: Material, x: number, y: number, z: number, parent: Group = this.root) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.set(x, y, z)
      mesh.castShadow = true
      parent.add(mesh)
      return mesh
    }
    part(MAST, this.wood, 0, SHIP_DECK_Y + SHIP_MAST_HEIGHT / 2, SHIP_MAST_Z)
    part(TOP, this.wood, 0, SHIP_DECK_Y + SHIP_MAST_HEIGHT * 0.8, SHIP_MAST_Z)
    part(TOPMAST, this.wood, 0, SHIP_DECK_Y + SHIP_MAST_HEIGHT + 1.1 * M, SHIP_MAST_Z)
    const bowsprit = part(BOWSPRIT, this.wood, 0, RAIL_Y + 0.7 * S, -HZ - 2.2 * L)
    bowsprit.rotation.x = 0.28
    // Stays from the masthead down to the bowsprit's end and to the stern.
    const masthead = new Vector3(0, SHIP_DECK_Y + SHIP_MAST_HEIGHT, SHIP_MAST_Z)
    for (const end of [new Vector3(0, RAIL_Y + 1.5 * S, -HZ - 5 * L), new Vector3(0, RAIL_Y + 0.3, HZ - 0.3), new Vector3(HX - 0.1, RAIL_Y, SHIP_MAST_Z + 1.2), new Vector3(0.1 - HX, RAIL_Y, SHIP_MAST_Z + 1.2)]) {
      const line = part(LINE, this.rope, (masthead.x + end.x) / 2, (masthead.y + end.y) / 2, (masthead.z + end.z) / 2)
      const along = new Vector3().subVectors(end, masthead)
      line.scale.set(1, along.length(), 1)
      line.quaternion.setFromUnitVectors(new Vector3(0, 1, 0), along.normalize())
    }

    // The helm, facing aft, and the stern lantern.
    const [, helmY, helmZ] = SHIP_HELM
    part(PEDESTAL, this.wood, 0, SHIP_DECK_Y + 0.35, helmZ - 0.1)
    const wheel = new Group()
    wheel.position.set(0, helmY + 0.2, helmZ)
    this.root.add(wheel)
    part(WHEEL, this.wood, 0, 0, 0, wheel)
    part(HUB, this.wood, 0, 0, 0, wheel)
    for (const angle of [0, Math.PI / 4, Math.PI / 2, (3 * Math.PI) / 4]) part(SPOKE, this.wood, 0, 0, 0, wheel).rotation.z = angle
    part(LANTERN, this.lamp, 0, RAIL_Y + 0.4, HZ - 0.2).castShadow = false
    // The gunports, outside and in, where her guns' barrels come through the bulwarks.
    for (const z of SHIP_GUN_Z) {
      for (const side of [-1, 1]) {
        for (const x of [beam(z) + 0.01, beam(z) - SHIP_BULWARK_THICK * 2 - 0.01]) part(PORT, this.black, side * x, PORT_Y, z).castShadow = false
      }
    }

    // The yard turns about the mast to the wind, with the sail under it, or the sail furled on it.
    this.yard.position.set(0, YARD_Y, SHIP_MAST_Z)
    this.root.add(this.yard)
    part(YARD, this.wood, 0, 0, 0.2, this.yard)
    this.sailMesh = part(this.sail, this.canvas, 0, 0, 0.25, this.yard)
    this.furled = part(FURLED, this.canvas, 0, -0.2, 0.25, this.yard)

    // The black flag at the masthead, with its skull and crossed bones on both sides.
    this.flagPole.position.set(0, SHIP_DECK_Y + SHIP_MAST_HEIGHT + 2.2 * M, SHIP_MAST_Z)
    this.flagPole.scale.set(0.8, 0.8, 0.8)
    this.root.add(this.flagPole)
    part(this.flag, this.black, 0, 0, 0, this.flagPole).castShadow = false
    for (const face of [-0.04, 0.04]) {
      part(SKULL, this.bone, 0.65, -0.32, face, this.flagPole).castShadow = false
      for (const tilt of [0.6, -0.6]) part(BONE, this.bone, 0.65, -0.6, face, this.flagPole).rotation.z = tilt
    }
    this.bend(0, 1)
  }

  update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    _steer: number,
    seats: readonly number[],
  ) {
    this.root.position.set(...position)
    this.root.quaternion.set(...rotation)
    const now = performance.now()
    const dt = this.lastAt ? Math.min(0.1, (now - this.lastAt) / 1000) : 1
    this.lastAt = now

    // The wind in the hull's frame, and her heading.
    this.turn.set(...rotation).invert()
    const [wx, wz] = seaWind.blowing
    this.local.set(wx, 0, wz).applyQuaternion(this.turn)
    const nose = new Vector3(0, 0, -1).applyQuaternion(this.root.quaternion)
    const flat = Math.hypot(nose.x, nose.z) || 1
    const heading: [number, number] = [nose.x / flat, nose.z / flat]
    // Set while someone has the helm, braced round and filling as the wind allows; furled otherwise.
    const manned = (seats[0] ?? 0) !== 0
    this.set = ease(this.set, manned ? 1 : 0, dt * SAIL_EASE)
    this.swing = ease(this.swing, manned ? boom(heading, seaWind.blowing) : 0, dt * SAIL_EASE)
    // The sail bellies forward, along its yard's -Z, with the wind behind it; blowing onto its front
    // it is taken aback, pressed back against the mast.
    const behind = this.local.x * -Math.sin(this.swing) + this.local.z * -Math.cos(this.swing) >= 0 ? 1 : -1
    this.fill = ease(this.fill, manned ? behind * pointOfSail(heading, seaWind.blowing) : 0, dt * SAIL_EASE)
    this.yard.rotation.y = this.swing
    this.sailMesh.visible = this.set > 0.05
    this.furled.visible = this.set < 0.95
    this.bend(this.fill, this.set)

    // The flag streams downwind, fluttering.
    this.flagPole.rotation.y = Math.atan2(-this.local.z, this.local.x)
    this.wave(now / 1000)
  }

  /** Shapes the sail: let down `set` of the way from the yard, and bellied out by `fill` (forward, or aft taken aback). */
  private bend(fill: number, set: number) {
    const positions = this.sail.positions
    const depth = (BELLY_SLACK + (BELLY_FULL - BELLY_SLACK) * Math.abs(fill)) * Math.sign(fill || 1)
    const drop = SAIL_DROP * Math.max(0.05, set)
    let at = 0
    for (let j = 0; j <= SAIL_ROWS; j++) {
      const v = j / SAIL_ROWS
      for (let i = 0; i <= SAIL_COLUMNS; i++) {
        const u = i / SAIL_COLUMNS
        const belly = Math.sin(Math.PI * u) * (0.3 + 0.7 * Math.sin(Math.PI * (0.15 + 0.8 * v)))
        // The foot is narrower than the head, sheeted in to the rail.
        const width = SAIL_WIDTH * (1 - 0.12 * v)
        positions[at++] = (u - 0.5) * width
        positions[at++] = -v * drop
        positions[at++] = -depth * belly * set
      }
    }
    this.sail.computeNormals()
    this.sail.needsUpdate()
  }

  /** The flag's flutter at `time` seconds. */
  private wave(time: number) {
    const positions = this.flag.positions
    let at = 0
    for (let j = 0; j <= 3; j++) {
      for (let i = 0; i <= 6; i++) {
        const u = i / 6
        positions[at++] = u * 1.3
        positions[at++] = -(j / 3) * 0.85
        positions[at++] = Math.sin(time * 7 - u * 5) * 0.12 * u
      }
    }
    this.flag.computeNormals()
    this.flag.needsUpdate()
  }

  dispose() {
    for (const material of [this.hull, this.wood, this.canvas, this.black, this.bone, this.rope, this.lamp]) material.dispose()
    this.sail.dispose()
    this.flag.dispose()
    this.root.removeFromParent()
  }
}
