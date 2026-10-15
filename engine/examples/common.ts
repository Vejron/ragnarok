import { PerspectiveCamera, Renderer } from '../src'

/** A full-window canvas with a renderer that follows the window's size. */
export async function setup() {
  const canvas = document.createElement('canvas')
  canvas.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;display:block'
  document.body.style.margin = '0'
  document.body.appendChild(canvas)
  let renderer: Renderer
  try {
    renderer = await Renderer.create(canvas)
  } catch (error) {
    document.body.textContent = `WebGPU unavailable: ${(error as Error).message}`
    throw error
  }
  const camera = new PerspectiveCamera(60, 1, 0.25, 2000)
  const resize = () => {
    renderer.setSize(innerWidth, innerHeight, Math.min(devicePixelRatio, 2))
    camera.aspect = innerWidth / innerHeight
  }
  addEventListener('resize', resize)
  resize()
  return { canvas, renderer, camera }
}

/** A corner readout of frame time and the renderer's stats, refreshed four times a second. */
export function statsOverlay(renderer: Renderer, extra: () => string = () => '') {
  const el = document.createElement('div')
  el.id = 'stats'
  el.style.cssText = 'position:fixed;top:8px;right:8px;font:12px/1.4 ui-monospace,Menlo,monospace;color:#eee;background:rgba(0,0,0,.6);padding:6px 8px;border-radius:4px;white-space:pre;pointer-events:none'
  document.body.appendChild(el)
  let last = performance.now()
  let frames = 0
  let cpu = 0
  let refreshed = last
  return () => {
    const now = performance.now()
    frames++
    cpu += renderer.stats.cpuMs
    if (now - refreshed < 250) return
    const s = renderer.stats
    const gpu = Object.entries(s.gpuMs).map(([k, v]) => `${k} ${v.toFixed(2)}`).join('  ')
    el.textContent = [
      `fps ${((frames * 1000) / (now - last)).toFixed(0)}   render cpu ${(cpu / frames).toFixed(3)} ms`,
      `draws ${s.drawCalls} (shadow ${s.shadowDraws})   tris ${(s.triangles / 1000).toFixed(1)}k   instances ${s.instances}`,
      `records ${s.records}   rewritten ${s.recordsWritten}`,
      `pipelines ${s.pipelines}   materials ${s.materials}   geometry ${(s.geometryBytes / 1048576).toFixed(1)} MB`,
      gpu ? `gpu ms  ${gpu}` : 'gpu ms  (no timestamp-query)',
      extra(),
    ].filter(Boolean).join('\n')
    last = refreshed = now
    frames = 0
    cpu = 0
  }
}

/** Orbits the camera around a point with the mouse (drag) and wheel (zoom). */
export function orbit(canvas: HTMLCanvasElement, camera: PerspectiveCamera, distance = 60, height = 25) {
  let yaw = Number(new URLSearchParams(location.search).get('yaw') ?? 0.6)
  let pitch = Math.atan2(height, distance)
  let dragging = false
  canvas.addEventListener('pointerdown', () => { dragging = true })
  addEventListener('pointerup', () => { dragging = false })
  addEventListener('pointermove', (e) => {
    if (!dragging) return
    yaw -= e.movementX * 0.005
    pitch = Math.min(1.5, Math.max(0.05, pitch + e.movementY * 0.005))
  })
  canvas.addEventListener('wheel', (e) => { distance *= Math.exp(e.deltaY * 0.001) }, { passive: true })
  return (spin = 0) => {
    yaw += spin
    const r = Math.hypot(distance, 0)
    camera.position.set(Math.sin(yaw) * Math.cos(pitch) * r, Math.sin(pitch) * r, Math.cos(yaw) * Math.cos(pitch) * r)
    camera.lookAt(0, 0, 0)
  }
}
