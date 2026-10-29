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
    limbs: w.limbs,
    thrust: w.thrust && { air: w.thrust.air / 255, flags: w.thrust.flags },
    arms: w.arms && { shots: w.arms.shots, heat: w.arms.heat / 255, rear_heat: w.arms.rear_heat / 255, bombs: w.arms.bombs, flags: w.arms.flags },
    health: w.health,
    damage: w.damage,
    velocity: w.velocity && { vx: w.velocity.vx / 100, vy: w.velocity.vy / 100, vz: w.velocity.vz / 100 },
    kind: w.kind,
    projectile: w.projectile,
    pickup: w.pickup,
    vitals: w.vitals && { hp: w.vitals.hp, progress: w.vitals.progress / 255, spent: w.vitals.spent, winded: w.vitals.winded, parts: w.vitals.parts },
    hotbar: w.hotbar,
    drive: w.drive && { vx: w.drive.vx / 100, vy: w.drive.vy / 100, vz: w.drive.vz / 100, wx: w.drive.wx / 1000, wy: w.drive.wy / 1000, wz: w.drive.wz / 1000, steer: w.drive.steer / 1000, spin: w.drive.spin / 1000 },
    seated: w.seated,
    look: w.look && { pitch: w.look.pitch / 1000 },
    carry: w.carry,
    struck: w.struck && { count: w.struck.count, part: w.struck.part, amount: w.struck.amount, yaw: w.struck.yaw / 1000 },
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
    orientation: a.orientation && b.orientation ? (a.orientation === b.orientation || (a.orientation.qx === b.orientation.qx && a.orientation.qy === b.orientation.qy && a.orientation.qz === b.orientation.qz && a.orientation.qw === b.orientation.qw) ? b.orientation : rotation(a.orientation, b.orientation, t)) : near.orientation,
    steer: a.steer && b.steer ? (a.steer === b.steer || (a.steer.steer === b.steer.steer) ? b.steer : { steer: lerp(a.steer.steer, b.steer.steer, t) }) : near.steer,
    turret: a.turret && b.turret ? (a.turret === b.turret || (a.turret.yaw === b.turret.yaw && a.turret.pitch === b.turret.pitch && a.turret.shots === b.turret.shots && a.turret.range === b.turret.range && a.turret.flags === b.turret.flags) ? b.turret : { yaw: angle(a.turret.yaw, b.turret.yaw, t), pitch: angle(a.turret.pitch, b.turret.pitch, t), shots: t < 0.5 ? a.turret.shots : b.turret.shots, range: t < 0.5 ? a.turret.range : b.turret.range, flags: t < 0.5 ? a.turret.flags : b.turret.flags }) : near.turret,
    limbs: near.limbs,
    thrust: near.thrust,
    arms: near.arms,
    health: near.health,
    damage: near.damage,
    velocity: a.velocity && b.velocity ? (a.velocity === b.velocity || (a.velocity.vx === b.velocity.vx && a.velocity.vy === b.velocity.vy && a.velocity.vz === b.velocity.vz) ? b.velocity : { vx: lerp(a.velocity.vx, b.velocity.vx, t), vy: lerp(a.velocity.vy, b.velocity.vy, t), vz: lerp(a.velocity.vz, b.velocity.vz, t) }) : near.velocity,
    kind: near.kind,
    projectile: near.projectile,
    pickup: near.pickup,
    vitals: near.vitals,
    hotbar: a.hotbar && b.hotbar ? (a.hotbar === b.hotbar ? b.hotbar : { slots: items(a.hotbar.slots, b.hotbar.slots, (p, q) => ({ item: t < 0.5 ? p.item : q.item, count: t < 0.5 ? p.count : q.count, ammo: t < 0.5 ? p.ammo : q.ammo })) }) : near.hotbar,
    drive: a.drive && b.drive ? (a.drive === b.drive || (a.drive.vx === b.drive.vx && a.drive.vy === b.drive.vy && a.drive.vz === b.drive.vz && a.drive.wx === b.drive.wx && a.drive.wy === b.drive.wy && a.drive.wz === b.drive.wz && a.drive.steer === b.drive.steer && a.drive.spin === b.drive.spin) ? b.drive : { vx: lerp(a.drive.vx, b.drive.vx, t), vy: lerp(a.drive.vy, b.drive.vy, t), vz: lerp(a.drive.vz, b.drive.vz, t), wx: angle(a.drive.wx, b.drive.wx, t), wy: angle(a.drive.wy, b.drive.wy, t), wz: angle(a.drive.wz, b.drive.wz, t), steer: angle(a.drive.steer, b.drive.steer, t), spin: angle(a.drive.spin, b.drive.spin, t) }) : near.drive,
    seated: near.seated,
    look: a.look && b.look ? (a.look === b.look || (a.look.pitch === b.look.pitch) ? b.look : { pitch: angle(a.look.pitch, b.look.pitch, t) }) : near.look,
    carry: near.carry,
    struck: a.struck && b.struck ? (a.struck === b.struck || (a.struck.count === b.struck.count && a.struck.part === b.struck.part && a.struck.amount === b.struck.amount && a.struck.yaw === b.struck.yaw) ? b.struck : { count: t < 0.5 ? a.struck.count : b.struck.count, part: t < 0.5 ? a.struck.part : b.struck.part, amount: t < 0.5 ? a.struck.amount : b.struck.amount, yaw: angle(a.struck.yaw, b.struck.yaw, t) }) : near.struck,
  }
}
