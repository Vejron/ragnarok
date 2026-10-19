import { Group, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import type { Tree } from '@/world'
import type { Particles } from './particles'
import { removeTreeInstance, standaloneTree } from './trees'

/** Resting angle of a fallen tree from upright: a little short of flat, so the crown props it up. */
const LANDED = Math.PI / 2 - 0.08

/** Where a felled tree is heard: its foot as it cracks, its crown as it hits the ground. `size` is its height in metres. */
export interface TreeNoise {
  phase: 'hit' | 'crack' | 'thud'
  at: { x: number; y: number; z: number }
  size: number
}

interface Falling {
  pivot: Group
  axis: THREE.Vector3
  duration: number
  elapsed: number
  foot: THREE.Vector3
  /** Horizontal unit direction of the fall. */
  dx: number
  dz: number
  height: number
  crown: number
  /** 0..1: how violently it was hit (see `DestroyedObject.force`). */
  force: number
  /** Seconds it first gives, bending away from the blow, before it lets go. */
  give: number
  /** Metres per second it is thrown along its fall, and up. Zero unless the blow snapped it. */
  throwSpeed: number
  /** Splinters and the sound of the break, which wait for the end of the give. Null once done. */
  crack: ((fx: TreeFx) => void) | null
  noise: TreeNoise
  /** Seconds since it landed; null while still falling. */
  settled: number | null
}

/** How long a landed tree takes to stop bouncing. */
const SETTLE = 0.6
/** Most trees animating at once: past this, further ones just lie down, so a forest fire of a barrage stays cheap. */
const MAX_FALLING = 24
/** Above this force a trunk snaps instead of toppling: it is thrown clear, not just tipped. */
const SNAP = 0.5
/** How far a trunk bends (radians) before it lets go. */
const GIVE_ANGLE = 0.12
const WOOD = 0xa07040
const LEAF = 0x4f7f2a

/**
 * Trees that have been cut down. A felled tree leaves its tile's instanced mesh and becomes a
 * standalone mesh that tips over about its foot, slowly at first and then faster, and lies
 * where it landed until its tile unloads. Purely visual: the server only says which tree fell
 * and which way.
 */
export class FallingTrees {
  private readonly falling: Falling[] = []

  /** Told when a tree cracks and when it lands, for the sound of it. */
  onNoise: ((noise: TreeNoise) => void) | null = null

  /**
   * Fells tree number `index` of `tile`, drawn in `group`. `yaw` is the way it falls (0 faces
   * -Z); `animate` false puts it straight on the ground, for a tile that streams in after the fact.
   * `force` (0..1) is how hard it was hit: a soft push bends it before it goes and it comes down
   * slowly, a hard one makes it go at once and fast, and a harder one snaps it and throws it.
   * Returns false if it was not standing in `group`.
   */
  fell(group: Node, tree: Tree, index: number, yaw: number, animate: boolean, force = 0): boolean {
    if (!removeTreeInstance(group, index)) return false
    const { pivot } = standaloneTree(tree)
    group.add(pivot)
    // Where it falls to: yaw 0 faces -Z, so the direction is (-sin, -cos). Tipping over about
    // up × direction takes the top that way.
    const dx = -Math.sin(yaw)
    const dz = -Math.cos(yaw)
    const axis = new THREE.Vector3(dz, 0, -dx)
    if (!animate || this.falling.length >= MAX_FALLING) {
      pivot.quaternion.setFromAxisAngle(axis, LANDED)
      return true
    }
    // Taller trees take longer to come down.
    const foot = new THREE.Vector3(tree.x, tree.base, tree.z)
    // The harder the blow, the faster it comes down: a full-force hit takes a third of a second or so.
    const duration = Math.max(0.3, (0.6 + tree.height * 0.025) / (1 + 4 * force))
    // A soft push is felt first: the trunk gives for a moment. A hard one does not wait.
    const give = 0.3 * (1 - force) * (1 - force)
    const throwSpeed = force > SNAP ? (force - SNAP) * 2 * 9 : 0
    // The blow itself is heard the moment it lands, harder the harder it was.
    this.onNoise?.({ phase: 'hit', at: foot, size: tree.height * (0.5 + force) })
    this.falling.push({ pivot, axis, duration, elapsed: -give, foot, dx, dz, height: tree.height, crown: tree.crown, force, give, throwSpeed, settled: null,
      // The trunk splits: wood flies off the foot and the crown shakes its leaves loose.
      crack: (fx) => crack(fx, foot, dx, dz, tree.height, tree.crown, force),
      noise: { phase: 'crack', at: foot, size: tree.height * (1 + force) },
    })
    return true
  }

  update(dt: number, fx: TreeFx) {
    for (let i = this.falling.length - 1; i >= 0; i--) {
      const fall = this.falling[i]!
      fall.elapsed += dt
      if (fall.crack && fall.elapsed >= 0) {
        fall.crack(fx)
        this.onNoise?.(fall.noise)
        fall.crack = null
      }
      if (fall.elapsed < 0) {
        // Giving: it leans away from the blow, straining, a little more as the breaking point nears.
        const strain = 1 + fall.elapsed / fall.give
        fall.pivot.quaternion.setFromAxisAngle(fall.axis, GIVE_ANGLE * strain * strain)
        continue
      }
      const t = Math.min(1, fall.elapsed / fall.duration)
      if (t < 1) {
        // Gravity on a tipping rod: nearly still at first, then quickly; a hard blow already has it moving.
        const start = Math.min(0.9, fall.force * 1.2)
        const angle = GIVE_ANGLE * (1 - t) + LANDED * (start * t + (1 - start) * t * t)
        fall.pivot.quaternion.setFromAxisAngle(fall.axis, angle)
        if (fall.throwSpeed > 0) {
          // Snapped: the whole tree is thrown along its fall, up and back down to the ground.
          const s = fall.elapsed
          const rise = Math.max(0, fall.throwSpeed * 0.5 * s - 6 * s * s)
          fall.pivot.position.set(fall.foot.x + fall.dx * fall.throwSpeed * s, fall.foot.y + rise, fall.foot.z + fall.dz * fall.throwSpeed * s)
        }
        continue
      }
      if (fall.throwSpeed > 0 && fall.settled === null) {
        const s = fall.duration
        fall.pivot.position.set(fall.foot.x + fall.dx * fall.throwSpeed * s, fall.foot.y, fall.foot.z + fall.dz * fall.throwSpeed * s)
        fall.foot = new THREE.Vector3(fall.pivot.position.x, fall.pivot.position.y, fall.pivot.position.z)
      }
      if (fall.settled === null) {
        fall.settled = 0
        const crown = new THREE.Vector3(fall.foot.x + fall.dx * fall.height * 0.85, fall.foot.y, fall.foot.z + fall.dz * fall.height * 0.85)
        land(fx, crown, fall.dx, fall.dz, fall.height, fall.crown, fall.force)
        this.onNoise?.({ phase: 'thud', at: crown, size: fall.height })
      }
      // It bounces on the crown and settles: a quick, shrinking rebound.
      fall.settled += dt
      const k = Math.min(1, fall.settled / SETTLE)
      const bounce = 0.09 * Math.exp(-7 * k) * Math.abs(Math.sin(k * 14))
      fall.pivot.quaternion.setFromAxisAngle(fall.axis, LANDED - bounce * (1 - k))
      if (k >= 1) this.falling.splice(i, 1)
    }
  }

  /** Forgets the animations in flight, for a tile group that is going away. */
  clear() {
    this.falling.length = 0
  }
}

/** What the felling draws into: the game's alpha-blended GPU particles, which bounce on the ground. */
export interface TreeFx {
  smoke: Pick<Particles, 'spawn'>
}

const rand = (min: number, max: number) => min + Math.random() * (max - min)
const pick = <T>(items: readonly T[]) => items[Math.floor(Math.random() * items.length)]!

const WOOD_TONES = [0xd2ad74, 0xb98d55, 0x8a5a2b, 0x5a3b22] as const
const LEAF_TONES = [0x4f7f2a, 0x6a9a35, 0x3d6b22, 0x8aa63a, 0x2f5a1c] as const

/** A fleck of wood: a sliver that tumbles, stretched along its flight, bouncing where it lands. */
function chip(fx: TreeFx, at: { x: number; y: number; z: number }, velocity: readonly [number, number, number], size: number) {
  fx.smoke.spawn({
    position: at,
    velocity,
    life: rand(1.2, 2.4),
    size: [size, size * 0.8],
    color: pick(WOOD_TONES),
    alpha: 1,
    gravity: 12,
    drag: 0.3,
    bounce: true,
    spin: [-14, 14],
    stretch: 0.05,
  })
}

/** A leaf: small, light, falling slowly and sideways, lying where it lands. */
function leaf(fx: TreeFx, at: { x: number; y: number; z: number }, velocity: readonly [number, number, number]) {
  const tone = pick(LEAF_TONES)
  fx.smoke.spawn({
    position: at,
    velocity,
    life: rand(2.2, 4.2),
    size: [rand(0.14, 0.26), 0.12],
    color: tone,
    colorEnd: 0x6b5a2a,
    alpha: 1,
    gravity: 1.6,
    drag: 1.1,
    bounce: true,
    spin: [-6, 6],
  })
}

/** A billow of dust. */
function dust(fx: TreeFx, at: { x: number; y: number; z: number }, velocity: readonly [number, number, number], size: number, alpha = 0.45) {
  fx.smoke.spawn({ position: at, velocity, life: rand(1.4, 2.6), size: [size * 0.3, size], color: 0xb5a88c, alpha, gravity: -0.15, drag: 1.4, shape: 'puff' })
}

/** The trunk splits: chips and bark thrown the way the tree falls, a cloud at the foot, and leaves shaken loose from the crown. */
function crack(fx: TreeFx, foot: THREE.Vector3, dx: number, dz: number, height: number, crown: number, force: number) {
  const scale = Math.min(2, 0.6 + height / 14) * (1 + force * 0.5)
  const at = { x: foot.x, y: foot.y + 0.5, z: foot.z }
  const chips = Math.min(70, Math.round(16 + height + force * 36))
  for (let i = 0; i < chips; i++) {
    const out = rand(3, 8 + force * 8)
    chip(fx, at, [dx * out + rand(-2, 2), rand(2, 6 + force * 4), dz * out + rand(-2, 2)], rand(0.1, 0.22) * scale)
  }
  for (let i = 0; i < 5; i++) dust(fx, at, [dx * rand(0.5, 2) + rand(-1, 1), rand(0.3, 1.2), dz * rand(0.5, 2) + rand(-1, 1)], rand(1.5, 3) * scale)
  // The crown shudders: a first rain of leaves.
  const top = { x: foot.x, y: foot.y + height * 0.8, z: foot.z }
  const leaves = Math.min(60, Math.round(18 + crown * 5 + force * 20))
  for (let i = 0; i < leaves; i++) {
    const at = { x: top.x + rand(-crown, crown) * 0.5, y: top.y + rand(-1.5, 1.5), z: top.z + rand(-crown, crown) * 0.5 }
    leaf(fx, at, [rand(-1.5, 1.5) + dx * force * 3, rand(-0.5, 1.5), rand(-1.5, 1.5) + dz * force * 3])
  }
}

/** The crown hits the ground: dust rolls out along the ground, chips jump, and the crown bursts into leaves along the trunk. */
function land(fx: TreeFx, crown: THREE.Vector3, dx: number, dz: number, height: number, width: number, force: number) {
  const scale = Math.min(2, 0.6 + height / 14) * (1 + force * 0.5)
  const ground = { x: crown.x, y: crown.y + 0.3, z: crown.z }
  const puffs = Math.round(10 + scale * 4)
  for (let i = 0; i < puffs; i++) {
    const angle = (i / puffs) * Math.PI * 2 + rand(-0.2, 0.2)
    const speed = rand(2, 5) * scale
    dust(fx, ground, [Math.cos(angle) * speed, rand(0.2, 0.8), Math.sin(angle) * speed], rand(1.5, 3.2) * scale, 0.5)
  }
  for (let i = 0; i < 14; i++) chip(fx, ground, [rand(-4, 4), rand(2, 6), rand(-4, 4)], rand(0.08, 0.16) * scale)
  // The foliage lies along the last stretch of the trunk: it throws its leaves up and they come down around it.
  const leaves = Math.min(80, Math.round(30 + width * 8 + force * 20))
  for (let i = 0; i < leaves; i++) {
    const along = rand(0.55, 0.98) * height
    const at = { x: crown.x - dx * (height * 0.85 - along), y: crown.y + rand(0.3, 1.6), z: crown.z - dz * (height * 0.85 - along) }
    leaf(fx, at, [rand(-2.5, 2.5), rand(2, 6), rand(-2.5, 2.5)])
  }
}
