import * as THREE from 'three'

/** Most blobs drawn in one frame; the rest are dropped. */
const CAPACITY = 128
/** Above this height a blob has faded out completely, metres. */
const FADE_HEIGHT = 24
/** Sample spacing for the ground slope under a blob, metres. */
const SLOPE_STEP = 0.6
/** How far a blob floats over the ground so it does not z-fight with it, metres. */
const LIFT = 0.04

/** A soft round falloff, black in the middle and clear at the rim. */
function falloff() {
  const size = 64
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = size
  const g = canvas.getContext('2d')!
  const gradient = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2)
  gradient.addColorStop(0, 'rgba(0,0,0,1)')
  gradient.addColorStop(0.45, 'rgba(0,0,0,0.65)')
  gradient.addColorStop(1, 'rgba(0,0,0,0)')
  g.fillStyle = gradient
  g.fillRect(0, 0, size, size)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

/**
 * Blob shadows: one instanced, flat, soft-edged disc per character and vehicle, laid on the ground
 * and tilted to its slope. The sun's shadow map is too coarse and too far offset to ground a small
 * thing; this darkens the spot right under it. Call `begin`, `add` for each thing, then `end`.
 */
export class ContactShadows {
  readonly root = new THREE.Group()
  private readonly mesh: THREE.InstancedMesh
  private readonly texture = falloff()
  private count = 0
  private enabled = true
  private readonly matrix = new THREE.Matrix4()
  private readonly position = new THREE.Vector3()
  private readonly rotation = new THREE.Quaternion()
  private readonly tilt = new THREE.Quaternion()
  private readonly turn = new THREE.Quaternion()
  private readonly scale = new THREE.Vector3()
  private readonly normal = new THREE.Vector3()
  private static readonly UP = new THREE.Vector3(0, 1, 0)

  constructor(private readonly groundAt: (x: number, z: number) => number) {
    const geometry = new THREE.PlaneGeometry(2, 2)
    geometry.rotateX(-Math.PI / 2)
    const material = new THREE.MeshBasicMaterial({
      map: this.texture,
      color: 0x10200c,
      transparent: true,
      opacity: 0.5,
      depthWrite: false,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
    })
    this.mesh = new THREE.InstancedMesh(geometry, material, CAPACITY)
    this.mesh.frustumCulled = false
    this.mesh.count = 0
    this.mesh.renderOrder = 1
    this.root.add(this.mesh)
  }

  get active() {
    return this.enabled
  }

  set active(on: boolean) {
    this.enabled = on
    this.root.visible = on
  }

  begin() {
    this.count = 0
  }

  /**
   * A blob under `(x, z)`, `halfWidth` by `halfLength` metres, turned to `yaw`. A thing `height`
   * metres above the ground casts a smaller one, gone by `FADE_HEIGHT`.
   */
  add(x: number, z: number, halfWidth: number, halfLength: number, yaw: number, height = 0) {
    if (!this.enabled || this.count >= CAPACITY) return
    const fade = 1 - height / FADE_HEIGHT
    if (fade <= 0.05) return
    const y = this.groundAt(x, z)
    const dx = (this.groundAt(x + SLOPE_STEP, z) - this.groundAt(x - SLOPE_STEP, z)) / (2 * SLOPE_STEP)
    const dz = (this.groundAt(x, z + SLOPE_STEP) - this.groundAt(x, z - SLOPE_STEP)) / (2 * SLOPE_STEP)
    this.normal.set(-dx, 1, -dz).normalize()
    this.tilt.setFromUnitVectors(ContactShadows.UP, this.normal)
    this.turn.setFromAxisAngle(ContactShadows.UP, yaw)
    this.rotation.copy(this.tilt).multiply(this.turn)
    this.position.set(x, y + LIFT, z)
    this.scale.set(halfWidth * fade, 1, halfLength * fade)
    this.matrix.compose(this.position, this.rotation, this.scale)
    this.mesh.setMatrixAt(this.count++, this.matrix)
  }

  end() {
    this.mesh.count = this.count
    this.mesh.instanceMatrix.needsUpdate = true
  }

  dispose() {
    this.mesh.geometry.dispose()
    ;(this.mesh.material as THREE.Material).dispose()
    this.texture.dispose()
    this.mesh.dispose()
  }
}
