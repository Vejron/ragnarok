import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Quaternion, Vector3 } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { Character, Crowd, parseCharacter } from '@/render/character'
import { boxOf } from './helpers'
import { ITEM_AK5, ITEM_BANDAGE, ITEM_GRENADE, ITEM_NONE, ITEM_REVOLVER } from '@/sim/items'

const file = resolve(process.cwd(), 'public/models/player.glb')

async function crowd() {
  const bytes = readFileSync(file)
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  const data = copy.buffer
  const c = new Crowd()
  c.asset = await parseCharacter(data)
  return c
}

/** Runs the character for `seconds`, moving at (vx, vz) while facing `yaw`. */
function run(person: Character, yaw: number, vx: number, vz: number, seconds = 1) {
  for (let t = 0; t < seconds; t += 1 / 60) person.update([0, 0.85, 0], yaw, vx, vz, true, 1 / 60)
}

/** How much a world matrix scales its X axis. */
const scaleOf = (m: ArrayLike<number>) => Math.hypot(m[0]!, m[1]!, m[2]!)

describe('Character holding things', () => {
  it('has a clip set per stance and a mesh and muzzle for each weapon', async () => {
    const c = await crowd()
    for (const prefix of ['pistol_', 'hold_']) {
      for (const name of ['idle', 'jump_air', 'walk_fwd', 'run_left', 'crouch_idle', 'crouch_back', 'crawl_idle', 'crawl_fwd', 'slide', 'sit']) {
        expect(c.asset!.clips.has(prefix + name), prefix + name).toBe(true)
      }
    }
    for (const name of ['weapon_ak5', 'weapon_sniper', 'weapon_revolver', 'weapon_grenade', 'weapon_bandage', 'muzzle', 'muzzle_pistol']) {
      expect(c.asset!.scene.getObjectByName(name), name).toBeTruthy()
    }
    c.dispose()
  })

  it('shows only the weapon in hand', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const shown = () =>
      ['weapon_ak5', 'weapon_sniper', 'weapon_revolver', 'weapon_grenade', 'weapon_bandage'].filter((name) => {
        let visible = false
        c.root.traverse((node) => {
          if (node.name === name) visible = node.visible
        })
        return visible
      })
    expect(shown()).toEqual(['weapon_ak5'])
    person.held = ITEM_REVOLVER
    expect(shown()).toEqual(['weapon_revolver'])
    person.held = ITEM_GRENADE
    expect(shown()).toEqual(['weapon_grenade'])
    person.held = ITEM_BANDAGE
    expect(shown()).toEqual(['weapon_bandage'])
    person.held = ITEM_NONE
    expect(shown()).toEqual([])
    c.dispose()
  })

  it('blends into the stance of what it holds, and keeps the weights summing to one', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0)
    expect(person.blend.get('idle')!).toBeGreaterThan(0.9)
    person.held = ITEM_REVOLVER
    run(person, 0, 0, 0, 1)
    expect(person.blend.get('pistol_idle')!).toBeGreaterThan(0.9)
    run(person, 0, 0, -3, 1)
    expect(person.blend.get('pistol_walk_fwd')!).toBeGreaterThan(0.9)
    person.held = ITEM_GRENADE
    run(person, 0, 0, -7.5, 1)
    expect(person.blend.get('hold_run_fwd')!).toBeGreaterThan(0.9)
    let sum = 0
    for (const w of person.blend.values()) sum += w
    expect(sum).toBeCloseTo(1, 5)
    c.dispose()
  })

  it('shoots from a pistol at arm\'s length in front, and from nothing in a hands-low stance', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_REVOLVER
    run(person, 0, 0, 0)
    const muzzle = person.muzzle()!
    expect(muzzle.z).toBeLessThan(-0.4)
    expect(muzzle.y).toBeGreaterThan(0.9)
    person.held = ITEM_AK5
    run(person, 0, 0, 0)
    expect(person.muzzle()).not.toBeNull()
    person.held = ITEM_BANDAGE
    expect(person.muzzle()).toBeNull()
    c.dispose()
  })
})

describe('Character', () => {
  it('has the clips the blend expects', async () => {
    const c = await crowd()
    for (const name of ['idle', 'jump_air', 'death', 'walk_fwd', 'walk_back', 'walk_left', 'walk_right', 'run_fwd', 'run_back', 'run_left', 'run_right', 'crouch_idle', 'crouch_fwd', 'crouch_back', 'crouch_left', 'crouch_right', 'crawl_idle', 'crawl_fwd', 'crawl_back', 'crawl_left', 'crawl_right', 'slide']) {
      expect(c.asset!.clips.has(name), name).toBe(true)
    }
    expect(c.asset!.scene.getObjectByName('muzzle')).toBeTruthy()
    c.dispose()
  })

  it('stands with its feet on the ground and the rifle out to its right', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0)
    const muzzle = person.muzzle()!
    // Yaw 0 faces -Z; the rifle points that way, on the +X (right) side, around chest height.
    expect(muzzle.z).toBeLessThan(-0.3)
    expect(muzzle.x).toBeGreaterThan(0.05)
    expect(muzzle.y).toBeGreaterThan(0.8)
    expect(muzzle.y).toBeLessThan(1.4)
    const box = boxOf(c.root)
    expect(box.min.y).toBeCloseTo(0, 1)
    c.dispose()
  })

  it('turns with its yaw', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, Math.PI / 2, 0, 0, 0.2)
    // Quarter turn to the left: forward is now -X.
    expect(person.muzzle()!.x).toBeLessThan(-0.3)
    c.dispose()
  })

  it('plays the strafe clip for sideways movement and the walk clip for forward movement', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    // Facing -Z, moving along +X is to the right.
    run(person, 0, 3, 0)
    expect(person.blend.get('walk_right')!).toBeGreaterThan(0.9)
    run(person, 0, -3, 0)
    expect(person.blend.get('walk_left')!).toBeGreaterThan(0.9)
    run(person, 0, 0, -3)
    expect(person.blend.get('walk_fwd')!).toBeGreaterThan(0.9)
    run(person, 0, 0, 3)
    expect(person.blend.get('walk_back')!).toBeGreaterThan(0.9)
    // Turning the body turns what counts as forward.
    run(person, Math.PI / 2, -3, 0)
    expect(person.blend.get('walk_fwd')!).toBeGreaterThan(0.9)
    run(person, 0, 0, -7.5)
    expect(person.blend.get('run_fwd')!).toBeGreaterThan(0.9)
    run(person, 0, 0, 0)
    expect(person.blend.get('idle')!).toBeGreaterThan(0.9)
    c.dispose()
  })

  it('keeps the blend weights summing to one', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    for (let i = 0; i < 120; i++) {
      person.update([0, 0.85, 0], i * 0.05, Math.sin(i * 0.2) * 6, Math.cos(i * 0.13) * 6, i % 40 > 30, 1 / 60)
      let sum = 0
      for (const w of person.blend.values()) sum += w
      expect(sum).toBeCloseTo(1, 5)
    }
    c.dispose()
  })

  it('lays a dead player down, and hides one in a car', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0, 0.2)
    person.dead = true
    run(person, 0, 0, 0, 1.5)
    const head = () => {
      c.root.updateMatrixWorld(true)
      return c.root.getObjectByName('head')!.getWorldPosition(new Vector3()).y
    }
    expect(head()).toBeLessThan(0.5)
    person.hidden = true
    expect(person.visible).toBe(false)
    expect(person.muzzle()).toBeNull()
    person.hidden = false
    person.dead = false
    run(person, 0, 0, 0, 1)
    expect(head()).toBeGreaterThan(1.4)
    c.dispose()
  })

  it('counts and releases the characters it draws', async () => {
    const c = await crowd()
    const people = Array.from({ length: 10 }, (_, i) => new Character(c, i))
    expect(c.size).toBe(10)
    people.forEach((p) => p.dispose())
    expect(c.size).toBe(0)
    c.dispose()
  })

  it('sits with its head on the seat and turns its chest to where it aims', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const seat = new Vector3(3, 1.5, -2)
    const upright = new Quaternion()
    const sit = (yaw: number) => {
      for (let t = 0; t < 1; t += 1 / 60) person.seat(seat, upright, yaw, 0, 1 / 60)
      c.root.updateMatrixWorld(true)
    }
    sit(0)
    expect(person.blend.get('sit')!).toBeGreaterThan(0.9)
    // The head bone is the base of the head, a little below its middle.
    const head = c.root.getObjectByName('head')!.getWorldPosition(new Vector3())
    expect(head.x).toBeCloseTo(seat.x, 1)
    expect(head.z).toBeCloseTo(seat.z, 1)
    expect(head.y).toBeGreaterThan(seat.y - 0.2)
    expect(head.y).toBeLessThan(seat.y)
    // Facing the vehicle's nose (-Z) the rifle points ahead; looking left, it swings to the left (-X).
    const ahead = person.muzzle()!.clone()
    expect(ahead.z).toBeLessThan(seat.z - 0.3)
    sit(Math.PI / 2)
    const left = person.muzzle()!.clone()
    expect(left.x).toBeLessThan(seat.x - 0.3)
    c.dispose()
  })

  it('crouches low, and shuffles sideways in the crouch strafe clips', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const head = () => {
      c.root.updateMatrixWorld(true)
      return c.root.getObjectByName('head')!.getWorldPosition(new Vector3()).y
    }
    const crouched = (vx: number, vz: number, seconds = 1) => {
      for (let t = 0; t < seconds; t += 1 / 60) person.update([0, 0.85, 0], 0, vx, vz, true, 1 / 60, 0, true)
    }
    run(person, 0, 0, 0)
    const standing = head()
    crouched(0, 0)
    expect(person.blend.get('crouch_idle')!).toBeGreaterThan(0.9)
    expect(head()).toBeLessThan(standing - 0.2)
    crouched(2, 0)
    expect(person.blend.get('crouch_right')!).toBeGreaterThan(0.9)
    crouched(-2, 0)
    expect(person.blend.get('crouch_left')!).toBeGreaterThan(0.9)
    crouched(0, -2)
    expect(person.blend.get('crouch_fwd')!).toBeGreaterThan(0.9)
    // Standing up again returns to the normal clips.
    run(person, 0, 0, 0, 1)
    expect(person.blend.get('idle')!).toBeGreaterThan(0.9)
    expect(head()).toBeGreaterThan(standing - 0.05)
    c.dispose()
  })

  it('lies down into the crawl clips and crawls in them', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const lying = (vx: number, vz: number, seconds = 1) => {
      for (let t = 0; t < seconds; t += 1 / 60) person.update([0, 0.3, 0], 0, vx, vz, true, 1 / 60, 0, false, false, true)
    }
    run(person, 0, 0, 0)
    lying(0, 0)
    expect(person.blend.get('crawl_idle')!).toBeGreaterThan(0.9)
    expect(person.blend.get('idle')!).toBeLessThan(0.05)
    lying(0, -1.5)
    expect(person.blend.get('crawl_fwd')!).toBeGreaterThan(0.9)
    expect(person.blend.get('crouch_fwd')!).toBeLessThan(0.05)
    // Getting up hands back to the standing clips.
    run(person, 0, 0, 0, 1)
    expect(person.blend.get('idle')!).toBeGreaterThan(0.9)
    expect(person.blend.get('crawl_idle')!).toBeLessThan(0.05)
    c.dispose()
  })

  it('slides along its heading, whatever way the camera faces', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    for (let t = 0; t < 1; t += 1 / 60) person.update([0, 0.85, 0], 0, 6, 0, true, 1 / 60, 0, true, true)
    expect(person.blend.get('slide')!).toBeGreaterThan(0.9)
    // Camera faces -Z, but it is sliding towards +X: the rifle points that way.
    expect(person.muzzle()!.x).toBeGreaterThan(0.3)
    c.dispose()
  })

  it('tilts the rifle with the aim', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const aim = (pitch: number) => {
      for (let i = 0; i < 5; i++) person.update([0, 0.85, 0], 0, 0, 0, true, 1 / 60, pitch)
      return person.muzzle()!.y
    }
    expect(aim(0.5)).toBeGreaterThan(aim(0))
    expect(aim(-0.8)).toBeLessThan(aim(0))
    c.dispose()
  })
})

describe('Character coming alive', () => {
  /** Runs it in the air for `seconds`: falling at `vy`, under the canopy when `chute`. */
  function fly(person: Character, vy: number, chute: boolean, seconds = 1) {
    for (let t = 0; t < seconds; t += 1 / 60) person.update([0, 100, 0], 0, 0, 0, false, 1 / 60, 0, false, false, false, chute, vy)
  }

  it('has the falling clips', async () => {
    const c = await crowd()
    for (const name of ['freefall', 'chute']) expect(c.asset!.clips.has(name), name).toBe(true)
    c.dispose()
  })

  it('hangs in the harness under the canopy, and falls flat with it shut', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    fly(person, -5, true)
    expect(person.blend.get('chute')).toBeGreaterThan(0.9)
    fly(person, -30, false)
    expect(person.blend.get('freefall')).toBeGreaterThan(0.9)
    c.dispose()
  })

  it('jumps rather than falls for a short hop', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    fly(person, 2, false, 0.4)
    expect(person.blend.get('jump_air')).toBeGreaterThan(person.blend.get('freefall')!)
    c.dispose()
  })

  it('turns its chest and gun to a small look round, and its feet to a big one', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0, 0.5)
    // A little to the left: the gun follows at once, the body stays.
    run(person, 0.5, 0, 0, 0.1)
    const muzzle = person.muzzle()!
    expect(Math.atan2(-muzzle.x, -muzzle.z)).toBeGreaterThan(0.3)
    expect(person.blend.get('idle')).toBeGreaterThan(0.8)
    // A half turn: it steps round.
    let stepped = 0
    for (let t = 0; t < 1; t += 1 / 60) {
      person.update([0, 0.85, 0], Math.PI - 0.01, 0, 0, true, 1 / 60)
      stepped = Math.max(stepped, (person.blend.get('walk_left') ?? 0) + (person.blend.get('walk_right') ?? 0))
    }
    expect(stepped).toBeGreaterThan(0.1)
    expect(person.muzzle()!.z).toBeGreaterThan(0.3)
    c.dispose()
  })

  it('flinches at a new hit, but not at the first word of an old one', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0, 0.5)
    const head = () => {
      const at = new Vector3()
      c.root.updateWorldMatrix(true, true)
      c.root.getObjectByName('head')!.getWorldPosition(at)
      return at.clone()
    }
    const still = head()
    person.setStruck({ count: 3, part: 16, amount: 30, yaw: 0 })
    run(person, 0, 0, 0, 0.05)
    expect(head().distanceTo(still)).toBeLessThan(0.02)
    person.setStruck({ count: 4, part: 16, amount: 30, yaw: 0 })
    run(person, 0, 0, 0, 0.08)
    // Shot from ahead, the head snaps back (+Z, as it faces -Z).
    expect(head().z - still.z).toBeGreaterThan(0.03)
    run(person, 0, 0, 0, 1.5)
    expect(head().distanceTo(still)).toBeLessThan(0.03)
    c.dispose()
  })

  it('pins a wound to the bone it is on, so it moves with the body', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const rootOf = (who: Character) => (who as unknown as { object: import('@ragnarok/engine').Node }).object
    const stand = (x: number, yaw: number) => {
      for (let i = 0; i < 10; i++) person.update([x, 1, 0], yaw, 0, 0, true, 1 / 60)
    }
    stand(0, 0)
    rootOf(person).updateWorldMatrix(true, true)
    const shin = rootOf(person).getObjectByName('shin_L')!
    const knee = shin.getWorldPosition(new Vector3())
    const spot = knee.clone().add(new Vector3(0, -0.1, 0))
    const wound = person.pin(spot)
    expect(wound().distanceTo(spot)).toBeLessThan(1e-3)
    // Walked 5 m on, the wound is 5 m on.
    stand(5, 0)
    const moved = wound()
    expect(moved.x - spot.x).toBeCloseTo(5, 1)
    expect(moved.y).toBeCloseTo(spot.y, 1)
    // Turned about, it goes round with the leg, still on the shin.
    stand(5, Math.PI)
    const turned = wound()
    const shinNow = shin.getWorldPosition(new Vector3())
    expect(turned.distanceTo(shinNow)).toBeCloseTo(spot.distanceTo(knee), 2)
    expect(Math.sign(turned.x - 5)).toBe(-Math.sign(spot.x))
    person.dispose()
    c.dispose()
  })
})

describe('Character losing limbs', () => {
  it('shrinks away a lost limb, keeps the gun in a hand with the right arm gone, and says what is new', async () => {
    const { LIMB_ARM_R, LIMB_BLEED_SHIFT, LIMB_LEG_L } = await import('@/net/codec')
    const c = await crowd()
    const person = new Character(c, 1)
    // The model it draws, which the character keeps to itself.
    const rootOf = (who: Character) => (who as unknown as { object: import('@ragnarok/engine').Node }).object
    person.held = ITEM_AK5
    run(person, 0, 0, 0, 0.1)
    expect(person.setLimbs(LIMB_LEG_L), 'the first word is not news: they may have lost it before they came into view').toBe(0)
    run(person, 0, 0, 0, 0.1)
    const thigh = rootOf(person).getObjectByName('thigh_L')!
    expect(thigh.scale.x).toBeLessThan(0.01)
    expect(rootOf(person).getObjectByName('thigh_R')!.scale.x).toBe(1)
    // The right arm goes, bleeding: that is new.
    expect(person.setLimbs(LIMB_LEG_L | LIMB_ARM_R | (LIMB_ARM_R << LIMB_BLEED_SHIFT))).toBe(LIMB_ARM_R)
    expect(person.limbs).toBe(LIMB_LEG_L | LIMB_ARM_R)
    run(person, 0, 0, 0, 0.1)
    const arm = rootOf(person).getObjectByName('upper_arm_R')!
    expect(arm.scale.x).toBeLessThan(0.01)
    rootOf(person).updateWorldMatrix(true, true)
    const hand = scaleOf(rootOf(person).getObjectByName('hand_R')!.matrixWorld)
    expect(hand / scaleOf(rootOf(person).matrixWorld)).toBeCloseTo(1, 1)
    // A burst leaves out the limbs already thrown.
    const keys = person.shatter().map((piece) => piece.key)
    expect(keys).not.toContain('thigh_L')
    expect(keys).not.toContain('forearm_R')
    expect(keys).toContain('thigh_R')
    // Respawned whole.
    person.dead = true
    person.dead = false
    expect(person.limbs).toBe(0)
    expect(thigh.scale.x).toBe(1)
    person.dispose()
    c.dispose()
  })
})
