import { describe, expect, it } from 'vitest'
import { Ribbons } from '../src/fx/ribbons'

const style = { color: 0xffffff, life: 1, width: 1, step: 1 }
const trails = (r: Ribbons) => (r as unknown as { trails: Map<string, { points: number[]; ended: boolean }> }).trails

describe('Ribbons', () => {
  it('lays points down only after the head has moved a step', () => {
    const r = new Ribbons()
    r.extend('a', 0, 0, 0, style)
    r.extend('a', 0.4, 0, 0, style)
    expect(trails(r).get('a')!.points.length).toBe(4)
    r.extend('a', 1.2, 0, 0, style)
    expect(trails(r).get('a')!.points.length).toBe(8)
  })

  it('keeps an ended trail until it has faded, then drops it', () => {
    const r = new Ribbons()
    r.extend('a', 0, 0, 0, style)
    r.extend('a', 3, 0, 0, style)
    r.end('a')
    r.update(0.5)
    expect(trails(r).has('a')).toBe(true)
    r.update(0.6)
    expect(trails(r).has('a')).toBe(false)
  })

  it('treats a trail nobody extends any more as ended', () => {
    const r = new Ribbons()
    r.extend('a', 0, 0, 0, style)
    r.update(0.6)
    expect(trails(r).get('a')!.ended).toBe(true)
  })
})
