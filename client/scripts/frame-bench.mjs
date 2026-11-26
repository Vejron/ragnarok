// What the main thread spends a frame while the world streams in or a house comes down.
//
// Drives the real game in headless Chrome, loaded with `?bench` so that it keeps every frame's
// timings and named spans (`src/spans.ts`, `render/perf.ts`), and records a CPU profile meanwhile.
// It prints the spread of frame times, how many frames were long, where the long ones went by span,
// and the functions that took the most time inside them.
//
//   node client/scripts/frame-bench.mjs --scenario drive|demolish|chaos [--world test] [--seconds 60]
//                                       [--speed 25] [--heading <degrees>] [--cuts <json>]
//                                       [--every 0.4] [--label <name>] [--quality high] [--scale 1]
//                                       [--url <url>] [--out <dir>] [--probe <js>]
//                                       [--craters 0] [--people 0] [--cars 0] [--reach 150]
//
// `chaos` is the end of a long fight: `--craters` already dug within `--reach` metres, `--people` on
// foot and `--cars` driving round with their crews (fed to the entity store as rebuilt snapshots, as
// there is no server), and the player walking a circle for `--seconds` while another crater lands
// every `--every` seconds.
//
// `--shoot` saves pictures of a demolition to `--out`: just after the first cut, half way, and at the end;
// of `chaos`, one at the end.
//
// `--probe` runs JS in the page before the run, with `g` the game, to gather what it likes into
// `window.__probe`, which is printed after it (say, wrapping a method to count what it is given).
//
// `drive` moves the player in a straight line at `--speed` m/s for `--seconds` once the spawn has
// settled, along `--heading` (0 is +z, 90 is +x) or else the heading that keeps longest over the
// world's tiles. `demolish` stands 25 m off the house the cuts are of, looking at it, and replays the
// cuts (`demolition_cost`'s `DEMOLITION_OUT`) through `WorldView.carved`, one every `--every` seconds.
//
// With no `--url` it starts Vite on port 5288 and loads `worlds/<--world>/`. The summary and every
// frame record go to `--out` (default `bench-out/`, untracked) as `<label>.json`.
import { spawn } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const args = process.argv.slice(2)
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length && !args[i + 1].startsWith('--') ? args[i + 1] : fallback
}
const client = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const out = resolve(option('out', join(client, 'bench-out')))
const scenario = option('scenario', 'drive')
const seconds = Number(option('seconds', 60))
const speed = Number(option('speed', 25))
const heading = option('heading', null)
const every = Number(option('every', 0.4))
const scale = Number(option('scale', 1))
const label = option('label', scenario)
const world = option('world', 'test')
const cutsFile = option('cuts', null)
const probe = option('probe', null)
const after = option('after', null)
const port = 5288
let url = option('url', null)
mkdirSync(out, { recursive: true })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const windows = process.platform === 'win32'

if (scenario === 'demolish' && !cutsFile) throw new Error('--scenario demolish needs --cuts <json> (demolition_cost with DEMOLITION_OUT)')

// ---- Vite and Chrome ----

let vite = null
if (!url) {
  url = `http://127.0.0.1:${port}/`
  let up = false
  try { up = (await fetch(url)).ok } catch {}
  if (!up) {
    const env = { ...process.env, VITE_WORLD_BASE: `/worlds/${world}/` }
    // `npx` is a script on Windows, which only a shell runs.
    vite = spawn(windows ? 'npx.cmd' : 'npx', ['vite', '--port', String(port), '--strictPort', '--host', '127.0.0.1'], { cwd: client, stdio: 'ignore', env, shell: windows })
    for (let i = 0; i < 200 && !up; i++) {
      await sleep(300)
      try { up = (await fetch(url)).ok } catch {}
    }
    if (!up) throw new Error('Vite did not start')
  }
}
const page = `${url}${url.includes('?') ? '&' : '?'}bench`

const profile = join(out, 'profile-frames')
const debugPort = 9336
const chromePath = windows ? 'C:/Program Files/Google/Chrome/Application/chrome.exe' : '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const chrome = spawn(chromePath, [
  '--headless=new', '--enable-unsafe-webgpu', ...(windows ? [] : ['--use-angle=metal']), `--remote-debugging-port=${debugPort}`,
  `--user-data-dir=${profile}`, '--window-size=1512,982', `--force-device-scale-factor=${scale}`,
  '--no-first-run', '--mute-audio', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', 'about:blank',
], { stdio: 'ignore' })
const stop = () => {
  chrome.kill()
  // Vite runs under a shell on Windows: the shell's whole tree has to go.
  if (vite && windows) spawn('taskkill', ['/pid', String(vite.pid), '/T', '/F'], { stdio: 'ignore' })
  else vite?.kill()
}
process.on('exit', stop)

let ws
for (let i = 0; i < 50 && !ws; i++) {
  await sleep(200)
  try {
    const list = await (await fetch(`http://127.0.0.1:${debugPort}/json`)).json()
    const target = list.find((t) => t.type === 'page')
    if (target) ws = new WebSocket(target.webSocketDebuggerUrl)
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
const send = (method, params = {}) => new Promise((r) => {
  const i = ++id
  const timer = setTimeout(() => { pending.delete(i); console.log(`no reply to ${method}`); r({}) }, 300000)
  pending.set(i, (msg) => { clearTimeout(timer); r(msg) })
  ws.send(JSON.stringify({ id: i, method, params }))
})
const evaluate = async (expression, timeout = 0) => {
  const asked = send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
  const reply = await (timeout ? Promise.race([asked, sleep(timeout).then(() => ({}))]) : asked)
  if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.exception?.description ?? reply.result.exceptionDetails.text)
  return reply.result?.result?.value
}
await send('Runtime.enable')
await send('Page.enable')
send('Page.navigate', { url: page })
await sleep(3000)
await evaluate(`localStorage.setItem('ragnarok.quality', '${option('quality', 'high')}'); 1`, 5000)
send('Page.navigate', { url: page })
await sleep(1000)
let ready = false
for (let i = 0; i < 240 && !ready; i++) {
  await sleep(500)
  ready = await evaluate('!!(window.__game?.scene?.renderer && window.__game.scene.renderer.stats.drawCalls > 50)', 3000)
}
if (!ready) throw new Error(`The game did not start: ${errors.slice(0, 5).join(' | ')}`)
// The spawn's tiles, their grass and the first redraws settle.
await sleep(8000)
await evaluate(`(() => { const s = document.createElement('style'); s.textContent = '* { visibility: hidden !important; backdrop-filter: none !important; } canvas { visibility: visible !important; }'; document.head.appendChild(s); return 1 })()`)

// ---- In the page ----

await evaluate(`(() => {
  const g = window.__game
  // Alone, as there is no server: the game stops trying to reach one, as each try that fails reloads a
  // world that has been hurt, and the damage being measured would be forgotten half way.
  clearTimeout(g.reconnectTimer)
  g.connect = async () => {}
  const state = () => g.predictor.state
  const place = (x, z, y) => { for (const q of [state().position, g.predictor.previous?.position].filter(Boolean)) { q[0] = x; q[1] = y; q[2] = z } }
  const ground = (x, z, fallback) => { const h = g.terrain?.heightAt(x, z); return Number.isFinite(h) ? h : fallback }
  // Spins for 20 ms in a function of its own: where its samples fall in the profile sets the profile's clock against the page's.
  window.__benchSync = function benchSyncMarker() { const t0 = performance.now(); while (performance.now() - t0 < 20) {} return t0 }
  window.__bench = {
    /** The heading (radians from +z towards +x) whose straight run of 'length' metres keeps longest over the world's tiles. */
    heading(length) {
      const w = g.view.world, tiles = new Set(w.tiles.map((t) => t.col + ',' + t.row))
      const p = state().position
      const inside = (x, z) => tiles.has(Math.floor((x + w.halfExtent) / w.tileSize) + ',' + Math.floor((z + w.halfExtent) / w.tileSize))
      let best = [0, -1]
      for (let k = 0; k < 16; k++) {
        const a = (k / 16) * Math.PI * 2
        let n = 0
        for (let d = 0; d <= length + 500; d += 50) if (inside(p[0] + Math.sin(a) * d, p[2] + Math.cos(a) * d)) n++
        if (n > best[1]) best = [a, n]
      }
      return best[0]
    },
    /** Moves the player along 'heading' at 'speed' m/s for 'seconds', a frame at a time, looking ahead. */
    drive(heading, speed, seconds) {
      const p = state().position
      const [x0, z0] = [p[0], p[2]]
      const [dx, dz] = [Math.sin(heading), Math.cos(heading)]
      let y = p[1]
      const start = performance.now()
      return new Promise((done) => {
        const step = () => {
          const t = (performance.now() - start) / 1000
          const [x, z] = [x0 + dx * speed * t, z0 + dz * speed * t]
          y = ground(x, z, y - 1.5) + 1.5
          place(x, z, y)
          g.input.yaw = Math.atan2(-dx, -dz)
          g.input.pitch = 0.02
          if (t < seconds) requestAnimationFrame(step)
          else done([x, z])
        }
        requestAnimationFrame(step)
      })
    },
    /** Stands 'away' metres off building 'index' of tile (col, row) from 'point' (metres), looking at it. */
    async face(col, row, index, point, away) {
      const w = g.view.world
      const x = -w.halfExtent + (col + 0.5) * w.tileSize, z = -w.halfExtent + (row + 0.5) * w.tileSize
      // To the tile first, for it to stream in.
      place(x, z, ground(x, z, 100) + 2)
      for (let i = 0; i < 100 && !g.view.loaded.get(row * 65536 + col); i++) await new Promise((r) => setTimeout(r, 200))
      const building = g.view.loaded.get(row * 65536 + col)?.tile.buildings[index]
      if (!building) return 'no building ' + index + ' on ' + col + ',' + row
      let [cx, cz] = [0, 0]
      for (const [px, pz] of building.footprint) { cx += px / building.footprint.length; cz += pz / building.footprint.length }
      let [dx, dz] = [point[0] - cx, point[2] - cz]
      const l = Math.hypot(dx, dz) || 1
      ;[dx, dz] = [dx / l, dz / l]
      const [sx, sz] = [point[0] + dx * away, point[2] + dz * away]
      for (let i = 0; i < 3; i++) {
        place(sx, sz, ground(sx, sz, building.base) + 1.7)
        g.input.yaw = Math.atan2(dx, dz)
        g.input.pitch = 0.12
        await new Promise((r) => setTimeout(r, 1500))
      }
      return 'ok'
    },
    /** Replays 'cuts' through the view as the server's carved messages, one every 'every' seconds, then waits 'tail' seconds; with 'shoot', keeps pictures in window.__shots. */
    async demolish(cuts, every, tail, shoot) {
      const shots = (window.__shots = [])
      const picture = (name) => { g.scene.render(); shots.push([name, g.scene.canvas.toDataURL('image/png').split(',')[1]]) }
      for (const [k, cut] of cuts.entries()) {
        g.view.carved([{ ...cut, point: cut.point.map((v) => v / 100) }], true)
        if (shoot && (k === 0 || k === cuts.length >> 1)) {
          // A moment after, with what fell on its way down.
          await new Promise((r) => setTimeout(r, 250))
          picture('cut' + k)
          await new Promise((r) => setTimeout(r, every * 1000 - 250))
        } else await new Promise((r) => setTimeout(r, every * 1000))
      }
      await new Promise((r) => setTimeout(r, tail * 1000))
      if (shoot) picture('end')
      return cuts.length
    },
    /**
     * A long fight's worth of the world at once: 'craters' dug within 'reach' metres of the player,
     * 'people' on foot there (one in six of them dead) and 'cars' driving round with their crews, a
     * third of them police with the siren on. Then the player walks a circle for 'seconds' while a
     * new crater lands every 'every' seconds. The people and cars are fed to the entity store as the
     * scenes the server's snapshots rebuild, 30 a second; decoding a snapshot is not part of it.
     */
    async chaos(kinds, craters, people, cars, seconds, every, reach, shoot) {
      const p = state().position
      const [x0, z0] = [p[0], p[2]]
      const eye = p[1] - ground(x0, z0, p[1])
      let seed = 0x2545f491
      const next = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296)
      const crater = () => {
        const a = next() * Math.PI * 2, d = Math.sqrt(next()) * reach
        // Mostly autocannon and grenades, now and then a shell.
        const radius = next() < 0.2 ? 5.6 : next() < 0.5 ? 3.5 : 2
        return { x: Math.round((x0 + Math.sin(a) * d) * 100), z: Math.round((z0 + Math.cos(a) * d) * 100), radius: Math.round(radius * 100), depth: Math.round(radius * 18), yaw: 0, dip: 0, contact: 0, solid: false, age: 0 }
      }
      // In batches, as a barrage comes, so no one message digs for seconds.
      for (let done = 0; done < craters; done += 50) {
        g.view.cratered(Array.from({ length: Math.min(50, craters - done) }, crater), true)
        await new Promise((r) => requestAnimationFrame(r))
      }

      const FIRST = 100000
      const walkers = Array.from({ length: people }, (_, i) => {
        const a = next() * Math.PI * 2, d = 5 + Math.sqrt(next()) * (reach - 5)
        return { id: FIRST + i, cx: x0 + Math.sin(a) * d, cz: z0 + Math.cos(a) * d, r: 3 + next() * 8, w: (next() < 0.5 ? -1 : 1) * (0.15 + next() * 0.3), phase: next() * 6.28, dead: i % 6 === 5, kind: i % 3 === 0 ? kinds.officer : kinds.civilian, wire: null }
      })
      const drivers = []
      const driving = Array.from({ length: cars }, (_, i) => {
        const police = i % 3 === 0
        const car = { id: FIRST + 10000 + i, r: 25 + next() * (reach - 25), w: (next() < 0.5 ? -1 : 1) * (8 + next() * 6), phase: next() * 6.28, kind: police ? kinds.police : kinds.car, police }
        for (let seat = 0; seat < (police ? 2 : 1); seat++) drivers.push({ id: FIRST + 20000 + i * 4 + seat, car, seat })
        return car
      })
      const scene = (seconds) => {
        const out = new Map()
        for (const w of walkers) {
          if (w.dead && w.wire) { out.set(w.id, w.wire); continue }
          const a = w.phase + seconds * w.w
          const [x, z] = [w.cx + Math.sin(a) * w.r, w.cz + Math.cos(a) * w.r]
          const yaw = Math.atan2(-Math.cos(a) * Math.sign(w.w), Math.sin(a) * Math.sign(w.w))
          w.wire = {
            kind: { kind: w.kind }, place: { x: Math.round(x * 100), y: Math.round((ground(x, z, p[1]) + eye) * 100), z: Math.round(z * 100) },
            heading: { yaw: Math.round(yaw * 1000) }, motion: { vy: 0, knock_x: 0, knock_z: 0 }, flags: { flags: kinds.grounded | (w.dead ? kinds.dead : 0), stance: 0 },
          }
          out.set(w.id, w.wire)
        }
        for (const c of driving) {
          // A circle about the player at 'w' m/s.
          const a = c.phase + (seconds * c.w) / c.r
          const [x, z] = [x0 + Math.sin(a) * c.r, z0 + Math.cos(a) * c.r]
          const yaw = Math.atan2(-Math.cos(a) * Math.sign(c.w), -Math.sin(a) * Math.sign(c.w))
          out.set(c.id, {
            kind: { kind: c.kind }, place: { x: Math.round(x * 100), y: Math.round((ground(x, z, p[1]) + 0.65) * 100), z: Math.round(z * 100) },
            orientation: { qx: 0, qy: Math.round(Math.sin(yaw / 2) * 32767), qz: 0 }, steer: { steer: 20, throttle: 60 }, health: { hp: 100 },
            ...(c.police ? { flags: { flags: kinds.lamp | kinds.siren, stance: 0 } } : {}),
          })
        }
        for (const d of drivers) {
          out.set(d.id, { kind: { kind: d.car.police ? kinds.officer : kinds.civilian }, seated: { vehicle: d.car.id, seat: d.seat, step: 0 }, heading: { yaw: 0 }, look: { pitch: 0 }, flags: { flags: 0, stance: 0 } })
        }
        return out
      }

      const start = performance.now()
      let pushed = 0, dug = 0
      const [radius, pace] = [40, 4]
      let y = p[1]
      window.__frames.length = 0
      return new Promise((done) => {
        const step = () => {
          const now = performance.now()
          const t = (now - start) / 1000
          const tick = Math.floor((now * 60) / 1000 / 2) * 2
          if (people + cars > 0 && tick > pushed) {
            pushed = tick
            g.store.push(tick, scene(t), true)
            g.clock.observe(tick, now)
          }
          if (every > 0 && t > (dug + 1) * every) {
            dug++
            g.view.cratered([crater()], true)
          }
          const a = (t * pace) / radius
          const [x, z] = [x0 - radius + Math.cos(a) * radius, z0 + Math.sin(a) * radius]
          y = ground(x, z, y - eye) + eye
          place(x, z, y)
          g.input.yaw = Math.atan2(Math.sin(a), -Math.cos(a))
          g.input.pitch = 0.05
          if (t < seconds) return requestAnimationFrame(step)
          if (shoot) { g.scene.render(); (window.__shots = []).push(['end', g.scene.canvas.toDataURL('image/png').split(',')[1]]) }
          done({ craters: craters + dug, people, cars, crews: drivers.length, drawn: { people: g.avatars?.all?.size ?? null, cars: g.vehicles?.placed?.size ?? null } })
        }
        requestAnimationFrame(step)
      })
    },
  }
  window.__frames = []
  return 1
})()`)

// ---- The run ----

if (probe) await evaluate(`(() => { const g = window.__game; window.__probe = []; ${probe}; return 1 })()`)

await send('Profiler.enable')
await send('Profiler.setSamplingInterval', { interval: 250 })
await send('Profiler.start')
const synced = await evaluate('window.__benchSync()')
let what = ''
if (scenario === 'drive') {
  const towards = heading !== null ? (Number(heading) * Math.PI) / 180 : await evaluate(`window.__bench.heading(${speed * seconds})`)
  const end = await evaluate(`window.__bench.drive(${towards}, ${speed}, ${seconds})`)
  what = `drove ${(speed * seconds).toFixed(0)} m at ${speed} m/s, heading ${((towards * 180) / Math.PI).toFixed(0)}°, to ${end?.map((v) => v.toFixed(0)).join(', ')}`
} else if (scenario === 'demolish') {
  const cuts = JSON.parse(readFileSync(resolve(cutsFile), 'utf8'))
  const first = cuts[0]
  // Placed before the profile's window matters: the frames before the first cut are kept but are not the house's.
  const placed = await evaluate(`window.__bench.face(${first.col}, ${first.row}, ${first.index}, ${JSON.stringify(first.point.map((v) => v / 100))}, 25)`)
  if (placed !== 'ok') throw new Error(placed)
  await evaluate('(() => { window.__frames.length = 0; window.__demolishFrom = performance.now(); return 1 })()')
  const n = await evaluate(`window.__bench.demolish(${JSON.stringify(cuts)}, ${every}, 5, ${args.includes('--shoot')})`)
  what = `replayed ${n} cuts of building ${first.index} on tile ${first.col},${first.row}, one every ${every} s`
} else if (scenario === 'chaos') {
  const [craters, people, cars] = [Number(option('craters', 0)), Number(option('people', 0)), Number(option('cars', 0))]
  // The page's own copies of the kinds and flags, so the scenes say what the server's would.
  const done = await evaluate(`(async () => {
    const c = await import('/src/net/codec.ts')
    const kinds = { civilian: c.KIND_CIVILIAN, officer: c.KIND_OFFICER, police: c.KIND_POLICE_CAR, car: c.KIND_VOLVO_240, grounded: c.FLAG_GROUNDED, dead: c.FLAG_DEAD, lamp: c.FLAG_LAMP, siren: c.FLAG_SIREN }
    return await window.__bench.chaos(kinds, ${craters}, ${people}, ${cars}, ${seconds}, ${every}, ${Number(option('reach', 150))}, ${args.includes('--shoot')})
  })()`)
  what = `walked a circle for ${seconds} s among ${done?.craters} craters (one more every ${every} s), ${done?.people} people on foot and ${done?.cars} cars with ${done?.crews} aboard; drawn at the end: ${JSON.stringify(done?.drawn)}`
} else {
  throw new Error(`no scenario ${scenario}`)
}
const { result } = await send('Profiler.stop')
if (args.includes('--shoot')) {
  // Fetched a piece at a time: a big message drops the connection.
  const count = (await evaluate('(window.__shots ?? []).length')) ?? 0
  for (let k = 0; k < count; k++) {
    const name = await evaluate(`window.__shots[${k}][0]`)
    const length = await evaluate(`window.__shots[${k}][1].length`)
    let data = ''
    for (let at = 0; at < length; at += 1 << 20) data += await evaluate(`window.__shots[${k}][1].slice(${at}, ${at + (1 << 20)})`)
    writeFileSync(join(out, `${label}-${name}.png`), Buffer.from(data, 'base64'))
  }
  console.log(`saved ${count} pictures to ${out}`)
}
const frames = await evaluate("window.__frames")
if (probe) console.log('probe:', JSON.stringify(await evaluate('window.__probe'), null, 1).slice(0, 6000))
// `--after`: JS run in the page once the run is over, with `g` the game, and what it gives printed.
if (after) console.log('after:', JSON.stringify(await evaluate(`(async () => { const g = window.__game; return await (async () => { ${after} })() })()`), null, 1).slice(0, 6000))

// ---- What it says ----

const cpu = result.profile
const nodes = new Map(cpu.nodes.map((n) => [n.id, n]))
const times = []
let t = cpu.startTime
for (const delta of cpu.timeDeltas) times.push((t += delta))
// The profile's clock against the page's: the first sample in the marker is within a sampling interval of its start.
const marker = cpu.samples.findIndex((s) => nodes.get(s)?.callFrame.functionName === 'benchSyncMarker')
const offset = marker >= 0 ? times[marker] / 1000 - synced : null
const nameOf = (node) => {
  const f = node.callFrame
  const file = f.url ? f.url.replace(/^.*\/(src|node_modules|@fs)\//, '').replace(/\?.*$/, '') : ''
  return `${f.functionName || '(anonymous)'}${file ? ` ${file}:${f.lineNumber + 1}` : ''}`
}
/** Self time by function, ms, of the samples between page times `from` and `to`. */
const selfTime = (windows, into) => {
  if (offset === null) return
  let w = 0
  for (let i = 0; i < cpu.samples.length; i++) {
    const at = times[i] / 1000 - offset
    while (w < windows.length && windows[w][1] < at) w++
    if (w >= windows.length) break
    if (at < windows[w][0]) continue
    const ms = (cpu.timeDeltas[i + 1] ?? cpu.timeDeltas[i]) / 1000
    const name = nameOf(nodes.get(cpu.samples[i]))
    into.set(name, (into.get(name) ?? 0) + ms)
  }
}

const pct = (values, p) => {
  const sorted = values.slice().sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0
}
const intervals = frames.map((f) => f.interval)
const cpus = frames.map((f) => f.cpu)
const line = (name, values) => `${name.padEnd(9)} p50 ${pct(values, 0.5).toFixed(1).padStart(5)}  p95 ${pct(values, 0.95).toFixed(1).padStart(5)}  p99 ${pct(values, 0.99).toFixed(1).padStart(5)}  max ${Math.max(...values).toFixed(1).padStart(6)} ms   over 16.7: ${values.filter((v) => v > 16.7).length}  25: ${values.filter((v) => v > 25).length}  33: ${values.filter((v) => v > 33).length}  50: ${values.filter((v) => v > 50).length}`

// A long frame shows in the interval of the frame after it, so frames are judged by their own
// callback's time (`cpu`). What ran outside any frame (fetches, decoding, worker replies, collections)
// shows as a stall: a long interval after a short frame.
const LONG = 12
const long = frames.filter((f) => f.cpu > LONG)
const spanTotals = (list, key) => {
  const sums = new Map()
  for (const f of list) for (const [name, ms] of Object.entries(f[key])) sums.set(name, (sums.get(name) ?? 0) + ms)
  return [...sums].sort((a, b) => b[1] - a[1])
}
const functions = new Map()
selfTime(long.map((f) => [f.at, f.at + f.cpu]), functions)
// The stretches between frames that went on longer than two vsyncs' worth, or held spans of 4 ms or more.
const busyBetween = (f) => Object.values(f.between).reduce((s, ms) => s + ms, 0)
const stalls = frames.flatMap((f, i) => (i > 0 && ((f.interval > 2 * pct(intervals, 0.5) + LONG && frames[i - 1].cpu < LONG) || busyBetween(f) >= 4) ? [[frames[i - 1].at + frames[i - 1].cpu, f.at]] : []))
const between = new Map()
selfTime(stalls, between)
const everywhere = new Map()
selfTime([[frames[0]?.at ?? 0, Infinity]], everywhere)

const top = (map, n) => [...map].sort((a, b) => b[1] - a[1]).slice(0, n)
const report = [
  `${label}: ${what}; ${frames.length} frames`,
  line('interval', intervals),
  line('frame cpu', cpus),
  `stalls between frames (a long interval after a short frame, or 4 ms of spans): ${stalls.length}, longest ${Math.max(0, ...stalls.map(([a, b]) => b - a)).toFixed(1)} ms`,
  `the most work between two frames: ${Math.max(0, ...frames.map(busyBetween)).toFixed(1)} ms of spans`,
  `uploads: ${(frames.reduce((s, f) => s + f.upload, 0) / 1e6).toFixed(1)} MB in all, most in a frame ${(Math.max(...frames.map((f) => f.upload)) / 1e6).toFixed(2)} MB`,
  '',
  `spans in the ${long.length} frames whose own time was over ${LONG} ms, ms in all:`,
  ...spanTotals(long, 'spans').slice(0, 25).map(([n, ms]) => `  ${n.padEnd(28)} ${ms.toFixed(1).padStart(8)}`),
  '',
  `spans in the stalls between frames, ms in all:`,
  ...spanTotals(frames.filter((f) => busyBetween(f) >= 4), 'between').slice(0, 12).map(([n, ms]) => `  ~${n.padEnd(27)} ${ms.toFixed(1).padStart(8)}`),
  `functions by self time in the stalls between frames:`,
  ...top(between, 25).map(([n, ms]) => `  ${ms.toFixed(1).padStart(8)}  ${n}`),
  '',
  `spans over the whole run, ms in all:`,
  ...spanTotals(frames, 'spans').slice(0, 25).map(([n, ms]) => `  ${n.padEnd(28)} ${ms.toFixed(1).padStart(8)}`),
  ...spanTotals(frames, 'between').slice(0, 10).map(([n, ms]) => `  ~${n.padEnd(27)} ${ms.toFixed(1).padStart(8)}`),
  '',
  offset === null ? 'no profile clock: the marker was not sampled' : `functions by self time in the frames over ${LONG} ms of their own:`,
  ...top(functions, 30).map(([n, ms]) => `  ${ms.toFixed(1).padStart(8)}  ${n}`),
  '',
  'functions by self time over the whole run:',
  ...top(everywhere, 20).map(([n, ms]) => `  ${ms.toFixed(1).padStart(8)}  ${n}`),
]
console.log(report.join('\n'))
// The worst frames by their own time, each with its spans, to read one hitch at a time.
const worst = frames.slice(1).sort((a, b) => b.cpu - a.cpu).slice(0, 10)
console.log('\nworst frames:')
for (const f of worst) {
  const parts = Object.entries(f.spans).filter(([, ms]) => ms > 0.5).sort((a, b) => b[1] - a[1]).slice(0, 7).map(([n, ms]) => `${n} ${ms.toFixed(1)}`)
  console.log(`  ${f.cpu.toFixed(1)} ms (render ${f.render.toFixed(1)}, upload ${(f.upload / 1e6).toFixed(2)} MB): ${parts.join(', ')}`)
}
writeFileSync(join(out, `${label}.json`), JSON.stringify({ label, scenario, what, report, frames, errors: errors.slice(0, 20) }, null, 1))
if (errors.length) console.log(`\nconsole errors:\n  ${errors.slice(0, 8).join('\n  ')}`)
ws.close()
stop()
process.exit(0)
