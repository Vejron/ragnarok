/**
 * The race course, as `scripts/course.ts` writes it from the GPX into `course.json`: world metres
 * (+X east, +Z south), the same frame as the world it was made for.
 */

/** A place on the course and which way it faces there, in the soldier's yaw (0 faces -Z, north). */
export interface Facing {
  x: number
  z: number
  yaw: number
}

export interface Course {
  /** What the course is called, for the readout. */
  name: string
  /** Total length along the line, metres. */
  length: number
  /** The centre line, about every 5 m, start to finish. */
  points: [number, number][]
  /** Distance along the line at each point, metres. */
  along: number[]
  /** Height of the world's ground at each point, metres (the height model the world was built from); absent in an older course file. */
  heights?: number[]
  /** A marker at each whole kilometre: where it stands and which kilometre it is. */
  km: { x: number; z: number; km: number }[]
  start: Facing
  finish: Facing
  /** Where the snowmobiles are parked. */
  snowmobiles: Facing[]
}

/** Start and finish nearer than this, metres, are one place: a loop, with one arch and one flag. */
const SAME_PLACE = 25

/** Whether the course ends where it starts. */
export function startIsFinish({ start, finish }: Course): boolean {
  return Math.hypot(start.x - finish.x, start.z - finish.z) < SAME_PLACE
}

/** Fetches and checks `course.json`. */
export async function loadCourse(url: string): Promise<Course> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)
  const course = (await response.json()) as Course
  if (!Array.isArray(course.points) || course.points.length < 2) throw new Error(`${url} has no course line`)
  return course
}

/** Nearer than this, metres, the skier is on the course; further, the readout says how far off they are. */
const ON_COURSE = 40
/** How many points either side of the last one are searched first, so a course that runs back past itself does not jump. */
const WINDOW = 60

/**
 * Follows a skier along the course: how far along they are, from the nearest point of the line, looked
 * for near where they were last so a course that passes itself keeps its order.
 */
export class Progress {
  private index = 0

  constructor(private readonly course: Course) {}

  /** Metres along the course at `(x, z)`, and how far off the line it is, metres. */
  at(x: number, z: number): { along: number; off: number } {
    const { points, along } = this.course
    const nearest = (from: number, to: number) => {
      let best = -1
      let bestDistance = Infinity
      for (let i = Math.max(0, from); i < Math.min(points.length - 1, to); i++) {
        const [ax, az] = points[i]!
        const [bx, bz] = points[i + 1]!
        const t = clamp01(((x - ax) * (bx - ax) + (z - az) * (bz - az)) / Math.max(1e-6, (bx - ax) ** 2 + (bz - az) ** 2))
        const d = Math.hypot(ax + (bx - ax) * t - x, az + (bz - az) * t - z)
        if (d < bestDistance) {
          bestDistance = d
          best = i + t
        }
      }
      return { best, bestDistance }
    }
    let found = nearest(this.index - WINDOW, this.index + WINDOW)
    if (found.bestDistance > ON_COURSE) found = nearest(0, points.length)
    this.index = Math.floor(found.best)
    const i = Math.floor(found.best)
    const t = found.best - i
    const a = along[i] ?? 0
    const b = along[i + 1] ?? a
    return { along: a + (b - a) * t, off: found.bestDistance }
  }
}

/** Half the width of the start and finish gates a skier is timed through, metres: the arch and a little either side. */
const GATE_HALF_WIDTH = 5
/** A run is finished only after this share of the course has been passed, so a lap of the start area is not one. */
const FINISH_AFTER = 0.8

/** Where the race clock is: before the start, running, or stopped at the finish. */
export interface ClockState {
  state: 'ready' | 'running' | 'finished'
  /** Time since the start gate, or the finishing time, seconds. */
  seconds: number
  /** A snowmobile was ridden during the run, so the time is not a skier's. */
  sled: boolean
}

/**
 * The race clock: started by going forwards through the start gate, stopped by going forwards
 * through the finish gate once most of the course has been passed. Going through the start again
 * starts it over.
 */
export class RaceClock {
  private startedAt = 0
  private stoppedAt = 0
  private state: ClockState['state'] = 'ready'
  private furthest = 0
  private sled = false

  constructor(private readonly course: Course) {}

  /**
   * The skier moved from `(px, pz)` to `(x, z)`, `along` metres along the course, at `seconds` of
   * play, on a snowmobile or not.
   */
  update(px: number, pz: number, x: number, z: number, along: number, seconds: number, riding: boolean) {
    const { start, finish } = this.course
    if (this.state === 'running') {
      this.furthest = Math.max(this.furthest, along)
      this.sled ||= riding
      if (this.furthest >= this.course.length * FINISH_AFTER && through(finish, px, pz, x, z)) {
        this.state = 'finished'
        this.stoppedAt = seconds
        return
      }
    }
    if (through(start, px, pz, x, z) && (this.state !== 'running' || this.furthest < this.course.length * FINISH_AFTER)) {
      this.state = 'running'
      this.startedAt = seconds
      this.furthest = 0
      this.sled = riding
    }
  }

  read(seconds: number): ClockState {
    const time = this.state === 'ready' ? 0 : (this.state === 'finished' ? this.stoppedAt : seconds) - this.startedAt
    return { state: this.state, seconds: time, sled: this.sled }
  }
}

/** Whether going from `(px, pz)` to `(x, z)` passes forwards through the gate at `gate`, across its heading. */
function through(gate: Facing, px: number, pz: number, x: number, z: number): boolean {
  const fx = -Math.sin(gate.yaw)
  const fz = -Math.cos(gate.yaw)
  const before = (px - gate.x) * fx + (pz - gate.z) * fz
  const after = (x - gate.x) * fx + (z - gate.z) * fz
  if (!(before < 0 && after >= 0)) return false
  const t = before / (before - after)
  const cx = px + (x - px) * t - gate.x
  const cz = pz + (z - pz) * t - gate.z
  return Math.abs(cx * -fz + cz * fx) <= GATE_HALF_WIDTH
}

/** `seconds` as a race time: "4:07,3", or "1:02:33" past an hour. */
export function timeText(seconds: number): string {
  const whole = Math.floor(seconds)
  const h = Math.floor(whole / 3600)
  const m = Math.floor((whole % 3600) / 60)
  const s = whole % 60
  const pad = (v: number) => String(v).padStart(2, '0')
  if (h > 0) return `${h}:${pad(m)}:${pad(s)}`
  return `${m}:${pad(s)},${Math.floor((seconds - whole) * 10)}`
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))

/** `metres` as the readout shows a distance: "3,4 km", or "650 m" under a kilometre. Swedish decimals. */
export function distanceText(metres: number): string {
  if (metres < 1000) return `${Math.round(metres)} m`
  return `${(metres / 1000).toFixed(1).replace('.', ',')} km`
}
