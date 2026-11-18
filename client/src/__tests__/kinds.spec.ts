import { describe, expect, it } from 'vitest'
import { KINDS, kindView } from '@/kinds'
import { VEHICLES, vehicleDef } from '@/sim/vehicles'
import * as before from './kinds.before'

describe('the kinds', () => {
  it('are the server’s vehicle kinds, one each, in its order', () => {
    expect(KINDS.map((kind) => kind.kind)).toEqual(VEHICLES.map((def) => def.kind))
  })

  it('draw an unknown kind as a jeep', () => {
    expect(kindView(200)).toBe(kindView(VEHICLES[0]!.kind))
  })
})

describe('each kind there was before kinds were files', () => {
  for (const def of before.BEFORE.map(vehicleDef)) {
    const view = kindView(def.kind)


    it(`leaves on the ground what the ${def.name} did`, () => {
      expect(view.fx.wheels).toBe(before.wheels(def.kind))
      // Which wheels are driven matters only for a kind that has some.
      const count = view.fx.wheels?.contacts.length ?? 0
      for (let wheel = 0; wheel < count; wheel++) expect(view.fx.drivenFrom !== null && wheel >= view.fx.drivenFrom, `wheel ${wheel}`).toBe(before.driven(def.kind, wheel))
      expect(view.fx.exhaust).toBe(before.exhaust(def.kind))
      expect(view.fx.backfire === true).toBe(before.backfire(def.kind))
      expect(view.fx.bay).toEqual(before.bay(def.kind))
    })
  }
})
