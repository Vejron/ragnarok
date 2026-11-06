import { describe, expect, it, vi } from 'vitest'
import { DIAL_START, DIAL_SWEEP, dialTicks, dialTop, fill, needleAngle } from '@/hud/speedo'
import { hotbarRects, hotbarRectsCss, slotDetail } from '@/hud/gauges'
import { PAPER_MAX, PAPER_METRES, PAPER_MIN, mapWindow, paperView, scaleLength, zoomPaper } from '@/hud/map'
import { TextCache, hexOf, uiScale } from '@/hud/paint'
import { hintAlpha, touchPrompt, wrap } from '@/hud/panels'
import { HOTBAR_SLOTS } from '@/sim/items'

describe('speedometer', () => {
  it('sweeps the needle from the bottom left at rest to the bottom right at the top of the dial', () => {
    expect(needleAngle(0, 160)).toBeCloseTo(DIAL_START)
    expect(needleAngle(160, 160)).toBeCloseTo(DIAL_START + DIAL_SWEEP)
    expect(needleAngle(80, 160)).toBeCloseTo(-Math.PI / 2)
    // Past the top or backwards it stays on the dial.
    expect(needleAngle(500, 160)).toBeCloseTo(DIAL_START + DIAL_SWEEP)
    expect(needleAngle(-20, 160)).toBeCloseTo(DIAL_START)
  })

  it('has a dial to suit the seat, raised in steps when outrun', () => {
    expect(dialTop('driver', 90)).toBe(160)
    expect(dialTop('aviator', 300)).toBe(400)
    expect(dialTop(null, 40)).toBe(80)
    expect(dialTop('driver', 171)).toBe(200)
  })

  it('ticks the dial evenly from nought to the top, numbering every other tick', () => {
    const ticks = dialTicks(160)
    expect(ticks[0]).toEqual({ kmh: 0, major: true })
    expect(ticks[ticks.length - 1]!.kmh).toBe(160)
    expect(ticks.filter((t) => t.major).map((t) => t.kmh)).toEqual([0, 40, 80, 120, 160])
  })

  it('fills a bar by the share left, within 0..1', () => {
    expect(fill(50, 200)).toBe(0.25)
    expect(fill(-5, 100)).toBe(0)
    expect(fill(150, 100)).toBe(1)
    expect(fill(1, 0)).toBe(0)
  })
})

describe('hotbar layout', () => {
  it('centres the slots along the bottom, a little higher off a desktop edge than a phone one', () => {
    const rects = hotbarRects(1000, 600, false)
    expect(rects).toHaveLength(HOTBAR_SLOTS)
    const left = rects[0]!.x
    const right = rects[rects.length - 1]!.x + rects[rects.length - 1]!.w
    expect(left + right).toBeCloseTo(1000)
    expect(rects[0]!.y + rects[0]!.h).toBe(600 - 12)
    expect(hotbarRects(1000, 600, true)[0]!.y + 44).toBe(600 - 10)
  })

  it('shows a gun by its magazine and a stack by its count', () => {
    expect(slotDetail({ item: 2, ammo: 7, count: 1 } as never)).toMatch(/^7\/\d+$/)
    expect(slotDetail({ item: 3, ammo: 0, count: 3 } as never)).toBe('×3')
    expect(slotDetail({ item: 3, ammo: 0, count: 1 } as never)).toBe('')
    expect(slotDetail({ item: 0, ammo: 0, count: 0 } as never)).toBe('')
  })
})

describe('HUD scale', () => {
  it('draws the HUD at its layout size up to 900 pixels high, larger on a big screen, at most 1.4 times', () => {
    expect(uiScale(1280, 720)).toBe(1)
    expect(uiScale(1920, 1080)).toBeCloseTo(1.2)
    expect(uiScale(3840, 2160)).toBe(1.4)
  })

  it('puts the hotbar tap targets where the scaled HUD draws the slots', () => {
    const [layout] = hotbarRects(1920 / 1.2, 1080 / 1.2, true)
    const [css] = hotbarRectsCss(1920, 1080, true)
    expect(css!.x).toBeCloseTo(layout!.x * 1.2)
    expect(css!.w).toBeCloseTo(44 * 1.2)
    expect(css!.y + css!.h).toBeCloseTo(1080 - 10 * 1.2)
  })
})

describe('HUD panels', () => {
  it('wraps text at spaces within a width', () => {
    const measure = (t: string) => t.length * 10
    expect(wrap(measure, 'one two three four', 90)).toEqual(['one two', 'three', 'four'])
    expect(wrap(measure, 'short', 90)).toEqual(['short'])
    // A word longer than the width stands on its own line.
    expect(wrap(measure, 'a extraordinarily b', 50)).toEqual(['a', 'extraordinarily', 'b'])
  })

  it('shows a seat’s keys in full at first, then fades them', () => {
    expect(hintAlpha(0)).toBe(1)
    expect(hintAlpha(5999)).toBe(1)
    expect(hintAlpha(6300)).toBeLessThan(1)
    expect(hintAlpha(60_000)).toBeCloseTo(0.45)
  })

  it('names the touch buttons in prompts that name a key', () => {
    expect(touchPrompt('E: drive the jeep')).toBe('USE: drive the jeep')
    expect(touchPrompt('G: open the parachute')).toBe('CHUTE: open the parachute')
    expect(touchPrompt('Q: something else')).toBe('Q: something else')
  })
})

describe('maps', () => {
  it('takes a window of the image centred on a point, in image pixels', () => {
    // A 1000 m world on a 2048 px image: 2.048 px a metre.
    const w = mapWindow(0, 0, 100, 500, 2048)
    expect(w.side).toBeCloseTo(204.8)
    expect(w.sx).toBeCloseTo(1024 - 102.4)
    expect(w.sy).toBeCloseTo(1024 - 102.4)
  })

  it('keeps the paper map on the world, centred on the reader away from its edges', () => {
    expect(paperView(100, -200, 4000)).toEqual({ cx: 100, cz: -200, metres: PAPER_METRES })
    // Near the east edge the sheet stops at it.
    expect(paperView(3900, 0, 4000, 1200).cx).toBe(4000 - 600)
    // A world smaller than a sheet is all on it.
    expect(paperView(50, 50, 300, 1200)).toEqual({ cx: 0, cz: 0, metres: 600 })
  })

  it('zooms the paper map by the wheel, within its limits', () => {
    expect(zoomPaper(500, 1)).toBeCloseTo(625)
    expect(zoomPaper(500, -1)).toBeCloseTo(400)
    expect(zoomPaper(500, 0)).toBe(500)
    expect(zoomPaper(500, -20)).toBe(PAPER_MIN)
    expect(zoomPaper(500, 20)).toBe(PAPER_MAX)
  })

  it('picks a round scale bar', () => {
    expect(scaleLength(1200)).toBe(250)
    expect(scaleLength(600)).toBe(100)
    expect(scaleLength(100)).toBe(20)
  })
})

describe('HUD text', () => {
  it('keeps the most recently used labels and drops the oldest', () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
    const cache = new TextCache(3)
    const style = { size: 12 }
    const a = cache.get('a', style)
    const b = cache.get('b', style)
    cache.get('c', style)
    cache.get('a', style)
    cache.get('d', style)
    expect(cache.size).toBe(3)
    // 'a' was touched again, so 'b' was the least recently used and went.
    expect(cache.get('a', style)).toBe(a)
    expect(cache.get('b', style)).not.toBe(b)
    vi.restoreAllMocks()
  })

  it('reads CSS hex colours', () => {
    expect(hexOf('#2ec4b6')).toBe(0x2ec4b6)
    expect(hexOf('#fff')).toBe(0xffffff)
    expect(hexOf('red')).toBe(0xffffff)
  })
})
