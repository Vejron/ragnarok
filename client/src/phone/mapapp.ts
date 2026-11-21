/**
 * The phone's map app (PHONE.md): the world's map tiles round the reader, with the marks every map
 * shows, a scale bar and north. It follows the reader until dragged; a drag pans, the wheel or the
 * buttons zoom, and the centre button follows the reader again.
 */

import { paintMap, scaleLength, type MapMarks } from '@/hud/map'
import type { MapTiles } from '@/hud/maptiles'
import { APP_H, APP_TOP, INK, SCREEN_W, button, rounded, text, type Region } from './ui'

/** Pixels a metre at first: about 800 m across the screen, the neighbourhood round the reader. */
export const START_PER_METRE = SCREEN_W / 800
/** The zoom's limits, pixels a metre: from about 8 km across to about 130 m. */
export const MIN_PER_METRE = SCREEN_W / 8000
export const MAX_PER_METRE = SCREEN_W / 130
/** Each notch of the wheel, or tap of a zoom button, zooms by this much: about ten steps end to end. */
const ZOOM_STEP = 1.5625
/** The marks are drawn this much larger than on the minimap: the phone's pixels are small in view. */
const MARK_SCALE = 2.2

/** The zoom after `notches` (positive zooms out), within its limits. */
export function zoomed(perMetre: number, notches: number): number {
  return Math.max(MIN_PER_METRE, Math.min(MAX_PER_METRE, perMetre / ZOOM_STEP ** notches))
}

export class MapApp {
  perMetre = START_PER_METRE
  /** Following the reader, or held where a drag left it (`centre`). */
  follow = true
  centre: [number, number] = [0, 0]

  /** Where the map is centred now. */
  middle(marks: MapMarks): [number, number] {
    return this.follow ? [marks.x, marks.z] : this.centre
  }

  /** Pans by a drag of `(dx, dy)` screen pixels. */
  pan(dx: number, dy: number, marks: MapMarks) {
    const [cx, cz] = this.middle(marks)
    this.centre = [cx - dx / this.perMetre, cz - dy / this.perMetre]
    this.follow = false
  }

  zoom(notches: number) {
    this.perMetre = zoomed(this.perMetre, notches)
  }

  draw(g: CanvasRenderingContext2D, regions: Region[], tiles: MapTiles, marks: MapMarks, job: string | null) {
    const [cx, cz] = this.middle(marks)
    g.save()
    g.translate(0, APP_TOP)
    g.beginPath()
    g.rect(0, 0, SCREEN_W, APP_H)
    g.clip()
    paintMap(g, tiles, cx, cz, this.perMetre, SCREEN_W, APP_H, marks, MARK_SCALE)
    g.restore()

    // Where the job in hand sends the reader, along the top.
    if (job) {
      rounded(g, 16, APP_TOP + 16, SCREEN_W - 32, 64, 32, 'rgba(20, 163, 143, 0.95)')
      text(g, job, SCREEN_W / 2, APP_TOP + 49, 26, '#fff', { weight: 800, align: 'center', max: SCREEN_W - 64 })
    }

    // Zoom, and back to following the reader, up the right side, clear of the thumbs.
    const right = SCREEN_W - 24 - 72
    const zoomTop = APP_TOP + APP_H * 0.36
    button(g, regions, right, zoomTop, 72, 72, '+', () => this.zoom(-1), { fill: 'rgba(255, 255, 255, 0.94)', color: INK, size: 44 })
    button(g, regions, right, zoomTop + 84, 72, 72, '−', () => this.zoom(1), { fill: 'rgba(255, 255, 255, 0.94)', color: INK, size: 44 })
    if (!this.follow) button(g, regions, right, zoomTop + 168, 72, 72, '◎', () => (this.follow = true), { fill: 'rgba(255, 255, 255, 0.94)', color: INK, size: 40 })
    const bottom = APP_TOP + APP_H - 16

    // The scale bar, bottom left.
    const across = SCREEN_W / this.perMetre
    const length = scaleLength(across)
    const bar = length * this.perMetre
    const [bx, by] = [56, bottom]
    rounded(g, bx - 16, by - 64, bar + 32, 64, 14, 'rgba(255, 255, 255, 0.85)')
    g.fillStyle = INK
    g.fillRect(bx, by - 20, bar, 5)
    g.fillRect(bx, by - 30, 4, 15)
    g.fillRect(bx + bar - 4, by - 30, 4, 15)
    text(g, length >= 1000 ? `${length / 1000} km` : `${length} m`, bx + bar / 2, by - 44, 22, INK, { weight: 800, align: 'center' })

    // North, top right, under the job's line.
    const [nx, ny] = [SCREEN_W - 52, APP_TOP + (job ? 130 : 56)]
    g.fillStyle = INK
    g.beginPath()
    g.moveTo(nx, ny - 22)
    g.lineTo(nx + 13, ny + 14)
    g.lineTo(nx, ny + 5)
    g.lineTo(nx - 13, ny + 14)
    g.closePath()
    g.fill()
    text(g, 'N', nx, ny + 36, 24, INK, { weight: 800, align: 'center' })
  }
}
