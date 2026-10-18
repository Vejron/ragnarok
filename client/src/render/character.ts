import { Animator, Group, Mesh, SkinnedMesh, parseGlb, type AnimationClip, type Geometry, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { Canopy } from './canopy'
import { toonMaterial } from './toon'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, PRONE_CENTRE, RUN_SPEED, STAND_CENTRE, WALK_SPEED } from '@/sim/consts'
import { ITEM_AK5, ITEM_BANDAGE, ITEM_GRENADE, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER, itemDef, type ItemDef } from '@/sim/items'
import { type Limb, poseLimb, splitLimbs } from './gibs'
import { colorFor } from '@/profile'

/** Gravity on a thrown body, m/s². */
const FLING_GRAVITY = 14
/** Feet sit this far below the capsule centre. */
const FOOT_OFFSET = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS

export const MODEL_URL = `${import.meta.env.BASE_URL}models/player.glb`

type Direction = 'fwd' | 'back' | 'left' | 'right'
const DIRECTIONS: Direction[] = ['fwd', 'back', 'left', 'right']

type Stance = ItemDef['stance']

/** Clip name prefix per stance, and the weapon mesh each item shows. Mirrors build_character.py. */
const PREFIX: Record<Stance, string> = { rifle: '', pistol: 'pistol_', hold: 'hold_' }
const WEAPON_MESH: Record<number, string> = {
  [ITEM_AK5]: 'weapon_ak5',
  [ITEM_SNIPER]: 'weapon_sniper',
  [ITEM_REVOLVER]: 'weapon_revolver',
  // No model of its own yet: it borrows the AK5's in hand.
  [ITEM_SHOTGUN]: 'weapon_ak5',
  [ITEM_GRENADE]: 'weapon_grenade',
  [ITEM_BANDAGE]: 'weapon_bandage',
}
/** The muzzle bone of the stance whose hand pose a gun is modelled in; none for the hands-low stance. */
const MUZZLE_BONE: Record<Stance, string | null> = { rifle: 'muzzle', pistol: 'muzzle_pistol', hold: null }

/**
 * Clips baked by `tools/character/build_character.py`, with the ground speed (m/s) at which their
 * feet stop sliding. Every stance has the same set under its own prefix. The speeds follow from each clip's stride and length, so they only change when
 * the script's gait does.
 */
const SPEED_OF: Record<string, number> = {
  walk_fwd: 3.8,
  walk_back: 3.8,
  walk_left: 3.1,
  walk_right: 3.1,
  run_fwd: 7.3,
  run_back: 6.4,
  run_left: 5.8,
  run_right: 5.8,
  crouch_fwd: 1.8,
  crouch_back: 1.8,
  crouch_left: 1.5,
  crouch_right: 1.5,
  // Crawling covers a stride every 0.8 s, so it cycles at the rate the body actually moves.
  crawl_fwd: 1.0,
  crawl_back: 1.0,
  crawl_left: 0.9,
  crawl_right: 0.9,
}
const LOCOMOTION = Object.keys(SPEED_OF)

/** Below this ground speed the character just stands; `FULL_STEP` is where the walk is fully in. */
const STILL_SPEED = 0.3
const FULL_STEP = 1.4
/** How quickly blend weights chase their targets, per second. */
const BLEND_RATE = 12
const AIR_RATE = 10
const DEATH_RATE = 9
const CROUCH_RATE = 10
const SLIDE_RATE = 14
/** Torso lean for aiming: how much of the camera pitch the chest and spine take. */
const AIM_CHEST = 0.4
const AIM_SPINE = 0.2
const RECOIL_KICK = 0.09
const RECOIL_DECAY = 14

export interface CharacterAsset {
  scene: Node
  clips: Map<string, AnimationClip>
}

/** Parses a GLB of the player. Materials become flat toon ones to match the rest of the scene. */
export async function parseCharacter(data: ArrayBuffer): Promise<CharacterAsset> {
  const gltf = parseGlb(data)
  gltf.scene.traverse((node) => {
    if (!(node instanceof Mesh)) return
    node.material = toonMaterial({ vertexColors: true, name: node.material.name })
    node.castShadow = true
  })
  return { scene: gltf.scene, clips: new Map(gltf.clips.map((clip) => [clip.name, clip])) }
}

export async function loadCharacter(url = MODEL_URL): Promise<CharacterAsset> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  return parseCharacter(await response.arrayBuffer())
}

/**
 * Where every character in the scene lives: the shared model and its materials. A `Character` is one
 * skinned clone of it.
 */
export class Crowd {
  readonly root = new Group()
  asset: CharacterAsset | null = null
  /** Outfit materials of live characters, so `dispose` can free them. */
  readonly outfits = new Set<Material>()
  /** Each skinned mesh's limbs, cut once, for gibs. */
  readonly limbs = new Map<Geometry, Limb[]>()
  private count = 0

  async load(url = MODEL_URL) {
    this.asset = await loadCharacter(url)
  }

  /** How many characters are being drawn. */
  get size() {
    return this.count
  }

  /** @internal */
  adjust(by: number) {
    this.count += by
  }

  dispose() {
    for (const material of this.outfits) material.dispose()
    this.outfits.clear()
    for (const limbs of this.limbs.values()) for (const limb of limbs) limb.geometry.dispose()
    this.limbs.clear()
    this.asset?.scene.traverse((node) => {
      if (!(node instanceof Mesh)) return
      node.geometry.dispose()
      node.material.dispose()
    })
    this.root.removeFromParent()
  }
}

const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)))
  return t * t * (3 - 2 * t)
}
const chase = (value: number, target: number, rate: number, dt: number) => value + (target - value) * (1 - Math.exp(-rate * dt))

const AXIS_X = new THREE.Vector3(1, 0, 0)
const SCRATCH = {
  q: new THREE.Quaternion(),
  parent: new THREE.Quaternion(),
  axis: new THREE.Vector3(),
}

/**
 * One player: a skinned clone of the model, placed at its capsule centre and animated from where it
 * is moving relative to where it faces, so strafing and backpedalling play their own clips.
 */
/** How long a hit whitens a character, seconds. */
const HIT_FLASH = 0.14

export class Character {
  /** The capsule centre, as last placed. */
  readonly position = new THREE.Vector3()
  private readonly object: Node
  private readonly animator: Animator
  private readonly actions: Animator['actions']
  private readonly weights = new Map<string, number>()
  private readonly outfit: Material
  private readonly chest: Node | null
  private readonly spine: Node | null
  private readonly muzzleBones = new Map<Stance, Node>()
  /** Each weapon's mesh, by item id; only the one in hand is shown. */
  private readonly weapons = new Map<number, Node>()
  private stance: Stance = 'rifle'
  private item = ITEM_AK5
  /** Walk/run cycle position, 0..1, shared by every locomotion clip so blends stay in step. */
  private phase = 0
  private idleTime = 0
  private air = 0
  private death = 0
  private crouch = 0
  /** How far lying down, 0 standing to 1 prone. */
  private lie = 0
  private slide = 0
  private down = false
  private shown = true
  /** False until the first `update` puts it somewhere, so it does not flash at the origin. */
  private placed = false
  private pitch = 0
  private recoil = 0
  /** Seconds of hit flash left, which whitens the outfit. */
  private flashLeft = 0
  /** Blown to pieces: the body is gone, only gibs remain. */
  private gibbed = false
  private readonly head: Node | null
  private headless = false
  /** A body thrown by its killing blow: where it has got to, relative to where it died. */
  private fling: { velocity: THREE.Vector3; spin: THREE.Vector3; offset: THREE.Vector3; groundAt: (x: number, z: number) => number } | null = null
  private readonly tumble = new THREE.Quaternion()
  private readonly canopy: Canopy

  constructor(private readonly crowd: Crowd, id: number) {
    const asset = crowd.asset
    if (!asset) throw new Error('Crowd.load() must finish before characters are created')
    this.object = asset.scene.clone()
    this.outfit = toonMaterial({ vertexColors: true, color: colorFor(id) })
    this.object.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'outfit') node.material = this.outfit
    })
    crowd.outfits.add(this.outfit)
    crowd.root.add(this.object)
    crowd.adjust(1)

    this.canopy = new Canopy(colorFor(id))
    this.object.add(this.canopy.root)

    this.chest = this.object.getObjectByName('chest') ?? null
    this.spine = this.object.getObjectByName('spine') ?? null
    this.head = this.object.getObjectByName('head') ?? null
    for (const stance of Object.keys(MUZZLE_BONE) as Stance[]) {
      const bone = MUZZLE_BONE[stance] ? this.object.getObjectByName(MUZZLE_BONE[stance]!) : null
      if (bone) this.muzzleBones.set(stance, bone)
    }
    for (const [item, name] of Object.entries(WEAPON_MESH)) {
      const mesh = this.object.getObjectByName(name)
      if (mesh) this.weapons.set(Number(item), mesh)
    }
    this.showWeapon()

    this.animator = new Animator(this.object, [...asset.clips.values()])
    this.actions = this.animator.actions
    for (const [name, action] of this.actions) {
      // Weights and time are driven by hand, so the animator only has to sample.
      action.timeScale = 0
      action.weight = 0
      // The death clip plays once and holds its last frame.
      if (name === 'death') action.loop = 'once'
      this.weights.set(name, 0)
    }
    this.weights.set(this.clip('idle'), 1)
    const idle = this.actions.get(this.clip('idle'))
    if (idle) idle.weight = 1
    this.object.visible = false
  }

  /** Whether it is drawn: false while it sits in a car. */
  get visible() {
    return this.shown
  }

  /** The blend weight of each clip, for tests and debugging. */
  get blend(): ReadonlyMap<string, number> {
    return this.weights
  }

  /**
   * Places the character at its capsule centre and animates it. `vx`/`vz` is its horizontal velocity
   * in the world, `yaw` where it faces (0 faces -Z), `pitch` where it aims (up is positive).
   */
  /** Whitens the outfit for a moment: it was just hit. */
  flash() {
    this.flashLeft = HIT_FLASH
  }

  /** Repaints the outfit: the player's chosen colour can arrive after their stickman. */
  setColor(hex: number) {
    this.outfit.color.setHex(hex)
    this.canopy.setColor(hex)
    this.outfit.needsUpdate()
  }

  update(
    position: readonly number[], yaw: number, vx: number, vz: number, grounded: boolean, dt: number, pitch = 0, crouching = false, sliding = false, prone = false, chute = false,
  ) {
    this.position.set(position[0]!, position[1]!, position[2]!)
    this.pitch = pitch
    if (this.flashLeft > 0) {
      this.flashLeft = Math.max(0, this.flashLeft - dt)
      const glow = (this.flashLeft / HIT_FLASH) * 0.9
      this.outfit.emissive.set(glow, glow, glow)
      this.outfit.needsUpdate()
    }
    this.flingStep(dt)
    if (!this.shown || this.gibbed) return

    // Velocity in the body's frame: forward is -Z at yaw 0, right is +X.
    const sin = Math.sin(yaw)
    const cos = Math.cos(yaw)
    const forward = -(vx * sin + vz * cos)
    const right = vx * cos - vz * sin
    const speed = Math.hypot(forward, right)

    this.canopy.update(chute && !this.down, dt)
    this.air = chase(this.air, !grounded && !this.down ? 1 : 0, AIR_RATE, dt)
    this.death = chase(this.death, this.down ? 1 : 0, DEATH_RATE, dt)
    this.crouch = chase(this.crouch, crouching ? 1 : 0, CROUCH_RATE, dt)
    this.lie = chase(this.lie, prone ? 1 : 0, CROUCH_RATE, dt)
    const alive = 1 - this.death
    const ground = alive * (1 - this.air)
    this.slide = chase(this.slide, sliding && crouching ? 1 : 0, SLIDE_RATE, dt)
    // Lying down is its own stance: the crouch and slide clips give way to the crawl clips.
    const standing = ground * (1 - this.crouch) * (1 - this.lie)
    const low = ground * this.crouch * (1 - this.slide) * (1 - this.lie)
    const crawl = ground * this.lie
    const sliding_ = ground * this.crouch * this.slide * (1 - this.lie)

    // How much of the standing weight is locomotion, and how much of that is a run.
    const moving = smooth(STILL_SPEED, FULL_STEP, speed)
    const running = smooth(WALK_SPEED + 0.4, RUN_SPEED - 0.6, speed)
    const targets = new Map<string, number>()
    // Every clip is pulled to zero unless this stance's blend asks for it, so a stance change crossfades.
    for (const name of this.actions.keys()) targets.set(name, 0)
    const along: Record<Direction, number> = {
      fwd: Math.max(0, forward),
      back: Math.max(0, -forward),
      right: Math.max(0, right),
      left: Math.max(0, -right),
    }
    const total = along.fwd + along.back + along.right + along.left || 1
    const set = (name: string, weight: number) => targets.set(this.clip(name), weight)
    for (const dir of DIRECTIONS) {
      const share = along[dir] / total
      set(`walk_${dir}`, standing * moving * (1 - running) * share)
      set(`run_${dir}`, standing * moving * running * share)
      set(`crouch_${dir}`, low * moving * share)
      set(`crawl_${dir}`, crawl * moving * share)
    }
    set('idle', standing * (1 - moving))
    set('crouch_idle', low * (1 - moving))
    set('crawl_idle', crawl * (1 - moving))
    set('slide', sliding_)
    set('jump_air', alive * this.air)
    targets.set('death', this.death)

    // Weights chase their targets; both sets sum to one, so the blend stays a convex mix.
    const rate = BLEND_RATE
    let sum = 0
    for (const [name, target] of targets) {
      const next = chase(this.weights.get(name) ?? 0, target, rate, dt)
      this.weights.set(name, next)
      sum += next
    }
    if (sum > 0) for (const [name, w] of this.weights) this.weights.set(name, w / sum)

    // The cycle advances by distance covered, so feet keep up with the ground at any speed.
    let stride = 0
    let weightSum = 0
    for (const prefix of Object.values(PREFIX)) {
      for (const base of LOCOMOTION) {
        const name = prefix + base
        const w = this.weights.get(name) ?? 0
        const action = this.actions.get(name)
        if (!action || w <= 0) continue
        stride += w * SPEED_OF[base]! * action.clip.duration
        weightSum += w
      }
    }
    if (weightSum > 0 && stride > 0) this.phase = (this.phase + (dt * speed * weightSum) / stride) % 1
    const idle = this.actions.get('idle')
    if (idle) this.idleTime = (this.idleTime + dt) % idle.clip.duration

    const current = this.clip('idle')
    for (const [name, action] of this.actions) {
      const w = this.weights.get(name)!
      const base = name.replace(/^(pistol_|hold_)/, '')
      action.enabled = w > 0.0005 || name === current
      action.weight = w
      if (base === 'idle' || base === 'crouch_idle' || base === 'crawl_idle' || base === 'slide') action.time = this.idleTime
      else if (base === 'jump_air') action.time = (this.idleTime / (idle?.clip.duration ?? 1)) * action.clip.duration
      else if (base !== 'death') action.time = this.phase * action.clip.duration
    }
    // The death clip plays out on the animator's clock once it starts.
    const dying = this.actions.get('death')
    if (dying) dying.timeScale = this.down ? 1 : 0

    this.animator.update(dt)
    this.head?.scale.setScalar(this.headless ? 0.001 : 1)
    this.recoil = chase(this.recoil, 0, RECOIL_DECAY, dt)
    this.aim()
    // A slide carries the body along its heading, rifle and all, rather than where the camera looks.
    const heading = speed > 0.5 ? Math.atan2(-vx, -vz) : yaw
    const turn = Math.atan2(Math.sin(heading - yaw), Math.cos(heading - yaw))
    this.place(yaw + turn * this.slide)
  }

  private place(yaw: number) {
    // Lying down, the centre is already lower; the pivot goes back up to the ground the feet stand on.
    this.object.position.set(this.position.x, this.position.y - FOOT_OFFSET + (STAND_CENTRE - PRONE_CENTRE) * this.lie, this.position.z)
    // The model faces +Z; the game's yaw 0 faces -Z.
    // Set all three: a tumble leaves its pitch and roll in the Euler angles, and `rotation.y` alone
    // would keep them, leaving a respawned player tilted.
    this.object.rotation.set(0, yaw + Math.PI, 0, 'YXZ')
    if (this.fling) {
      // Tumble about the middle of the body, not the feet.
      const middle = SCRATCH.axis.set(0, FOOT_OFFSET, 0)
      this.object.quaternion.premultiply(this.tumble)
      this.object.position.add(this.fling.offset).add(middle).sub(middle.applyQuaternion(this.tumble))
    }
    this.placed = true
    this.object.visible = this.shown && !this.gibbed
  }

  /** Flies a thrown body: gravity, a bounce, and a tumble that settles into the death pose. */
  private flingStep(dt: number) {
    const fling = this.fling
    if (!fling) return
    const { velocity, spin, offset } = fling
    velocity.y -= FLING_GRAVITY * dt
    offset.addScaledVector(velocity, dt)
    const rate = spin.length()
    const floor = fling.groundAt(this.position.x + offset.x, this.position.z + offset.z) + 0.1 + 0.6 * Math.min(1, rate / 6)
    if (this.position.y - FOOT_OFFSET + offset.y < floor) {
      offset.y = floor - (this.position.y - FOOT_OFFSET)
      velocity.y = Math.abs(velocity.y) > 2 ? Math.abs(velocity.y) * 0.3 : 0
      velocity.x *= 0.75
      velocity.z *= 0.75
      spin.multiplyScalar(0.7)
    }
    if (rate > 1e-3) this.tumble.premultiply(SCRATCH.q.setFromAxisAngle(SCRATCH.axis.copy(spin).divideScalar(rate), rate * dt))
    // Once it has stopped bouncing, lie down the way the death clip lays it.
    if (velocity.y === 0 && rate < 3) this.tumble.slerp(SCRATCH.q.identity(), 1 - Math.exp(-6 * dt))
    this.tumble.normalize()
  }

  /**
   * Throws the body along `velocity`, tumbling with `spin` (rad/s about world axes): the killing blow
   * carried it off. It lands and lies as a death does.
   */
  launch(velocity: THREE.Vector3, spin: THREE.Vector3, groundAt: (x: number, z: number) => number) {
    this.fling = { velocity: velocity.clone(), spin: spin.clone(), offset: new THREE.Vector3(), groundAt }
    this.tumble.identity()
  }

  /** The head is gone (it is thrown separately, see `shatter`). */
  decapitate() {
    this.headless = true
  }

  /**
   * Blows the body apart: hides it and returns its limbs where they are now, ready to throw. With
   * `only`, just those limbs come off (a head), and the body stays.
   */
  shatter(only?: readonly string[]): { key: string; object: Group }[] {
    const pieces: { key: string; object: Group }[] = []
    this.object.updateWorldMatrix(true, true)
    this.object.traverse((node) => {
      if (!(node instanceof SkinnedMesh) || node.name.startsWith('weapon_')) return
      const mesh = node
      let limbs = this.crowd.limbs.get(mesh.geometry)
      if (!limbs) this.crowd.limbs.set(mesh.geometry, (limbs = splitLimbs(mesh)))
      for (const limb of limbs) {
        if (only && !only.includes(limb.key)) continue
        const object = poseLimb(mesh, limb)
        if (object) pieces.push({ key: limb.key, object })
      }
    })
    if (!only) {
      this.gibbed = true
      this.object.visible = false
    }
    return pieces
  }

  /** Leans the torso (and the rifle it carries) with the camera pitch and the last shot's kick. */
  private aim() {
    if (this.down || this.death > 0.3) return
    const lean = -this.pitch - this.recoil
    this.bend(this.spine, lean * AIM_SPINE)
    this.bend(this.chest, lean * AIM_CHEST)
  }

  /** Rotates a bone about the body's left-right axis, in model space. */
  private bend(bone: Node | null, angle: number) {
    if (!bone || Math.abs(angle) < 1e-4) return
    // The bone's parent frame relative to the model root, to carry the model-space axis down into it.
    const parent = SCRATCH.parent.identity()
    for (let node = bone.parent; node && node !== this.object; node = node.parent) {
      parent.premultiply(SCRATCH.q.set(node.quaternion.x, node.quaternion.y, node.quaternion.z, node.quaternion.w))
    }
    SCRATCH.axis.copy(AXIS_X).applyQuaternion(parent.invert())
    bone.quaternion.premultiply(SCRATCH.q.setFromAxisAngle(SCRATCH.axis, angle))
  }

  /** Kicks the rifle back; call when this player fires. */
  fire() {
    this.recoil = RECOIL_KICK
  }

  /** The clip for `base` in the stance being held. */
  private clip(base: string) {
    return PREFIX[this.stance] + base
  }

  /** Shows the weapon in hand and hides the rest. */
  private showWeapon() {
    // Two items may share a mesh, so hide them all before showing the one in hand.
    for (const mesh of this.weapons.values()) mesh.visible = false
    const mesh = this.weapons.get(this.item)
    if (mesh) mesh.visible = true
  }

  /** The item in hand (`ITEM_*`, 0 for none): picks the weapon drawn and how the arms hold it. */
  set held(item: number) {
    if (item === this.item) return
    this.item = item
    this.stance = itemDef(item)?.stance ?? 'hold'
    this.showWeapon()
  }

  get held() {
    return this.item
  }

  /** Where the gun in hand ends, in the world; null while hidden or when nothing is held that shoots. */
  muzzle(out = new THREE.Vector3()): THREE.Vector3 | null {
    const bone = this.muzzleBones.get(this.stance)
    if (!this.shown || !bone) return null
    this.object.updateWorldMatrix(true, true)
    return bone.getWorldPosition(out)
  }

  /** Lying where it fell. */
  set dead(dead: boolean) {
    if (this.down === dead) return
    this.down = dead
    if (!dead) {
      // Respawned: whole again, upright and drawn.
      this.gibbed = false
      this.headless = false
      this.fling = null
      this.tumble.identity()
      this.head?.scale.setScalar(1)
      this.object.visible = this.shown && this.placed
    }
    const action = this.actions.get('death')
    if (!action) return
    // Dying plays the clip from its start; a respawn winds it back and holds it there.
    action.time = 0
    action.timeScale = dead ? 1 : 0
  }

  /** Hides the character while it sits in a car. */
  set hidden(hidden: boolean) {
    this.shown = !hidden
    this.object.visible = this.shown && this.placed && !this.gibbed
  }

  dispose() {
    this.canopy.dispose()
    this.object.removeFromParent()
    this.crowd.outfits.delete(this.outfit)
    this.outfit.dispose()
    this.crowd.adjust(-1)
  }
}
