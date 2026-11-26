import { Geometry, Group, InstancedMesh, Material, Texture, boxGeometry, cylinderGeometry, mat4, sphereGeometry } from '@ragnarok/engine'
import {
  STATION_CANOPY_AT,
  STATION_CANOPY_DEPTH,
  STATION_CANOPY_HEIGHT,
  STATION_CANOPY_SIZE,
  STATION_SIGN_AT,
  STATION_SIGN_HEIGHT,
  STATION_SIGN_RADIUS,
  WASH_BAY_HEIGHT,
  WASH_BAY_WIDTH,
  WASH_BAY_X,
  WASH_BAY_Z,
  WASH_LEAD,
} from '@/sim/consts'
import { ClientSim } from '@/sim/wasm/sim'
import type { FuelStation, Point2 } from '@/world'
import { painted } from './lines'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * The world's fuel stations (`WorldData.fuelStations`, FUEL.md), drawn as a 1960s roadside station:
 * a tall round sign on the corner by the road, orange inside a navy ring, a white canopy with a navy
 * fascia and the name on it over a pump island, and behind it a drive-through wash with roll-up doors
 * at both ends and two tall brushes inside.
 *
 * What is solid comes from the physics' own parts (`shared::station::structure`, through the WASM's
 * `stationParts`), so what is seen is what is hit; the rest is painted on and not solid. Every station
 * has the same layout in its own frame, so each piece is one instanced mesh for all of them, placed by
 * the station's middle, turn and height. The doors roll up while any vehicle is in the wash's lane
 * (the wash drives it through, on the server and in the prediction alike), and the brushes spin while
 * one is between them. The sign and the canopy's lamps light up by night.
 */

const WHITE = 0xf2efe6
const NAVY = 0x1d2f6b
const ORANGE = 0xf47b20
const CONCRETE = 0xbdb9ae
const STEEL = 0x8a9196
const GLASS = 0x2a3b4c
const RUBBER = 0x1f2326
const BRUSH = 0x2f6fd0
const BRISTLE = 0xcfe2ff
/** The colour of each `PAINT_*` of `shared::station`, by its number. */
const PAINTS = [WHITE, NAVY, ORANGE, CONCRETE, STEEL]

/** The name on the signs: a station chain of the game's own. */
const NAME = 'NORD'
/** Stations further than this from the eye are not drawn, metres. */
const STATION_REACH = 900
/** How long a wash door takes to roll up or down, seconds. */
const DOOR_TRAVEL = 1.6
/** How long the doors stay up after the lane has emptied, seconds. */
const DOOR_HOLD = 2
/** How much of a door is left showing when it is rolled up, a share of its height. */
const ROLLED = 0.08
/** How far either side of the lane's middle a vehicle opens the doors, metres. */
const DOOR_ASIDE = 3.5
/** How fast the brushes spin while a vehicle is between them, radians a second. */
const BRUSH_SPIN = 9
/** The brushes' radius, and how far either side of the lane's middle each stands, metres. */
const BRUSH_RADIUS = 0.4
const BRUSH_ASIDE = 1.75
/** How far along the bay from its middle the two brushes stand, metres: one a little ahead of the other. */
const BRUSH_ALONG = [-1.5, 1.5] as const

/** The floor of a station, metres over its base: the top of its paving. */
const FLOOR = 0.05
const BAY_MIDDLE = (WASH_BAY_X[0] + WASH_BAY_X[1]) / 2
const BAY_HALF = (WASH_BAY_X[1] - WASH_BAY_X[0]) / 2
/** The navy band along the wash bay's walls and over its doors: its middle over the base, and how tall it is, metres. */
const BAND_Y = FLOOR + WASH_BAY_HEIGHT + 0.06
const BAND_TALL = 0.5

/** A box `w × h × d` with its middle at `(x, y, z)` in a station's frame, in `color`. */
function block(w: number, h: number, d: number, x: number, y: number, z: number, color: number): Geometry {
  return painted(boxGeometry(w, h, d).translate(x, y, z), color)
}

/** The solid parts of a station at the origin, unturned, its base at 0, as the physics stands them. */
function solidParts(): Geometry[] {
  const flat = ClientSim.stationParts(0, 0, 0, 0)
  const parts: Geometry[] = []
  for (let i = 0; i + 9 <= flat.length; i += 9) {
    const [shape, x, y, z, , a, b, c, look] = flat.slice(i, i + 9) as unknown as number[]
    const geometry = shape === 0 ? boxGeometry(a! * 2, b! * 2, c! * 2) : cylinderGeometry(a!, a!, b! * 2, 12)
    parts.push(painted(geometry.translate(x!, y!, z!), PAINTS[look!] ?? WHITE))
  }
  return parts
}

/** The canopy's fascia: how far up its roof it starts, and how deep it is, metres. */
const FASCIA_Y = FLOOR + STATION_CANOPY_HEIGHT + 0.12
const FASCIA_TALL = STATION_CANOPY_DEPTH - 0.18

/**
 * A navy band round a box `w × d` (in plan, its middle at `(x, z)`) from `y` up `h`, with an orange
 * stripe along its foot, leaving the band over the stripe for lettering.
 */
function fascia(w: number, d: number, x: number, z: number, y: number, h: number): Geometry[] {
  return [block(w + 0.08, h, d + 0.08, x, y + h / 2, z, NAVY), block(w + 0.12, h * 0.2, d + 0.12, x, y + h * 0.2, z, ORANGE)]
}

/** Everything of a station that is painted on, not solid, and not lit or moving: one geometry in its frame. */
function stationGeometry(): Geometry {
  const parts = solidParts()
  const [cx, cz] = STATION_CANOPY_AT
  const [cw, cd] = STATION_CANOPY_SIZE
  // The canopy's deep fascia, all round its roof.
  parts.push(...fascia(cw, cd, cx, cz, FASCIA_Y, FASCIA_TALL))
  // The pumps: a white face either side of each orange cabinet, a dark hose and a white globe on top.
  for (const x of [-2.5, 2.5]) {
    for (const side of [-1, 1]) {
      parts.push(block(0.5, 0.75, 0.02, cx + x, FLOOR + 0.18 + 1.0, cz + side * 0.31, WHITE))
      parts.push(block(0.32, 0.18, 0.025, cx + x, FLOOR + 0.18 + 1.25, cz + side * 0.315, RUBBER))
      parts.push(block(0.05, 0.8, 0.05, cx + x + 0.4, FLOOR + 0.18 + 0.75, cz + side * 0.22, RUBBER))
    }
    parts.push(painted(sphereGeometry(0.22, 12, 8).translate(cx + x, FLOOR + 0.18 + 1.85, cz), WHITE))
  }
  // The wash bay: a navy band along each outer wall and over each door, and the rollers' boxes inside.
  for (const side of [-1, 1]) {
    const z = WASH_BAY_Z + side * (WASH_BAY_WIDTH / 2 + 0.3)
    parts.push(block(BAY_HALF * 2 + 0.02, BAND_TALL, 0.04, BAY_MIDDLE, BAND_Y, z + side * 0.01, NAVY))
    parts.push(block(BAY_HALF * 2 + 0.03, 0.09, 0.05, BAY_MIDDLE, BAND_Y, z + side * 0.015, ORANGE))
  }
  for (const x of WASH_BAY_X) {
    parts.push(block(0.06, BAND_TALL, WASH_BAY_WIDTH + 0.6, x + Math.sign(x - BAY_MIDDLE) * 0.03, BAND_Y, WASH_BAY_Z, NAVY))
    // The drum each door rolls up onto, under the roof just inside the door.
    parts.push(painted(cylinderGeometry(0.25, 0.25, WASH_BAY_WIDTH, 12).rotateX(Math.PI / 2).translate(x - Math.sign(x - BAY_MIDDLE) * 0.35, FLOOR + WASH_BAY_HEIGHT - 0.25, WASH_BAY_Z), STEEL))
  }
  // The brushes' gantry: a beam across the bay over each pair of brushes.
  for (const along of BRUSH_ALONG) {
    parts.push(block(0.15, 0.15, WASH_BAY_WIDTH, BAY_MIDDLE + along, FLOOR + WASH_BAY_HEIGHT - 0.3, WASH_BAY_Z, STEEL))
  }
  // A kerb of navy paint along the lane each way out of the bay, so the way in reads from the forecourt.
  for (const x of WASH_BAY_X) {
    const out = Math.sign(x - BAY_MIDDLE)
    for (const side of [-1, 1]) parts.push(block(WASH_LEAD, 0.012, 0.15, x + (out * WASH_LEAD) / 2, FLOOR + 0.006, WASH_BAY_Z + side * (WASH_BAY_WIDTH / 2 - 0.2), NAVY))
  }
  // The sign: a navy ring round an orange disc on top of the post, facing the road both ways; a price board under it.
  const [sx, sz] = STATION_SIGN_AT
  const sy = FLOOR + STATION_SIGN_HEIGHT
  parts.push(painted(cylinderGeometry(STATION_SIGN_RADIUS + 0.18, STATION_SIGN_RADIUS + 0.18, 0.28, 40).rotateX(Math.PI / 2).translate(sx, sy, sz), NAVY))
  parts.push(block(2.2, 1.0, 0.16, sx, sy - STATION_SIGN_RADIUS - 0.9, sz, WHITE))
  parts.push(block(2.26, 0.12, 0.2, sx, sy - STATION_SIGN_RADIUS - 0.4, sz, NAVY))
  parts.push(block(2.26, 0.08, 0.2, sx, sy - STATION_SIGN_RADIUS - 1.4, sz, NAVY))
  for (const [i, x] of [-0.65, 0, 0.65].entries()) {
    parts.push(block(0.42, 0.42, 0.2, sx + x, sy - STATION_SIGN_RADIUS - 0.92, sz, i === 1 ? ORANGE : NAVY))
  }
  return Geometry.merge(parts)
}

/** What of a station lights up by night: the canopy's lamps. */
function lampGeometry(): Geometry {
  const [cx, cz] = STATION_CANOPY_AT
  const [cw, cd] = STATION_CANOPY_SIZE
  const parts: Geometry[] = []
  for (const x of [-0.36, -0.12, 0.12, 0.36]) {
    for (const z of [-0.3, 0.3]) parts.push(block(1.0, 0.04, 0.35, cx + x * cw, FLOOR + STATION_CANOPY_HEIGHT - 0.02, cz + z * cd, 0xfff6dc))
  }
  return Geometry.merge(parts)
}

/** The orange disc of the sign, which glows by night. */
function discGeometry(): Geometry {
  const [sx, sz] = STATION_SIGN_AT
  return painted(cylinderGeometry(STATION_SIGN_RADIUS, STATION_SIGN_RADIUS, 0.34, 40).rotateX(Math.PI / 2).translate(sx, FLOOR + STATION_SIGN_HEIGHT, sz), ORANGE)
}

/** Where on the lettering texture each word is: the top half `NAME`, the bottom half "WASH". */
const LETTERING = { name: [0, 0.5], wash: [0.5, 1] } as const

/** The lettering, white on nothing: the name over the wash's word, each in its half of the texture. */
function letteringTexture(): Texture {
  const canvas = document.createElement('canvas')
  canvas.width = 512
  canvas.height = 256
  const g = canvas.getContext('2d')
  if (g) {
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.fillStyle = '#ffffff'
    g.font = '900 104px "Arial Black", system-ui, sans-serif'
    g.fillText(NAME, 256, 68)
    g.font = '800 92px system-ui, sans-serif'
    g.fillText('WASH', 256, 196)
  }
  return new Texture(canvas, { srgb: true, wrap: 'clamp' })
}

/**
 * A quad `w × h` with its middle at `(x, y, z)` facing `face` (+1 along +Z, -1 along -Z, ±2 along ±X),
 * showing the rows `[top, bottom]` of the lettering texture.
 */
function letters(w: number, h: number, x: number, y: number, z: number, face: number, [top, bottom]: readonly [number, number]): Geometry {
  const [hx, hy] = [w / 2, h / 2]
  // Corners in the quad's own plane, left to right as seen from in front of it.
  const corners = [[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]] as const
  const positions = new Float32Array(12)
  const normals = new Float32Array(12)
  const normal = face === 1 ? [0, 0, 1] : face === -1 ? [0, 0, -1] : face === 2 ? [1, 0, 0] : [-1, 0, 0]
  for (const [k, [u, v]] of corners.entries()) {
    const across = face === 1 ? [u, 0] : face === -1 ? [-u, 0] : face === 2 ? [0, -u] : [0, u]
    positions.set([x + across[0]!, y + v, z + across[1]!], k * 3)
    normals.set(normal, k * 3)
  }
  const uvs = new Float32Array([0, bottom, 1, bottom, 1, top, 0, top])
  return new Geometry({ positions, normals, uvs, indices: new Uint16Array([0, 1, 2, 0, 2, 3]) })
}

/** All the lettering of a station: the name both ways on the sign and on the canopy's fascia, and "WASH" over each of the wash's doors. */
function letteringGeometry(): Geometry {
  const [sx, sz] = STATION_SIGN_AT
  const sy = FLOOR + STATION_SIGN_HEIGHT
  const [cx, cz] = STATION_CANOPY_AT
  const [, cd] = STATION_CANOPY_SIZE
  // Over the fascia's stripe, just proud of its face.
  const fy = FASCIA_Y + FASCIA_TALL * 0.64
  const fz = cd / 2 + 0.08
  const parts = [
    letters(2.6, 1.3, sx, sy, sz - 0.18, -1, LETTERING.name),
    letters(2.6, 1.3, sx, sy, sz + 0.18, 1, LETTERING.name),
    letters(1.9, FASCIA_TALL * 0.62, cx, fy, cz - fz, -1, LETTERING.name),
    letters(1.9, FASCIA_TALL * 0.62, cx, fy, cz + fz, 1, LETTERING.name),
  ]
  for (const x of WASH_BAY_X) {
    const out = Math.sign(x - BAY_MIDDLE)
    parts.push(letters(2.2, 0.5, x + out * 0.08, BAND_Y, WASH_BAY_Z, out * 2, LETTERING.wash))
  }
  return Geometry.merge(parts)
}

/** A wash door: ribbed sections across the doorway, its top at the origin and hanging down, in the YZ plane. */
function doorGeometry(): Geometry {
  const parts: Geometry[] = []
  const sections = 8
  const tall = WASH_BAY_HEIGHT / sections
  for (let i = 0; i < sections; i++) {
    parts.push(block(0.06, tall - 0.04, WASH_BAY_WIDTH, 0, -(i + 0.5) * tall, 0, i === sections - 1 ? NAVY : 0xe4e1d8))
    parts.push(block(0.09, 0.05, WASH_BAY_WIDTH, 0, -(i + 1) * tall + 0.02, 0, 0xc9c6bc))
  }
  // A band of windows across the middle section.
  parts.push(block(0.08, tall * 0.5, WASH_BAY_WIDTH - 0.6, 0, -2.5 * tall, 0, GLASS))
  return Geometry.merge(parts)
}

/** A brush: a tall blue roller with stripes of pale bristles down it, so it is seen to spin, about its own axis. */
function brushGeometry(): Geometry {
  const tall = WASH_BAY_HEIGHT - 0.6
  const parts = [painted(cylinderGeometry(BRUSH_RADIUS, BRUSH_RADIUS, tall, 14).translate(0, tall / 2 + 0.15, 0), BRUSH), block(0.06, 0.4, 0.06, 0, tall + 0.35, 0, STEEL)]
  for (let k = 0; k < 4; k++) {
    const angle = (k / 4) * Math.PI * 2
    parts.push(painted(boxGeometry(0.1, tall - 0.1, 0.1).translate(Math.sin(angle) * BRUSH_RADIUS, tall / 2 + 0.15, Math.cos(angle) * BRUSH_RADIUS), BRISTLE))
  }
  return Geometry.merge(parts)
}

/** One station's moving parts: how far its doors are up (0 shut, 1 up), how long they have been free, and its brushes' turn. */
interface Wash {
  doors: number
  idle: number
  spin: number
  /** Whether a vehicle is between the doors. */
  busy: boolean
  /** Whether a vehicle is in the lane, in or out of the bay. */
  near: boolean
}

/** The world's fuel stations, drawn. */
export class FuelStations {
  readonly root = new Group()
  /** Each station's middle, metres: what the HUD's fuel gauge and map read. */
  readonly points: Point2[]
  private readonly meshes: InstancedMesh[] = []
  private readonly body: InstancedMesh | null = null
  private readonly lamps: InstancedMesh | null = null
  private readonly disc: InstancedMesh | null = null
  private readonly lettering: InstancedMesh | null = null
  private readonly doors: InstancedMesh | null = null
  private readonly brushes: InstancedMesh | null = null
  private readonly lampMaterial: Material | null = null
  private readonly discMaterial: Material | null = null
  private readonly letterMaterial: Material | null = null
  private readonly washes: Wash[]
  private readonly matrix = mat4.create()
  /** The night the materials were last lit for, so they are only touched when it changes. */
  private litFor = -1

  constructor(readonly stations: readonly FuelStation[]) {
    this.points = stations.map((station) => [station.x, station.z])
    this.washes = stations.map(() => ({ doors: 0, idle: DOOR_HOLD, spin: 0, busy: false, near: false }))
    if (stations.length === 0) return
    const n = stations.length
    const add = (mesh: InstancedMesh) => {
      mesh.count = 0
      this.meshes.push(mesh)
      this.root.add(mesh)
      return mesh
    }
    this.body = add(new InstancedMesh(stationGeometry(), toonMaterial({ vertexColors: true, snow: winter }), n))
    this.lampMaterial = toonMaterial({ vertexColors: true })
    this.lamps = add(new InstancedMesh(lampGeometry(), this.lampMaterial, n))
    this.discMaterial = toonMaterial({ vertexColors: true })
    this.disc = add(new InstancedMesh(discGeometry(), this.discMaterial, n))
    this.letterMaterial = new Material({ map: letteringTexture(), blend: 'alpha', depthWrite: false })
    this.lettering = add(new InstancedMesh(letteringGeometry(), this.letterMaterial, n))
    this.doors = add(new InstancedMesh(doorGeometry(), toonMaterial({ vertexColors: true }), n * WASH_BAY_X.length))
    this.brushes = add(new InstancedMesh(brushGeometry(), toonMaterial({ vertexColors: true }), n * BRUSH_ALONG.length * 2))
  }

  /**
   * Places every station nearer the eye than `fog` metres (where the distance fog is whole), lights it
   * by `night` (0 by day, 1 at night), and works its wash: the doors roll up while one of `movers`
   * (the vehicles, metres) is in its lane, and the brushes spin while one is between them. `dt` is
   * the seconds since the last frame.
   */
  update(eye: { x: number; z: number }, fog: number, night: number, dt: number, movers: Iterable<{ x: number; z: number }>) {
    if (!this.body) return
    const reach = Math.min(STATION_REACH, fog)
    for (const wash of this.washes) wash.busy = wash.near = false
    for (const mover of movers) {
      for (const [index, station] of this.stations.entries()) {
        const dx = mover.x - station.x
        const dz = mover.z - station.z
        if (Math.abs(dx) > 40 || Math.abs(dz) > 40) continue
        // The station's +X and +Z in the world, as `shared::station::Frame` has them.
        const [cos, sin] = [Math.cos(station.yaw), Math.sin(station.yaw)]
        const along = dx * cos - dz * sin - BAY_MIDDLE
        const aside = dx * sin + dz * cos - WASH_BAY_Z
        if (Math.abs(aside) > DOOR_ASIDE || Math.abs(along) > BAY_HALF + WASH_LEAD + 2) continue
        const wash = this.washes[index]!
        wash.near = true
        if (Math.abs(along) < BAY_HALF && Math.abs(aside) < WASH_BAY_WIDTH / 2) wash.busy = true
      }
    }
    let placed = 0
    let doors = 0
    let brushes = 0
    for (const [index, station] of this.stations.entries()) {
      const wash = this.washes[index]!
      wash.idle = wash.near ? 0 : wash.idle + dt
      const up = wash.idle < DOOR_HOLD
      wash.doors = Math.min(1, Math.max(0, wash.doors + ((up ? 1 : -1) * dt) / DOOR_TRAVEL))
      if (wash.busy) wash.spin = (wash.spin + BRUSH_SPIN * dt) % (Math.PI * 2)
      if (Math.hypot(station.x - eye.x, station.z - eye.z) > reach) continue
      const half = station.yaw / 2
      const [qy, qw] = [Math.sin(half), Math.cos(half)]
      mat4.compose(this.matrix, station.x, station.base, station.z, 0, qy, 0, qw, 1, 1, 1)
      for (const mesh of [this.body, this.lamps!, this.disc!, this.lettering!]) mesh.setMatrixAt(placed, this.matrix)
      placed++
      const [cos, sin] = [Math.cos(station.yaw), Math.sin(station.yaw)]
      const world = (x: number, z: number): Point2 => [station.x + x * cos + z * sin, station.z - x * sin + z * cos]
      // Each door hangs from the top of its doorway, rolled up towards it.
      const t = wash.doors * wash.doors * (3 - 2 * wash.doors)
      for (const x of WASH_BAY_X) {
        const [wx, wz] = world(x - Math.sign(x - BAY_MIDDLE) * 0.1, WASH_BAY_Z)
        mat4.compose(this.matrix, wx, station.base + FLOOR + WASH_BAY_HEIGHT, wz, 0, qy, 0, qw, 1, 1 - (1 - ROLLED) * t, 1)
        this.doors!.setMatrixAt(doors++, this.matrix)
      }
      // The brushes: a pair at each place along the bay, spinning opposite ways.
      for (const along of BRUSH_ALONG) {
        for (const side of [-1, 1]) {
          const [wx, wz] = world(BAY_MIDDLE + along, WASH_BAY_Z + side * BRUSH_ASIDE)
          const turn = (station.yaw + side * wash.spin) / 2
          mat4.compose(this.matrix, wx, station.base + FLOOR, wz, 0, Math.sin(turn), 0, Math.cos(turn), 1, 1, 1)
          this.brushes!.setMatrixAt(brushes++, this.matrix)
        }
      }
    }
    this.body.count = this.lamps!.count = this.disc!.count = this.lettering!.count = placed
    this.doors!.count = doors
    this.brushes!.count = brushes
    for (const mesh of this.meshes) mesh.needsUpdate()
    // By night the lamps are lit warm, the disc glows orange and the letters white.
    if (Math.abs(night - this.litFor) < 0.01) return
    this.litFor = night
    const glow = night * 1.6
    this.lampMaterial!.emissive.set(glow, glow * 0.92, glow * 0.75)
    this.lampMaterial!.needsUpdate()
    this.discMaterial!.emissive.set(night * 0.7, night * 0.32, night * 0.07)
    this.discMaterial!.needsUpdate()
    this.letterMaterial!.emissive.set(glow, glow, glow)
    this.letterMaterial!.needsUpdate()
  }
}
