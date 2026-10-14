import * as THREE from 'three'

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
 * A tank gunner's reticle, drawn in its own pass over the scene rather than as page elements: a
 * crosshair along the barrel's line with a ladder of range marks below it. The sight's camera looks
 * exactly down the barrel, so the centre is where a shell leaves; the marks are where it lands at
 * 100, 150, ... m, and the numbers are hundreds of metres.
 */
export class GunSightView {
  readonly scene = new THREE.Scene()
  /** Draws the reticle with the main camera's field of view. */
  readonly camera = new THREE.PerspectiveCamera(20, 1, 0.1, 10)
  visible = false
  private readonly quad = new THREE.PlaneGeometry(1, 1)
  private readonly line = new THREE.MeshBasicMaterial({ color: YELLOW, depthTest: false, depthWrite: false })
  private readonly shade = new THREE.MeshBasicMaterial({ color: 0x000000, depthTest: false, depthWrite: false })
  private readonly ladder = new THREE.Group()
  private readonly rim: THREE.Mesh
  private readonly labels = new Map<number, THREE.Sprite>()
  private readonly rangeSprite: THREE.Sprite
  private readonly rangeCanvas = document.createElement('canvas')
  private rangeText = ''
  private readonly textures: THREE.Texture[] = []

  constructor() {
    const ring = new THREE.RingGeometry(1, 6, 64)
    this.rim = new THREE.Mesh(ring, this.shade)
    this.rim.renderOrder = 1
    this.scene.add(this.rim)
    this.scene.add(this.ladder)
    // The crosshair: two arms that stop short of the centre, so the target stays in view.
    this.bar(-0.05, 0, 0.03, THICKNESS * 1.6)
    this.bar(0.05, 0, 0.03, THICKNESS * 1.6)
    this.bar(0, 0.05, THICKNESS * 1.6, 0.03)
    this.bar(0, -0.05, THICKNESS * 1.6, 0.03)
    this.bar(0, 0, THICKNESS * 3, THICKNESS * 3)
    this.rangeCanvas.width = 128
    this.rangeCanvas.height = 32
    this.rangeSprite = this.sprite(this.rangeCanvas, LABEL_SIZE * 3.2, LABEL_SIZE * 0.8)
    this.rangeSprite.position.set(0, -0.35 * DISTANCE, -DISTANCE)
    this.scene.add(this.rangeSprite)
  }

  /** A flat rectangle `w` by `h` (rad) centred `x`, `y` (tangent) from the view's centre. */
  private bar(x: number, y: number, w: number, h: number, parent: THREE.Object3D = this.scene) {
    const mesh = new THREE.Mesh(this.quad, this.line)
    mesh.scale.set(w * DISTANCE, h * DISTANCE, 1)
    mesh.position.set(x * DISTANCE, y * DISTANCE, -DISTANCE)
    mesh.renderOrder = 2
    parent.add(mesh)
    return mesh
  }

  private sprite(source: HTMLCanvasElement, w: number, h: number) {
    const texture = new THREE.CanvasTexture(source)
    texture.colorSpace = THREE.SRGBColorSpace
    this.textures.push(texture)
    const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: texture, depthTest: false, depthWrite: false, transparent: true }))
    sprite.scale.set(w * DISTANCE, h * DISTANCE, 1)
    sprite.renderOrder = 3
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
      sprite = this.sprite(canvas, LABEL_SIZE, LABEL_SIZE)
      this.labels.set(hundreds, sprite)
    }
    return sprite
  }

  /**
   * Lays the ladder out and shows the reticle. `fov` is the sight camera's vertical field of view,
   * degrees; `marks` are in view-plane tangents; `range` is the distance to what the centre is on.
   */
  show(fov: number, aspect: number, marks: readonly Mark[], range: number | null) {
    this.visible = true
    this.camera.fov = fov
    this.camera.aspect = aspect
    this.camera.updateProjectionMatrix()
    const half = Math.tan((fov * Math.PI) / 360)
    this.rim.scale.setScalar(half * RIM * DISTANCE)
    for (const child of [...this.ladder.children]) this.ladder.remove(child)
    if (marks.length > 0) {
      // A stem from the crosshair down to the last mark.
      const last = marks[marks.length - 1]!.y
      this.bar(0, last / 2, THICKNESS * 1.2, -last, this.ladder)
    }
    for (const mark of marks) {
      const major = mark.range % 100 === 0
      const width = major ? MAJOR : MINOR
      this.bar(0, mark.y, width * 2, THICKNESS * 1.6, this.ladder)
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
        ;(this.rangeSprite.material.map as THREE.Texture).needsUpdate = true
      }
    }
  }

  hide() {
    this.visible = false
  }

  dispose() {
    this.quad.dispose()
    this.line.dispose()
    this.shade.dispose()
    this.rim.geometry.dispose()
    for (const texture of this.textures) texture.dispose()
  }
}
