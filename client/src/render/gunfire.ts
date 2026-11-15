/**
 * How a gun's shot looks at its muzzle, by the kind of gun: how big the flash and the flame, how much
 * smoke it blows out and how long that hangs, what a muzzle brake throws sideways, the burning grains,
 * the dust it lifts, and how long the barrel goes on smoking afterwards. `Effects.gunfire` draws a shot
 * from one and `Effects.barrelSmoke` the smoke after it; `docs/GUNSMOKE.md` has the whole of it.
 */
export interface MuzzleProfile {
  /** How wide the flash at the muzzle is, metres. */
  flash: number
  /** How far the tongue of flame reaches along the bore, metres; 0 for none. */
  flame: number
  /** Puffs of smoke blown out ahead of the muzzle. */
  smoke: number
  /** How wide one of those puffs has swollen by the time it is gone, metres. */
  puff: number
  /** The smoke's colours, each puff one of them. */
  tone: readonly number[]
  /** How opaque the smoke is as it leaves, 0 to 1. */
  alpha: number
  /** Seconds the blown smoke hangs. */
  hang: number
  /** Puffs thrown out of each side of a muzzle brake; 0 for a bare muzzle. */
  brake: number
  /** Burning grains of propellant thrown out ahead. */
  sparks: number
  /** How wide a cloud of dust the blast lifts off ground near the muzzle, metres; 0 for none. */
  dust: number
  /** Seconds the barrel goes on smoking after its last shot; 0 for a gun that does not. */
  linger: number
  /** Wisps a second out of the muzzle when the barrel is at its hottest. */
  wisps: number
  /** How much of its full heat one shot gives the barrel: 1 for a gun that smokes its most after every shot, less for one that builds up to it. */
  heat: number
  /** The flash's light on the world around it: its colour, strength, reach in metres and seconds. */
  light: { color: number; intensity: number; radius: number; life: number }
}

const GREY = [0xa8a6a2, 0x8a8885, 0x74726f] as const
const PALE = [0x9a9895, 0x858380, 0xa8a6a2] as const
const POWDER = [0x2a2826, 0x3a3734, 0x1e1d1c, 0x4a4743] as const

/** A tank's or an anti-tank gun's cannon: a great flash, jets out of both sides of the brake, a bank of smoke and the ground blown clean. */
export const CANNON: MuzzleProfile = {
  flash: 4.5, flame: 3, smoke: 18, puff: 4.2, tone: GREY, alpha: 0.8, hang: 5.5, brake: 4, sparks: 22, dust: 5,
  linger: 5, wisps: 14, heat: 1, light: { color: 0xffc060, intensity: 7, radius: 22, life: 0.12 },
}

/** An autocannon: a small sharp flash and thin smoke each round, and a barrel that smokes more the longer it fires. */
export const AUTOCANNON: MuzzleProfile = {
  flash: 2, flame: 1.2, smoke: 4, puff: 2, tone: PALE, alpha: 0.4, hang: 2, brake: 0, sparks: 6, dust: 0,
  linger: 3, wisps: 10, heat: 0.3, light: { color: 0xffc060, intensity: 4, radius: 14, life: 0.09 },
}

/** A Titan's rotary cannon: an autocannon's shot, larger, with a haze that thickens while the barrels turn. */
export const ROTARY: MuzzleProfile = {
  flash: 2.8, flame: 1.8, smoke: 5, puff: 2.6, tone: PALE, alpha: 0.45, hang: 2.5, brake: 0, sparks: 10, dust: 0,
  linger: 4, wisps: 16, heat: 0.2, light: { color: 0xffc060, intensity: 6, radius: 24, life: 0.12 },
}

/** A mortar: a flash and a column of smoke up out of the tube, and dust off the ground round its baseplate. */
export const MORTAR: MuzzleProfile = {
  flash: 2.6, flame: 1.5, smoke: 10, puff: 3.4, tone: PALE, alpha: 0.5, hang: 3.5, brake: 0, sparks: 8, dust: 4,
  linger: 3.5, wisps: 10, heat: 1, light: { color: 0xffc060, intensity: 5, radius: 16, life: 0.1 },
}

/** A black-powder gun: a long tongue of fire and a great bank of thick, dark smoke that hangs as the wind takes it. */
export const BLACK_POWDER: MuzzleProfile = {
  flash: 4.5, flame: 2.8, smoke: 18, puff: 6.5, tone: POWDER, alpha: 0.75, hang: 7, brake: 0, sparks: 14, dust: 0,
  linger: 6, wisps: 12, heat: 1, light: { color: 0xffa040, intensity: 6, radius: 20, life: 0.15 },
}

/** A recoilless launcher's muzzle: a flash and a little smoke ahead (its backblast is drawn apart). */
export const LAUNCHER: MuzzleProfile = {
  flash: 2.4, flame: 1, smoke: 4, puff: 2.2, tone: PALE, alpha: 0.35, hang: 1.8, brake: 0, sparks: 4, dust: 0,
  linger: 0, wisps: 0, heat: 0, light: { color: 0xffb060, intensity: 3, radius: 10, life: 0.09 },
}

/** A walker's gun, which fires light and not powder: the flash alone. */
export const BEAM: MuzzleProfile = {
  flash: 2.4, flame: 0, smoke: 0, puff: 0, tone: PALE, alpha: 0, hang: 0, brake: 0, sparks: 0, dust: 0,
  linger: 0, wisps: 0, heat: 0, light: { color: 0xffc060, intensity: 4, radius: 14, life: 0.09 },
}
