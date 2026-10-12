import { describe, expect, it } from 'vitest'
import { VehicleSounds } from '@/vehicleaudio'
import type { SoundInput } from '@/sim/engine'
import { KIND_HELI, KIND_JEEP, KIND_TANK } from '@/net/codec'
import { SURFACE_ASPHALT } from '@/sim/surface'

class Param {
  value = 0
  setValueAtTime() {}
  setTargetAtTime() {}
  linearRampToValueAtTime() {}
  exponentialRampToValueAtTime() {}
  cancelScheduledValues() {}
}

/** A node that accepts everything Web Audio code does to one, and counts what starts and stops. */
class Node {
  type = ''
  buffer: unknown = null
  loop = false
  frequency = new Param()
  gain = new Param()
  Q = new Param()
  positionX = new Param()
  positionY = new Param()
  positionZ = new Param()
  panningModel = ''
  distanceModel = ''
  refDistance = 0
  rolloffFactor = 0
  maxDistance = 0
  started = 0
  stopped = 0
  constructor(private readonly log: { sources: Node[] }, readonly kind: string) {}
  connect<T>(to: T): T {
    return to
  }
  disconnect() {}
  start() {
    this.started++
    this.log.sources.push(this)
  }
  stop() {
    this.stopped++
  }
}

class FakeContext {
  readonly log = { sources: [] as Node[] }
  currentTime = 0
  sampleRate = 8000
  destination = new Node(this.log, 'destination')
  listener = {
    positionX: new Param(), positionY: new Param(), positionZ: new Param(),
    forwardX: new Param(), forwardY: new Param(), forwardZ: new Param(),
    upX: new Param(), upY: new Param(), upZ: new Param(),
  }
  createGain() { return new Node(this.log, 'gain') }
  createOscillator() { return new Node(this.log, 'osc') }
  createBiquadFilter() { return new Node(this.log, 'filter') }
  createBufferSource() { return new Node(this.log, 'noise') }
  createPanner() { return new Node(this.log, 'panner') }
  createDynamicsCompressor() { return new Node(this.log, 'compressor') }
  createBuffer(_: number, length: number) {
    return { getChannelData: () => new Float32Array(length) }
  }
}

const sounds = () => {
  const ctx = new FakeContext()
  return { ctx, sounds: new VehicleSounds(ctx as unknown as BaseAudioContext) }
}

const input = (over: Partial<SoundInput> = {}): SoundInput => ({
  kind: KIND_JEEP, speed: 12, accel: 1, climb: 0, powered: true, hp: 100, maxHp: 100, slip: 0, surface: SURFACE_ASPHALT, grounded: true, wheels: null, ...over,
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
    frame(s, [{ id: 1, at: [20, 0, 0], input: { kind: KIND_JEEP } }, { id: 2, at: [30, 0, 0], input: { kind: KIND_TANK } }, { id: 3, at: [40, 0, 0], input: { kind: KIND_HELI } }])
    expect(s.voiceCount).toBe(3)
    const made = ctx.log.sources.length
    expect(made).toBeGreaterThan(9)
    // Re-reporting makes nothing new.
    frame(s, [{ id: 1, at: [21, 0, 0] }, { id: 2, at: [31, 0, 0], input: { kind: KIND_TANK } }, { id: 3, at: [41, 0, 0], input: { kind: KIND_HELI } }])
    expect(ctx.log.sources.length).toBe(made)
    s.drop(2)
    frame(s, [{ id: 1, at: [21, 0, 0] }, { id: 3, at: [41, 0, 0], input: { kind: KIND_HELI } }])
    expect(s.voiceCount).toBe(2)
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
})
