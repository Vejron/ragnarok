import { afterEach, describe, expect, it, vi } from 'vitest'
import { airCut, echoImpulse, echoLevel, VehicleSounds } from '@/vehicleaudio'
import type { SoundInput } from '@/sim/engine'
import { KIND_HELI, KIND_JEEP, KIND_PLANE, KIND_TANK, KIND_WALKER } from '@/net/codec'
import { assumeWorklet } from '@/sound/worklet'
import { FakeContext, Param } from './fakeaudio'

const sounds = () => {
  const ctx = new FakeContext()
  return { ctx, sounds: new VehicleSounds(ctx as unknown as BaseAudioContext) }
}

const input = (over: Partial<SoundInput> = {}): SoundInput => ({
  kind: KIND_JEEP, speed: 12, accel: 1, climb: 0, throttle: 0.5, powered: true, hp: 100, maxHp: 100, slip: 0, ground: 'tarmac', grounded: true, yawRate: 0, wheels: null, ...over,
})

const frame = (s: VehicleSounds, cars: { id: number; at: [number, number, number]; own?: boolean; input?: Partial<SoundInput> }[]) => {
  s.begin()
  for (const car of cars) s.engine(car.id, input(car.input), car.at, [10, 0, 0], car.own ?? false)
  s.end(1 / 60)
}

describe('VehicleSounds', () => {
  it('does nothing without a context', () => {
    const s = new VehicleSounds()
    expect(() => {
      frame(s, [{ id: 1, at: [0, 0, 0] }])
      s.crash([0, 0, 0], 20)
      s.door()
      s.dispose()
    }).not.toThrow()
  })

  it('starts one voice for a running vehicle in earshot, and none for an empty or distant one', () => {
    const { sounds: s } = sounds()
    frame(s, [
      { id: 1, at: [20, 0, 0] },
      { id: 2, at: [30, 0, 0], input: { powered: false } },
      { id: 3, at: [500, 0, 0] },
    ])
    expect(s.voiceCount).toBe(1)
  })

  it('plays only the nearest few engines and the listeners own first', () => {
    const { sounds: s } = sounds()
    const cars = Array.from({ length: 10 }, (_, i) => ({ id: i + 1, at: [10 + i * 10, 0, 0] as [number, number, number] }))
    frame(s, cars)
    expect(s.voiceCount).toBe(6)
    // The one sat in is kept even though it is listed last.
    frame(s, [...cars.slice(0, 9), { id: 10, at: [100, 0, 0], own: true }])
    expect(s.voiceCount).toBe(6)
  })

  it('stops every node of a voice that goes away, and leaks none', () => {
    const { ctx, sounds: s } = sounds()
    frame(s, [{ id: 1, at: [20, 0, 0], input: { kind: KIND_JEEP } }, { id: 2, at: [30, 0, 0], input: { kind: KIND_TANK } }, { id: 3, at: [40, 0, 0], input: { kind: KIND_HELI } }, { id: 4, at: [50, 0, 0], input: { kind: KIND_PLANE, grounded: false, cut: true } }])
    expect(s.voiceCount).toBe(4)
    const made = ctx.log.sources.length
    expect(made).toBeGreaterThan(9)
    // Re-reporting makes nothing new.
    frame(s, [{ id: 1, at: [21, 0, 0] }, { id: 2, at: [31, 0, 0], input: { kind: KIND_TANK } }, { id: 3, at: [41, 0, 0], input: { kind: KIND_HELI } }, { id: 4, at: [51, 0, 0], input: { kind: KIND_PLANE, grounded: false } }])
    expect(ctx.log.sources.length).toBe(made)
    s.drop(2)
    frame(s, [{ id: 1, at: [21, 0, 0] }, { id: 3, at: [41, 0, 0], input: { kind: KIND_HELI } }, { id: 4, at: [51, 0, 0], input: { kind: KIND_PLANE, grounded: false, cut: true } }])
    expect(s.voiceCount).toBe(3)
    s.dispose()
    expect(s.voiceCount).toBe(0)
    expect(ctx.log.sources.every((node) => node.started === 1 && node.stopped === 1)).toBe(true)
  })

  it('lets a wreck burn and a scraping empty car grind, and nothing else', () => {
    const { sounds: s } = sounds()
    frame(s, [{ id: 1, at: [20, 0, 0], input: { hp: 0, powered: false } }])
    expect(s.voiceCount).toBe(1)
    frame(s, [{ id: 2, at: [20, 0, 0], input: { powered: false } }])
    expect(s.voiceCount).toBe(0)
    s.scrape(2)
    frame(s, [{ id: 2, at: [20, 0, 0], input: { powered: false } }])
    expect(s.voiceCount).toBe(1)
  })

  it('makes one-shots, within earshot only and not without limit', () => {
    const { ctx, sounds: s } = sounds()
    s.crash([500, 0, 0], 20)
    expect(ctx.log.sources.length).toBe(0)
    s.crash([10, 0, 0], 20)
    const smash = ctx.log.sources.length
    expect(smash).toBeGreaterThan(4)
    s.crash([10, 0, 0], 4)
    expect(ctx.log.sources.length - smash).toBeLessThan(smash)
    for (let i = 0; i < 40; i++) s.thud([5, 0, 0], 10)
    const before = ctx.log.sources.length
    s.explosion([5, 0, 0])
    s.cannon([5, 0, 0], true)
    s.door()
    expect(ctx.log.sources.length).toBe(before)
    ctx.currentTime = 10
    s.explosion([5, 0, 0])
    expect(ctx.log.sources.length).toBeGreaterThan(before)
  })

  it('rattles a machine gun a round at a time, in one slot', () => {
    const { ctx, sounds: s } = sounds()
    s.machineGun([900, 0, 0], 2)
    expect(ctx.log.sources.length).toBe(0)
    s.machineGun([10, 0, 0], 3)
    expect(ctx.log.sources.length).toBe(6)
    s.machineGun([10, 0, 0], 0)
    expect(ctx.log.sources.length).toBe(6)
    // Twelve rounds a second for a second never crowd out more than a few slots at a time.
    for (let k = 0; k < 12; k++) {
      ctx.currentTime += 1 / 12
      s.machineGun([10, 0, 0], 1, k % 2 === 0)
    }
    const before = ctx.log.sources.length
    s.explosion([5, 0, 0])
    expect(ctx.log.sources.length).toBeGreaterThan(before)
  })

  it('hears a gunshot 500 m away late, at the speed of sound, and dulled', () => {
    const { ctx, sounds: s } = sounds()
    s.gunshot([500, 0, 0], 'rifle')
    expect(ctx.log.sources.length).toBeGreaterThan(0)
    for (const source of ctx.log.sources) expect(source.startedAt).toBeGreaterThanOrEqual(500 / 343)
    expect(airCut(500)).toBeLessThan(2000)
    expect(airCut(5)).toBeGreaterThan(15000)
    // Far off, more of it is echo than shot; close by, well under the shot.
    expect(echoLevel(500)).toBeGreaterThan(2 * (12 / 500))
    expect(echoLevel(10)).toBeLessThan(0.4)
  })

  it('echoes a shot as open ground does: silent at first, then separate slaps, at a known loudness', () => {
    const rate = 48000
    const channels = [0, 1].map(() => new Float32Array(Math.floor(rate * 2.2)))
    const ctx = { sampleRate: rate, createBuffer: () => ({ getChannelData: (c: number) => channels[c]! }) }
    echoImpulse(ctx as unknown as BaseAudioContext)
    for (const data of channels) {
      // Unit energy, so the send's gain is the echo's loudness against the shot.
      let energy = 0
      for (const v of data) energy += v * v
      expect(energy).toBeCloseTo(1, 3)
      // Nothing comes back while the sound is still on its way out: no ring of a room about the gun.
      for (let i = 0; i < rate * 0.09; i++) expect(data[i]).toBe(0)
      // Most of it is in the first slaps, not spread evenly through a dense tail.
      let early = 0
      for (let i = Math.floor(rate * 0.09); i < rate * 0.4; i++) early += data[i]! * data[i]!
      expect(early).toBeGreaterThan(0.5)
    }
  })

  it('carries blasts and guns far but still stops footsteps and crashes close by', () => {
    const { ctx, sounds: s } = sounds()
    s.crash([300, 0, 0], 20)
    s.step([300, 0, 0], 'grass', 1)
    expect(ctx.log.sources.length).toBe(0)
    s.explosion([1200, 0, 0])
    expect(ctx.log.sources.length).toBeGreaterThan(0)
  })

  it('keeps slots free for nearer sounds while far shots are in the air', () => {
    const { ctx, sounds: s } = sounds()
    for (let i = 0; i < 30; i++) s.gunshot([600, 0, 0], 'rifle')
    const far = ctx.log.sources.length
    s.crash([10, 0, 0], 20)
    expect(ctx.log.sources.length).toBeGreaterThan(far)
  })

  describe('with the engine worklet loaded', () => {
    /** The worklet nodes made, and the messages each was sent. */
    const made: { name: string; told: unknown[]; rpm: Param }[] = []
    class FakeWorkletNode {
      readonly parameters = new Map<string, Param>()
      readonly told: unknown[] = []
      readonly port = { postMessage: (message: unknown) => this.told.push(message) }
      constructor(_ctx: unknown, name: string) {
        for (const param of ['rpm', 'load', 'rate', 'level', 'hz', 'ring', 'length']) this.parameters.set(param, new Param())
        made.push({ name, told: this.told, rpm: this.parameters.get('rpm')! })
      }
      connect<T>(to: T): T {
        return to
      }
    }
    afterEach(() => {
      vi.unstubAllGlobals()
      vi.useRealTimers()
      made.length = 0
    })

    it('gives a running engine its own synthesiser and crunch, and tells them to stop when it goes', () => {
      vi.useFakeTimers()
      vi.stubGlobal('AudioWorkletNode', FakeWorkletNode)
      const { ctx, sounds: s } = sounds()
      assumeWorklet(ctx as unknown as BaseAudioContext)
      frame(s, [{ id: 1, at: [20, 0, 0], input: { kind: KIND_TANK } }])
      expect(made.map((node) => node.name).sort()).toEqual(['engine', 'grit'])
      frame(s, [{ id: 1, at: [20, 0, 0], input: { kind: KIND_TANK } }])
      expect(made).toHaveLength(2)
      s.drop(1)
      vi.runAllTimers()
      expect(made.every((node) => node.told.length === 1)).toBe(true)
    })

    it('makes no engine for a kind that has none', () => {
      vi.stubGlobal('AudioWorkletNode', FakeWorkletNode)
      const { ctx, sounds: s } = sounds()
      assumeWorklet(ctx as unknown as BaseAudioContext)
      frame(s, [{ id: 1, at: [20, 0, 0], input: { kind: KIND_WALKER, hp: 0, powered: false } }])
      expect(made.map((node) => node.name)).toEqual(['grit'])
    })
  })

  describe('launches, flight, water and destruction', () => {
    it('hears projectiles in flight, the nearest four at most, and lets each go when it ends', () => {
      const { ctx, sounds: s } = sounds()
      s.beginFlights()
      for (let id = 1; id <= 6; id++) s.flight(id, 2, [id * 10, 0, 0], [-300, 0, 0])
      // A grenade is silent in flight, and one far off is not heard.
      s.flight(7, 1, [5, 0, 0], [0, 0, 0])
      s.flight(8, 2, [900, 0, 0], [0, 0, 0])
      s.endFlights()
      expect(s.flightCount).toBe(4)
      s.dropFlight(1)
      expect(s.flightCount).toBe(3)
      s.beginFlights()
      s.endFlights()
      expect(s.flightCount).toBe(0)
      expect(ctx.log.sources.every((node) => node.started === 1 && node.stopped === 1)).toBe(true)
    })

    it('splashes, cracks, strikes and breaks only within earshot', () => {
      const { ctx, sounds: s } = sounds()
      s.splash([400, 0, 0], 2, 10)
      s.impact([100, 0, 0], 'stone')
      expect(ctx.log.sources.length).toBe(0)
      s.splash([10, 0, 0], 2, 10)
      const splashed = ctx.log.sources.length
      expect(splashed).toBeGreaterThan(0)
      s.impact([10, 0, 0], 'dirt')
      s.glass([10, 0, 0], 3)
      s.crack([2, 0, 0])
      s.launch([10, 0, 0], 'rocket')
      expect(ctx.log.sources.length).toBeGreaterThan(splashed + 4)
    })

    it('gives way with the small sounds, so they never crowd out a blast', () => {
      const { ctx, sounds: s } = sounds()
      for (let i = 0; i < 40; i++) {
        s.impact([5, 0, 0], 'dirt')
        s.crack([1, 0, 0])
        s.splash([5, 0, 0], 0.2, 2)
      }
      const before = ctx.log.sources.length
      s.explosion([5, 0, 0])
      expect(ctx.log.sources.length).toBeGreaterThan(before)
    })

    it('hears a building coming down as a few crashes, not one for every section', () => {
      const { ctx, sounds: s } = sounds()
      s.debris([20, 0, 0], 4, 1000)
      const one = ctx.log.sources.length
      expect(one).toBeGreaterThan(0)
      for (let i = 0; i < 10; i++) s.debris([20 + i * 0.5, 0, 0], 4, 1000 + i * 20)
      expect(ctx.log.sources.length).toBe(one)
      ctx.currentTime = 5
      s.debris([21, 0, 0], 4, 2000)
      expect(ctx.log.sources.length).toBeGreaterThan(one)
    })
  })
})
