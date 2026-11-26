import { describe, expect, it } from 'vitest'
import { Group, Vector3 } from '@ragnarok/engine'
import { Cameras, type Seated } from '@/cameras'
import { KIND_DRONE, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PBV, KIND_STRF, KIND_TANK } from '@/net/codec'
import type { Placed } from '@/entities/presenters/vehicles'
import { MortarModel } from '@/render/mortar'
import { TankModel } from '@/render/tank'
import { crewedSeatView, seatView } from '@/seats'
import { MORTAR_RANGE_MAX, TANK_MG_PITCH_MAX, TANK_MG_SEAT, TANK_MG_YAW_LIMIT } from '@/sim/consts'

describe('the seats table', () => {
  it('gives a seat past the last its last row', () => {
    expect(seatView(KIND_TANK, 5)).toBe(seatView(KIND_TANK, TANK_MG_SEAT))
  })

  it("gives a driver alone in a turreted hull the gunner's view, and gives it back when a gunner climbs in", () => {
    for (const kind of [KIND_TANK, KIND_STRF, KIND_PBV]) {
      const alone = crewedSeatView(kind, 0, [7, 0, 0])
      expect(alone.role).toBe('tankDriver')
      expect(alone.rig).toBe(seatView(kind, 1).rig)
      expect(alone.sight).toBe(true)
      expect(alone.gauges).toContain('boost')
      expect(crewedSeatView(kind, 0, [7, 8, 0])).toBe(seatView(kind, 0))
    }
    // The gunner's own seat, and a car with no gun, are as they always are.
    expect(crewedSeatView(KIND_TANK, 1, [0, 7, 0])).toBe(seatView(KIND_TANK, 1))
    expect(crewedSeatView(KIND_JEEP, 0, [7, 0])).toBe(seatView(KIND_JEEP, 0))
  })
})

/** A camera rig's world: the look it moves and the views it asks for. */
function rigged() {
  const views: string[] = []
  const input = {
    yaw: 0,
    pitch: 0,
    layPitch: null as number | null,
    clampPitch(min: number, max: number) {
      this.pitch = Math.max(min, Math.min(max, this.pitch))
    },
    followYaw() {},
  }
  const scene = {
    gunsight: { hide() {}, show: () => views.push('sight') },
    follow: () => views.push('follow'),
    aimFrom: () => views.push('aim'),
    fpvFrom: () => views.push('fpv'),
    downFrom: () => views.push('down'),
    scopeFrom: () => views.push('scope'),
    camera: { fov: 60, rotateZ() {}, project: () => false },
    groundAt: () => 0,
  }
  const cameras = new Cameras(scene as never, input as never, () => null)
  return { views, input, cameras }
}

/** The nodes of a tank a roof gun needs. */
function roofGunModel() {
  const root = new Group()
  for (const [name, parent] of [['turret', root], ['gun', root], ['mg', root], ['mg_gun', root]] as const) {
    const node = new Group()
    node.name = name
    parent.add(node)
  }
  return root
}

const placed = (kind: number, turretYaw = 0): Placed => ({
  kind, x: 0, y: 1, z: 0, yaw: 0.5, rotation: [0, 0, 0, 1], steer: 0, seats: [1, 2], turret: { yaw: turretYaw, pitch: 0.1, shots: 0, range: 0, heat: 0, flags: 0 },
  pintle: null, arms: null, sub: null, hp: 100, damage: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], doors: { left: 0, right: 0, gone: 0 }, jets: 0, air: null, fuel: null, throttle: 0,
})

function seated(kind: number, index: number, extra: Partial<Seated> = {}): Seated {
  return {
    now: 0, dt: 1 / 60, vehicle: 9, seat: seatView(kind, index), ride: placed(kind), model: undefined, motion: undefined, distance: 10,
    sighted: false, scoped: false, aimBlend: 0, chase: null, aimHeld: false, head: null, ...extra,
  }
}

describe('camera rigs', () => {
  it('keep the look on a roof gun within its travel of the turret, however that turns', () => {
    const { input, cameras } = rigged()
    const tank = new TankModel(1, roofGunModel())
    const ride = (turret: number): Placed => ({ ...placed(KIND_TANK, turret), pintle: { yaw: 0.2, pitch: 0.1, shots: 0, heat: 0, flags: 0 } })
    const seat = TANK_MG_SEAT
    cameras.seated(seated(KIND_TANK, seat, { ride: ride(0.25), model: tank }), seat)
    expect(input.yaw).toBeCloseTo(0.5 + 0.25 + 0.2)
    expect(input.pitch).toBeCloseTo(0.1)
    input.yaw = 0.5 + 0.25 + 3
    input.pitch = 1.5
    cameras.seated(seated(KIND_TANK, seat, { ride: ride(0.25), model: tank }), seat)
    expect(input.yaw).toBeCloseTo(0.5 + 0.25 + TANK_MG_YAW_LIMIT)
    expect(input.pitch).toBeCloseTo(TANK_MG_PITCH_MAX)
    // The turret swinging away pushes the look along.
    cameras.seated(seated(KIND_TANK, seat, { ride: ride(0.1), model: tank }), seat)
    expect(input.yaw).toBeCloseTo(0.5 + 0.1 + TANK_MG_YAW_LIMIT)
  })

  it('lay a mortar from above while aim is held: the mouse moves a mark, the tube is laid onto it, and letting go gives the look back', () => {
    const { input, cameras } = rigged()
    const mortar = new MortarModel(9)
    const ride: Placed = { ...placed(KIND_MORTAR, 0.25), y: 0.5, turret: { yaw: 0.25, pitch: 1, shots: 0, range: 100, heat: 0, flags: 0 } }
    mortar.update([ride.x, ride.y, ride.z], [0, 0, 0, 1], 0, [1], ride.turret)
    const sit = (aimHeld: boolean) => cameras.seated(seated(KIND_MORTAR, 0, { ride, model: mortar, aimHeld }), 0)
    sit(false)
    input.pitch = -0.3
    sit(true)
    const first = cameras.layMark!
    expect(Math.hypot(first.x, first.z)).toBeCloseTo(100, 0)
    // Laid along the gun's bearing when aim was taken, to an elevation within the dial.
    expect(input.yaw).toBeCloseTo(0.75, 1)
    expect(input.layPitch).toBeGreaterThan(0.78)
    expect(input.layPitch).toBeLessThan(1.48)
    const steep = input.layPitch!
    // The mouse pushed up: the mark goes farther, and the tube lowers to reach it.
    input.pitch += 0.1
    sit(true)
    expect(Math.hypot(cameras.layMark!.x, cameras.layMark!.z)).toBeGreaterThan(105)
    expect(input.layPitch).toBeLessThan(steep)
    // Never past the end of the dial.
    input.pitch += 10
    sit(true)
    expect(Math.hypot(cameras.layMark!.x, cameras.layMark!.z)).toBeLessThan(MORTAR_RANGE_MAX + 1)
    sit(false)
    expect(cameras.layMark).toBeNull()
    expect(input.layPitch).toBeNull()
    expect(input.pitch).toBeCloseTo(-0.3)
    mortar.dispose()
  })

  it('turn the look where a seat points when it is taken, and only then', () => {
    const { input, cameras } = rigged()
    cameras.seated(seated(KIND_MORTAR, 0, { ride: placed(KIND_MORTAR, 0.25) }), 0)
    expect(input.yaw).toBeCloseTo(0.75)
    input.yaw = 2
    cameras.seated(seated(KIND_MORTAR, 0, { ride: placed(KIND_MORTAR, 0.25) }), 0)
    expect(input.yaw).toBe(2)
    expect(cameras.rig).toBe('mortar')
  })

  it('fly a drone through its camera, hiding it from itself, and cut the feed when the link goes', () => {
    const { views, cameras } = rigged()
    const drone = { root: new Group() } as never
    cameras.seated(seated(KIND_DRONE, 0, { model: drone }), 0)
    expect(views).toEqual(['fpv'])
    expect((drone as { root: Group }).root.visible).toBe(false)
    cameras.leave(1234)
    expect(cameras.feedLostAt).toBe(1234)
    expect((drone as { root: Group }).root.visible).toBe(true)
    expect(cameras.rig).toBeNull()
  })

  it('swap a loitering drone between its nose and belly cameras on a fresh aim press', () => {
    const { cameras } = rigged()
    const loiter = seatView(KIND_LOITER, 0)
    expect(cameras.zoom(loiter, false, false, false, false)).toBe('fpv')
    expect(cameras.zoom(loiter, true, false, false, false)).toBe('down')
    expect(cameras.zoom(loiter, true, false, false, false)).toBe('down')
    expect(cameras.zoom(loiter, false, false, false, false)).toBe('down')
    expect(cameras.zoom(loiter, true, false, false, false)).toBe('fpv')
  })

  it('sit behind a gun whose model has none yet as behind the vehicle, and aim once it has', () => {
    const { views, input, cameras } = rigged()
    cameras.seated(seated(KIND_TANK, 1, { ride: placed(KIND_TANK, 0.3) }), 1)
    expect(views).toEqual(['follow'])
    // The model's root carries the hull's heading.
    const root = new Group()
    root.quaternion.setFromAxisAngle(new Vector3(0, 1, 0), 0.5)
    const barrel = { root, barrel: () => ({ origin: new Vector3(), direction: new Vector3(0, 0, -1) }) } as never
    cameras.seated(seated(KIND_TANK, 1, { ride: placed(KIND_TANK, 0.3), model: barrel }), 1)
    expect(views).toEqual(['follow', 'aim'])
    // Hull yaw 0.5 and turret 0.3: the gunner looks along the barrel.
    expect(input.yaw).toBeCloseTo(0.8)
  })
})
