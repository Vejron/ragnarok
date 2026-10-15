import { Group, Material, Mesh, Texture, quadGeometry, ringGeometry, type Node, type PerspectiveCamera } from '@ragnarok/engine'

/** How far in front of the sight camera the reticle is drawn; only its angular size matters. */
const DISTANCE = 1
const YELLOW = 0xffec9f
/** Line thickness, as an angle (rad): about two pixels at the sight's zoom. */
const THICKNESS = 0.0007
/** Half-widths (rad) of a hundred-metre mark and of a fifty-metre one. */
const MAJOR = 0.02
const MINOR = 0.011
/** Where the dark rim of the optic starts, as a fraction of the half-height of the view. */
const RIM = 0.94
const LABEL_SIZE = 0.012

/** One mark of the range ladder: a shell lands this far (rad, view-plane tangent) below the crosshair. */
export interface Mark {
  range: number
  /** Tangent offset from the view's centre; down is negative. */
  y: number
}

/**
 * A tank gunner's reticle, drawn over the frame rather than as page elements: a crosshair along the
 * barrel's line with a ladder of range marks below it. The sight's camera looks exactly down the
 * barrel, so the centre is where a shell leaves; the marks are where it lands at 100, 150, ... m, and
 * the numbers are hundreds of metres. The parts are overlay meshes laid out in view space, in a group
 * that `follow` keeps on the camera, so they are drawn after everything else and never hidden by it.
 */
export class GunSightView {
  /** Add this to the scene; `follow` keeps it in front of the camera. */
  readonly root = new Group()
  private shown = false
  private readonly quad = quadGeometry(1, 1)
  private readonly line = new Material({ color: YELLOW, lit: false, blend: 'alpha', fog: false })
  private readonly shade = new Material({ color: 0x000000, lit: false, blend: 'alpha', fog: false })
  private readonly ladder = new Group()
  private readonly rim: Mesh
  private readonly labels = new Map<number, Mesh>()
  private readonly rangeSprite: Mesh
  private readonly rangeTexture: Texture
  private readonly rangeCanvas = document.createElement('canvas')
  private rangeText = ''
  private readonly textures: Texture[] = []
  private readonly materials: Material[] = []

  constructor() {
    this.rim = this.overlay(new Mesh(ringGeometry(1, 6, 64), this.shade), 1)
    this.root.add(this.rim)
    this.root.add(this.ladder)
    // The crosshair: two arms that stop short of the centre, so the target stays in view.
    this.bar(-0.05, 0, 0.03, THICKNESS * 1.6)
    this.bar(0.05, 0, 0.03, THICKNESS * 1.6)
    this.bar(0, 0.05, THICKNESS * 1.6, 0.03)
    this.bar(0, -0.05, THICKNESS * 1.6, 0.03)
    this.bar(0, 0, THICKNESS * 3, THICKNESS * 3)
    this.rangeCanvas.width = 128
    this.rangeCanvas.height = 32
    this.rangeTexture = new Texture(this.rangeCanvas, { srgb: true, wrap: 'clamp' })
    this.textures.push(this.rangeTexture)
    this.rangeSprite = this.sprite(this.rangeTexture, LABEL_SIZE * 3.2, LABEL_SIZE * 0.8)
    this.rangeSprite.position.set(0, -0.35 * DISTANCE, -DISTANCE)
    this.root.add(this.rangeSprite)
    this.root.visible = false
  }

  get visible() {
    return this.shown
  }

  private overlay(mesh: Mesh, order: number) {
    mesh.overlay = true
    mesh.renderOrder = order
    mesh.frustumCulled = false
    return mesh
  }

  /** A flat rectangle `w` by `h` (rad) centred `x`, `y` (tangent) from the view's centre. */
  private bar(x: number, y: number, w: number, h: number, parent: Node = this.root) {
    const mesh = this.overlay(new Mesh(this.quad, this.line), 2)
    mesh.scale.set(w * DISTANCE, h * DISTANCE, 1)
    mesh.position.set(x * DISTANCE, y * DISTANCE, -DISTANCE)
    parent.add(mesh)
    return mesh
  }

  private readonly pool: Mesh[] = []
  private barsUsed = 0

  /** A bar of the ladder, reused from frame to frame. */
  private ladderBar(x: number, y: number, w: number, h: number) {
    let mesh = this.pool[this.barsUsed++]
    if (!mesh) {
      mesh = this.overlay(new Mesh(this.quad, this.line), 2)
      this.pool.push(mesh)
    }
    mesh.scale.set(w * DISTANCE, h * DISTANCE, 1)
    mesh.position.set(x * DISTANCE, y * DISTANCE, -DISTANCE)
    this.ladder.add(mesh)
    return mesh
  }

  private sprite(texture: Texture, w: number, h: number) {
    const material = new Material({ map: texture, lit: false, blend: 'alpha', fog: false })
    this.materials.push(material)
    const sprite = this.overlay(new Mesh(this.quad, material), 3)
    sprite.scale.set(w * DISTANCE, h * DISTANCE, 1)
    return sprite
  }

  private label(hundreds: number) {
    let sprite = this.labels.get(hundreds)
    if (!sprite) {
      const canvas = document.createElement('canvas')
      canvas.width = canvas.height = 32
      const context = canvas.getContext('2d')
      if (context) {
        context.font = 'bold 26px ui-rounded, system-ui, sans-serif'
        context.textAlign = 'center'
        context.textBaseline = 'middle'
        context.fillStyle = '#ffec9f'
        context.shadowColor = '#000'
        context.shadowBlur = 3
        context.fillText(String(hundreds), 16, 17)
      }
      const texture = new Texture(canvas, { srgb: true, wrap: 'clamp' })
      this.textures.push(texture)
      sprite = this.sprite(texture, LABEL_SIZE, LABEL_SIZE)
      this.labels.set(hundreds, sprite)
    }
    return sprite
  }

  /** Puts the reticle where `camera` looks. Call once a frame, before rendering. */
  follow(camera: PerspectiveCamera) {
    this.root.position.copy(camera.position)
    this.root.quaternion.copy(camera.quaternion)
  }

  /**
   * Lays the ladder out and shows the reticle. `fov` is the camera's vertical field of view,
   * degrees; `marks` are in view-plane tangents; `range` is the distance to what the centre is on.
   */
  show(fov: number, marks: readonly Mark[], range: number | null) {
    this.shown = true
    this.root.visible = true
    const half = Math.tan((fov * Math.PI) / 360)
    this.rim.scale.setScalar(half * RIM * DISTANCE)
    // The ladder's bars come from a pool, so a frame makes no new meshes (each would be a new object record).
    for (const child of [...this.ladder.children]) this.ladder.remove(child)
    this.barsUsed = 0
    if (marks.length > 0) {
      // A stem from the crosshair down to the last mark.
      const last = marks[marks.length - 1]!.y
      this.ladderBar(0, last / 2, THICKNESS * 1.2, -last)
    }
    for (const mark of marks) {
      const major = mark.range % 100 === 0
      const width = major ? MAJOR : MINOR
      this.ladderBar(0, mark.y, width * 2, THICKNESS * 1.6)
      if (major) {
        const sprite = this.label(mark.range / 100)
        sprite.position.set((MAJOR + LABEL_SIZE * 0.9) * DISTANCE, mark.y * DISTANCE, -DISTANCE)
        this.ladder.add(sprite)
      }
    }
    const text = range === null ? '— —' : `${Math.round(range)} m`
    if (text !== this.rangeText) {
      this.rangeText = text
      const context = this.rangeCanvas.getContext('2d')
      if (context) {
        context.clearRect(0, 0, 128, 32)
        context.font = 'bold 22px ui-rounded, system-ui, sans-serif'
        context.textAlign = 'center'
        context.textBaseline = 'middle'
        context.fillStyle = '#ffec9f'
        context.shadowColor = '#000'
        context.shadowBlur = 3
        context.fillText(text, 64, 17)
        this.rangeTexture.needsUpdate()
      }
    }
  }

  hide() {
    this.shown = false
    this.root.visible = false
  }

  dispose() {
    this.quad.dispose()
    this.line.dispose()
    this.shade.dispose()
    this.rim.geometry.dispose()
    for (const material of this.materials) material.dispose()
    for (const texture of this.textures) texture.dispose()
    this.root.removeFromParent()
  }
}
