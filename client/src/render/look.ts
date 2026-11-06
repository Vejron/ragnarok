import type { ToneMapping } from '@ragnarok/engine'

/** The soft-lighting options a player can switch on and off, and what else the HUD shows. Each is independent. */
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
  /** Water mirrors the scene: the world is drawn a second time, mirrored, at half size. */
  reflect: boolean
  /** Grass that sways and parts around whoever goes through it; thinner on phones and tablets. */
  grass: boolean
  /** Craters marked on the ground: char, rays and the rim's shadow (SCORCH.md). The dent is there either way. */
  scorch: boolean
  /** Draw at a lower resolution while frames are slow, and sharpen again when they are not. */
  adaptive: boolean
  /** The HUD's network numbers: ping, players in view, the tally and the sync line. F3 toggles it. */
  netStats: boolean
}

/** The options as the settings show them: a name, what it does, and whether it costs a slow device much. */
export const LOOK_OPTIONS: { key: keyof Look; label: string; about: string; heavy?: boolean }[] = [
  { key: 'soft', label: 'Soft light', about: 'A gentler palette, filmic tone mapping and softer sun shadows.' },
  { key: 'contact', label: 'Contact shadows', about: 'A dark patch under every character and vehicle, so they sit on the ground.' },
  { key: 'far', label: 'Far shadows', about: 'Sun shadows reach further from you, a little coarser.', heavy: true },
  { key: 'ink', label: 'Ink outlines', about: 'Dark cartoon outlines where one thing meets another.' },
  { key: 'bloom', label: 'Bloom', about: 'A glow around fire, tracers and flashes.', heavy: true },
  { key: 'reflect', label: 'Water reflections', about: 'Water mirrors the world, which draws it a second time.', heavy: true },
  { key: 'grass', label: 'Grass', about: 'Grass that sways and parts as you go through it.' },
  { key: 'scorch', label: 'Scorched craters', about: 'Char and rays on the ground where something blew up.' },
  { key: 'adaptive', label: 'Adaptive resolution', about: 'Draws fewer pixels while the game runs slow, and more again when it catches up.' },
  { key: 'netStats', label: 'Network stats (F3)', about: 'Ping, players in view, your kills and deaths, and the sync numbers, top left.' },
]

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
  return { soft: true, contact: true, far: !coarse, ink: true, bloom: !coarse, reflect: !coarse, grass: true, scorch: true, adaptive: true, netStats: false }
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
  hemiIntensity: 1.25,
  sun: 0xfff0d6,
  sunIntensity: 2.0,
  toneMapping: 'neutral',
  exposure: 1,
  shadowRadius: 4,
}

/** `palette` under a snowy overcast: a paler sky, light thrown back up off the snow, and a sun that does not blow it out. */
export function wintry(palette: Palette): Palette {
  return { ...palette, sky: 0xc9d9e6, hemiSky: 0xeaf1f8, hemiGround: 0xe6ecf3, hemiIntensity: palette.hemiIntensity * 1.8, sunIntensity: palette.sunIntensity * 0.85, exposure: palette.exposure }
}
