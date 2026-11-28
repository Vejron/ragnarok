/// <reference lib="webworker" />
import { setWinter } from './season'
import { bakeTreeAtlas } from './trees'

/*
 * Bakes the trees' impostor atlas off the main thread while the game starts (`bakeTreesAhead`): told
 * the season, it answers with the atlas's two pictures and is done.
 */

declare const self: DedicatedWorkerGlobalScope

self.onmessage = (event: MessageEvent<{ winter: boolean }>) => {
  setWinter(event.data.winter)
  const pixels = bakeTreeAtlas()
  self.postMessage(pixels, [pixels.albedo.buffer, pixels.normalDepth.buffer])
}
