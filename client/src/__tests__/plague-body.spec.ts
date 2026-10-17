import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { feet, pose, worldCapsules } from '../sim/plague-body'

// `fixtures/parity/plague.json` is generated from `crates/shared/src/plague.rs` by
// `cargo test -p shared plague` (`UPDATE_PARITY=1` rewrites it). A failure here means
// `client/src/sim/plague-body.ts` drifted from it: the model and the server's hitboxes no longer agree.
const fixture = JSON.parse(readFileSync(resolve(process.cwd(), '../fixtures/parity/plague.json'), 'utf8')) as {
  mode: number
  progress: number
  toppling: boolean
  feet: [number, number, number][]
  capsules: { a: number[]; b: number[]; radius: number; solid: boolean }[]
}[]

const close = (actual: readonly number[], expected: readonly number[], what: string) => {
  actual.forEach((v, i) => expect(Math.abs(v - expected[i]!), `${what}[${i}]: ${v} vs ${expected[i]}`).toBeLessThan(2e-3))
}

describe("the Orange Plague's body matches the server's", () => {
  it.each(fixture.map((c) => [`mode ${c.mode} at ${c.progress}${c.toppling ? ', toppling' : ''}`, c] as const))('%s', (_, c) => {
    const p = pose(c.mode, c.progress, c.toppling)
    const body = worldCapsules(p, [10, 2, -5], 0.7)
    expect(body.length).toBe(c.capsules.length)
    body.forEach((capsule, i) => {
      const expected = c.capsules[i]!
      close(capsule.a, expected.a, `capsule ${i} a`)
      close(capsule.b, expected.b, `capsule ${i} b`)
      expect(capsule.radius).toBeCloseTo(expected.radius, 4)
      expect(capsule.solid).toBe(expected.solid)
    })
    feet(p).forEach((foot, i) => close(foot, c.feet[i]!, `foot ${i}`))
  })
})
