import { describe, expect, it } from 'vitest'
import { formatBytes, formatDuration, formatRate, percent, remaining, stageWidth } from '../editor/format'

describe('editor progress text', () => {
  it('sizes bytes in binary units with one decimal when small', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(312 * 1024 * 1024)).toBe('312 MB')
    expect(formatBytes(1.4 * 1024 ** 3)).toBe('1.4 GB')
  })

  it('shows a rate in megabytes and megabits', () => {
    expect(formatRate(0)).toBe('idle')
    expect(formatRate(2.4 * 1024 * 1024)).toBe('2.4 MB/s · 20 Mbit/s')
    expect(formatRate(20 * 1024 * 1024)).toBe('20 MB/s · 168 Mbit/s')
  })

  it('reads durations as a clock', () => {
    expect(formatDuration(42_000)).toBe('0:42')
    expect(formatDuration(725_000)).toBe('12:05')
    expect(formatDuration(3_723_000)).toBe('1:02:03')
  })

  it('estimates what is left from the pace so far', () => {
    expect(remaining(60_000, 0, 10)).toBe(0)
    expect(remaining(60_000, 3, 10)).toBe(140_000)
    expect(remaining(60_000, 10, 10)).toBe(0)
  })

  it('sizes a stage bar from its steps and state', () => {
    expect(stageWidth(1, 4, 'running')).toBe('25%')
    expect(stageWidth(0, 0, 'running')).toBeUndefined()
    expect(stageWidth(0, 0, 'done')).toBe('100%')
    expect(stageWidth(0, 0, 'failed')).toBe('0%')
  })

  it('caps a bar at full', () => {
    expect(percent(1, 4)).toBe('25%')
    expect(percent(5, 4)).toBe('100%')
    expect(percent(0, 0)).toBe('0%')
  })
})
