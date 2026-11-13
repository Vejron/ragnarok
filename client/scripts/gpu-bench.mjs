// What a frame costs the GPU at a Retina laptop's size, view by view.
//
// Drives the real game in headless Chrome (WebGPU over Metal) with a 1512×982 window at device scale 2,
// so the drawing buffer is 3024×1964 at quality `high`, the size of a 14" MacBook Pro's screen. At each
// view it stops the game's loop and times `frames` renders to `onSubmittedWorkDone`: whole frames to the
// GPU's finish, which headless Metal measures honestly where its per-pass timers do not. Variants
// (something turned off) are timed alternately with the full frame, `rounds` times, and the medians
// printed, so drift over the run falls on both alike.
//
//   node client/scripts/gpu-bench.mjs [--out <dir>] [--url <url>] [--world <name>] [--views a,b]
//                                     [--breakdown] [--parts a,b] [--extra 'name=<js>'] [--shoot]
//                                     [--rounds 5] [--frames 60] [--scale 2] [--label <name>]
//
// With no `--url` it starts Vite on port 5287 and loads `worlds/<--world>/`, or the world the editor
// activated (`worlds/active.json`). `--breakdown` also times the frame with each costly part turned off. Each view's
// screenshot goes to `--out` (default `bench-out/`, untracked) with the timings as JSON.
import { spawn } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const args = process.argv.slice(2)
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length && !args[i + 1].startsWith('--') ? args[i + 1] : fallback
}
const flag = (name) => args.includes(`--${name}`)
const client = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const out = resolve(option('out', join(client, 'bench-out')))
const rounds = Number(option('rounds', 5))
const frames = Number(option('frames', 60))
const scale = Number(option('scale', 2))
const label = option('label', 'run')
const port = 5287
let url = option('url', null)
mkdirSync(out, { recursive: true })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

/** Views: where the soldier stands, which way the camera looks, and the hour (`setTimeOfDay`). */
const VIEWS = {
  // The spawn: forest on the active world's start.
  forest: { phase: 0.5, yaw: 0, pitch: 0.05, place: null },
  // The highest ground near the spawn, looking out over the land: the most distance and impostors.
  vista: { phase: 0.45, yaw: 2.2, pitch: 0.12, place: 'hill' },
  // A shore, looking over water: the reflection pass.
  water: { phase: 0.55, yaw: null, pitch: 0.08, place: 'shore' },
  // The spawn at night with the headlamp on: the lights, beams, stars.
  night: { phase: 0.02, yaw: 0.3, pitch: 0.0, place: null, lamp: true },
}

/** What `--breakdown` turns off, one at a time: [name, JS that turns it off and returns an undo]. */
const PARTS = [
  ['shadows', `(() => { const s = e.sun.shadow; s.enabled = false; return () => { s.enabled = true } })()`],
  ['reflection', `(() => { const h = r.reflectionHeight; r.reflectionHeight = null; return () => { r.reflectionHeight = h } })()`],
  ['sky', `(() => { const k = e.sky; e.sky = null; return () => { e.sky = k } })()`],
  ['bloom', `(() => { const b = e.bloom; e.bloom = null; return () => { e.bloom = b } })()`],
  ['ink', `(() => { const i = e.ink; e.ink = null; return () => { e.ink = i } })()`],
  ['shafts+flare', `(() => { const a = e.shafts, b = e.flare; e.shafts = null; e.flare = null; return () => { e.shafts = a; e.flare = b } })()`],
  ['effects', `(() => { const hid = []; const walk = (n) => { if (n.prepare && n.draw && n.visible) { n.visible = false; hid.push(n) } n.children.forEach(walk) }; walk(e); return () => hid.forEach((n) => { n.visible = true }) })()`],
  ['trees', `(() => { const hid = []; const walk = (n) => { if (n.levels && n.visible) { n.visible = false; hid.push(n) } n.children.forEach(walk) }; walk(e); return () => hid.forEach((n) => { n.visible = true }) })()`],
  ['grass', `(() => { const hid = []; const walk = (n) => { if (n.visible && /grass/i.test(n.name ?? '')) { n.visible = false; hid.push(n) } n.children.forEach(walk) }; walk(e); return () => hid.forEach((n) => { n.visible = true }) })()`],
]

// ---- Vite and Chrome ----

let vite = null
if (!url) {
  url = `http://127.0.0.1:${port}/`
  let up = false
  try { up = (await fetch(url)).ok } catch {}
  if (!up) {
    const world = option('world', null)
    const env = world ? { ...process.env, VITE_WORLD_BASE: `/worlds/${world}/` } : process.env
    vite = spawn('npx', ['vite', '--port', String(port), '--strictPort', '--host', '127.0.0.1'], { cwd: client, stdio: 'ignore', env })
    for (let i = 0; i < 100 && !up; i++) {
      await sleep(300)
      try { up = (await fetch(url)).ok } catch {}
    }
    if (!up) throw new Error('Vite did not start')
  }
}

const profile = join(out, 'profile')
const debugPort = 9334
const chrome = spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', [
  '--headless=new', '--enable-unsafe-webgpu', '--use-angle=metal', `--remote-debugging-port=${debugPort}`,
  `--user-data-dir=${profile}`, '--window-size=1512,982', `--force-device-scale-factor=${scale}`,
  '--no-first-run', '--mute-audio', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', 'about:blank',
], { stdio: 'ignore' })
const stop = () => { chrome.kill(); vite?.kill() }
process.on('exit', stop)

let ws
for (let i = 0; i < 50 && !ws; i++) {
  await sleep(200)
  try {
    const list = await (await fetch(`http://127.0.0.1:${debugPort}/json`)).json()
    const page = list.find((t) => t.type === 'page')
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl)
  } catch {}
}
if (!ws) throw new Error('Chrome did not start')
await new Promise((r) => ws.addEventListener('open', r))
let id = 0
const pending = new Map()
const errors = []
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data)
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id) }
  if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') {
    errors.push(msg.params.args.map((a) => a.value ?? a.description ?? '').join(' '))
  }
  if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text)
})
// Every call gives up after two minutes rather than hang the run on a reply lost to a navigation.
const send = (method, params = {}) => new Promise((r) => {
  const i = ++id
  const timer = setTimeout(() => { pending.delete(i); console.log(`no reply to ${method}`); r({}) }, 120000)
  pending.set(i, (msg) => { clearTimeout(timer); r(msg) })
  ws.send(JSON.stringify({ id: i, method, params }))
})
const evaluate = async (expression, timeout = 0) => {
  const asked = send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
  // A reply can be lost when the page navigates under it: polls give up and ask again.
  const reply = await (timeout ? Promise.race([asked, sleep(timeout).then(() => ({}))]) : asked)
  if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.exception?.description ?? reply.result.exceptionDetails.text)
  return reply.result?.result?.value
}
await send('Runtime.enable')
await send('Page.enable')
send('Page.navigate', { url })
await sleep(3000)
await evaluate(`localStorage.setItem('ragnarok.quality', 'high'); 1`, 5000)
send('Page.navigate', { url })
await sleep(1000)
let ready = false
for (let i = 0; i < 160 && !ready; i++) {
  await sleep(500)
  ready = await evaluate('!!(window.__game?.scene?.renderer && window.__game.scene.renderer.stats.drawCalls > 50)', 3000)
}
if (!ready) throw new Error(`The game did not start: ${errors.slice(0, 5).join(' | ')}`)
await sleep(3000)
await evaluate(`(() => { const s = document.createElement('style'); s.textContent = '* { visibility: hidden !important; backdrop-filter: none !important; } canvas { visibility: visible !important; }'; document.head.appendChild(s); return 1 })()`)

// ---- In the page ----

await evaluate(`(() => {
  const g = window.__game
  const sleep = (ms) => new Promise((f) => setTimeout(f, ms))
  const state = g.predictor.state
  const start = [state.position[0], state.position[2]]
  const place = (x, z, above = 1) => { for (const q of [state.position, g.predictor.previous?.position].filter(Boolean)) { q[0] = x; q[1] = g.terrain.heightAt(x, z) + above; q[2] = z } }
  window.__bench = {
    async view(v) {
      g.scene.setTimeOfDay(v.phase)
      if (g.player) g.player.lamp = !!v.lamp
      let yaw = v.yaw
      place(start[0], start[1])
      await sleep(300)
      if (v.place === 'hill') {
        let best = [start[0], start[1], -1e9]
        for (let x = -350; x <= 350; x += 10) for (let z = -350; z <= 350; z += 10) {
          const h = g.terrain.heightAt(start[0] + x, start[1] + z)
          if (Number.isFinite(h) && h > best[2]) best = [start[0] + x, start[1] + z, h]
        }
        place(best[0], best[1])
      } else if (v.place === 'shore') {
        const wet = (x, z) => { const w = g.fxWorld.waterAt?.(x, z); return typeof w === 'number' && w > g.terrain.heightAt(x, z) + 0.3 }
        let found = null
        for (let step = 0; step <= 6 && !found; step++) {
          if (step) { place(start[0] + step * 250, start[1]); await sleep(2500) }
          const p = state.position
          for (let r = 10; r < 400 && !found; r += 10) for (let a = 0; a < 48 && !found; a++) {
            const x = p[0] + Math.cos(a / 48 * 6.283) * r, z = p[2] + Math.sin(a / 48 * 6.283) * r
            if (wet(x, z)) found = [x, z, p[0], p[2]]
          }
        }
        if (!found) return 'no water'
        let [x, z, sx, sz] = found
        const dx = sx - x, dz = sz - z, l = Math.hypot(dx, dz)
        for (let t = 0; t < l; t += 2) {
          const qx = x + dx / l * t, qz = z + dz / l * t
          if (!wet(qx, qz)) { x = qx + dx / l * 6; z = qz + dz / l * 6; break }
        }
        place(x, z, 4)
        yaw = Math.atan2(dx / l, dz / l)
      }
      g.input.yaw = yaw
      g.input.pitch = v.pitch
      // Let the tiles round the new place load and the eye adapt.
      await sleep(5000)
      return 'ok'
    },
    pause() {
      cancelAnimationFrame(g.frameHandle)
      g.scene.setRenderScale(1)
      const r = g.scene.renderer
      return r.width + 'x' + r.height
    },
    resume() {
      g.frameHandle = requestAnimationFrame((t) => g.frame(t))
    },
    async time(n) {
      const s = g.scene, r = s.renderer
      // Twice to settle: targets made, pipelines compiled.
      s.render(); s.render()
      await r.device.queue.onSubmittedWorkDone()
      const t = performance.now()
      for (let i = 0; i < n; i++) s.render()
      await r.device.queue.onSubmittedWorkDone()
      return (performance.now() - t) / n
    },
    stats() {
      g.scene.render()
      const st = g.scene.renderer.stats
      return { draws: st.drawCalls, triangles: Math.round(st.triangles), cpuMs: +st.cpuMs.toFixed(2) }
    },
  }
  return 1
})()`)

/**
 * Saves the canvas as `<file>.png`: drawn and read back in one task, so the picture is the frame just
 * drawn (the page's own screenshot waits on the compositor, which a heavy world can stall).
 */
async function shoot(file) {
  const url = await evaluate(`(() => { const s = window.__game.scene; s.render(); return s.canvas.toDataURL('image/png') })()`, 30000)
  if (typeof url === 'string') writeFileSync(join(out, `${file}.png`), Buffer.from(url.split(',')[1], 'base64'))
}

const median = (a) => a.slice().sort((x, y) => x - y)[a.length >> 1]
const views = (option('views', null)?.split(',') ?? Object.keys(VIEWS)).filter((v) => VIEWS[v])
// `--parts a,b` limits the breakdown to those; `--extra name=<js returning an undo>` adds one of your own.
const only = option('parts', null)?.split(',')
const extra = args.flatMap((a, i) => (a === '--extra' ? [args[i + 1].split(/=(.*)/s).slice(0, 2)] : []))
const variants = [['full', null], ...(flag('breakdown') || only ? PARTS.filter(([p]) => !only || only.includes(p)) : []), ...extra]
const results = {}
for (const name of views) {
  if (flag('verbose')) console.log(`${name}: placing`)
  const placed = await evaluate(`window.__bench.view(${JSON.stringify(VIEWS[name])})`)
  if (flag('verbose')) console.log(`${name}: ${placed}`)
  if (placed !== 'ok') {
    console.log(`${name}: ${placed}`)
    continue
  }
  if (!flag('no-shots')) await shoot(`${label}-${name}`)
  const size = await evaluate('window.__bench.pause()')
  const times = Object.fromEntries(variants.map(([v]) => [v, []]))
  for (let round = 0; round < rounds; round++) {
    for (const [variant, off] of variants) {
      const ms = await evaluate(`(async () => {
        const g = window.__game, r = g.scene.renderer, e = g.scene.scene
        const undo = ${off ?? 'null'}
        try { return await window.__bench.time(${frames}) } finally { undo?.() }
      })()`)
      times[variant].push(ms)
      if (flag('verbose')) console.log(`  ${variant} ${ms?.toFixed?.(2)}`)
    }
  }
  const stats = await evaluate('window.__bench.stats()')
  // `--shoot`: a screenshot of each variant too, to see what it changes.
  if (flag('shoot')) {
    for (const [variant, off] of variants.slice(1)) {
      await evaluate(`(() => { const g = window.__game, r = g.scene.renderer, e = g.scene.scene; window.__undo = ${off}; return 1 })()`)
      await shoot(`${label}-${name}-${variant}`)
      await evaluate('(() => { window.__undo?.(); return 1 })()')
    }
  }
  await evaluate('window.__bench.resume()')
  const full = median(times.full)
  results[name] = { size, stats, ms: Object.fromEntries(Object.entries(times).map(([v, t]) => [v, +median(t).toFixed(3)])) }
  const parts = Object.entries(results[name].ms).filter(([v]) => v !== 'full').map(([v, ms]) => `${v} −${(full - ms).toFixed(2)}`)
  console.log(`${name.padEnd(7)} ${size}  ${full.toFixed(2)} ms  (${stats.draws} draws, ${(stats.triangles / 1e6).toFixed(2)} M tris)${parts.length ? `\n        ${parts.join('  ')}` : ''}`)
}
writeFileSync(join(out, `${label}.json`), JSON.stringify({ label, rounds, frames, scale, results, errors: errors.slice(0, 20) }, null, 2))
if (errors.length) console.log(`console errors:\n  ${errors.slice(0, 8).join('\n  ')}`)
ws.close()
stop()
process.exit(0)
