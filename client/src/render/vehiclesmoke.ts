import * as THREE from 'three'
import { KIND_HELI, KIND_PLANE, KIND_STRV, KIND_TANK } from '@/net/codec'
import type { FxWorld } from './tyres'

/** Past this the plume is not drawn at all, metres; past HALF_RANGE it is thinned. */
const RANGE = 140
const HALF_RANGE = 70
/** A steady breeze that every plume leans with, m/s. */
const WIND = new THREE.Vector3(1.1, 0, 0.5)

const rand = (low: number, high: number) => low + Math.random() * (high - low)
const pick = <T>(items: readonly T[]) => items[Math.floor(Math.random() * items.length)]!

/** Where the engine sits in the hull's frame: the front of a car, the rear of a tank, the top of a heli. */
function engineBay(kind: number): [number, number, number] {
  if (kind === KIND_TANK) return [0, 1.3, 1.6]
  if (kind === KIND_STRV) return [0, 1.1, 1.8]
  if (kind === KIND_HELI) return [0, 1.6, 0.4]
  if (kind === KIND_PLANE) return [0, 0.5, -1.8]
  return [0, 0.75, -1.35]
}

const WHITE = [0xd8d8d4, 0xc4c4c0, 0xb0b0ac] as const
const GREY = [0x8a8885, 0x777572, 0x686663] as const
const DARK = [0x3a3836, 0x2a2928, 0x1c1b1a] as const
const SOOT = [0x161514, 0x0f0e0e, 0x1f1c1a] as const

/**
 * The plume of a hurt vehicle for the `dt` seconds just passed. `severity` runs 0..1 from the first
 * wisp of steam to a thick black column; a `wreck` also burns. Puffs start small and hot at the engine,
 * leave the vehicle behind as it drives, rise, swell and grey out as they cool, and lean with the wind.
 * Returns the unspent fraction of a puff, which the caller carries to the next frame.
 */
export function damageSmoke(
  world: FxWorld,
  kind: number,
  position: readonly [number, number, number],
  rotation: THREE.Quaternion,
  velocity: THREE.Vector3,
  severity: number,
  wreck: boolean,
  dt: number,
  carried: number,
): number {
  const at = new THREE.Vector3(...position)
  const distance = at.distanceTo(world.camera)
  if (distance > RANGE) return 0
  const level = wreck ? 1 : Math.min(1, Math.max(0, severity))
  const thin = distance > HALF_RANGE ? 0.5 : 1
  // Puffs per second: a trickle when scuffed, a rolling column when ablaze.
  const owed = carried + dt * (6 + level * 26) * thin
  const puffs = Math.floor(owed)
  const bay = new THREE.Vector3(...engineBay(kind)).applyQuaternion(rotation).add(at)
  const palette = wreck ? SOOT : level > 0.65 ? DARK : level > 0.3 ? GREY : WHITE
  const speed = Math.hypot(velocity.x, velocity.z)
  for (let i = 0; i < puffs; i++) {
    // Spread the puffs along this frame's travel so a fast car lays a line, not a clump.
    const lag = rand(0, dt)
    world.smoke.spawn({
      position: [bay.x + rand(-0.25, 0.25) - velocity.x * lag, bay.y + rand(-0.1, 0.2), bay.z + rand(-0.25, 0.25) - velocity.z * lag],
      velocity: [
        velocity.x * 0.35 + WIND.x * rand(0.4, 1) + rand(-0.5, 0.5),
        rand(1.4, 2.6) + level * 1.8,
        velocity.z * 0.35 + WIND.z * rand(0.4, 1) + rand(-0.5, 0.5),
      ],
      life: rand(2.2, 3.2) + level * 1.6,
      size: [rand(0.3, 0.5), rand(1.6, 2.4) + level * 1.8 + Math.min(1, speed / 15) * 0.4],
      color: pick(palette),
      alpha: 0.32 + level * 0.3,
      gravity: -0.9,
      drag: 0.55,
    })
  }
  // Fire and falling sparks: only once it is really going, and flickering rather than steady.
  if (level > 0.65 || wreck) {
    const flames = Math.floor(dt * (wreck ? 30 : 12) * thin + Math.random())
    for (let i = 0; i < flames; i++) {
      world.glow.spawn({
        position: [bay.x + rand(-0.4, 0.4), bay.y + rand(0, 0.3), bay.z + rand(-0.4, 0.4)],
        velocity: [velocity.x * 0.2 + rand(-0.4, 0.4), rand(1.5, 3.5), velocity.z * 0.2 + rand(-0.4, 0.4)],
        life: rand(0.18, 0.4),
        size: [rand(0.35, 0.6), 0.08],
        color: i % 3 === 0 ? 0xffd27a : i % 3 === 1 ? 0xff9a2e : 0xe0480f,
        alpha: 0.85,
        gravity: -3,
        drag: 1.5,
      })
    }
    if (Math.random() < dt * (wreck ? 6 : 2) * thin) {
      world.glow.spawn({
        position: bay,
        velocity: [rand(-2.5, 2.5), rand(2, 5), rand(-2.5, 2.5)],
        life: rand(0.6, 1.2),
        size: [0.1, 0.03],
        color: 0xffa23a,
        alpha: 1,
        gravity: 9.8,
        drag: 0.4,
        bounce: true,
      })
    }
  }
  return owed - puffs
}
