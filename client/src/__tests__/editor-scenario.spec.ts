import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { emptyScenario, renameName, squadsFromBots, usesName, validateScenario, type Scenario } from '@/editor/scenario'

/** Every point is in the world, except where a test says otherwise. */
const inside = () => true

/** A small valid scenario to break in each test. */
function sample(): Scenario {
  return {
    restart: 30,
    areas: { meadow: { lat: 63.84, lon: 20.31, radius: 70 } },
    squads: {
      thieves: { team: 2, skill: 0.4, loadout: ['Revolver'], bots: [{ lat: 63.84, lon: 20.31, route: [{ lat: 63.84, lon: 20.32 }] }], tanks: [] },
      tankers: { team: 3, skill: 0.6, bots: [{ lat: 63.83, lon: 20.31 }], tanks: [{ lat: 63.82, lon: 20.31 }] },
    },
    triggers: [
      { when: { start: true }, do: [{ spawn: 'thieves' }, { objective: 'Clear the meadow' }] },
      { name: 'cleared', when: { cleared: 'thieves' }, wait: 5, do: [{ spawn: 'tankers' }] },
      { when: { enter: 'meadow', by: 'tankers' }, do: [{ lose: 'Reached' }] },
      { when: { done: 'cleared' }, times: 1, do: [{ win: 'Done' }] },
    ],
  }
}

const messages = (s: Scenario, where: (lat: number, lon: number) => boolean = inside) => validateScenario(s, where).map((i) => `${i.path}: ${i.message}`)

describe('validateScenario', () => {
  it('accepts a valid scenario and an empty one', () => {
    expect(messages(sample())).toEqual([])
    expect(messages(emptyScenario())).toEqual([])
  })

  it('accepts the shipped Mariehem scenario', () => {
    const shipped = JSON.parse(readFileSync(resolve(process.cwd(), '../worlds/boda/scenario.json'), 'utf8')) as Scenario
    expect(messages(shipped)).toEqual([])
  })

  it('flags positions outside the world, with where they are', () => {
    const s = sample()
    s.squads.tankers!.tanks![0] = { lat: 0, lon: 0 }
    expect(messages(s, (lat) => lat > 60)).toEqual(['squads.tankers.tanks[0]: is outside the world'])
  })

  it('flags references to names that do not exist', () => {
    const s = sample()
    s.triggers[0]!.do = [{ spawn: 'nobody' }]
    s.triggers[2]!.when = { enter: 'shed', by: 'ghosts' }
    s.triggers[3]!.when = { done: 'later' }
    expect(messages(s)).toEqual([
      'triggers[0].do[0]: there is no squad called "nobody"',
      'triggers[2].when.enter: there is no area called "shed"',
      'triggers[2].when.by: there is no squad called "ghosts"',
      'triggers[3].when.done: there is no trigger called "later"',
    ])
  })

  it('needs a condition on every trigger, and "by" only with "enter"', () => {
    const s = sample()
    s.triggers[0]!.when = {}
    s.triggers[1]!.when = { by: 'thieves', cleared: 'thieves' }
    expect(messages(s)).toEqual(['triggers[0]: needs a condition: start, after, enter, cleared or done', 'triggers.cleared.when.by: "by" goes with "enter"'])
  })

  it('flags duplicate trigger names', () => {
    const s = sample()
    s.triggers.push({ name: 'cleared', when: { start: true }, do: [] })
    expect(messages(s)).toEqual(['triggers.cleared: two triggers have this name'])
  })

  it('checks squads: a crew needs a bot or tank, team 1 or more, skill 0 to 1, a loadout the game knows', () => {
    const s = sample()
    s.squads.empty = { team: 0, skill: 1.5, bots: [], tanks: [] }
    s.squads.thieves!.loadout = ['Rocket', 'AK5', 'AK5', 'AK5', 'AK5', 'AK5']
    expect(messages(s)).toEqual([
      'squads.thieves.loadout: has at most 5 items',
      'squads.thieves.loadout[0]: no item called "Rocket"',
      'squads.empty.team: must be 1 or more',
      'squads.empty.skill: must be between 0 and 1',
      'squads.empty: needs at least one bot or tank',
    ])
  })

  it('flags text over 200 characters and a bad radius', () => {
    const s = sample()
    s.triggers[0]!.do = [{ say: 'x'.repeat(201) }]
    s.areas.meadow!.radius = 0
    expect(messages(s)).toEqual(['areas.meadow.radius: must be more than 0 metres', 'triggers[0].do[0]: is longer than 200 characters'])
  })
})

describe('renaming and using names', () => {
  it('lists the triggers that use a name', () => {
    expect(usesName(sample(), 'squad', 'tankers')).toEqual(['trigger cleared', 'trigger 3'])
    expect(usesName(sample(), 'trigger', 'cleared')).toEqual(['trigger 4'])
    expect(usesName(sample(), 'area', 'meadow')).toEqual(['trigger 3'])
  })

  it('renames a squad in its definition and everywhere a trigger names it', () => {
    const renamed = renameName(sample(), 'squad', 'tankers', 'crew')
    expect(Object.keys(renamed.squads)).toEqual(['thieves', 'crew'])
    expect(renamed.triggers[1]!.do).toEqual([{ spawn: 'crew' }])
    expect(renamed.triggers[2]!.when).toEqual({ enter: 'meadow', by: 'crew' })
  })

  it('renames a trigger and the done it is named by', () => {
    const renamed = renameName(sample(), 'trigger', 'cleared', 'thieves-gone')
    expect(renamed.triggers[1]!.name).toBe('thieves-gone')
    expect(renamed.triggers[3]!.when.done).toBe('thieves-gone')
  })

  it('leaves the scenario alone for a taken or empty name', () => {
    const s = sample()
    expect(renameName(s, 'squad', 'tankers', 'thieves')).toBe(s)
    expect(renameName(s, 'area', 'meadow', '  ')).toBe(s)
  })
})

describe('squadsFromBots', () => {
  const bot = (team: number, skill: number, lat: number, route: { lat: number; lon: number }[] = []) => ({ lat, lon: 20.3, team, skill, route })

  it('makes a squad for each team and skill, spawned together at the start, and respawning', () => {
    const next = squadsFromBots(sample(), [bot(2, 0.5, 63.81), bot(2, 0.5, 63.82), bot(3, 0.8, 63.83)])
    expect(Object.keys(next.squads)).toEqual(['thieves', 'tankers', 'placed-team-2', 'placed-team-3'])
    expect(next.squads['placed-team-2']).toMatchObject({ team: 2, skill: 0.5, respawn: true, tanks: [] })
    expect(next.squads['placed-team-2']!.bots).toHaveLength(2)
    expect(next.triggers[next.triggers.length - 1]).toEqual({ when: { start: true }, do: [{ spawn: 'placed-team-2' }, { spawn: 'placed-team-3' }] })
    expect(messages(next)).toEqual([])
  })

  it('keeps routes, and names squads of one team with different skills apart', () => {
    const route = [{ lat: 63.81, lon: 20.31 }]
    const next = squadsFromBots(emptyScenario(), [bot(2, 0.2, 63.81, route), bot(2, 0.9, 63.82)])
    expect(Object.keys(next.squads)).toEqual(['placed-team-2-skill-0.2', 'placed-team-2-skill-0.9'])
    expect(next.squads['placed-team-2-skill-0.2']!.bots![0]!.route).toEqual(route)
    expect(next.squads['placed-team-2-skill-0.9']!.bots![0]).not.toHaveProperty('route')
  })

  it('does not overwrite a squad that is already there, and does nothing with no bots', () => {
    const s = squadsFromBots(sample(), [bot(2, 0.5, 63.81)])
    const again = squadsFromBots(s, [bot(2, 0.5, 63.82)])
    expect(Object.keys(again.squads).slice(-2)).toEqual(['placed-team-2', 'placed-team-2-2'])
    const s2 = sample()
    expect(squadsFromBots(s2, [])).toBe(s2)
  })
})
