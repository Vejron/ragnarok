// GENERATED from crates/shared/src/replicated.rs by its tests. Do not edit: change the Rust and
// run `UPDATE_PARITY=1 cargo test -p shared`. What each unit means, and how it blends, is documented
// at `UNITS` there.

import { angle, items, lerp, rotation, rotationOf } from './blending'
import type { Components } from '@/net/components.generated'
import type { Entity } from './units'

/** Entity `w`, as the wire has it, in the game's units. A component with no units is `w`'s own. */
export function toGame(w: Components): Entity {
  return {
    place: w.place && { x: w.place.x / 100, y: w.place.y / 100, z: w.place.z / 100 },
    heading: w.heading && { yaw: w.heading.yaw / 1000 },
    motion: w.motion && { vy: w.motion.vy / 100, knock_x: w.motion.knock_x / 100, knock_z: w.motion.knock_z / 100 },
    flags: w.flags,
    orientation: w.orientation && rotationOf(w.orientation.qx, w.orientation.qy, w.orientation.qz),
    steer: w.steer && { steer: Math.max(-1, Math.min(1, w.steer.steer / 127)) },
    turret: w.turret && { yaw: w.turret.yaw / 1000, pitch: w.turret.pitch / 1000, shots: w.turret.shots, range: w.turret.range / 10, flags: w.turret.flags },
    crew: w.crew && { riders: w.crew.riders.map((r) => ({ yaw: r.yaw / 1000, pitch: r.pitch / 1000, held: r.held })) },
    thrust: w.thrust && { air: w.thrust.air / 255, flags: w.thrust.flags },
    arms: w.arms && { shots: w.arms.shots, heat: w.arms.heat / 255, rear_heat: w.arms.rear_heat / 255, bombs: w.arms.bombs, flags: w.arms.flags },
    seats: w.seats,
    health: w.health,
    damage: w.damage,
    velocity: w.velocity && { vx: w.velocity.vx / 100, vy: w.velocity.vy / 100, vz: w.velocity.vz / 100 },
    kind: w.kind,
    projectile: w.projectile,
    pickup: w.pickup,
  }
}

/**
 * `a` and `b`, two snapshots of one entity, a fraction `t` of the way from `a` to `b`. A component only
 * one of them has is the nearer snapshot's to have or not.
 */
export function blend(a: Entity, b: Entity, t: number): Entity {
  if (a === b) return a
  const near = t < 0.5 ? a : b
  return {
    place: a.place && b.place ? (a.place === b.place || (a.place.x === b.place.x && a.place.y === b.place.y && a.place.z === b.place.z) ? b.place : { x: lerp(a.place.x, b.place.x, t), y: lerp(a.place.y, b.place.y, t), z: lerp(a.place.z, b.place.z, t) }) : near.place,
    heading: a.heading && b.heading ? (a.heading === b.heading || (a.heading.yaw === b.heading.yaw) ? b.heading : { yaw: angle(a.heading.yaw, b.heading.yaw, t) }) : near.heading,
    motion: a.motion && b.motion ? (a.motion === b.motion || (a.motion.vy === b.motion.vy && a.motion.knock_x === b.motion.knock_x && a.motion.knock_z === b.motion.knock_z) ? b.motion : { vy: lerp(a.motion.vy, b.motion.vy, t), knock_x: lerp(a.motion.knock_x, b.motion.knock_x, t), knock_z: lerp(a.motion.knock_z, b.motion.knock_z, t) }) : near.motion,
    flags: near.flags,
    orientation: a.orientation && b.orientation ? (a.orientation === b.orientation || (a.orientation.qx === b.orientation.qx && a.orientation.qy === b.orientation.qy && a.orientation.qz === b.orientation.qz) ? b.orientation : rotation(a.orientation, b.orientation, t)) : near.orientation,
    steer: a.steer && b.steer ? (a.steer === b.steer || (a.steer.steer === b.steer.steer) ? b.steer : { steer: lerp(a.steer.steer, b.steer.steer, t) }) : near.steer,
    turret: a.turret && b.turret ? (a.turret === b.turret || (a.turret.yaw === b.turret.yaw && a.turret.pitch === b.turret.pitch && a.turret.shots === b.turret.shots && a.turret.range === b.turret.range && a.turret.flags === b.turret.flags) ? b.turret : { yaw: angle(a.turret.yaw, b.turret.yaw, t), pitch: angle(a.turret.pitch, b.turret.pitch, t), shots: t < 0.5 ? a.turret.shots : b.turret.shots, range: t < 0.5 ? a.turret.range : b.turret.range, flags: t < 0.5 ? a.turret.flags : b.turret.flags }) : near.turret,
    crew: a.crew && b.crew ? (a.crew === b.crew ? b.crew : { riders: items(a.crew.riders, b.crew.riders, (p, q) => ({ yaw: angle(p.yaw, q.yaw, t), pitch: angle(p.pitch, q.pitch, t), held: t < 0.5 ? p.held : q.held })) }) : near.crew,
    thrust: near.thrust,
    arms: near.arms,
    seats: near.seats,
    health: near.health,
    damage: near.damage,
    velocity: a.velocity && b.velocity ? (a.velocity === b.velocity || (a.velocity.vx === b.velocity.vx && a.velocity.vy === b.velocity.vy && a.velocity.vz === b.velocity.vz) ? b.velocity : { vx: lerp(a.velocity.vx, b.velocity.vx, t), vy: lerp(a.velocity.vy, b.velocity.vy, t), vz: lerp(a.velocity.vz, b.velocity.vz, t) }) : near.velocity,
    kind: near.kind,
    projectile: near.projectile,
    pickup: near.pickup,
  }
}
