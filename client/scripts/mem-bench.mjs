// What the client holds in memory, and how much garbage a frame makes.
//
// Drives the real game in headless Chrome (WebGPU over Metal) as `gpu-bench.mjs` does, at quality `high`
// with a 1512×982 window at device scale 2. It lets the tiles round the spawn settle and prints:
//
//   - the JS heap and the typed arrays' storage after a forced collection, on the page and in each worker,
//   - every GPU texture and buffer alive, summed by label (WebGPU cannot be asked what it holds, so the
//     page's `createTexture` and `createBuffer` are wrapped before the game starts and the sizes added up),
//   - the resident memory of Chrome's renderer and GPU processes,
//   - over `--seconds` of the game running: how much the page allocated (the sampling heap profiler, which
//     counts what was collected too), which functions allocated it, and the collections it cost,
//   - with `--walk`, the same totals after going 600 m off through four places and back, which shows
//     what a tile leaves behind once it is unloaded.
//
//   node client/scripts/mem-bench.mjs [--out <dir>] [--url <url>] [--world <name>] [--label <name>]
//                                     [--seconds 10] [--scale 2] [--quality high] [--walk] [--top 25]
//                                     [--eval <js>] [--held-only] [--no-transient]
//
// With no `--url` it starts Vite on `[::1]:5288`. The game server takes only the `localhost` and
// `127.0.0.1` origins, so from there the client plays alone: the world as its files have it, the same
// every run, with no other players or vehicles. To measure a busy game, pass `--url http://127.0.0.1:5173/`
// with a server running. Compare runs made the same way only. The totals go to `--out` as JSON.
import { execSync, spawn } from 'node:child_process'
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
const seconds = Number(option('seconds', 10))
const scale = Number(option('scale', 2))
const top = Number(option('top', 25))
const label = option('label', 'mem')
const port = 5288
let url = option('url', null)
mkdirSync(out, { recursive: true })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const MB = 1024 * 1024
const mb = (bytes) => (bytes / MB).toFixed(1).padStart(7)

// ---- Vite and Chrome ----

let vite = null
if (!url) {
  url = `http://[::1]:${port}/`
  let up = false
  try { up = (await fetch(url)).ok } catch {}
  if (!up) {
    const world = option('world', null)
    const env = world ? { ...process.env, VITE_WORLD_BASE: `/worlds/${world}/` } : process.env
    vite = spawn('npx', ['vite', '--port', String(port), '--strictPort', '--host', '::1'], { cwd: client, stdio: 'ignore', env })
    for (let i = 0; i < 100 && !up; i++) {
      await sleep(300)
      try { up = (await fetch(url)).ok } catch {}
    }
    if (!up) throw new Error('Vite did not start')
  }
}

const profile = join(out, 'mem-profile')
const debugPort = 9335
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
/** The workers the page started, by the session Chrome attached to each. */
const workers = new Map()
let traced = []
let traceDone = null
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data)
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id) }
  if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') {
    errors.push(msg.params.args.map((a) => a.value ?? a.description ?? '').join(' '))
  }
  if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text)
  if (msg.method === 'Target.attachedToTarget' && msg.params.targetInfo.type === 'worker') workers.set(msg.params.sessionId, msg.params.targetInfo.url)
  if (msg.method === 'Target.detachedFromTarget') workers.delete(msg.params.sessionId)
  if (msg.method === 'Tracing.dataCollected') traced.push(...msg.params.value)
  if (msg.method === 'Tracing.tracingComplete') traceDone?.()
})
// Every call gives up after two minutes rather than hang the run on a reply lost to a navigation.
const send = (method, params = {}, sessionId) => new Promise((r) => {
  const i = ++id
  const timer = setTimeout(() => { pending.delete(i); console.log(`no reply to ${method}`); r({}) }, 120000)
  pending.set(i, (msg) => { clearTimeout(timer); r(msg) })
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }))
})
const evaluate = async (expression, timeout = 0) => {
  const asked = send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
  const reply = await (timeout ? Promise.race([asked, sleep(timeout).then(() => ({}))]) : asked)
  if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.exception?.description ?? reply.result.exceptionDetails.text)
  return reply.result?.result?.value
}

/**
 * Runs in the page before any of its own code: wraps the making and destroying of GPU textures and
 * buffers, so `__gpuAlive()` can list what is alive with its size. One dropped without `destroy` is
 * taken off the list when it is collected.
 */
const GPU_LEDGER = `(() => {
  if (!globalThis.GPUDevice) return
  const BYTES = { r8unorm: 1, rg8unorm: 2, r16float: 2, depth16unorm: 2, rgba16float: 8, rgba32float: 16 }
  const alive = new Map()
  let next = 0
  const gone = new FinalizationRegistry((key) => alive.delete(key))
  const keep = (object, entry) => { const key = ++next; alive.set(key, entry); object.__ledger = key; gone.register(object, key); return object }
  const createTexture = GPUDevice.prototype.createTexture
  GPUDevice.prototype.createTexture = function (d) {
    if (NO_TRANSIENT) d = { ...d, usage: d.usage & ~(GPUTextureUsage.TRANSIENT_ATTACHMENT ?? 0) }
    const texture = createTexture.call(this, d)
    let bytes = 0
    for (let level = 0; level < texture.mipLevelCount; level++) bytes += Math.max(1, texture.width >> level) * Math.max(1, texture.height >> level)
    bytes *= texture.depthOrArrayLayers * texture.sampleCount * (BYTES[texture.format] ?? 4)
    // A transient attachment lives in the GPU's tile memory: it is given none of its own.
    const transient = (d.usage & (GPUTextureUsage.TRANSIENT_ATTACHMENT ?? 0)) !== 0
    if (transient) bytes = 0
    return keep(texture, { kind: 'texture', label: d.label ?? '', bytes, note: texture.width + 'x' + texture.height + ' ' + texture.format + (texture.sampleCount > 1 ? ' x' + texture.sampleCount : '') + (texture.mipLevelCount > 1 ? ' mips' : '') + (transient ? ' transient' : '') })
  }
  const createBuffer = GPUDevice.prototype.createBuffer
  GPUDevice.prototype.createBuffer = function (d) {
    const buffer = createBuffer.call(this, d)
    return keep(buffer, { kind: 'buffer', label: d.label ?? '', bytes: buffer.size, note: '' })
  }
  for (const type of [GPUTexture, GPUBuffer]) {
    const destroy = type.prototype.destroy
    type.prototype.destroy = function () { alive.delete(this.__ledger); return destroy.call(this) }
  }
  globalThis.__gpuAlive = () => [...alive.values()]
})()`

await send('Runtime.enable')
await send('Page.enable')
await send('HeapProfiler.enable')
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true })
// `--no-transient`: the attachments the engine asks for as transient are made as ordinary ones, to see what that saves.
await send('Page.addScriptToEvaluateOnNewDocument', { source: `globalThis.NO_TRANSIENT = ${flag('no-transient')};${GPU_LEDGER}` })
send('Page.navigate', { url })
await sleep(3000)
await evaluate(`localStorage.setItem('ragnarok.quality', '${option('quality', 'high')}'); 1`, 5000)
send('Page.navigate', { url })
await sleep(1000)
let ready = false
for (let i = 0; i < 160 && !ready; i++) {
  await sleep(500)
  ready = await evaluate('!!(window.__game?.scene?.renderer && window.__game.scene.renderer.stats.drawCalls > 50)', 3000)
}
if (!ready) throw new Error(`The game did not start: ${errors.slice(0, 5).join(' | ')}`)

// ---- In the page ----

await evaluate(`(() => {
  const g = window.__game
  const sleep = (ms) => new Promise((f) => setTimeout(f, ms))
  const start = [g.predictor.state.position[0], g.predictor.state.position[2]]
  // The predictor's state is a new object every tick: it is read afresh, not kept.
  const place = (x, z) => { for (const q of [g.predictor.state.position, g.predictor.previous?.position].filter(Boolean)) { q[0] = x; q[1] = (g.terrain.heightAt(x, z) || 0) + 1; q[2] = z } }
  window.__mem = {
    /** Stands at the spawn, moved by (dx, dz) metres, until the tiles round it have loaded. */
    async stand(dx, dz) {
      g.scene.setTimeOfDay(0.5)
      g.input.yaw = 0
      g.input.pitch = 0.05
      // Put there again as the ground arrives under it: until then there is none to stand on.
      for (let i = 0; i < 10; i++) {
        place(start[0] + dx, start[1] + dz)
        await sleep(400)
      }
      await sleep(4000)
      const p = g.predictor.state.position
      return Math.hypot(p[0] - start[0] - dx, p[2] - start[1] - dz) < 20
    },
    frames: 0,
  }
  // Frames drawn, for the allocation per frame.
  const render = g.scene.render.bind(g.scene)
  g.scene.render = () => { window.__mem.frames++; render() }
  return 1
})()`)

/** Chrome's processes of this run, by kind, with their resident memory in bytes. */
function processes() {
  const found = {}
  try {
    for (const line of execSync('ps -axo rss=,command=', { encoding: 'utf8', maxBuffer: 64 * MB }).split('\n')) {
      if (!line.includes(profile)) continue
      const kind = /--type=([\w-]+)/.exec(line)?.[1] ?? 'browser'
      found[kind] = (found[kind] ?? 0) + Number(line.trim().split(/\s+/)[0]) * 1024
      // The GPU process's footprint is where the textures and buffers are: its resident size leaves most of them out.
      if (kind === 'gpu-process') {
        const pid = execSync(`pgrep -f -- "--type=gpu-process.*${profile}"`, { encoding: 'utf8' }).trim().split('\n')[0]
        const footprint = /phys_footprint: (\d+) (KB|MB|GB)/.exec(execSync(`footprint ${pid}`, { encoding: 'utf8' }))
        if (footprint) found['gpu-process footprint'] = Number(footprint[1]) * { KB: 1024, MB: MB, GB: 1024 * MB }[footprint[2]]
      }
    }
  } catch {}
  return found
}

/** What is held now, after a collection: the heaps, the GPU's textures and buffers, the processes. */
async function held() {
  // Twice: the first collection runs finalisers that free more.
  for (let i = 0; i < 2; i++) {
    await send('HeapProfiler.collectGarbage')
    for (const session of workers.keys()) await send('HeapProfiler.collectGarbage', {}, session)
    await sleep(500)
  }
  const usage = (await send('Runtime.getHeapUsage')).result ?? {}
  const inWorkers = []
  for (const [session, worker] of workers) {
    const u = (await send('Runtime.getHeapUsage', {}, session)).result
    if (u) inWorkers.push({ worker: worker.split('/').pop().split('?')[0], heap: u.usedSize, arrays: u.backingStorageSize })
  }
  const gpu = (await evaluate('globalThis.__gpuAlive?.() ?? []')) ?? []
  const stats = await evaluate(`(() => { const s = window.__game.scene.renderer.stats; return { geometryBytes: s.geometryBytes, draws: s.drawCalls, materials: s.materials } })()`)
  return { heap: usage.usedSize, arrays: usage.backingStorageSize, workers: inWorkers, gpu, stats, processes: processes() }
}

/** GPU objects summed by label (with the digits taken out, so "texture 12" and "texture 40" are one row). */
function byLabel(gpu) {
  const rows = new Map()
  for (const { kind, label, bytes, note } of gpu) {
    const name = `${kind} ${label.replace(/\d+/g, '#') || '(no label)'}`
    const row = rows.get(name) ?? { name, count: 0, bytes: 0, note }
    row.count++
    row.bytes += bytes
    rows.set(name, row)
  }
  return [...rows.values()].sort((a, b) => b.bytes - a.bytes)
}

function print(title, h) {
  const gpuTotal = h.gpu.reduce((sum, o) => sum + o.bytes, 0)
  console.log(`\n${title}`)
  console.log(`  page heap ${mb(h.heap)} MB   typed arrays ${mb(h.arrays)} MB`)
  for (const w of h.workers) console.log(`  ${w.worker.padEnd(24)} heap ${mb(w.heap)} MB   typed arrays ${mb(w.arrays)} MB`)
  console.log(`  GPU, as the page made it ${mb(gpuTotal)} MB in ${h.gpu.length} objects   (arena in use ${mb(h.stats.geometryBytes)} MB)`)
  for (const row of byLabel(h.gpu).slice(0, top)) console.log(`    ${mb(row.bytes)} MB  ${String(row.count).padStart(4)} × ${row.name}${row.count === 1 && row.note ? `  (${row.note})` : ''}`)
  console.log(`  processes: ${Object.entries(h.processes).map(([kind, bytes]) => `${kind} ${(bytes / MB).toFixed(0)} MB`).join('   ')}`)
}

await evaluate('window.__mem.stand(0, 0)')
const settled = await held()
print('Settled at the spawn', settled)

// `--eval <js>`: an expression of your own, run in the settled page (`g` is the game), printed and saved as `<label>-eval.json`.
const probe = option('eval', null)
if (probe) {
  const answer = JSON.stringify(await evaluate(`(async () => { const g = window.__game; return ${probe} })()`), null, 1)
  // To a file as well: a long answer is cut short on a pipe when the script exits.
  writeFileSync(join(out, `${label}-eval.json`), answer ?? 'null')
  console.log(answer?.length > 20000 ? `${answer.slice(0, 2000)}\n... (all of it is in ${join(out, `${label}-eval.json`)})` : answer)
}
if (flag('held-only')) { ws.close(); stop(); process.exit(0) }

// ---- A stretch of the game running ----

await send('HeapProfiler.startSampling', { samplingInterval: 8192, includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true })
traced = []
await send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { includedCategories: ['v8', 'devtools.timeline', 'disabled-by-default-v8.gc'] } })
const framesBefore = await evaluate('window.__mem.frames')
const began = Date.now()
await sleep(seconds * 1000)
const elapsed = (Date.now() - began) / 1000
const frames = (await evaluate('window.__mem.frames')) - framesBefore
const traceEnded = new Promise((r) => { traceDone = r })
await send('Tracing.end')
await Promise.race([traceEnded, sleep(20000)])
const sampling = (await send('HeapProfiler.stopSampling')).result?.profile

/** Allocated bytes by function, from the sampling profile's tree. */
const sites = new Map()
let allocated = 0
const walk = (node) => {
  if (node.selfSize > 0) {
    const f = node.callFrame
    const name = `${f.functionName || '(anonymous)'}  ${f.url.split('/').slice(-2).join('/').split('?')[0]}:${f.lineNumber + 1}`
    sites.set(name, (sites.get(name) ?? 0) + node.selfSize)
    allocated += node.selfSize
  }
  node.children?.forEach(walk)
}
if (sampling) walk(sampling.head)
const collections = {}
for (const event of traced) {
  if ((event.name !== 'MinorGC' && event.name !== 'MajorGC') || event.ph !== 'X') continue
  const c = (collections[event.name] ??= { count: 0, ms: 0, worst: 0 })
  c.count++
  c.ms += event.dur / 1000
  c.worst = Math.max(c.worst, event.dur / 1000)
}
const running = {
  seconds: +elapsed.toFixed(1),
  frames,
  allocatedMBs: +(allocated / MB / elapsed).toFixed(2),
  allocatedKBframe: frames ? +(allocated / 1024 / frames).toFixed(1) : null,
  collections,
  sites: [...sites.entries()].sort((a, b) => b[1] - a[1]).slice(0, top).map(([site, bytes]) => ({ site, kbFrame: frames ? +(bytes / 1024 / frames).toFixed(2) : null, bytes })),
}
console.log(`\nRunning for ${running.seconds} s, ${frames} frames (${(frames / elapsed).toFixed(0)} a second)`)
console.log(`  allocated ${running.allocatedMBs} MB/s, ${running.allocatedKBframe} KB a frame (objects on the JS heap; typed arrays' storage is not counted)`)
for (const [kind, c] of Object.entries(collections)) console.log(`  ${kind} ${c.count} times, ${c.ms.toFixed(1)} ms in all, worst ${c.worst.toFixed(1)} ms`)
for (const s of running.sites) console.log(`    ${String(s.kbFrame).padStart(8)} KB/frame  ${s.site}`)

// ---- Away and back ----

let returned = null
if (flag('walk')) {
  for (const [dx, dz] of [[600, 0], [600, 600], [0, 600], [0, 0]]) {
    if (!(await evaluate(`window.__mem.stand(${dx}, ${dz})`))) console.log(`  the soldier did not get to (${dx}, ${dz}) from the spawn: the walk shows less than it should`)
  }
  returned = await held()
  print('Back at the spawn after a walk', returned)
  const delta = (pick) => ((pick(returned) - pick(settled)) / MB).toFixed(1)
  console.log(`  against settled: heap ${delta((h) => h.heap)} MB, typed arrays ${delta((h) => h.arrays)} MB, GPU ${delta((h) => h.gpu.reduce((s, o) => s + o.bytes, 0))} MB`)
}

const summary = (h) => h && { heap: h.heap, arrays: h.arrays, workers: h.workers, gpuBytes: h.gpu.reduce((s, o) => s + o.bytes, 0), gpu: byLabel(h.gpu), stats: h.stats, processes: h.processes }
writeFileSync(join(out, `${label}.json`), JSON.stringify({ label, url, scale, settled: summary(settled), running, returned: summary(returned), errors: errors.slice(0, 20) }, null, 2))
if (errors.length) console.log(`console errors:\n  ${errors.slice(0, 8).join('\n  ')}`)
ws.close()
stop()
process.exit(0)
