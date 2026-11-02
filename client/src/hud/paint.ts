import { Texture, type HudLayer } from '@ragnarok/engine'

export const FONT = "ui-rounded, 'SF Pro Rounded', system-ui, sans-serif"
/** The HUD's dark ink, as the page's CSS had it. */
export const INK = 0x1d2b36

export interface TextStyle {
  /** CSS pixels. */
  size: number
  weight?: number
  /** CSS colour. */
  color?: string
  /** A soft dark shadow under the letters, for text straight on the scene. */
  shadow?: boolean
  /** Extra space between letters, em. */
  tracking?: number
}

export type Align = 'left' | 'center' | 'right'

/** HUD textures are sampled as they are, in sRGB, and never minified much. */
export const hudTexture = (source: HTMLCanvasElement | HTMLImageElement, mipmaps = false) => new Texture(source, { srgb: false, wrap: 'clamp', mipmaps })

/** `#rrggbb` (or `#rgb`) to 0xRRGGBB; anything else is white. */
export function hexOf(css: string): number {
  if (css.startsWith('#')) {
    const body = css.slice(1)
    if (body.length === 3) return parseInt(body.split('').map((c) => c + c).join(''), 16)
    if (body.length === 6) return parseInt(body, 16)
  }
  return 0xffffff
}

export function canvas2d(width: number, height: number): [HTMLCanvasElement, CanvasRenderingContext2D | null] {
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(1, Math.ceil(width))
  canvas.height = Math.max(1, Math.ceil(height))
  return [canvas, canvas.getContext('2d')]
}

let measuring: CanvasRenderingContext2D | null | undefined
const measurer = () => (measuring === undefined ? (measuring = canvas2d(1, 1)[1]) : measuring)

interface Label {
  texture: Texture | null
  /** CSS pixels. */
  width: number
  height: number
}

/** Padding round a label's letters for their shadow, CSS pixels. */
const PAD = 3

/**
 * Strings drawn once on a canvas and kept as textures, the least recently used dropped past `capacity`.
 * Text that changes (a ping, a speed) just makes new entries.
 */
export class TextCache {
  private readonly labels = new Map<string, Label>()
  private scale = 1

  constructor(private readonly capacity = 256) {}

  get size() {
    return this.labels.size
  }

  /** Drawing-buffer pixels per CSS pixel; changing it redraws everything as it is next asked for. */
  setScale(scale: number) {
    if (scale === this.scale) return
    this.scale = scale
    this.clear()
  }

  get(text: string, style: TextStyle): Label {
    const key = `${style.size}|${style.weight ?? 700}|${style.color ?? '#fff'}|${style.shadow ? 1 : 0}|${style.tracking ?? 0}|${text}`
    const found = this.labels.get(key)
    if (found) {
      // Most recently used last.
      this.labels.delete(key)
      this.labels.set(key, found)
      return found
    }
    const label = this.draw(text, style)
    this.labels.set(key, label)
    if (this.labels.size > this.capacity) {
      const [oldest, dropped] = this.labels.entries().next().value!
      this.labels.delete(oldest)
      dropped.texture?.dispose()
    }
    return label
  }

  private draw(text: string, style: TextStyle): Label {
    const font = `${style.weight ?? 700} ${style.size}px ${FONT}`
    const tracking = (style.tracking ?? 0) * style.size
    const measure = measurer()
    let width = text.length * style.size * 0.6
    if (measure) {
      measure.font = font
      width = measure.measureText(text).width
    }
    width += tracking * Math.max(0, text.length - 1)
    const height = Math.ceil(style.size * 1.3)
    const s = this.scale
    const [canvas, g] = canvas2d((width + PAD * 2) * s, (height + PAD * 2) * s)
    if (!g) return { texture: null, width, height }
    g.scale(s, s)
    g.font = font
    g.textBaseline = 'middle'
    g.fillStyle = style.color ?? '#fff'
    if (style.shadow) {
      g.shadowColor = 'rgba(0, 0, 0, 0.8)'
      g.shadowBlur = 2 * s
      g.shadowOffsetY = 1 * s
    }
    if (tracking === 0) g.fillText(text, PAD, PAD + height / 2)
    else {
      let x = PAD
      for (const ch of text) {
        g.fillText(ch, x, PAD + height / 2)
        x += g.measureText(ch).width + tracking
      }
    }
    return { texture: hudTexture(canvas), width, height }
  }

  clear() {
    for (const label of this.labels.values()) label.texture?.dispose()
    this.labels.clear()
  }
}

/**
 * A texture drawn on a canvas by `paint`, redrawn only when `key` changes: for HUD parts with shapes
 * quads cannot make (the hit marker, the body map, the minimap).
 */
export class CanvasSprite {
  readonly canvas: HTMLCanvasElement
  readonly texture: Texture
  private readonly g: CanvasRenderingContext2D | null
  private key: unknown = undefined

  /** `width` × `height` CSS pixels at `scale` drawing pixels each. */
  constructor(readonly width: number, readonly height: number, private scale = 1, mipmaps = false) {
    ;[this.canvas, this.g] = canvas2d(width * scale, height * scale)
    this.texture = hudTexture(this.canvas, mipmaps)
  }

  /** Redraws with `paint` (in CSS pixels) unless `key` is what it was last time. */
  update(key: unknown, paint: (g: CanvasRenderingContext2D) => void, scale = this.scale) {
    if (scale !== this.scale) {
      this.scale = scale
      this.canvas.width = Math.ceil(this.width * scale)
      this.canvas.height = Math.ceil(this.height * scale)
      this.key = undefined
    }
    if (key === this.key || !this.g) return
    this.key = key
    const g = this.g
    g.setTransform(1, 0, 0, 1, 0, 0)
    g.clearRect(0, 0, this.canvas.width, this.canvas.height)
    g.setTransform(this.scale, 0, 0, this.scale, 0, 0)
    paint(g)
    this.texture.needsUpdate()
  }

  dispose() {
    this.texture.dispose()
  }
}

/** A white disc, for rounded ends and dots, cut into nine for rounded boxes. */
function disc(): Texture {
  const size = 64
  const [canvas, g] = canvas2d(size, size)
  if (g) {
    g.fillStyle = '#fff'
    g.beginPath()
    g.arc(size / 2, size / 2, size / 2 - 0.5, 0, Math.PI * 2)
    g.fill()
  }
  return new Texture(canvas, { srgb: false, wrap: 'clamp', mipmaps: true })
}

/** Drawing on a `HudLayer` in CSS pixels: boxes, rounded boxes, bars, dots and text. */
export class Painter {
  readonly text = new TextCache()
  readonly disc = disc()
  /** CSS pixels of the view. */
  width = 1
  height = 1
  scale = 1

  constructor(readonly layer: HudLayer) {}

  begin(width: number, height: number, scale: number) {
    this.width = width
    this.height = height
    this.scale = scale
    this.text.setScale(scale)
    this.layer.begin(width, height)
  }

  rect(x: number, y: number, w: number, h: number, color: number, alpha = 1) {
    this.layer.quad(x, y, w, h, color, alpha)
  }

  /** A box with corners of `radius`, from the disc cut into nine. */
  round(x: number, y: number, w: number, h: number, radius: number, color: number, alpha = 1) {
    const r = Math.max(0, Math.min(radius, w / 2, h / 2))
    if (r < 0.5) return this.rect(x, y, w, h, color, alpha)
    const d = this.disc
    const L = this.layer
    // Corners: quarters of the disc.
    L.quad(x, y, r, r, color, alpha, d, [0, 0, 0.5, 0.5])
    L.quad(x + w - r, y, r, r, color, alpha, d, [0.5, 0, 1, 0.5])
    L.quad(x, y + h - r, r, r, color, alpha, d, [0, 0.5, 0.5, 1])
    L.quad(x + w - r, y + h - r, r, r, color, alpha, d, [0.5, 0.5, 1, 1])
    // Edges and middle: flat.
    if (w > 2 * r) {
      L.quad(x + r, y, w - 2 * r, h, color, alpha)
    }
    if (h > 2 * r) {
      L.quad(x, y + r, r, h - 2 * r, color, alpha)
      L.quad(x + w - r, y + r, r, h - 2 * r, color, alpha)
    }
  }

  /** A pill (fully rounded ends) with an outline `ring` pixels wide outside it. */
  pill(x: number, y: number, w: number, h: number, color: number, alpha = 1, ring = 0, ringColor = 0x000000, ringAlpha = 0.25) {
    if (ring > 0) this.round(x - ring, y - ring, w + 2 * ring, h + 2 * ring, h / 2 + ring, ringColor, ringAlpha)
    this.round(x, y, w, h, h / 2, color, alpha)
  }

  /** A box with corners of `radius` and an outline `ring` wide outside it. */
  box(x: number, y: number, w: number, h: number, radius: number, color: number, alpha = 1, ring = 0, ringColor = 0x000000, ringAlpha = 0.25) {
    if (ring > 0) this.round(x - ring, y - ring, w + 2 * ring, h + 2 * ring, radius + ring, ringColor, ringAlpha)
    this.round(x, y, w, h, radius, color, alpha)
  }

  dot(cx: number, cy: number, r: number, color: number, alpha = 1) {
    this.layer.centred(cx, cy, 2 * r, 2 * r, color, alpha, this.disc)
  }

  /** A ring of `segments` short lines. */
  ring(cx: number, cy: number, r: number, width: number, color: number, alpha = 1, segments = 32, dashed = false) {
    for (let k = 0; k < segments; k++) {
      if (dashed && k % 2) continue
      const a0 = (k / segments) * Math.PI * 2
      const a1 = ((k + 1) / segments) * Math.PI * 2
      // A little long, so the segments meet without gaps.
      const grow = dashed ? 0 : 0.6 / r
      this.layer.line(cx + Math.cos(a0 - grow) * r, cy + Math.sin(a0 - grow) * r, cx + Math.cos(a1 + grow) * r, cy + Math.sin(a1 + grow) * r, width, color, alpha)
    }
  }

  line(x0: number, y0: number, x1: number, y1: number, width: number, color: number, alpha = 1) {
    this.layer.line(x0, y0, x1, y1, width, color, alpha)
  }

  /** `text` with its left, middle or right at `x` and its middle at `y`; returns its width. */
  label(text: string, x: number, y: number, style: TextStyle, align: Align = 'left', alpha = 1): number {
    if (!text) return 0
    const label = this.text.get(text, style)
    const left = align === 'left' ? x : align === 'center' ? x - label.width / 2 : x - label.width
    if (label.texture) this.layer.quad(left - PAD, y - label.height / 2 - PAD, label.width + PAD * 2, label.height + PAD * 2, 0xffffff, alpha, label.texture)
    return label.width
  }

  /** How wide `text` is in `style`, CSS pixels. */
  measure(text: string, style: TextStyle): number {
    return text ? this.text.get(text, style).width : 0
  }

  image(texture: Texture, x: number, y: number, w: number, h: number, alpha = 1, color = 0xffffff, uv?: readonly number[], rotation = 0) {
    this.layer.quad(x, y, w, h, color, alpha, texture, uv, rotation)
  }

  /** A canvas sprite at its own size. */
  sprite(sprite: CanvasSprite, x: number, y: number, alpha = 1) {
    this.layer.quad(x, y, sprite.width, sprite.height, 0xffffff, alpha, sprite.texture)
  }

  dispose() {
    this.text.clear()
    this.disc.dispose()
  }
}
