import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { type Node, Quaternion, Vector3 } from '@ragnarok/engine'
import { KIND_TECHNICAL, TURRET_JAMMED } from '@/net/codec'
import { seatFrame } from '@/entities/presenters/riders'
import { mountGun } from '@/gauges'
import { kindView } from '@/kinds'
import { parseCarModel } from '@/render/car'
import { TechnicalModel } from '@/render/technical'
import { seatHint } from '@/seathints'
import { TECHNICAL_MG_HEAD, TECHNICAL_MG_PIVOT, TECHNICAL_MG_SEAT } from '@/sim/consts'
import { crewGun, seatHead } from '@/sim/vehicles'

const IDENTITY = [0, 0, 0, 1] as const
const named = (root: Node, name: string) => root.getObjectByName(name)!
const gun = (yaw: number, pitch: number, flags = 0, heat = 0) => ({ yaw, pitch, shots: 0, range: 0, heat, flags })

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/technical.glb'))
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseCarModel('technical', copy.buffer)
})

describe('the technical', () => {
  it('is drawn as a car with its gun on a post', () => {
    const truck = kindView(KIND_TECHNICAL).model(1)
    expect(truck).toBeInstanceOf(TechnicalModel)
    expect(named(truck.root, 'gun').parent).toBe(named(truck.root, 'turret'))
    truck.dispose()
  })

  it('points its gun where the server aims it, from where the server fires it', () => {
    const truck = new TechnicalModel(1)
    truck.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], gun(0, 0))
    const ahead = truck.barrel()
    expect(ahead.origin.toArray().map((v) => Math.round(v * 1000) / 1000)).toEqual([...TECHNICAL_MG_PIVOT])
    expect(ahead.direction.z).toBeCloseTo(-1)
    // Over the tailgate and up: it goes all the way round.
    const [yaw, pitch] = [3.0, 0.4]
    truck.update([0, 0, 0], IDENTITY, 0, [0, 0, 0], gun(yaw, pitch))
    const { direction } = truck.barrel()
    expect(direction.x).toBeCloseTo(-Math.sin(yaw) * Math.cos(pitch))
    expect(direction.y).toBeCloseTo(Math.sin(pitch))
    expect(direction.z).toBeCloseTo(-Math.cos(yaw) * Math.cos(pitch))
    truck.dispose()
  })

  it('stands its gunner behind the gun, turned with it, and leaves their hands on it', () => {
    const truck = new TechnicalModel(1)
    const [head, rotation] = [new Vector3(), new Quaternion()]
    expect(seatFrame(truck, KIND_TECHNICAL, TECHNICAL_MG_SEAT, head, rotation, 0)).toBe(true)
    expect(head.toArray()).toEqual([TECHNICAL_MG_PIVOT[0] + TECHNICAL_MG_HEAD[0], TECHNICAL_MG_PIVOT[1] + TECHNICAL_MG_HEAD[1], TECHNICAL_MG_PIVOT[2] + TECHNICAL_MG_HEAD[2]])
    const behind = seatHead(KIND_TECHNICAL, TECHNICAL_MG_SEAT, Math.PI)!
    expect(behind[2]).toBeCloseTo(TECHNICAL_MG_PIVOT[2] - TECHNICAL_MG_HEAD[2])
    expect(crewGun(KIND_TECHNICAL, 0)).toBe(false)
    expect(crewGun(KIND_TECHNICAL, 1)).toBe(true)
    expect(crewGun(KIND_TECHNICAL, TECHNICAL_MG_SEAT)).toBe(false)
    expect(kindView(KIND_TECHNICAL).ride.seats.map((seat) => seat.stand === true)).toEqual([false, false, true])
    truck.dispose()
  })

  it('tells its gunner how hot the gun is and when it has jammed', () => {
    expect(kindView(KIND_TECHNICAL).ride.seats[TECHNICAL_MG_SEAT]!.gauges).toContain('heat')
    expect(mountGun(gun(0, 0, TURRET_JAMMED, 1)).jammed).toBe(true)
    expect(mountGun(gun(0, 0, 0, 0.3))).toMatchObject({ heat: 0.3, jammed: false })
    expect(mountGun(null).heat).toBeNull()
    const stats = { ammo: null, reload: 1, range: 0, canSwitchSeat: true }
    expect(seatHint('gunner', { ...stats, jammed: true, heat: 1 })).toContain('jammed!')
    expect(seatHint('gunner', { ...stats, jammed: false, heat: 0.7 })).toContain('overheating')
    expect(seatHint('gunner', { ...stats, jammed: false, heat: 0.2 })).toContain('firing')
  })
})
