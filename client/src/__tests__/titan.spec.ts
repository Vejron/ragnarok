import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { beforeAll, describe, expect, it, vi } from 'vitest'
import { triangleCount } from './helpers'
import { Quaternion, Vector3 } from '@ragnarok/engine'
import { bend, headEye, headTurn, parseTitanModel, STEP_AIRBORNE, TitanModel } from '@/render/titan'
import { gazeOf } from '@/xr/headset'
import { TitanHands } from '@/titanhands'
import { fistsFromHands, PILOT_ARM, PILOT_SHOULDER_DOWN, PILOT_SHOULDER_OUT, reversedDepth } from '@/xr/hands'
import { kindView } from '@/kinds'
import { seatView } from '@/seats'
import { HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, HANDS_REACH_LEFT, HANDS_REACH_RIGHT, KIND_TITAN, THRUST_AIR, THRUST_LIFT, TITAN_LAMP } from '@/net/codec'
import { BUTTON_ALT, BUTTON_FIRE, BUTTON_RUN, TITAN_ELBOW_POLE, TITAN_EYE, TITAN_FIST_REST, TITAN_FOREARM_LENGTH, TITAN_HEAD_DOWN, TITAN_HEAD_PIVOT, TITAN_HEAD_TURN, TITAN_REACH, TITAN_UPPER_ARM_LENGTH, TITAN_RIDE_HEIGHT, TITAN_SHOULDER, TITAN_STEP_LIFT } from '@/sim/consts'
import { aimsReticle, seatHint } from '@/seathints'
import { vehicleDef } from '@/sim/vehicles'
import type { Titan } from '@/entities/units'

const IDENTITY: [number, number, number, number] = [0, 0, 0, 1]
const STANDING: [number, number, number] = [100, TITAN_RIDE_HEIGHT, -40]
/** Its limbs as the server sends them at rest: the fists hanging, the feet on the ground under the hips. */
const AT_REST: Titan = { lx: -TITAN_FIST_REST[0], ly: TITAN_FIST_REST[1], lz: TITAN_FIST_REST[2], rx: TITAN_FIST_REST[0], ry: TITAN_FIST_REST[1], rz: TITAN_FIST_REST[2], flx: -5, fly: -TITAN_RIDE_HEIGHT, flz: 0, frx: 5, fry: -TITAN_RIDE_HEIGHT, frz: 0, flags: 0, step: 0, weight: 0.5, head_yaw: 0, head_pitch: 0 }

beforeAll(async () => {
  const bytes = readFileSync(resolve(process.cwd(), 'public/models/titan.glb'))
  // A copy made here, so it is an ArrayBuffer of the realm the loader checks against.
  const copy = new Uint8Array(bytes.length)
  copy.set(bytes)
  await parseTitanModel(copy.buffer)
})

/** Where the node called `name` is in the world. */
function where(titan: TitanModel, name: string) {
  const node = titan.root.getObjectByName(name)!
  node.updateWorldMatrix(true, false)
  return new Vector3(node.matrixWorld[12]!, node.matrixWorld[13]!, node.matrixWorld[14]!)
}

describe('TitanModel', () => {
  it('is cheap enough to draw, is what its kind builds, and has a fist to move', () => {
    const titan = kindView(KIND_TITAN).model(1)
    expect(titan).toBeInstanceOf(TitanModel)
    // Its big plates are cut into a 1.2 m grid for the scars' dents to press in (build_titan.py's
    // DENT_GRID): about 23,000 triangles, which cost 0.06 ms more than the 3,000 of the plain boxes with
    // a Titan filling a 2268×1258 frame.
    expect(triangleCount(titan.root)).toBeLessThan(25_000)
    expect(vehicleDef(KIND_TITAN).fists).toBe(true)
    expect(vehicleDef(KIND_TITAN).board).not.toBeNull()
    titan.dispose()
  })

  /** The way the visor of `titan` faces in the world: the head's own -Z. */
  const visor = (titan: TitanModel) => {
    const head = titan.root.getObjectByName('head')!
    head.updateWorldMatrix(true, false)
    return new Vector3(-head.matrixWorld[8]!, -head.matrixWorld[9]!, -head.matrixWorld[10]!).normalize()
  }

  it('turns its head on its neck to where the server has it looking', () => {
    const titan = new TitanModel(0)
    titan.setLimbs({ ...AT_REST, head_yaw: 0.6, head_pitch: 0.3 })
    titan.update(STANDING, IDENTITY)
    // + yaw is to the left (-X), + pitch is up.
    const flat = Math.cos(0.3)
    expect(visor(titan).distanceTo(new Vector3(-Math.sin(0.6) * flat, Math.sin(0.3), -Math.cos(0.6) * flat))).toBeLessThan(1e-3)
    expect(where(titan, 'head').distanceTo(new Vector3(...TITAN_HEAD_PIVOT).add(new Vector3(...STANDING)))).toBeLessThan(1e-3)
    titan.dispose()
  })

  it('turns its own pilot\'s head to their look at once, as far as the neck allows, with the eye riding it', () => {
    const titan = new TitanModel(0)
    titan.setLimbs(AT_REST)
    // A hull facing 0.7 to the left, and a look 0.4 further left and 0.2 up.
    const turn = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), 0.7)
    const rotation: [number, number, number, number] = [turn.x, turn.y, turn.z, turn.w]
    const head = headTurn(1.1, 0.2, rotation)
    expect(head[0]).toBeCloseTo(0.4)
    expect(head[1]).toBeCloseTo(0.2)
    titan.lookTo(head, true)
    titan.update(STANDING, rotation)
    expect(visor(titan).distanceTo(new Vector3(-Math.sin(1.1) * Math.cos(0.2), Math.sin(0.2), -Math.cos(1.1) * Math.cos(0.2)))).toBeLessThan(1e-3)
    // Looking behind and at its feet, the head stops where the neck does.
    expect(headTurn(0.7 + 2.5, -1.5, rotation)).toEqual([TITAN_HEAD_TURN, -TITAN_HEAD_DOWN])
    // The eye is at the visor when the head is straight, and swings round the neck with it.
    expect(headEye([0, 0]).distanceTo(new Vector3(...TITAN_EYE))).toBeLessThan(1e-6)
    const eye = headEye([Math.PI / 2, 0])
    expect(eye.distanceTo(new Vector3(TITAN_EYE[2], TITAN_EYE[1], 0))).toBeLessThan(1e-6)
    // With no word from the pilot the next frame, it is the server's again.
    titan.update(STANDING, rotation)
    expect(visor(titan).distanceTo(new Vector3(0, 0, -1).applyQuaternion(turn))).toBeLessThan(1e-3)
    titan.dispose()
  })

  it('shines its searchlight from its visor the way its head faces, when it is switched on', () => {
    const titan = new TitanModel(0)
    titan.setLimbs({ ...AT_REST, head_yaw: 0.6, head_pitch: -0.4 })
    titan.update(STANDING, IDENTITY)
    expect(titan.searchlight()).toBeNull()
    titan.setLimbs({ ...AT_REST, head_yaw: 0.6, head_pitch: -0.4, flags: TITAN_LAMP })
    titan.update(STANDING, IDENTITY)
    const beam = titan.searchlight()!
    expect(beam.direction.distanceTo(visor(titan))).toBeLessThan(1e-6)
    // It starts within a couple of metres of the pilot's eye, ahead of it.
    const eye = headEye([0.6, -0.4]).add(new Vector3(...STANDING))
    const out = beam.at.clone().sub(eye)
    expect(out.length()).toBeLessThan(2)
    expect(out.dot(beam.direction)).toBeGreaterThan(0.5)
    titan.dispose()
  })

  it('reads which way a headset looks from its orientation', () => {
    const left = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), 0.5)
    const up = new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), 0.3)
    const [yaw, pitch] = gazeOf(left.clone().multiply(up))
    expect(yaw).toBeCloseTo(0.5)
    expect(pitch).toBeCloseTo(0.3)
  })

  it('puts its fists where the server has them, turned and moved with its hull', () => {
    const titan = new TitanModel(0)
    const turn = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), 0.7)
    const rotation: [number, number, number, number] = [turn.x, turn.y, turn.z, turn.w]
    const limbs: Titan = { ...AT_REST, rx: 8.5, ry: 2, rz: -20, flags: HANDS_GRIP_RIGHT }
    titan.setLimbs(limbs)
    titan.update(STANDING, rotation)
    const expected = new Vector3(8.5, 2, -20).applyQuaternion(turn).add(new Vector3(...STANDING))
    expect(titan.fistWorld(1).distanceTo(expected)).toBeLessThan(1e-3)
    expect(where(titan, 'fist_R').distanceTo(expected)).toBeLessThan(1e-3)
    // The arm between is whole: shoulder to elbow and elbow to fist are each as long as built.
    const shoulder = where(titan, 'upper_R')
    const elbow = where(titan, 'fore_R')
    expect(shoulder.distanceTo(new Vector3(...TITAN_SHOULDER).applyQuaternion(turn).add(new Vector3(...STANDING)))).toBeLessThan(1e-3)
    expect(shoulder.distanceTo(elbow)).toBeCloseTo(12, 3)
    expect(elbow.distanceTo(expected)).toBeCloseTo(12, 3)
    // The closed hand shows its curled fingers, the open one its straight ones.
    expect(titan.root.getObjectByName('closed_R')!.visible).toBe(true)
    expect(titan.root.getObjectByName('open_R')!.visible).toBe(false)
    expect(titan.root.getObjectByName('open_L')!.visible).toBe(true)
    titan.dispose()
  })

  it('stands on the feet the server has planted, and thuds when one comes down', () => {
    const titan = new TitanModel(0)
    titan.setLimbs(AT_REST)
    titan.update(STANDING, IDENTITY)
    // The ankle is over the sole, and the knee is bent forward of the line from hip to ankle.
    const ankle = where(titan, 'foot_L')
    expect(ankle.x).toBeCloseTo(STANDING[0] - 5, 3)
    expect(ankle.y).toBeCloseTo(1.5, 3)
    expect(where(titan, 'shin_L').z).toBeLessThan(STANDING[2] - 1)
    expect(titan.takeSteps()).toEqual([])
    // The left foot swings ahead, lifted in the middle of its step; the right stays where it is.
    titan.setLimbs({ ...AT_REST, flz: -4, step: 1 + 50 })
    titan.update(STANDING, IDENTITY)
    expect(where(titan, 'foot_L').y).toBeGreaterThan(1.5 + 0.9 * TITAN_STEP_LIFT)
    expect(where(titan, 'foot_R').y).toBeCloseTo(1.5, 3)
    expect(titan.takeSteps()).toEqual([])
    // It comes down: that is a footfall, where it stands.
    titan.setLimbs({ ...AT_REST, flz: -8, step: 202 })
    titan.update(STANDING, IDENTITY)
    const steps = titan.takeSteps()
    expect(steps).toHaveLength(1)
    expect(steps[0]!.at[2]).toBeCloseTo(STANDING[2] - 8, 3)
    expect(where(titan, 'foot_L').y).toBeCloseTo(1.5, 3)
    titan.dispose()
  })

  it('keeps a planted foot pointing the way it was put down while the hull turns over it', () => {
    const titan = new TitanModel(0)
    titan.setLimbs(AT_REST)
    titan.update(STANDING, IDENTITY)
    const pointing = (name: string) => new Vector3(0, 0, -1).applyQuaternion(titan.root.getObjectByName(name)!.getWorldQuaternion(new Quaternion()))
    // The hull turns a quarter to the left with the right foot in the air: the left still points north.
    const turned = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.PI / 2)
    titan.setLimbs({ ...AT_REST, step: 101 + 99 })
    titan.update(STANDING, [turned.x, turned.y, turned.z, turned.w])
    expect(pointing('foot_L').z).toBeCloseTo(-1, 3)
    expect(pointing('foot_R').x).toBeCloseTo(-1, 3)
    titan.dispose()
  })

  it('burns under both feet while its thrusters fire, and reports a hard landing once', () => {
    const titan = new TitanModel(0)
    let clock = 1000
    const now = vi.spyOn(performance, 'now').mockImplementation(() => clock)
    const hanging = TITAN_RIDE_HEIGHT - 2
    const up = (height: number): [number, number, number] => [STANDING[0], STANDING[1] + height, STANDING[2]]
    titan.setLimbs({ ...AT_REST, fly: -hanging, fry: -hanging, step: STEP_AIRBORNE })
    titan.update(up(60), IDENTITY)
    expect(titan.jets(0)).toEqual([])
    expect(titan.jets(THRUST_AIR)).toEqual([])
    const jets = titan.jets(THRUST_AIR | THRUST_LIFT)
    expect(jets).toHaveLength(2)
    // One under each foot's sole, blowing down.
    expect(jets[0]!.at.x).toBeCloseTo(STANDING[0] - 5, 3)
    expect(jets[1]!.at.x).toBeCloseTo(STANDING[0] + 5, 3)
    expect(jets[0]!.at.y).toBeCloseTo(STANDING[1] + 60 - hanging, 3)
    expect(jets[0]!.direction.y).toBe(-1)
    // It comes down at 30 m/s and its feet meet the ground.
    for (let frame = 1; frame <= 4; frame++) {
      clock += 100
      titan.update(up(60 - 3 * frame), IDENTITY)
      expect(titan.takeSlam()).toBeNull()
    }
    clock += 100
    titan.setLimbs(AT_REST)
    titan.update(STANDING, IDENTITY)
    const slam = titan.takeSlam()
    expect(slam!.speed).toBeCloseTo(30, 3)
    expect(slam!.at).toEqual([STANDING[0], 0, STANDING[2]])
    expect(titan.takeSlam()).toBeNull()
    expect(seatView(KIND_TITAN, 0).gauges).toEqual(['air', 'heat', 'ammo', 'tubes'])
    now.mockRestore()
    titan.dispose()
  })

  it('hangs its arms until the server says where they are', () => {
    const titan = new TitanModel(0)
    titan.update(STANDING, IDENTITY)
    expect(titan.fistWorld(0).distanceTo(new Vector3(STANDING[0] - TITAN_FIST_REST[0], STANDING[1] + TITAN_FIST_REST[1], STANDING[2] + TITAN_FIST_REST[2]))).toBeLessThan(1e-3)
    titan.dispose()
  })
})

describe('bend', () => {
  it('brings a target out of reach to the end of the arm', () => {
    const [joint, end] = [new Vector3(), new Vector3()]
    bend(new Vector3(0, 0, 0), new Vector3(0, 0, -100), 12, 12, new Vector3(0, -1, 0), joint, end)
    expect(end.z).toBeCloseTo(-23.99, 2)
    expect(joint.distanceTo(new Vector3())).toBeCloseTo(12, 3)
    expect(joint.distanceTo(end)).toBeCloseTo(12, 3)
    expect(joint.y).toBeLessThan(0)
  })
})

describe('a Titan under mouse and keyboard', () => {
  const HULL: [number, number, number] = [0, TITAN_RIDE_HEIGHT, 0]
  const aim = new Vector3(3, TITAN_RIDE_HEIGHT - 5, -60)
  const fist = (hands: ReturnType<TitanHands['step']>, hand: number) => (hand === 0 ? new Vector3(hands.lx, hands.ly, hands.lz) : new Vector3(hands.rx, hands.ry, hands.rz)).divideScalar(100)
  const shoulder = (hand: number) => new Vector3((hand === 0 ? -1 : 1) * TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2])

  it('holds both fists up in a guard, open, towards what it looks at', () => {
    const hands = new TitanHands().step(0, aim, HULL, IDENTITY)
    expect(hands.flags).toBe(HANDS_REACH_LEFT | HANDS_REACH_RIGHT)
    // The left close in; the right, with the gun, held out to lay it.
    expect(fist(hands, 0).distanceTo(shoulder(0))).toBeLessThan(TITAN_REACH * 0.6)
    for (const hand of [0, 1]) expect(fist(hands, hand).z).toBeLessThan(-5)
  })

  it('throws a closed fist at the point on a click, and brings it back when it is let go', () => {
    const controller = new TitanHands()
    controller.step(0, aim, HULL, IDENTITY)
    const punch = controller.step(BUTTON_FIRE, aim, HULL, IDENTITY)
    expect(punch.flags & HANDS_GRIP_LEFT).not.toBe(0)
    expect(punch.flags & HANDS_GRIP_RIGHT).toBe(0)
    expect(fist(punch, 0).distanceTo(shoulder(0))).toBeGreaterThan(TITAN_REACH)
    // Along the line from the shoulder to the point.
    const line = new Vector3(aim.x, aim.y - HULL[1], aim.z).sub(shoulder(0)).normalize()
    expect(fist(punch, 0).sub(shoulder(0)).normalize().dot(line)).toBeGreaterThan(0.999)
    const after = controller.step(0, aim, HULL, IDENTITY)
    expect(after.flags & HANDS_GRIP_LEFT).toBe(0)
    expect(fist(after, 0).distanceTo(shoulder(0))).toBeLessThan(TITAN_REACH * 0.6)
  })

  it('reaches out open-handed on Shift, closes a hand on a click, and keeps hold when Shift is let go', () => {
    const controller = new TitanHands()
    // A point within the arms' reach: on a roof beside it, say.
    const near = new Vector3(8, TITAN_RIDE_HEIGHT - 8, -14)
    const reach = controller.step(BUTTON_RUN, near, HULL, IDENTITY)
    expect(reach.flags).toBe(HANDS_REACH_LEFT | HANDS_REACH_RIGHT)
    // One hand either side of the point, clear of it.
    const local = new Vector3(near.x, near.y - HULL[1], near.z)
    expect(fist(reach, 0).x).toBeLessThan(local.x - 2.5)
    expect(fist(reach, 1).x).toBeGreaterThan(local.x + 2.5)
    expect(fist(reach, 0).distanceTo(local)).toBeLessThan(4)
    // The left hand grips: the right holds the gun.
    const grip = controller.step(BUTTON_RUN | BUTTON_FIRE, near, HULL, IDENTITY)
    expect(grip.flags & HANDS_GRIP_LEFT).not.toBe(0)
    expect(fist(grip, 0).distanceTo(fist(reach, 0))).toBeLessThan(1e-6)
    const carry = controller.step(BUTTON_FIRE, near, HULL, IDENTITY)
    expect(carry.flags & HANDS_GRIP_LEFT).not.toBe(0)
    expect(fist(carry, 0).distanceTo(shoulder(0))).toBeLessThan(TITAN_REACH * 0.6)
    expect(controller.step(0, near, HULL, IDENTITY).flags & HANDS_GRIP_LEFT).toBe(0)
  })

  it('holds the gun fist up with its forearm laid on the point, open, and keeps it there while it fires', () => {
    const controller = new TitanHands()
    const local = new Vector3(aim.x, aim.y - HULL[1], aim.z)
    const pole = new Vector3(TITAN_ELBOW_POLE[0], TITAN_ELBOW_POLE[1], TITAN_ELBOW_POLE[2])
    for (const buttons of [0, BUTTON_ALT]) {
      const hands = controller.step(buttons, aim, HULL, IDENTITY)
      expect(hands.flags & HANDS_GRIP_RIGHT).toBe(0)
      // The forearm as the model bends it points at the point.
      const joint = new Vector3()
      const end = new Vector3()
      bend(shoulder(1), fist(hands, 1), TITAN_UPPER_ARM_LENGTH, TITAN_FOREARM_LENGTH, pole, joint, end)
      const along = end.clone().sub(joint).normalize()
      expect(along.dot(local.clone().sub(end).normalize())).toBeGreaterThan(0.995)
      expect(fist(hands, 1).distanceTo(shoulder(1))).toBeLessThan(TITAN_REACH)
    }
  })

  it('throws the gun fist as a punch on Shift and a right click, and never grips with it', () => {
    const controller = new TitanHands()
    controller.step(BUTTON_RUN, aim, HULL, IDENTITY)
    const punch = controller.step(BUTTON_RUN | BUTTON_ALT, aim, HULL, IDENTITY)
    expect(punch.flags & HANDS_GRIP_RIGHT).not.toBe(0)
    expect(fist(punch, 1).distanceTo(shoulder(1))).toBeGreaterThan(TITAN_REACH)
    const near = new Vector3(8, TITAN_RIDE_HEIGHT - 8, -14)
    controller.step(BUTTON_RUN, near, HULL, IDENTITY)
    expect(controller.step(BUTTON_RUN, near, HULL, IDENTITY).flags & HANDS_GRIP_RIGHT).toBe(0)
  })

  it('keeps a fist at its side when the look is behind it', () => {
    const hands = new TitanHands().step(BUTTON_FIRE, new Vector3(0, TITAN_RIDE_HEIGHT, 80), HULL, IDENTITY)
    expect(fist(hands, 0).z).toBeLessThan(0)
  })

  it('has a seat that aims with the reticle, and a line of keys', () => {
    const seat = seatView(KIND_TITAN, 0)
    expect(seat.rig.name).toBe('titan')
    expect(aimsReticle(seat.role)).toBe(true)
    const hint = seatHint(seat.role, { ammo: 'AP', reload: 1, range: 0, canSwitchSeat: false, jammed: false, heat: 0, tubes: { loaded: 3, clear: true } })
    expect(hint).toContain('punch')
    expect(hint).toContain('cannon')
    expect(hint).toContain('3 missiles ready')
    expect(seat.gauges).toContain('tubes')
  })
})

describe('a Titan under a VR pilot\'s hands', () => {
  const HEAD: [number, number, number] = [0.05, -0.02, 0.1]
  const shoulderOf = (hand: number): [number, number, number] => [HEAD[0] + (hand === 0 ? -1 : 1) * PILOT_SHOULDER_OUT, HEAD[1] - PILOT_SHOULDER_DOWN, HEAD[2]]
  const at = (hand: number, offset: [number, number, number]): [number, number, number] => {
    const s = shoulderOf(hand)
    return [s[0] + offset[0], s[1] + offset[1], s[2] + offset[2]]
  }
  const fist = (hands: ReturnType<typeof fistsFromHands>, hand: number) => (hand === 0 ? new Vector3(hands.lx, hands.ly, hands.lz) : new Vector3(hands.rx, hands.ry, hands.rz)).divideScalar(100)

  it('moves each fist from the Titan\'s shoulder as the hand is from the pilot\'s, an arm for an arm', () => {
    const hands = fistsFromHands(HEAD, [{ at: at(0, [0, -PILOT_ARM, 0]), closed: false }, { at: at(1, [0, 0, -PILOT_ARM]), closed: true }], 0, IDENTITY)
    expect(hands.flags).toBe(HANDS_REACH_LEFT | HANDS_REACH_RIGHT | HANDS_GRIP_RIGHT)
    // The left hand hangs at the pilot's side: the fist hangs a full arm below the Titan's shoulder.
    expect(fist(hands, 0).distanceTo(new Vector3(-TITAN_SHOULDER[0], TITAN_SHOULDER[1] - TITAN_REACH, TITAN_SHOULDER[2]))).toBeLessThan(0.02)
    // The right is held straight out ahead, closed: the fist is at the end of its reach ahead.
    expect(fist(hands, 1).distanceTo(new Vector3(TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2] - TITAN_REACH))).toBeLessThan(0.02)
  })

  it('lets a hand it cannot see hang, and asks in the hull\'s frame however the Titan is turned or leans', () => {
    const turned = new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), 1.1)
    const hull: [number, number, number, number] = [turned.x, turned.y, turned.z, turned.w]
    // The cockpit faces the way the hull does: straight ahead of the pilot is straight ahead of the Titan.
    const hands = fistsFromHands(HEAD, [{ at: null, closed: true }, { at: at(1, [0, 0, -PILOT_ARM]), closed: false }], 1.1, hull)
    expect(hands.flags).toBe(HANDS_REACH_RIGHT)
    expect(fist(hands, 1).distanceTo(new Vector3(TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2] - TITAN_REACH))).toBeLessThan(0.02)
    // Leaning forward, the hull's frame tips while the cockpit stays level: level ahead is up and ahead in it.
    const leaning = turned.clone().multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), -0.3))
    const lean = fistsFromHands(HEAD, [{ at: null, closed: false }, { at: at(1, [0, 0, -PILOT_ARM]), closed: false }], 1.1, [leaning.x, leaning.y, leaning.z, leaning.w])
    expect(fist(lean, 1).y).toBeGreaterThan(TITAN_SHOULDER[1] + 5)
  })

  it('turns a headset\'s projection to the engine\'s reversed depth', () => {
    // A symmetric lens, depth 0 at the near plane (0.1 m) and 1 at the far (1000 m).
    const [near, far] = [0.1, 1000]
    const projection = [1, 0, 0, 0, 0, 1.5, 0, 0, 0.1, -0.05, far / (near - far), -1, 0, 0, (near * far) / (near - far), 0]
    const lens = reversedDepth(projection)
    const depth = (z: number) => (lens[10]! * z + lens[14]!) / (lens[11]! * z + lens[15]!)
    expect(depth(-near)).toBeCloseTo(1, 5)
    expect(depth(-far)).toBeCloseTo(0, 5)
    // Nothing else of it changes: the off-centre terms a headset's eye has are kept.
    expect([...lens.slice(0, 10)]).toEqual([...Float32Array.from(projection).slice(0, 10)])
  })
})
