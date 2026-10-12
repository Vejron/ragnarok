import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { TICK_RATE } from '../sim/consts'
import * as items from '../sim/items'
import { VEHICLES } from '../sim/vehicles'

// `fixtures/parity/gameplay.json` is generated from the Rust tables by `cargo test -p shared parity`
// (`UPDATE_PARITY=1` rewrites it). A failure here means `client/src/sim/items.ts` drifted from
// `crates/shared/src/items.rs`.
const fixture = JSON.parse(readFileSync(resolve(process.cwd(), '../fixtures/parity/gameplay.json'), 'utf8')) as {
  items: Record<string, Record<string, unknown>>
  vehicles: Record<string, Record<string, unknown>>
  consts: Record<string, number>
}

describe('gameplay tables match the server', () => {
  it('defines every item the server does, with the same numbers', () => {
    for (const [id, expected] of Object.entries(fixture.items)) {
      expect(items.itemDef(Number(id)), `item ${id}`).toEqual(expect.objectContaining(expected))
    }
  })

  it('defines no item the server lacks', () => {
    const ids = Object.keys(fixture.items).map(Number)
    for (let id = 1; id < 32; id++) {
      expect(items.itemDef(id) !== undefined, `item ${id}`).toBe(ids.includes(id))
    }
  })

  it('defines every vehicle kind the server does, with the same facts', () => {
    expect(VEHICLES.length).toBe(Object.keys(fixture.vehicles).length)
    for (const [kind, expected] of Object.entries(fixture.vehicles)) {
      const mirrored = VEHICLES.find((def) => def.kind === Number(kind))
      expect(mirrored, `vehicle kind ${kind}`).toEqual(expect.objectContaining(expected))
    }
  })

  it('shares the scalar constants', () => {
    const mirrored: Record<string, number> = { ...items, TICK_RATE } as never
    for (const [name, value] of Object.entries(fixture.consts)) {
      expect(mirrored[name], name).toBe(value)
    }
  })
})
