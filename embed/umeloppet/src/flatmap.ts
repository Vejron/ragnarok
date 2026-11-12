import { startIsFinish, type Course, type Facing } from './course'

/** The course line's colour, and the white it is edged with so it reads on any ground. */
export const COURSE_COLOUR = '#e4572e'
const EDGE = 'rgba(255,255,255,0.9)'
/** The ground where the world has no map image. */
const SNOW = '#eef2f5'

/** How close and how far the flat map zooms, metres across. */
const MIN_METRES = 150
const MAX_METRES = 20000

/** What is drawn over the map besides the course: the skier and the parked snowmobiles. */
export interface Marks {
  player?: Facing | null
  snowmobiles?: readonly Facing[]
}

/**
 * The world's top-down map image (`minimap.webp`, the importer's render of the whole square) with the
 * course drawn on it, in a 2D canvas: the small map in the corner of the 3D view, and the whole element
 * where there is no WebGPU, where it pans and zooms as the Leaflet map did.
 */
export class FlatMap {
  /** The world point at the middle of the canvas, metres. */
  cx = 0
  cz = 0
  /** How much ground the canvas shows across, metres. */
  metres = 1000
  private readonly abort = new AbortController()

  constructor(
    readonly canvas: HTMLCanvasElement,
    private readonly image: HTMLImageElement | null,
    private readonly halfExtent: number,
    private readonly course: Course,
  ) {}

  /** Centres the view on the whole course, with a margin. */
  fit() {
    let [minX, minZ, maxX, maxZ] = [Infinity, Infinity, -Infinity, -Infinity]
    for (const [x, z] of this.course.points) {
      minX = Math.min(minX, x)
      maxX = Math.max(maxX, x)
      minZ = Math.min(minZ, z)
      maxZ = Math.max(maxZ, z)
    }
    this.cx = (minX + maxX) / 2
    this.cz = (minZ + maxZ) / 2
    const { width, height } = this.size()
    const aspect = width / Math.max(1, height)
    this.metres = clampMetres(Math.max(maxX - minX, (maxZ - minZ) * aspect) * 1.15)
  }

  /** Lets the visitor drag the map about and zoom it with the wheel or two fingers. */
  interactive() {
    const signal = this.abort.signal
    const canvas = this.canvas
    const pointers = new Map<number, [number, number]>()
    let pinch = 0
    canvas.style.touchAction = 'none'
    canvas.addEventListener('pointerdown', (event) => {
      canvas.setPointerCapture(event.pointerId)
      pointers.set(event.pointerId, [event.clientX, event.clientY])
      pinch = 0
    }, { signal })
    canvas.addEventListener('pointermove', (event) => {
      const was = pointers.get(event.pointerId)
      if (!was) return
      pointers.set(event.pointerId, [event.clientX, event.clientY])
      const scale = this.metres / Math.max(1, this.size().width)
      if (pointers.size === 1) {
        this.cx -= (event.clientX - was[0]) * scale
        this.cz -= (event.clientY - was[1]) * scale
      } else if (pointers.size === 2) {
        const [a, b] = [...pointers.values()] as [[number, number], [number, number]]
        const span = Math.hypot(a[0] - b[0], a[1] - b[1])
        if (pinch > 0) this.metres = clampMetres(this.metres * (pinch / Math.max(1, span)))
        pinch = span
      }
      this.draw()
    }, { signal })
    const lift = (event: PointerEvent) => {
      pointers.delete(event.pointerId)
      pinch = 0
    }
    canvas.addEventListener('pointerup', lift, { signal })
    canvas.addEventListener('pointercancel', lift, { signal })
    canvas.addEventListener('wheel', (event) => {
      event.preventDefault()
      // Zoom about the point under the pointer, so it stays put.
      const rect = canvas.getBoundingClientRect()
      const [wx, wz] = this.toWorld(event.clientX - rect.left, event.clientY - rect.top)
      const metres = clampMetres(this.metres * Math.exp(event.deltaY * 0.0015))
      const k = metres / this.metres
      this.cx = wx + (this.cx - wx) * k
      this.cz = wz + (this.cz - wz) * k
      this.metres = metres
      this.draw()
    }, { signal, passive: false })
  }

  /** CSS pixels across and down. */
  private size() {
    return { width: this.canvas.clientWidth || this.canvas.width, height: this.canvas.clientHeight || this.canvas.height }
  }

  /** The world point under CSS pixel `(px, py)`. */
  private toWorld(px: number, py: number): [number, number] {
    const { width, height } = this.size()
    const scale = this.metres / Math.max(1, width)
    return [this.cx + (px - width / 2) * scale, this.cz + (py - height / 2) * scale]
  }

  /** Redraws the map, the course and `marks`. */
  draw(marks: Marks = {}) {
    const canvas = this.canvas
    const { width, height } = this.size()
    const ratio = Math.min(2, globalThis.devicePixelRatio || 1)
    const w = Math.max(1, Math.round(width * ratio))
    const h = Math.max(1, Math.round(height * ratio))
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w
      canvas.height = h
    }
    const g = canvas.getContext('2d')
    if (!g) return
    g.setTransform(ratio, 0, 0, ratio, 0, 0)
    g.fillStyle = SNOW
    g.fillRect(0, 0, width, height)
    const scale = width / this.metres
    const at = (x: number, z: number): [number, number] => [width / 2 + (x - this.cx) * scale, height / 2 + (z - this.cz) * scale]
    if (this.image?.complete && this.image.naturalWidth > 0) {
      const [left, top] = at(-this.halfExtent, -this.halfExtent)
      const side = 2 * this.halfExtent * scale
      g.imageSmoothingEnabled = true
      g.drawImage(this.image, left, top, side, side)
    }

    const line = () => {
      g.beginPath()
      this.course.points.forEach(([x, z], i) => {
        const [px, py] = at(x, z)
        if (i === 0) g.moveTo(px, py)
        else g.lineTo(px, py)
      })
    }
    g.lineJoin = 'round'
    g.lineCap = 'round'
    line()
    g.strokeStyle = EDGE
    g.lineWidth = 7
    g.stroke()
    line()
    g.strokeStyle = COURSE_COLOUR
    g.lineWidth = 3.5
    g.stroke()

    // A kilometre's number only once there is room for it.
    const labelled = this.metres < 6000
    g.font = '600 11px system-ui, sans-serif'
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    for (const { x, z, km } of this.course.km) {
      const [px, py] = at(x, z)
      g.beginPath()
      g.arc(px, py, labelled ? 9 : 4, 0, Math.PI * 2)
      g.fillStyle = '#fff'
      g.fill()
      g.lineWidth = 2
      g.strokeStyle = COURSE_COLOUR
      g.stroke()
      if (labelled) {
        g.fillStyle = '#222'
        g.fillText(String(km), px, py + 0.5)
      }
    }
    const { start, finish } = this.course
    if (startIsFinish(this.course)) this.flag(g, at(start.x, start.z), 'START · MÅL', '#1f8a4c')
    else {
      this.flag(g, at(finish.x, finish.z), 'MÅL', '#222')
      this.flag(g, at(start.x, start.z), 'START', '#1f8a4c')
    }

    for (const sled of marks.snowmobiles ?? []) {
      const [px, py] = at(sled.x, sled.z)
      g.fillStyle = '#2b6cb0'
      g.strokeStyle = '#fff'
      g.lineWidth = 1.5
      g.beginPath()
      g.rect(px - 4, py - 4, 8, 8)
      g.fill()
      g.stroke()
    }
    if (marks.player) this.arrow(g, at(marks.player.x, marks.player.z), marks.player.yaw)
  }

  /** A flag with `text` on it, standing on `(px, py)`. */
  private flag(g: CanvasRenderingContext2D, [px, py]: [number, number], text: string, colour: string) {
    g.font = '700 10px system-ui, sans-serif'
    const w = g.measureText(text).width + 8
    g.fillStyle = colour
    g.strokeStyle = '#fff'
    g.lineWidth = 1.5
    g.beginPath()
    g.moveTo(px, py)
    g.lineTo(px, py - 22)
    g.stroke()
    g.fillRect(px, py - 22, w, 13)
    g.fillStyle = '#fff'
    g.textAlign = 'left'
    g.fillText(text, px + 4, py - 15)
    g.textAlign = 'center'
    g.beginPath()
    g.arc(px, py, 3, 0, Math.PI * 2)
    g.fillStyle = colour
    g.fill()
    g.stroke()
  }

  /** The skier: an arrow pointing the way they face (`yaw`, 0 is north, up the map). */
  private arrow(g: CanvasRenderingContext2D, [px, py]: [number, number], yaw: number) {
    g.save()
    g.translate(px, py)
    g.rotate(-yaw)
    g.beginPath()
    g.moveTo(0, -9)
    g.lineTo(6, 7)
    g.lineTo(0, 3.5)
    g.lineTo(-6, 7)
    g.closePath()
    g.fillStyle = '#ffd400'
    g.strokeStyle = '#222'
    g.lineWidth = 1.5
    g.fill()
    g.stroke()
    g.restore()
  }

  dispose() {
    this.abort.abort()
  }
}

const clampMetres = (metres: number) => Math.min(MAX_METRES, Math.max(MIN_METRES, metres))

/** Loads the world's map image, `minimap.webp` or else `minimap.png`; null if it has neither. */
export async function loadMapImage(base: string): Promise<HTMLImageElement | null> {
  for (const name of ['minimap.webp', 'minimap.png']) {
    const image = new Image()
    image.crossOrigin = 'anonymous'
    image.src = `${base}${name}`
    try {
      await image.decode()
      return image
    } catch {
      // Try the next.
    }
  }
  return null
}
