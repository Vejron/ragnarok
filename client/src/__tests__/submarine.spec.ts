import { describe, expect, it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { kindView } from '@/kinds'
import { KIND_MISSILE, KIND_SUBMARINE, SUB_MAST_UP } from '@/net/codec'
import { missileExhaust, missileLaunch } from '@/render/launch'
import { MissileModel } from '@/render/missile'
import { SubmarineModel } from '@/render/submarine'
import type { FxWorld } from '@/render/tyres'
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

  it('gives its raised periscope away from afar: a glint brightest down the lens, and a feather of foam', () => {
    const model = new SubmarineModel(1)
    // At periscope depth: the water half a metre under the head, over the sail.
    const waterAt = () => SUB_MAST_HEIGHT - 0.5
    const seen = (camera: { x: number; y: number; z: number }) => {
      for (let t = 0; t < 2; t += 0.05) {
        model.update([0, 0, 0], IDENTITY, 0, [1, 2])
        model.setSub(sub(0b1111, 0, SUB_MAST_UP), 0.05, { camera, waterAt })
      }
      return { size: model.glint.scale.x, shine: model.glint.material.opacity }
    }
    // The lens looks along -Z; 400 m out ahead of it, and as far behind.
    const ahead = seen({ x: 0, y: 2, z: -400 })
    const behind = seen({ x: 0, y: 2, z: 400 })
    expect(model.glint.visible).toBe(true)
    expect(ahead.size).toBeGreaterThan(1.5)
    expect(ahead.shine).toBeGreaterThan(2 * behind.shine)
    expect(behind.shine).toBeGreaterThan(0)
    expect(model.collar.visible).toBe(true)
    expect(model.streak.visible).toBe(false)

    // Under way it trails a streak back from the mast.
    for (let t = 0; t < 1; t += 0.05) {
      model.update([0, 0, -t * 4], IDENTITY, 0, [1, 2])
      model.setSub(sub(0b1111, 0, SUB_MAST_UP), 0.05, { camera: { x: 0, y: 2, z: -400 }, waterAt })
    }
    expect(model.streak.visible).toBe(true)
    expect(model.streak.scale.z).toBeGreaterThan(4)

    // Down, nothing shows.
    settle(model, sub(0b1111, 0, 0))
    model.setSub(sub(0b1111, 0, 0), 0.05, { camera: { x: 0, y: 2, z: -400 }, waterAt })
    expect(model.glint.visible || model.collar.visible || model.streak.visible).toBe(false)
  })

  it('opens the lid of each empty tube and shuts it once the tube is loaded again', () => {
    const model = new SubmarineModel(1)
    expect(model.lids).toHaveLength(SUB_TUBE_AT.length)
    settle(model, sub(0b1010, 0, 0))
    // An open lid stands up over the deck, swung towards the bow.
    const lifted = model.lids.map((lid) => lid.children[0]!.getWorldPosition(new Vector3()).y - lid.position.y)
    expect(lifted.map((height) => height > 0.25)).toEqual([true, false, true, false])
    settle(model, sub(0b1111, 0, 0))
    expect(model.lids.every((lid) => Math.abs(lid.rotation.x) === 0)).toBe(true)
  })

  it('tells a launch by a tube that was loaded and is not any more, and smokes from it for a while', () => {
    const model = new SubmarineModel(1)
    // Seen first with a tube already empty: nothing was launched while it was watched.
    model.setSub(sub(0b1110, 0, 0), 0.05)
    expect(model.takeLaunches()).toEqual([])
    model.setSub(sub(0b1010, 0, 0), 0.05)
    expect(model.takeLaunches()).toEqual([2])
    expect(model.takeLaunches()).toEqual([])
    expect(model.smoking.map((left) => left > 0)).toEqual([false, false, true, false])
    model.update([10, 0, 0], IDENTITY, 0, [1, 2])
    const { at, up } = model.tubeMouth(2)
    expect([at.x - 10, at.y, at.z]).toEqual([...SUB_TUBE_AT[2]].map((v) => expect.closeTo(v)))
    expect(up.y).toBeCloseTo(1)
    // A reload is no launch, and the smoke has stopped long before it.
    settle(model, sub(0b1111, 0, 0), 5)
    expect(model.takeLaunches()).toEqual([])
    expect(model.smoking.every((left) => left === 0)).toBe(true)
  })

  it('throws fire, sparks and smoke out of a tube as it fires, and spray instead from one under water', () => {
    const spawned = { smoke: 0, glow: 0 }
    const world = {
      smoke: { spawn: () => spawned.smoke++, alive: 0 },
      glow: { spawn: () => spawned.glow++, alive: 0 },
      camera: { x: 0, y: 5, z: 30 },
    } as unknown as FxWorld
    missileLaunch(world, { x: 0, y: 1, z: 0 }, { x: 0, y: 1, z: 0 }, null)
    expect(spawned.smoke).toBeGreaterThan(50)
    expect(spawned.glow).toBeGreaterThan(20)
    const dry = { ...spawned }
    missileLaunch(world, { x: 0, y: 1, z: 0 }, { x: 0, y: 1, z: 0 }, 3)
    expect(spawned.smoke - dry.smoke).toBeGreaterThan(50)
    // Too far off to see, nothing is drawn.
    const before = { ...spawned }
    missileLaunch(world, { x: 0, y: 1, z: 2000 }, { x: 0, y: 1, z: 0 }, null)
    expect(spawned).toEqual(before)
    // A missile slow off its tube burns harder than one at its speed.
    const burn = (speed: number) => {
      const was = spawned.smoke + spawned.glow
      for (let i = 0; i < 60; i++) missileExhaust(world, [0, 20, 0], [0, speed, 0], 1 / 60, 0.5)
      return spawned.smoke + spawned.glow - was
    }
    expect(burn(12)).toBeGreaterThan(1.4 * burn(70))
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
