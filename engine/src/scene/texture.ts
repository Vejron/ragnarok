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
  /** Lets the source go once the renderer holds a copy (`Texture.uploadOnce`). */
  uploadOnce?: boolean
}

const NO_PIXELS = new Uint8Array(0)

/**
 * An image for materials to sample. The renderer uploads it on first use and again after `needsUpdate()`
 * (for a canvas that has been drawn on again, say).
 *
 * The source stays here after the upload, a second copy of every pixel, so that it can be sent again
 * (to a new device when the old one is lost, too). A big texture made once sets `uploadOnce` to keep
 * the GPU's copy only.
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
  /**
   * Drops the source once the renderer has uploaded it (a bitmap is closed): the texture keeps its
   * size and is drawn from the GPU's copy, but on another device (after a lost one) it is white until
   * it is given a source again.
   */
  uploadOnce: boolean
  private held: TextureSource
  private gone = false

  constructor(source: TextureSource, params: TextureParams = {}) {
    this.held = source
    this.uploadOnce = params.uploadOnce ?? false
    this.srgb = params.srgb ?? true
    this.filter = params.filter ?? 'linear'
    this.wrap = params.wrap ?? 'repeat'
    this.mipmaps = params.mipmaps ?? this.filter === 'linear'
    this.anisotropy = params.anisotropy ?? 1
  }

  /** The image. After it was let go (`released`), its size with no pixels. Set a new one and call `needsUpdate()` to change it. */
  get source(): TextureSource {
    return this.held
  }

  set source(source: TextureSource) {
    this.held = source
    this.gone = false
  }

  /** Whether the source was let go after the upload (`uploadOnce`). */
  get released() {
    return this.gone
  }

  /** @internal lets the source go, keeping its size: the renderer calls it once it holds a copy. */
  release() {
    if (this.gone) return
    const { width, height } = this.held
    if (typeof ImageBitmap !== 'undefined' && this.held instanceof ImageBitmap) this.held.close()
    this.held = { data: NO_PIXELS, width, height }
    this.gone = true
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
