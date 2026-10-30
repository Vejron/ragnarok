import { Quaternion, Vector3 } from '@ragnarok/engine'
import { kindView } from '@/kinds'
import type { FxWorld } from './tyres'

/** Past this the plume is not drawn at all, metres; past HALF_RANGE it is thinned. */
const RANGE = 140
const HALF_RANGE = 70
/** A steady breeze that every plume leans with, m/s. */
const WIND = new Vector3(1.1, 0, 0.5)

const rand = (low: number, high: number) => low + Math.random() * (high - low)
const pick = <T>(items: readonly T[]) => items[Math.floor(Math.random() * items.length)]!

const WHITE = [0xd8d8d4, 0xc4c4c0, 0xb0b0ac] as const
const GREY = [0x8a8885, 0x777572, 0x686663] as const
const DARK = [0x3a3836, 0x2a2928, 0x1c1b1a] as const
const SOOT = [0x161514, 0x0f0e0e, 0x1f1c1a] as const

/** Where a plume comes out: a hole in the hull (or the engine bay), the way out of it, and seconds since it was made. */
export interface Outlet {
  x: number
  y: number
  z: number
  out: { x: number; y: number; z: number }
  age: number
}

/** A fresh hole in a burning vehicle throws embers hard for this long, seconds, then settles to a steady trickle. */
const BURST_SECONDS = 6
/** Embers a second a burning hole throws once settled, and on top of that when fresh. */
const EMBERS = [10, 110] as const
/** Flame puffs a second a burning hole gives, settled and on top of that when fresh. */
const FLAMES = [9, 18] as const

/** The engine bay of `kind` posed at `position` and `rotation`, as an outlet: for a vehicle with no hole to burn from. */
export function bayOutlet(kind: number, position: readonly [number, number, number], rotation: Quaternion): Outlet {
  const bay = new Vector3(...kindView(kind).fx.bay).applyQuaternion(rotation).add(new Vector3(...position))
  return { x: bay.x, y: bay.y, z: bay.z, out: { x: 0, y: 1, z: 0 }, age: Infinity }
}

/** Some number of a thing due at `rate` a second over `dt`, the fraction left to chance. */
const due = (rate: number, dt: number) => Math.floor(rate * dt + Math.random())

/**
 * The plume of a hurt vehicle for the `dt` seconds just passed, out of its `outlets` (the holes rounds
 * made in it, or its engine bay). `severity` runs 0..1 from the first wisp of steam to a thick black
 * column; a `wreck` also burns. Puffs start small and hot at a hole, leave the vehicle behind as it
 * drives, rise, swell and grey out as they cool, and lean with the wind. Once it burns, each hole has
 * licks of flame and throws glowing embers out on arcs like a blast's, hard while the hole is fresh.
 * Returns the unspent fraction of a puff, which the caller carries to the next frame.
 */
export function damageSmoke(
  world: FxWorld,
  outlets: readonly Outlet[],
  velocity: Vector3,
  severity: number,
  wreck: boolean,
  dt: number,
  carried: number,
): number {
  const first = outlets[0]
  if (!first) return 0
  const distance = Math.hypot(first.x - world.camera.x, first.y - world.camera.y, first.z - world.camera.z)
  if (distance > RANGE) return 0
  const level = wreck ? 1 : Math.min(1, Math.max(0, severity))
  const thin = distance > HALF_RANGE ? 0.5 : 1
  // Puffs per second: a trickle when scuffed, a rolling column when ablaze, shared out among the holes.
  const owed = carried + dt * (6 + level * 26) * thin
  const puffs = Math.floor(owed)
  const palette = wreck ? SOOT : level > 0.65 ? DARK : level > 0.3 ? GREY : WHITE
  const speed = Math.hypot(velocity.x, velocity.z)
  for (let i = 0; i < puffs; i++) {
    const { x, y, z, out } = outlets[i % outlets.length]!
    // Spread the puffs along this frame's travel so a fast car lays a line, not a clump.
    const lag = rand(0, dt)
    world.smoke.spawn({
      position: [x + out.x * 0.15 + rand(-0.12, 0.12) - velocity.x * lag, y + out.y * 0.15 + rand(-0.05, 0.12), z + out.z * 0.15 + rand(-0.12, 0.12) - velocity.z * lag],
      velocity: [
        velocity.x * 0.35 + out.x * 0.8 + WIND.x * rand(0.4, 1) + rand(-0.5, 0.5),
        out.y * 0.8 + rand(1.4, 2.6) + level * 1.8,
        velocity.z * 0.35 + out.z * 0.8 + WIND.z * rand(0.4, 1) + rand(-0.5, 0.5),
      ],
      life: rand(2.2, 3.2) + level * 1.6,
      size: [rand(0.25, 0.4), rand(1.6, 2.4) + level * 1.8 + Math.min(1, speed / 15) * 0.4],
      color: pick(palette),
      alpha: 0.32 + level * 0.3,
      gravity: -0.9,
      drag: 0.55,
    })
  }
  if (level <= 0.65 && !wreck) return owed - puffs
  // A crowded sky thins the fire before the pools start evicting each other.
  const budget = thin * (world.glow.alive > 1400 ? 0.4 : world.glow.alive > 800 ? 0.7 : 1)
  for (const { x, y, z, out, age } of outlets) {
    const fresh = Math.max(0, 1 - age / BURST_SECONDS) ** 2
    // Licks of flame: small fireballs, as a blast's, that cool from yellow to a dark ember red as they rise.
    for (let i = due((FLAMES[0] + FLAMES[1] * fresh) * (wreck ? 1.4 : 1) * budget, dt); i > 0; i--) {
      world.smoke.spawn({
        position: [x + out.x * 0.08 + rand(-0.1, 0.1), y + out.y * 0.08 + rand(-0.05, 0.1), z + out.z * 0.08 + rand(-0.1, 0.1)],
        velocity: [velocity.x * 0.2 + out.x * rand(0.5, 1.5) + rand(-0.3, 0.3), out.y + rand(1.2, 2.8), velocity.z * 0.2 + out.z * rand(0.5, 1.5) + rand(-0.3, 0.3)],
        life: rand(0.3, 0.6),
        size: [rand(0.12, 0.2), rand(0.45, 0.8) * (wreck ? 1.4 : 1)],
        color: i % 3 === 0 ? 0xffeaa0 : i % 3 === 1 ? 0xffb347 : 0xff8a2a,
        colorEnd: 0x4a1405,
        glow: 3,
        shape: 'puff',
        spin: [-2, 2],
        alpha: 0.9,
        gravity: -2.5,
        drag: 2,
      })
    }
    // Embers tossed out of the hole: tiny, white-hot to orange, flying in arcs; some bounce and skitter.
    for (let i = due((EMBERS[0] + EMBERS[1] * fresh) * budget, dt); i > 0; i--) {
      const throwSpeed = rand(2, 7) + fresh * rand(0, 6)
      world.glow.spawn({
        position: [x + out.x * 0.05, y + out.y * 0.05, z + out.z * 0.05],
        velocity: [
          velocity.x * 0.3 + out.x * throwSpeed + rand(-2, 2),
          out.y * throwSpeed + rand(1, 4),
          velocity.z * 0.3 + out.z * throwSpeed + rand(-2, 2),
        ],
        life: rand(0.5, 1.3),
        size: [rand(0.035, 0.07), 0.012],
        color: i % 4 === 0 ? 0xffffff : i % 4 === 1 ? 0xffe08a : i % 4 === 2 ? 0xffa23a : 0xff6a1a,
        colorEnd: 0xff3a05,
        glow: 6,
        stretch: 0.03,
        alpha: 1,
        gravity: 9.8,
        drag: 0.5,
        bounce: i % 3 === 0,
      })
    }
    // A few that ride the hot air up instead of falling.
    for (let i = due((2 + 8 * fresh) * budget, dt); i > 0; i--) {
      world.glow.spawn({
        position: [x + rand(-0.15, 0.15), y + rand(0, 0.2), z + rand(-0.15, 0.15)],
        velocity: [WIND.x * rand(0.3, 1) + rand(-0.8, 0.8), rand(1.5, 4), WIND.z * rand(0.3, 1) + rand(-0.8, 0.8)],
        life: rand(1.2, 2.4),
        size: [0.06, 0.015],
        color: 0xffb04a,
        colorEnd: 0xff2a00,
        glow: 4,
        spin: [-6, 6],
        alpha: 0.9,
        gravity: -0.6,
        drag: 1,
      })
    }
  }
  return owed - puffs
}

/**
 * The exhaust of a rocket in flight for the `dt` seconds just passed: billowing grey-white smoke that hangs in the air where the rocket has been, thinning and drifting with the
 * wind. Puffs are spread along this frame's travel so a fast rocket lays a line, not a string of blobs.
 * Returns the unspent fraction of a puff, which the caller carries to the next frame.
 */
export function rocketTrail(
  world: FxWorld,
  position: readonly [number, number, number],
  velocity: readonly [number, number, number],
  dt: number,
  carried: number,
): number {
  const distance = Math.hypot(position[0] - world.camera.x, position[1] - world.camera.y, position[2] - world.camera.z)
  if (distance > RANGE * 2) return 0
  const owed = carried + dt * (distance > RANGE ? 40 : 70)
  const puffs = Math.floor(owed)
  const speed = Math.hypot(velocity[0], velocity[1], velocity[2]) || 1
  const back = [-velocity[0] / speed, -velocity[1] / speed, -velocity[2] / speed] as const
  for (let i = 0; i < puffs; i++) {
    // Somewhere along the stretch flown this frame, plus the nozzle's offset behind the body.
    const along = 0.5 + rand(0, speed * dt)
    world.smoke.spawn({
      position: [position[0] + back[0] * along + rand(-0.05, 0.05), position[1] + back[1] * along + rand(-0.05, 0.05), position[2] + back[2] * along + rand(-0.05, 0.05)],
      velocity: [WIND.x * rand(0.3, 1) + rand(-0.3, 0.3), rand(0.2, 0.7), WIND.z * rand(0.3, 1) + rand(-0.3, 0.3)],
      life: rand(1.8, 2.8),
      size: [rand(0.2, 0.3), rand(1.0, 1.6)],
      color: pick(WHITE),
      colorEnd: 0x8c8c88,
      alpha: 0.5,
      gravity: -0.2,
      drag: 0.8,
      shape: 'puff',
      spin: [-0.8, 0.8],
    })
  }
  return owed - puffs
}
