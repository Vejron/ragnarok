import type { Blip } from '@/blips'
import { circleOnMap } from '@/sim/minimap'
import type { MapTiles } from './maptiles'
import { INK_CSS } from './paint'

/** The paper colour of the map picture, the importer's `LAND`. */
export const PAPER = '#e9e5d9'
export const ARROW = '#ff5a5f'
/** The minimap's side on a view `width` × `height` layout pixels: 180 on a desktop, down to 110 on a phone. */
export const miniSize = (width: number, height: number) => Math.round(Math.max(110, Math.min(180, Math.min(width, height) * 0.3)))
/** How far down the minimap's frame reaches, from the top of the screen. */
export const miniBottom = (width: number, height: number) => 12 + miniSize(width, height) + 4

/** The colour of a fuel station's mark: its sign's (`render/fuelstations.ts`). */
const FUEL_MARK = '#d8342b'
/** The colour of a vehicle dealer's mark: its sign's (`render/dealers.ts`). */
const DEALER_MARK = '#1f5fbf'

const MODEM = (alpha: number) => `rgba(255, 140, 0, ${alpha})`

/** What a map shows on top of the picture: who reads it, the marks, and the backpack's circle. */
export interface MapMarks {
  /** The reader, metres (x east, z south), and where they look, radians (camera yaw). */
  x: number
  z: number
  yaw: number
  blips: readonly Blip[]
  circle: { x: number; z: number; radius: number } | null
  /** Where the delivery job in hand sends the reader (DELIVERY.md), if anywhere. */
  target?: { x: number; z: number } | null
  /** The world's fuel stations, metres (FUEL.md). */
  stations?: readonly (readonly [number, number])[]
  /** The world's vehicle dealers, metres (DEALERS.md). */
  dealers?: readonly (readonly [number, number])[]
  /** 0..1, for the pulse of the backpack's circle. */
  pulse: number
}

/** The waypoint's colour: the courier's bag's (`render/parcel.ts`). */
const WAYPOINT = '#14a38f'

/** The importer's picture of the whole world, by name: the overview the settings page shows. The game's maps are tiles (`maptiles.ts`). */
export const MAP_FILE = 'minimap.webp'


/**
 * Draws the map at `perMetre` pixels a metre centred on (`cx`, `cz`) into a `width` × `height` px
 * rectangle of `g`, from `tiles`, with the reader's arrow, the blips, the backpack's circle and the
 * delivery's pin on top. `scale` sizes the marks (1 on the small map).
 */
export function paintMap(g: CanvasRenderingContext2D, tiles: MapTiles, cx: number, cz: number, perMetre: number, width: number, height: number, marks: MapMarks, scale = 1) {
  g.fillStyle = PAPER
  g.fillRect(0, 0, width, height)
  tiles.draw(g, cx, cz, perMetre, width, height)
  const at = (x: number, z: number): [number, number] => [width / 2 + (x - cx) * perMetre, height / 2 + (z - cz) * perMetre]
  const glow = 1 - 0.45 * (0.5 - 0.5 * Math.cos(marks.pulse * Math.PI * 2))

  const circle = marks.circle
  if (circle) {
    const { left, top, r, pip } = circleOnMap(circle.x, circle.z, circle.radius, cx, cz, perMetre, width, height)
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

  if (marks.target) {
    // A pin on the spot, or a pip on the map's edge pointing the way to it.
    const { left, top, pip } = circleOnMap(marks.target.x, marks.target.z, 0, cx, cz, perMetre, width, height)
    if (pip) disc(g, pip.left, pip.top, 5 * scale, WAYPOINT, 1.5 * scale)
    else {
      g.beginPath()
      g.arc(left, top, (7 + 4 * glow) * scale, 0, Math.PI * 2)
      g.lineWidth = 2 * scale
      g.strokeStyle = WAYPOINT
      g.stroke()
      disc(g, left, top, 4.5 * scale, WAYPOINT, 1.5 * scale)
    }
  }

  for (const [x, z] of marks.stations ?? []) {
    const [sx, sy] = at(x, z)
    const edge = 6 * scale
    if (sx < edge || sy < edge || sx > width - edge || sy > height - edge) continue
    pump(g, sx, sy, scale)
  }

  for (const [x, z] of marks.dealers ?? []) {
    const [sx, sy] = at(x, z)
    const edge = 6 * scale
    if (sx < edge || sy < edge || sx > width - edge || sy > height - edge) continue
    car(g, sx, sy, scale)
  }

  for (const blip of marks.blips) {
    let [bx, by] = at(blip.x, blip.z)
    const edge = 4 * scale
    if (blip.kind === 'ball') {
      // The player's golf ball: kept on the edge, the way to it, when it is off the map.
      const inset = 7 * scale
      ;[bx, by] = [Math.max(inset, Math.min(width - inset, bx)), Math.max(inset, Math.min(height - inset, by))]
      golfBall(g, bx, by, scale)
      continue
    }
    if (bx < edge || by < edge || bx > width - edge || by > height - edge) continue
    g.globalAlpha = blip.kind === 'shot' ? 0.25 + 0.75 * blip.strength : 1
    if (blip.kind === 'police' || blip.kind === 'ambulance') {
      // A square, upright, where an ally is a disc and a shot a diamond.
      g.fillStyle = '#fff'
      g.fillRect(bx - 5.5 * scale, by - 5.5 * scale, 11 * scale, 11 * scale)
      g.fillStyle = blip.color
      g.fillRect(bx - 4 * scale, by - 4 * scale, 8 * scale, 8 * scale)
    } else if (blip.kind === 'shot') {
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
  g.strokeStyle = INK_CSS
  g.stroke()
}

/** A fuel station's mark: a red pump on a white tile. */
function pump(g: CanvasRenderingContext2D, x: number, y: number, scale: number) {
  g.fillStyle = '#fff'
  g.fillRect(x - 6 * scale, y - 6 * scale, 12 * scale, 12 * scale)
  g.fillStyle = FUEL_MARK
  g.fillRect(x - 4.5 * scale, y - 4.5 * scale, 9 * scale, 9 * scale)
  // The pump's window and its hose.
  g.fillStyle = '#fff'
  g.fillRect(x - 2.5 * scale, y - 3 * scale, 3.5 * scale, 2.5 * scale)
  g.fillRect(x + 2 * scale, y - 1.5 * scale, 1.2 * scale, 4.5 * scale)
}

/** A vehicle dealer's mark: a white car seen side on, on its sign's blue. */
function car(g: CanvasRenderingContext2D, x: number, y: number, scale: number) {
  g.fillStyle = '#fff'
  g.fillRect(x - 6 * scale, y - 6 * scale, 12 * scale, 12 * scale)
  g.fillStyle = DEALER_MARK
  g.fillRect(x - 4.5 * scale, y - 4.5 * scale, 9 * scale, 9 * scale)
  g.fillStyle = '#fff'
  // The body, the cabin over it and the two wheels under it.
  g.fillRect(x - 3.5 * scale, y - 0.5 * scale, 7 * scale, 2 * scale)
  g.fillRect(x - 2 * scale, y - 2.5 * scale, 3.5 * scale, 2 * scale)
  g.fillRect(x - 2.5 * scale, y + 1.5 * scale, 1.5 * scale, 1.5 * scale)
  g.fillRect(x + 1 * scale, y + 1.5 * scale, 1.5 * scale, 1.5 * scale)
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

/** A round length for a scale bar about a fifth of `metres` across. */
export function scaleLength(metres: number): number {
  const target = metres / 5
  const steps = [10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000]
  return steps.reduce((best, step) => (Math.abs(step - target) < Math.abs(best - target) ? step : best), steps[0]!)
}
