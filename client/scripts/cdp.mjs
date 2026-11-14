// Evaluates an expression in the game's page over CDP, or saves a screenshot of it.
const [, , mode, arg] = process.argv
const pages = await (await fetch('http://localhost:9333/json')).json()
const page = pages.find((p) => p.type === 'page' && p.url.includes('5287'))
if (!page) { console.log('no page', pages.map((p) => p.url)); process.exit(1) }
const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((r) => (ws.onopen = r))
let id = 0
const send = (method, params = {}) => new Promise((resolve) => {
  const mine = ++id
  const on = (e) => { const m = JSON.parse(e.data); if (m.id === mine) { ws.removeEventListener('message', on); resolve(m.result) } }
  ws.addEventListener('message', on)
  ws.send(JSON.stringify({ id: mine, method, params }))
})
if (mode === 'eval') {
  const r = await send('Runtime.evaluate', { expression: arg, awaitPromise: true, returnByValue: true })
  console.log(JSON.stringify(r.result?.value ?? r.exceptionDetails ?? r, null, 1).slice(0, 3000))
} else {
  const r = await send('Page.captureScreenshot', { format: 'png' })
  ;(await import('node:fs')).writeFileSync(arg, Buffer.from(r.data, 'base64'))
  console.log('saved', arg)
}
ws.close()
