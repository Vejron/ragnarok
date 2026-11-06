import { beforeEach, describe, expect, it } from 'vitest'
import { loadQuality, PRESETS, resolveQuality, saveQuality } from '@/render/quality'

describe('quality', () => {
  beforeEach(() => localStorage.clear())

  it('is auto until chosen, and remembers the choice', () => {
    expect(loadQuality()).toBe('auto')
    saveQuality('low')
    expect(loadQuality()).toBe('low')
  })

  it('falls back to auto on junk', () => {
    localStorage.setItem('ragnarok.quality', 'ultra')
    expect(loadQuality()).toBe('auto')
  })

  it('picks medium for touch screens and high otherwise', () => {
    expect(resolveQuality('auto', true)).toBe('medium')
    expect(resolveQuality('auto', false)).toBe('high')
    expect(resolveQuality('low', false)).toBe('low')
  })

  it('costs less at each level down', () => {
    const [high, medium, low] = [PRESETS.high, PRESETS.medium, PRESETS.low]
    for (const [more, less] of [[high, medium], [medium, low]] as const) {
      expect(less.maxPixelRatio).toBeLessThanOrEqual(more.maxPixelRatio)
      expect(less.msaa).toBeLessThanOrEqual(more.msaa)
      expect(less.loadRadius).toBeLessThanOrEqual(more.loadRadius)
      expect(less.fogFar).toBeLessThanOrEqual(more.fogFar)
    }
  })

  it('keeps the edge of the loaded world inside the fog', () => {
    for (const preset of Object.values(PRESETS)) {
      expect(preset.unloadRadius).toBeGreaterThan(preset.loadRadius)
      // At the load radius the fog is most of the way in, as at high.
      expect((preset.loadRadius - preset.fogNear) / (preset.fogFar - preset.fogNear)).toBeGreaterThan(0.6)
    }
  })
})
