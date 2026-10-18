import { describe, expect, it } from 'vitest'
import { BOT_TEAMS, headingOf, isGuard, nearest, routeLines, routeSummary, teamColour, yawOf, type PlacedBot } from '../editor/placements'

describe('editor placements', () => {
  it('turns a compass heading into the simulation yaw and back', () => {
    expect(yawOf(0)).toBe(0)
    expect(yawOf(2)).toBeCloseTo(-Math.PI / 2) // east is a right turn from north
    for (let h = 0; h < 8; h++) expect(headingOf(yawOf(h))).toBe(h)
  })

  it('picks the nearest point within reach', () => {
    const points = [{ x: 0, y: 0 }, { x: 10, y: 0 }]
    expect(nearest(points, 9, 1, 5)).toBe(1)
    expect(nearest(points, 50, 50, 5)).toBe(-1)
  })
})

describe('editor bots', () => {
  const home = { lat: 1, lon: 2 }
  const bot = (route: { lat: number; lon: number }[]): PlacedBot => ({ ...home, team: 1, skill: 0.5, route })

  it('says what a route makes the bot do', () => {
    expect(routeSummary(bot([]))).toBe('makes up its own loop near home')
    expect(routeSummary(bot([home]))).toBe('stands guard at home')
    expect(isGuard(bot([{ lat: 1, lon: 2.001 }]))).toBe(false)
    expect(routeSummary(bot([home, { lat: 3, lon: 4 }]))).toBe('walks 2 waypoints')
  })

  it('draws the way out from home and the loop back to the first waypoint', () => {
    expect(routeLines(bot([]))).toEqual({ lead: [], loop: [] })
    expect(routeLines(bot([home]))).toEqual({ lead: [], loop: [] })
    expect(routeLines(bot([{ lat: 3, lon: 4 }]))).toEqual({ lead: [[1, 2], [3, 4]], loop: [] })
    const loop = routeLines(bot([{ lat: 3, lon: 4 }, { lat: 5, lon: 6 }]))
    expect(loop.lead).toEqual([[1, 2], [3, 4]])
    expect(loop.loop).toEqual([[3, 4], [5, 6], [3, 4]])
    // A route that starts at home needs no way out to it.
    expect(routeLines(bot([home, { lat: 5, lon: 6 }])).lead).toEqual([])
  })

  it('gives every team its own colour', () => {
    expect(new Set(BOT_TEAMS.map((t) => teamColour(t.team))).size).toBe(BOT_TEAMS.length)
  })
})
