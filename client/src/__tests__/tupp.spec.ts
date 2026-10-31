import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { triangleCount } from './helpers'
import { Mesh } from '@ragnarok/engine'
import { parseTuppModel, TuppModel } from '@/render/tupp'
import { kindView } from '@/kinds'
import { KIND_TUPP } from '@/net/codec'
import { TUPP_HEAD_PIVOT } from '@/sim/consts'
import { aimsReticle } from '@/seathints'
import type { Turret } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const ORIGIN: [number, number, number] = [0, 0, 0]
const aimed = (yaw: number, pitch: number) => ({ yaw, pitch, shots: 0 }) as Turret

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/tupp.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseTuppModel(copy.buffer)
})

describe('TuppModel', () => {
  it('is cheap enough to draw, and is what its kind builds', () => {
    const tupp = kindView(KIND_TUPP).model(1)
    expect(tupp).toBeInstanceOf(TuppModel)
    expect(triangleCount(tupp.root)).toBeLessThan(6000)
    tupp.dispose()
  })

  it('wears each breed its own plumage and hackle', () => {
    const colours = (tupp: TuppModel) => {
      const seen = new Set<number>()
      tupp.root.traverse((node) => {
        if (node instanceof Mesh && ['paint', ''].includes(node.material.name)) seen.add(node.material.color.getHex())
      })
      return seen
    }
    expect(colours(new TuppModel(0))).not.toEqual(colours(new TuppModel(1)))
  })

  it('has its skull where the server aims the eyes from', () => {
    const tupp = new TuppModel(0)
    tupp.update(ORIGIN, IDENTITY, 0, [0], aimed(0, 0))
    const { origin, direction } = tupp.barrel()
    expect(origin.toArray().map((v) => Math.round(v * 100) / 100)).toEqual([...TUPP_HEAD_PIVOT])
    expect(direction.z).toBeCloseTo(-1)
    tupp.dispose()
  })

  it('turns its eyes left and up with the mount, in that order', () => {
    const tupp = new TuppModel(0)
    tupp.update(ORIGIN, IDENTITY, 0, [0], aimed(0.5, 0.3))
    const { direction } = tupp.barrel()
    // The server's own sum: `aim_from` in gunnery.rs.
    expect(direction.x).toBeCloseTo(-Math.sin(0.5) * Math.cos(0.3))
    expect(direction.y).toBeCloseTo(Math.sin(0.3))
    expect(direction.z).toBeCloseTo(-Math.cos(0.5) * Math.cos(0.3))
    const [left, right] = tupp.eyes()
    expect(left!.x).toBeLessThan(right!.x)
    tupp.dispose()
  })

  it('aims with a reticle', () => {
    expect(aimsReticle('tupp')).toBe(true)
  })
})
