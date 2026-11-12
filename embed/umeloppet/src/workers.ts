/**
 * The game prepares tiles and building looks in module workers (`render/tileworker.ts`,
 * `render/lookworker.ts`). A browser starts a worker only from the page's own origin, and the embed's
 * scripts may well be served from another one than the club's page. So a worker asked for from another
 * origin is started from a same-origin blob that imports the real script instead (which needs that host
 * to allow CORS, as the rest of the embed's files do).
 */
export function allowCrossOriginWorkers() {
  const Native = globalThis.Worker as typeof Worker & { umeloppet?: true }
  if (typeof Native === 'undefined' || Native.umeloppet) return
  class CrossOriginWorker extends Native {
    static readonly umeloppet = true
    constructor(url: string | URL, options?: WorkerOptions) {
      const href = new URL(url, location.href)
      if (href.origin === location.origin) {
        super(href, options)
        return
      }
      const shim = new Blob([`import ${JSON.stringify(href.href)}`], { type: 'text/javascript' })
      super(URL.createObjectURL(shim), { ...options, type: 'module' })
    }
  }
  globalThis.Worker = CrossOriginWorker
}
