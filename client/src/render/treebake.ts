import type { ImpostorAtlas } from './impostors'
import { winter } from './season'
import { adoptTreeAtlas } from './trees'

/**
 * Bakes the trees' impostor atlas in a worker of its own, and resolves once the first tree can be
 * built from it. The bake is half a second of work that the first tile with trees would else do on
 * the main thread (LOAD-TIME.md). Where there are no workers (tests), or the worker fails, it resolves
 * with nothing handed over and that tile bakes it as before. Call it once the season is known.
 */
export function bakeTreesAhead(): Promise<void> {
  if (typeof Worker === 'undefined') return Promise.resolve()
  return new Promise((resolve) => {
    const worker = new Worker(new URL('./impostor.worker.ts', import.meta.url), { type: 'module', name: 'impostors' })
    const done = () => {
      worker.terminate()
      resolve()
    }
    worker.onmessage = (event: MessageEvent<ImpostorAtlas>) => {
      adoptTreeAtlas(event.data)
      done()
    }
    worker.onerror = done
    worker.postMessage({ winter })
  })
}
