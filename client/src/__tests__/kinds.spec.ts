import { describe, expect, it } from 'vitest'
import { KINDS, kindView } from '@/kinds'
import { VEHICLES } from '@/sim/vehicles'

describe('the kinds', () => {
  it('are the server’s vehicle kinds, one each, in its order', () => {
    expect(KINDS.map((kind) => kind.kind)).toEqual(VEHICLES.map((def) => def.kind))
  })

  it('draw an unknown kind as a jeep', () => {
    expect(kindView(200)).toBe(kindView(VEHICLES[0]!.kind))
  })
})
