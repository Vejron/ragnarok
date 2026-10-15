import { Group, Material, Mesh, Texture, quadGeometry, type PerspectiveCamera } from '@ragnarok/engine'
import * as THREE from 'three'

const LIFE = 0.95
const MAX_ACTIVE = 14
/** Metres a number rises over its life. */
const RISE = 1.7

interface Floater {
  sprite: Mesh
  from: THREE.Vector3
  drift: number
  age: number
  size: number
  texture: Texture
}

/** One unit quad every number shares; it is turned to face the camera and scaled. */
const QUAD = quadGeometry(1, 1)

/**
 * Damage numbers floating up out of what was hit, drawn in the scene so they have depth and
 * shrink with distance (kept readable by a size that grows with it). A kill's number is bigger.
 */
export class DamageNumbers {
  readonly root = new Group()
  private readonly active: Floater[] = []

  /** A number of `amount` at `at`: white for a body, amber for a vehicle, red and big for a kill. */
  spawn(at: THREE.Vector3, amount: number, kind: 'body' | 'vehicle' | 'kill' | 'rooster', camera: PerspectiveCamera) {
    if (amount <= 0) return
    if (this.active.length >= MAX_ACTIVE) this.finish(0)
    const fill = { body: '#ffffff', vehicle: '#ffb020', kill: '#ff3b3b', rooster: '#ffe14a' }[kind]
    const canvas = document.createElement('canvas')
    canvas.width = 160
    canvas.height = 80
    const g = canvas.getContext('2d')
    if (g) {
      g.font = `900 ${kind === 'kill' ? 60 : 48}px system-ui, sans-serif`
      g.textAlign = 'center'
      g.textBaseline = 'middle'
      g.lineJoin = 'round'
      g.lineWidth = 9
      g.strokeStyle = 'rgba(0,0,0,0.85)'
      const text = String(amount)
      g.strokeText(text, 80, 42)
      g.fillStyle = fill
      g.fillText(text, 80, 42)
    }
    const texture = new Texture(canvas, { srgb: true, wrap: 'clamp' })
    // Unlit, on top of everything: a sprite is a quad turned to face the camera each frame.
    const material = new Material({ map: texture, lit: false, blend: 'alpha', depthTest: false, depthWrite: false, fog: false })
    const sprite = new Mesh(QUAD, material)
    sprite.renderOrder = 999
    sprite.frustumCulled = false
    this.root.add(sprite)
    const distance = Math.hypot(camera.position.x - at.x, camera.position.y - at.y, camera.position.z - at.z)
    // Roughly constant on screen from near to far, within bounds.
    const size = THREE.MathUtils.clamp(distance * 0.05, 0.7, 4) * (kind === 'kill' ? 1.35 : 1)
    this.active.push({ sprite, from: at.clone(), drift: (Math.random() - 0.5) * 0.9, age: 0, size, texture })
  }

  private finish(index: number) {
    const f = this.active[index]!
    this.root.remove(f.sprite)
    f.sprite.material.dispose()
    f.texture.dispose()
    this.active.splice(index, 1)
  }

  update(dt: number, camera: PerspectiveCamera) {
    for (let i = this.active.length - 1; i >= 0; i--) {
      const f = this.active[i]!
      f.age += dt
      const p = f.age / LIFE
      if (p >= 1) {
        this.finish(i)
        continue
      }
      const rise = 1 - (1 - p) ** 3
      f.sprite.position.set(f.from.x + f.drift * rise, f.from.y + 0.3 + RISE * rise, f.from.z)
      // A punch in, a settle, then a shrink as it goes.
      const pop = p < 0.12 ? 0.5 + (p / 0.12) * 0.9 : 1.4 - Math.min(1, (p - 0.12) / 0.2) * 0.4
      const shrink = p > 0.65 ? 1 - (p - 0.65) * 0.8 : 1
      f.sprite.scale.set(f.size * 2 * pop * shrink, f.size * pop * shrink, 1)
      f.sprite.quaternion.copy(camera.quaternion)
      f.sprite.material.opacity = p > 0.6 ? 1 - (p - 0.6) / 0.4 : 1
      f.sprite.material.needsUpdate()
    }
  }

  dispose() {
    while (this.active.length > 0) this.finish(0)
  }
}
