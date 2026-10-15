import { Color } from '../math/vec'

let nextId = 1

export type Blend = 'opaque' | 'alpha' | 'additive'
export type Side = 'front' | 'back' | 'double'

/**
 * A lighting ramp for toon shading: the brightness of each band, 0..255, from the side facing away from
 * the sun to the side facing it. Share one instance between materials; each distinct ramp takes a row
 * of the renderer's ramp texture.
 */
export class Ramp {
  readonly id = nextId++
  constructor(readonly bands: Uint8Array) {}
}

export interface MaterialParams {
  /** Toon-lit by the sun and hemisphere, or plain colour. */
  lit?: boolean
  color?: Color | number
  opacity?: number
  emissive?: Color | number
  vertexColors?: boolean
  ramp?: Ramp | null
  blend?: Blend
  side?: Side
  depthTest?: boolean
  depthWrite?: boolean
  /** Pushes the surface toward the camera in depth-buffer units, to draw decals over what they lie on. */
  depthBias?: number
  depthBiasSlope?: number
  fog?: boolean
}

/**
 * What a surface looks like. Materials are data: each one is a record in a storage buffer that every
 * draw can index, so a scene with a thousand materials still draws with a handful of pipelines. Only the
 * render-state fields (blend, side, depth) pick the pipeline; the rest can change per frame for free.
 * After changing a field, call `needsUpdate()`.
 */
export class Material {
  readonly id = nextId++
  lit: boolean
  readonly color: Color
  opacity: number
  readonly emissive: Color
  vertexColors: boolean
  ramp: Ramp | null
  blend: Blend
  side: Side
  depthTest: boolean
  depthWrite: boolean
  depthBias: number
  depthBiasSlope: number
  fog: boolean
  /** Bumped by `needsUpdate`. */
  version = 0
  /** @internal the renderer's slot in the material buffer. */
  gpu: unknown = undefined
  disposed = false

  constructor(params: MaterialParams = {}) {
    this.lit = params.lit ?? true
    this.color = toColor(params.color ?? 0xffffff)
    this.opacity = params.opacity ?? 1
    this.emissive = toColor(params.emissive ?? 0x000000)
    this.vertexColors = params.vertexColors ?? false
    this.ramp = params.ramp ?? null
    this.blend = params.blend ?? 'opaque'
    this.side = params.side ?? 'front'
    this.depthTest = params.depthTest ?? true
    this.depthWrite = params.depthWrite ?? this.blend === 'opaque'
    this.depthBias = params.depthBias ?? 0
    this.depthBiasSlope = params.depthBiasSlope ?? 0
    this.fog = params.fog ?? true
  }

  get transparent() {
    return this.blend !== 'opaque'
  }

  needsUpdate() {
    this.version++
  }

  dispose() {
    this.disposed = true
    this.version++
  }
}

function toColor(c: Color | number): Color {
  return typeof c === 'number' ? Color.hex(c) : new Color(c.r, c.g, c.b)
}
