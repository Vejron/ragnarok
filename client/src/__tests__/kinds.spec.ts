import { describe, expect, it } from 'vitest'
import { KINDS, kindView } from '@/kinds'
import { mixOf, type SoundInput } from '@/sim/engine'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_WATER } from '@/sim/surface'
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

/** Inputs over the ways a vehicle moves and the states it is in. */
function* inputs(kind: number): Generator<SoundInput> {
  for (const speed of [0, 3, 9, 17, 30, 44, -6])
    for (const accel of [-5, 0, 3])
      for (const climb of [-4, 0, 7])
        for (const [powered, hp] of [[true, 100], [true, 30], [true, 8], [false, 100], [true, 0]] as const)
          for (const grounded of [true, false])
            for (const surface of [SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_WATER])
              for (const [slip, wheels, cut, jets] of [[0, null, false, 0], [0.7, [0, 140, 255, 0], true, 3]] as const)
                yield { kind, speed, accel, climb, powered, hp, maxHp: 100, slip, surface, grounded, wheels, cut, jets }
}

describe('each kind there was before kinds were files', () => {
  for (const def of before.BEFORE.map(vehicleDef)) {
    const view = kindView(def.kind)

    it(`sounds as the ${def.name} did`, () => {
      expect(view.sound.voices).toEqual(before.voices(def.kind))
      expect(view.sound.q).toBe(before.q(def.kind))
      expect(view.sound.chop).toEqual(before.chop(def.kind))
      for (const input of inputs(def.kind)) expect(mixOf(input, view.sound.engine)).toEqual(before.mix(input))
    })

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
