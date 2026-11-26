import { Geometry, Group, InstancedMesh, boxGeometry, cylinderGeometry, mat4 } from '@ragnarok/engine'
import type { Point2 } from '@/world'
import { painted } from './lines'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * The fuel stations of a world (`WorldData.fuelStations`, FUEL.md): at each one a pump stands beside
 * a tall sign, to be seen from down the road. A vehicle near it has its tank filled, which is the
 * server's doing: these are world data, like the bus stops, so nothing is replicated and nothing
 * collides with them. All of them are one instanced mesh, placed every frame from the ground of the
 * tiles that are loaded, so a station appears with the ground it stands on.
 */

const POLE = 0x6f777c
const SIGN = 0xd8342b
const BAND = 0xf4f1e6
const PUMP = 0xe9e6dc
const HOSE = 0x1f2326
/** How high the sign's pole stands, metres, and how wide and tall the sign on it is. */
const POLE_HEIGHT = 7
const SIGN_WIDTH = 1.8
const SIGN_HEIGHT = 1.4
/** How far to the side of the sign's pole the pump stands, metres. */
const PUMP_ASIDE = 1.6
/** Stations further than this from the eye are not drawn, metres. */
const STATION_REACH = 900

/** One station, the foot of its sign's pole at the origin. */
function stationGeometry(): Geometry {
  return Geometry.merge([
    painted(cylinderGeometry(0.09, 0.09, POLE_HEIGHT, 8).translate(0, POLE_HEIGHT / 2, 0), POLE),
    painted(boxGeometry(SIGN_WIDTH, SIGN_HEIGHT, 0.16).translate(0, POLE_HEIGHT - SIGN_HEIGHT / 2, 0), SIGN),
    // A pale band across the sign, proud of it on both faces.
    painted(boxGeometry(SIGN_WIDTH, 0.3, 0.2).translate(0, POLE_HEIGHT - SIGN_HEIGHT / 2, 0), BAND),
    // The pump on its plinth: a pale cabinet under a red top, with its hose down one side.
    painted(boxGeometry(1.1, 0.15, 0.7).translate(PUMP_ASIDE, 0.075, 0), POLE),
    painted(boxGeometry(0.7, 1.35, 0.45).translate(PUMP_ASIDE, 0.825, 0), PUMP),
    painted(boxGeometry(0.74, 0.3, 0.49).translate(PUMP_ASIDE, 1.65, 0), SIGN),
    painted(boxGeometry(0.06, 0.9, 0.06).translate(PUMP_ASIDE + 0.4, 0.95, 0), HOSE),
  ])
}

/** The world's fuel stations, drawn. */
export class FuelStations {
  readonly root = new Group()
  private readonly mesh: InstancedMesh | null = null
  private readonly matrix = mat4.create()

  constructor(readonly stations: readonly Point2[]) {
    if (stations.length === 0) return
    this.mesh = new InstancedMesh(stationGeometry(), toonMaterial({ vertexColors: true, snow: winter }), stations.length)
    this.mesh.count = 0
    this.root.add(this.mesh)
  }

  /**
   * Places every station nearer the eye than `fog` metres (where the distance fog is whole) whose
   * ground is loaded: `ground` is its height, or null where the tile is not there yet.
   */
  update(eye: { x: number; z: number }, fog: number, ground: (x: number, z: number) => number | null) {
    const mesh = this.mesh
    if (!mesh) return
    let count = 0
    for (const [x, z] of this.stations) {
      if (Math.hypot(x - eye.x, z - eye.z) > Math.min(STATION_REACH, fog)) continue
      const y = ground(x, z)
      if (y === null) continue
      mat4.compose(this.matrix, x, y, z, 0, 0, 0, 1, 1, 1, 1)
      mesh.setMatrixAt(count++, this.matrix)
    }
    mesh.count = count
    mesh.needsUpdate()
  }
}
