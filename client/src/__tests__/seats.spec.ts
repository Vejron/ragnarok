import { describe, expect, it } from 'vitest'
import { Group, Vector3 } from '@ragnarok/engine'
import { Cameras, type Seated } from '@/cameras'
import { isDrone, KIND_DRONE, KIND_HELI, KIND_HEN, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import type { Placed } from '@/entities/presenters/vehicles'
import { rideView, seatView } from '@/seats'
import { HELI_GUN_COOLDOWN_TICKS, MORTAR_COOLDOWN_TICKS, SHELL_COOLDOWN_TICKS, WALKER_GUN_COOLDOWN_TICKS } from '@/sim/consts'
import { vehicleDef } from '@/sim/vehicles'
import { BEFORE } from './kinds.before'

// What the if-chains in `game.ts` said before the seats became a table, kept here to hold the table to them.
const before = {
  prompt(kind: number, driverSeatFree: boolean) {
    if (kind === KIND_MORTAR) return 'E: man the mortar'
    if (kind === KIND_TANK) return driverSeatFree ? 'E: drive the tank' : 'E: man the gun'
    if (kind === KIND_STRV) return 'E: drive the Strv S'
    if (kind === KIND_HELI) return driverSeatFree ? 'E: fly' : 'E: man the nose gun'
    if (kind === KIND_PLANE) return driverSeatFree ? 'E: fly the triplane' : 'E: man the rear gun'
    if (kind === KIND_SPORTS) return driverSeatFree ? 'E: drive the sports car' : 'E: ride along'
    if (kind === KIND_ROADSTER) return driverSeatFree ? 'E: drive the Roadster' : 'E: ride along'
    if (kind === KIND_WALKER) return 'E: drive the walker'
    if (kind === KIND_HEN) return 'E: drive the hen'
    return driverSeatFree ? 'E: drive the jeep' : 'E: ride along'
  },
  role(kind: number, index: number) {
    if (index > 0) return vehicleDef(kind).gun ? 'gunner' : 'passenger'
    return kind === KIND_DRONE ? 'drone' : kind === KIND_LOITER ? 'loiter' : kind === KIND_HELI ? 'pilot' : kind === KIND_PLANE ? 'aviator' : kind === KIND_WALKER ? 'walker' : kind === KIND_HEN ? 'hen' : kind === KIND_STRV ? 'strv' : kind === KIND_MORTAR ? 'mortar' : kind === KIND_ROADSTER ? 'roadster' : 'driver'
  },
  reload: (kind: number) => (kind === KIND_PLANE ? null : kind === KIND_HELI ? HELI_GUN_COOLDOWN_TICKS : kind === KIND_WALKER ? WALKER_GUN_COOLDOWN_TICKS : kind === KIND_MORTAR ? MORTAR_COOLDOWN_TICKS : SHELL_COOLDOWN_TICKS),
  sight: (kind: number, index: number) => (kind === KIND_TANK ? index === 1 : kind === KIND_STRV && index === 0),
  chase: (kind: number, index: number) => index === 0 && kind !== KIND_HELI && kind !== KIND_PLANE && kind !== KIND_MORTAR && !isDrone(kind),
}

describe('the seats table', () => {
  for (const def of BEFORE.map(vehicleDef)) {
    it(`says what the camera, the HUD and the prompt did for the ${def.name}`, () => {
      const ride = rideView(def.kind)
      expect(ride.seats).toHaveLength(def.seats)
      expect(ride.reload).toBe(before.reload(def.kind))
      ride.seats.forEach((seat, index) => {
        expect(seat.role, `seat ${index}`).toBe(before.role(def.kind, index))
        expect(seat.sight === true, `seat ${index}`).toBe(before.sight(def.kind, index))
        expect(seat.chase === true, `seat ${index}`).toBe(before.chase(def.kind, index))
        // Boarding takes the lowest free seat, so seat `index` is boarded when every seat before it is taken.
        expect(seat.board, `seat ${index}`).toBe(isDrone(def.kind) ? undefined : before.prompt(def.kind, index === 0))
        // A seat whose occupant fires their own guns is shot from over their shoulder.
        expect(seat.rig.name === 'passenger', `seat ${index}`).toBe(def.crewGuns && index > 0)
      })
    })
  }

  it('gives a seat past the last its last row', () => {
    expect(seatView(KIND_TANK, 5)).toBe(seatView(KIND_TANK, 1))
  })
})

/** A camera rig's world: the look it moves and the views it asks for. */
function rigged() {
  const views: string[] = []
  const input = {
    yaw: 0,
    pitch: 0,
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
    camera: { fov: 60, rotateZ() {} },
  }
  const cameras = new Cameras(scene as never, input as never, () => null)
  return { views, input, cameras }
}

const placed = (kind: number, turretYaw = 0): Placed => ({
  kind, x: 0, y: 1, z: 0, yaw: 0.5, rotation: [0, 0, 0, 1], steer: 0, seats: [1, 2], turret: { yaw: turretYaw, pitch: 0.1, shots: 0, range: 0, flags: 0 },
  arms: null, hp: 100, damage: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], jets: 0, air: null,
})

function seated(kind: number, index: number, extra: Partial<Seated> = {}): Seated {
  return {
    now: 0, dt: 1 / 60, vehicle: 9, seat: seatView(kind, index), ride: placed(kind), model: undefined, motion: undefined, distance: 10,
    sighted: false, scoped: false, aimBlend: 0, chase: null, head: null, ...extra,
  }
}

describe('camera rigs', () => {
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
    const barrel = { root: new Group(), barrel: () => ({ origin: new Vector3(), direction: new Vector3(0, 0, -1) }) } as never
    cameras.seated(seated(KIND_TANK, 1, { ride: placed(KIND_TANK, 0.3), model: barrel }), 1)
    expect(views).toEqual(['follow', 'aim'])
    // Hull yaw 0.5 and turret 0.3: the gunner looks along the barrel.
    expect(input.yaw).toBeCloseTo(0.8)
  })
})
