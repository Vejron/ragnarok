import type { ToneMapping } from '@ragnarok/engine'

/** The soft-lighting options a player can switch on and off. Each is independent. */
export interface Look {
  /** A gentler palette, filmic tone mapping and blurrier sun shadows. */
  soft: boolean
  /** A dark blob under every character and vehicle, so they sit on the ground. */
  contact: boolean
  /** The sun's shadows reach further from the player, at a slightly coarser grain. */
  far: boolean
  /** Dark outlines wherever one thing meets another: a cartoon's ink. */
  ink: boolean
  /** A glow around fire, tracers and flashes. */
  bloom: boolean
}

const KEY = 'ragnarok.look'

/**
 * Bump this when a default changes for existing players. A saved look from an older version drops
 * the flags in `RESET_ON_BUMP`, so they fall back to the new default until the player turns them on.
 */
const LOOK_VERSION = 4
const RESET_ON_BUMP: (keyof Look)[] = ['ink']

/** The long shadows are off on phones and tablets, where they cost fill rate and memory. */
export function defaultLook(): Look {
  const coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
  return { soft: true, contact: true, far: !coarse, ink: true, bloom: !coarse }
}

export function loadLook(): Look {
  const look = defaultLook()
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null') as (Partial<Look> & { version?: number }) | null
    const current = saved?.version === LOOK_VERSION
    for (const name of Object.keys(look) as (keyof Look)[]) {
      if (!current && RESET_ON_BUMP.includes(name)) continue
      if (typeof saved?.[name] === 'boolean') look[name] = saved[name]
    }
  } catch {
    // No storage, or junk in it: the defaults stand.
  }
  return look
}

export function saveLook(look: Look) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...look, version: LOOK_VERSION }))
  } catch {
    // Private mode and the like: the choice just lasts the session.
  }
}

/** Colours, light strengths and shadow blur for one way of lighting the world. */
export interface Palette {
  sky: number
  hemiSky: number
  hemiGround: number
  hemiIntensity: number
  sun: number
  sunIntensity: number
  toneMapping: ToneMapping
  exposure: number
  /** Blur of the sun's shadow edge, in shadow-map texels. */
  shadowRadius: number
}

/** The original look: white sun, a bluish fill and hard-edged shadows. */
export const CLASSIC: Palette = {
  sky: 0x9fd3f0,
  hemiSky: 0xdff3ff,
  hemiGround: 0x6b8f4e,
  hemiIntensity: 1.0,
  sun: 0xffffff,
  sunIntensity: 2.4,
  toneMapping: 'none',
  exposure: 1,
  shadowRadius: 1,
}

/**
 * The soft look: a warm sun over a cool sky, with a lighter, warmer bounce from the grass so shaded
 * sides pick up colour instead of going grey. The sun is weaker and the fill stronger, which flattens
 * the contrast between lit and shaded faces. Neutral tone mapping keeps the toy colours as authored
 * while rolling the brightest highlights off.
 */
export const SOFT: Palette = {
  sky: 0xb3dcf4,
  hemiSky: 0xeaf4ff,
  hemiGround: 0x9bb871,
  hemiIntensity: 1.5,
  sun: 0xfff0d6,
  sunIntensity: 1.8,
  toneMapping: 'neutral',
  exposure: 1,
  shadowRadius: 4,
}
