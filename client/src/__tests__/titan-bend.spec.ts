import { describe, expect, it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { bend } from '@/render/titan'
import fixture from './titan-bend.fixture.json'

describe('a Titan\'s limbs', () => {
  it('bend as the server bends its hit shapes (titan-bend.fixture.json, from shared::vehicles::titan::bend)', () => {
    for (const c of fixture.cases) {
      const joint = new Vector3()
      const end = new Vector3()
      bend(new Vector3(...(c.root as [number, number, number])), new Vector3(...(c.target as [number, number, number])), c.a, c.b, new Vector3(...(c.pole as [number, number, number])), joint, end)
      for (let i = 0; i < 3; i++) {
        expect(Math.abs(joint.toArray()[i]! - c.joint[i]!), `joint of ${JSON.stringify(c)}`).toBeLessThan(1e-3)
        expect(Math.abs(end.toArray()[i]! - c.end[i]!), `end of ${JSON.stringify(c)}`).toBeLessThan(1e-3)
      }
    }
  })
})
