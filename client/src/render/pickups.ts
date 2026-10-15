import { Group, Material, Mesh, ringGeometry } from '@ragnarok/engine'
import { buildItem } from './items'

/** Items bob this high and spin this fast, so they catch the eye. */
const BOB = 0.08
const SPIN = 1.6
/** Drawn larger than in hand: a revolver is easy to lose in grass. */
const SCALE = 1.7

interface Placed {
  item: number
  group: Group
  /** Phase, so a field of them does not bob in step. */
  phase: number
  base: number
}

/** The items lying on the ground, redrawn from each snapshot's list. */
export class Pickups {
  readonly root = new Group()
  private readonly placed = new Map<number, Placed>()
  private readonly ring = ringGeometry(0.35, 0.45, 20)
  private readonly ringMaterial = new Material({ color: 0xffe9a0, lit: false, blend: 'alpha', opacity: 0.55, side: 'double' })

  /** Makes the drawn set exactly `items`: `{ id, item, x, y, z }`. */
  set(items: readonly { id: number; item: number; x: number; y: number; z: number }[]) {
    const listed = new Set<number>()
    for (const { id, item, x, y, z } of items) {
      listed.add(id)
      let p = this.placed.get(id)
      if (p && p.item !== item) {
        this.drop(id)
        p = undefined
      }
      if (!p) {
        const group = new Group()
        const model = buildItem(item)
        model.scale.setScalar(SCALE)
        group.add(model)
        const ring = new Mesh(this.ring, this.ringMaterial)
        ring.rotation.x = -Math.PI / 2
        ring.position.y = -0.4
        group.add(ring)
        this.root.add(group)
        p = { item, group, phase: id * 1.7, base: y }
        this.placed.set(id, p)
      }
      p.base = y
      p.group.position.set(x, y, z)
    }
    for (const id of this.placed.keys()) if (!listed.has(id)) this.drop(id)
  }

  private drop(id: number) {
    const p = this.placed.get(id)
    if (!p) return
    p.group.removeFromParent()
    this.placed.delete(id)
  }

  update(seconds: number) {
    for (const p of this.placed.values()) {
      p.group.position.y = p.base + Math.sin(seconds * 2 + p.phase) * BOB
      p.group.rotation.y = seconds * SPIN + p.phase
    }
  }

  clear() {
    for (const id of this.placed.keys()) this.drop(id)
  }

  dispose() {
    this.clear()
    this.ring.dispose()
    this.ringMaterial.dispose()
    this.root.removeFromParent()
  }
}
