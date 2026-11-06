import { Geometry, Group, Material, Mesh, Texture, boxGeometry, type Color, type PerspectiveCamera } from '@ragnarok/engine'
import { paintPaper, type MapMarks } from '@/hud/map'

/** The sheet's side, metres: a folded map, held at reading distance. */
const SHEET = 0.4
/** How far in front of the eye it is held, and how far below the line of sight, metres. */
const REACH = 0.5
const BELOW = 0.05
/** Lowered, it drops this far out of view. */
const DROP = 0.42
/** The top leans away from the eye, radians, as a map is held to be read. */
const TILT = 0.5
/** Each half of the fold turns back this much, radians: the sheet is a shallow V. */
const FOLD = 0.07
/** Pixels of the map texture's side. */
const TEXTURE = 1024
/** How often the map is redrawn while it is up, ms, unless the quality preset says otherwise. */
const REDRAW_MS = 125

const GLOVE = 0x8a7a5c

/** Half the sheet, `left` or right, with its half of the picture: a quad from the fold outwards. */
function halfSheet(left: boolean): Geometry {
  const w = SHEET / 2
  const h = SHEET / 2
  const [x0, x1] = left ? [-w, 0] : [0, w]
  const [u0, u1] = left ? [0, 0.5] : [0.5, 1]
  return new Geometry({
    positions: new Float32Array([x0, -h, 0, x1, -h, 0, x1, h, 0, x0, h, 0]),
    normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
    uvs: new Float32Array([u0, 1, u1, 1, u1, 0, u0, 0]),
    indices: new Uint16Array([0, 1, 2, 0, 2, 3]),
  })
}

/**
 * The paper map the soldier holds up to read, seen in first person: a folded sheet in two gloved hands,
 * laid out in view space under a group that `follow` keeps on the camera, as the gun sight is. Its parts
 * are held meshes (`Mesh.held`), so the ground or a wall never cuts into it; the hands go under the sheet
 * and the thumbs over it by draw order. The picture is `minimap.png` round the reader with the marks on it,
 * redrawn a few times a second while it is up, `span` metres across (the wheel zooms it).
 */
export class HeldMapView {
  readonly root = new Group()
  private readonly hold = new Group()
  private readonly canvas = document.createElement('canvas')
  private readonly g: CanvasRenderingContext2D | null
  private readonly texture: Texture
  private readonly paper: Material
  private readonly margin: Material
  private readonly glove = new Material({ color: GLOVE, lit: true, fog: false })
  /** The sleeves are in the soldier's colour (`dress`). */
  private sleeve = new Material({ color: 0x777777, lit: true, fog: false })
  private sleeveColor = -1
  private readonly arms: Mesh[] = []
  private readonly geometries: Geometry[] = []
  private readonly sheets: Mesh[] = []
  private drawnAt = -Infinity
  /** How often the map is redrawn while it is up, ms. */
  redrawMs = REDRAW_MS
  /** Metres across the picture shows, as last drawn. */
  private span = 0
  /** 0 put away, 1 held up to read. */
  private raise = 0
  private bob = 0
  private lagYaw = 0
  private lagPitch = 0

  constructor() {
    this.canvas.width = this.canvas.height = TEXTURE
    this.g = this.canvas.getContext('2d')
    this.texture = new Texture(this.canvas, { srgb: true, wrap: 'clamp', mipmaps: true, anisotropy: 4 })
    this.paper = new Material({ map: this.texture, lit: false, fog: false })
    this.margin = new Material({ color: 0xf2eee2, lit: false, fog: false })
    this.root.add(this.hold)

    // The sheet's white margin behind the picture, then the picture, in two halves about the fold.
    for (const left of [true, false]) {
      const half = new Group()
      half.rotateY(left ? FOLD : -FOLD)
      const back = halfSheet(left)
      const front = halfSheet(left)
      this.geometries.push(back, front)
      const border = this.inHand(new Mesh(back, this.margin), 11)
      border.scale.set(1.06, 1.06, 1)
      border.position.z = -0.001
      const picture = this.inHand(new Mesh(front, this.paper), 12)
      this.sheets.push(border, picture)
      half.add(border, picture)
      this.hold.add(half)
    }

    // The hands hold the bottom corners: palms behind the sheet, showing past its sides, thumbs over its
    // front, and the forearms running down and back out of view.
    const box = (w: number, h: number, d: number) => {
      const geometry = boxGeometry(w, h, d)
      this.geometries.push(geometry)
      return geometry
    }
    const forearm = box(0.06, 0.055, 0.2)
    const palm = box(0.05, 0.075, 0.026)
    const fingers = box(0.016, 0.07, 0.02)
    const thumb = box(0.018, 0.045, 0.014)
    for (const side of [-1, 1]) {
      const hand = new Group()
      hand.position.set(side * (SHEET / 2 - 0.005), -SHEET / 2 + 0.08, 0)
      hand.rotateZ(side * 0.12)
      const arm = this.inHand(new Mesh(forearm, this.sleeve), 9)
      arm.position.set(side * 0.03, -0.1, 0.05)
      arm.rotateX(1.0)
      this.arms.push(arm)
      const back = this.inHand(new Mesh(palm, this.glove), 10)
      back.position.set(side * 0.01, -0.005, -0.018)
      // The fingertips curl round the sheet's side edge.
      const tips = this.inHand(new Mesh(fingers, this.glove), 13)
      tips.position.set(side * 0.03, 0, -0.004)
      const over = this.inHand(new Mesh(thumb, this.glove), 13)
      over.position.set(-side * 0.008, 0.012, 0.01)
      over.rotateZ(side * 0.7)
      hand.add(arm, back, tips, over)
      this.hold.add(hand)
    }
    this.root.visible = false
  }

  private inHand(mesh: Mesh, order: number) {
    mesh.held = true
    mesh.renderOrder = order
    mesh.frustumCulled = false
    return mesh
  }

  /** Sleeves in `color` (0xRRGGBB), the soldier's own. */
  dress(color: number) {
    if (color === this.sleeveColor) return
    this.sleeveColor = color
    const old = this.sleeve
    this.sleeve = new Material({ color, lit: true, fog: false })
    for (const arm of this.arms) arm.material = this.sleeve
    old.dispose()
  }

  /** 0 put away, 1 up: where the sheet is. */
  get raised() {
    return this.raise
  }

  /**
   * Moves the map towards held up (`up`) or put away over `dt` seconds, swaying with `speed` (m/s, the
   * walk) and lagging the look (`yaw`, `pitch`), lit by `light` (the light on the ground, white at noon)
   * or the headlamp. Redraws the picture from `image` with `marks` every so often while it shows.
   */
  update(up: boolean, dt: number, now: number, speed: number, yaw: number, pitch: number, light: Readonly<Color>, lamp: boolean, image: HTMLImageElement | null, halfExtent: number, marks: MapMarks, span: number) {
    const target = up ? 1 : 0
    // About a third of a second either way.
    this.raise += Math.sign(target - this.raise) * Math.min(Math.abs(target - this.raise), dt * 3.2)
    const shown = this.raise > 0.001
    this.root.visible = shown
    if (!shown) {
      this.lagYaw = yaw
      this.lagPitch = pitch
      return
    }
    const ease = 1 - (1 - this.raise) ** 3
    this.bob += dt * Math.min(speed, 7) * 1.6
    const lag = Math.min(1, dt * 10)
    this.lagYaw += wrap(yaw - this.lagYaw) * lag
    this.lagPitch += (pitch - this.lagPitch) * lag
    const swayX = Math.max(-0.04, Math.min(0.04, wrap(yaw - this.lagYaw) * 0.25)) + Math.sin(this.bob) * 0.008
    const swayY = Math.max(-0.04, Math.min(0.04, (this.lagPitch - pitch) * 0.25)) + Math.abs(Math.cos(this.bob)) * 0.01
    this.hold.position.set(swayX, -BELOW - DROP * (1 - ease) + swayY, -REACH)
    this.hold.rotation.set(-TILT - 0.5 * (1 - ease), 0, swayX * 1.5)

    // Paper is unlit, so it takes the light of the hour by hand; the headlamp reads it at night.
    const lit = (channel: number) => Math.min(1, Math.max(0.12, channel, lamp ? 0.85 : 0))
    for (const sheet of this.sheets) sheet.tint.set([lit(light.r), lit(light.g), lit(light.b), 1])

    // A zoom shows at once; otherwise the marks move a few times a second.
    if ((now - this.drawnAt >= this.redrawMs || span !== this.span) && this.g) {
      this.span = span
      this.drawnAt = now
      this.g.save()
      paintPaper(this.g, image, halfExtent, TEXTURE, marks, span)
      this.g.restore()
      this.texture.needsUpdate()
    }
  }

  /** Puts the sheet in front of `camera`. Call once a frame, before rendering. */
  follow(camera: PerspectiveCamera) {
    this.root.position.copy(camera.position)
    this.root.quaternion.copy(camera.quaternion)
  }

  dispose() {
    for (const geometry of this.geometries) geometry.dispose()
    for (const material of [this.paper, this.margin, this.glove, this.sleeve]) material.dispose()
    this.texture.dispose()
    this.root.removeFromParent()
  }
}

const wrap = (a: number) => Math.atan2(Math.sin(a), Math.cos(a))
