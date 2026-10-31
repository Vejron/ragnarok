import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { Group, Quaternion, Vector3 } from '@ragnarok/engine'
import { KIND_TANK } from '@/net/codec'
import { seatFrame } from '@/entities/presenters/riders'
import { roofGun } from '@/gauges'
import { parseTankModel, TankModel } from '@/render/tank'
import { aimsReticle, seatHint } from '@/seathints'
import { TANK_MG_AT, TANK_MG_HEAD, TANK_MG_SEAT, TANK_TURRET_PIVOT } from '@/sim/consts'
import { seatHead } from '@/sim/vehicles'

const IDENTITY = [0, 0, 0, 1] as const
const turret = (yaw: number) => ({ yaw, pitch: 0, shots: 0, range: 0, flags: 0 })
const gun = (yaw: number, pitch: number, flags = 0, heat = 0) => ({ yaw, pitch, shots: 0, heat, flags })

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/tank.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseTankModel(copy.buffer)
})

describe('the T-55 roof gun', () => {
  it('stands where the server fires it from, and goes round with the turret', () => {
    const tank = new TankModel(1)
    tank.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], turret(0))
    tank.setPintle(gun(0, 0))
    const ahead = tank.pintleBarrel()!
    expect(ahead.origin.x).toBeCloseTo(TANK_TURRET_PIVOT[0] + TANK_MG_AT[0])
    expect(ahead.origin.y).toBeCloseTo(TANK_TURRET_PIVOT[1] + TANK_MG_AT[1])
    expect(ahead.origin.z).toBeCloseTo(TANK_TURRET_PIVOT[2] + TANK_MG_AT[2])
    expect(ahead.direction.z).toBeCloseTo(-1)

    // The turret a quarter left: its right side comes round to the front, as `on_turret` has it.
    tank.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], turret(Math.PI / 2))
    const round = tank.pintleBarrel()!
    expect(round.origin.x).toBeCloseTo(TANK_TURRET_PIVOT[0] + TANK_MG_AT[2])
    expect(round.origin.z).toBeCloseTo(TANK_TURRET_PIVOT[2] - TANK_MG_AT[0])
    expect(round.direction.x).toBeCloseTo(-1)
  })

  it('points as the server aims it: yaw from the turret, then elevation', () => {
    const tank = new TankModel(1)
    const [turretYaw, yaw, pitch] = [0.4, 1.1, 0.5]
    tank.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], turret(turretYaw))
    tank.setPintle(gun(yaw, pitch))
    const { direction } = tank.pintleBarrel()!
    const total = turretYaw + yaw
    expect(direction.x).toBeCloseTo(-Math.sin(total) * Math.cos(pitch))
    expect(direction.y).toBeCloseTo(Math.sin(pitch))
    expect(direction.z).toBeCloseTo(-Math.cos(total) * Math.cos(pitch))
  })

  it('leaves a turretless tank nothing to point', () => {
    const source = new Group()
    const barrel = new Group()
    barrel.name = 'gun'
    source.add(barrel)
    const bare = new TankModel(1, source)
    bare.setPintle(gun(1, 1))
    expect(bare.pintleBarrel()).toBeNull()
  })

  it('stands its gunner in the hatch, turned with the turret, and only them', () => {
    const tank = new TankModel(1)
    const [head, rotation] = [new Vector3(), new Quaternion()]
    expect(seatHead(KIND_TANK, 0)).toBeNull()
    expect(seatHead(KIND_TANK, 1)).toBeNull()
    expect(seatFrame(tank, KIND_TANK, 1, head, rotation)).toBe(false)
    expect(seatFrame(tank, KIND_TANK, TANK_MG_SEAT, head, rotation, 0)).toBe(true)
    expect(head.toArray()).toEqual([TANK_TURRET_PIVOT[0] + TANK_MG_HEAD[0], TANK_TURRET_PIVOT[1] + TANK_MG_HEAD[1], TANK_TURRET_PIVOT[2] + TANK_MG_HEAD[2]])
    seatFrame(tank, KIND_TANK, TANK_MG_SEAT, head, rotation, Math.PI)
    expect(head.x).toBeCloseTo(TANK_TURRET_PIVOT[0] - TANK_MG_HEAD[0])
    expect(head.z).toBeCloseTo(TANK_TURRET_PIVOT[2] - TANK_MG_HEAD[2])
  })

  it('tells its gunner how hot it is and when it has jammed', () => {
    expect(roofGun(gun(0, 0, 1, 1)).jammed).toBe(true)
    expect(roofGun(gun(0, 0, 0, 0.3))).toMatchObject({ heat: 0.3, jammed: false })
    expect(roofGun(null).heat).toBeNull()
    const stats = { ammo: null, reload: 1, range: 0, canSwitchSeat: true }
    expect(seatHint('roofgunner', { ...stats, jammed: true, heat: 1 })).toContain('jammed!')
    expect(seatHint('roofgunner', { ...stats, jammed: false, heat: 0.7 })).toContain('overheating')
    expect(aimsReticle('roofgunner')).toBe(true)
  })
})
