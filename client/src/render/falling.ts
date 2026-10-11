import * as THREE from 'three'
import type { Tree } from '@/world'
import { removeTreeInstance, standaloneTree } from './trees'

/** Resting angle of a fallen tree from upright: a little short of flat, so the crown props it up. */
const LANDED = Math.PI / 2 - 0.08

interface Falling {
  pivot: THREE.Group
  axis: THREE.Vector3
  duration: number
  elapsed: number
}

/**
 * Trees that have been cut down. A felled tree leaves its tile's instanced mesh and becomes a
 * standalone mesh that tips over about its foot, slowly at first and then faster, and lies
 * where it landed until its tile unloads. Purely visual: the server only says which tree fell
 * and which way.
 */
export class FallingTrees {
  private readonly falling: Falling[] = []

  /**
   * Fells tree number `index` of `tile`, drawn in `group`. `yaw` is the way it falls (0 faces
   * -Z); `animate` false puts it straight on the ground, for a tile that streams in after the fact.
   * Returns false if it was not standing in `group`.
   */
  fell(group: THREE.Object3D, tree: Tree, index: number, yaw: number, animate: boolean): boolean {
    if (!removeTreeInstance(group, index)) return false
    const { pivot } = standaloneTree(tree)
    group.add(pivot)
    // Where it falls to: yaw 0 faces -Z, so the direction is (-sin, -cos). Tipping over about
    // up × direction takes the top that way.
    const dx = -Math.sin(yaw)
    const dz = -Math.cos(yaw)
    const axis = new THREE.Vector3(dz, 0, -dx)
    if (!animate) {
      pivot.quaternion.setFromAxisAngle(axis, LANDED)
      return true
    }
    // Taller trees take longer to come down.
    this.falling.push({ pivot, axis, duration: 0.9 + tree.height * 0.04, elapsed: 0 })
    return true
  }

  update(dt: number) {
    for (let i = this.falling.length - 1; i >= 0; i--) {
      const fall = this.falling[i]!
      fall.elapsed += dt
      const t = Math.min(1, fall.elapsed / fall.duration)
      // Gravity on a tipping rod: nearly still at first, then quickly.
      fall.pivot.quaternion.setFromAxisAngle(fall.axis, LANDED * t * t)
      if (t >= 1) this.falling.splice(i, 1)
    }
  }

  /** Forgets the animations in flight, for a tile group that is going away. */
  clear() {
    this.falling.length = 0
  }
}
