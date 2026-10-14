import { GRAVITY, TICK_DT } from './consts'
import { GRENADE_BOUNCE, GRENADE_FUSE_TICKS, GRENADE_LOFT, GRENADE_MUZZLE, GRENADE_RADIUS, GRENADE_SPEED, MAX_AIM_PITCH } from './items'
import type { Vec3 } from './physics'

/** Casts a ray: how far it travels before it meets something, and that surface's normal. */
export type RayCaster = (origin: Vec3, dir: Vec3, max: number) => { t: number; normal: Vec3 } | null

export interface Trajectory {
  /** Where the grenade is at every `every`-th tick, from the hand to where it bursts. */
  points: Vec3[]
  /** Where it first meets something, or null if the fuse runs out in the air. */
  firstImpact: Vec3 | null
  /** How many of `points` are before that first impact: the flight, as against the roll that follows. */
  flying: number
  /** Where it is when the fuse runs out: the centre of the blast. */
  burst: Vec3
}

const KILL_Y = -50

/**
 * Where a grenade thrown from `eye` while looking along `yaw` and `pitch` goes, stepped exactly like
 * the server's `step_grenade`: gravity, a bounce off whatever a ray meets, rest when slow, and a
 * burst when the fuse runs out. It ignores people, who a real grenade also passes through.
 */
export function throwTrajectory(cast: RayCaster, eye: Vec3, yaw: number, pitch: number, every = 2): Trajectory {
  const aim = Math.max(-MAX_AIM_PITCH, Math.min(MAX_AIM_PITCH, pitch + GRENADE_LOFT))
  const flat = Math.cos(aim)
  const dir: Vec3 = [-Math.sin(yaw) * flat, Math.sin(aim), -Math.cos(yaw) * flat]

  // Up against a wall it starts at the wall instead of on its far side.
  const wall = cast(eye, dir, GRENADE_MUZZLE + GRENADE_RADIUS)
  const reach = wall ? Math.max(0, wall.t - GRENADE_RADIUS) : GRENADE_MUZZLE
  let position: Vec3 = [eye[0] + dir[0] * reach, eye[1] + dir[1] * reach, eye[2] + dir[2] * reach]
  let velocity: Vec3 = [dir[0] * GRENADE_SPEED, dir[1] * GRENADE_SPEED, dir[2] * GRENADE_SPEED]

  const points: Vec3[] = [position]
  let firstImpact: Vec3 | null = null
  let flying = -1
  for (let tick = 1; tick <= GRENADE_FUSE_TICKS; tick++) {
    velocity = [velocity[0], velocity[1] - GRAVITY * TICK_DT, velocity[2]]
    let remaining = TICK_DT
    for (let pass = 0; pass < 3; pass++) {
      const speed = Math.hypot(velocity[0], velocity[1], velocity[2])
      if (speed < 1e-3) break
      const step = speed * remaining
      const unit: Vec3 = [velocity[0] / speed, velocity[1] / speed, velocity[2] / speed]
      const hit = cast(position, unit, step + GRENADE_RADIUS)
      if (!hit) {
        position = [position[0] + unit[0] * step, position[1] + unit[1] * step, position[2] + unit[2] * step]
        break
      }
      const travel = Math.max(0, Math.min(step, hit.t - GRENADE_RADIUS))
      position = [position[0] + unit[0] * travel, position[1] + unit[1] * travel, position[2] + unit[2] * travel]
      if (firstImpact === null) {
        firstImpact = position
        flying = points.length
      }
      const along = velocity[0] * hit.normal[0] + velocity[1] * hit.normal[1] + velocity[2] * hit.normal[2]
      velocity = [
        (velocity[0] - 2 * along * hit.normal[0]) * GRENADE_BOUNCE,
        (velocity[1] - 2 * along * hit.normal[1]) * GRENADE_BOUNCE,
        (velocity[2] - 2 * along * hit.normal[2]) * GRENADE_BOUNCE,
      ]
      if (velocity[0] ** 2 + velocity[1] ** 2 + velocity[2] ** 2 < 0.5) velocity = [0, 0, 0]
      remaining *= 1 - travel / Math.max(step, 1e-6)
    }
    if (tick % every === 0) points.push(position)
    if (position[1] < KILL_Y) break
  }
  return { points, firstImpact, flying: flying === -1 ? points.length : flying, burst: position }
}

/** How long a dropped bomb is followed, ticks: the server's `SHELL_TTL_TICKS * 2`. */
const BOMB_TTL_TICKS = 12 * 60

/**
 * Where a bomb let go at `start` with the plane's `velocity` falls, stepped exactly like the server's
 * `step_shells`: gravity, then a ray along the step, and it bursts at the first thing it meets.
 */
export function dropTrajectory(cast: RayCaster, start: Vec3, velocity: Vec3, every = 3, ttl = BOMB_TTL_TICKS): Trajectory {
  let position: Vec3 = [...start]
  let v: Vec3 = [...velocity]
  const points: Vec3[] = [position]
  for (let tick = 1; tick <= ttl; tick++) {
    v = [v[0], v[1] - GRAVITY * TICK_DT, v[2]]
    const speed = Math.hypot(v[0], v[1], v[2])
    const unit: Vec3 = [v[0] / speed, v[1] / speed, v[2] / speed]
    const step = speed * TICK_DT
    const hit = cast(position, unit, step)
    if (hit) {
      const impact: Vec3 = [position[0] + unit[0] * hit.t, position[1] + unit[1] * hit.t, position[2] + unit[2] * hit.t]
      points.push(impact)
      return { points, firstImpact: impact, flying: points.length, burst: impact }
    }
    position = [position[0] + unit[0] * step, position[1] + unit[1] * step, position[2] + unit[2] * step]
    if (tick % every === 0) points.push(position)
    if (position[1] < KILL_Y) break
  }
  return { points, firstImpact: null, flying: points.length, burst: position }
}
