import { Group, Mesh, boxGeometry, cylinderGeometry, type Geometry, type Material } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/** Warm-less LED white for the lens, past 1 so that it blooms. */
const LENS_ON = 0xffffff
const HOUSING = 0x1b1e24
const LENS_OFF = 0x6d7480

/** Where it sits on the head, in the head's frame with +Z ahead, metres. */
const ON_HEAD = [0, 0.13, 0.17] as const

let parts: { body: Geometry; glass: Geometry; housing: Material; off: Material; on: Material } | null = null

function shared() {
  return (parts ??= {
    body: boxGeometry(0.085, 0.055, 0.05),
    glass: cylinderGeometry(0.021, 0.021, 0.012, 8),
    housing: toonMaterial({ color: HOUSING }),
    off: toonMaterial({ color: LENS_OFF }),
    on: toonMaterial({ color: LENS_ON, emissive: LENS_ON }),
  })
}

/** A headlamp, +Z ahead, whose lens can be lit. */
export class Headlamp {
  readonly root = new Group()
  private readonly lens: Mesh

  constructor() {
    const { body, glass, housing, off } = shared()
    this.root.position.set(...ON_HEAD)
    this.root.add(new Mesh(body, housing))
    this.lens = new Mesh(glass, off)
    this.lens.rotation.x = Math.PI / 2
    this.lens.position.z = 0.028
    this.root.add(this.lens)
  }

  set lit(on: boolean) {
    this.lens.material = on ? shared().on : shared().off
  }
}
