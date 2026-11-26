import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Quaternion, Vector3 } from '@ragnarok/engine'
import { describe, expect, it } from 'vitest'
import { Character, Crowd, parseCharacter } from '@/render/character'
import { boxOf } from './helpers'
import { colorFor } from '@/profile'
import { LIMB_ARM_L } from '@/net/codec'
import { ITEM_AK5, ITEM_BANDAGE, ITEM_GRENADE, ITEM_NONE, ITEM_REVOLVER, fillsHands } from '@/sim/items'

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

describe('Character on skis', () => {
  const visible = (c: Crowd, name: string) => {
    let shown = false
    c.root.traverse((node) => {
      if (node.name === name) shown = node.visible
    })
    return shown
  }

  it('has the skis-with-a-gun clips in every stance', async () => {
    const c = await crowd()
    for (const prefix of ['', 'pistol_', 'hold_', 'shoulder_', 'free_']) {
      for (const name of ['glide', 'skate', 'climb', 'tuck', 'plow', 'air']) expect(c.asset!.clips.has(`${prefix}armed_${name}`), prefix + name).toBe(true)
    }
    c.dispose()
  })

  it('shows the gun and blends the armed clips with a gun in hand', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_AK5
    person.skis = true
    run(person, 0, 0, -6)
    expect(visible(c, 'weapon_ak5')).toBe(true)
    expect(person.blend.get('armed_glide')! + person.blend.get('armed_skate')!).toBeGreaterThan(0.8)
    expect(person.blend.get('ski_pole') ?? 0).toBeLessThan(0.05)
    c.dispose()
  })

  it('goes back to the poles with empty hands, gun put away', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_AK5
    person.skis = true
    person.held = ITEM_NONE
    run(person, 0, 0, -6)
    expect(visible(c, 'weapon_ak5')).toBe(false)
    expect(person.blend.get('armed_glide') ?? 0).toBeLessThan(0.05)
    c.dispose()
  })

  it('counts what shoots as filling the hands', () => {
    expect(fillsHands(ITEM_AK5)).toBe(true)
    expect(fillsHands(ITEM_REVOLVER)).toBe(true)
    expect(fillsHands(ITEM_GRENADE)).toBe(false)
    expect(fillsHands(ITEM_NONE)).toBe(false)
  })
})

describe('Character holding things', () => {
  it('has a clip set per stance and a mesh and muzzle for each weapon', async () => {
    const c = await crowd()
    for (const prefix of ['pistol_', 'hold_']) {
      for (const name of ['idle', 'jump_air', 'walk_fwd', 'run_left', 'crouch_idle', 'crouch_back', 'crawl_idle', 'crawl_fwd', 'slide', 'sit', 'car_out', 'car_out_r']) {
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
    for (const name of ['idle', 'jump_air', 'walk_fwd', 'walk_back', 'walk_left', 'walk_right', 'run_fwd', 'run_back', 'run_left', 'run_right', 'crouch_idle', 'crouch_fwd', 'crouch_back', 'crouch_left', 'crouch_right', 'crawl_idle', 'crawl_fwd', 'crawl_back', 'crawl_left', 'crawl_right', 'slide', 'swim_tread', 'swim_fwd', 'dive_idle', 'dive_fwd', 'zombie_idle', 'zombie_walk', 'zombie_claw', 'zombie_rise']) {
      expect(c.asset!.clips.has(name), name).toBe(true)
    }
    expect(c.asset!.scene.getObjectByName('muzzle')).toBeTruthy()
    c.dispose()
  })

  it('stands, shambles and claws in its own clips as a zombie, empty-handed, and crawls as a soldier does', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_NONE
    person.zombie = true
    run(person, 0, 0, 0, 1.5)
    expect(person.blend.get('zombie_idle')).toBeGreaterThan(0.8)
    run(person, 0, 0, -1.2, 1.5)
    expect(person.blend.get('zombie_walk')).toBeGreaterThan(0.8)
    expect(person.blend.get('free_walk_fwd') ?? 0).toBeLessThan(0.05)
    person.clawing = true
    run(person, 0, 0, 0, 0.3)
    expect(person.blend.get('zombie_claw')).toBeGreaterThan(0.5)
    // Still said to be clawing, it does not start over: the claw plays out once, then it stands again.
    run(person, 0, 0, 0, 1.2)
    expect(person.blend.get('zombie_claw') ?? 0).toBeLessThan(0.05)
    for (let k = 0; k < 90; k++) person.update([0, 0.3, 0], 0, 0, -1.5, true, 1 / 60, 0, false, false, true)
    expect(person.blend.get('free_crawl_fwd')).toBeGreaterThan(0.8)
    person.zombie = false
    run(person, 0, 0, 0, 1.5)
    expect(person.blend.get('free_idle')).toBeGreaterThan(0.8)
    c.dispose()
  })

  it('climbs out of the grave in one clip while the server says so, then stands as a zombie', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_NONE
    person.zombie = true
    person.graveRise = true
    run(person, 0, 0, 0, 0.1)
    expect(person.blend.get('zombie_rise')).toBe(1)
    // Still said to be rising, it does not start over, and walking does not take it out of the ground.
    person.graveRise = true
    run(person, 0, 0, -1.2, 2)
    expect(person.blend.get('zombie_rise')).toBe(1)
    person.graveRise = false
    run(person, 0, 0, 0, 1.5)
    expect(person.blend.get('zombie_idle')).toBeGreaterThan(0.8)
    expect(person.blend.get('zombie_rise') ?? 0).toBeLessThan(0.05)
    c.dispose()
  })

  it('treads water still, strokes on the move, and dives under the surface', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const swim = (mode: 0 | 1 | 2, vx: number, vy = 0) => {
      person.swim = mode
      for (let k = 0; k < 90; k++) person.update([0, 0, 0], 0, vx, 0, true, 1 / 60, 0, false, false, false, false, vy)
    }
    swim(1, 0)
    expect(person.blend.get('swim_tread')).toBeGreaterThan(0.8)
    swim(1, 2)
    expect(person.blend.get('swim_fwd')).toBeGreaterThan(0.8)
    swim(2, 0)
    expect(person.blend.get('dive_idle')).toBeGreaterThan(0.8)
    swim(2, 1.5, -1)
    expect(person.blend.get('dive_fwd')).toBeGreaterThan(0.8)
    expect(person.blend.get('jump_air') ?? 0).toBeLessThan(0.05)
    swim(0, 0)
    expect(person.blend.get('idle')).toBeGreaterThan(0.5)
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

  it('holds the pose it died in for its ragdoll to take, and hides one in a car', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    run(person, 0, 0, 0, 0.2)
    const head = () => {
      c.root.updateMatrixWorld(true)
      return c.root.getObjectByName('head')!.getWorldPosition(new Vector3()).y
    }
    const standing = head()
    person.dead = true
    run(person, 0, 0, 0, 1.5)
    // No clip lays it down: the ragdoll does (ragdoll.spec.ts).
    expect(head()).toBeCloseTo(standing, 2)
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

  it('climbs out of a car by its door from the seat to standing on the spot beside it', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    const seat = new Vector3(-0.38, 1.2, 0)
    const spot = new Vector3(-1.4, 0, 0.5)
    const upright = new Quaternion()
    const at = (step: number) => {
      // The first frame of a climb takes the step as it is; those after ease towards it.
      for (let t = 0; t < 1; t += 1 / 60) person.seat(seat, upright, 0, 0, 1 / 60, false, { step, side: -1, spot })
      c.root.updateMatrixWorld(true)
      return c.root.getObjectByName('head')!.getWorldPosition(new Vector3())
    }
    const start = at(0.01)
    expect(person.blend.get('car_out')!).toBeCloseTo(1, 3)
    expect(start.x).toBeCloseTo(seat.x, 1)
    expect(start.y).toBeLessThan(seat.y)
    const out = at(1)
    // Out over the spot (the head a hand's breadth off the feet, as standing), a head taller than sitting.
    expect(Math.hypot(out.x - spot.x, out.z - spot.z)).toBeLessThan(0.2)
    expect(out.y).toBeGreaterThan(start.y + 0.3)
    // Sitting still again: the sitting clip, back in the seat.
    for (let t = 0; t < 1; t += 1 / 60) person.seat(seat, upright, 0, 0, 1 / 60)
    expect(person.blend.get('sit')!).toBeGreaterThan(0.9)
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

describe('Character’s looks', () => {
  const BODIES = {
    soldier: ['body_soldier_m', 'body_soldier_f'],
    officer: ['body_officer_m', 'body_officer_f'],
    medic: ['body_medic_m', 'body_medic_f'],
    civilian: ['body_civilian_man', 'body_civilian_sleeves', 'body_civilian_suit', 'body_civilian_woman', 'body_civilian_casual', 'body_civilian_dress', 'body_civilian_tanktop'],
  }
  const ALL = Object.values(BODIES).flat()
  /** The model a character draws, which it keeps to itself. */
  const rootOf = (who: Character) => (who as unknown as { object: import('@ragnarok/engine').Node }).object
  /** The bodies it shows: the ones in its model, as those it does not wear are taken out of it. */
  const shown = (who: Character) => ALL.filter((name) => rootOf(who).getObjectByName(name)?.visible === true)
  /** The colour of its own material `name` (`outfit`, `gear` or `skin`), whether the body it wears has it or not. */
  const tint = (who: Character, name: 'outfit' | 'gear' | 'skin') => (who as unknown as Record<string, { color: { getHex(): number } }>)[name === 'gear' ? 'kit' : name]!.color.getHex()

  it('has every body, and the joints the game moves by name', async () => {
    const c = await crowd()
    for (const name of [...ALL, 'hips', 'spine', 'chest', 'neck', 'head', 'upper_arm_L', 'forearm_R', 'hand_R', 'fingers_L', 'thumb_R', 'thigh_L', 'shin_R', 'foot_L', 'muzzle']) expect(c.asset!.scene.getObjectByName(name), name).toBeTruthy()
    c.dispose()
  })

  it('shows one body of the look it wears, a uniform for an officer and a medic', async () => {
    const c = await crowd()
    for (let id = 1; id <= 6; id++) {
      const person = new Character(c, id)
      for (const look of ['soldier', 'officer', 'medic', 'civilian'] as const) {
        person.look = look
        expect(shown(person)).toEqual([person.dressed])
        expect(BODIES[look]).toContain(person.dressed)
      }
      person.dispose()
    }
    c.dispose()
  })

  it('dresses civilians in every body, with tops and skin of their own, the same every time', async () => {
    const c = await crowd()
    const bodies = new Set<string>()
    const tops = new Set<number>()
    const skins = new Set<number>()
    for (let id = 1; id <= 60; id++) {
      const person = new Character(c, id)
      person.look = 'civilian'
      bodies.add(person.dressed)
      tops.add(tint(person, 'outfit'))
      skins.add(tint(person, 'skin'))
      const again = new Character(c, id)
      again.look = 'civilian'
      expect(shown(again)).toEqual(shown(person))
      expect(tint(again, 'outfit')).toBe(tint(person, 'outfit'))
      person.dispose()
      again.dispose()
    }
    expect([...bodies].sort()).toEqual([...BODIES.civilian].sort())
    expect(tops.size).toBeGreaterThan(5)
    expect(skins.size).toBeGreaterThan(3)
    c.dispose()
  })

  it('wears the player’s colour on a soldier’s patches, but not on an officer’s uniform', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.setColor(0x123456)
    expect(tint(person, 'outfit')).toBe(0x123456)
    person.look = 'officer'
    person.setColor(0x654321)
    expect(tint(person, 'outfit')).toBe(0xffffff)
    person.look = 'soldier'
    expect(tint(person, 'outfit')).toBe(0x654321)
    c.dispose()
  })

  it('rises as a zombie in a civilian’s body, in rags with grey skin, and goes back to the uniform', async () => {
    const c = await crowd()
    const person = new Character(c, 5)
    const skin = tint(person, 'skin')
    person.zombie = true
    expect(BODIES.civilian).toContain(shown(person)[0])
    expect(tint(person, 'outfit')).not.toBe(colorFor(5))
    expect(tint(person, 'skin')).not.toBe(skin)
    person.zombie = false
    expect(BODIES.soldier).toContain(shown(person)[0])
    expect(tint(person, 'skin')).toBe(skin)
    c.dispose()
  })

  it('breaks up only the body it wears', async () => {
    const c = await crowd()
    const one = new Character(c, 1)
    one.held = ITEM_AK5
    run(one, 0, 0, 0, 0.1)
    const pieces = one.shatter()
    // Each limb comes off once for each of the materials it is made in, and no more: not from the bodies it does not wear.
    const perLimb = new Map<string, number>()
    for (const { key } of pieces) perLimb.set(key, (perLimb.get(key) ?? 0) + 1)
    expect([...perLimb.keys()].sort()).toEqual(['forearm_L', 'forearm_R', 'head', 'shin_L', 'shin_R', 'thigh_L', 'thigh_R', 'torso', 'upper_arm_L', 'upper_arm_R'])
    for (const count of perLimb.values()) expect(count).toBeLessThanOrEqual(3)
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

describe('Character’s footsteps', () => {
  /** The steps taken over `seconds` moving at (vx, vz), on the ground or not. */
  function steps(person: Character, vx: number, vz: number, seconds = 3, grounded = true) {
    person.takeSteps()
    const taken: number[] = []
    for (let t = 0; t < seconds; t += 1 / 60) {
      person.update([0, 0.85, 0], 0, vx, vz, grounded, 1 / 60)
      taken.push(...person.takeSteps())
    }
    return taken
  }

  it('puts a foot down twice a stride, harder running than walking, and none standing still', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    steps(person, 0, 0, 1)
    expect(steps(person, 0, 0)).toEqual([])
    steps(person, 0, -3.8, 1)
    const walking = steps(person, 0, -3.8, 4)
    steps(person, 0, -7.3, 1)
    const running = steps(person, 0, -7.3, 4)
    expect(walking.length).toBeGreaterThan(4)
    expect(running.length).toBeGreaterThan(walking.length)
    expect(Math.min(...running)).toBeGreaterThan(Math.max(...walking))
    c.dispose()
  })

  it('takes no steps in the air, on skis or swimming', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    steps(person, 0, -5, 1, false)
    expect(steps(person, 0, -5, 3, false)).toEqual([])
    person.skis = true
    steps(person, 0, -5, 1)
    expect(steps(person, 0, -5)).toEqual([])
    person.skis = false
    person.swim = 1
    steps(person, 0, -1.5, 1)
    expect(steps(person, 0, -1.5)).toEqual([])
    c.dispose()
  })
})

describe('Character’s runtime layer', () => {
  /** The world place of `name` among the crowd's bones. */
  const where = (c: Crowd, name: string) => {
    let found: Vector3 | null = null
    c.root.updateWorldMatrix(true, true)
    c.root.traverse((node) => {
      if (node.name === name && !found) found = node.getWorldPosition(new Vector3())
    })
    return found! as Vector3
  }
  /** Stands it still facing -Z with its capsule centre at (x, y, z), for `seconds`. */
  const stand = (person: Character, at: [number, number, number], seconds: number, pitch = 0) => {
    for (let t = 0; t < seconds; t += 1 / 60) person.update(at, 0, 0, 0, true, 1 / 60, pitch)
  }

  it('stands each foot on the slope under it and lowers the hips onto the lower one', async () => {
    const flat = await crowd()
    flat.groundAt = () => 0
    const level = new Character(flat, 1)
    stand(level, [0, 0.85, 0], 1)
    const left = where(flat, 'foot_L')
    const right = where(flat, 'foot_R')
    const hips = where(flat, 'hips')
    flat.dispose()

    // Rising 0.3 m per metre along +X, across the way it faces: one foot stands higher than the other.
    const slope = (x: number) => 0.3 * x
    const c = await crowd()
    c.groundAt = (x) => slope(x)
    const person = new Character(c, 1)
    stand(person, [0, 0.85, 0], 1)
    for (const [name, before] of [['foot_L', left], ['foot_R', right]] as const) {
      const after = where(c, name)
      // As high over its own ground as it stood over the flat.
      expect(after.y - slope(after.x), name).toBeCloseTo(before.y, 1)
      expect(Math.abs(after.y - slope(after.x) - before.y), name).toBeLessThan(0.02)
    }
    expect(where(c, 'hips').y).toBeLessThan(hips.y - 0.01)
    c.dispose()
  })

  it('keeps off a floor the terrain is far below', async () => {
    const c = await crowd()
    // A first floor 3 m over the ground: the terrain says nothing about where the feet stand.
    c.groundAt = (x) => -3 + 0.3 * x
    const person = new Character(c, 1)
    stand(person, [0, 0.85, 0], 1)
    const left = where(c, 'foot_L')
    const right = where(c, 'foot_R')
    expect(Math.abs(left.y - right.y)).toBeLessThan(0.01)
    c.dispose()
  })

  it('holds a planted foot still when the body shifts a little under it', async () => {
    const c = await crowd()
    c.groundAt = () => 0
    const person = new Character(c, 1)
    stand(person, [0, 0.85, 0], 1)
    const before = where(c, 'foot_L')
    // A small correction moves the body 6 cm; the planted feet stay where they were.
    stand(person, [0.06, 0.85, 0], 0.1)
    const after = where(c, 'foot_L')
    expect(Math.hypot(after.x - before.x, after.z - before.z)).toBeLessThan(0.01)
    c.dispose()
  })

  it('lets go of the feet at once when the body is moved a long way', async () => {
    const c = await crowd()
    c.groundAt = () => 0
    const person = new Character(c, 1)
    stand(person, [0, 0.85, 0], 1)
    // A respawn or a big correction: the feet go with the body, not back to where they stood.
    stand(person, [5, 0.85, 0], 1 / 60)
    const foot = where(c, 'foot_L')
    const hips = where(c, 'hips')
    expect(Math.hypot(foot.x - hips.x, foot.z - hips.z)).toBeLessThan(0.4)
    c.dispose()
  })

  it('keeps the left hand on the rifle’s grip as it aims up and flinches', async () => {
    const c = await crowd()
    c.groundAt = () => 0
    const person = new Character(c, 1)
    person.held = ITEM_AK5
    stand(person, [0, 0.85, 0], 1, 0.7)
    expect(where(c, 'hand_L').distanceTo(where(c, 'grip'))).toBeLessThan(0.005)
    person.flinch(LIMB_ARM_L, 0, 40)
    stand(person, [0, 0.85, 0], 0.05, 0.7)
    expect(where(c, 'hand_L').distanceTo(where(c, 'grip'))).toBeLessThan(0.005)
    c.dispose()
  })

  it('moves the body through a clip snapped in rather than jumping it', async () => {
    const c = await crowd()
    c.groundAt = () => 0
    const person = new Character(c, 1)
    person.zombie = true
    stand(person, [0, 0.85, 0], 1)
    const before = where(c, 'head')
    // Climbing out of the grave snaps every other clip out at once.
    person.graveRise = true
    stand(person, [0, 0.85, 0], 1 / 60)
    const jump = where(c, 'head').distanceTo(before)
    expect(jump).toBeLessThan(0.05)
    c.dispose()
  })
})

describe('Character landing', () => {
  /** Falls for a third of a second at 6 m/s, then lands moving at (vx, vz), and runs `after` seconds on the ground. */
  const fall = (person: Character, vx: number, vz: number, after: number) => {
    for (let t = 0; t < 0.5; t += 1 / 60) person.update([0, 0.85, 0], 0, vx, vz, true, 1 / 60)
    for (let t = 0; t < 0.33; t += 1 / 60) person.update([0, 2, 0], 0, vx, vz, false, 1 / 60, 0, false, false, false, false, -6)
    for (let t = 0; t < after; t += 1 / 60) person.update([0, 0.85, 0], 0, vx, vz, true, 1 / 60)
  }

  it('squats under a fall it lands standing, gun in hand, and stands up again', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_AK5
    fall(person, 0, 0, 0.2)
    expect(person.blend.get('jump_land')!).toBeGreaterThan(0.5)
    for (let t = 0; t < 1.5; t += 1 / 60) person.update([0, 0.85, 0], 0, 0, 0, true, 1 / 60)
    expect(person.blend.get('jump_land')!).toBeLessThan(0.02)
    expect(person.blend.get('idle')!).toBeGreaterThan(0.9)
    c.dispose()
  })

  it('runs on through a landing on the move', async () => {
    const c = await crowd()
    const person = new Character(c, 1)
    person.held = ITEM_AK5
    fall(person, 0, -4, 0.2)
    expect(person.blend.get('jump_land') ?? 0).toBeLessThan(0.05)
    c.dispose()
  })

  it('has a landing in every stance', async () => {
    const c = await crowd()
    for (const prefix of ['', 'pistol_', 'hold_', 'shoulder_', 'free_']) expect(c.asset!.clips.has(`${prefix}jump_land`), prefix).toBe(true)
    c.dispose()
  })
})
