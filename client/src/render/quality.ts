/** How much the picture may cost: `auto` picks by the device. */
export type Quality = 'auto' | 'low' | 'medium' | 'high'
export type Level = Exclude<Quality, 'auto'>

export const QUALITIES: { key: Quality; label: string }[] = [
  { key: 'auto', label: 'Auto' },
  { key: 'low', label: 'Low' },
  { key: 'medium', label: 'Medium' },
  { key: 'high', label: 'High' },
]

/** What a quality level sets: the costs no look toggle covers. */
export interface Preset {
  /** Multisampling: 4, or 1 for none. Changing it makes a new renderer. */
  msaa: 1 | 4
  /** The picture renders at the screen's pixel ratio, to at most this... */
  maxPixelRatio: number
  /** ...times a render scale that drops while frames are slow, to no less than this. */
  minScale: number
  /** The sun's shadow map, texels a side: near and far shadows (`Look.far`). */
  shadowNear: number
  shadowFar: number
  /** Shadow comparisons per pixel: a soft 3×3 filter, or one tap. */
  shadowTaps: 1 | 9
  /** Grass density: `GRASS_FULL` or `GRASS_LITE`. */
  grass: 'full' | 'lite'
  /** Fog from `fogNear` to all fog at `fogFar`, metres; the camera sees no further. */
  fogNear: number
  fogFar: number
  /** Tiles load within `loadRadius` metres and unload past `unloadRadius`. */
  loadRadius: number
  unloadRadius: number
  /** Scales the distances at which trees change to coarser shapes (`TREE_LEVELS_FROM`): under 1 they come nearer. */
  treeDetail: number
  /** Characters nearer the camera than this animate every frame; further ones every second or fourth. */
  animateRadius: number
  /** How often the paper map in the hands is repainted, ms. */
  heldMapMs: number
  /** Whether the clouds' shadows drift over the ground (a texture read per lit pixel). */
  cloudShadows: boolean
}

export const PRESETS: Record<Level, Preset> = {
  high: { msaa: 4, maxPixelRatio: 1.5, minScale: 0.7, shadowNear: 2048, shadowFar: 4096, shadowTaps: 9, grass: 'full', fogNear: 150, fogFar: 600, loadRadius: 450, unloadRadius: 550, treeDetail: 1, animateRadius: 60, heldMapMs: 125, cloudShadows: true },
  medium: { msaa: 4, maxPixelRatio: 1.5, minScale: 0.6, shadowNear: 2048, shadowFar: 2048, shadowTaps: 9, grass: 'lite', fogNear: 130, fogFar: 500, loadRadius: 380, unloadRadius: 460, treeDetail: 0.7, animateRadius: 40, heldMapMs: 125, cloudShadows: true },
  low: { msaa: 1, maxPixelRatio: 1.25, minScale: 0.5, shadowNear: 1024, shadowFar: 2048, shadowTaps: 1, grass: 'lite', fogNear: 110, fogFar: 400, loadRadius: 320, unloadRadius: 400, treeDetail: 0.5, animateRadius: 25, heldMapMs: 250, cloudShadows: false },
}

/** Phones and tablets start at medium; the adaptive resolution takes it from there. */
export function resolveQuality(quality: Quality, coarse = isCoarse()): Level {
  return quality === 'auto' ? (coarse ? 'medium' : 'high') : quality
}

function isCoarse() {
  return typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
}

const KEY = 'ragnarok.quality'

export function loadQuality(): Quality {
  try {
    const saved = localStorage.getItem(KEY)
    if (QUALITIES.some((q) => q.key === saved)) return saved as Quality
  } catch {
    // No storage: auto.
  }
  return 'auto'
}

export function saveQuality(quality: Quality) {
  try {
    localStorage.setItem(KEY, quality)
  } catch {
    // Private mode and the like: the choice just lasts the session.
  }
}
