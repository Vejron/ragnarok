import type * as THREE from 'three'
import { physicsClock } from '../sim/physics'

/** Samples kept for the averages: about two seconds at 60 fps. */
const WINDOW = 120
/** How often the readout's text is rewritten, ms. Rewriting it every frame would cost what it measures. */
const REFRESH_MS = 250

/** One frame's CPU timings, ms. `physics` is time in Rapier since the previous frame (it includes snapshot handling). */
interface Sample {
  interval: number
  update: number
  render: number
  physics: number
  calls: number
  ticks: number
}

/**
 * A debug overlay for frame cost: frame rate, where the main-thread time goes (game update, renderer
 * submit, Rapier), and what the renderer drew. Shown with F3, or always with `?perf` in the URL.
 * The readout is a DOM element outside Vue, so it does not add reactivity to the HUD.
 */
export class PerfReadout {
  private readonly el: HTMLDivElement
  private readonly samples: Sample[] = []
  private visible = new URLSearchParams(location.search).has('perf')
  private lastFrame: number | null = null
  private lastRefresh = 0
  private frameStart = 0
  private pendingInterval = 0
  private renderStart = 0
  private update = 0
  private readonly onKey = (event: KeyboardEvent) => {
    if (event.code !== 'F3') return
    event.preventDefault()
    this.visible = !this.visible
    this.el.style.display = this.visible ? 'block' : 'none'
    this.lastRefresh = 0
  }

  constructor(private readonly renderer: THREE.WebGLRenderer) {
    // Counts are read per frame, so they are summed over every render pass in it (the AO composer has several).
    renderer.info.autoReset = false
    this.el = document.createElement('div')
    this.el.setAttribute('aria-hidden', 'true')
    this.el.style.cssText = [
      'position:fixed', 'top:8px', 'right:8px', 'z-index:1000', 'pointer-events:none',
      'font:12px/1.4 ui-monospace,Menlo,monospace', 'color:#e8e8e8', 'background:rgba(0,0,0,0.6)',
      'padding:6px 8px', 'border-radius:4px', 'white-space:pre', `display:${this.visible ? 'block' : 'none'}`,
    ].join(';')
    document.body.appendChild(this.el)
    window.addEventListener('keydown', this.onKey)
  }

  /** Call first thing in each animation frame. Resets the per-frame counters. */
  begin(now: number) {
    this.frameStart = performance.now()
    this.renderer.info.reset()
    physicsClock.ms = 0
    physicsClock.calls = 0
    this.lastFrame = this.lastFrame ?? now
    this.pendingInterval = now - this.lastFrame
    this.lastFrame = now
  }

  /** Call just before the scene is rendered. `update` is the game's own time so far this frame. */
  beforeRender() {
    this.renderStart = performance.now()
    this.update = this.renderStart - this.frameStart
  }

  /** Call after the frame's rendering. `ticks` is how many fixed simulation ticks ran this frame. */
  end(ticks: number) {
    const now = performance.now()
    this.samples.push({
      interval: this.pendingInterval,
      update: this.update,
      render: now - this.renderStart,
      physics: physicsClock.ms,
      calls: physicsClock.calls,
      ticks,
    })
    if (this.samples.length > WINDOW) this.samples.shift()
    if (this.visible && now - this.lastRefresh >= REFRESH_MS) {
      this.lastRefresh = now
      this.el.textContent = this.text()
    }
  }

  private text() {
    const n = this.samples.length || 1
    const avg = (pick: (s: Sample) => number) => this.samples.reduce((sum, s) => sum + pick(s), 0) / n
    const interval = avg((s) => s.interval)
    const worst = this.samples.reduce((max, s) => Math.max(max, s.interval), 0)
    const { info } = this.renderer
    const triangles = info.render.triangles
    return [
      `fps ${interval > 0 ? (1000 / interval).toFixed(0) : '-'}   frame ${interval.toFixed(1)} ms  (worst ${worst.toFixed(1)})`,
      `game ${avg((s) => s.update).toFixed(2)} ms   render ${avg((s) => s.render).toFixed(2)} ms`,
      `physics ${avg((s) => s.physics).toFixed(2)} ms   ${avg((s) => s.calls).toFixed(0)} calls   ticks/frame ${avg((s) => s.ticks).toFixed(2)}`,
      `draw calls ${info.render.calls}   triangles ${(triangles / 1000).toFixed(1)}k`,
      `geometries ${info.memory.geometries}   textures ${info.memory.textures}   programs ${info.programs?.length ?? 0}`,
    ].join('\n')
  }

  dispose() {
    window.removeEventListener('keydown', this.onKey)
    this.el.remove()
  }
}
