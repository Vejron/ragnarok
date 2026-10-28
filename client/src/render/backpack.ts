import { Group, Mesh, boxGeometry, cylinderGeometry, ringGeometry, Material, type Geometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/** The Intinor Compact Backpack (BACKPACK.md): a dark grey pack with an orange stripe and six modem antennas on top. */

const SHELL = 0x2b2f36
const STRIPE = 0xff8c00
const ANTENNA = 0x111316

let parts: { body: Geometry; stripe: Geometry; antenna: Geometry; shell: Material; trim: Material; tip: Material } | null = null

/**
 * The pack, 0.36 m wide, 0.46 m tall and 0.18 m deep, its straps' side towards -Z (worn, against the
 * back of a stickman facing -Z), with its bottom at the origin.
 */
export function buildBackpack(): Group {
  parts ??= {
    body: boxGeometry(0.36, 0.46, 0.18),
    stripe: boxGeometry(0.37, 0.05, 0.19),
    antenna: cylinderGeometry(0.012, 0.012, 0.14, 6),
    shell: toonMaterial({ color: SHELL }),
    trim: toonMaterial({ color: STRIPE }),
    tip: toonMaterial({ color: ANTENNA }),
  }
  const group = new Group()
  const body = new Mesh(parts.body, parts.shell)
  body.position.y = 0.23
  group.add(body)
  const stripe = new Mesh(parts.stripe, parts.trim)
  stripe.position.y = 0.3
  group.add(stripe)
  // Six 5G modems, an antenna each, in two rows of three.
  for (let i = 0; i < 6; i++) {
    const antenna = new Mesh(parts.antenna, parts.tip)
    antenna.position.set(-0.12 + (i % 3) * 0.12, 0.53, i < 3 ? -0.04 : 0.04)
    group.add(antenna)
  }
  return group
}

/** Bob and spin of the backpack lying about, so it catches the eye. */
const BOB = 0.08
const SPIN = 1.2
/** Drawn larger than worn: it is the prize. */
const SCALE = 1.6

/** The backpack lying on the ground, with a pulsing ring under it. */
export class LyingBackpacks {
  readonly root = new Group()
  private readonly placed = new Map<number, { group: Group; ring: Mesh }>()
  private readonly ring = ringGeometry(0.55, 0.75, 28)
  private readonly ringMaterial = new Material({ color: STRIPE, lit: false, blend: 'alpha', opacity: 0.6, side: 'double' })

  /** Makes the drawn set exactly `lying`: where each lies, its feet. */
  set(lying: readonly { id: number; x: number; y: number; z: number }[], seconds: number) {
    const listed = new Set<number>()
    for (const { id, x, y, z } of lying) {
      listed.add(id)
      let p = this.placed.get(id)
      if (!p) {
        const group = new Group()
        const model = buildBackpack()
        model.scale.setScalar(SCALE)
        group.add(model)
        const ring = new Mesh(this.ring, this.ringMaterial)
        ring.rotation.x = -Math.PI / 2
        this.root.add(ring)
        this.root.add(group)
        p = { group, ring }
        this.placed.set(id, p)
      }
      p.group.position.set(x, y + 0.15 + Math.sin(seconds * 2) * BOB, z)
      p.group.rotation.y = seconds * SPIN
      p.ring.position.set(x, y + 0.04, z)
      p.ring.scale.setScalar(1 + 0.15 * Math.sin(seconds * 4))
    }
    for (const [id, p] of this.placed) {
      if (listed.has(id)) continue
      p.group.removeFromParent()
      p.ring.removeFromParent()
      this.placed.delete(id)
    }
  }

  clear() {
    this.set([], 0)
  }
}
