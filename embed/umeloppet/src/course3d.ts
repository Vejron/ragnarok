import { Material, Mesh, Node, Texture, boxGeometry, cylinderGeometry } from '@ragnarok/engine'
import { startIsFinish, type Course } from './course'

/** How far from the skier the course is drawn, metres. */
const REACH = 400
/** The course is laid again once the skier has moved this far, metres, or a tile under it has come in. */
const RELAY_DISTANCE = 30
/** How often a lay may happen, seconds. */
const RELAY_EVERY = 0.4
/** An orange marker stick stands this often beside the course, metres, this far to its right, this tall. */
const STICK_EVERY = 25
const STICK_SIDE = 2.6
const STICK_HEIGHT = 1.3
/** Width of the start and finish arches, metres, and how high their banners hang. */
const ARCH_WIDTH = 7
const ARCH_HEIGHT = 3

const stickMaterial = new Material({ color: 0xff6a1a, emissive: 0x401000 })
const postMaterial = new Material({ color: 0x5a4632 })
const stickGeometry = cylinderGeometry(0.025, 0.025, STICK_HEIGHT, 6).translate(0, STICK_HEIGHT / 2, 0)

/**
 * A sign's face: `text` in white letters on `colour`, as a texture. Drawn upside down, since a box's
 * faces (`boxGeometry`) have their v running up, and an image's rows run down.
 */
function signTexture(text: string, colour: string, ink = '#fff'): Texture {
  const canvas = document.createElement('canvas')
  const font = '800 84px system-ui, sans-serif'
  const g = canvas.getContext('2d')!
  g.font = font
  canvas.width = Math.max(512, Math.ceil(g.measureText(text).width) + 80)
  canvas.height = 128
  g.fillStyle = colour
  g.fillRect(0, 0, canvas.width, canvas.height)
  g.translate(0, canvas.height)
  g.scale(1, -1)
  g.fillStyle = ink
  g.font = font
  g.textAlign = 'center'
  g.textBaseline = 'middle'
  g.fillText(text, canvas.width / 2, canvas.height / 2 + 4)
  return new Texture(canvas, { mipmaps: false })
}

/** Something stood on the snow at a place on the course, once the ground under it has come in. */
interface Standing {
  node: Node
  x: number
  z: number
}

/**
 * The course in the 3D world, which is groomed into the snow as a ski track (`trails.ts`): orange marker sticks
 * beside it, a sign at each kilometre, and arches at the start and the finish.
 */
export class CourseView {
  readonly root = new Node()
  private readonly sticks: Mesh[] = []
  private readonly standing: Standing[] = []
  private laidAt: [number, number] | null = null
  private incomplete = true
  private wait = 0

  constructor(
    private readonly course: Course,
    /** The ground's height, or null where its tile has not come in. */
    private readonly heightAt: (x: number, z: number) => number | null,
  ) {
    for (const { x, z, km } of course.km) this.stand(this.sign(`${km} km`, '#e4572e'), x, z, this.yawAt(x, z))
    const { start, finish } = course
    if (startIsFinish(course)) this.stand(this.arch('START · MÅL', '#1f8a4c'), start.x, start.z, start.yaw)
    else {
      this.stand(this.arch('START', '#1f8a4c'), start.x, start.z, start.yaw)
      this.stand(this.arch('MÅL', '#1d1d1d'), finish.x, finish.z, finish.yaw)
    }
  }

  /** Places the sticks around the skier at `(x, z)` again if they have moved on or the ground has come in. */
  update(x: number, z: number, dt: number) {
    this.wait -= dt
    if (this.wait > 0) return
    const moved = !this.laidAt || Math.hypot(x - this.laidAt[0], z - this.laidAt[1]) > RELAY_DISTANCE
    if (!moved && !this.incomplete) return
    this.wait = RELAY_EVERY
    this.laidAt = [x, z]
    this.incomplete = false
    this.laySticks(x, z)
    for (const thing of this.standing) {
      const y = this.heightAt(thing.x, thing.z)
      thing.node.visible = y !== null
      if (y === null) this.incomplete ||= Math.hypot(thing.x - x, thing.z - z) < REACH
      else thing.node.position.y = y
    }
  }

  /** The course's heading near `(x, z)`, in the soldier's yaw. */
  private yawAt(x: number, z: number): number {
    const points = this.course.points
    let best = 0
    let bestDistance = Infinity
    points.forEach(([px, pz], i) => {
      const d = (px - x) ** 2 + (pz - z) ** 2
      if (d < bestDistance) {
        bestDistance = d
        best = i
      }
    })
    const [ax, az] = points[Math.max(0, best - 1)]!
    const [bx, bz] = points[Math.min(points.length - 1, best + 1)]!
    return Math.atan2(-(bx - ax), -(bz - az))
  }

  /** The marker sticks within reach, every `STICK_EVERY` metres along, on the course's right. */
  private laySticks(x: number, z: number) {
    const { points, along } = this.course
    let used = 0
    let next = 0
    for (let i = 0; i < points.length - 1; i++) {
      const a = along[i]!
      const b = along[i + 1]!
      while (next <= b) {
        if (next < a) {
          next += STICK_EVERY
          continue
        }
        const t = (next - a) / Math.max(1e-6, b - a)
        next += STICK_EVERY
        const [ax, az] = points[i]!
        const [bx, bz] = points[i + 1]!
        const length = Math.hypot(bx - ax, bz - az) || 1
        const px = ax + (bx - ax) * t + (-(bz - az) / length) * STICK_SIDE
        const pz = az + (bz - az) * t + ((bx - ax) / length) * STICK_SIDE
        if (Math.hypot(px - x, pz - z) > REACH) continue
        const y = this.heightAt(px, pz)
        if (y === null) {
          this.incomplete = true
          continue
        }
        let stick = this.sticks[used]
        if (!stick) {
          stick = new Mesh(stickGeometry, stickMaterial)
          stick.castShadow = true
          this.sticks.push(stick)
          this.root.add(stick)
        }
        stick.visible = true
        stick.position.set(px, y, pz)
        used++
      }
    }
    for (let i = used; i < this.sticks.length; i++) this.sticks[i]!.visible = false
  }

  private stand(node: Node, x: number, z: number, yaw: number) {
    node.position.set(x, 0, z)
    node.rotation.set(0, yaw, 0)
    node.visible = false
    this.root.add(node)
    this.standing.push({ node, x, z })
  }

  /** A kilometre sign beside the course, on a post at each end of its board so neither hides the number. */
  private sign(text: string, colour: string): Node {
    const node = new Node()
    const width = 1.1
    const board = new Mesh(boxGeometry(width, 0.42, 0.05), new Material({ map: signTexture(text, colour), lit: false }))
    board.position.set(STICK_SIDE + 0.4, 1.65, 0)
    board.castShadow = true
    node.add(board)
    for (const side of [-1, 1]) {
      const post = new Mesh(cylinderGeometry(0.04, 0.04, 1.9, 8).translate(0, 0.95, 0), postMaterial)
      post.position.x = board.position.x + side * (width / 2 + 0.04)
      post.castShadow = true
      node.add(post)
    }
    return node
  }

  /** An arch over the course: two posts and a banner saying `text`. */
  private arch(text: string, colour: string): Node {
    const node = new Node()
    for (const side of [-1, 1]) {
      const post = new Mesh(cylinderGeometry(0.09, 0.09, ARCH_HEIGHT + 0.5, 10).translate(0, (ARCH_HEIGHT + 0.5) / 2, 0), postMaterial)
      post.position.x = (side * ARCH_WIDTH) / 2
      post.castShadow = true
      node.add(post)
    }
    const banner = new Mesh(boxGeometry(ARCH_WIDTH, 0.9, 0.06), new Material({ map: signTexture(text, colour), lit: false }))
    banner.position.y = ARCH_HEIGHT
    banner.castShadow = true
    node.add(banner)
    return node
  }

}
