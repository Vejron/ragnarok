/**
 * Replicated components in the game's units, and blended between two snapshots. Both are driven by
 * the units the Rust schema gives each field (`Unit`), so nothing here knows any component by name: a
 * new component, or a new field, is converted and interpolated as its unit says.
 *
 * - `cm`, `mrad`, `quat` and `axis` are interpolated: positions, angles (the short way round), rotations
 *   (as a whole, normalised) and steering.
 * - Everything else (counters, flags, ids, health, gauges) is taken from the nearer snapshot.
 */

import { SCHEMA, type Components, type FieldSchema, type Unit } from '@/net/components.generated'
import { wrapAngle } from '@/net/codec'

/** A rotation: a replicated quaternion's x, y, z with its `w` rebuilt. */
export interface Rotation {
  qx: number
  qy: number
  qz: number
  qw: number
}

/** An entity in the game's units: the components it has, every field scaled by its unit. */
export type Entity = Omit<Components, 'orientation'> & { orientation?: Rotation }

type Fields = Record<string, unknown>

/** What one of each unit's integers is worth in the game's units. */
const SCALE: Record<Unit, number> = { cm: 100, mrad: 1000, quat: 32767, axis: 127, byte: 255, dm: 10 }
const BLENDED: ReadonlySet<Unit | undefined> = new Set<Unit | undefined>(['cm', 'mrad', 'quat', 'axis'])

const isRotation = (fields: readonly FieldSchema[]) => fields.length > 0 && fields.every((field) => field.unit === 'quat')

function value(raw: unknown, field: FieldSchema): unknown {
  if (field.struct) return (raw as Fields[]).map((item) => fieldsOf(item, field.struct!))
  const unit = field.unit
  if (!unit) return raw
  const scale = (n: number) => {
    const v = n / SCALE[unit]
    return unit === 'axis' ? Math.max(-1, Math.min(1, v)) : v
  }
  return Array.isArray(raw) ? raw.map(scale) : scale(raw as number)
}

function fieldsOf(raw: Fields, fields: readonly FieldSchema[]): Fields {
  const out: Fields = {}
  for (const field of fields) out[field.name] = value(raw[field.name], field)
  if (isRotation(fields)) {
    const [x, y, z] = fields.map((field) => out[field.name] as number) as [number, number, number]
    out.qw = Math.sqrt(Math.max(0, 1 - x * x - y * y - z * z))
  }
  return out
}

/**
 * Converted components, by the wire object they came from. A component that did not change is the same
 * object from one rebuilt scene to the next, so each is converted once however many scenes hold it.
 */
const converted = new WeakMap<object, Fields>()

/** `wire` in the game's units. */
export function toGame(wire: Components): Entity {
  const entity: Record<string, Fields> = {}
  for (const { name, fields } of SCHEMA) {
    const raw = wire[name] as Fields | undefined
    if (!raw) continue
    let game = converted.get(raw)
    if (!game) converted.set(raw, (game = fieldsOf(raw, fields)))
    entity[name] = game
  }
  return entity as Entity
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t

function blendField(a: unknown, b: unknown, field: FieldSchema, t: number): unknown {
  if (field.struct) {
    // Each item that both have is blended; one only the later has is taken as it is.
    const before = a as Fields[]
    return (b as Fields[]).map((item, i) => (before[i] ? blendFields(before[i], item, field.struct!, t) : item))
  }
  if (!BLENDED.has(field.unit) || typeof a !== 'number' || typeof b !== 'number') return t < 0.5 ? a : b
  return field.unit === 'mrad' ? a + wrapAngle(b - a) * t : lerp(a, b, t)
}

function blendFields(a: Fields, b: Fields, fields: readonly FieldSchema[], t: number): Fields {
  if (a === b) return a
  if (isRotation(fields)) return blendRotation(a as unknown as Rotation, b as unknown as Rotation, t) as unknown as Fields
  const out: Fields = {}
  for (const field of fields) out[field.name] = blendField(a[field.name], b[field.name], field, t)
  return out
}

/** Normalised lerp, which is plenty for two snapshots 33 ms apart; q and -q are the same rotation. */
function blendRotation(a: Rotation, b: Rotation, t: number): Rotation {
  const flip = a.qx * b.qx + a.qy * b.qy + a.qz * b.qz + a.qw * b.qw < 0 ? -1 : 1
  const qx = lerp(a.qx, b.qx * flip, t)
  const qy = lerp(a.qy, b.qy * flip, t)
  const qz = lerp(a.qz, b.qz * flip, t)
  const qw = lerp(a.qw, b.qw * flip, t)
  const length = Math.hypot(qx, qy, qz, qw) || 1
  return { qx: qx / length, qy: qy / length, qz: qz / length, qw: qw / length }
}

/**
 * `a` and `b`, two snapshots of one entity, a fraction `t` of the way from `a` to `b`. A component only
 * one of them has is the nearer snapshot's to have or not.
 */
export function blend(a: Entity, b: Entity, t: number): Entity {
  if (a === b) return a
  const near = (t < 0.5 ? a : b) as Record<string, Fields | undefined>
  const from = a as Record<string, Fields | undefined>
  const to = b as Record<string, Fields | undefined>
  const out: Record<string, Fields> = {}
  for (const { name, fields } of SCHEMA) {
    const x = from[name]
    const y = to[name]
    if (x && y) out[name] = blendFields(x, y, fields, t)
    else if (near[name]) out[name] = near[name]
  }
  return out as Entity
}
