import { describe, expect, it } from 'vitest'
import { Callouts, CALLOUT_MS } from '@/callouts'
import { HIT_DEALT, HIT_TAKEN } from '@/feedback'
import { ARMOR_BLOCKED, ARMOR_PIERCED, ARMOR_SLIT, DAMAGE_PARTS } from '@/net/codec'
import { GONE, HURT, PART_AMMO, PART_ENGINE, PART_FUEL, SOUND, calloutLines, partState } from '@/sim/armor'

describe('an armoured vehicle as the snapshot carries it', () => {
  it('unpacks each part from two bits of slots 8 and 9, as armor::pack packs them', () => {
    const damage = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    // Engine gone (2), ammunition hurt (1 << 10), fuel gone (2 << 12).
    const bits = 2 | (1 << 10) | (2 << 12)
    damage[DAMAGE_PARTS] = bits & 0xff
    damage[DAMAGE_PARTS + 1] = bits >> 8
    expect(partState(damage, PART_ENGINE)).toBe(GONE)
    expect(partState(damage, PART_AMMO)).toBe(HURT)
    expect(partState(damage, PART_FUEL)).toBe(GONE)
    expect(partState(damage, 1)).toBe(SOUND)
  })
})

describe('armour callouts', () => {
  const engine = 1 << PART_ENGINE
  it('tell the shooter it went in and what it broke, and the crew what they lost', () => {
    expect(calloutLines({ role: HIT_DEALT, result: ARMOR_PIERCED, hurt: engine, gone: engine, killed: 1 }, true).map((l) => l.text)).toEqual(['Penetration', 'Driver killed', 'Engine destroyed'])
    expect(calloutLines({ role: HIT_TAKEN, result: ARMOR_SLIT, hurt: 0, gone: 0, killed: 0 }, false)).toEqual([{ text: 'Shot through the slit!', tone: 'bad' }])
  })

  it('count a burst of the same instead of stacking it, newest first, and fade', () => {
    const callouts = new Callouts()
    const blocked = { role: HIT_DEALT, result: ARMOR_BLOCKED, hurt: 0, gone: 0, killed: 0 }
    callouts.receive(blocked, 0)
    callouts.receive(blocked, 100)
    callouts.receive(blocked, 200)
    expect(callouts.view(200)).toEqual([{ text: 'No penetration', tone: 'plain', count: 3, alpha: 1 }])
    callouts.receive({ ...blocked, result: ARMOR_PIERCED }, 300)
    expect(callouts.view(300).map((v) => v.text)).toEqual(['Penetration', 'No penetration'])
    expect(callouts.view(300 + CALLOUT_MS)).toEqual([])
  })
})

describe('finding the model surface a round struck', () => {
  it('projects onto a sloped plate and takes its outward normal', async () => {
    const { nearestOnTriangles } = await import('@/render/surface')
    // A glacis leaning back 45 degrees, facing forward (-Z) and up, wound counter-clockwise seen from the front.
    const positions = [-1, 0, 0, 1, 1, 1, 1, 0, 0, -1, 0, 0, -1, 1, 1, 1, 1, 1]
    const hit = nearestOnTriangles(positions, [0, 1, 2, 0, 4, 1], [0, 0.5, -0.2], 1)!
    expect(hit.point[1]).toBeCloseTo(0.15)
    expect(hit.point[2]).toBeCloseTo(0.15)
    expect(hit.normal[1]).toBeCloseTo(Math.SQRT1_2)
    expect(hit.normal[2]).toBeCloseTo(-Math.SQRT1_2)
    expect(nearestOnTriangles(positions, [0, 1, 2], [0, 5, -5], 1)).toBeNull()
  })
})

describe('following a round into the model', () => {
  it('marks a sloped plate where the line of fire meets it, not where the box was', async () => {
    const { nearestOnTriangles, rayTriangles } = await import('@/render/surface')
    // The same 45-degree glacis; the physics box's face is upright at z = -0.2, ahead of it.
    const positions = [-1, 0, 0, 1, 1, 1, 1, 0, 0, -1, 0, 0, -1, 1, 1, 1, 1, 1]
    const indices = [0, 1, 2, 0, 4, 1]
    const box = [0, 0.5, -0.2]
    // A level round going back along +Z at the height it struck the box goes in at that height...
    const hit = rayTriangles(positions, indices, [box[0]!, box[1]!, box[2]! - 0.8], [0, 0, 1], 1.6)!
    expect(box[2]! - 0.8 + hit.t).toBeCloseTo(0.5)
    expect(hit.normal[2]).toBeLessThan(0)
    // ...where the nearest point would have slid it down the slope.
    expect(nearestOnTriangles(positions, indices, box, 1)!.point[1]).toBeCloseTo(0.15)
  })
})

describe('where a hurt vehicle smokes and burns from', () => {
  it('is the newest few holes, not scuffs, wherever the vehicle has got to, and not on a turret thrown off', async () => {
    const { Node, Vec3 } = await import('@ragnarok/engine')
    const { HullMarks, MAX_VENTS } = await import('@/render/hullmarks')
    const root = new Node()
    const turret = new Node()
    root.add(turret)
    const marks = new HullMarks()
    const at = (parent: InstanceType<typeof Node>, z: number) => ({ parent, local: new Vec3(0, 0, z), normal: new Vec3(0, 0, -1), unit: 1, world: new Vec3(), outward: new Vec3() })
    marks.mark(1, root, at(turret, -1), true)
    for (let z = 0; z < MAX_VENTS; z++) marks.mark(1, root, at(root, z), true)
    marks.mark(1, root, at(root, 9), false)
    root.position.set(10, 0, 0)
    const vents = marks.vents(1)
    expect(vents.map((v) => Math.round(v.z) + 0)).toEqual([2, 1, 0])
    expect(vents[0]!.x).toBeCloseTo(10)
    expect(vents[0]!.out.z).toBeCloseTo(-1)
    // A hole whose round hurt something inside comes first, however old.
    marks.mark(1, root, at(root, 7), true, 1, true)
    for (let z = 0; z < MAX_VENTS; z++) marks.mark(1, root, at(root, z), true)
    expect(Math.round(marks.vents(1)[0]!.z)).toBe(7)
    // A hole in a turret that has been thrown off is not one.
    turret.removeFromParent()
    marks.clear(1)
    marks.mark(1, root, at(turret, -1), true)
    expect(marks.vents(1)).toEqual([])
  })
})
