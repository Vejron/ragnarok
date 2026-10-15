// Opens a page in headless Chrome on the real GPU, waits, then prints console errors and the #stats
// overlay text and saves a screenshot. For checking examples without a person at the browser.
//
//   node scripts/inspect.mjs <url> [waitMs=5000] [screenshot.png]
//
// EVAL="js" runs that script in the page once it has loaded, and AFTER=ms (default 1500) waits before the
// screenshot, so a scene can be poked into a state first. Its value is printed.
//
// Set CHROME to the browser binary if it is not the macOS default.
import { spawn } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'

const [url, waitMs = '5000', shot] = process.argv.slice(2)
if (!url) {
  console.error('usage: node scripts/inspect.mjs <url> [waitMs] [screenshot.png]')
  process.exit(1)
}
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const port = 9300 + Math.floor(Math.random() * 600)
const profile = mkdtempSync(`${tmpdir()}/engine-inspect-`)
const chrome = spawn(CHROME, [
  '--headless=new', '--enable-unsafe-webgpu', `--user-data-dir=${profile}`,
  '--window-size=1280,800', `--remote-debugging-port=${port}`, 'about:blank',
], { stdio: 'ignore' })

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
let targets
for (let i = 0; i < 50 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json() } catch { await sleep(200) }
}
const page = targets.find((t) => t.type === 'page')
const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener('open', r))
let id = 0
const pending = new Map()
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data)
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg)
    pending.delete(msg.id)
  } else if (msg.method === 'Runtime.consoleAPICalled' && ['error', 'warning'].includes(msg.params.type)) {
    console.log(`console.${msg.params.type}:`, msg.params.args.map((a) => a.value ?? a.description).join(' '))
  } else if (msg.method === 'Runtime.exceptionThrown') {
    const d = msg.params.exceptionDetails
    console.log('exception:', d.exception?.description ?? d.text)
  }
})
const send = (method, params = {}) => new Promise((r) => {
  const n = ++id
  pending.set(n, r)
  ws.send(JSON.stringify({ id: n, method, params }))
})
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true })).result?.result?.value

await send('Runtime.enable')
await send('Page.enable')
await send('Page.navigate', { url })
await sleep(Number(waitMs))
if (process.env.EVAL) {
  const out = await send('Runtime.evaluate', { expression: process.env.EVAL, returnByValue: true, awaitPromise: true })
  console.log('eval:', JSON.stringify(out.result?.result?.value ?? out.result?.exceptionDetails?.exception?.description))
  await sleep(Number(process.env.AFTER ?? 1500))
}
console.log(await evaluate(`document.getElementById('stats')?.textContent ?? document.body.innerText.slice(0, 300)`))
if (shot) {
  const { result } = await send('Page.captureScreenshot', { format: 'png' })
  writeFileSync(shot, Buffer.from(result.data, 'base64'))
  console.log(`screenshot: ${shot}`)
}
ws.close()
chrome.kill()
process.exit(0)
