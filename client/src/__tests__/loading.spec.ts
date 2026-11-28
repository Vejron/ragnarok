import { describe, expect, it } from 'vitest'
import { fractionOf, freshStages, LoadTracker, STAGE_WEIGHTS, type LoadProgress } from '@/loading'

describe('the loading screen\'s progress', () => {
  it('shares the whole bar out between the stages', () => {
    const sum = Object.values(STAGE_WEIGHTS).reduce((a, b) => a + b, 0)
    expect(sum).toBeCloseTo(1, 9)
    expect(fractionOf(freshStages())).toBe(0)
    expect(fractionOf(freshStages().map((stage) => ({ ...stage, state: 'done' as const })))).toBeCloseTo(1, 9)
  })

  it('fills a counted stage by its count', () => {
    const stages = freshStages()
    const terrain = stages.find((stage) => stage.key === 'terrain')!
    Object.assign(terrain, { state: 'running', done: 1, of: 4 })
    expect(fractionOf(stages)).toBeCloseTo(STAGE_WEIGHTS.terrain / 4, 9)
  })

  it('follows stages that run at once and marks the one that fails', async () => {
    const seen: LoadProgress[] = []
    const tracker = new LoadTracker((progress) => seen.push(progress))
    let finish = () => {}
    const slow = tracker.run('physics', () => new Promise<void>((resolve) => (finish = resolve)))
    const broken = tracker.run('world', () => Promise.reject(new Error('no world')))
    expect(tracker.progress.stages.filter((stage) => stage.state === 'running').map((stage) => stage.key)).toEqual(['world', 'physics'])
    await expect(broken).rejects.toThrow('no world')
    finish()
    await slow
    const states = Object.fromEntries(tracker.progress.stages.map((stage) => [stage.key, stage.state]))
    expect(states.world).toBe('failed')
    expect(states.physics).toBe('done')
    expect(states.terrain).toBe('waiting')
    expect(seen.length).toBeGreaterThanOrEqual(4)
  })

  it('never moves the bar back', async () => {
    const tracker = new LoadTracker()
    void tracker.run('terrain', () => new Promise<void>(() => {}))
    tracker.count('terrain', 3, 4)
    const before = tracker.progress.fraction
    // More tiles came into range: the count's share drops, the bar does not.
    tracker.count('terrain', 3, 12)
    expect(tracker.progress.fraction).toBe(before)
    tracker.count('terrain', 12, 12)
    expect(tracker.progress.fraction).toBeGreaterThan(before)
  })

  it('says what is still arriving behind the first frame until it has', async () => {
    const tracker = new LoadTracker()
    let arrive = () => {}
    tracker.behind('Loading the vehicles', new Promise<void>((resolve) => (arrive = resolve)))
    expect(tracker.progress.background).toBe('Loading the vehicles')
    arrive()
    await Promise.resolve()
    expect(tracker.progress.background).toBeNull()
  })
})
