import { Geometry, Group, InstancedMesh, boxGeometry, cylinderGeometry, mat4 } from '@ragnarok/engine'
import { BUS_STOP_SETBACK } from '@/sim/consts'
import { busLineAt, type BusLine } from '@/world'
import { painted } from './lines'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * The bus stops of a world with public transport (`WorldData.busLines`, BUSES.md): at every stop of
 * every line a pole with a sign stands on each side of the road, `BUS_STOP_SETBACK` from its middle,
 * where a bus going either way halts. They are world data, like the ski lifts: nothing is replicated
 * and nothing collides with them. All the signs are one instanced mesh, placed every frame from the
 * ground of the tiles that are loaded, so a sign appears with the ground it stands on.
 */

const POLE = 0x6f777c
const SIGN = 0x1f5fb4
const BAND = 0xf2c230
/** How high the pole stands, metres, and the side of its square sign. */
const POLE_HEIGHT = 2.7
const SIGN_SIDE = 0.6
/** Signs further than this from the eye are not drawn, metres. */
const SIGN_REACH = 600

/** One sign, its pole's foot at the origin, its face across the X axis so it is read from along Z. */
function signGeometry(): Geometry {
  return Geometry.merge([
    painted(cylinderGeometry(0.04, 0.04, POLE_HEIGHT, 6).translate(0, POLE_HEIGHT / 2, 0), POLE),
    painted(boxGeometry(SIGN_SIDE, SIGN_SIDE, 0.05).translate(0, POLE_HEIGHT - SIGN_SIDE / 2, 0), SIGN),
    // A yellow band across the foot of the sign, proud of it on both faces.
    painted(boxGeometry(SIGN_SIDE, 0.14, 0.07).translate(0, POLE_HEIGHT - SIGN_SIDE + 0.07, 0), BAND),
  ])
}

/** Where a sign stands, metres, and how it is turned. */
interface Sign {
  x: number
  z: number
  yaw: number
}

/** Where the signs of `lines` stand: two a stop, one on each side of the road, facing along it. */
export function busStopSigns(lines: readonly BusLine[]): Sign[] {
  const signs: Sign[] = []
  for (const line of lines) {
    for (const stop of line.stops) {
      const { at, way } = busLineAt(line, stop)
      // With the way ahead along -Z the right is +X: a quarter turn clockwise from the way.
      const right = [-way[1], way[0]]
      const yaw = Math.atan2(way[0], way[1])
      for (const side of [1, -1]) signs.push({ x: at[0] + right[0]! * BUS_STOP_SETBACK * side, z: at[1] + right[1]! * BUS_STOP_SETBACK * side, yaw })
    }
  }
  return signs
}

/** The world's bus stop signs, drawn. */
export class BusStops {
  readonly root = new Group()
  private readonly signs: Sign[]
  private readonly mesh: InstancedMesh | null = null
  private readonly matrix = mat4.create()

  constructor(lines: readonly BusLine[]) {
    this.signs = busStopSigns(lines)
    if (this.signs.length === 0) return
    this.mesh = new InstancedMesh(signGeometry(), toonMaterial({ vertexColors: true, snow: winter }), this.signs.length)
    this.mesh.count = 0
    this.root.add(this.mesh)
  }

  /**
   * Places every sign nearer the eye than `fog` metres (where the distance fog is whole) whose ground
   * is loaded: `ground` is its height, or null where the tile is not there yet.
   */
  update(eye: { x: number; z: number }, fog: number, ground: (x: number, z: number) => number | null) {
    const mesh = this.mesh
    if (!mesh) return
    let count = 0
    for (const sign of this.signs) {
      if (Math.hypot(sign.x - eye.x, sign.z - eye.z) > Math.min(SIGN_REACH, fog)) continue
      const y = ground(sign.x, sign.z)
      if (y === null) continue
      const half = sign.yaw / 2
      mat4.compose(this.matrix, sign.x, y, sign.z, 0, Math.sin(half), 0, Math.cos(half), 1, 1, 1)
      mesh.setMatrixAt(count++, this.matrix)
    }
    mesh.count = count
    mesh.needsUpdate()
  }
}
