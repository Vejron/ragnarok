/**
 * Protocol v2's entity state on the client: what each replicated thing is, as the components it has
 * (`crates/shared/src/replicated.rs`). Nothing here knows any component: decoding and applying walk the
 * generated `SCHEMA`, so a new component on the server is decoded here without a change.
 *
 * An update is `[id, set, (cleared), fields…]`: `set` says which components' fields follow, in
 * wire-id order; bit 31 of it says a `cleared` mask follows, of the components the entity no longer
 * has. A component sent as a difference (`diff`) adds to the baseline's when the baseline has it, and
 * is the value itself when it does not.
 */

import { SCHEMA, type ComponentName, type Components, type FieldSchema } from './components.generated'

type Fields = Record<string, unknown>

/** One snapshot's word on one entity: the components it sets, as sent, and the bits it clears. */
export interface Update {
  id: number
  cleared: number
  set: Partial<Record<ComponentName, Fields>>
}

/** What a client holds after a snapshot: every entity it has been told of, by id. */
export type Scene = ReadonlyMap<number, Components>

const has = (mask: number, bit: number) => ((mask >>> bit) & 1) === 1

/** A field's value as sent; a list of structs becomes a list of objects. Null if malformed. */
function field(raw: unknown, schema: FieldSchema): unknown {
  if (!schema.struct) return raw
  if (!Array.isArray(raw)) return null
  const members = schema.struct
  const items: Fields[] = []
  for (const item of raw) {
    if (!Array.isArray(item) || item.length !== members.length) return null
    items.push(Object.fromEntries(members.map((member, i) => [member, item[i]])))
  }
  return items
}

/** Decodes one update, or null if it is malformed. */
export function decodeUpdate(raw: unknown): Update | null {
  if (!Array.isArray(raw) || raw.length < 2) return null
  const [id, set] = raw as [unknown, unknown]
  if (typeof id !== 'number' || typeof set !== 'number') return null
  let at = 2
  let cleared = 0
  if (has(set, 31)) {
    const mask = raw[at++]
    if (typeof mask !== 'number') return null
    cleared = mask
  }
  const components: Update['set'] = {}
  for (const component of SCHEMA) {
    if (!has(set, component.id)) continue
    const fields: Fields = {}
    for (const schema of component.fields) {
      if (at >= raw.length) return null
      const value = field(raw[at++], schema)
      if (value === null) return null
      fields[schema.name] = value
    }
    components[component.name] = fields
  }
  return at === raw.length ? { id, cleared, set: components } : null
}

/** Decodes a snapshot's list of updates, or null if any is malformed. */
export function decodeUpdates(raw: unknown): Update[] | null {
  if (!Array.isArray(raw)) return null
  const updates: Update[] = []
  for (const item of raw) {
    const update = decodeUpdate(item)
    if (!update) return null
    updates.push(update)
  }
  return updates
}

/** The entity `update` makes of the baseline's `base` (or of nothing). */
export function applyUpdate(base: Components | undefined, update: Update): Components {
  const now: Record<string, unknown> = { ...base }
  for (const component of SCHEMA) {
    if (has(update.cleared, component.id)) delete now[component.name]
    const wire = update.set[component.name]
    if (!wire) continue
    const was = base?.[component.name] as Fields | undefined
    now[component.name] = component.diff && was ? Object.fromEntries(component.fields.map(({ name }) => [name, (was[name] as number) + (wire[name] as number)])) : wire
  }
  return now as Components
}

/** The scene `updates` and `removed` make of `base`. `base` is left as it was: the client keeps it. */
export function applyUpdates(base: Scene, updates: readonly Update[], removed: readonly number[]): Map<number, Components> {
  const scene = new Map(base)
  for (const id of removed) scene.delete(id)
  for (const update of updates) scene.set(update.id, applyUpdate(scene.get(update.id), update))
  return scene
}
