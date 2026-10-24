/**
 * The local player's bricks, predicted: the server's rules for where a cube may go
 * (`mechanics/building.rs`, `may_place`) and when bricks are used (`ItemKind::Build` in
 * `combat::use_item`), mirrored so a cube is placed or dug on screen at once instead of a round trip
 * later. `WorldView` keeps what was predicted until the server's `built` agrees or the wait runs out.
 */

import { BUILD_COOLDOWN_TICKS, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, MAX_BUILT } from './consts'
import { CELL } from './carve'
import { NEIGHBOURS, cubeCentre, type Construct, type Pos } from './construct'
import type { Vec3 } from './motor'

/** Half a cube's side, metres. Mirrors `HALF_CUBE`. */
export const HALF_CUBE = CELL / 200

/** What the placing rules need to know of the world around the local player. */
export interface BuildWorld {
  construct: Construct
  /** Terrain height at `(x, z)`, metres. */
  groundAt(x: number, z: number): number
  /** Whether a cube of half side `half` at `centre` overlaps a wall, tree, vehicle or placed cubes (`SimMotor.cubeBlocked`). */
  cubeBlocked(centre: Vec3, half: number): boolean
  /** Whether a building's standing cells reach within `radius` of `point` (`SimMotor.buildingNear`). */
  buildingNear(point: Vec3, radius: number): boolean
  /** The local stickman's capsule centre, or null when it is not standing in the world. */
  body: Vec3 | null
}

/**
 * Whether a new cube may go at `pos`, as the server's `may_place` judges it: the world is not full,
 * nothing is there (no cube, wall, tree, vehicle, nor the local player), it is not buried, and it rests
 * on a cube, a building's cell or the ground. Other players' bodies are not looked at: the server
 * refuses a cube in one, and the prediction is taken back.
 */
export function mayPlace(world: BuildWorld, pos: Pos): boolean {
  const { construct } = world
  if (construct.size >= MAX_BUILT || construct.has(pos)) return false
  const middle = cubeCentre(pos)
  const ground = world.groundAt(middle[0], middle[2])
  if (middle[1] + HALF_CUBE <= ground) return false
  if (world.cubeBlocked(middle, HALF_CUBE - 0.02)) return false
  const body = world.body
  if (
    body &&
    Math.abs(body[0] - middle[0]) < HALF_CUBE + CAPSULE_RADIUS &&
    Math.abs(body[2] - middle[2]) < HALF_CUBE + CAPSULE_RADIUS &&
    Math.abs(body[1] - middle[1]) < HALF_CUBE + CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS
  ) {
    return false
  }
  if (NEIGHBOURS.some(([dx, dy, dz]) => construct.has([pos[0] + dx, pos[1] + dy, pos[2] + dz]))) return true
  if ((pos[1] * CELL) / 100 <= ground + 0.05) return true
  // A building's cell beside, above or below it reaches into the ball just past its faces; one that only
  // touches an edge does not.
  return world.buildingNear(middle, HALF_CUBE + 0.05)
}

/**
 * When bricks in hand are used, as the server counts it: once fire is held and the wait since the last
 * use (`BUILD_COOLDOWN_TICKS`) has run out, whether or not anything comes of it. The server's wait is
 * shared with the guns; this one only counts bricks, so just after a shot a cube can show a tick or
 * two before the server places it, where the aim has hardly moved.
 */
export class BrickTrigger {
  private cooldown = 0

  /** One input tick, with bricks in hand and fire held (`using`) or not: whether bricks are used on it. */
  tick(using: boolean): boolean {
    this.cooldown = Math.max(0, this.cooldown - 1)
    if (!using || this.cooldown > 0) return false
    this.cooldown = BUILD_COOLDOWN_TICKS
    return true
  }
}
