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

/** One animated property of one node. */
interface Binding {
  node: Node
  path: TrackPath
  /** The property's value before any animation: what is left when weights add up to less than one. */
  rest: Float32Array
}

/** A clip's track, and which binding (by its place in `bindings`) it drives. */
interface Driven {
  slot: number
  rotation: boolean
  track: Track
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
  /** Every action with the tracks it drives, in the clips' order. */
  private readonly playing: { action: Action; tracks: Driven[] }[] = []
  private readonly sample = new Float32Array(4)
  /** The weighted sum of each binding's samples, four numbers a binding, and the weight summed into each. */
  private readonly sums: Float32Array
  private readonly totals: Float64Array

  constructor(readonly root: Node, clips: AnimationClip[]) {
    const nodes = new Map<string, Node>()
    root.traverse((node) => {
      if (node.name && !nodes.has(node.name)) nodes.set(node.name, node)
    })
    const byKey = new Map<string, number>()
    for (const clip of clips) {
      const action = new Action(clip)
      this.actions.set(clip.name, action)
      const tracks: Driven[] = []
      for (const track of clip.tracks) {
        const node = nodes.get(track.node)
        if (!node) continue
        const key = `${track.node}|${track.path}`
        let slot = byKey.get(key)
        if (slot === undefined) {
          slot = this.bindings.length
          byKey.set(key, slot)
          this.bindings.push({ node, path: track.path, rest: read(node, track.path) })
        }
        tracks.push({ slot, rotation: track.path === 'rotation', track })
      }
      this.playing.push({ action, tracks })
    }
    this.sums = new Float32Array(this.bindings.length * 4)
    this.totals = new Float64Array(this.bindings.length)
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

  /**
   * Poses the nodes from the actions' current times and weights, without advancing them. Only the
   * actions that count are sampled: a soldier has well over a hundred clips and a handful playing.
   */
  apply() {
    const sample = this.sample
    const sums = this.sums
    const totals = this.totals
    sums.fill(0)
    totals.fill(0)
    for (const { action, tracks } of this.playing) {
      const w = action.weight
      if (!action.enabled || w <= 0) continue
      const time = action.time
      for (const { slot, rotation, track } of tracks) {
        sampleTrack(track, time, sample)
        const at = slot * 4
        if (rotation) {
          if (totals[slot]! > 0 && sums[at]! * sample[0]! + sums[at + 1]! * sample[1]! + sums[at + 2]! * sample[2]! + sums[at + 3]! * sample[3]! < 0) {
            // The other hemisphere: the same rotation, but it would cancel out in the sum.
            for (let k = 0; k < 4; k++) sample[k] = -sample[k]!
          }
          sums[at + 3] = sums[at + 3]! + sample[3]! * w
        }
        sums[at] = sums[at]! + sample[0]! * w
        sums[at + 1] = sums[at + 1]! + sample[1]! * w
        sums[at + 2] = sums[at + 2]! + sample[2]! * w
        totals[slot] = totals[slot]! + w
      }
    }
    const bindings = this.bindings
    for (let slot = 0; slot < bindings.length; slot++) {
      const binding = bindings[slot]!
      const at = slot * 4
      const size = binding.path === 'rotation' ? 4 : 3
      let total = totals[slot]!
      if (total < 1) {
        const rest = binding.rest
        const w = 1 - total
        let sign = 1
        if (size === 4 && total > 0 && sums[at]! * rest[0]! + sums[at + 1]! * rest[1]! + sums[at + 2]! * rest[2]! + sums[at + 3]! * rest[3]! < 0) sign = -1
        for (let k = 0; k < size; k++) sums[at + k] = sums[at + k]! + rest[k]! * w * sign
        total = 1
      }
      const node = binding.node
      if (size === 4) {
        const len = Math.hypot(sums[at]!, sums[at + 1]!, sums[at + 2]!, sums[at + 3]!) || 1
        node.quaternion.set(sums[at]! / len, sums[at + 1]! / len, sums[at + 2]! / len, sums[at + 3]! / len)
      } else {
        const v = binding.path === 'translation' ? node.position : node.scale
        v.set(sums[at]! / total, sums[at + 1]! / total, sums[at + 2]! / total)
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
