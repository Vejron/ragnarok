import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { clone as cloneSkinned } from 'three/examples/jsm/utils/SkeletonUtils.js'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, RUN_SPEED, WALK_SPEED } from '@/sim/consts'
import { ITEM_AK5, ITEM_BANDAGE, ITEM_GRENADE, ITEM_REVOLVER, itemDef, type ItemDef } from '@/sim/items'

/** Feet sit this far below the capsule centre. */
const FOOT_OFFSET = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS

/** Bright, goofy colours, picked by entity id. */
const PALETTE = [0xff5a5f, 0x2ec4b6, 0xffbe0b, 0x8338ec, 0x3a86ff, 0xfb5607, 0x06d6a0, 0xff006e]

/** The outfit colour of entity `id`, also used for the heads of passengers peeking out of cars. */
export const colorFor = (id: number) => PALETTE[id % PALETTE.length]!

export const MODEL_URL = `${import.meta.env.BASE_URL}models/player.glb`

type Direction = 'fwd' | 'back' | 'left' | 'right'
const DIRECTIONS: Direction[] = ['fwd', 'back', 'left', 'right']

type Stance = ItemDef['stance']

/** Clip name prefix per stance, and the weapon mesh each item shows. Mirrors build_character.py. */
const PREFIX: Record<Stance, string> = { rifle: '', pistol: 'pistol_', hold: 'hold_' }
const WEAPON_MESH: Record<number, string> = {
  [ITEM_AK5]: 'weapon_ak5',
  [ITEM_REVOLVER]: 'weapon_revolver',
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
  scene: THREE.Object3D
  clips: Map<string, THREE.AnimationClip>
}

/** Parses a GLB of the player. Materials become flat toon ones to match the rest of the scene. */
export function parseCharacter(data: ArrayBuffer): Promise<CharacterAsset> {
  return new Promise((resolve, reject) => {
    new GLTFLoader().parse(
      data,
      '',
      (gltf) => {
        gltf.scene.traverse((node) => {
          const mesh = node as THREE.Mesh
          if (!mesh.isMesh) return
          const source = mesh.material as THREE.MeshStandardMaterial
          const toon = new THREE.MeshToonMaterial({ vertexColors: true, name: source.name })
          source.dispose()
          mesh.material = toon
          mesh.castShadow = true
        })
        resolve({ scene: gltf.scene, clips: new Map(gltf.animations.map((clip) => [clip.name, clip])) })
      },
      reject,
    )
  })
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
  readonly root = new THREE.Group()
  asset: CharacterAsset | null = null
  /** Outfit materials of live characters, so `dispose` can free them. */
  readonly outfits = new Set<THREE.Material>()
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
    this.asset?.scene.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (!mesh.isMesh) return
      mesh.geometry.dispose()
      ;(mesh.material as THREE.Material).dispose()
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
export class Character {
  /** The capsule centre, as last placed. */
  readonly position = new THREE.Vector3()
  private readonly object: THREE.Object3D
  private readonly mixer: THREE.AnimationMixer
  private readonly actions = new Map<string, THREE.AnimationAction>()
  private readonly weights = new Map<string, number>()
  private readonly outfit: THREE.MeshToonMaterial
  private readonly chest: THREE.Object3D | null
  private readonly spine: THREE.Object3D | null
  private readonly muzzleBones = new Map<Stance, THREE.Object3D>()
  /** Each weapon's mesh, by item id; only the one in hand is shown. */
  private readonly weapons = new Map<number, THREE.Object3D>()
  private stance: Stance = 'rifle'
  private item = ITEM_AK5
  /** Walk/run cycle position, 0..1, shared by every locomotion clip so blends stay in step. */
  private phase = 0
  private idleTime = 0
  private air = 0
  private death = 0
  private crouch = 0
  private slide = 0
  private down = false
  private shown = true
  /** False until the first `update` puts it somewhere, so it does not flash at the origin. */
  private placed = false
  private pitch = 0
  private recoil = 0

  constructor(private readonly crowd: Crowd, id: number) {
    const asset = crowd.asset
    if (!asset) throw new Error('Crowd.load() must finish before characters are created')
    this.object = cloneSkinned(asset.scene)
    this.outfit = new THREE.MeshToonMaterial({ vertexColors: true, color: colorFor(id) })
    this.object.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh && (mesh.material as THREE.Material).name === 'outfit') mesh.material = this.outfit
    })
    crowd.outfits.add(this.outfit)
    crowd.root.add(this.object)
    crowd.adjust(1)

    this.chest = this.object.getObjectByName('chest') ?? null
    this.spine = this.object.getObjectByName('spine') ?? null
    for (const stance of Object.keys(MUZZLE_BONE) as Stance[]) {
      const bone = MUZZLE_BONE[stance] ? this.object.getObjectByName(MUZZLE_BONE[stance]!) : null
      if (bone) this.muzzleBones.set(stance, bone)
    }
    for (const [item, name] of Object.entries(WEAPON_MESH)) {
      const mesh = this.object.getObjectByName(name)
      if (mesh) this.weapons.set(Number(item), mesh)
    }
    this.showWeapon()

    this.mixer = new THREE.AnimationMixer(this.object)
    for (const [name, clip] of asset.clips) {
      const action = this.mixer.clipAction(clip)
      action.play()
      // Weights and time are driven by hand, so the mixer only has to sample.
      action.timeScale = 0
      action.setEffectiveWeight(0)
      if (name === 'death') {
        action.setLoop(THREE.LoopOnce, 1)
        action.clampWhenFinished = true
      }
      this.actions.set(name, action)
      this.weights.set(name, 0)
    }
    this.weights.set(this.clip('idle'), 1)
    this.actions.get(this.clip('idle'))?.setEffectiveWeight(1)
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
  update(
    position: readonly number[], yaw: number, vx: number, vz: number, grounded: boolean, dt: number, pitch = 0, crouching = false, sliding = false,
  ) {
    this.position.set(position[0]!, position[1]!, position[2]!)
    this.pitch = pitch
    if (!this.shown) return

    // Velocity in the body's frame: forward is -Z at yaw 0, right is +X.
    const sin = Math.sin(yaw)
    const cos = Math.cos(yaw)
    const forward = -(vx * sin + vz * cos)
    const right = vx * cos - vz * sin
    const speed = Math.hypot(forward, right)

    this.air = chase(this.air, !grounded && !this.down ? 1 : 0, AIR_RATE, dt)
    this.death = chase(this.death, this.down ? 1 : 0, DEATH_RATE, dt)
    this.crouch = chase(this.crouch, crouching ? 1 : 0, CROUCH_RATE, dt)
    const alive = 1 - this.death
    const ground = alive * (1 - this.air)
    this.slide = chase(this.slide, sliding && crouching ? 1 : 0, SLIDE_RATE, dt)
    const standing = ground * (1 - this.crouch)
    const low = ground * this.crouch * (1 - this.slide)
    const sliding_ = ground * this.crouch * this.slide

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
    }
    set('idle', standing * (1 - moving))
    set('crouch_idle', low * (1 - moving))
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
        stride += w * SPEED_OF[base]! * action.getClip().duration
        weightSum += w
      }
    }
    if (weightSum > 0 && stride > 0) this.phase = (this.phase + (dt * speed * weightSum) / stride) % 1
    const idle = this.actions.get('idle')
    if (idle) this.idleTime = (this.idleTime + dt) % idle.getClip().duration

    const current = this.clip('idle')
    for (const [name, action] of this.actions) {
      const w = this.weights.get(name)!
      const base = name.replace(/^(pistol_|hold_)/, '')
      action.enabled = w > 0.0005 || name === current
      action.setEffectiveWeight(w)
      if (base === 'idle' || base === 'crouch_idle' || base === 'slide') action.time = this.idleTime
      else if (base === 'jump_air') action.time = (this.idleTime / (idle?.getClip().duration ?? 1)) * action.getClip().duration
      else if (base !== 'death') action.time = this.phase * action.getClip().duration
    }
    // The death clip plays out on the mixer's clock once it starts.
    const dying = this.actions.get('death')
    if (dying) dying.timeScale = this.down ? 1 : 0

    this.mixer.update(dt)
    this.recoil = chase(this.recoil, 0, RECOIL_DECAY, dt)
    this.aim()
    // A slide carries the body along its heading, rifle and all, rather than where the camera looks.
    const heading = speed > 0.5 ? Math.atan2(-vx, -vz) : yaw
    const turn = Math.atan2(Math.sin(heading - yaw), Math.cos(heading - yaw))
    this.place(yaw + turn * this.slide)
  }

  private place(yaw: number) {
    this.object.position.set(this.position.x, this.position.y - FOOT_OFFSET, this.position.z)
    // The model faces +Z; the game's yaw 0 faces -Z.
    this.object.rotation.y = yaw + Math.PI
    this.placed = true
    this.object.visible = this.shown
  }

  /** Leans the torso (and the rifle it carries) with the camera pitch and the last shot's kick. */
  private aim() {
    if (this.down || this.death > 0.3) return
    const lean = -this.pitch - this.recoil
    this.bend(this.spine, lean * AIM_SPINE)
    this.bend(this.chest, lean * AIM_CHEST)
  }

  /** Rotates a bone about the body's left-right axis, in model space. */
  private bend(bone: THREE.Object3D | null, angle: number) {
    if (!bone || Math.abs(angle) < 1e-4) return
    // The bone's parent frame relative to the model root, to carry the model-space axis down into it.
    const parent = SCRATCH.parent.identity()
    for (let node = bone.parent; node && node !== this.object; node = node.parent) parent.premultiply(node.quaternion)
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
    for (const [item, mesh] of this.weapons) mesh.visible = item === this.item
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
    const action = this.actions.get('death')
    if (!action) return
    if (dead) {
      action.reset()
      action.play()
      action.timeScale = 1
    } else {
      action.stop()
      action.play()
      action.timeScale = 0
    }
  }

  /** Hides the character while it sits in a car. */
  set hidden(hidden: boolean) {
    this.shown = !hidden
    this.object.visible = this.shown && this.placed
  }

  dispose() {
    this.mixer.stopAllAction()
    this.object.removeFromParent()
    this.crowd.outfits.delete(this.outfit)
    this.outfit.dispose()
    this.crowd.adjust(-1)
  }
}
