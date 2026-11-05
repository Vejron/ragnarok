import type { Blip } from '@/blips'
import { circleOnSmall } from '@/sim/minimap'

/** The paper colour of `minimap.png`, the importer's `LAND`. */
export const PAPER = '#e9e5d9'
export const ARROW = '#ff5a5f'
const MODEM = (alpha: number) => `rgba(255, 140, 0, ${alpha})`

/** What a map shows on top of the picture: who reads it, the marks, and the backpack's circle. */
export interface MapMarks {
  /** The reader, metres (x east, z south), and where they look, radians (camera yaw). */
  x: number
  z: number
  yaw: number
  blips: readonly Blip[]
  circle: { x: number; z: number; radius: number } | null
  /** 0..1, for the pulse of the backpack's circle. */
  pulse: number
}

/** `minimap.png` from `url`, or null when the world has none. */
export function loadMap(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const image = new Image()
    image.onload = () => resolve(image)
    image.onerror = () => resolve(null)
    image.src = url
  })
}

/**
 * The square of the map image `metres` across centred on world (`cx`, `cz`), as a source rectangle in
 * image pixels: it may run off the image, where nothing is drawn.
 */
export function mapWindow(cx: number, cz: number, metres: number, halfExtent: number, imageSize: number) {
  const perMetre = imageSize / (2 * halfExtent)
  const side = metres * perMetre
  return { sx: (cx - metres / 2 + halfExtent) * perMetre, sy: (cz - metres / 2 + halfExtent) * perMetre, side }
}

/**
 * Draws `metres` of the map centred on (`cx`, `cz`) into a `size`-px square of `g`, with the reader's
 * arrow, the blips and the backpack's circle on top. `scale` sizes the marks (1 on the small map).
 */
export function paintMap(g: CanvasRenderingContext2D, image: HTMLImageElement, halfExtent: number, cx: number, cz: number, metres: number, size: number, marks: MapMarks, scale = 1) {
  g.fillStyle = PAPER
  g.fillRect(0, 0, size, size)
  const { sx, sy, side } = mapWindow(cx, cz, metres, halfExtent, image.naturalWidth)
  // Clip the source to the image, and the destination with it, so no browser stretches an edge.
  const k = size / side
  const x0 = Math.max(0, sx)
  const y0 = Math.max(0, sy)
  const x1 = Math.min(image.naturalWidth, sx + side)
  const y1 = Math.min(image.naturalHeight, sy + side)
  if (x1 > x0 && y1 > y0) {
    g.imageSmoothingQuality = 'high'
    g.drawImage(image, x0, y0, x1 - x0, y1 - y0, (x0 - sx) * k, (y0 - sy) * k, (x1 - x0) * k, (y1 - y0) * k)
  }
  const perMetre = size / metres
  const at = (x: number, z: number): [number, number] => [size / 2 + (x - cx) * perMetre, size / 2 + (z - cz) * perMetre]
  const glow = 1 - 0.45 * (0.5 - 0.5 * Math.cos(marks.pulse * Math.PI * 2))

  const circle = marks.circle
  if (circle) {
    const { left, top, r, pip } = circleOnSmall(circle.x, circle.z, circle.radius, cx, cz, metres, size)
    g.globalAlpha = glow
    if (pip) {
      disc(g, pip.left, pip.top, 5 * scale, MODEM(1), 1.5 * scale)
    } else {
      g.beginPath()
      g.arc(left, top, Math.max(1, r - scale), 0, Math.PI * 2)
      g.fillStyle = MODEM(0.18)
      g.fill()
      g.lineWidth = 2 * scale
      g.strokeStyle = MODEM(0.95)
      g.stroke()
    }
    g.globalAlpha = 1
  }

  for (const blip of marks.blips) {
    let [bx, by] = at(blip.x, blip.z)
    const edge = 4 * scale
    if (blip.kind === 'ball') {
      // The player's golf ball: kept on the edge, the way to it, when it is off the map.
      const inset = 7 * scale
      ;[bx, by] = [Math.max(inset, Math.min(size - inset, bx)), Math.max(inset, Math.min(size - inset, by))]
      golfBall(g, bx, by, scale)
      continue
    }
    if (bx < edge || by < edge || bx > size - edge || by > size - edge) continue
    g.globalAlpha = blip.kind === 'ally' ? 1 : 0.25 + 0.75 * blip.strength
    if (blip.kind === 'shot') {
      g.save()
      g.translate(bx, by)
      g.rotate(Math.PI / 4)
      g.fillStyle = '#fff'
      g.fillRect(-6 * scale, -6 * scale, 12 * scale, 12 * scale)
      g.fillStyle = blip.color
      g.fillRect(-4.5 * scale, -4.5 * scale, 9 * scale, 9 * scale)
      g.restore()
    } else disc(g, bx, by, 4.5 * scale, blip.color, 1.5 * scale)
    g.globalAlpha = 1
  }

  const [ax, ay] = at(marks.x, marks.z)
  arrow(g, ax, ay, -marks.yaw, scale)
}

/** The reader's arrow: a red triangle pointing where they look (up, north, at yaw 0), edged in white. */
export function arrow(g: CanvasRenderingContext2D, x: number, y: number, turn: number, scale = 1) {
  g.save()
  g.translate(x, y)
  g.rotate(turn)
  g.beginPath()
  g.moveTo(0, -8 * scale)
  g.lineTo(7 * scale, 8 * scale)
  g.lineTo(-7 * scale, 8 * scale)
  g.closePath()
  g.lineJoin = 'round'
  g.lineWidth = 2.5 * scale
  g.strokeStyle = '#fff'
  g.stroke()
  g.fillStyle = ARROW
  g.fill()
  g.restore()
}

/** A golf ball's mark: white, ringed dark so it shows on the paper's pale greens. */
function golfBall(g: CanvasRenderingContext2D, x: number, y: number, scale: number) {
  g.beginPath()
  g.arc(x, y, 5 * scale, 0, Math.PI * 2)
  g.fillStyle = '#ffffff'
  g.fill()
  g.lineWidth = 2 * scale
  g.strokeStyle = '#1d2b36'
  g.stroke()
}

function disc(g: CanvasRenderingContext2D, x: number, y: number, r: number, color: string, edge: number) {
  g.beginPath()
  g.arc(x, y, r + edge, 0, Math.PI * 2)
  g.fillStyle = '#fff'
  g.fill()
  g.beginPath()
  g.arc(x, y, r, 0, Math.PI * 2)
  g.fillStyle = color
  g.fill()
}

/** How much ground the held paper map shows across at first, metres, unless the world is smaller. */
export const PAPER_METRES = 500
/** How far the wheel zooms it in and out, metres across. */
export const PAPER_MIN = 200
export const PAPER_MAX = 1500
/** Each notch of the wheel zooms by this much. */
const PAPER_STEP = 1.25

/** The paper map's span after `notches` of the wheel (positive zooms out), within its limits. */
export function zoomPaper(metres: number, notches: number): number {
  return Math.max(PAPER_MIN, Math.min(PAPER_MAX, metres * PAPER_STEP ** notches))
}

/**
 * The paper map's view: `metres` across (at most the world's side), centred on the reader but kept
 * inside the world, so its edge is the sheet's edge and the reader walks across a fixed sheet near it.
 */
export function paperView(x: number, z: number, halfExtent: number, metres = PAPER_METRES) {
  const across = Math.min(metres, 2 * halfExtent)
  const reach = halfExtent - across / 2
  const clamp = (v: number) => Math.max(-reach, Math.min(reach, v))
  return { cx: clamp(x), cz: clamp(z), metres: across }
}

/** A round length for a scale bar about a fifth of `metres` across. */
export function scaleLength(metres: number): number {
  const target = metres / 5
  const steps = [10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000]
  return steps.reduce((best, step) => (Math.abs(step - target) < Math.abs(best - target) ? step : best), steps[0]!)
}

/**
 * The held paper map in a `size`-px square: `span` metres of the picture round the reader (`paperView`), the marks,
 * a north arrow and a scale bar in the corners. Without a picture, plain paper that says so.
 */
export function paintPaper(g: CanvasRenderingContext2D, image: HTMLImageElement | null, halfExtent: number, size: number, marks: MapMarks, span = PAPER_METRES) {
  if (!image || halfExtent <= 0) {
    g.fillStyle = PAPER
    g.fillRect(0, 0, size, size)
    g.fillStyle = 'rgba(29, 43, 54, 0.5)'
    g.font = `700 ${size / 20}px ui-rounded, system-ui, sans-serif`
    g.textAlign = 'center'
    g.fillText('No map of this area', size / 2, size / 2)
    return
  }
  const { cx, cz, metres } = paperView(marks.x, marks.z, halfExtent, span)
  const scale = size / 360
  paintMap(g, image, halfExtent, cx, cz, metres, size, marks, scale)
  const ink = '#1d2b36'
  g.fillStyle = 'rgba(255, 255, 255, 0.8)'
  // The scale bar, bottom left.
  const length = scaleLength(metres)
  const barPx = (length / metres) * size
  const pad = size * 0.04
  g.fillRect(pad - 6 * scale, size - pad - 22 * scale, barPx + 12 * scale, 28 * scale)
  g.fillStyle = ink
  g.fillRect(pad, size - pad - 2 * scale, barPx, 3 * scale)
  g.fillRect(pad, size - pad - 8 * scale, 2 * scale, 9 * scale)
  g.fillRect(pad + barPx - 2 * scale, size - pad - 8 * scale, 2 * scale, 9 * scale)
  g.font = `700 ${11 * scale}px ui-rounded, system-ui, sans-serif`
  g.textAlign = 'center'
  g.fillText(length >= 1000 ? `${length / 1000} km` : `${length} m`, pad + barPx / 2, size - pad - 9 * scale)
  // North, top right.
  const nx = size - pad - 10 * scale
  const ny = pad + 14 * scale
  g.beginPath()
  g.moveTo(nx, ny - 12 * scale)
  g.lineTo(nx + 7 * scale, ny + 8 * scale)
  g.lineTo(nx, ny + 3 * scale)
  g.lineTo(nx - 7 * scale, ny + 8 * scale)
  g.closePath()
  g.fill()
  g.fillText('N', nx, ny + 22 * scale)
}
