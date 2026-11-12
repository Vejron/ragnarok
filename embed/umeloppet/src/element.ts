import { allowCrossOriginWorkers } from './workers'
import { distanceText, loadCourse, timeText, type Course } from './course'
import { Profile } from './profile'
import { FlatMap, loadMapImage } from './flatmap'
import type { Report, SoloGame } from './solo'

/**
 * `<umeloppet-map base="https://…/umeloppet/">`: the race course on the world's map, which the visitor
 * can drag and zoom like the Leaflet map it replaces, and, where the browser has WebGPU, a button that
 * puts them at the start on skis to ski the course in 3D.
 *
 * Attributes:
 * - `base`: where the world's files are (`world.msgpack`, `tiles/`, `minimap.webp`, `course.json`,
 *   `models/`, `sfx/`). Defaults to the folder this script was loaded from.
 * - `course`: the course file, if not `course.json` under `base`.
 *
 * The element has no shadow root: the browser hands pointer lock to an element in a shadow tree as its
 * host, and the game's mouse look checks for its own canvas.
 */

/** Read by the client's modules, in place of `import.meta.env.BASE_URL` (`vite.config.ts`). */
declare global {
  var __umeloppetBase: string | undefined
}

const STYLE = `
umeloppet-map { display: block; position: relative; height: 520px; overflow: hidden; border-radius: 6px; background: #eef2f5; font: 14px/1.35 system-ui, sans-serif; color: #1d1d1d; user-select: none; -webkit-user-select: none; }
umeloppet-map .um-flat, umeloppet-map .um-game { position: absolute; inset: 0; width: 100%; height: 100%; }
umeloppet-map .um-game canvas { display: block; width: 100%; height: 100%; touch-action: none; }
umeloppet-map .um-game { display: none; }
umeloppet-map[playing] .um-game { display: block; }
umeloppet-map[playing] .um-flat { display: none; }
umeloppet-map .um-panel { position: absolute; left: 12px; bottom: 12px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; max-width: calc(100% - 24px); }
umeloppet-map button { font: 600 14px system-ui, sans-serif; border: 0; border-radius: 6px; padding: 9px 14px; cursor: pointer; background: #1f8a4c; color: #fff; box-shadow: 0 1px 3px rgba(0,0,0,.3); }
umeloppet-map button.um-quiet { background: rgba(255,255,255,.92); color: #1d1d1d; }
umeloppet-map .um-note { background: rgba(255,255,255,.92); border-radius: 6px; padding: 7px 10px; box-shadow: 0 1px 3px rgba(0,0,0,.2); }
umeloppet-map .um-title { position: absolute; left: 12px; top: 12px; background: rgba(255,255,255,.92); border-radius: 6px; padding: 7px 10px; font-weight: 700; box-shadow: 0 1px 3px rgba(0,0,0,.2); }
umeloppet-map .um-readout { position: absolute; left: 12px; top: 12px; background: rgba(20,24,28,.72); color: #fff; border-radius: 6px; padding: 8px 11px; display: none; min-width: 150px; }
umeloppet-map .um-readout b { font-size: 20px; }
umeloppet-map .um-mini { position: absolute; right: 12px; top: 12px; width: 180px; height: 180px; border-radius: 50%; border: 3px solid rgba(255,255,255,.9); box-shadow: 0 1px 4px rgba(0,0,0,.35); display: none; }
umeloppet-map[playing] .um-readout, umeloppet-map[playing] .um-mini { display: block; }
@media (max-width: 600px) { umeloppet-map .um-mini { width: 112px; height: 112px; } umeloppet-map .um-readout { min-width: 0; } umeloppet-map .um-readout b { font-size: 17px; } }
umeloppet-map[playing] .um-title, umeloppet-map[playing] .um-start { display: none; }
umeloppet-map .um-back { display: none; }
umeloppet-map[playing] .um-back { display: inline-block; }
umeloppet-map .um-pause { position: absolute; inset: 0; display: none; align-items: center; justify-content: center; background: rgba(10,14,18,.35); color: #fff; text-align: center; cursor: pointer; }
umeloppet-map[playing][paused] .um-pause { display: flex; }
umeloppet-map .um-pause div { background: rgba(20,24,28,.8); border-radius: 8px; padding: 14px 18px; max-width: 340px; }
umeloppet-map .um-pause kbd { display: inline-block; background: #fff; color: #111; border-radius: 3px; padding: 0 5px; margin: 0 1px; font: 600 12px ui-monospace, monospace; }
umeloppet-map .um-touch { display: none; position: absolute; inset: 0; pointer-events: none; }
umeloppet-map[playing][touch] .um-touch { display: block; }
umeloppet-map .um-stick { position: absolute; left: 0; bottom: 0; width: 45%; height: 70%; pointer-events: auto; touch-action: none; }
umeloppet-map .um-look { position: absolute; right: 0; bottom: 0; width: 55%; height: 100%; pointer-events: auto; touch-action: none; }
umeloppet-map .um-knob { position: absolute; width: 56px; height: 56px; margin: -28px 0 0 -28px; border-radius: 50%; background: rgba(255,255,255,.35); border: 2px solid rgba(255,255,255,.8); display: none; }
umeloppet-map .um-buttons { position: absolute; right: 12px; bottom: 64px; display: flex; flex-direction: column; gap: 8px; pointer-events: auto; }
umeloppet-map .um-error { color: #a01c1c; }
umeloppet-map button.um-small { font-size: 12px; padding: 5px 9px; border-radius: 5px; }
umeloppet-map .um-buttons button[hidden] { display: none; }
umeloppet-map .um-profile { position: absolute; left: 12px; bottom: 12px; width: calc(100% - 24px); height: 84px; background: rgba(20,24,28,.62); border-radius: 6px; display: none; pointer-events: none; }
umeloppet-map[profiled] .um-profile { display: block; }
umeloppet-map[profiled] .um-panel { bottom: 106px; }
umeloppet-map[profiled] .um-flat { height: calc(100% - 102px); }
umeloppet-map[profiled] .um-buttons { bottom: 112px; }
umeloppet-map .um-clock { font: 700 20px system-ui, sans-serif; font-variant-numeric: tabular-nums; }
umeloppet-map .um-clock.um-done { color: #ffd166; }
@media (max-width: 600px) { umeloppet-map .um-profile { height: 64px; } umeloppet-map[profiled] .um-panel { bottom: 86px; } umeloppet-map[profiled] .um-flat { height: calc(100% - 82px); } umeloppet-map[profiled] .um-buttons { bottom: 92px; } umeloppet-map .um-clock { font-size: 17px; } }
`

/** Further off the course than this, metres, the skier is not marked on its profile, and the readout says how far off. */
const OFF_COURSE = 60

/** The folder this script was served from, the default `base`. */
const SCRIPT_BASE = new URL('./', import.meta.url).href

function withSlash(url: string) {
  return url.endsWith('/') ? url : `${url}/`
}

class UmeloppetMap extends HTMLElement {
  private flat: FlatMap | null = null
  private game: SoloGame | null = null
  private course: Course | null = null
  private base = SCRIPT_BASE
  private readonly abort = new AbortController()
  private report: Report | null = null

  connectedCallback() {
    if (!document.getElementById('umeloppet-style')) {
      const style = document.createElement('style')
      style.id = 'umeloppet-style'
      style.textContent = STYLE
      document.head.append(style)
    }
    this.base = withSlash(new URL(this.getAttribute('base') ?? SCRIPT_BASE, location.href).href)
    this.innerHTML = `
      <canvas class="um-flat"></canvas>
      <div class="um-game"></div>
      <div class="um-title">Umeloppet</div>
      <div class="um-readout"></div>
      <canvas class="um-mini"></canvas>
      <canvas class="um-profile"></canvas>
      <div class="um-pause"><div></div></div>
      <div class="um-touch">
        <div class="um-stick"><div class="um-knob"></div></div>
        <div class="um-look"></div>
        <div class="um-buttons">
          <button class="um-quiet um-small um-sled" hidden>Snöskoter</button>
        </div>
      </div>
      <div class="um-panel">
        <button class="um-start" hidden>Åk banan i 3D</button>
        <button class="um-back um-quiet um-small" title="Tillbaka till kartan">← Kartan</button>
        <span class="um-note" hidden></span>
      </div>`
    void this.load()
  }

  disconnectedCallback() {
    this.abort.abort()
    this.flat?.dispose()
    this.stopGame()
  }

  private find<T extends Element>(selector: string): T {
    return this.querySelector<T>(selector)!
  }

  private note(text: string | null, error = false) {
    const note = this.find<HTMLElement>('.um-note')
    note.hidden = text === null
    note.textContent = text
    note.classList.toggle('um-error', error)
  }

  /** The course and the map first, which every browser can show; then whether it can do 3D. */
  private async load() {
    globalThis.__umeloppetBase = this.base
    try {
      const courseUrl = new URL(this.getAttribute('course') ?? 'course.json', this.base).href
      const [course, image, halfExtent] = await Promise.all([loadCourse(courseUrl), loadMapImage(this.base), this.halfExtent()])
      this.course = course
      this.find<HTMLElement>('.um-title').textContent = `${course.name} · ${distanceText(course.length)}`
      // The profile first, since the map is fitted into the space left above it.
      const profile = new Profile(this.find('.um-profile'), course)
      if (profile.usable) {
        this.profile = profile
        this.toggleAttribute('profiled', true)
        profile.draw()
      }
      this.flat = new FlatMap(this.find('.um-flat'), image, halfExtent, course)
      this.flat.fit()
      this.flat.interactive()
      this.flat.draw()
      new ResizeObserver(() => {
        this.flat?.draw()
        this.profile?.draw(this.game && this.report && this.report.off <= OFF_COURSE ? this.report.along : null)
      }).observe(this)
      const mini = new FlatMap(this.find('.um-mini'), image, halfExtent, course)
      mini.metres = 900
      this.miniMap = mini
    } catch (error) {
      console.error(error)
      this.note('Kartan kunde inte laddas.', true)
      return
    }
    if (await hasWebGpu()) {
      const start = this.find<HTMLButtonElement>('.um-start')
      start.hidden = false
      start.addEventListener('click', () => void this.startGame(), { signal: this.abort.signal })
    } else {
      this.note('Webbläsaren kan inte visa banan i 3D (WebGPU saknas).')
    }
    this.find('.um-back').addEventListener('click', () => this.stopGame(), { signal: this.abort.signal })
  }

  private miniMap: FlatMap | null = null
  private profile: Profile | null = null

  /** Half the side of the world square, metres, from its manifest: where the map image lies. */
  private async halfExtent(): Promise<number> {
    const { decodeManifest } = await import('@/world')
    const response = await fetch(`${this.base}world.msgpack`)
    if (!response.ok) throw new Error(`world.msgpack: ${response.status}`)
    return decodeManifest(new Uint8Array(await response.arrayBuffer())).halfExtent
  }

  private async startGame() {
    if (this.game || !this.course) return
    allowCrossOriginWorkers()
    this.setAttribute('playing', '')
    this.note('Laddar banan…')
    const { SoloGame } = await import('./solo')
    const game = new SoloGame(this.find('.um-game'), this.base, this.course, (report) => this.onReport(report))
    this.game = game
    game.unlockAudio()
    const touch = game.input.coarse
    this.toggleAttribute('touch', touch)
    this.find<HTMLElement>('.um-pause div').innerHTML = touch
      ? 'Dra till vänster för att åka, till höger för att se dig om.<br>Tryck för att fortsätta.'
      : '<b>Klicka för att åka</b><br><br><kbd>W</kbd><kbd>A</kbd><kbd>S</kbd><kbd>D</kbd> åk · musen styr · <kbd>Shift</kbd> staka hårt · <kbd>C</kbd> huka<br><kbd>E</kbd> snöskoter på/av · <kbd>K</kbd> skidor på/av · <kbd>Esc</kbd> släpp musen'
    try {
      await game.start()
    } catch (error) {
      console.error(error)
      this.note('Banan kunde inte startas i 3D.', true)
      return
    }
    if (this.game !== game) return
    this.note(null)
    this.watchFocus(game)
    if (touch) this.touchControls(game)
  }

  /**
   * Keys and mouse go to the game only while the pointer is locked to it (a click on it does that, Esc
   * lets go), so typing elsewhere on the page never moves the skier; and while they do, the arrow keys and
   * space do not scroll the page.
   */
  private watchFocus(game: SoloGame) {
    const signal = this.abort.signal
    const update = () => {
      game.active = game.input.coarse ? true : document.pointerLockElement === game.canvas
      this.toggleAttribute('paused', !game.active)
    }
    update()
    document.addEventListener('pointerlockchange', update, { signal })
    this.find('.um-pause').addEventListener('click', () => {
      if (game.input.coarse) {
        game.active = true
        this.toggleAttribute('paused', false)
      } else void game.canvas.requestPointerLock?.()
    }, { signal })
    window.addEventListener('keydown', (event) => {
      if (game.active && ['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.code)) event.preventDefault()
    }, { signal })
  }

  /** On a phone: a stick under the left thumb, the view dragged by the right, and buttons for E and K. */
  private touchControls(game: SoloGame) {
    const signal = this.abort.signal
    const input = game.input
    const stick = this.find<HTMLElement>('.um-stick')
    const knob = this.find<HTMLElement>('.um-knob')
    let origin: [number, number] | null = null
    const RADIUS = 50
    stick.addEventListener('pointerdown', (event) => {
      stick.setPointerCapture(event.pointerId)
      const rect = stick.getBoundingClientRect()
      origin = [event.clientX, event.clientY]
      knob.style.display = 'block'
      knob.style.left = `${event.clientX - rect.left}px`
      knob.style.top = `${event.clientY - rect.top}px`
    }, { signal })
    stick.addEventListener('pointermove', (event) => {
      if (!origin) return
      const dx = (event.clientX - origin[0]) / RADIUS
      const dy = (event.clientY - origin[1]) / RADIUS
      const length = Math.max(1, Math.hypot(dx, dy))
      // Pushed right out, the skier poles hard.
      input.setStick(dx / length, -dy / length, Math.hypot(dx, dy) > 1.3)
    }, { signal })
    const release = () => {
      origin = null
      knob.style.display = 'none'
      input.setStick(0, 0, false)
    }
    stick.addEventListener('pointerup', release, { signal })
    stick.addEventListener('pointercancel', release, { signal })
    const look = this.find<HTMLElement>('.um-look')
    let last: [number, number] | null = null
    look.addEventListener('pointerdown', (event) => {
      look.setPointerCapture(event.pointerId)
      last = [event.clientX, event.clientY]
    }, { signal })
    look.addEventListener('pointermove', (event) => {
      if (!last) return
      input.lookBy((event.clientX - last[0]) * 1.4, (event.clientY - last[1]) * 1.4)
      last = [event.clientX, event.clientY]
    }, { signal })
    look.addEventListener('pointerup', () => (last = null), { signal })
    look.addEventListener('pointercancel', () => (last = null), { signal })
    // The one button: on or off a snowmobile, shown only when there is one to get on (`onReport`).
    void import('@/sim/consts').then(({ BUTTON_USE }) => {
      this.find<HTMLButtonElement>('.um-sled').addEventListener('pointerdown', (event) => {
        event.preventDefault()
        input.tap(BUTTON_USE)
      }, { signal })
    })
  }

  private onReport(report: Report) {
    this.report = report
    const readout = this.find<HTMLElement>('.um-readout')
    const course = this.course!
    const where = report.off > OFF_COURSE ? `${distanceText(report.off)} från spåret` : `${distanceText(Math.min(report.along, course.length))} av ${distanceText(course.length)}`
    // On a phone the snowmobile button says it all; skis stay on there.
    const touch = this.hasAttribute('touch')
    const hint = touch ? '' : report.riding ? 'E: kliv av' : report.nearSled ? 'E: kör snöskotern' : report.skis ? '' : 'K: skidorna på'
    const sled = this.find<HTMLButtonElement>('.um-sled')
    sled.hidden = !report.riding && !report.nearSled
    sled.textContent = report.riding ? 'Kliv av' : 'Snöskoter'

    const { clock } = report
    const time =
      clock.state === 'ready'
        ? '<small>Klockan startar vid START</small>'
        : clock.state === 'running'
          ? `<span class="um-clock">${timeText(clock.seconds)}</span>`
          : `<span class="um-clock um-done">Mål! ${timeText(clock.seconds)}</span>${clock.sled ? '<br><small>med snöskoter</small>' : ''}`
    readout.innerHTML = `${time}<br><b>${Math.round(report.speed)}</b> km/h<br>${where}${hint ? `<br><small>${hint}</small>` : ''}`
    this.profile?.draw(report.off <= OFF_COURSE ? report.along : null)
    const mini = this.miniMap
    if (mini) {
      mini.cx = report.at.x
      mini.cz = report.at.z
      mini.draw({ player: report.at, snowmobiles: report.snowmobiles })
    }
  }

  private stopGame() {
    if (document.pointerLockElement === this.game?.canvas) document.exitPointerLock()
    const game = this.game
    this.game = null
    try {
      game?.dispose()
    } catch (error) {
      // Whatever went wrong letting the 3D view go, the element goes back to its map.
      console.error(error)
    }
    this.find('.um-game').replaceChildren()
    this.removeAttribute('playing')
    this.removeAttribute('paused')
    this.note(null)
    if (this.report && this.flat) this.flat.draw({ player: this.report.at })
    else this.flat?.draw()
    this.profile?.draw()
  }
}

/** Whether the browser can draw the 3D view: WebGPU with an adapter behind it. */
async function hasWebGpu(): Promise<boolean> {
  try {
    const gpu = (navigator as Navigator & { gpu?: GPU }).gpu
    return gpu !== undefined && (await gpu.requestAdapter()) !== null
  } catch {
    return false
  }
}

if (!customElements.get('umeloppet-map')) customElements.define('umeloppet-map', UmeloppetMap)
