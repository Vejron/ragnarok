import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { Vector3 } from '@ragnarok/engine'
import { Character, Crowd, parseCharacter } from '@/render/character'
import { Ragdolls, type Knockable, type RagdollBody, type RagdollPhysics } from '@/sim/ragdoll'
import { STANDING_Y } from '@/sim/consts'
import { LIMB_ARM_L } from '@/net/codec'
import { flatMotor } from './worlds'
import type { Vec3 } from '@/sim/motor'

const file = resolve(process.cwd(), 'public/models/player.glb')

async function crowd() {
  const bytes = readFileSync(file)
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  const c = new Crowd()
  c.asset = await parseCharacter(copy.buffer)
  return c
}

/** Physics that only keeps count and what it was told, and a body that only remembers what was done to it. */
function fakes() {
  const live = new Set<number>()
  const told: string[] = []
  const asleep = new Set<number>()
  const physics: RagdollPhysics = {
    spawnRagdoll: (key, _s, _side, velocity) => (live.add(key), told.push(`spawn ${key} ${velocity.join(',')}`), true),
    setRagdollTarget: () => {},
    clearRagdollTarget: (key) => void told.push(`free ${key}`),
    pushRagdoll: (key, velocity, part = -1) => void told.push(`push ${key} ${velocity.join(',')} ${part}`),
    severRagdoll: (key, limbs) => void told.push(`sever ${key} ${limbs}`),
    ragdollAsleep: (key) => asleep.has(key),
    stepRagdolls: () => {},
    ragdollPose: (key) => (live.has(key) ? new Float32Array(77) : null),
    removeRagdoll: (key) => void live.delete(key),
  }
  const body = (): RagdollBody & { log: string[]; thrown: { velocity: Vec3; head: Vec3 } | null } => {
    const log: string[] = []
    let frozen = false
    const b = {
      log,
      intact: true,
      thrown: null as { velocity: Vec3; head: Vec3 } | null,
      get frozen() {
        return frozen
      },
      ragdollSegments: () => ({ segments: new Float32Array(66), side: [1, 0, 0] as Vec3 }),
      startRagdoll: () => void log.push('start'),
      driveRagdoll: () => void log.push('drive'),
      endRagdoll: (rise = true) => void log.push(rise ? 'rise' : 'let go'),
      freezeRagdoll: () => ((frozen = true), void log.push('freeze')),
      takeThrow: () => {
        const t = b.thrown
        b.thrown = null
        return t
      },
    }
    return b
  }
  return { live, physics, body, told, asleep }
}

const soldier = (key: number, body: RagdollBody, down: boolean, dead = false, x = 0): Knockable => ({
  key, body, down, dead, position: [x, STANDING_Y, 0], velocity: [6, 4, 0], lost: 0,
})

describe('Ragdolls', () => {
  it('makes one as a soldier is knocked down and lets it go to get up', () => {
    const { live, physics, body } = fakes()
    const ragdolls = new Ragdolls(physics)
    const a = body()
    ragdolls.update([soldier(1, a, false)], [0, 0, 0], 1 / 60)
    expect(live.size).toBe(0)
    ragdolls.update([soldier(1, a, true)], [0, 0, 0], 1 / 60)
    expect(live.has(1)).toBe(true)
    ragdolls.update([soldier(1, a, true)], [0, 0, 0], 1 / 60)
    ragdolls.update([soldier(1, a, false)], [0, 0, 0], 1 / 60)
    expect(live.size).toBe(0)
    expect(a.log).toEqual(['start', 'drive', 'drive', 'rise'])
  })

  it('keeps a body that dies down as a ragdoll, and lets it go without getting up', () => {
    const { live, physics, body } = fakes()
    const ragdolls = new Ragdolls(physics)
    const a = body()
    ragdolls.update([soldier(1, a, true)], [0, 0, 0], 1 / 60)
    ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    expect(live.has(1)).toBe(true)
    // Blown apart, or gone from view.
    ragdolls.update([], [0, 0, 0], 1 / 60)
    expect(live.size).toBe(0)
    expect(a.log[a.log.length - 1]).toBe('drive')
  })

  it('makes a death an untethered ragdoll thrown by the blow as well as by what moved it', () => {
    const { live, physics, body, told } = fakes()
    const ragdolls = new Ragdolls(physics)
    const a = body()
    ragdolls.update([soldier(1, a, false)], [0, 0, 0], 1 / 60)
    a.thrown = { velocity: [1, 0, 0], head: [0, 0, 3] }
    ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    expect(live.has(1)).toBe(true)
    expect(told).toEqual(['spawn 1 7,4,0', 'free 1', 'push 1 0,0,3 2'])
  })

  it('pushes a body whose killing blow is told after it fell, and takes off a limb lost in flight', () => {
    const { physics, body, told } = fakes()
    const ragdolls = new Ragdolls(physics)
    const a = body()
    ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    told.length = 0
    a.thrown = { velocity: [0, 1, 0], head: [0, 0, 0] }
    ragdolls.update([{ ...soldier(1, a, false, true), lost: LIMB_ARM_L }], [0, 0, 0], 1 / 60)
    expect(told).toEqual([`sever 1 ${LIMB_ARM_L}`, 'push 1 0,1,0 -1'])
  })

  it('freezes a body once it has lain still, and makes no more of it', () => {
    const { live, physics, body, asleep } = fakes()
    const ragdolls = new Ragdolls(physics)
    const a = body()
    ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    asleep.add(1)
    for (let i = 0; i < 3; i++) ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    expect(live.size).toBe(0)
    expect(a.log.filter((l) => l === 'freeze')).toHaveLength(1)
    expect(a.log.filter((l) => l === 'start')).toHaveLength(1)
  })

  it('makes room for a death at the cap by freezing the longest dead', () => {
    const { live, physics, body } = fakes()
    const ragdolls = new Ragdolls(physics, 2)
    const [a, b, c] = [body(), body(), body()]
    ragdolls.update([soldier(1, a, false, true)], [0, 0, 0], 1 / 60)
    ragdolls.update([soldier(1, a, false, true), soldier(2, b, false, true)], [0, 0, 0], 1 / 60)
    ragdolls.update([soldier(1, a, false, true), soldier(2, b, false, true), soldier(3, c, false, true)], [0, 0, 0], 1 / 60)
    expect([...live].sort()).toEqual([2, 3])
    expect(a.frozen).toBe(true)
  })

  it('holds no more than its cap, the nearest first', () => {
    const { live, physics, body } = fakes()
    const ragdolls = new Ragdolls(physics, 2)
    const crowdOf = [30, 5, 10, 50].map((x, i) => soldier(i + 1, body(), true, false, x))
    ragdolls.update(crowdOf, [0, 0, 0], 1 / 60)
    expect([...live].sort()).toEqual([2, 3])
  })
})

describe('A ragdolled soldier', () => {
  it('is thrown, lands, and gets up again where it lies', async () => {
    const c = await crowd()
    const motor = await flatMotor()
    const person = new Character(c, 1)
    const ragdolls = new Ragdolls(motor)
    const hips = () => (person as unknown as { bones: Map<string, import('@ragnarok/engine').Node> }).bones.get('hips')!.getWorldPosition(new Vector3())
    let at: [number, number, number] = [0, STANDING_Y, 0]
    const frame = (down: boolean) => {
      person.update(at, 0, 0, 0, true, 1 / 60)
      ragdolls.update([{ key: 1, body: person, down, dead: false, position: at, velocity: [8, 5, 0], lost: 0 }], [0, 5, 10], 1 / 60)
    }
    for (let i = 0; i < 10; i++) frame(false)
    const standing = hips()
    // Thrown along +x; the server's capsule slides there too.
    for (let i = 0; i < 120; i++) {
      at = [Math.min(4, i * 0.08), STANDING_Y, 0]
      frame(true)
    }
    expect(person.ragdolled).toBe(true)
    const lying = hips()
    expect(lying.x).toBeGreaterThan(2.5)
    expect(lying.y).toBeLessThan(standing.y - 0.4)
    // Up again: through the get-up clip and back on its feet over the capsule.
    for (let i = 0; i < 90; i++) frame(false)
    expect(person.ragdolled).toBe(false)
    const up = hips()
    expect(up.y).toBeCloseTo(standing.y, 1)
    expect(Math.hypot(up.x - at[0], up.z - at[2])).toBeLessThan(0.3)
    expect(person.blend.get('getup_back') ?? 0).toBeLessThan(0.5)
    expect(person.blend.get('getup_front') ?? 0).toBeLessThan(0.5)
    motor.dispose()
    person.dispose()
    c.dispose()
  })

  it('killed, flies back off its feet as a ragdoll, comes to rest lying down and is frozen as it lies', async () => {
    const c = await crowd()
    const motor = await flatMotor()
    const person = new Character(c, 1)
    const ragdolls = new Ragdolls(motor)
    const hips = () => (person as unknown as { bones: Map<string, import('@ragnarok/engine').Node> }).bones.get('hips')!.getWorldPosition(new Vector3())
    const at: [number, number, number] = [0, STANDING_Y, 0]
    const frame = (dead: boolean) => {
      person.update(at, 0, 0, 0, true, 1 / 60)
      ragdolls.update([{ key: 1, body: person, down: false, dead, position: at, velocity: [0, 0, 0], lost: 0 }], [0, 5, 10], 1 / 60)
    }
    for (let i = 0; i < 10; i++) frame(false)
    const standing = hips()
    // A round from the front (-z) knocks it back along +z.
    person.dead = true
    person.throwBody([0, 1.2, 3])
    frame(true)
    expect(person.ragdolled).toBe(true)
    for (let i = 0; i < 360 && person.ragdolled; i++) frame(true)
    expect(person.ragdolled).toBe(false)
    expect(person.frozen).toBe(true)
    const lying = hips()
    expect(lying.z).toBeGreaterThan(0.5)
    expect(lying.y).toBeLessThan(standing.y - 0.4)
    // Frozen, it stays as it lies.
    for (let i = 0; i < 30; i++) frame(true)
    expect(hips().distanceTo(lying)).toBeLessThan(1e-4)
    // Respawned: on its feet again.
    person.dead = false
    for (let i = 0; i < 10; i++) frame(false)
    expect(person.frozen).toBe(false)
    expect(hips().y).toBeCloseTo(standing.y, 1)
    motor.dispose()
    person.dispose()
    c.dispose()
  })
})
