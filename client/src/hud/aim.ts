import { Texture } from '@ragnarok/engine'
import type { HudStats } from '@/hud'
import type { MarkerView } from '@/feedback'
import { aimsReticle } from '@/seathints'
import { miniSize } from './map'
import { CanvasSprite, canvas2d, hudTexture, type Painter } from './paint'

const backOut = (t: number) => 1 + 2.70158 * (t - 1) ** 3 + 1.70158 * (t - 1) ** 2
/** On for the first half of every `period` ms: CSS's `steps(2)` blink. */
const blink = (now: number, period: number) => (now % period) < period / 2

/** A radial gradient over a square, stretched over the screen as CSS's `ellipse at center` is. */
function radial(stops: [number, string][]): Texture {
  const size = 256
  const [canvas, g] = canvas2d(size, size)
  if (g) {
    const gradient = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, (size / 2) * Math.SQRT2)
    for (const [at, color] of stops) gradient.addColorStop(at, color)
    g.fillStyle = gradient
    g.fillRect(0, 0, size, size)
  }
  return hudTexture(canvas)
}

/** Black with a round hole: the scope's view. */
function scopeMask(): Texture {
  const size = 512
  const [canvas, g] = canvas2d(size, size)
  if (g) {
    g.fillStyle = '#000'
    g.fillRect(0, 0, size, size)
    g.globalCompositeOperation = 'destination-out'
    const gradient = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2)
    gradient.addColorStop(0, '#000')
    gradient.addColorStop(38 / 38.4, '#000')
    gradient.addColorStop(1, 'rgba(0, 0, 0, 0)')
    g.fillStyle = gradient
    g.fillRect(0, 0, size, size)
  }
  return hudTexture(canvas)
}

/** The red dome of a damage arc, fading downwards. */
function arcShape(): Texture {
  const [canvas, g] = canvas2d(320, 32)
  if (g) {
    const gradient = g.createLinearGradient(0, 0, 0, 32)
    gradient.addColorStop(0, 'rgba(255, 40, 40, 0.95)')
    gradient.addColorStop(1, 'rgba(255, 40, 40, 0)')
    g.fillStyle = gradient
    g.beginPath()
    g.ellipse(160, 32, 160, 32, 0, Math.PI, 0)
    g.fill()
  }
  return hudTexture(canvas, true)
}

/** Repeating textures for a drone's feed: scanlines and static. */
function scanlines(): Texture {
  const [canvas, g] = canvas2d(1, 3)
  if (g) {
    g.fillStyle = 'rgba(0, 0, 0, 0.12)'
    g.fillRect(0, 0, 1, 1)
  }
  return new Texture(canvas, { srgb: false, wrap: 'repeat', mipmaps: false, filter: 'nearest' })
}

function noise(): Texture {
  const size = 128
  const [canvas, g] = canvas2d(size, size)
  if (g) {
    const image = g.createImageData(size, size)
    for (let i = 0; i < size * size; i++) {
      const v = Math.random() < 0.5 ? 255 : 0
      image.data[i * 4] = image.data[i * 4 + 1] = image.data[i * 4 + 2] = v
      image.data[i * 4 + 3] = Math.random() < 0.35 ? 140 : 0
    }
    g.putImageData(image, 0, 0)
  }
  return new Texture(canvas, { srgb: false, wrap: 'repeat', mipmaps: false, filter: 'nearest' })
}

/**
 * What is drawn at and around the crosshair, and over the whole view: the crosshair or reticle, the
 * scope, the hit marker, the arcs of harm taken, the armour callouts, a pilot's sights, a drone's feed,
 * and the red edges of being hurt.
 */
export class AimMarks {
  private readonly hurt = radial([[0.45, 'rgba(200, 0, 0, 0)'], [0.8, 'rgba(200, 0, 0, 0.55)'], [1, 'rgba(160, 0, 0, 0.85)']])
  private readonly lens = radial([[0.55, 'rgba(0, 0, 0, 0)'], [1, 'rgba(0, 0, 0, 0.55)']])
  private readonly scope = scopeMask()
  private readonly arc = arcShape()
  private readonly lines = scanlines()
  private readonly grain = noise()
  private readonly marker = new CanvasSprite(200, 200)

  /** Everything under the HUD's panels: drawn first. */
  under(p: Painter, s: HudStats, now: number) {
    const { width: W, height: H } = p
    if (s.drone) this.feed(p, s.drone, now)
    if (s.feedLost > 0.01) {
      p.rect(0, 0, W, H, 0x333333, s.feedLost)
      this.static(p, s.feedLost, now)
    }
    if (s.heat === null && s.scoped) this.scoped(p)
    if (s.hands && s.hp <= s.maxHp / 4) {
      // A heartbeat: 0.2 to 0.5 and back every 1.1 s.
      const t = (now % 1100) / 1100
      const beat = t < 0.3 ? t / 0.3 : (1 - t) / 0.7
      p.image(this.hurt, 0, 0, W, H, 0.2 + 0.3 * beat)
    }
    if (s.hurt > 0.01) p.image(this.hurt, 0, 0, W, H, s.hurt)
  }

  /** The marks at the crosshair: drawn over the panels. */
  over(p: Painter, s: HudStats, now: number) {
    const cx = p.width / 2
    const cy = p.height / 2
    if (s.feedLost > 0.01) p.label('SIGNAL LOST', cx, cy, { size: 28, weight: 900, color: '#fff', shadow: true, tracking: 0.2 }, 'center', s.feedLost)
    if (aimsReticle(s.seat) && !s.sight) {
      const loading = s.reload < 1 || s.jammed
      // Laying a mortar from above, the reticle sits on the mark rather than in the middle.
      const [x, y] = s.mark ? [s.mark[0] * p.width, s.mark[1] * p.height] : [cx, cy]
      p.ring(x, y, 13, 4.5, 0x000000, 0.45, 40)
      p.ring(x, y, 13, 3, loading ? 0xffbe78 : 0xffffff, loading ? 0.9 : 0.95, loading ? 20 : 40, loading)
    }
    if (s.seat === 'aviator') {
      if (s.nose) {
        const [x, y] = [s.nose[0] * p.width, s.nose[1] * p.height]
        p.rect(x - 12, y - 2, 24, 4, 0x000000, 0.4)
        p.rect(x - 2, y - 12, 4, 24, 0x000000, 0.4)
        p.rect(x - 11, y - 1, 22, 2, 0xffeca0, 0.95)
        p.rect(x - 1, y - 11, 2, 22, 0xffeca0, 0.95)
      }
      if (s.aim) {
        const [x, y] = [s.aim[0] * p.width, s.aim[1] * p.height]
        p.ring(x, y, 16, 4, 0x000000, 0.4)
        p.ring(x, y, 16, 2, 0xffffff, 0.9)
      }
      if (s.stall && blink(now, 500)) p.label('STALL', cx, p.height * 0.3, { size: 28, weight: 900, color: '#ff5a5f', shadow: true, tracking: 0.2 }, 'center')
    }
    if (s.drone) {
      p.rect(cx - 10, cy - 2, 20, 4, 0x000000, 0.4)
      p.rect(cx - 2, cy - 10, 4, 20, 0x000000, 0.4)
      p.rect(cx - 9, cy - 1, 18, 2, 0xffffff, 0.85)
      p.rect(cx - 1, cy - 9, 2, 18, 0xffffff, 0.85)
      const live = `● LIVE${s.drone.camera ? ` · ${s.drone.camera === 'down' ? 'CAM DOWN' : 'CAM FWD'}` : ''}`
      // Left of the minimap, where there is one.
      if (blink(now, 1000)) p.label(live, s.touch ? p.width - 16 : p.width - 12 - miniSize(p.width, p.height) - 18, 22, { size: 13, weight: 800, color: '#ff5a5f', shadow: true, tracking: 0.1 }, 'right')
      // Looking straight down, the top of the screen is where it heads.
      if (s.drone.camera === 'down') p.label('▲', cx, 24, { size: 14, color: 'rgba(255, 255, 255, 0.85)', shadow: true }, 'center')
    }
    if (s.heat === null && !s.scoped && s.armed) {
      // The gap is the gun's spread on the screen, in CSS pixels.
      const gap = s.crossGap / p.ui + (s.hitMarker ? 6 * (1 - s.hitMarker.progress) ** 2 : 0)
      const arm = (x: number, y: number, w: number, h: number) => {
        p.rect(x - 0.5, y - 0.5, w + 1, h + 1, 0x000000, 0.5)
        p.rect(x, y, w, h, 0xffffff, 0.8)
      }
      arm(cx - 1, cy - gap - 6, 2, 6)
      arm(cx - 1, cy + gap, 2, 6)
      arm(cx - gap - 6, cy - 1, 6, 2)
      arm(cx + gap, cy - 1, 6, 2)
    }
    if (s.blocked) {
      const [x, y] = [s.blocked[0] * p.width, s.blocked[1] * p.height]
      for (const turn of [1, -1]) {
        p.line(x - 7 * turn, y - 7, x + 7 * turn, y + 7, 3, 0x000000, 0.5)
        p.line(x - 7 * turn, y - 7, x + 7 * turn, y + 7, 2, 0xff4a3d)
      }
    }
    for (const arc of s.hurtArcs) {
      p.image(this.arc, cx + Math.sin(arc.angle) * 192 - 80, cy - Math.cos(arc.angle) * 192 - 8, 160, 16, arc.alpha, 0xffffff, undefined, arc.angle)
    }
    if (s.hitMarker) {
      this.marker.update(`${s.hitMarker.progress}|${s.hitMarker.kill}`, (g) => drawMarker(g, s.hitMarker!), p.scale)
      p.sprite(this.marker, cx - 100, cy - 100)
    }
    let y = cy + 34
    s.callouts.forEach((line, k) => {
      const size = k === 0 ? 15 : 13
      const color = line.tone === 'good' ? '#ffd27a' : line.tone === 'bad' ? '#ff7a6a' : '#f4f1ea'
      const text = `${line.text}${line.count > 1 ? ` ×${line.count}` : ''}`.toUpperCase()
      p.label(text, cx, y + size * 0.65, { size, weight: 800, color, shadow: true, tracking: 0.02 }, 'center', line.alpha)
      y += size * 1.3 + 2
    })
  }

  private scoped(p: Painter) {
    const { width: W, height: H } = p
    const vmin = Math.min(W, H) / 100
    const r = 38.4 * vmin
    const [cx, cy] = [W / 2, H / 2]
    p.image(this.scope, cx - r, cy - r, 2 * r, 2 * r)
    // Black round the square the hole is in.
    p.rect(0, 0, W, cy - r + 0.5, 0x000000)
    p.rect(0, cy + r - 0.5, W, H - cy - r + 0.5, 0x000000)
    p.rect(0, cy - r, cx - r + 0.5, 2 * r, 0x000000)
    p.rect(cx + r - 0.5, cy - r, W - cx - r + 0.5, 2 * r, 0x000000)
    p.rect(cx - 38 * vmin, cy - 0.5, 76 * vmin, 1, 0x000000, 0.85)
    p.rect(cx - 0.5, cy - 38 * vmin, 1, 76 * vmin, 0x000000, 0.85)
  }

  private feed(p: Painter, drone: NonNullable<HudStats['drone']>, now: number) {
    const { width: W, height: H } = p
    p.image(this.lines, 0, 0, W, H, 1, 0xffffff, [0, 0, 1, H / 3])
    p.image(this.lens, 0, 0, W, H)
    this.static(p, 0.06 + 0.6 * (1 - drone.signal) ** 2, now)
  }

  /** Static: the grain, jumping four times every quarter second. */
  private static(p: Painter, alpha: number, now: number) {
    const step = Math.floor(now / 62.5)
    const u = ((step * 0.37) % 1) * 128
    const v = ((step * 0.61) % 1) * 128
    const cell = 2
    const across = p.width / (128 * cell)
    const down = p.height / (128 * cell)
    p.image(this.grain, 0, 0, p.width, p.height, alpha, 0xffffff, [u / 128, v / 128, u / 128 + across, v / 128 + down])
  }

  dispose() {
    for (const texture of [this.hurt, this.lens, this.scope, this.arc, this.lines, this.grain]) texture.dispose()
    this.marker.dispose()
  }
}

/** The hit marker in a 200-px square: four tapered ticks that punch out and settle, and for a kill a red X and a ring. */
function drawMarker(g: CanvasRenderingContext2D, m: MarkerView) {
  const p = m.progress
  const fade = 1 - p * p * p
  const punch = 0.55 + 0.45 * backOut(Math.min(1, p * 3.5))
  const colour = m.kill ? '#ff3030' : '#ffffff'
  g.save()
  g.translate(100, 100)
  g.globalAlpha = fade
  g.lineJoin = 'round'
  const reach = (m.kill ? 15 : 10) + 8 * p
  const length = (m.kill ? 15 : 11) * punch
  for (let k = 0; k < 4; k++) {
    g.save()
    g.rotate(Math.PI / 4 + (k * Math.PI) / 2)
    g.beginPath()
    g.moveTo(reach, -1)
    g.lineTo(reach + length, -(m.kill ? 4.5 : 3))
    g.lineTo(reach + length, m.kill ? 4.5 : 3)
    g.lineTo(reach, 1)
    g.closePath()
    g.lineWidth = 3
    g.strokeStyle = 'rgba(0,0,0,0.75)'
    g.stroke()
    g.fillStyle = colour
    g.fill()
    g.restore()
  }
  if (m.kill) {
    const r = 9 + 22 * backOut(Math.min(1, p * 3)) * 0.9
    g.lineCap = 'round'
    for (const [stroke, width] of [['rgba(0,0,0,0.7)', 8], ['#ff3030', 4.5]] as const) {
      g.strokeStyle = stroke
      g.lineWidth = width
      g.beginPath()
      g.moveTo(-r, -r)
      g.lineTo(r, r)
      g.moveTo(r, -r)
      g.lineTo(-r, r)
      g.stroke()
    }
    g.globalAlpha = Math.max(0, 1 - p * 1.6)
    g.strokeStyle = '#ff6040'
    g.lineWidth = 3 * (1 - p) + 1
    g.beginPath()
    g.arc(0, 0, 18 + 70 * p, 0, Math.PI * 2)
    g.stroke()
  }
  g.restore()
}

