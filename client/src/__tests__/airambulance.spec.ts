import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import { Mesh } from '@ragnarok/engine'
import { boxOf, triangleCount } from './helpers'
import { dialMax } from '@/hud/speedo'
import { kindView } from '@/kinds'
import { KIND_AIR_AMBULANCE, KIND_HELI } from '@/net/codec'
import { AirAmbulanceModel, parseAirAmbulanceModel } from '@/render/airambulance'
import { RotorModel } from '@/render/rotor'
import { aimsReticle, seatHint } from '@/seathints'
import {
  AIR_AMBULANCE_FAN_HUB, AIR_AMBULANCE_FIN_CENTRE, AIR_AMBULANCE_FIN_HALF, AIR_AMBULANCE_HALF_EXTENTS, AIR_AMBULANCE_LAMP, AIR_AMBULANCE_ROTOR_HUB,
  AIR_AMBULANCE_ROTOR_RADIUS, AIR_AMBULANCE_SEAT_HEADS, BUTTON_FIRE, BUTTON_JUMP, BUTTON_RUN,
} from '@/sim/consts'
import { crewGun, isRotorcraft, seatHead, vehicleDef } from '@/sim/vehicles'
import { touchButtons } from '@/touchbuttons'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const SEATS = [0, 1, 2, 3]

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/air_ambulance.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseAirAmbulanceModel(copy.buffer)
})

describe('AirAmbulanceModel', () => {
  it('has its rotor and its fan on the hubs the game has them on', () => {
    const model = new AirAmbulanceModel()
    for (const [name, at] of [['rotor', AIR_AMBULANCE_ROTOR_HUB], ['tail_rotor', AIR_AMBULANCE_FAN_HUB]] as const) {
      const node = model.root.getObjectByName(name)!
      at.forEach((v, i) => expect(node.position.toArray()[i], name).toBeCloseTo(v, 3))
    }
    const rotor = boxOf(model.root.getObjectByName('rotor')!)
    // Four blades on the diagonals: the box round them is the disc's radius over root two each way.
    expect(rotor.max.x).toBeCloseTo(AIR_AMBULANCE_ROTOR_RADIUS * Math.SQRT1_2, 0)
    model.dispose()
  })

  it('marks each seat where the server puts its occupant\'s head', () => {
    const model = new AirAmbulanceModel()
    for (const seat of SEATS) {
      const marker = model.root.getObjectByName(`seat_${seat}`)!.position.toArray()
      AIR_AMBULANCE_SEAT_HEADS[seat]!.forEach((v, i) => expect(marker[i], `seat ${seat}`).toBeCloseTo(v, 3))
    }
    model.dispose()
  })

  it('stands on the bottom of its hull box and ends where its fin\'s solid does', () => {
    const model = new AirAmbulanceModel()
    const cabin = boxOf(model.root.getObjectByName('hull')!)
    const [, halfY, halfZ] = AIR_AMBULANCE_HALF_EXTENTS
    expect(cabin.min.y).toBeCloseTo(-halfY, 1)
    expect(Math.abs(cabin.min.z + halfZ)).toBeLessThan(0.15)
    expect(Math.abs(cabin.max.z - (AIR_AMBULANCE_FIN_CENTRE[2] + AIR_AMBULANCE_FIN_HALF[2]))).toBeLessThan(0.15)
    model.dispose()
  })

  it('stays low-poly and in a few draws, with glass to see its crew through', () => {
    const model = new AirAmbulanceModel()
    expect(triangleCount(model.root)).toBeLessThan(3500)
    const meshes: Mesh[] = []
    model.root.traverse((node) => {
      if (node instanceof Mesh) meshes.push(node)
    })
    expect(meshes.length).toBeLessThanOrEqual(8)
    expect(meshes.some((mesh) => mesh.material.opacity < 0.5)).toBe(true)
    model.dispose()
  })

  it('turns its rotor and its fan as fast as the physics says, whoever sits in it', () => {
    let now = 1000
    const clock = vi.spyOn(performance, 'now').mockImplementation(() => now)
    const model = new AirAmbulanceModel()
    expect(model).toBeInstanceOf(RotorModel)
    const [rotor, fan] = [model.root.getObjectByName('rotor')!, model.root.getObjectByName('tail_rotor')!]
    model.setRotor(0)
    for (let k = 0; k < 30; k++, now += 16) model.update([0, 0, 0], IDENTITY, 0, [3, 0, 0, 0])
    expect(rotor.rotation.y).toBe(0)
    model.setRotor(0.8)
    for (let k = 0; k < 30; k++, now += 16) model.update([0, 0, 0], IDENTITY, 0, [0, 0, 0, 0])
    expect(Math.abs(rotor.rotation.y)).toBeGreaterThan(0)
    expect(Math.abs(fan.rotation.x)).toBeGreaterThan(0)
    model.dispose()
    clock.mockRestore()
  })

  it('shines its searchlight from under the nose, ahead and down, turning with the hull', () => {
    const model = new AirAmbulanceModel()
    // A quarter turn to the left: the nose points along -x.
    model.update([10, 20, 0], [0, Math.SQRT1_2, 0, Math.SQRT1_2], 0, [1, 0, 0, 0])
    const { at, direction } = model.searchlight()
    expect(at.x).toBeLessThan(10 + AIR_AMBULANCE_LAMP[2])
    expect(at.y).toBeCloseTo(20 + AIR_AMBULANCE_LAMP[1], 3)
    expect(direction.x).toBeLessThan(-0.9)
    expect(direction.y).toBeLessThan(-0.1)
    model.dispose()
  })
})

describe('the air ambulance', () => {
  const { ride } = kindView(KIND_AIR_AMBULANCE)
  const stats = { ammo: null, reload: 1, range: 0, canSwitchSeat: true, jammed: false, heat: null }

  it('seats a pilot and three passengers, all seen through the glass, and none of them armed', () => {
    const def = vehicleDef(KIND_AIR_AMBULANCE)
    expect(def).toMatchObject({ seats: 4, crew: 'glazed', gun: false, arms: false, gunners: [] })
    expect(ride.seats.map((seat) => seat.role)).toEqual(['airPilot', 'rider', 'rider', 'rider'])
    expect(SEATS.map((seat) => crewGun(KIND_AIR_AMBULANCE, seat))).toEqual([false, false, false, false])
    for (const seat of SEATS) expect(seatHead(KIND_AIR_AMBULANCE, seat)).not.toBeNull()
  })

  it('is flown as the gunship is, hovering on a lever of its own', () => {
    expect(ride.seats[0]).toMatchObject({ rig: { name: 'pilot' }, lever: true, gauges: ['rotor'] })
    expect(isRotorcraft(KIND_AIR_AMBULANCE)).toBe(true)
    expect(vehicleDef(KIND_AIR_AMBULANCE).rotor!.hoverLever).not.toBe(vehicleDef(KIND_HELI).rotor!.hoverLever)
    expect(dialMax('airPilot')).toBeGreaterThan(dialMax('pilot'))
  })

  it('never tells anyone aboard to fire', () => {
    for (const { role } of ride.seats) {
      expect(seatHint(role, stats)).not.toMatch(/fire|rocket|reload|click/i)
      expect(seatHint(role, stats)).toContain('R switch seat')
      expect(aimsReticle(role)).toBe(false)
    }
    expect(seatHint('pilot', stats)).toContain('rockets')
  })

  it('gives a touch player the lever and a seat switch, and no fire button', () => {
    const context = { canSwitch: true, winter: false, ammo: false }
    const [pilot, rider] = [touchButtons('airPilot', context), touchButtons('rider', context)]
    for (const buttons of [pilot, rider]) {
      expect(buttons.some((button) => button.mask === BUTTON_FIRE)).toBe(false)
      expect(buttons.some((button) => button.id === 'seat')).toBe(true)
    }
    expect(pilot.find((button) => button.mask === BUTTON_JUMP)!.label).toBe('UP')
    expect(pilot.find((button) => button.mask === BUTTON_RUN)!.label).toBe('DOWN')
  })
})
