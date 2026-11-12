import { distanceText, type Course } from './course'

/** Space kept round the graph inside its canvas, CSS pixels: left for the height labels, below for the kilometres. */
const PAD_LEFT = 34
const PAD_RIGHT = 10
const PAD_TOP = 8
const PAD_BOTTOM = 16

/**
 * The course's height profile, start to finish, drawn on a 2D canvas: the ground filled under the line,
 * the kilometres along the bottom, the lowest and highest heights at the side, and the skier as a
 * marker where they are along it.
 */
export class Profile {
  private readonly lowest: number
  private readonly highest: number

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly course: Course,
  ) {
    const heights = course.heights ?? [0]
    this.lowest = Math.min(...heights)
    this.highest = Math.max(...heights)
  }

  /** Whether the course has heights to draw. */
  get usable(): boolean {
    return (this.course.heights?.length ?? 0) === this.course.points.length
  }

  /** Draws the profile, with the skier `along` metres along the course if given. */
  draw(along: number | null = null) {
    const { canvas, course } = this
    const heights = course.heights
    if (!heights) return
    const ratio = window.devicePixelRatio || 1
    const width = canvas.clientWidth
    const height = canvas.clientHeight
    if (width === 0 || height === 0) return
    if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
      canvas.width = Math.round(width * ratio)
      canvas.height = Math.round(height * ratio)
    }
    const g = canvas.getContext('2d')!
    g.setTransform(ratio, 0, 0, ratio, 0, 0)
    g.clearRect(0, 0, width, height)

    const span = Math.max(10, this.highest - this.lowest)
    const bottom = this.lowest - span * 0.08
    const top = this.highest + span * 0.08
    const plotWidth = width - PAD_LEFT - PAD_RIGHT
    const plotHeight = height - PAD_TOP - PAD_BOTTOM
    const xOf = (metres: number) => PAD_LEFT + (metres / course.length) * plotWidth
    const yOf = (h: number) => PAD_TOP + (1 - (h - bottom) / (top - bottom)) * plotHeight

    // The ground under the line.
    g.beginPath()
    g.moveTo(xOf(0), PAD_TOP + plotHeight)
    course.along.forEach((metres, i) => g.lineTo(xOf(metres), yOf(heights[i]!)))
    g.lineTo(xOf(course.length), PAD_TOP + plotHeight)
    g.closePath()
    const fill = g.createLinearGradient(0, PAD_TOP, 0, PAD_TOP + plotHeight)
    fill.addColorStop(0, 'rgba(228,87,46,.55)')
    fill.addColorStop(1, 'rgba(228,87,46,.12)')
    g.fillStyle = fill
    g.fill()

    // The kilometres.
    g.font = '600 10px system-ui, sans-serif'
    g.textAlign = 'center'
    g.textBaseline = 'top'
    const every = course.length > 30_000 ? 5000 : course.length > 12_000 ? 2000 : 1000
    for (let metres = every; metres < course.length - every * 0.3; metres += every) {
      const x = xOf(metres)
      g.strokeStyle = 'rgba(255,255,255,.18)'
      g.beginPath()
      g.moveTo(x, PAD_TOP)
      g.lineTo(x, PAD_TOP + plotHeight)
      g.stroke()
      g.fillStyle = 'rgba(255,255,255,.75)'
      g.fillText(`${metres / 1000}`, x, PAD_TOP + plotHeight + 3)
    }
    g.textAlign = 'right'
    g.fillText('km', width - 2, PAD_TOP + plotHeight + 3)

    // The line itself.
    g.beginPath()
    course.along.forEach((metres, i) => (i === 0 ? g.moveTo : g.lineTo).call(g, xOf(metres), yOf(heights[i]!)))
    g.strokeStyle = '#e4572e'
    g.lineWidth = 2
    g.lineJoin = 'round'
    g.stroke()

    // The lowest and highest heights.
    g.fillStyle = 'rgba(255,255,255,.75)'
    g.textAlign = 'right'
    g.textBaseline = 'middle'
    g.fillText(`${Math.round(this.highest)} m`, PAD_LEFT - 5, yOf(this.highest))
    g.fillText(`${Math.round(this.lowest)} m`, PAD_LEFT - 5, yOf(this.lowest))

    if (along === null) return
    const at = Math.min(course.length, Math.max(0, along))
    const x = xOf(at)
    const y = yOf(this.heightAt(at))
    g.strokeStyle = '#fff'
    g.lineWidth = 1.5
    g.beginPath()
    g.moveTo(x, PAD_TOP)
    g.lineTo(x, PAD_TOP + plotHeight)
    g.stroke()
    g.fillStyle = '#fff'
    g.beginPath()
    g.arc(x, y, 4.5, 0, Math.PI * 2)
    g.fill()
    g.strokeStyle = '#e4572e'
    g.lineWidth = 2
    g.stroke()
    g.font = '700 11px system-ui, sans-serif'
    g.textBaseline = 'top'
    g.textAlign = x > width - 110 ? 'right' : 'left'
    g.fillText(`${distanceText(at)} · ${Math.round(this.heightAt(at))} m ö.h.`, x + (g.textAlign === 'right' ? -7 : 7), PAD_TOP)
  }

  /** The course's height `metres` along it. */
  private heightAt(metres: number): number {
    const { along } = this.course
    const heights = this.course.heights!
    let lo = 0
    let hi = along.length - 1
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1
      if (along[mid]! <= metres) lo = mid
      else hi = mid
    }
    const t = (metres - along[lo]!) / Math.max(1e-6, along[hi]! - along[lo]!)
    return heights[lo]! + (heights[hi]! - heights[lo]!) * Math.min(1, Math.max(0, t))
  }
}
