import type { HudLayer } from '@ragnarok/engine'
import type { HudStats } from '@/hud'
import { AimMarks } from './aim'
import { Gauges } from './gauges'
import { loadMap, miniSize, paintMap } from './map'
import { CanvasSprite, Painter, uiScale } from './paint'
import { Panels } from './panels'

/** The minimap is painted at its desktop size, and drawn smaller on a small screen. */
const MINI_PX = 180
const MINI_METRES = 600
/** How often the small map is redrawn, ms. */
const MINI_EVERY = 50
/** The backpack's circle pulses once in this many ms. */
const PULSE_MS = 1600

/**
 * The game's HUD, drawn by the renderer over the finished frame (`HudLayer`) rather than as page
 * elements: everything in `HudStats`, every frame. Menus, banners and the touch controls stay HTML.
 */
export class InGameHud {
  private readonly painter: Painter
  private readonly aim = new AimMarks()
  private readonly gauges = new Gauges()
  private readonly panels = new Panels()
  private readonly mini = new CanvasSprite(MINI_PX, MINI_PX)
  private map: HTMLImageElement | null = null

  constructor(layer: HudLayer) {
    this.painter = new Painter(layer)
  }

  /** Loads the world's map picture from `url`, once the world is known. */
  loadMap(url: string) {
    this.map = null
    void loadMap(url).then((image) => {
      this.map = image
    })
  }

  /** The world's map picture, once loaded (null without one). */
  get mapImage() {
    return this.map
  }

  /** Lays the HUD for `s` onto the layer, for a view `width` × `height` CSS pixels at `scale`, larger on a large screen. */
  draw(s: HudStats, now: number, width: number, height: number, scale: number) {
    const p = this.painter
    p.begin(width, height, scale, uiScale(width, height))
    this.aim.under(p, s, now)
    if (s.halfExtent > 0 && !s.touch) this.minimap(p, s, now)
    this.panels.draw(p, s, now)
    this.gauges.draw(p, s, now)
    this.aim.over(p, s, now)
  }

  private minimap(p: Painter, s: HudStats, now: number) {
    const image = this.map
    if (!image) return
    const size = miniSize(p.width, p.height)
    const step = Math.floor(now / MINI_EVERY)
    this.mini.update(step, (g) => {
      g.save()
      g.beginPath()
      g.roundRect(0, 0, MINI_PX, MINI_PX, 12)
      g.clip()
      paintMap(g, image, s.halfExtent, s.x, s.z, MINI_METRES, MINI_PX, { x: s.x, z: s.z, yaw: s.yaw, blips: s.blips, circle: s.backpack, pulse: (now % PULSE_MS) / PULSE_MS })
      g.restore()
    }, (p.scale * size) / MINI_PX)
    const x = p.width - 12 - size - 2
    const y = 12
    p.box(x - 2, y + 1, size + 8, size + 8, 18, 0x000000, 0.12)
    p.box(x, y, size + 4, size + 4, 14, 0xffffff, 0.9)
    p.image(this.mini.texture, x + 2, y + 2, size, size)
  }

  dispose() {
    this.aim.dispose()
    this.gauges.dispose()
    this.mini.dispose()
    this.painter.dispose()
  }
}
