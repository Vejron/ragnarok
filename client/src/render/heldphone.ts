import { Geometry, Group, Material, Mesh, Texture, Vector3, boxGeometry, type PerspectiveCamera } from '@ragnarok/engine'
import type { Point } from '@/phone/homography'
import { SCREEN_H, SCREEN_W } from '@/phone/ui'

/** The phone's body, metres: a big modern phone. */
const BODY_W = 0.078
const BODY_H = 0.158
const BODY_D = 0.009
/** Its screen, metres: the body less a thin bezel, in the canvas's proportions. */
const SCREEN_HEIGHT = 0.146
const SCREEN_WIDTH = (SCREEN_HEIGHT * SCREEN_W) / SCREEN_H
/**
 * The phone and hands are built at life size but drawn this much larger and as much farther off: the
 * same picture, kept beyond the camera's near plane (0.25 m), which a phone at reading distance is not.
 */
const SCALE = 2.6
/** How far in front of the eye it is held, and how far below the line of sight, metres as it looks. */
const REACH = 0.19 * SCALE
const BELOW = 0.012 * SCALE
/** Lowered, it drops this far out of view. */
const DROP = 0.22 * SCALE
/** The top leans away from the eye, radians, as a phone is held to be read. */
const TILT = 0.22
/** How often the screen is redrawn while it is up and nothing asks for it sooner, ms. */
const REDRAW_MS = 125
/** Only this far raised does the screen take taps, so one does not land while it swings up. */
const READY = 0.95

const BODY = 0x1b1f24
const GLOVE = 0x8a7a5c

/** The four corners on the page of the screen, as fractions of the view: top left, top right, bottom right, bottom left. */
export type ScreenCorners = [Point, Point, Point, Point]

/** A flat quad `w` × `h` facing +Z with the whole texture on it. */
function quad(w: number, h: number): Geometry {
  const [x, y] = [w / 2, h / 2]
  return new Geometry({
    positions: new Float32Array([-x, -y, 0, x, -y, 0, x, y, 0, -x, y, 0]),
    normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
    uvs: new Float32Array([0, 1, 1, 1, 1, 0, 0, 0]),
    indices: new Uint16Array([0, 1, 2, 0, 2, 3]),
  })
}

/**
 * The phone the soldier holds up (PHONE.md), seen in first person: a dark slab in two gloved hands,
 * laid out in view space under a group that `follow` keeps on the camera, as the gun sight is. Its parts
 * are held meshes (`Mesh.held`), so the ground or a wall never cuts into it. The screen shows a canvas
 * the phone's apps draw on (`phone/screen.ts`), lit by itself; `follow` works out where its corners are
 * on the page, so a pointer there can be turned into a tap on it.
 */
export class HeldPhoneView {
  readonly root = new Group()
  /** What the apps draw on: `SCREEN_W` × `SCREEN_H`. */
  readonly canvas = document.createElement('canvas')
  readonly g: CanvasRenderingContext2D | null
  private readonly hold = new Group()
  private readonly texture: Texture
  private readonly screenMaterial: Material
  private readonly body = new Material({ color: BODY, lit: true, fog: false })
  private readonly glove = new Material({ color: GLOVE, lit: true, fog: false })
  /** The sleeves are in the soldier's colour (`dress`). */
  private sleeve = new Material({ color: 0x777777, lit: true, fog: false })
  private sleeveColor = -1
  private readonly arms: Mesh[] = []
  private readonly geometries: Geometry[] = []
  private readonly screen: Mesh
  private drawnAt = -Infinity
  /** How often the screen is redrawn while it is up and nothing asks for it sooner, ms. */
  redrawMs = REDRAW_MS
  /** 0 put away, 1 held up. */
  private raise = 0
  private bob = 0
  private lagYaw = 0
  private lagPitch = 0
  /** Where the screen's corners were on the page at the last `follow`, while it takes taps. */
  corners: ScreenCorners | null = null

  constructor() {
    this.canvas.width = SCREEN_W
    this.canvas.height = SCREEN_H
    this.g = this.canvas.getContext('2d')
    this.texture = new Texture(this.canvas, { srgb: true, wrap: 'clamp', mipmaps: true, anisotropy: 8 })
    this.screenMaterial = new Material({ map: this.texture, lit: false, fog: false })
    this.root.add(this.hold)
    this.hold.scale.setScalar(SCALE)

    const box = (w: number, h: number, d: number) => {
      const geometry = boxGeometry(w, h, d)
      this.geometries.push(geometry)
      return geometry
    }
    // The slab, its edges rounded off by a slightly smaller box over each long side.
    const slab = this.inHand(new Mesh(box(BODY_W - 0.004, BODY_H, BODY_D), this.body), 11)
    const sides = this.inHand(new Mesh(box(BODY_W, BODY_H - 0.006, BODY_D - 0.002), this.body), 11)
    const face = quad(SCREEN_WIDTH, SCREEN_HEIGHT)
    this.geometries.push(face)
    this.screen = this.inHand(new Mesh(face, this.screenMaterial), 12)
    this.screen.position.z = BODY_D / 2 + 0.0004
    this.hold.add(slab, sides, this.screen)

    // The hands hold its lower half: palms behind it, fingers curled round its sides, thumbs resting
    // along its edges with only their tips on the screen, and the forearms running down and back out of view.
    const forearm = box(0.055, 0.05, 0.2)
    const palm = box(0.05, 0.08, 0.026)
    const fingers = box(0.014, 0.065, 0.018)
    const thumb = box(0.013, 0.04, 0.012)
    for (const side of [-1, 1]) {
      const hand = new Group()
      hand.position.set(side * (BODY_W / 2 + 0.004), -BODY_H / 2 + 0.045, 0)
      hand.rotateZ(side * 0.18)
      const arm = this.inHand(new Mesh(forearm, this.sleeve), 9)
      arm.position.set(side * 0.03, -0.1, 0.05)
      arm.rotateX(1.0)
      this.arms.push(arm)
      const back = this.inHand(new Mesh(palm, this.glove), 10)
      back.position.set(side * 0.012, -0.01, -0.02)
      const tips = this.inHand(new Mesh(fingers, this.glove), 13)
      tips.position.set(side * 0.006, 0, 0.002)
      const over = this.inHand(new Mesh(thumb, this.glove), 13)
      over.position.set(side * 0.002, -0.012, BODY_D / 2 + 0.006)
      over.rotateZ(side * 0.25)
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

  /** 0 put away, 1 up: where the phone is. */
  get raised() {
    return this.raise
  }

  /**
   * Moves the phone towards held up (`up`) or put away over `dt` seconds, swaying with `speed` (m/s, the
   * walk) and lagging the look (`yaw`, `pitch`). While it shows, redraws the screen with `paint` when
   * `urgent` (something changed) or every `redrawMs` (the reader moves on the map).
   */
  update(up: boolean, dt: number, now: number, speed: number, yaw: number, pitch: number, urgent: boolean, paint: (g: CanvasRenderingContext2D) => void) {
    const target = up ? 1 : 0
    // About a quarter of a second either way.
    this.raise += Math.sign(target - this.raise) * Math.min(Math.abs(target - this.raise), dt * 4)
    const shown = this.raise > 0.001
    this.root.visible = shown
    if (!shown) {
      this.lagYaw = yaw
      this.lagPitch = pitch
      this.corners = null
      return
    }
    const ease = 1 - (1 - this.raise) ** 3
    this.bob += dt * Math.min(speed, 7) * 1.6
    const lag = Math.min(1, dt * 10)
    this.lagYaw += wrap(yaw - this.lagYaw) * lag
    this.lagPitch += (pitch - this.lagPitch) * lag
    const swayX = (Math.max(-0.02, Math.min(0.02, wrap(yaw - this.lagYaw) * 0.12)) + Math.sin(this.bob) * 0.004) * SCALE
    const swayY = (Math.max(-0.02, Math.min(0.02, (this.lagPitch - pitch) * 0.12)) + Math.abs(Math.cos(this.bob)) * 0.005) * SCALE
    this.hold.position.set(swayX, -BELOW - DROP * (1 - ease) + swayY, -REACH)
    this.hold.rotation.set(-TILT - 0.6 * (1 - ease), 0, (swayX / SCALE) * 1.5)

    if ((urgent || now - this.drawnAt >= this.redrawMs) && this.g) {
      this.drawnAt = now
      this.g.save()
      paint(this.g)
      this.g.restore()
      this.texture.needsUpdate()
    }
  }

  /** Puts the phone in front of `camera`, and works out where its screen is on the page. Call once a frame, before rendering. */
  follow(camera: PerspectiveCamera) {
    this.root.position.copy(camera.position)
    this.root.quaternion.copy(camera.quaternion)
    if (!this.root.visible || this.raise < READY) {
      this.corners = null
      return
    }
    this.screen.updateWorldMatrix(true, false)
    const [x, y] = [SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2]
    const corners: Point[] = []
    for (const [cx, cy] of [[-x, y], [x, y], [x, -y], [-x, -y]] as const) {
      const point = this.screen.localToWorld(new Vector3(cx, cy, 0))
      if (!camera.project(point)) {
        this.corners = null
        return
      }
      corners.push([(point.x + 1) / 2, (1 - point.y) / 2])
    }
    this.corners = corners as ScreenCorners
  }

  dispose() {
    for (const geometry of this.geometries) geometry.dispose()
    for (const material of [this.screenMaterial, this.body, this.glove, this.sleeve]) material.dispose()
    this.texture.dispose()
    this.root.removeFromParent()
  }
}

const wrap = (a: number) => Math.atan2(Math.sin(a), Math.cos(a))
