import { describe, expect, it } from 'vitest'
import { FACE_EYE_LEFT, FACE_MOUTH, PLAGUE_STOMP, PLAGUE_STRIKE, PLAGUE_WALK } from '../net/codec'
import { PlagueModel } from '../render/plague'
import { pose, worldCapsules } from '../sim/plague-body'
import { muffle, plagueLine } from '../plague-lines'
import { phrases, voiceScore } from '../plague-voice'
import { BARK_FIRED, BARK_GAGGED, BARK_STRIKE } from '../net/codec'

/** Where a node of the model is in the world. */
const at = (node: { matrixWorld: ArrayLike<number> }) => [node.matrixWorld[12]!, node.matrixWorld[13]!, node.matrixWorld[14]!]

describe('the Orange Plague is drawn where he is hit', () => {
  it.each([
    ['walking', PLAGUE_WALK, 0.3],
    ['stomping', PLAGUE_STOMP, 1],
    ['on the phone', PLAGUE_STRIKE, 1],
  ])('%s', (_, mode, progress) => {
    const model = new PlagueModel()
    const position: [number, number, number] = [30, 2, -12]
    model.update(position, 0.9, mode, progress, false, 0, 0, null, 1 / 60)
    model.root.updateWorldMatrix(true, true)
    const body = worldCapsules(pose(mode, progress, false), position, 0.9)
    // Shins start at the knees and forearms at the elbows, his left first (see `capsules`).
    const knees = body.filter((c) => c.part === 'leg').filter((_, i) => i % 3 === 1)
    // Arms are upper arm, forearm and (on his left) hand: forearms at 1 and 4.
    const arms = body.filter((c) => c.part === 'arm')
    const elbows = [arms[1]!, arms[4]!]
    model['knees'].forEach((knee, i) => at(knee).forEach((v, k) => expect(v).toBeCloseTo(knees[i]!.a[k]!, 3)))
    model['elbows'].forEach((elbow, i) => at(elbow).forEach((v, k) => expect(v).toBeCloseTo(elbows[i]!.a[k]!, 3)))
    model.dispose()
  })

  it('swaps what is shot out of his face for its bandage', () => {
    const model = new PlagueModel()
    model.update([0, 0, 0], 0, PLAGUE_WALK, 0, false, FACE_EYE_LEFT | FACE_MOUTH, 0, null, 1 / 60)
    const features = model['features']
    expect(features[0]!.whole.visible).toBe(false)
    expect(features[0]!.lost.visible).toBe(true)
    expect(features[1]!.whole.visible).toBe(true)
    expect(features[4]!.lost.visible).toBe(true)
    model.dispose()
  })
})

describe('his lines', () => {
  it('names the player in the subtitle, and points at a recording without the name', () => {
    const line = plagueLine(BARK_FIRED, 0, 'Bob')!
    expect(line.text).toBe("You're fired, Bob!")
    expect(line.clip).toBe('fired_1')
  })

  it('skips lines that need a name when there is nobody to name', () => {
    for (let variant = 0; variant < 8; variant++) expect(plagueLine(BARK_STRIKE, variant, null)!.text).not.toMatch(/\byou\b/)
  })

  it('comes out muffled through tape', () => {
    expect(muffle("You can't silence me!")).toBe('Mmf Mmmmph Mmmmmph Mmf!')
    expect(plagueLine(BARK_GAGGED, 0, null)).not.toBeNull()
  })

  it('is said a phrase at a time, shouted where it ends in a bang', () => {
    const parts = phrases('Nice town. Small! Very small...')
    expect(parts.map((p) => p.text)).toEqual(['Nice town.', 'Small!', 'Very small...'])
    expect(parts[1]!.pitch).toBeGreaterThan(parts[0]!.pitch)
  })

  it("prefers a natural man's English voice to a robot or a foreign one", () => {
    const natural = voiceScore({ name: 'Microsoft Guy Online (Natural) - English (United States)', lang: 'en-US', localService: false })
    const plain = voiceScore({ name: 'Google US English', lang: 'en-US', localService: false })
    const novelty = voiceScore({ name: 'Bad News', lang: 'en-US', localService: true })
    const swedish = voiceScore({ name: 'Alva', lang: 'sv-SE', localService: true })
    expect(natural).toBeGreaterThan(plain)
    expect(plain).toBeGreaterThan(novelty)
    expect(swedish).toBeLessThan(0)
  })
})
