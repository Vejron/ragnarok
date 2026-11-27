import { Geometry, Group, InstancedMesh, Material, Texture, boxGeometry, cylinderGeometry, mat4 } from '@ragnarok/engine'
import { DEALER_PORT, DEALER_PORT_HEIGHT } from '@/sim/consts'
import { ClientSim } from '@/sim/wasm/sim'
import type { Dealer, Point2 } from '@/world'
import { painted } from './lines'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * The world's vehicle dealers (`WorldData.dealers`, DEALERS.md), each a carport against its store's
 * wall: a flat white roof on steel posts with a blue fascia round it, a lit sign with the dealer's name
 * standing on its front edge, and a roller door in the store's wall at its back. An ordered vehicle
 * comes out of that door: the server stands it in the carport, and the door rolls up as it does.
 *
 * What is solid comes from the physics' own parts (`shared::dealer::structure`, through the WASM's
 * `dealerParts`), so what is seen is what is hit; the rest is painted on. Every carport has the same
 * layout in its own frame (its mouth at the origin, its +Z out onto the lot, the store's wall at
 * -`DEALER_PORT[1]`), so each piece is one instanced mesh for all of them.
 *
 * A vehicle driven in towards the door is sold to the dealer when its nose reaches it (the server
 * judges that); the door rolls up as soon as a vehicle is under the carport's roof, to take it in.
 */

const WHITE = 0xf4f4f0
const BLUE = 0x1f5fbf
const CONCRETE = 0xbdb9ae
const STEEL = 0x8a9196
const DARK = 0x1a1d21
const DOOR = 0xd9dde2
const RIB = 0xb7bcc3
/** The colour of each `PAINT_*` of `shared::station`, by its number, as the dealer's parts use them. */
const PAINTS = [WHITE, BLUE, BLUE, CONCRETE, STEEL]

/** The name on the signs: a dealer of the game's own. */
const NAME = 'NORD BIL'
/** Carports further than this from the eye are not drawn, metres. */
const DEALER_REACH = 900
/** How long the door takes to roll up or down, seconds. */
const DOOR_TRAVEL = 1.8
/** How long the door stays up after a vehicle is ordered, seconds. */
const DOOR_HOLD = 6
/** How long the door stays up after a vehicle under the roof has gone, seconds. */
const DOOR_NEAR = 2
/** How much of the door is left showing when it is rolled up, a share of its height. */
const ROLLED = 0.08

const [WIDTH, DEPTH] = DEALER_PORT
/** The floor of a carport, metres over its base: the top of its paving. */
const FLOOR = 0.05
/** The roof's thickness, and how far it reaches past the posts at the sides and over the mouth, as `shared::dealer` has them. */
const ROOF = 0.35
const EAVES = 0.4
const POST_RADIUS = 0.16
/** The roof's half width across the mouth, and its front edge along Z, metres. */
const ROOF_HALF = WIDTH / 2 + POST_RADIUS + EAVES
const ROOF_FRONT = EAVES
/** The roof's underside and its top, metres over the base. */
const ROOF_UNDER = FLOOR + DEALER_PORT_HEIGHT
const ROOF_TOP = ROOF_UNDER + ROOF
/** The door in the store's wall: its width and height, and how far in front of the wall it hangs, metres. */
const DOOR_WIDTH = WIDTH - 1
const DOOR_TALL = DEALER_PORT_HEIGHT - 0.6
const DOOR_Z = -DEPTH + 0.1
/** The sign on the roof's front edge: its width, height and middle over the base, metres. */
const SIGN = [6.4, 1.3, ROOF_TOP + 0.75] as const

/** A box `w × h × d` with its middle at `(x, y, z)` in a carport's frame, in `color`. */
function block(w: number, h: number, d: number, x: number, y: number, z: number, color: number): Geometry {
  return painted(boxGeometry(w, h, d).translate(x, y, z), color)
}

/** The solid parts of a carport at the origin, unturned, its base at 0, as the physics stands them. */
function solidParts(): Geometry[] {
  const flat = ClientSim.dealerParts(0, 0, 0, 0)
  const parts: Geometry[] = []
  for (let i = 0; i + 9 <= flat.length; i += 9) {
    const [shape, x, y, z, , a, b, c, look] = flat.slice(i, i + 9) as unknown as number[]
    const geometry = shape === 0 ? boxGeometry(a! * 2, b! * 2, c! * 2) : cylinderGeometry(a!, a!, b! * 2, 12)
    parts.push(painted(geometry.translate(x!, y!, z!), PAINTS[look!] ?? WHITE))
  }
  return parts
}

/** Everything of a carport that is not lit or moving: its solid parts, the fascia, the sign's frame and the dark doorway. */
function carportGeometry(): Geometry {
  const parts = solidParts()
  const middle = (ROOF_FRONT - DEPTH) / 2
  const length = ROOF_FRONT + DEPTH
  // A blue fascia round the roof's edge, a little proud of its faces, so the two never fight.
  parts.push(block(ROOF_HALF * 2 + 0.08, ROOF * 0.8, 0.04, 0, ROOF_UNDER + ROOF / 2, ROOF_FRONT + 0.02, BLUE))
  for (const side of [-1, 1]) parts.push(block(0.04, ROOF * 0.8, length, side * (ROOF_HALF + 0.02), ROOF_UNDER + ROOF / 2, middle, BLUE))
  // The sign's two legs on the roof and its blue board.
  const [sw, sh, sy] = SIGN
  for (const x of [-sw / 3, sw / 3]) parts.push(block(0.12, sy - sh / 2 - ROOF_TOP, 0.12, x, (ROOF_TOP + sy - sh / 2) / 2, ROOF_FRONT - 0.4, STEEL))
  parts.push(block(sw, sh, 0.2, 0, sy, ROOF_FRONT - 0.4, BLUE))
  // The doorway behind the roller door: dark, so the door rolling up opens onto the store's inside.
  parts.push(block(DOOR_WIDTH + 0.3, DOOR_TALL + 0.15, 0.04, 0, FLOOR + (DOOR_TALL + 0.15) / 2, -DEPTH + 0.03, DARK))
  // The door's frame and the drum it rolls onto.
  for (const side of [-1, 1]) parts.push(block(0.15, DOOR_TALL + 0.3, 0.12, side * (DOOR_WIDTH / 2 + 0.2), FLOOR + (DOOR_TALL + 0.3) / 2, -DEPTH + 0.06, STEEL))
  parts.push(painted(cylinderGeometry(0.28, 0.28, DOOR_WIDTH + 0.3, 12).rotateZ(Math.PI / 2).translate(0, FLOOR + DOOR_TALL + 0.4, -DEPTH + 0.3), STEEL))
  // Painted bays on the floor: a white line down either side of the clear width.
  for (const side of [-1, 1]) parts.push(block(0.12, 0.012, DEPTH - 0.6, side * (WIDTH / 2 - 0.4), FLOOR + 0.006, -DEPTH / 2, WHITE))
  return Geometry.merge(parts)
}

/** What of a carport lights up by night: the lamps under its roof. */
function lampGeometry(): Geometry {
  const parts: Geometry[] = []
  for (const z of [-0.2, -0.5, -0.8]) {
    for (const x of [-WIDTH / 4, WIDTH / 4]) parts.push(block(1.2, 0.04, 0.3, x, ROOF_UNDER - 0.02, z * DEPTH, 0xfff6dc))
  }
  return Geometry.merge(parts)
}

/** The dealer's name, white on nothing. */
function letteringTexture(): Texture {
  const canvas = document.createElement('canvas')
  canvas.width = 512
  canvas.height = 128
  const g = canvas.getContext('2d')
  if (g) {
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.fillStyle = '#ffffff'
    g.font = '900 92px "Arial Black", system-ui, sans-serif'
    g.fillText(NAME, 256, 68)
  }
  return new Texture(canvas, { srgb: true, wrap: 'clamp' })
}

/** The name on the sign's face and on the fascia over the mouth, just proud of each, facing out (+Z). */
function letteringGeometry(): Geometry {
  const quad = (w: number, h: number, y: number, z: number) => {
    const [hx, hy] = [w / 2, h / 2]
    const positions = new Float32Array([-hx, y - hy, z, hx, y - hy, z, hx, y + hy, z, -hx, y + hy, z])
    const normals = new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1])
    const uvs = new Float32Array([0, 1, 1, 1, 1, 0, 0, 0])
    return new Geometry({ positions, normals, uvs, indices: new Uint16Array([0, 1, 2, 0, 2, 3]) })
  }
  const [sw, sh, sy] = SIGN
  return Geometry.merge([quad(sw - 0.4, sh - 0.2, sy, ROOF_FRONT - 0.29), quad(3.2, ROOF * 0.75, ROOF_UNDER + ROOF / 2, ROOF_FRONT + 0.05)])
}

/** The roller door: ribbed sections across the doorway, its top at the origin and hanging down, in the XY plane. */
function doorGeometry(): Geometry {
  const parts: Geometry[] = []
  const sections = 10
  const tall = DOOR_TALL / sections
  for (let i = 0; i < sections; i++) {
    parts.push(block(DOOR_WIDTH, tall - 0.04, 0.06, 0, -(i + 0.5) * tall, 0, i === sections - 1 ? BLUE : DOOR))
    parts.push(block(DOOR_WIDTH, 0.05, 0.09, 0, -(i + 1) * tall + 0.02, 0, RIB))
  }
  return Geometry.merge(parts)
}

/** The world's vehicle dealers, drawn. */
export class Dealers {
  readonly root = new Group()
  /** Each carport's mouth, metres: what the map marks. */
  readonly points: Point2[]
  private readonly meshes: InstancedMesh[] = []
  private readonly body: InstancedMesh | null = null
  private readonly lamps: InstancedMesh | null = null
  private readonly lettering: InstancedMesh | null = null
  private readonly doors: InstancedMesh | null = null
  private readonly lampMaterial: Material | null = null
  private readonly letterMaterial: Material | null = null
  /** How far each door is up (0 shut, 1 up), and how long until it comes down again, seconds. */
  private readonly door: { up: number; hold: number }[]
  private readonly matrix = mat4.create()
  /** The night the materials were last lit for, so they are only touched when it changes. */
  private litFor = -1

  constructor(readonly dealers: readonly Dealer[]) {
    this.points = dealers.map((dealer) => [dealer.x, dealer.z])
    this.door = dealers.map(() => ({ up: 0, hold: 0 }))
    if (dealers.length === 0) return
    const n = dealers.length
    const add = (mesh: InstancedMesh) => {
      mesh.count = 0
      this.meshes.push(mesh)
      this.root.add(mesh)
      return mesh
    }
    this.body = add(new InstancedMesh(carportGeometry(), toonMaterial({ vertexColors: true, snow: winter }), n))
    this.lampMaterial = toonMaterial({ vertexColors: true })
    this.lamps = add(new InstancedMesh(lampGeometry(), this.lampMaterial, n))
    this.letterMaterial = new Material({ map: letteringTexture(), blend: 'alpha', depthWrite: false })
    this.lettering = add(new InstancedMesh(letteringGeometry(), this.letterMaterial, n))
    this.doors = add(new InstancedMesh(doorGeometry(), toonMaterial({ vertexColors: true }), n))
  }

  /** A vehicle was bought at dealer `index` (`Ordered`), or sold to it (`Sold`): its door rolls up to let it through. */
  ordered(index: number) {
    const door = this.door[index]
    if (door) door.hold = DOOR_HOLD
  }

  /**
   * Places every carport nearer the eye than `fog` metres (where the distance fog is whole), lights
   * it by `night` (0 by day, 1 at night) and works its door, which rolls up while one of `movers` (the
   * vehicles, metres) is under the roof. `dt` is the seconds since the last frame.
   */
  update(eye: { x: number; z: number }, fog: number, night: number, dt: number, movers: Iterable<{ x: number; z: number }> = []) {
    if (!this.body) return
    const reach = Math.min(DEALER_REACH, fog)
    for (const mover of movers) {
      for (const [index, dealer] of this.dealers.entries()) {
        const dx = mover.x - dealer.x
        const dz = mover.z - dealer.z
        if (Math.abs(dx) > 20 || Math.abs(dz) > 20) continue
        // The carport's +X and +Z in the world, as `shared::station::Frame` has them.
        const [cos, sin] = [Math.cos(dealer.yaw), Math.sin(dealer.yaw)]
        const across = dx * cos - dz * sin
        const out = dx * sin + dz * cos
        if (Math.abs(across) < WIDTH / 2 && out < 0 && out > -DEPTH) this.door[index]!.hold = Math.max(this.door[index]!.hold, DOOR_NEAR)
      }
    }
    let placed = 0
    for (const [index, dealer] of this.dealers.entries()) {
      const door = this.door[index]!
      door.hold = Math.max(0, door.hold - dt)
      door.up = Math.min(1, Math.max(0, door.up + ((door.hold > 0 ? 1 : -1) * dt) / DOOR_TRAVEL))
      if (Math.hypot(dealer.x - eye.x, dealer.z - eye.z) > reach) continue
      const half = dealer.yaw / 2
      const [qy, qw] = [Math.sin(half), Math.cos(half)]
      mat4.compose(this.matrix, dealer.x, dealer.base, dealer.z, 0, qy, 0, qw, 1, 1, 1)
      for (const mesh of [this.body, this.lamps!, this.lettering!]) mesh.setMatrixAt(placed, this.matrix)
      // The door hangs from the top of its doorway, rolled up towards it.
      const t = door.up * door.up * (3 - 2 * door.up)
      const [cos, sin] = [Math.cos(dealer.yaw), Math.sin(dealer.yaw)]
      mat4.compose(this.matrix, dealer.x + DOOR_Z * sin, dealer.base + FLOOR + DOOR_TALL, dealer.z + DOOR_Z * cos, 0, qy, 0, qw, 1, 1 - (1 - ROLLED) * t, 1)
      this.doors!.setMatrixAt(placed, this.matrix)
      placed++
    }
    this.body.count = this.lamps!.count = this.lettering!.count = this.doors!.count = placed
    for (const mesh of this.meshes) mesh.needsUpdate()
    // By night the lamps are lit warm and the letters white.
    if (Math.abs(night - this.litFor) < 0.01) return
    this.litFor = night
    const glow = night * 1.6
    this.lampMaterial!.emissive.set(glow, glow * 0.92, glow * 0.75)
    this.lampMaterial!.needsUpdate()
    this.letterMaterial!.emissive.set(glow, glow, glow)
    this.letterMaterial!.needsUpdate()
  }
}
