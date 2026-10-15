let nextId = 1

/** Raw pixels: RGBA8, `width × height × 4` bytes, rows top to bottom. */
export interface PixelData {
  data: Uint8Array
  width: number
  height: number
}

export type TextureSource = PixelData | HTMLCanvasElement | OffscreenCanvas | ImageBitmap | HTMLImageElement | ImageData

export interface TextureParams {
  /** Colour textures are sRGB (decoded to linear when sampled); data such as normal maps are not. */
  srgb?: boolean
  filter?: 'linear' | 'nearest'
  wrap?: 'repeat' | 'clamp'
  mipmaps?: boolean
  anisotropy?: number
}

/**
 * An image for materials to sample. The renderer uploads it on first use and again after `needsUpdate()`
 * (for a canvas that has been drawn on again, say).
 */
export class Texture {
  readonly id = nextId++
  srgb: boolean
  filter: 'linear' | 'nearest'
  wrap: 'repeat' | 'clamp'
  mipmaps: boolean
  anisotropy: number
  version = 0
  /** @internal the renderer's GPU copy. */
  gpu: unknown = undefined
  disposed = false

  constructor(public source: TextureSource, params: TextureParams = {}) {
    this.srgb = params.srgb ?? true
    this.filter = params.filter ?? 'linear'
    this.wrap = params.wrap ?? 'repeat'
    this.mipmaps = params.mipmaps ?? this.filter === 'linear'
    this.anisotropy = params.anisotropy ?? 1
  }

  get width() {
    return this.source.width
  }

  get height() {
    return this.source.height
  }

  needsUpdate() {
    this.version++
  }

  dispose() {
    this.disposed = true
    this.version++
  }
}
