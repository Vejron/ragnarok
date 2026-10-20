import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { boxOf, triangleCount } from './helpers'
import { Mesh } from '@ragnarok/engine'
import { HenModel, parseHenModel } from '@/render/hen'
import { THRUST_AIR, THRUST_GLIDE, THRUST_LIFT, type Arms } from '@/net/codec'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const AT: [number, number, number] = [0, 5.3, 0]
const arms = (shots: number): Arms => ({
  shots,
  heat: 0,
  rearHeat: 0,
  bombs: 6,
  flags: 0,
})

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/hen.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseHenModel(copy.buffer)
})

// The model eases by the time between frames: step the clock a 60th of a second a frame.
let clock = 0
beforeEach(() => {
  clock = 0
  vi.spyOn(performance, 'now').mockImplementation(() => (clock += 1000 / 60))
})
afterEach(() => vi.restoreAllMocks())

/** `frames` frames standing at `AT` with `flags`. */
function run(hen: HenModel, flags: number, frames: number, shots = 0) {
  for (let k = 0; k < frames; k++) {
    hen.setFlags(flags)
    hen.update(AT, IDENTITY, 0, [0], null, arms(shots))
  }
}

const named = (hen: HenModel, name: string) => hen.root.getObjectByName(name)!
/** The lowest point of the foot under `ankle`, in the world: `boxOf` updates only below the node it is given. */
function sole(hen: HenModel, ankle: string) {
  hen.root.updateMatrixWorld(true)
  return boxOf(named(hen, ankle)).min.y
}

describe('HenModel', () => {
  it('has the parts the game moves, and stays low-poly', () => {
    const hen = new HenModel(1)
    for (const name of ['body', 'head', 'wing_L', 'wing_R', 'tail', 'hip_L', 'knee_R', 'ankle_L']) expect(named(hen, name), name).toBeTruthy()
    expect(triangleCount(hen.root)).toBeLessThan(4000)
    hen.dispose()
  })

  it('comes as a white hen with a red comb or a silver one with a dark grey comb', () => {
    const colours = (hen: HenModel) => {
      const found = new Set<number>()
      hen.root.traverse((node) => {
        if (node instanceof Mesh) found.add(node.material.color.getHex())
      })
      return found
    }
    const white = colours(new HenModel(0))
    const silver = colours(new HenModel(1))
    expect(white.has(0xf2efe6) && !white.has(0xa9adb3) && !white.has(0x3b3d40)).toBe(true)
    expect(silver.has(0xa9adb3) && silver.has(0x3b3d40) && !silver.has(0xf2efe6)).toBe(true)
    // The silver hen keeps none of the white one's red comb: the only colours it adds or drops are those three.
    expect([...white].filter((c) => !silver.has(c))).toHaveLength(2)
  })

  it('stands with its feet on the ground under the hull', () => {
    const hen = new HenModel(1)
    run(hen, 0, 30)
    expect(sole(hen, 'ankle_L')).toBeGreaterThan(-0.3)
    expect(sole(hen, 'ankle_L')).toBeLessThan(0.4)
    hen.dispose()
  })

  it('folds its wings on the ground, spreads them to glide and beats them to flap', () => {
    const hen = new HenModel(1)
    run(hen, 0, 30)
    const folded = named(hen, 'wing_R').rotation.z
    expect(Math.abs(folded)).toBeLessThan(0.05)
    run(hen, THRUST_AIR | THRUST_GLIDE, 60)
    expect(named(hen, 'wing_R').rotation.z).toBeCloseTo(1.5, 1)
    expect(named(hen, 'wing_L').rotation.z).toBeCloseTo(-1.5, 1)
    const beat: number[] = []
    for (let k = 0; k < 60; k++) {
      run(hen, THRUST_AIR | THRUST_LIFT, 1)
      beat.push(named(hen, 'wing_R').rotation.z)
    }
    expect(Math.max(...beat) - Math.min(...beat)).toBeGreaterThan(1)
    expect(hen.takeEvents().beats).toBeGreaterThanOrEqual(3)
    hen.dispose()
  })

  it('tucks its feet up in the air and thumps down on both when it lands', () => {
    const hen = new HenModel(1)
    run(hen, 0, 30)
    const standing = sole(hen, 'ankle_R')
    hen.takeSteps()
    run(hen, THRUST_AIR | THRUST_LIFT, 30)
    expect(hen.takeEvents().leapt).toBe(true)
    expect(sole(hen, 'ankle_R')).toBeGreaterThan(standing + 1)
    run(hen, 0, 1)
    const landing = hen.takeSteps()
    expect(landing).toHaveLength(2)
    expect(landing.every((step) => step.weight === 1)).toBe(true)
    hen.dispose()
  })

  it('flicks its tail up as an egg is laid', () => {
    const hen = new HenModel(1)
    run(hen, 0, 10, 0)
    expect(hen.takeEvents().eggs).toBe(0)
    run(hen, 0, 1, 1)
    expect(hen.takeEvents().eggs).toBe(1)
    expect(named(hen, 'tail').rotation.x).toBeLessThan(-0.5)
    run(hen, 0, 120, 1)
    expect(named(hen, 'tail').rotation.x).toBeGreaterThan(-0.05)
    hen.dispose()
  })
})
