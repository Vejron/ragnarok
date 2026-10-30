/**
 * What the generated `toGame` and `blend` (`units.generated.ts`) are made of: one helper per way a unit
 * converts or blends (`UNITS` in `crates/shared/src/replicated.rs`).
 */

import { wrapAngle } from '@/net/codec'
import type { Rotation } from './units'
import { lerp } from '@/math'

/** A replicated quaternion's x, y, z (scaled by 32767, `w >= 0`) as a rotation. */
export function rotationOf(x: number, y: number, z: number): Rotation {
  const [qx, qy, qz] = [x / 32767, y / 32767, z / 32767]
  return { qx, qy, qz, qw: Math.sqrt(Math.max(0, 1 - qx * qx - qy * qy - qz * qz)) }
}

export { lerp }

/** An angle, turned the short way round. */
export const angle = (a: number, b: number, t: number) => a + wrapAngle(b - a) * t

/** A list of structs: each item that both have is blended; one only the later has is taken as it is. */
export function items<T>(a: readonly T[], b: readonly T[], blend: (p: T, q: T) => T): T[] {
  return b.map((item, i) => (i < a.length ? blend(a[i]!, item) : item))
}

/** Normalised lerp, which is plenty for two snapshots 33 ms apart; q and -q are the same rotation. */
export function rotation(a: Rotation, b: Rotation, t: number): Rotation {
  const flip = a.qx * b.qx + a.qy * b.qy + a.qz * b.qz + a.qw * b.qw < 0 ? -1 : 1
  const qx = lerp(a.qx, b.qx * flip, t)
  const qy = lerp(a.qy, b.qy * flip, t)
  const qz = lerp(a.qz, b.qz * flip, t)
  const qw = lerp(a.qw, b.qw * flip, t)
  const length = Math.hypot(qx, qy, qz, qw) || 1
  return { qx: qx / length, qy: qy / length, qz: qz / length, qw: qw / length }
}
