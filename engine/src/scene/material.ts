import { Color } from '../math/vec'
import type { Texture } from './texture'

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
  /** Multiplies the colour; sampled at the geometry's uv. */
  map?: Texture | null
  /** An object-space normal map (as three's ObjectSpaceNormalMap): the texel is the normal in the mesh's own space. */
  normalMap?: Texture | null
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
  name = ''
  lit: boolean
  readonly color: Color
  opacity: number
  readonly emissive: Color
  vertexColors: boolean
  map: Texture | null
  normalMap: Texture | null
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
    this.map = params.map ?? null
    this.normalMap = params.normalMap ?? null
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

/** A uniform value of a ShaderMaterial: a number is an f32, a Color a vec3f, an array of 2 to 4 numbers a vecNf. */
export type UniformValue = number | Color | [number, number] | [number, number, number] | [number, number, number, number]

export interface ShaderMaterialParams extends Omit<MaterialParams, 'lit' | 'map' | 'normalMap' | 'ramp'> {
  /**
   * WGSL defining `fn surface(s: Surface) -> vec4f`, the fragment's linear colour and alpha before fog.
   * `Surface` has `world`, `normal` (unit, facing the viewer), `color` (material colour × tint × vertex
   * colour), `uv`, `custom` (the geometry's custom attribute), `front` and `viewDepth`. Uniforms are
   * fields of `params`; textures are bound by their names, with `materialSampler` to sample them. The
   * frame's `frame.cameraPos`, `frame.sunDir`, `frame.fog.w` (seconds) and the rest are in scope too.
   */
  wgsl: string
  uniforms?: Record<string, UniformValue>
  textures?: Record<string, Texture>
}

/**
 * A material with its own fragment code. It draws with its own pipeline, so it does not batch with
 * standard materials, but meshes sharing one ShaderMaterial and geometry still draw as one call.
 * Change `uniforms` values freely and call `needsUpdate()` to send them.
 */
export class ShaderMaterial extends Material {
  readonly wgsl: string
  readonly uniforms: Record<string, UniformValue>
  readonly textures: Record<string, Texture>

  constructor(params: ShaderMaterialParams) {
    super({ ...params, lit: false })
    this.wgsl = params.wgsl
    this.uniforms = { ...params.uniforms }
    this.textures = { ...params.textures }
  }
}
