import { MISSILE_HALF_EXTENTS, MISSILE_SPEED } from '@/sim/consts'
import type { FxWorld } from './tyres'

/** Anything with x, y and z. */
type V = { x: number; y: number; z: number }

/** Further from the camera than this, metres, a launch and a missile's exhaust are not drawn. */
const RANGE = 280
/** Further than this, metres, a missile's exhaust is thinned. */
const THIN_RANGE = 140
/** The breeze the smoke leans with, m/s: the vehicles' smoke's wind (`vehiclesmoke.ts`). */
const WIND = [1.1, 0.5] as const

const WHITE = [0xe2e2de, 0xd0d0cc, 0xbcbcb8] as const
const GREY = [0x9a9895, 0x858380, 0x74726f] as const
const FIRE = [0xffe9a0, 0xffc060, 0xff9a30, 0xff7a1a] as const
const SPARK = [0xffffff, 0xffe08a, 0xffa23a, 0xff6a1a] as const

const rand = (low: number, high: number) => low + Math.random() * (high - low)
const pick = <T>(items: readonly T[]) => items[Math.floor(Math.random() * items.length)]!
/** Some number of a thing due at `rate` a second over `dt`, the fraction left to chance. */
const due = (rate: number, dt: number) => Math.floor(rate * dt + Math.random())
const far = (world: FxWorld, at: V) => Math.hypot(at.x - world.camera.x, at.y - world.camera.y, at.z - world.camera.z)

/** Two directions square to `up` and to each other, to spread things round it. */
function across(up: V): [V, V] {
  const length = Math.hypot(up.z, up.x) || 1
  // Level and square to `up`, unless `up` is level itself.
  const a = Math.abs(up.y) > 0.99 ? { x: 1, y: 0, z: 0 } : { x: up.z / length, y: 0, z: -up.x / length }
  return [a, { x: up.y * a.z - up.z * a.y, y: up.z * a.x - up.x * a.z, z: up.x * a.y - up.y * a.x }]
}

/**
 * A missile leaving a submarine's launch tube at `mouth`, which points along `up`. From a tube in the
 * open air: a glare, a column of fire out of the mouth, burning grains thrown up with it, and the
 * motor's smoke rolling out over the deck on every side and climbing after the missile. From a tube
 * under water (`surface` is the water's height over it, metres): the sea over the tube erupts in
 * steam and spray as the missile breaks through with its motor lit.
 */
export function missileLaunch(world: FxWorld, mouth: V, up: V, surface: number | null) {
  if (far(world, mouth) > RANGE) return
  const [a, b] = across(up)
  /** `along` metres up the tube's line and `out` metres off it, `turn` radians round. */
  const off = (along: number, out: number, turn: number): [number, number, number] => {
    const ca = Math.cos(turn) * out, cb = Math.sin(turn) * out
    return [up.x * along + a.x * ca + b.x * cb, up.y * along + a.y * ca + b.y * cb, up.z * along + a.z * ca + b.z * cb]
  }
  const from = (base: V, o: readonly [number, number, number]): [number, number, number] => [base.x + o[0], base.y + o[1], base.z + o[2]]

  if (surface !== null) {
    const at = { x: mouth.x, y: surface, z: mouth.z }
    world.glow.spawn({ position: from(at, [0, 0.8, 0]), velocity: [0, 0, 0], life: 0.22, size: [3, 5.5], color: 0xffb060, glow: 7, alpha: 1, shape: 'glare' })
    // Steam bursting out over the water and boiling up.
    for (let i = 0; i < 26; i++) {
      const turn = rand(0, Math.PI * 2), out = rand(2, 9)
      world.smoke.spawn({
        position: from(at, [Math.cos(turn) * rand(0, 0.8), rand(0.1, 0.6), Math.sin(turn) * rand(0, 0.8)]),
        velocity: [Math.cos(turn) * out + WIND[0], rand(0.5, 5), Math.sin(turn) * out + WIND[1]],
        life: rand(2.5, 5), size: [rand(0.8, 1.4), rand(3.5, 6)], color: pick(WHITE), colorEnd: 0xb8bcbc, alpha: rand(0.5, 0.7),
        gravity: -0.5, drag: 1.8, shape: 'puff', spin: [-0.7, 0.7],
      })
    }
    // A column of spray thrown up round the missile.
    for (let i = 0; i < 34; i++) {
      const turn = rand(0, Math.PI * 2), out = rand(0.3, 3)
      world.smoke.spawn({
        position: from(at, [Math.cos(turn) * 0.3, 0.05, Math.sin(turn) * 0.3]),
        velocity: [Math.cos(turn) * out, rand(6, 17), Math.sin(turn) * out],
        life: rand(0.9, 1.9), size: [rand(0.1, 0.22), 0.05], color: 0xeaf5ff, alpha: 0.85, gravity: 9.8,
      })
    }
    // The motor's fire, seen through it.
    for (let i = 0; i < 10; i++) {
      const o = off(rand(0.4, 2.5), rand(0, 0.4), rand(0, Math.PI * 2))
      world.smoke.spawn({ position: from(at, o), velocity: [o[0] * 2, rand(3, 9), o[2] * 2], life: rand(0.25, 0.5), size: [0.6, rand(1.4, 2.2)], color: pick(FIRE), colorEnd: 0x4a1405, glow: 3, alpha: 0.9, gravity: -2.5, drag: 3, spin: [-2, 2], shape: 'puff' })
    }
    world.ripple?.(at, 4)
    return
  }

  world.glow.spawn({ position: from(mouth, off(0.6, 0, 0)), velocity: [0, 0, 0], life: 0.2, size: [3.5, 6.5], color: 0xffb060, glow: 8, alpha: 1, shape: 'glare' })
  world.glow.spawn({ position: from(mouth, off(0.3, 0, 0)), velocity: [0, 0, 0], life: 0.1, size: [2, 3.5], color: 0xfff2c8, glow: 12, alpha: 1, shape: 'glare' })
  // The column of fire out of the mouth.
  for (let i = 0; i < 18; i++) {
    const speed = rand(5, 18)
    const lean = off(speed, rand(0, 1.5), rand(0, Math.PI * 2))
    world.smoke.spawn({
      position: from(mouth, off(rand(0, 1.6), rand(0, 0.2), rand(0, Math.PI * 2))), velocity: lean,
      life: rand(0.25, 0.6), size: [rand(0.5, 0.8), rand(1.4, 2.4)], color: pick(FIRE), colorEnd: 0x4a1405, glow: 3, alpha: 0.9, gravity: -2.5, drag: 3, spin: [-2, 2], shape: 'puff',
    })
  }
  // Burning grains thrown up and out, falling back on arcs.
  for (let i = 0; i < 26; i++) {
    world.glow.spawn({
      position: from(mouth, off(0.3, 0, 0)), velocity: off(rand(6, 20), rand(1, 7), rand(0, Math.PI * 2)),
      life: rand(0.5, 1.3), size: [rand(0.07, 0.12), 0.03], color: pick(SPARK), colorEnd: 0xff3a05, glow: 6, stretch: 0.03, alpha: 1, gravity: 9.8, drag: 0.6,
    })
  }
  // The blast turned back out of the tube rolls over the deck on every side.
  for (let i = 0; i < 30; i++) {
    const turn = (i / 30) * Math.PI * 2 + rand(-0.2, 0.2)
    const v = off(rand(0.2, 1.4), rand(3, 10), turn)
    world.smoke.spawn({
      position: from(mouth, off(rand(0.1, 0.4), rand(0.2, 0.6), turn)), velocity: [v[0] + WIND[0], v[1], v[2] + WIND[1]],
      life: rand(3.5, 6.5), size: [rand(0.7, 1.1), rand(3.5, 6)], color: pick(i % 3 ? WHITE : GREY), colorEnd: 0x8c8c88, alpha: rand(0.5, 0.7),
      gravity: -0.3, drag: 1.5, shape: 'puff', spin: [-0.7, 0.7],
    })
  }
  // And a thick column of it climbs after the missile.
  for (let i = 0; i < 16; i++) {
    const v = off(rand(2, 11), rand(0, 1.2), rand(0, Math.PI * 2))
    world.smoke.spawn({
      position: from(mouth, off(rand(0.3, 2.5), rand(0, 0.4), rand(0, Math.PI * 2))), velocity: [v[0] + WIND[0], v[1], v[2] + WIND[1]],
      life: rand(3, 5.5), size: [rand(0.6, 1), rand(3, 4.8)], color: pick(WHITE), colorEnd: 0x8c8c88, alpha: rand(0.5, 0.65),
      gravity: -0.4, drag: 1.3, shape: 'puff', spin: [-0.7, 0.7],
    })
  }
}

/**
 * A tube that has just fired, for the `dt` seconds just passed: smoke pours out of its open mouth at
 * `mouth` along `up` and thins to a wisp as `heat` runs from 1 down to 0, with licks of flame in it at
 * first. Nothing comes out of one that is under water.
 */
export function tubeSmoke(world: FxWorld, mouth: V, up: V, heat: number, dt: number) {
  if (far(world, mouth) > THIN_RANGE) return
  for (let i = due(8 + 30 * heat * heat, dt); i > 0; i--) {
    const speed = rand(0.8, 2 + 5 * heat)
    world.smoke.spawn({
      position: [mouth.x + rand(-0.15, 0.15), mouth.y + 0.1, mouth.z + rand(-0.15, 0.15)],
      velocity: [up.x * speed + WIND[0] * rand(0.3, 1) + rand(-0.4, 0.4), up.y * speed, up.z * speed + WIND[1] * rand(0.3, 1) + rand(-0.4, 0.4)],
      life: rand(1.8, 3.2), size: [rand(0.3, 0.45), rand(1.3, 2.4)], color: pick(heat > 0.5 ? WHITE : GREY), colorEnd: 0x6c6c69, alpha: 0.25 + 0.3 * heat,
      gravity: -0.5, drag: 1.2, shape: 'puff', spin: [-0.8, 0.8],
    })
  }
  if (heat < 0.75) return
  for (let i = due(26, dt); i > 0; i--) {
    const speed = rand(2, 6)
    world.smoke.spawn({
      position: [mouth.x + rand(-0.12, 0.12), mouth.y + 0.1, mouth.z + rand(-0.12, 0.12)], velocity: [up.x * speed, up.y * speed, up.z * speed],
      life: rand(0.2, 0.4), size: [0.4, rand(0.7, 1.1)], color: pick(FIRE), colorEnd: 0x4a1405, glow: 3, alpha: 0.85, gravity: -2.5, drag: 3, spin: [-2, 2], shape: 'puff',
    })
  }
}

/**
 * The exhaust of a guided missile's motor for the `dt` seconds just passed: a jet of fire behind its
 * nozzle and a thick trail of white smoke hanging where it has flown. Both are laid along this
 * frame's travel, so that a fast missile draws a line and not a string of blobs. Slow off the tube,
 * while it still gathers its `MISSILE_SPEED`, it burns hardest: the fire is longer and sheds burning
 * grains, and the smoke comes thicker and lasts longer. Returns the unspent fraction of a puff, which
 * the caller carries to the next frame.
 */
export function missileExhaust(world: FxWorld, position: readonly [number, number, number], velocity: readonly [number, number, number], dt: number, carried: number): number {
  const distance = far(world, { x: position[0], y: position[1], z: position[2] })
  if (distance > RANGE) return 0
  const speed = Math.hypot(velocity[0], velocity[1], velocity[2]) || 1
  const boost = Math.min(1, Math.max(0, 1 - speed / MISSILE_SPEED))
  const back = [-velocity[0] / speed, -velocity[1] / speed, -velocity[2] / speed] as const
  const thin = distance > THIN_RANGE ? 0.5 : 1
  /** A point `along` metres behind the nozzle, `loose` metres off the line at most. */
  const behind = (along: number, loose: number): [number, number, number] => {
    const reach = MISSILE_HALF_EXTENTS[2] + along
    return [position[0] + back[0] * reach + rand(-loose, loose), position[1] + back[1] * reach + rand(-loose, loose), position[2] + back[2] * reach + rand(-loose, loose)]
  }

  const owed = carried + dt * (80 + 120 * boost) * thin
  const puffs = Math.floor(owed)
  for (let i = 0; i < puffs; i++) {
    world.smoke.spawn({
      position: behind(0.4 + rand(0, speed * dt), 0.06 + 0.1 * boost),
      velocity: [WIND[0] * rand(0.3, 1) + rand(-0.4, 0.4) + back[0] * 2 * boost, rand(0.2, 0.7) + back[1] * 2 * boost, WIND[1] * rand(0.3, 1) + rand(-0.4, 0.4) + back[2] * 2 * boost],
      life: rand(2.2, 3.4) + 1.5 * boost, size: [rand(0.25, 0.35) + 0.2 * boost, rand(1.3, 2) + 1.4 * boost], color: pick(WHITE), colorEnd: 0x8c8c88, alpha: 0.5 + 0.15 * boost,
      gravity: -0.2, drag: 0.8, shape: 'puff', spin: [-0.8, 0.8],
    })
  }
  // The jet: short-lived fire that keeps most of the missile's speed, so it stays a tongue behind it.
  for (let i = due((70 + 60 * boost) * thin, dt); i > 0; i--) {
    const out = rand(6, 16)
    world.glow.spawn({
      position: behind(rand(0, speed * dt) + rand(0, 0.3), 0.04),
      velocity: [velocity[0] + back[0] * out, velocity[1] + back[1] * out, velocity[2] + back[2] * out],
      life: rand(0.07, 0.14) + 0.1 * boost, size: [rand(0.3, 0.45) + 0.25 * boost, 0.06], color: pick(FIRE), colorEnd: 0xff3a05, glow: 5, alpha: 1, drag: 2,
    })
  }
  for (let i = due(45 * boost * thin, dt); i > 0; i--) {
    const out = rand(4, 12)
    world.glow.spawn({
      position: behind(0.2, 0.05),
      velocity: [back[0] * out + rand(-2.5, 2.5), back[1] * out + rand(-2.5, 2.5), back[2] * out + rand(-2.5, 2.5)],
      life: rand(0.4, 0.9), size: [rand(0.06, 0.1), 0.03], color: pick(SPARK), colorEnd: 0xff3a05, glow: 6, stretch: 0.03, alpha: 1, gravity: 9.8, drag: 0.6,
    })
  }
  return owed - puffs
}
