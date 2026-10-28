import { afterEach, describe, expect, it, vi } from 'vitest'
import { TankMusic } from '@/music'

function rig() {
  const param = { cancelScheduledValues: vi.fn(), setTargetAtTime: vi.fn(), value: 1 }
  const gain = { gain: param, connect: vi.fn((n) => n), disconnect: vi.fn() }
  const source = { connect: vi.fn(() => gain) }
  const ctx = { currentTime: 0, destination: {}, createGain: () => gain, createMediaElementSource: vi.fn(() => source) } as unknown as BaseAudioContext
  const audio = { play: vi.fn(async () => {}), pause: vi.fn(), src: '', loop: false, preload: '' } as unknown as HTMLAudioElement
  const make = vi.fn(() => audio)
  vi.stubGlobal('Audio', function () {})
  return { ctx, audio, make, param }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('TankMusic', () => {
  it('asks for nothing until it is wanted, then fades in', () => {
    const { ctx, audio, make, param } = rig()
    const music = new TankMusic(ctx, make)
    music.want(false)
    expect(make).not.toHaveBeenCalled()
    music.want(true)
    expect(make).toHaveBeenCalledTimes(1)
    expect(audio.src).toContain('music/')
    expect(audio.play).toHaveBeenCalled()
    expect(param.setTargetAtTime.mock.calls.at(-1)![0]).toBeGreaterThan(0)
  })

  it('fades out, pauses later, and cancels the pause on a quick return', () => {
    vi.useFakeTimers()
    const { ctx, audio, make, param } = rig()
    const music = new TankMusic(ctx, make)
    music.want(true)
    music.want(false)
    expect(param.setTargetAtTime.mock.calls.at(-1)![0]).toBe(0)
    expect(audio.pause).not.toHaveBeenCalled()
    vi.advanceTimersByTime(3000)
    expect(audio.pause).toHaveBeenCalledTimes(1)
    music.want(true)
    music.want(false)
    music.want(true)
    vi.advanceTimersByTime(3000)
    expect(audio.pause).toHaveBeenCalledTimes(1)
    expect(make).toHaveBeenCalledTimes(1)
  })

  it('stays silent and can retry when playback is refused', async () => {
    const { ctx, audio, make } = rig()
    ;(audio.play as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error('no'))
    const music = new TankMusic(ctx, make)
    music.want(true)
    await Promise.resolve()
    await Promise.resolve()
    music.want(true)
    expect(audio.play).toHaveBeenCalledTimes(2)
  })
})
