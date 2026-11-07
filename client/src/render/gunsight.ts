import { Group, Material, Mesh, Texture, quadGeometry, ringGeometry, type Node, type PerspectiveCamera } from '@ragnarok/engine'

/** How far in front of the sight camera the reticle is drawn; only its angular size matters. */
const DISTANCE = 1
const YELLOW = 0xffec9f
/** Line thickness, as an angle (rad): about two pixels at the sight's zoom. */
const THICKNESS = 0.0007
/** Where the dark rim of the optic starts, as a fraction of the half-height of the view. */
const RIM = 0.94
const LABEL_SIZE = 0.012

/**
 * A tank gunner's reticle, drawn over the frame rather than as page elements: a crosshair, and the range
 * to what it is on. The server lays the gun so the shell lands where the crosshair is, its fall and all,
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
  private readonly rim: Mesh
  private readonly rangeSprite: Mesh
  private readonly rangeTexture: Texture
  private readonly rangeCanvas = document.createElement('canvas')
  private rangeText = ''
  private readonly textures: Texture[] = []
  private readonly materials: Material[] = []

  constructor() {
    this.rim = this.overlay(new Mesh(ringGeometry(1, 6, 64), this.shade), 1)
    this.root.add(this.rim)
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

  private sprite(texture: Texture, w: number, h: number) {
    const material = new Material({ map: texture, lit: false, blend: 'alpha', fog: false })
    this.materials.push(material)
    const sprite = this.overlay(new Mesh(this.quad, material), 3)
    sprite.scale.set(w * DISTANCE, h * DISTANCE, 1)
    return sprite
  }

  /** Puts the reticle where `camera` looks. Call once a frame, before rendering. */
  follow(camera: PerspectiveCamera) {
    this.root.position.copy(camera.position)
    this.root.quaternion.copy(camera.quaternion)
  }

  /** Shows the reticle. `fov` is the camera's vertical field of view, degrees; `range` is the distance to what the centre is on. */
  show(fov: number, range: number | null) {
    this.shown = true
    this.root.visible = true
    const half = Math.tan((fov * Math.PI) / 360)
    this.rim.scale.setScalar(half * RIM * DISTANCE)
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
