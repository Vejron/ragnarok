import { Geometry, Group, Mesh, cylinderGeometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/** The open canopy: a dome this wide and tall, held this far above the feet, metres. */
const RADIUS = 1.7
const HEIGHT = 0.95
const LIFT = 3.3
/** Where the lines meet the body: shoulder height above the feet, and how far out they spread. */
const SHOULDER = 1.45
const LINES = 6
/** The canopy fills and empties at this rate, 1/s. */
const OPEN_RATE = 4

/** A half dome, outside facing out and inside facing in so it reads from below. */
function domeGeometry(radius: number, height: number, around = 16, rings = 5): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  for (const side of [1, -1]) {
    const base = positions.length / 3
    for (let ring = 0; ring <= rings; ring++) {
      const phi = (ring / rings) * (Math.PI / 2)
      for (let i = 0; i <= around; i++) {
        const theta = (i / around) * Math.PI * 2
        const nx = Math.cos(theta) * Math.sin(phi)
        const ny = Math.cos(phi)
        const nz = Math.sin(theta) * Math.sin(phi)
        positions.push(nx * radius, ny * height, nz * radius)
        normals.push(nx * side, ny * side, nz * side)
        uvs.push(i / around, ring / rings)
      }
    }
    const row = around + 1
    for (let ring = 0; ring < rings; ring++) {
      for (let i = 0; i < around; i++) {
        const a = base + ring * row + i
        const b = a + 1
        const c = a + row
        const d = c + 1
        if (side === 1) indices.push(a, c, b, b, c, d)
        else indices.push(a, b, c, b, d, c)
      }
    }
  }
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), uvs: new Float32Array(uvs), indices: new Uint32Array(indices) })
}

// Shared by every canopy: parts only ever move.
const DOME = domeGeometry(RADIUS, HEIGHT)
const LINE = cylinderGeometry(0.012, 0.012, 1, 4)

/** A parachute above a soldier's head. Its root is placed at the feet; it grows open and folds away. */
export class Canopy {
  readonly root = new Group()
  private readonly cloth: ReturnType<typeof toonMaterial>
  private readonly cord = toonMaterial({ color: 0xdddddd })
  private fill = 0

  constructor(color: number) {
    this.cloth = toonMaterial({ color })
    const dome = new Mesh(DOME, this.cloth)
    dome.position.set(0, LIFT, 0)
    dome.castShadow = true
    this.root.add(dome)
    for (let i = 0; i < LINES; i++) {
      const angle = (i / LINES) * Math.PI * 2
      const top: [number, number, number] = [Math.cos(angle) * RADIUS * 0.96, LIFT + 0.05, Math.sin(angle) * RADIUS * 0.96]
      const bottom: [number, number, number] = [Math.cos(angle) * 0.18, SHOULDER, Math.sin(angle) * 0.18]
      const along: [number, number, number] = [top[0] - bottom[0], top[1] - bottom[1], top[2] - bottom[2]]
      const length = Math.hypot(...along)
      const line = new Mesh(LINE, this.cord)
      line.scale.set(1, length, 1)
      line.position.set((top[0] + bottom[0]) / 2, (top[1] + bottom[1]) / 2, (top[2] + bottom[2]) / 2)
      line.quaternion.setFromUnitVectors({ x: 0, y: 1, z: 0 }, { x: along[0] / length, y: along[1] / length, z: along[2] / length })
      this.root.add(line)
    }
    this.root.visible = false
  }

  setColor(hex: number) {
    this.cloth.color.setHex(hex)
  }

  /** Opens or folds the canopy over `dt` seconds, keeping it where the body's feet are. */
  update(open: boolean, dt: number) {
    this.fill = open ? Math.min(1, this.fill + OPEN_RATE * dt) : Math.max(0, this.fill - OPEN_RATE * dt)
    this.root.visible = this.fill > 0.01
    // Shoots up from the pack and spreads.
    this.root.scale.set(this.fill, 0.4 + 0.6 * this.fill, this.fill)
  }

  dispose() {
    this.cloth.dispose()
    this.cord.dispose()
    this.root.removeFromParent()
  }
}
