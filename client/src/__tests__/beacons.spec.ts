import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Group, Mesh, Vec3 } from '@ragnarok/engine'
import { beforeAll, describe, expect, it } from 'vitest'
import { Presenters, type Frame } from '@/entities/presenter'
import { BEACON_SIDE_MS, Beacons } from '@/entities/presenters/beacons'
import { Seating } from '@/entities/presenters/seating'
import { Vehicles, type VehicleWorld } from '@/entities/presenters/vehicles'
import { EntityStore } from '@/entities/store'
import { FLAG_LAMP, FLAG_SIREN, KIND_AMBULANCE, KIND_POLICE_CAR, KIND_VOLVO_240 } from '@/net/codec'
import type { Components } from '@/net/components.generated'
import { CarModel, parseCarModel } from '@/render/car'
import { DAY_SECONDS } from '@/render/daynight'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { AMBULANCE_HALF_EXTENTS, POLICE_CAR_HALF_EXTENTS, TICK_RATE } from '@/sim/consts'

beforeAll(async () => {
  for (const name of ['police_car', 'ambulance', 'volvo_240'] as const) {
    const bytes = readFileSync(resolve(process.cwd(), `public/models/${name}.glb`))
    const copy = new Uint8Array(bytes.length)
    copy.set(bytes)
    await parseCarModel(name, copy.buffer)
  }
})

/** Ticks from the server's start (noon) to midnight and to the next noon. */
const MIDNIGHT = (DAY_SECONDS / 2) * TICK_RATE
const NOON = DAY_SECONDS * TICK_RATE

/** A vehicle of `kind` at `x` metres with `hp` left, sending `flags` if any. */
const vehicle = (kind: number, x: number, flags = 0, hp = 100): Components => ({
  kind: { kind }, place: { x: x * 100, y: 100, z: 0 }, orientation: { qx: 0, qy: 0, qz: 0 }, steer: { steer: 0, throttle: 0, spin: 0 }, health: { hp },
  ...(flags ? { flags: { flags, stance: 0 } } : {}),
})

function harness() {
  const held: { x: number; y: number; color: number; intensity: number; radius: number }[] = []
  /** The vehicles whose sirens were reported in the last frame, and those let go since the start. */
  const wailing: number[] = []
  const dropped: number[] = []
  const fx = { smoke: new Particles(50), glow: new Particles(10, true), skids: new Skidmarks(10), groundAt: () => 0, surfaceAt: () => 0, camera: new Vec3(0, 5, 10) }
  const world: VehicleWorld = {
    root: new Group(),
    sounds: {
      begin() {}, end() {}, engine() {}, drop() {}, thud() {}, footstep() {},
      beginSirens: () => { wailing.length = 0 }, siren: (id: number) => wailing.push(id), endSirens() {}, dropSiren: (id: number) => dropped.push(id),
    } as never,
    effects: {} as never,
    rounds: {} as never,
    debris: {} as never,
    lights: { hold: (x: number, y: number, _z: number, light: { color: number; intensity: number; radius: number }) => held.push({ x, y, ...light }) } as never,
    fx: () => fx as never,
    castRay: () => null,
    driven: () => null,
    seated: () => null,
    listen() {},
    ownShot() {},
  }
  const store = new EntityStore()
  const machines = new Presenters<Frame>()
  const vehicles = machines.add(new Vehicles(world, machines.add(new Seating())))
  machines.add(new Beacons(world, vehicles))
  /** One frame at `tick` with the frame's clock at `now` milliseconds; the lights it holds. */
  const draw = (tick: number, now: number) => {
    held.length = 0
    machines.run(store.sampleAll(tick), { tick, dt: 1 / 60, now, localId: null })
    return held.slice()
  }
  /** Which of vehicle `id`'s lenses are drawn alight, left then right. */
  const lenses = (id: number) => ['beacon_L', 'beacon_R'].map((name) => {
    const lens = vehicles.model(id)!.root.getObjectByName(name)
    return lens instanceof Mesh && !lens.material.lit
  })
  return { store, draw, lenses, wailing, dropped, vehicles }
}

describe('the light bar of a police car or an ambulance', () => {
  it('is two lenses on the roof, one each side, which no other car has', () => {
    for (const [name, half] of [['police_car', POLICE_CAR_HALF_EXTENTS], ['ambulance', AMBULANCE_HALF_EXTENTS]] as const) {
      const car = new CarModel(name, 1)
      car.update([0, 0, 0], [0, 0, 0, 1], 0)
      expect(car.hasBeacons).toBe(true)
      const left = car.flash(0)!.clone()
      const right = car.flash(1)!.clone()
      expect(left.x).toBeLessThan(-0.2)
      expect(right.x).toBeCloseTo(-left.x, 5)
      // Over the roof, which is well above the chassis box, and no wider than the body.
      expect(left.y).toBeGreaterThan(half[1] + 0.5)
      expect(right.x).toBeLessThan(half[0])
      expect(car.flash(null)).toBeNull()
      car.dispose()
    }
    const volvo = new CarModel('volvo_240', 1)
    expect(volvo.hasBeacons).toBe(false)
    expect(volvo.flash(0)).toBeNull()
    volvo.dispose()
  })

  it('flashes and wails only while the siren flag is set, and is put out when it clears', () => {
    const { store, draw, lenses, wailing, dropped } = harness()
    // Vehicle 2's lamps are on, which is another bit; the Volvo sets the siren's bit and has no bar.
    store.push(MIDNIGHT, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_LAMP)], [4, vehicle(KIND_AMBULANCE, 30)], [6, vehicle(KIND_VOLVO_240, 60, FLAG_SIREN)]]), true)
    expect(draw(MIDNIGHT, 0)).toEqual([])
    expect(lenses(2)).toEqual([false, false])
    expect(wailing).toEqual([])

    store.push(MIDNIGHT + 1, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_LAMP | FLAG_SIREN)], [4, vehicle(KIND_AMBULANCE, 30, FLAG_SIREN)], [6, vehicle(KIND_VOLVO_240, 60, FLAG_SIREN)]]), true)
    const lights = draw(MIDNIGHT + 1, 0)
    expect(lights).toHaveLength(2)
    expect(lights[0]).toMatchObject({ color: 0x3f7dff, intensity: 4, radius: 18 })
    expect(Math.abs(lights[0]!.x)).toBeLessThan(1)
    expect(Math.abs(lights[1]!.x - 30)).toBeLessThan(1)
    expect(lenses(2).filter(Boolean)).toHaveLength(1)
    expect(lenses(4).filter(Boolean)).toHaveLength(1)
    expect(wailing).toEqual([2, 4])

    store.push(MIDNIGHT + 2, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_LAMP)], [4, vehicle(KIND_AMBULANCE, 30, FLAG_SIREN)], [6, vehicle(KIND_VOLVO_240, 60)]]), true)
    expect(draw(MIDNIGHT + 2, 0)).toHaveLength(1)
    expect(lenses(2)).toEqual([false, false])
    expect(wailing).toEqual([4])

    // The ambulance goes out of view: its siren is let go with it.
    store.push(MIDNIGHT + 3, new Map([[2, vehicle(KIND_POLICE_CAR, 0)]]), true)
    expect(draw(MIDNIGHT + 3, 0)).toEqual([])
    expect(dropped).toContain(4)
  })

  it('lights its lenses in turn, a quarter second each by the frame\'s clock', () => {
    const { store, draw, lenses } = harness()
    store.push(MIDNIGHT, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_SIREN)]]), true)
    const sides: string[] = []
    for (const now of [0, BEACON_SIDE_MS - 1, BEACON_SIDE_MS, 2 * BEACON_SIDE_MS - 1, 2 * BEACON_SIDE_MS, 3 * BEACON_SIDE_MS]) {
      const [light] = draw(MIDNIGHT, now)
      const [left, right] = lenses(2)
      // The light is held over the lens that is drawn alight, and the other is dark.
      expect(left).not.toBe(right)
      expect(light!.x < 0).toBe(left)
      sides.push(left ? 'left' : 'right')
    }
    expect(sides).toEqual(['left', 'left', 'right', 'right', 'left', 'right'])
  })

  it('starts on the other side for the next vehicle, so two side by side do not flash as one', () => {
    const { store, draw, lenses } = harness()
    store.push(MIDNIGHT, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_SIREN)], [3, vehicle(KIND_POLICE_CAR, 30, FLAG_SIREN)]]), true)
    draw(MIDNIGHT, 0)
    expect(lenses(2)).toEqual([true, false])
    expect(lenses(3)).toEqual([false, true])
  })

  it('is dimmer by day, and dark and silent on a wreck', () => {
    const { store, draw, lenses, wailing } = harness()
    store.push(NOON, new Map([[2, vehicle(KIND_POLICE_CAR, 0, FLAG_SIREN)], [4, vehicle(KIND_AMBULANCE, 30, FLAG_SIREN, 0)]]), true)
    const lights = draw(NOON, 0)
    expect(lights).toHaveLength(1)
    expect(lights[0]!.intensity).toBeCloseTo(4 * 0.3)
    expect(lenses(4)).toEqual([false, false])
    expect(wailing).toEqual([2])
  })
})
