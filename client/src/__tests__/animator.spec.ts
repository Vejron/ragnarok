import { describe, expect, it } from 'vitest'
import { AnimationClip, Animator, Group, sampleTrack, type Track } from '@ragnarok/engine'

/** A stream of numbers in 0..1 that is the same every run. */
function random(seed: number) {
  return () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296)
}

const BONES = ['hips', 'spine', 'head', 'arm']

/** A rig of `BONES` in a chain, each a little off its parent. */
function rig() {
  const root = new Group()
  let parent = root
  for (const [i, name] of BONES.entries()) {
    const bone = new Group()
    bone.name = name
    bone.position.set(0.1 * i, 0.2, -0.05 * i)
    bone.quaternion.set(0, Math.sin(0.1 * i), 0, Math.cos(0.1 * i))
    parent.add(bone)
    parent = bone
  }
  return root
}

/** `count` clips of three keys, each turning and moving some of the bones, rotations in either hemisphere. */
function clips(count: number, next: () => number) {
  return Array.from({ length: count }, (_, c) => {
    const tracks: Track[] = []
    for (const node of BONES) {
      if (next() < 0.25) continue
      const turns = new Float32Array(12)
      for (let key = 0; key < 3; key++) {
        const q = [next() - 0.5, next() - 0.5, next() - 0.5, next() - 0.5]
        const length = Math.hypot(...q)
        for (let k = 0; k < 4; k++) turns[key * 4 + k] = q[k]! / length
      }
      tracks.push({ node, path: 'rotation', interpolation: 'LINEAR', times: new Float32Array([0, 0.5, 1]), values: turns })
      if (next() < 0.5) tracks.push({ node, path: 'translation', interpolation: 'LINEAR', times: new Float32Array([0, 0.5, 1]), values: Float32Array.from({ length: 9 }, () => next() - 0.5) })
    }
    return new AnimationClip(`clip${c}`, tracks, 1)
  })
}

/**
 * The pose worked out a property at a time, every clip's track looked at whether it plays or not: what
 * the animator did before it sampled only the clips that count.
 */
function expected(list: AnimationClip[], animator: Animator, rest: Map<string, number[]>) {
  const out = new Map<string, number[]>()
  const sample = new Float32Array(4)
  for (const node of BONES) {
    for (const path of ['rotation', 'translation'] as const) {
      const size = path === 'rotation' ? 4 : 3
      const sum = new Float32Array(4)
      let total = 0
      let driven = false
      for (const clip of list) {
        const action = animator.action(clip.name)!
        for (const track of clip.tracks) {
          if (track.node !== node || track.path !== path) continue
          driven = true
          if (!action.enabled || action.weight <= 0) continue
          sampleTrack(track, action.time, sample)
          if (size === 4 && total > 0 && sum[0]! * sample[0]! + sum[1]! * sample[1]! + sum[2]! * sample[2]! + sum[3]! * sample[3]! < 0) for (let k = 0; k < 4; k++) sample[k] = -sample[k]!
          for (let k = 0; k < size; k++) sum[k] = sum[k]! + sample[k]! * action.weight
          total += action.weight
        }
      }
      if (!driven) continue
      if (total < 1) {
        const r = new Float32Array(rest.get(`${node}|${path}`)!)
        const w = 1 - total
        const sign = size === 4 && total > 0 && sum[0]! * r[0]! + sum[1]! * r[1]! + sum[2]! * r[2]! + sum[3]! * r[3]! < 0 ? -1 : 1
        for (let k = 0; k < size; k++) sum[k] = sum[k]! + r[k]! * w * sign
        total = 1
      }
      const scale = size === 4 ? Math.hypot(sum[0]!, sum[1]!, sum[2]!, sum[3]!) || 1 : total
      out.set(`${node}|${path}`, Array.from(sum.subarray(0, size), (v) => v / scale))
    }
  }
  return out
}

describe('Animator', () => {
  it('poses the bones as the weighted mix of the clips that play, whatever the others hold', () => {
    const next = random(11)
    const root = rig()
    const rest = new Map<string, number[]>()
    for (const name of BONES) {
      const bone = root.getObjectByName(name)!
      rest.set(`${name}|rotation`, [bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w])
      rest.set(`${name}|translation`, [bone.position.x, bone.position.y, bone.position.z])
    }
    const list = clips(40, next)
    const animator = new Animator(root, list)
    for (let round = 0; round < 50; round++) {
      // A handful playing, at weights that sum to one, to less, or to nothing at all; the rest hold a
      // time and sometimes a weight they are disabled with.
      const share = round % 5 === 0 ? 0 : round % 3 === 0 ? 0.6 : 1
      const playing = new Set(Array.from({ length: 1 + Math.floor(next() * 4) }, () => Math.floor(next() * list.length)))
      for (const [i, clip] of list.entries()) {
        const action = animator.action(clip.name)!
        action.time = next()
        action.enabled = playing.has(i)
        action.weight = playing.has(i) ? share / playing.size : next() < 0.3 ? 0.5 : 0
      }
      animator.apply()
      for (const [key, want] of expected(list, animator, rest)) {
        const [name, path] = key.split('|') as [string, string]
        const bone = root.getObjectByName(name)!
        const got = path === 'rotation' ? [bone.quaternion.x, bone.quaternion.y, bone.quaternion.z, bone.quaternion.w] : [bone.position.x, bone.position.y, bone.position.z]
        for (let k = 0; k < want.length; k++) expect(got[k]).toBeCloseTo(want[k]!, 6)
      }
    }
  })
})
