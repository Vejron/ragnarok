import { describe, expect, it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { kindView } from '@/kinds'
import { KIND_MISSILE, KIND_SUBMARINE, SUB_MAST_UP } from '@/net/codec'
import { MissileModel } from '@/render/missile'
import { SubmarineModel } from '@/render/submarine'
import { seatHint } from '@/seathints'
import { seatView } from '@/seats'
import { SUB_MAST_HEIGHT, SUB_SAIL_Z, SUB_TUBE_AT } from '@/sim/consts'
import { isDrone, vehicleDef } from '@/sim/vehicles'
import { triangleCount } from './helpers'

const IDENTITY = [0, 0, 0, 1] as const
const sub = (tubes: number, scope: number, flags: number) => ({ tubes, scope, flags })
const STATS = { ammo: null, reload: 1, range: 0, canSwitchSeat: false, jammed: false, heat: null }

/** Draws `model` with `state` over `seconds`, a frame at a time, so its parts ease all the way there. */
const settle = (model: SubmarineModel, state: ReturnType<typeof sub> | null, seconds = 2) => {
  for (let t = 0; t < seconds; t += 0.05) model.setSub(state, 0.05)
  model.update([0, 0, 0], IDENTITY, 0, [1, 2])
  model.root.updateMatrixWorld(true)
}

describe('the submarine', () => {
  it('is a sealed two-seat kind: the captain drives, the officer works the periscope', () => {
    const def = vehicleDef(KIND_SUBMARINE)
    expect(def.seats).toBe(2)
    expect(def.crew).toBe('sealed')
    expect(seatView(KIND_SUBMARINE, 0).role).toBe('captain')
    expect(seatView(KIND_SUBMARINE, 1).rig.name).toBe('periscope')
    expect(seatView(KIND_SUBMARINE, 1).sight).toBe(true)
  })

  it('raises its periscope to its head height while the officer is at it, turned with their look', () => {
    const model = new SubmarineModel(1)
    settle(model, sub(0b1111, 0, 0))
    expect(model.periscope.visible).toBe(false)
    settle(model, sub(0b1111, 1.2, SUB_MAST_UP))
    expect(model.periscope.visible).toBe(true)
    expect(model.periscope.rotation.y).toBeCloseTo(1.2)
    // Its head (the mast's second part) stands just under where the camera's eye goes.
    const head = model.periscope.children[1]!.getWorldPosition(new Vector3())
    expect(head.z).toBeCloseTo(SUB_SAIL_Z)
    expect(head.y).toBeCloseTo(SUB_MAST_HEIGHT, 0)
  })

  it('opens the lid of each empty tube and shuts it once the tube is loaded again', () => {
    const model = new SubmarineModel(1)
    expect(model.lids).toHaveLength(SUB_TUBE_AT.length)
    settle(model, sub(0b1010, 0, 0))
    expect(model.lids.map((lid) => lid.rotation.x > 1)).toEqual([true, false, true, false])
    settle(model, sub(0b1111, 0, 0))
    expect(model.lids.every((lid) => lid.rotation.x === 0)).toBe(true)
  })

  it('stays within the triangle budget', () => {
    expect(triangleCount(new SubmarineModel(1).root)).toBeLessThan(2500)
    expect(triangleCount(new MissileModel(1).root)).toBeLessThan(600)
  })

  it('says how deep it is to its captain, and what the tubes can do to its officer', () => {
    expect(seatHint('captain', { ...STATS, depth: 4.25 })).toContain('Depth 4.3 m')
    expect(seatHint('periscope', { ...STATS, tubes: { loaded: 3, clear: true } })).toContain('3 tubes ready')
    expect(seatHint('periscope', { ...STATS, tubes: { loaded: 0, clear: true } })).toContain('reloading')
    expect(seatHint('periscope', { ...STATS, tubes: { loaded: 4, clear: false } })).toContain('too deep')
  })
})

describe('the guided missile', () => {
  it('is flown from afar through its nose, as a drone is', () => {
    expect(isDrone(KIND_MISSILE)).toBe(true)
    expect(seatView(KIND_MISSILE, 0).rig.name).toBe('fpv')
    expect(kindView(KIND_MISSILE).fx.exhaust).toBe('rocket')
  })
})
