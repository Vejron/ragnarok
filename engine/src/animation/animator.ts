import type { Node } from '../scene/node'
import { sampleTrack, type AnimationClip, type Track, type TrackPath } from './clip'

/** One clip playing on an Animator: where it is, how fast it moves and how much it counts. */
export class Action {
  time = 0
  weight = 0
  /** Seconds of clip per second of `update`; 0 holds the time where it is set. */
  timeScale = 1
  /** 'once' stops at the last frame and holds it. */
  loop: 'repeat' | 'once' = 'repeat'
  /** Disabled actions are skipped whatever their weight. */
  enabled = true

  constructor(readonly clip: AnimationClip) {}
}

/** One animated property of one node, with the clips' tracks that drive it. */
interface Binding {
  node: Node
  path: TrackPath
  /** The property's value before any animation: what is left when weights add up to less than one. */
  rest: Float32Array
  tracks: { action: Action; track: Track }[]
}

/**
 * Blends clips onto a node tree, as three's AnimationMixer does for weights set by hand: each animated
 * property becomes the weighted sum of its clips' samples (rotations aligned and normalised), topped up
 * with its rest value when the weights sum to less than one. Tracks find their nodes by name under
 * `root`, so one set of clips drives every clone of a model.
 */
export class Animator {
  readonly actions = new Map<string, Action>()
  private readonly bindings: Binding[] = []
  private readonly sample = new Float32Array(4)
  private readonly sum = new Float32Array(4)

  constructor(readonly root: Node, clips: AnimationClip[]) {
    const nodes = new Map<string, Node>()
    root.traverse((node) => {
      if (node.name && !nodes.has(node.name)) nodes.set(node.name, node)
    })
    const byKey = new Map<string, Binding>()
    for (const clip of clips) {
      const action = new Action(clip)
      this.actions.set(clip.name, action)
      for (const track of clip.tracks) {
        const node = nodes.get(track.node)
        if (!node) continue
        const key = `${track.node}|${track.path}`
        let binding = byKey.get(key)
        if (!binding) {
          binding = { node, path: track.path, rest: read(node, track.path), tracks: [] }
          byKey.set(key, binding)
          this.bindings.push(binding)
        }
        binding.tracks.push({ action, track })
      }
    }
  }

  action(name: string): Action | undefined {
    return this.actions.get(name)
  }

  /** Advances every action by `dt` (times its time scale), then poses the nodes. */
  update(dt: number) {
    for (const action of this.actions.values()) {
      if (!action.enabled || !action.timeScale) continue
      const duration = action.clip.duration
      action.time += dt * action.timeScale
      if (action.loop === 'once') action.time = Math.min(Math.max(action.time, 0), duration)
      else if (duration > 0) action.time = ((action.time % duration) + duration) % duration
    }
    this.apply()
  }

  /** Poses the nodes from the actions' current times and weights, without advancing them. */
  apply() {
    const sample = this.sample
    const sum = this.sum
    for (const binding of this.bindings) {
      const size = binding.path === 'rotation' ? 4 : 3
      sum.fill(0)
      let total = 0
      for (const { action, track } of binding.tracks) {
        const w = action.weight
        if (!action.enabled || w <= 0) continue
        sampleTrack(track, action.time, sample)
        if (size === 4 && total > 0 && sum[0]! * sample[0]! + sum[1]! * sample[1]! + sum[2]! * sample[2]! + sum[3]! * sample[3]! < 0) {
          // The other hemisphere: the same rotation, but it would cancel out in the sum.
          for (let k = 0; k < 4; k++) sample[k] = -sample[k]!
        }
        for (let k = 0; k < size; k++) sum[k] = sum[k]! + sample[k]! * w
        total += w
      }
      if (total < 1) {
        const rest = binding.rest
        const w = 1 - total
        let sign = 1
        if (size === 4 && total > 0 && sum[0]! * rest[0]! + sum[1]! * rest[1]! + sum[2]! * rest[2]! + sum[3]! * rest[3]! < 0) sign = -1
        for (let k = 0; k < size; k++) sum[k] = sum[k]! + rest[k]! * w * sign
        total = 1
      }
      const node = binding.node
      if (size === 4) {
        const len = Math.hypot(sum[0]!, sum[1]!, sum[2]!, sum[3]!) || 1
        node.quaternion.set(sum[0]! / len, sum[1]! / len, sum[2]! / len, sum[3]! / len)
      } else {
        const v = binding.path === 'translation' ? node.position : node.scale
        v.set(sum[0]! / total, sum[1]! / total, sum[2]! / total)
      }
    }
  }
}

function read(node: Node, path: TrackPath): Float32Array {
  if (path === 'rotation') {
    const q = node.quaternion
    return new Float32Array([q.x, q.y, q.z, q.w])
  }
  const v = path === 'translation' ? node.position : node.scale
  return new Float32Array([v.x, v.y, v.z])
}
