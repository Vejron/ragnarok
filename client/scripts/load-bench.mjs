// How long a cold start takes, and where it goes (docs/LOAD-TIME.md).
//
// Builds the client as a deploy does (the release WASM, `vite build`), serves it over HTTP/2 with
// brotli as the game's server does, and loads it in headless Chrome with an empty cache and no
// service worker, `--runs` times. The game leaves a `load.<stage>` measure in the page's performance
// timeline for each stage of its start (`src/loading.ts`); this prints when each began and ended, the
// median over the runs, and what was downloaded before the first frame.
//
//   node client/scripts/load-bench.mjs [--world umea] [--runs 5] [--down <Mbit/s>] [--latency <ms>]
//                                      [--label <name>] [--out <dir>] [--no-build] [--quality high]
//                                      [--shot <png> --shot-at <ms>]
//
// `--down` and `--latency` hold the page's own requests to a slower line (Chrome's network emulation).
// A worker's requests are not held back, and nor is the time to open a connection, so a throttled run
// is a comparison between builds, not a prediction of a real line.
//
// `--shot` saves a picture of the page `--shot-at` ms into the last run, to see the loading screen.
//
// The build goes to `<out>/load-dist` (default `bench-out/`, untracked) and leaves the release WASM in
// `src/sim/wasm`; `npm run build:wasm` puts the development one back. `--no-build` reuses the last build.
import { execFileSync, spawn } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { createSecureServer } from 'node:http2'
import { dirname, extname, join, normalize, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { brotliCompressSync, constants } from 'node:zlib'

const args = process.argv.slice(2)
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length && !args[i + 1].startsWith('--') ? args[i + 1] : fallback
}
const client = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const worlds = resolve(client, '..', 'worlds')
const out = resolve(option('out', join(client, 'bench-out')))
const dist = join(out, 'load-dist')
const world = option('world', 'umea')
const runs = Number(option('runs', 5))
const down = Number(option('down', 0))
const latency = Number(option('latency', 0))
const label = option('label', 'load')
const shot = option('shot', null)
const shotAt = Number(option('shot-at', 1500))
const port = 5289
const debugPort = 9337
mkdirSync(out, { recursive: true })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const windows = process.platform === 'win32'
if (!existsSync(join(worlds, world, 'world.msgpack'))) throw new Error(`no world at worlds/${world}`)

// ---- The build ----

if (!args.includes('--no-build') || !existsSync(join(dist, 'index.html'))) {
  const run = (cmd, argv, env = {}) => execFileSync(windows ? `${cmd}.cmd` : cmd, argv, { cwd: client, stdio: ['ignore', 'ignore', 'inherit'], env: { ...process.env, ...env }, shell: windows })
  console.log('building the client…')
  run('npm', ['run', 'build:wasm', '--', '--release'])
  run('npx', ['vite', 'build', '--outDir', dist, '--emptyOutDir'], { VITE_WORLD_BASE: `/worlds/${world}/` })
}

// ---- The server: the build and the world, over HTTP/2 with brotli, as the game's server sends them ----

const cert = join(out, 'load-bench-cert.pem')
const key = join(out, 'load-bench-key.pem')
if (!existsSync(cert)) {
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert, '-days', '3650', '-subj', '/CN=localhost'], { stdio: 'ignore' })
}
const TYPES = {
  '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json',
  '.msgpack': 'application/msgpack', '.glb': 'model/gltf-binary', '.webp': 'image/webp', '.png': 'image/png', '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon', '.m4a': 'audio/mp4', '.webmanifest': 'application/manifest+json', '.txt': 'text/plain',
}
/** What `precompress.mjs` compresses for a deploy. */
const PACKED = new Set(['.js', '.mjs', '.css', '.html', '.wasm', '.json', '.msgpack', '.glb', '.svg', '.webmanifest', '.txt', '.xml', '.ico'])
/** Each file's brotli copy, made the first time it is asked for: the unmeasured first load makes them all. */
const packed = new Map()
const server = createSecureServer({ key: readFileSync(key), cert: readFileSync(cert), allowHTTP1: true }, (req, res) => {
  let path = decodeURIComponent((req.url ?? '/').split('?')[0])
  if (path === '/') path = '/index.html'
  const file = path.startsWith('/worlds/') ? normalize(join(worlds, path.slice('/worlds/'.length))) : normalize(join(dist, path))
  if (!(file.startsWith(worlds) || file.startsWith(dist)) || !existsSync(file) || !statSync(file).isFile()) {
    res.statusCode = 404
    return res.end()
  }
  const ext = extname(file).toLowerCase()
  res.setHeader('content-type', TYPES[ext] ?? 'application/octet-stream')
  let body = readFileSync(file)
  if (PACKED.has(ext) && body.length >= 1024 && String(req.headers['accept-encoding'] ?? '').includes('br')) {
    if (!packed.has(file)) {
      const quality = body.length > 4 << 20 || file.includes('/tiles/') ? 9 : 11
      packed.set(file, brotliCompressSync(body, { params: { [constants.BROTLI_PARAM_QUALITY]: quality, [constants.BROTLI_PARAM_SIZE_HINT]: body.length } }))
    }
    body = packed.get(file)
    res.setHeader('content-encoding', 'br')
  }
  res.setHeader('content-length', body.length)
  res.end(req.method === 'HEAD' ? undefined : body)
})
await new Promise((r) => server.listen(port, '127.0.0.1', r))
const url = `https://localhost:${port}/`

// ---- Chrome ----

const profile = join(out, 'profile-load')
rmSync(profile, { recursive: true, force: true })
const chromePath = windows ? 'C:/Program Files/Google/Chrome/Application/chrome.exe' : '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const chrome = spawn(chromePath, [
  '--headless=new', '--enable-unsafe-webgpu', ...(windows ? [] : ['--use-angle=metal']), `--remote-debugging-port=${debugPort}`,
  `--user-data-dir=${profile}`, '--window-size=1512,982', '--force-device-scale-factor=1', '--ignore-certificate-errors',
  '--no-first-run', '--mute-audio', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', 'about:blank',
], { stdio: 'ignore' })
const stop = () => {
  chrome.kill()
  server.close()
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
  if (msg.method === 'Runtime.consoleAPICalled' && msg.params.type === 'error') errors.push(msg.params.args.map((a) => a.value ?? a.description ?? '').join(' '))
  if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text)
})
const send = (method, params = {}) => new Promise((r) => {
  const i = ++id
  pending.set(i, r)
  ws.send(JSON.stringify({ id: i, method, params }))
})
const evaluate = async (expression) => {
  const reply = await Promise.race([send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }), sleep(5000).then(() => ({}))])
  return reply.result?.result?.value
}
await send('Runtime.enable')
await send('Page.enable')
await send('Network.enable')
await send('Network.setBypassServiceWorker', { bypass: true })
await send('Network.setCacheDisabled', { cacheDisabled: true })

/** Loads the page afresh and waits for its first frame to be on the screen. Returns its `load.*` measures and what it fetched. */
async function load(picture) {
  await send('Page.navigate', { url: 'about:blank' })
  await send('Network.clearBrowserCache')
  const started = Date.now()
  send('Page.navigate', { url })
  let taken = !picture
  for (let i = 0; i < 1200; i++) {
    await sleep(100)
    if (!taken && Date.now() - started >= shotAt) {
      taken = true
      const reply = await send('Page.captureScreenshot', { format: 'png' })
      writeFileSync(picture, Buffer.from(reply.result.data, 'base64'))
    }
    // The picture may be asked for after the first frame, to see what that frame left for later.
    if (taken && (await evaluate(`performance.getEntriesByName('load.shown').length > 0`))) break
    if (i === 1199) throw new Error(`The game did not start: ${errors.slice(0, 5).join(' | ')}`)
  }
  return JSON.parse(await evaluate(`(() => {
    const stages = performance.getEntriesByType('measure').filter((m) => m.name.startsWith('load.')).map((m) => ({ name: m.name.slice(5), start: m.startTime, end: m.startTime + m.duration }))
    const shown = stages.find((s) => s.name === 'first-frame').end
    const fetched = performance.getEntriesByType('resource').filter((r) => r.responseEnd <= shown)
    const nav = performance.getEntriesByType('navigation')[0]
    return JSON.stringify({
      stages,
      script: stages.reduce((first, s) => (s.start > 0 ? Math.min(first, s.start) : first), Infinity),
      requests: fetched.length,
      wire: fetched.reduce((sum, r) => sum + r.encodedBodySize, nav.encodedBodySize),
      raw: fetched.reduce((sum, r) => sum + r.decodedBodySize, nav.decodedBodySize),
    })
  })()`))
}

// The first load is not measured: it makes the server's brotli copies, and sets the quality the rest run at.
await load(null)
await evaluate(`localStorage.setItem('ragnarok.quality', '${option('quality', 'high')}'); 1`)
if (down > 0 || latency > 0) {
  const bytes = down > 0 ? (down * 1e6) / 8 : -1
  await send('Network.emulateNetworkConditions', { offline: false, latency, downloadThroughput: bytes, uploadThroughput: bytes })
}
const results = []
for (let run = 0; run < runs; run++) results.push(await load(run === runs - 1 ? shot : null))

// ---- The report ----

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted.length % 2 ? sorted[(sorted.length - 1) / 2] : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2
}
const names = [...new Set(results.flatMap((r) => r.stages.map((s) => s.name)))]
const of = (name, field) => median(results.map((r) => r.stages.find((s) => s.name === name)?.[field] ?? NaN))
const ms = (value) => `${Math.round(value)}`.padStart(7)
const line = down > 0 || latency > 0 ? `${down || 'unlimited'} Mbit/s, ${latency} ms` : 'unthrottled'
console.log(`\n${label}: worlds/${world}, ${runs} cold loads, ${line}; medians, ms since the page was asked for\n`)
console.log(`${'stage'.padEnd(14)}${'start'.padStart(7)}${'end'.padStart(7)}${'took'.padStart(7)}`)
console.log(`${'script runs'.padEnd(14)}${ms(0)}${ms(median(results.map((r) => r.script)))}${ms(median(results.map((r) => r.script)))}`)
const rows = names.map((name) => ({ name, start: of(name, 'start'), end: of(name, 'end') })).sort((a, b) => a.end - b.end || a.start - b.start)
for (const row of rows) console.log(`${row.name.padEnd(14)}${ms(row.start)}${ms(row.end)}${ms(row.end - row.start)}`)
const mb = (bytes) => `${(bytes / 1048576).toFixed(2)} MB`
console.log(`\nbefore the first frame: ${median(results.map((r) => r.requests))} requests, ${mb(median(results.map((r) => r.wire)))} on the wire, ${mb(median(results.map((r) => r.raw)))} unpacked`)
console.log(`first frame, each run: ${results.map((r) => Math.round(r.stages.find((s) => s.name === 'first-frame').end)).join(', ')} ms`)
if (errors.length) console.log(`\npage errors:\n  ${[...new Set(errors)].slice(0, 8).join('\n  ')}`)
writeFileSync(join(out, `${label}.json`), JSON.stringify({ world, runs, down, latency, results }, null, 1))
ws.close()
stop()
process.exit(0)
