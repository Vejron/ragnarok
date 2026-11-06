import { Animator, Group, Mesh, SkinnedMesh, parseGlb, type AnimationClip, type Geometry, type Material, type Node, Matrix4, Quaternion, Vector3 } from '@ragnarok/engine'
import { Canopy } from './canopy'
import { toonMaterial } from './toon'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, PRONE_CENTRE, RUN_SPEED, SKI_BOOST_TOP, STAND_CENTRE, WALK_SPEED } from '@/sim/consts'
import { ITEM_AK5, ITEM_AT_RIFLE, ITEM_BANDAGE, ITEM_BRICKS, ITEM_CARL_GUSTAF, ITEM_FLARE, ITEM_GRENADE, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER, fillsHands, itemDef, type ItemDef } from '@/sim/items'
import { type Limb, poseLimb, splitLimbs } from './gibs'
import { colorFor } from '@/profile'
import { LIMB_ALL, LIMB_ARM_L, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_R, PART_HEAD } from '@/net/codec'
import { buildBackpack } from './backpack'
import { Headlamp } from './headlamp'
import { POLE_REACH, SOLE_DROP, buildPole, buildSki } from './skigear'
import { buildClub, swingLength, swingPhase } from './golfclub'
import { smoothstep } from '@/math'

/** Gravity on a thrown body, m/s². */
const FLING_GRAVITY = 14
/** The worn backpack's bottom: this high above the feet standing, this much lower crouched, and this far behind the body's middle. */
const PACK_Y = 0.85
const PACK_CROUCH_DROP = 0.3
const PACK_BACK = 0.2
/** Feet sit this far below the capsule centre. */
const FOOT_OFFSET = CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS

export const MODEL_URL = `${import.meta.env.BASE_URL}models/player.glb`

type Direction = 'fwd' | 'back' | 'left' | 'right'
const DIRECTIONS: Direction[] = ['fwd', 'back', 'left', 'right']

type Stance = ItemDef['stance']

/** Clip name prefix per stance, and the weapon mesh each item shows. Mirrors build_character.py. */
const PREFIX: Record<Stance, string> = { rifle: '', pistol: 'pistol_', hold: 'hold_', shoulder: 'shoulder_' }
const WEAPON_MESH: Record<number, string> = {
  [ITEM_AK5]: 'weapon_ak5',
  [ITEM_SNIPER]: 'weapon_sniper',
  [ITEM_REVOLVER]: 'weapon_revolver',
  // No model of its own yet: it borrows the AK5's in hand.
  [ITEM_SHOTGUN]: 'weapon_ak5',
  [ITEM_GRENADE]: 'weapon_grenade',
  [ITEM_BANDAGE]: 'weapon_bandage',
  // The same: a pistol grip, so it borrows the revolver's.
  [ITEM_FLARE]: 'weapon_revolver',
  // A block in the hand, as a bandage roll is.
  [ITEM_BRICKS]: 'weapon_bandage',
  [ITEM_CARL_GUSTAF]: 'weapon_carl_gustaf',
  // No model of its own yet: a long rifle, it borrows the sniper's in hand.
  [ITEM_AT_RIFLE]: 'weapon_sniper',
}
/** The muzzle bone of the stance whose hand pose a gun is modelled in; none for the hands-low stance. */
/** The bones a lost limb is shrunk away at, in `LIMB_*` bit order. */
const LIMB_BONES = ['upper_arm_L', 'upper_arm_R', 'thigh_L', 'thigh_R']

/** The `LIMB_*` bit a gib (`limbOf`'s key) belongs to; 0 for the head and the torso. */
function limbBit(key: string): number {
  const side = key.endsWith('_L') ? 0 : key.endsWith('_R') ? 1 : -1
  if (side < 0) return 0
  if (key.startsWith('upper_arm') || key.startsWith('forearm')) return 1 << side
  if (key.startsWith('thigh') || key.startsWith('shin')) return 1 << (2 + side)
  return 0
}

const MUZZLE_BONE: Record<Stance, string | null> = { rifle: 'muzzle', pistol: 'muzzle_pistol', hold: null, shoulder: 'muzzle_shoulder' }

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
  // Ground covered by one cycle of the arms, not by the feet: a ski just slides.
  ski_stride: 4.2,
  ski_pole: 9,
  ski_climb: 1.5,
  ski_stride_hard: 3.4,
  ski_pole_hard: 7,
  armed_skate: 3,
  armed_climb: 1.5,
  crouch_fwd: 1.8,
  crouch_back: 1.8,
  crouch_left: 1.5,
  crouch_right: 1.5,
  // The knees push off about 0.4 m/s worth (the build script prints it), but lying flat the slide
  // hardly shows and a crawl cycling that fast looks frantic: this keeps a steady pull at prone speed.
  crawl_fwd: 0.7,
  crawl_back: 0.7,
  crawl_left: 0.7,
  crawl_right: 0.7,
}
const LOCOMOTION = Object.keys(SPEED_OF)

/** Speed the arms go from the diagonal stride to double poling, m/s. */
const SKI_POLE_FROM = 5.5
/** Slowing that begins to look like a snowplow, and that is one, m/s². */
const SKI_PLOW_FROM = 1.2
/** Rise over run from which it is a herringbone, how far each ski turns out, radians, and the speed its steps never pace slower than, m/s. */
const HERRING_RISE = 0.16
const HERRING_SPLAY = 0.52
const HERRING_PACE = 1.1
/** The speed the arms' tempo never drops under when poling hard, m/s. */
const HARD_PACE = 6
const SKI_PLOW_FULL = 2.4
const SKI_SENSE_RATE = 5
/** How far a pole's tip trails behind the hand, radians. */
const POLE_TILT = 0.32
/** How much further back they go in a tuck, and how far they splay out from the body, radians. */
const POLE_TUCK = 1.05
const POLE_SPLAY = 0.1
/** How far a pole swings back at the end of its push, radians, its tip off the snow. */
const POLE_BACK = 1.5
/** How far the headlamp's beam is dipped below where the face looks: a rise over a run of 1. */
const LAMP_DIP = 0.12
const SKI_TILT = new Quaternion()
const SKI_V = new Quaternion()
const SKI_SPLAY = new Quaternion()
/** How far through its stroke a pole is off the snow, 0 being on it, from the plant with the hand forward: held there through the push, then back and round to the next plant. */
const poleLift = (stroke: number) => {
  const q = stroke % 1
  return smoothstep(0.36, 0.56, q) * (1 - smoothstep(0.56, 1, q))
}

/** Below this ground speed the character just stands; `FULL_STEP` is where the walk is fully in. */
const STILL_SPEED = 0.3
const FULL_STEP = 1.4
/** How quickly blend weights chase their targets, per second. */
const BLEND_RATE = 12
const AIR_RATE = 10
const DEATH_RATE = 9
const CROUCH_RATE = 10
/** Getting down onto the belly and back up is slower than a crouch, so it reads as a movement. */
const LIE_RATE = 5
const SLIDE_RATE = 14
/** Torso lean for aiming: how much of the camera pitch the chest and spine take. */
const AIM_CHEST = 0.4
const AIM_SPINE = 0.2
/** And the neck, so the head looks where it aims. */
const AIM_NECK = 0.3
/** Falling faster than this with the chute shut, m/s, or for longer than `FREEFALL_AFTER` seconds, is a freefall rather than a jump. */
const FREEFALL_VY = 9
const FREEFALL_AFTER = 0.9
const FALL_RATE = 4
/**
 * Standing still, the legs stay put while the look turns, and the waist and chest take the difference
 * (`TWIST_SPINE` of it the waist, the rest the chest, so the gun points where the player looks). Past
 * `TWIST_LIMIT` radians the feet step round until the body has caught up to within `TWIST_SETTLED`.
 */
const TWIST_LIMIT = 1.0
const TWIST_SETTLED = 0.08
const TWIST_SPINE = 0.4
/** How far a skier's body takes the gun off the way the skis go, rad; past it the legs turn too. */
const SKI_TWIST = 1.2
const TURN_RATE = 9
/** Turning on the spot steps the feet as walking this fast would, m/s per rad/s of turn. */
const STEP_PER_TURN = 0.35
/** Idle glances: how far the head turns and nods, rad, and how long one holds and the gaps between, s. */
const GLANCE_YAW = 0.7
const GLANCE_PITCH = 0.2
const GLANCE_HOLD = [0.8, 2.2] as const
const GLANCE_GAP = [2.5, 7] as const
/** Settling the weight onto one leg and then the other, rad of hip roll, every few seconds. */
const SHIFT_ROLL = 0.05
const SHIFT_GAP = [3, 8] as const
/** Seconds stood still before glancing and shifting starts. */
const SETTLE_TIME = 1.5
/** Breathing hard after a run: how fast it comes on and goes, per second, and how deep, rad of chest. */
const EXERT_UP = 0.35
const EXERT_DOWN = 0.07
const BREATH_DEEP = 0.07
/** Leaning into a turn (rad per rad/s × m/s) and into a change of speed (rad per m/s²), at most `LEAN_MAX`. */
const TURN_LEAN = 0.025
const SPEED_LEAN = 0.02
const LEAN_MAX = 0.22
const LEAN_RATE = 6
/** A landing sinks into a crouch this much per m/s of fall, at most `LAND_MAX`. */
const LAND_DIP = 0.07
const LAND_MAX = 0.85
/** Hit reactions: the springs' stiffness and damping, and how hard a hit kicks, rad/s (`KICK_MIN` plus this per point lost). */
const SPRING_K = 260
const SPRING_C = 16
const KICK_PER_POINT = 0.22
const KICK_MIN = 3
const KICK_MAX = 14
const RECOIL_KICK = 0.09
const RECOIL_DECAY = 14
/**
 * Height of the head's middle above the model's origin in the `sit` clip: the hips come down 0.42 m
 * (`SIT_DROP` in build_character.py). A seated player is placed so their head lands on the seat's head point.
 */
export const SIT_HEAD = 1.22
/** Height of the head's middle above the model's origin standing: `SIT_HEAD` with the hips back up `SIT_DROP`. */
const STAND_HEAD = SIT_HEAD + 0.42
/** How far a seated body turns at the waist and chest towards where it aims, either way, radians. */
const SIT_TWIST = 1.7
/** Of that turn, the share the spine takes; the chest takes the rest. */
const SIT_SPINE = 0.4

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

/** A small seeded random source, so each soldier glances and shifts on its own time. */
function seeded(seed: number) {
  let a = (seed * 2654435761) >>> 0 || 1
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const wrap = (angle: number) => Math.atan2(Math.sin(angle), Math.cos(angle))

/** A damped spring about zero, kicked by hits. */
class Spring {
  x = 0
  v = 0
  step(dt: number) {
    // Small steps keep a stiff spring stable at a low frame rate.
    const n = Math.max(1, Math.ceil(dt * 120))
    const h = dt / n
    for (let i = 0; i < n; i++) {
      this.v += (-SPRING_K * this.x - SPRING_C * this.v) * h
      this.x += this.v * h
    }
  }
  reset() {
    this.x = 0
    this.v = 0
  }
}

/** The springs a hit kicks: a bone and axis, or the whole body's tilt. */
const SPRINGS = ['headX', 'headY', 'chestX', 'chestY', 'chestZ', 'armL', 'armR', 'legL', 'legR', 'tiltX', 'tiltZ'] as const
type SpringName = (typeof SPRINGS)[number]

const chase = (value: number, target: number, rate: number, dt: number) => value + (target - value) * (1 - Math.exp(-rate * dt))

const AXIS_X = new Vector3(1, 0, 0)
const AXIS_Y = new Vector3(0, 1, 0)
const AXIS_Z = new Vector3(0, 0, 1)
/**
 * The bones a wound can be pinned to, each with the bone at its far end: the segment between them is
 * the bone's length. The head has none; its segment runs `HEAD_LENGTH` along itself.
 */
const WOUND_BONES: [string, string | null][] = [
  ['hips', 'spine'],
  ['spine', 'chest'],
  ['chest', 'neck'],
  ['neck', 'head'],
  ['head', null],
  ...(['L', 'R'] as const).flatMap((side): [string, string][] => [
    [`upper_arm_${side}`, `forearm_${side}`],
    [`forearm_${side}`, `hand_${side}`],
    [`thigh_${side}`, `shin_${side}`],
    [`shin_${side}`, `foot_${side}`],
  ]),
]
const HEAD_LENGTH = 0.25

/**
 * The parts of a ragdoll (`crates/sim/src/ragdoll.rs`, in its order), each as the bones its segment
 * runs between: the second `null` for the head, which runs `HEAD_LENGTH` along itself.
 */
const RAGDOLL_SEGMENTS: readonly [string, string | null][] = [
  ['thigh_L', 'thigh_R'],
  ['spine', 'neck'],
  ['head', null],
  ['upper_arm_L', 'forearm_L'],
  ['forearm_L', 'hand_L'],
  ['upper_arm_R', 'forearm_R'],
  ['forearm_R', 'hand_R'],
  ['thigh_L', 'shin_L'],
  ['shin_L', 'foot_L'],
  ['thigh_R', 'shin_R'],
  ['shin_R', 'foot_R'],
]
/** The bones a ragdoll drives, parents first, each with the part it rides on. The rest keep their pose. */
const RAGDOLL_BONES: readonly [string, number][] = [
  ['hips', 0],
  ['spine', 1],
  ['chest', 1],
  ['neck', 1],
  ['head', 2],
  ['upper_arm_L', 3],
  ['forearm_L', 4],
  ['upper_arm_R', 5],
  ['forearm_R', 6],
  ['thigh_L', 7],
  ['shin_L', 8],
  ['thigh_R', 9],
  ['shin_R', 10],
]
/** Floats per part in a ragdoll's pose: position, then rotation x, y, z, w. */
const RAGDOLL_POSE = 7
/** Seconds getting up takes, and over how many of them it eases out of the pose the ragdoll left. */
const GETUP_TIME = 1.1
const GETUP_BLEND = 0.3

/** Part `part` of ragdoll pose `pose` as a matrix, into `out`. */
function partMatrix(pose: ArrayLike<number>, part: number, out = new Matrix4()): Matrix4 {
  const o = part * RAGDOLL_POSE
  return out.compose(
    SCRATCH.at.set(pose[o]!, pose[o + 1]!, pose[o + 2]!),
    SCRATCH.q.set(pose[o + 3]!, pose[o + 4]!, pose[o + 5]!, pose[o + 6]!),
    SCRATCH.one,
  )
}

/** How far `p` is from the segment `a`–`b`. */
function toSegment(p: Vector3, a: Vector3, b: Vector3): number {
  const ab = SCRATCH.axis.subVectors(b, a)
  const t = Math.min(1, Math.max(0, SCRATCH.forward.subVectors(p, a).dot(ab) / Math.max(ab.lengthSq(), 1e-6)))
  return a.clone().addScaledVector(ab, t).distanceTo(p)
}

const SCRATCH = {
  at: new Vector3(),
  one: new Vector3(1, 1, 1),
  size: new Vector3(),
  matrix: new Matrix4(),
  q: new Quaternion(),
  parent: new Quaternion(),
  axis: new Vector3(),
  forward: new Vector3(),
  foot: new Vector3(),
  turn: new Quaternion(),
}

/**
 * One player: a skinned clone of the model, placed at its capsule centre and animated from where it
 * is moving relative to where it faces, so strafing and backpedalling play their own clips.
 */
/** How long a hit whitens a character, seconds. */
const HIT_FLASH = 0.14

export class Character {
  /** The capsule centre, as last placed. */
  readonly position = new Vector3()
  private readonly object: Node
  private readonly animator: Animator
  private readonly actions: Animator['actions']
  private readonly weights = new Map<string, number>()
  private readonly outfit: Material
  private readonly chest: Node | null
  private readonly spine: Node | null
  private readonly hips: Node | null
  private readonly neck: Node | null
  /** Forearms and shins, in `LIMB_*` bit order, for hit reactions. */
  private readonly lowerLimbs: (Node | null)[]
  /** `WOUND_BONES` as found in the model. */
  private readonly woundBones: [Node, Node | null][]
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
  /** The bone each limb hangs from, in `LIMB_*` bit order: left arm, right arm, left leg, right leg. */
  private readonly limbBones: (Node | null)[]
  /** The right hand, which holds the gun: kept its size when the right arm is gone. */
  private readonly gunHand: Node | null
  /** The limbs it has lost (`LIMB_*`), and whether it has been told yet. */
  private lostLimbs = 0
  private limbsKnown = false
  /** A body thrown by its killing blow: where it has got to, relative to where it died. */
  /** Knocked down: the ragdoll's parts drive these bones, each at its offset from the part it rides on. */
  private ragdoll: { bones: { bone: Node; part: number; offset: Matrix4 }[]; front: Vector3 } | null = null
  /** Getting up after a knockdown: the clip, how long it has played, and where each ragdoll bone was. */
  private rising: { clip: string; time: number; from: Matrix4[] } | null = null
  /** Bones by name. */
  private readonly bones = new Map<string, Node>()
  private fling: { velocity: Vector3; spin: Vector3; offset: Vector3; groundAt: (x: number, z: number) => number } | null = null
  private readonly tumble = new Quaternion()
  private readonly canopy: Canopy
  /** The backpack on its back (BACKPACK.md), made the first time it carries it. */
  private backpack: Group | null = null
  private carrying = false
  /** The lamp on its head, made the first time it is lit. */
  private headlamp: Headlamp | null = null
  private lampOn = false
  /** The head bone's turn from the model's, unanimated: undoing it lays the lamp along the face. */
  private readonly headRest = new Quaternion()
  private readonly beam = { at: new Vector3(), direction: new Vector3() }
  /** Its skis and poles, made the first time it wears them. */
  private gear: { skis: [Group, Group]; poles: [Group, Group] } | null = null
  /** The golf club in its right hand, made when it first holds one, and which club that is. */
  private club: Group | null = null
  private clubItem = 0
  /** The `golf_swing` phase the club is held at while the swing is made on the meter; seconds into a swing (-1 for none), and the phase it was struck from. */
  private clubHeld = 0
  private swingTime = -1
  private swungFrom = 0
  /** Where the `golf_swing` clip stands, 0..1, and how much the golf stance has taken the body over. */
  private golfPhase = 0
  private golf = 0
  /** Wearing skis, and coasting with the weapon up (aim held) rather than poling. */
  private onSkis = false
  private skiAiming = false
  /** How far the body has gone over to skiing with a gun up and the poles stowed, 0 to 1. */
  private skiGun = 0
  /** Poling hard (run held), and how far the clips have gone over to it, 0 to 1. */
  private skiHard = false
  private poling = 0
  private frameDt = 0
  /** How far into a herringbone it is, 0 to 1: skis splayed, the body staying up over the hill. */
  private herring = 0
  /** How far the body has gone over from the walking clips to the skiing ones, 0 to 1. */
  private ski = 0
  /** Smoothed: how fast the ground rises under it along the way it goes (rise over run), and how fast it is slowing. */
  private skiRise = 0
  private skiBraking = 0
  private skiLast: { y: number; speed: number } | null = null
  /** Freefalling with the chute shut, and hanging under it, 0..1; and seconds off the ground. */
  private fall = 0
  private hang = 0
  /** Hands up, on its feet: the arms of the `chute` clip, with nothing in them. */
  private handsUp = false
  private airTime = 0
  /** The fastest fall since leaving the ground, m/s (negative), for the landing. */
  private fallSpeed = 0
  private wasGrounded = true
  /** A clock for the falling clips. */
  private clock = 0
  /** Where the legs face, which lags the look standing still; null until first placed. */
  private bodyYaw: number | null = null
  /** Stepping round on the spot until the body has caught up with the look. */
  private turning = false
  /** Seconds stood still, for when the idle life starts. */
  private stillFor = 0
  private readonly random: () => number
  private readonly glance = { yaw: 0, pitch: 0, toYaw: 0, toPitch: 0, left: 0, on: false }
  private readonly shift = { roll: 0, to: 0, left: 0 }
  /** How hard it is breathing, 0..1, and the breath's phase. */
  private exertion = 0
  private breath = 0
  /** Leaning into a turn (roll) and a change of speed (pitch), rad; last frame's velocity and facing. */
  private leanRoll = 0
  private leanPitch = 0
  private lastVelocity: [number, number] | null = null
  private lastYaw = 0
  private readonly springs = Object.fromEntries(SPRINGS.map((name) => [name, new Spring()])) as Record<SpringName, Spring>
  /** The `struck` count last seen, null until the first word. */
  private struckCount: number | null = null

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
    this.hips = this.object.getObjectByName('hips') ?? null
    this.neck = this.object.getObjectByName('neck') ?? null
    this.lowerLimbs = ['forearm_L', 'forearm_R', 'shin_L', 'shin_R'].map((name) => this.object.getObjectByName(name) ?? null)
    this.random = seeded(id + 1)
    this.head =this.object.getObjectByName('head') ?? null
    if (this.head) {
      this.object.updateWorldMatrix(true, true)
      this.head.getWorldQuaternion(this.headRest).premultiply(this.object.getWorldQuaternion(new Quaternion()).invert())
    }
    this.limbBones = LIMB_BONES.map((name) => this.object.getObjectByName(name) ?? null)
    this.object.traverse((node) => {
      if (node.name && !this.bones.has(node.name)) this.bones.set(node.name, node)
    })
    this.woundBones = WOUND_BONES.flatMap(([name, end]): [Node, Node | null][] => {
      const bone = this.object.getObjectByName(name)
      return bone ? [[bone, end === null ? null : (this.object.getObjectByName(end) ?? null)]] : []
    })
    this.gunHand = this.object.getObjectByName('hand_R') ?? null
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

  /** Whether it is drawn: false while it sits inside armour, or looks down its own scope. */
  get visible() {
    return this.shown
  }

  /** Drawn and in one piece: what a ragdoll can be made of. */
  get intact() {
    return this.shown && !this.gibbed
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

  /** Repaints the outfit: the player's chosen colour can arrive after their soldier. */
  setColor(hex: number) {
    this.outfit.color.setHex(hex)
    this.canopy.setColor(hex)
    this.outfit.needsUpdate()
  }

  update(
    position: readonly number[], yaw: number, vx: number, vz: number, grounded: boolean, dt: number, pitch = 0, crouching = false, sliding = false, prone = false, chute = false, vy = 0,
  ) {
    this.position.set(position[0]!, position[1]!, position[2]!)
    this.pitch = pitch
    this.fadeFlash(dt)
    if (!this.ragdoll) this.flingStep(dt)
    if (!this.shown || this.gibbed) return
    // A ragdoll poses it (`driveRagdoll`); nothing here does.
    if (this.ragdoll) {
      this.canopy.update(false, dt)
      return
    }

    this.clock += dt
    this.canopy.update(chute && !this.down, dt)

    // Off the ground long enough, or falling fast enough, with the chute shut: a freefall.
    this.airTime = grounded ? 0 : this.airTime + dt
    if (!grounded) this.fallSpeed = Math.min(this.fallSpeed, vy)
    const falling = !grounded && !chute && !this.down && (vy < -FREEFALL_VY || this.airTime > FREEFALL_AFTER)
    this.fall = chase(this.fall, falling ? 1 : 0, FALL_RATE, dt)
    this.hang = chase(this.hang, ((chute && !grounded) || this.handsUp) && !this.down ? 1 : 0, FALL_RATE, dt)
    // A landing sinks into a crouch as deep as the fall was hard, and comes back up.
    if (grounded && !this.wasGrounded && !this.down) this.crouch = Math.max(this.crouch, Math.min(LAND_MAX, -this.fallSpeed * LAND_DIP))
    if (grounded) this.fallSpeed = 0
    this.wasGrounded = grounded

    // Where the legs face: standing still they stay put while the look turns, until it has turned too
    // far; otherwise they face where it looks.
    const speedNow = Math.hypot(vx, vz)
    // Over the ball the golfer stands square to where they aim, the target on their left.
    const free = speedNow > STILL_SPEED || !grounded || prone || this.lie > 0.3 || this.down || sliding || crouching || this.golf > 0.01
    let body = this.bodyYaw ?? yaw
    const gap = wrap(yaw - body)
    if (this.rising) this.turning = false
    else if (free || Math.abs(gap) > TWIST_LIMIT) this.turning = true
    else if (Math.abs(gap) < TWIST_SETTLED) this.turning = false
    const before = body
    if (this.turning) body = wrap(body + gap * (1 - Math.exp(-(free ? BLEND_RATE * 2 : TURN_RATE) * dt)))
    this.bodyYaw = body
    // Turning on the spot steps the feet round, sideways the way it turns.
    const turnRate = dt > 0 ? wrap(body - before) / dt : 0
    const stepping = free ? 0 : Math.abs(turnRate) * STEP_PER_TURN

    // Velocity in the body's frame: forward is -Z at yaw 0, right is +X.
    const sin = Math.sin(body)
    const cos = Math.cos(body)
    const forward = stepping > 0 ? 0 : -(vx * sin + vz * cos)
    const right = stepping > 0 ? (turnRate > 0 ? -stepping : stepping) : vx * cos - vz * sin
    const speed = Math.hypot(forward, right)

    this.air = chase(this.air, !grounded && !this.down ? 1 : 0, AIR_RATE, dt)
    this.death = chase(this.death, this.down ? 1 : 0, DEATH_RATE, dt)
    this.crouch = chase(this.crouch, crouching ? 1 : 0, CROUCH_RATE, dt)
    this.lie = chase(this.lie, prone ? 1 : 0, LIE_RATE, dt)
    if (this.backpack) {
      // Between the shoulder blades, lower as it crouches; lying down, falling flat or dead it is hidden rather than posed.
      this.backpack.visible = this.carrying && !this.down && this.lie < 0.5 && this.fall < 0.5
      this.backpack.position.set(0, PACK_Y - PACK_CROUCH_DROP * this.crouch, -PACK_BACK)
    }
    const alive = 1 - this.death
    const ground = alive * (1 - this.air) * (this.handsUp ? 1 - this.hang : 1)
    this.slide = chase(this.slide, sliding && crouching ? 1 : 0, SLIDE_RATE, dt)
    // Lying down is its own stance: the crouch and slide clips give way to the crawl clips.
    this.frameDt = dt
    this.ski = chase(this.ski, this.onSkis && !this.gunUp && !this.down ? 1 : 0, BLEND_RATE, dt)
    this.skiGun = chase(this.skiGun, this.onSkis && this.gunUp && !this.down ? 1 : 0, BLEND_RATE, dt)
    this.senseSlope(grounded, speedNow, dt)
    const walking = Math.max(0, 1 - this.ski - this.skiGun)
    const standing = ground * walking * (1 - this.crouch) * (1 - this.lie)
    const low = ground * walking * this.crouch * (1 - this.slide) * (1 - this.lie)
    const crawl = ground * this.lie
    const sliding_ = ground * walking * this.crouch * this.slide * (1 - this.lie)

    // How much of the standing weight is locomotion, and how much of that is a run.
    const moving = this.onSkis ? 0 : smoothstep(STILL_SPEED, FULL_STEP, speed)
    const running = smoothstep(WALK_SPEED + 0.4, RUN_SPEED - 0.6, speed)
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
    // With a club, standing still is standing over the ball; a swing is played out wherever it goes.
    this.swingOn(dt)
    const golfing = this.golfer && !crouching && !prone
    const addressing = golfing ? (this.swingTime >= 0 ? 1 : 1 - moving) : 0
    targets.set('golf_swing', standing * addressing)
    this.golf = chase(this.golf, standing * addressing, BLEND_RATE, dt)
    set('idle', standing * (1 - moving) * (1 - addressing))
    set('crouch_idle', low * (1 - moving))
    set('crawl_idle', crawl * (1 - moving))
    set('slide', sliding_)
    // In the air: a jump, a freefall, or hanging under the canopy. The falling clips are every stance's.
    const aloft = alive * this.air
    set('jump_air', aloft * walking * (1 - this.fall) * (1 - this.hang))
    const skiing = this.ski + this.skiGun
    if (skiing > 0) this.skiTargets(targets, ground * skiing * (1 - this.lie), aloft * skiing * (1 - this.fall) * (1 - this.hang), speedNow)
    targets.set('freefall', aloft * this.fall * (1 - this.hang))
    targets.set('chute', (this.handsUp ? alive : aloft) * this.hang)
    targets.set('death', this.death)
    // Getting up is all there is until it is up; then the stance's clips take over again.
    const rising = this.rising
    if (rising) {
      rising.time += dt
      if (rising.time >= GETUP_TIME) this.rising = null
      else {
        for (const name of targets.keys()) targets.set(name, 0)
        targets.set(rising.clip, 1)
        for (const name of this.actions.keys()) this.weights.set(name, name === rising.clip ? 1 : 0)
      }
    }

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
    // Poling hard, the arms go flat out from the first stride, however little it has got going yet.
    const paced = Math.max(this.poling > 0 ? speed + (Math.max(speed, HARD_PACE) - speed) * this.poling : speed, HERRING_PACE * this.herring)
    if (weightSum > 0 && stride > 0) this.phase = (this.phase + (dt * paced * weightSum) / stride) % 1
    const idle = this.actions.get('idle')
    if (idle) this.idleTime = (this.idleTime + dt) % idle.clip.duration

    const current = this.clip('idle')
    for (const [name, action] of this.actions) {
      const w = this.weights.get(name)!
      const base = name.replace(/^(pistol_|hold_|shoulder_)/, '')
      action.enabled = w > 0.0005 || name === current
      action.weight = w
      if (base === 'idle' || base === 'crouch_idle' || base === 'crawl_idle' || base === 'slide' || base === 'ski_glide' || base === 'ski_tuck' || base === 'ski_plow' || base === 'armed_glide' || base === 'armed_tuck' || base === 'armed_plow') action.time = this.idleTime % action.clip.duration
      else if (base === 'ski_air' || base === 'armed_air') action.time = this.clock % action.clip.duration
      else if (base === 'jump_air') action.time = (this.idleTime / (idle?.clip.duration ?? 1)) * action.clip.duration
      else if (base === 'freefall' || base === 'chute') action.time = this.clock % action.clip.duration
      else if (base === 'getup_back' || base === 'getup_front') action.time = Math.min(1, (this.rising?.time ?? GETUP_TIME) / GETUP_TIME) * action.clip.duration
      else if (base === 'golf_swing') action.time = this.golfPhase * action.clip.duration
      else if (base !== 'death') action.time = this.phase * action.clip.duration
    }
    // The death clip plays out on the animator's clock once it starts.
    const dying = this.actions.get('death')
    if (dying) dying.timeScale = this.down ? 1 : 0

    this.animator.update(dt)
    this.head?.scale.setScalar(this.headless ? 0.001 : 1)
    this.hideLimbs()
    this.recoil = chase(this.recoil, 0, RECOIL_DECAY, dt)
    this.aim()
    // The waist and chest take what the legs have not turned, so the gun points where it looks.
    // On skis with a gun the legs go the way the skis do and the torso alone carries it to the look.
    const heading = speedNow > 0.5 ? Math.atan2(-vx, -vz) : body
    const skiTurn = this.skiGun * (1 - this.air) * wrap(yaw + Math.max(-SKI_TWIST, Math.min(SKI_TWIST, wrap(heading - yaw))) - body)
    const twist = wrap(yaw - body - skiTurn) * alive * (1 - this.lie) * (1 - this.air) * (1 - this.golf) * (this.rising ? 0 : 1)
    this.turn(this.spine, twist * TWIST_SPINE, AXIS_Y)
    this.turn(this.chest, twist * (1 - TWIST_SPINE), AXIS_Y)
    this.live(dt, speedNow, standing * (1 - moving), alive)
    this.react(dt)
    this.lean(dt, vx, vz, body, ground * (1 - this.lie))
    // A slide carries the body along its heading, rifle and all, rather than where the camera looks.
    const turn = Math.atan2(Math.sin(heading - body), Math.cos(heading - body))
    this.place(body + skiTurn + turn * Math.max(this.slide, this.ski))
    this.easeUp()
    if (this.onSkis) this.fitSkis()
  }

  /** How the ground rises and how fast it is slowing, from how it moved: enough to choose a gait, and it needs nothing sent. */
  private senseSlope(grounded: boolean, speed: number, dt: number) {
    const last = this.skiLast
    if (this.onSkis && last && dt > 0) {
      const rise = grounded && speed > 0.5 ? Math.max(-1, Math.min(1, (this.position.y - last.y) / (speed * dt))) : this.skiRise * 0.9
      this.skiRise = chase(this.skiRise, rise, SKI_SENSE_RATE, dt)
      this.skiBraking = chase(this.skiBraking, (last.speed - speed) / dt, SKI_SENSE_RATE, dt)
    }
    this.skiLast = this.onSkis ? { y: this.position.y, speed } : null
  }

  /** The skiing clips' share of `ground` and `aloft`: the gait is told from the speed, the slope and the crouch. */
  private skiTargets(targets: Map<string, number>, ground: number, aloft: number, speed: number) {
    const tuck = this.crouch
    // The legs do the same either way; what the arms hold is the share of each set of clips.
    const gun = this.skiGun / (this.ski + this.skiGun)
    const both = (poles: string, armed: string, weight: number) => {
      targets.set(poles, weight * (1 - gun))
      targets.set(this.clip(armed), weight * gun)
    }
    const plow = (1 - tuck) * smoothstep(SKI_PLOW_FROM, SKI_PLOW_FULL, this.skiBraking) * smoothstep(1, 3, speed)
    const rest = ground * (1 - tuck) * (1 - plow)
    const coasting = 1 - smoothstep(-0.08, -0.03, this.skiRise) * smoothstep(0.1, 0.8, speed)
    const pushing = rest * (1 - coasting)
    const climbing = smoothstep(HERRING_RISE - 0.05, HERRING_RISE, this.skiRise) * (1 - smoothstep(3, 5, speed))
    this.herring = chase(this.herring, climbing * ground, BLEND_RATE, this.frameDt)
    const poles = (1 - climbing) * smoothstep(SKI_POLE_FROM - 1, SKI_POLE_FROM + 1, speed)
    both('ski_plow', 'armed_plow', ground * (1 - tuck) * plow)
    both('ski_glide', 'armed_glide', rest * coasting)
    both('ski_climb', 'armed_climb', pushing * climbing)
    // Past the speed where poling gives anything, a skier stops throwing themselves at it.
    this.poling = chase(this.poling, this.skiHard && speed < SKI_BOOST_TOP && climbing < 0.5 ? 1 : 0, BLEND_RATE, this.frameDt)
    const stride = pushing * (1 - climbing) * (1 - poles)
    const double = pushing * poles
    targets.set('ski_pole', double * (1 - this.poling) * (1 - gun))
    targets.set('ski_pole_hard', double * this.poling * (1 - gun))
    targets.set('ski_stride', stride * (1 - this.poling) * (1 - gun))
    targets.set('ski_stride_hard', stride * this.poling * (1 - gun))
    targets.set(this.clip('armed_skate'), pushing * (1 - climbing) * gun)
    // A tuck holds through the air.
    both('ski_tuck', 'armed_tuck', ground * tuck + aloft * tuck)
    both('ski_air', 'armed_air', aloft * (1 - tuck))
  }

  /** Lays each ski along the ground under its foot, whatever the foot is doing: only the heel lifts off. */
  private fitSkis() {
    const gear = this.gear
    if (!gear) return
    this.object.updateWorldMatrix(true, true)
    const level = SCRATCH.q.copy(this.object.quaternion)
    if (this.object.parent) this.object.getWorldQuaternion(level)
    const inverse = SCRATCH.turn
    gear.skis.forEach((ski, i) => {
      const foot = ski.parent
      if (!foot) return
      foot.getWorldQuaternion(inverse).invert()
      // The V of a herringbone, tips out and tails in, each ski lying on the hill as the body does not.
      SKI_V.setFromAxisAngle(AXIS_Y, this.herring * (i === 0 ? HERRING_SPLAY : -HERRING_SPLAY))
      SKI_TILT.setFromAxisAngle(AXIS_X, -this.herring * Math.atan(this.skiRise))
      ski.quaternion.copy(inverse).multiply(level).multiply(SKI_V).multiply(SKI_TILT)
    })
    // The snow holds a pole's tip through the push, so the hand coming down over it is what tips it back.
    // From there it swings on back and off the snow. The stride's and the climb's arms take turns.
    const weight = (name: string) => this.weights.get(name) ?? 0
    const double = weight('ski_pole') + weight('ski_pole_hard')
    const stride = weight('ski_stride') + weight('ski_stride_hard')
    const climb = weight('ski_climb')
    const up = SCRATCH.axis.set(0, 1, 0).applyQuaternion(level)
    const [skiL, skiR] = gear.skis
    const snow = Math.min(skiL.getWorldPosition(SCRATCH.at).dot(up), skiR.getWorldPosition(SCRATCH.foot).dot(up)) - SOLE_DROP
    gear.poles.forEach((pole, i) => {
      const hand = pole.parent
      if (!hand) return
      const left = i === 0
      const lift = double * poleLift(this.phase) + stride * poleLift(this.phase + (left ? 0.25 : 0.75)) + climb * poleLift(this.phase + (left ? 0.75 : 0.25))
      const height = hand.getWorldPosition(SCRATCH.at).dot(up) - snow
      const planted = Math.max(POLE_TILT + POLE_TUCK * this.crouch, Math.acos(Math.max(0, Math.min(1, height / POLE_REACH))))
      const tilt = SKI_TILT.setFromAxisAngle(AXIS_X, planted + (POLE_BACK - planted) * lift)
      hand.getWorldQuaternion(inverse).invert()
      pole.quaternion.copy(inverse).multiply(level).multiply(tilt).multiply(SKI_SPLAY.setFromAxisAngle(AXIS_Z, left ? POLE_SPLAY : -POLE_SPLAY))
    })
  }

  /** Just up from a ragdoll, eases each bone it drove out of where the ragdoll left it into the clip. */
  private easeUp() {
    const rising = this.rising
    if (!rising || rising.time >= GETUP_BLEND) return
    const k = rising.time / GETUP_BLEND
    const ease = k * k * (3 - 2 * k)
    this.object.updateWorldMatrix(true, true)
    const from = { at: new Vector3(), q: new Quaternion(), size: new Vector3() }
    const to = { at: new Vector3(), q: new Quaternion(), size: new Vector3() }
    RAGDOLL_BONES.forEach(([name], i) => {
      const bone = this.bones.get(name)
      const was = rising.from[i]
      if (!bone || !was) return
      was.decompose(from.at, from.q, from.size)
      new Matrix4().fromArray(bone.matrixWorld).decompose(to.at, to.q, to.size)
      this.setWorld(bone, new Matrix4().compose(from.at.lerp(to.at, ease), from.q.slerp(to.q, ease), to.size))
    })
  }

  /**
   * The small things that keep it from standing like a statue: breathing hard after a run, and, once it
   * has stood still a while, glancing about and shifting its weight. `idle` is how much it stands idle.
   */
  private live(dt: number, speed: number, idle: number, alive: number) {
    // Out of breath after a run, and getting it back slowly.
    this.exertion = Math.min(1, Math.max(0, this.exertion + (speed > WALK_SPEED + 0.5 ? EXERT_UP : -EXERT_DOWN) * dt))
    this.breath = (this.breath + dt * Math.PI * 2 * (0.3 + 0.6 * this.exertion)) % (Math.PI * 2)
    const heave = Math.sin(this.breath) * BREATH_DEEP * this.exertion * alive * (1 - this.air)
    this.turn(this.chest, -heave, AXIS_X)
    this.turn(this.neck, heave * 0.6, AXIS_X)

    this.stillFor = idle > 0.5 ? this.stillFor + dt : 0
    const settled = this.stillFor > SETTLE_TIME && !this.down
    const pick = ([a, b]: readonly [number, number]) => a + (b - a) * this.random()
    const glance = this.glance
    glance.left -= dt
    if (!settled) {
      glance.on = false
      glance.left = 0
      glance.toYaw = glance.toPitch = 0
    } else if (glance.left <= 0) {
      // Look somewhere for a moment, then back ahead for a while.
      glance.on = !glance.on
      glance.left = pick(glance.on ? GLANCE_HOLD : GLANCE_GAP)
      glance.toYaw = glance.on ? (this.random() * 2 - 1) * GLANCE_YAW : 0
      glance.toPitch = glance.on ? (this.random() * 2 - 1) * GLANCE_PITCH : 0
    }
    glance.yaw = chase(glance.yaw, glance.toYaw, 5, dt)
    glance.pitch = chase(glance.pitch, glance.toPitch, 5, dt)
    this.turn(this.neck, glance.yaw * 0.5, AXIS_Y)
    this.turn(this.head, glance.yaw * 0.5, AXIS_Y)
    this.turn(this.head, glance.pitch, AXIS_X)

    const shift = this.shift
    shift.left -= dt
    if (!settled) shift.to = 0
    else if (shift.left <= 0) {
      shift.left = pick(SHIFT_GAP)
      shift.to = shift.to > 0 ? -SHIFT_ROLL : SHIFT_ROLL
    }
    shift.roll = chase(shift.roll, shift.to * idle, 1.5, dt)
    // The hips roll onto one leg and the waist leans back over it, so the head stays level.
    this.turn(this.hips, shift.roll, AXIS_Z)
    this.turn(this.spine, -shift.roll * 1.4, AXIS_Z)
  }

  /** Leans the whole body into a turn and a change of speed, by `on` (on the ground, upright). */
  private lean(dt: number, vx: number, vz: number, yaw: number, on: number) {
    let roll = 0
    let pitch = 0
    if (this.lastVelocity && dt > 0) {
      const yawRate = wrap(yaw - this.lastYaw) / dt
      // Turning left (yaw growing) leans to its left, the model's +X: a negative roll about its +Z.
      roll = -yawRate * Math.hypot(vx, vz) * TURN_LEAN
      const ax = (vx - this.lastVelocity[0]) / dt
      const az = (vz - this.lastVelocity[1]) / dt
      pitch = -(ax * Math.sin(yaw) + az * Math.cos(yaw)) * SPEED_LEAN
    }
    this.lastVelocity = [vx, vz]
    this.lastYaw = yaw
    const clamp = (v: number) => Math.max(-LEAN_MAX, Math.min(LEAN_MAX, v)) * on
    this.leanRoll = chase(this.leanRoll, clamp(roll), LEAN_RATE, dt)
    this.leanPitch = chase(this.leanPitch, clamp(pitch), LEAN_RATE, dt)
  }

  /**
   * Its last hit, as `struck` replicates it: a new one makes it flinch, and is told back. The first
   * word is not news: it may have been hit before it came into view.
   */
  setStruck(struck: { count: number; part: number; amount: number; yaw: number } | undefined): boolean {
    if (!struck) {
      // A new life starts counting again.
      if (this.struckCount !== null) this.struckCount = 0
      return false
    }
    const fresh = this.struckCount !== null && struck.count !== this.struckCount
    this.struckCount = struck.count
    if (fresh) this.flinch(struck.part, struck.yaw, struck.amount)
    return fresh
  }

  /**
   * Kicks the springs for a hit on `part` (`PART_*`, a limb by its `LIMB_*` bit) that came from bearing
   * `from` in the world and took `amount`.
   */
  flinch(part: number, from: number, amount: number) {
    if (this.down || this.gibbed) return
    const kick = Math.min(KICK_MAX, KICK_MIN + amount * KICK_PER_POINT)
    // Where it came from, relative to the way the body faces: 0 ahead, + to its left.
    const rel = wrap(from - (this.bodyYaw ?? 0))
    const ahead = Math.cos(rel)
    const side = Math.sin(rel)
    const s = this.springs
    // Every hit rocks the body away from the blow a little.
    s.tiltX.v -= ahead * kick * 0.12
    s.tiltZ.v += side * kick * 0.12
    if (part === PART_HEAD) {
      s.headX.v -= ahead * kick
      s.headY.v -= side * kick * 0.8
      s.chestX.v -= ahead * kick * 0.3
    } else if (part === LIMB_ARM_L || part === LIMB_ARM_R) {
      // The struck shoulder is knocked back, twisting the chest that way.
      const left = part === LIMB_ARM_L
      ;(left ? s.armL : s.armR).v += kick
      s.chestY.v += (left ? -1 : 1) * kick * 0.4
    } else if (part === LIMB_LEG_L || part === LIMB_LEG_R) {
      // The knee buckles and the body drops onto it.
      ;(part === LIMB_LEG_L ? s.legL : s.legR).v += kick
      this.crouch = Math.max(this.crouch, Math.min(0.6, amount / 60))
      s.chestX.v += kick * 0.3
    } else {
      s.chestX.v -= ahead * kick * 0.7
      s.chestY.v -= side * kick * 0.4
      s.chestZ.v += side * kick * 0.4
    }
  }

  /** Steps the hit springs and bends the bones by them. */
  private react(dt: number) {
    for (const spring of Object.values(this.springs)) spring.step(dt)
    const s = this.springs
    this.turn(this.chest, s.chestX.x, AXIS_X)
    this.turn(this.chest, s.chestY.x, AXIS_Y)
    this.turn(this.chest, s.chestZ.x, AXIS_Z)
    this.turn(this.neck, s.headX.x * 0.4, AXIS_X)
    this.turn(this.head, s.headX.x * 0.6, AXIS_X)
    this.turn(this.head, s.headY.x, AXIS_Y)
    const [upperL, upperR, thighL, thighR] = this.limbBones
    const [, , shinL, shinR] = this.lowerLimbs
    this.turn(upperL ?? null, s.armL.x, AXIS_X)
    this.turn(upperR ?? null, s.armR.x, AXIS_X)
    // A buckled knee: the shin folds back and the thigh comes forward.
    this.turn(thighL ?? null, -s.legL.x * 0.5, AXIS_X)
    this.turn(shinL ?? null, s.legL.x, AXIS_X)
    this.turn(thighR ?? null, -s.legR.x * 0.5, AXIS_X)
    this.turn(shinR ?? null, s.legR.x, AXIS_X)
  }

  /**
   * Sits the character in a vehicle seat: its head on `head` (world), the body turned with the
   * vehicle's `rotation`, and the chest twisted towards `yaw` and leaning with `pitch`, where it aims.
   */
  seat(head: Vector3, rotation: Quaternion, yaw: number, pitch: number, dt: number, standing = false) {
    this.position.copy(head)
    this.pitch = pitch
    this.fadeFlash(dt)
    if (!this.shown || this.gibbed) return

    // Every other clip gives way to sitting, or standing still for a seat stood in, in the stance of the item in hand.
    const pose = standing ? 'idle' : 'sit'
    const sit = this.clip(pose)
    let sum = 0
    for (const name of this.actions.keys()) {
      const next = chase(this.weights.get(name) ?? 0, name === sit ? 1 : 0, BLEND_RATE, dt)
      this.weights.set(name, next)
      sum += next
    }
    if (sum > 0) for (const [name, w] of this.weights) this.weights.set(name, w / sum)
    const clip = this.actions.get(sit)
    if (clip) this.idleTime = (this.idleTime + dt) % clip.clip.duration
    for (const [name, action] of this.actions) {
      const w = this.weights.get(name)!
      action.enabled = w > 0.0005 || name === sit
      action.weight = w
      if (name.endsWith(pose)) action.time = this.idleTime
    }
    this.animator.update(dt)
    this.head?.scale.setScalar(this.headless ? 0.001 : 1)
    this.hideLimbs()
    this.recoil = chase(this.recoil, 0, RECOIL_DECAY, dt)
    this.aim()

    // The legs stay with the seat; the waist and chest turn towards the aim, as far as a body turns.
    const forward = SCRATCH.forward.set(0, 0, -1).applyQuaternion(rotation)
    const facing = Math.atan2(-forward.x, -forward.z)
    const twist = Math.max(-SIT_TWIST, Math.min(SIT_TWIST, Math.atan2(Math.sin(yaw - facing), Math.cos(yaw - facing))))
    this.turn(this.spine, twist * SIT_SPINE, AXIS_Y)
    this.turn(this.chest, twist * (1 - SIT_SPINE), AXIS_Y)

    // The model faces +Z; the vehicle's nose is its -Z.
    this.object.quaternion.copy(rotation).multiply(SCRATCH.turn.setFromAxisAngle(AXIS_Y, Math.PI))
    this.object.position.copy(head).sub(SCRATCH.axis.set(0, standing ? STAND_HEAD : SIT_HEAD, 0).applyQuaternion(rotation))
    this.placed = true
    this.object.visible = this.shown && !this.gibbed
  }

  /** Fades a hit's whitening. */
  private fadeFlash(dt: number) {
    if (this.flashLeft <= 0) return
    this.flashLeft = Math.max(0, this.flashLeft - dt)
    // Faint: the wound is what shows where it was hit; this only says it was.
    const glow = (this.flashLeft / HIT_FLASH) * 0.35
    this.outfit.emissive.set(glow, glow, glow)
    this.outfit.needsUpdate()
  }

  private place(yaw: number) {
    // Lying down, the centre is already lower; the pivot goes back up to the ground the feet stand on.
    this.object.position.set(this.position.x, this.position.y - FOOT_OFFSET + (STAND_CENTRE - PRONE_CENTRE) * this.lie, this.position.z)
    // The model faces +Z; the game's yaw 0 faces -Z.
    // Set all three: a tumble leaves its pitch and roll in the Euler angles, and `rotation.y` alone
    // would keep them, leaving a respawned player tilted.
    this.object.rotation.set(0, yaw + Math.PI, 0, 'YXZ')
    // Leaning into a turn or a change of speed, and rocked by a hit, about the feet: the model's +Z is
    // ahead, so a turn about its +X tips it forward and one about its +Z to its right.
    // On skis the body goes with the slope. Rising ground tips the model's nose (+Z ahead) up: a negative turn about +X.
    const tipX = this.leanPitch + this.springs.tiltX.x * 0.5 - this.ski * (1 - this.herring) * Math.atan(this.skiRise)
    const tipZ = this.leanRoll + this.springs.tiltZ.x * 0.5
    if (!this.fling && (tipX !== 0 || tipZ !== 0)) {
      this.object.quaternion.multiply(SCRATCH.turn.setFromAxisAngle(AXIS_X, tipX)).multiply(SCRATCH.q.setFromAxisAngle(AXIS_Z, tipZ))
    }
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
  launch(velocity: Vector3, spin: Vector3, groundAt: (x: number, z: number) => number) {
    // A ragdoll already carries it.
    if (this.ragdoll) return
    this.fling = { velocity: velocity.clone(), spin: spin.clone(), offset: new Vector3(), groundAt }
    this.tumble.identity()
  }

  /**
   * The limbs it has lost now, from its `limbs` (`LIMB_*`; the bleeding bits above are ignored), and
   * which of them are new since it was last told: none the first time, or once it is in pieces. The
   * round that kills can take a limb too, so a body lying dead still loses one.
   */
  setLimbs(wire: number): number {
    const lost = wire & LIMB_ALL
    const gained = this.limbsKnown && !this.gibbed ? lost & ~this.lostLimbs : 0
    this.lostLimbs = lost
    this.limbsKnown = true
    return gained
  }

  /** The limbs it has lost (`LIMB_*`). */
  get limbs() {
    return this.lostLimbs
  }

  /**
   * Pins world `point` (on the body as it stands now) to the bone it lies on: the function returned
   * gives where that spot is from then on, as the body moves, bends and falls, into `out`.
   */
  pin(point: Vector3): (out?: Vector3) => Vector3 {
    this.object.updateWorldMatrix(true, true)
    let nearest: Node | null = null
    let best = Infinity
    const a = new Vector3()
    const b = new Vector3()
    for (const [bone, end] of this.woundBones) {
      bone.getWorldPosition(a)
      if (end) end.getWorldPosition(b)
      else bone.localToWorld(b.set(0, HEAD_LENGTH, 0))
      const distance = toSegment(point, a, b)
      if (distance < best) {
        best = distance
        nearest = bone
      }
    }
    if (!nearest) {
      const offset = point.clone().sub(this.position)
      return (out = new Vector3()) => out.copy(this.position).add(offset)
    }
    const bone = nearest
    const local = bone.worldToLocal(point.clone())
    return (out = new Vector3()) => {
      this.object.updateWorldMatrix(true, true)
      return bone.localToWorld(out.copy(local))
    }
  }

  /** Whether a ragdoll is driving it. */
  get ragdolled() {
    return this.ragdoll !== null
  }

  /**
   * What a ragdoll of it as it stands now is made of (`ClientSim.spawnRagdoll`): each part's segment,
   * six numbers from one joint to the next, and its right in the world, the way elbows and knees bend.
   */
  ragdollSegments(): { segments: Float32Array; side: [number, number, number] } {
    this.object.updateWorldMatrix(true, true)
    const segments = new Float32Array(RAGDOLL_SEGMENTS.length * 6)
    const a = new Vector3()
    const b = new Vector3()
    RAGDOLL_SEGMENTS.forEach(([from, to], i) => {
      const start = this.bones.get(from)
      if (start) start.getWorldPosition(a)
      else a.copy(this.position)
      const end = to === null ? null : this.bones.get(to)
      if (end) end.getWorldPosition(b)
      else if (start && to === null) start.localToWorld(b.set(0, HEAD_LENGTH, 0))
      else b.copy(a).add(SCRATCH.at.set(0, 0.2, 0))
      segments.set([a.x, a.y, a.z, b.x, b.y, b.z], i * 6)
    })
    // The model's right is its -X.
    const side = new Vector3(-1, 0, 0).applyQuaternion(this.worldRotation(this.object))
    return { segments, side: [side.x, side.y, side.z] }
  }

  /**
   * Hands it over to a ragdoll whose parts are at `pose` (`ClientSim.ragdollPose`), made from
   * `ragdollSegments` a moment ago: from now on `driveRagdoll` poses it, until `endRagdoll`.
   */
  startRagdoll(pose: ArrayLike<number>) {
    this.object.updateWorldMatrix(true, true)
    const bones: { bone: Node; part: number; offset: Matrix4 }[] = []
    for (const [name, part] of RAGDOLL_BONES) {
      const bone = this.bones.get(name)
      if (!bone) continue
      const offset = partMatrix(pose, part).invert().multiply(new Matrix4().fromArray(bone.matrixWorld))
      bones.push({ bone, part, offset })
    }
    // Which way the chest faces, as the chest sees it: whether it lands on its back or its belly.
    const chest = this.bones.get('chest') ?? this.object
    const front = new Vector3(0, 0, 1).applyQuaternion(this.worldRotation(this.object)).applyQuaternion(this.worldRotation(chest).invert())
    this.ragdoll = { bones, front }
    this.rising = null
    this.fling = null
    this.tumble.identity()
  }

  /** Poses it as its ragdoll's parts are now (`ClientSim.ragdollPose`). */
  driveRagdoll(pose: ArrayLike<number>) {
    if (!this.ragdoll || pose.length < RAGDOLL_SEGMENTS.length * RAGDOLL_POSE) return
    // The model's root goes along over the pelvis, so it is drawn where the body is.
    this.object.position.set(pose[0]!, pose[1]! - STAND_CENTRE, pose[2]!)
    this.position.set(pose[0]!, pose[1]!, pose[2]!)
    this.object.updateWorldMatrix(true, true)
    for (const { bone, part, offset } of this.ragdoll.bones) this.setWorld(bone, partMatrix(pose, part, SCRATCH.matrix).multiply(offset))
  }

  /**
   * The ragdoll lets go. Alive, it gets up: off its back or its belly, whichever it lies on, facing
   * the way its body lies, easing out of the pose the ragdoll left. Otherwise (a respawn) it is simply
   * let go.
   */
  endRagdoll(rise = true) {
    const ragdoll = this.ragdoll
    if (!ragdoll) return
    this.ragdoll = null
    if (!rise || this.down) return
    const from = ragdoll.bones.map(({ bone }) => new Matrix4().fromArray(bone.matrixWorld))
    const chest = this.bones.get('chest') ?? this.object
    const faceUp = ragdoll.front.clone().applyQuaternion(this.worldRotation(chest)).y > 0
    const head = (this.bones.get('head') ?? this.object).getWorldPosition(new Vector3())
    const hips = (this.bones.get('hips') ?? this.object).getWorldPosition(new Vector3())
    // On its back the clip starts with the head behind it; on its belly, ahead.
    const along = head.sub(hips).setY(0).multiplyScalar(faceUp ? -1 : 1)
    if (along.lengthSq() > 1e-4) this.bodyYaw = Math.atan2(-along.x, -along.z)
    const clip = faceUp ? 'getup_back' : 'getup_front'
    if (!this.actions.has(clip)) return
    this.rising = { clip, time: 0, from }
  }

  /** Gives `bone` the world transform `world`, keeping its scale, and carries it to its children. */
  private setWorld(bone: Node, world: Matrix4) {
    const parent = bone.parent ? new Matrix4().fromArray(bone.parent.matrixWorld).invert() : new Matrix4()
    parent.multiply(world).decompose(SCRATCH.at, SCRATCH.q, SCRATCH.size)
    bone.position.set(SCRATCH.at.x, SCRATCH.at.y, SCRATCH.at.z)
    bone.quaternion.set(SCRATCH.q.x, SCRATCH.q.y, SCRATCH.q.z, SCRATCH.q.w)
    bone.updateWorldMatrix(false, true)
  }

  /** `node`'s rotation in the world. */
  private worldRotation(node: Node): Quaternion {
    return node.getWorldQuaternion(new Quaternion())
  }

  /** Where the stump of `limb` (a `LIMB_*` bit) is in the world: the shoulder or the hip. */
  stump(limb: number, out = new Vector3()): Vector3 {
    const bone = this.limbBones[Math.log2(limb)]
    if (!bone) return out.copy(this.position)
    this.object.updateWorldMatrix(true, true)
    return bone.getWorldPosition(out)
  }

  /**
   * Shrinks each lost limb to nothing at its root bone. The gun is skinned to the right hand, so with
   * the right arm gone that hand is grown back to size: the gun stays, held at the shoulder.
   */
  private hideLimbs() {
    this.limbBones.forEach((bone, i) => bone?.scale.setScalar(this.lostLimbs & (1 << i) ? 0.001 : 1))
    this.gunHand?.scale.setScalar(this.lostLimbs & LIMB_ARM_R ? 1000 : 1)
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
        // A limb already off has been thrown.
        if (!only && this.lostLimbs & limbBit(limb.key)) continue
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
    // Falling flat or hanging in the harness, there is nothing to aim.
    // A golfer's look is down at the ball, which the swing clip already has.
    const held = (1 - Math.max(this.fall, this.hang)) * (1 - this.golf)
    const lean = (-this.pitch - this.recoil) * held
    this.turn(this.spine, lean * AIM_SPINE, AXIS_X)
    this.turn(this.chest, lean * AIM_CHEST, AXIS_X)
    this.turn(this.neck, -this.pitch * AIM_NECK * held, AXIS_X)
  }

  /** Rotates a bone about `axis` in model space: X bends it forward and back, Y turns it. */
  private turn(bone: Node | null, angle: number, axis: Vector3) {
    if (!bone || Math.abs(angle) < 1e-4) return
    // The bone's parent frame relative to the model root, to carry the model-space axis down into it.
    const parent = SCRATCH.parent.identity()
    for (let node = bone.parent; node && node !== this.object; node = node.parent) {
      parent.premultiply(SCRATCH.q.set(node.quaternion.x, node.quaternion.y, node.quaternion.z, node.quaternion.w))
    }
    SCRATCH.axis.copy(axis).applyQuaternion(parent.invert())
    bone.quaternion.premultiply(SCRATCH.q.setFromAxisAngle(SCRATCH.axis, angle))
  }

  /** Kicks the rifle back; call when this player fires. */
  fire() {
    this.recoil = RECOIL_KICK
  }

  /** Swings the golf club in hand through; call when this player strikes. */
  swing() {
    if (this.swingTime >= 0 && this.swingTime < 0.3) return
    this.swungFrom = this.clubHeld > 0 ? this.golfPhase : 0
    this.swingTime = 0
    this.clubHeld = 0
  }

  /** The `golf_swing` phase to hold the club at while the swing is being made (`golfclub.backswingPhase`, `downswingPhase`); 0 at the address. */
  set clubPose(phase: number) {
    if (this.swingTime < 0) this.clubHeld = phase
  }

  /** Whether it holds a golf club as one: on its feet, not on skis. */
  private get golfer() {
    return itemDef(this.item)?.kind === 'club' && !this.onSkis && !this.handsUp
  }

  /** Moves the swing on: the clip's phase from the charge, or from how far the swing has got. */
  private swingOn(dt: number) {
    if (this.swingTime >= 0) {
      this.swingTime += dt
      if (this.swingTime >= swingLength(this.swungFrom)) {
        // The clip ends at the address it starts from: go back to its start rather than easing there,
        // which would play the whole swing backwards.
        this.swingTime = -1
        this.golfPhase = 0
        return
      }
    }
    this.golfPhase = this.swingTime >= 0 ? swingPhase(this.swingTime, this.swungFrom) : chase(this.golfPhase, this.clubHeld, 12, dt)
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
    // The poles fill both hands: on skis the weapon shows only instead of them.
    if (mesh) mesh.visible = !this.handsUp && (!this.onSkis || this.gunUp)
    const golf = itemDef(this.item)?.kind === 'club'
    if (golf && this.clubItem !== this.item && this.gunHand) {
      this.club?.removeFromParent()
      // Along the hand, as the swing clip lays the hand along the club.
      this.club = buildClub(this.item)
      this.gunHand.add(this.club)
      this.clubItem = this.item
      this.swingTime = -1
      this.clubHeld = 0
    }
    if (this.club) this.club.visible = golf && !this.handsUp && !this.onSkis
  }

  set surrendering(on: boolean) {
    if (on === this.handsUp) return
    this.handsUp = on
    this.showWeapon()
  }

  /** The item in hand (`ITEM_*`, 0 for none): picks the weapon drawn and how the arms hold it. */
  set held(item: number) {
    if (item === this.item) return
    this.item = item
    this.stance = itemDef(item)?.stance ?? 'hold'
    this.showGear()
  }

  get held() {
    return this.item
  }

  /** Where the gun in hand ends, in the world; null while hidden or when nothing is held that shoots. */
  muzzle(out = new Vector3()): Vector3 | null {
    const bone = this.muzzleBones.get(this.stance)
    if (!this.shown || !bone) return null
    this.object.updateWorldMatrix(true, true)
    return bone.getWorldPosition(out)
  }

  /** Whether it wears skis. */
  set skis(on: boolean) {
    if (on === this.onSkis) return
    this.onSkis = on
    if (on && !this.gear) {
      const ski = (name: string) => {
        const group = buildSki()
        this.bones.get(name)?.add(group)
        return group
      }
      const pole = (name: string) => {
        const group = buildPole()
        this.bones.get(name)?.add(group)
        return group
      }
      this.gear = { skis: [ski('foot_L'), ski('foot_R')], poles: [pole('hand_L'), pole('hand_R')] }
    }
    this.showGear()
  }

  get skis() {
    return this.onSkis
  }

  /** Whether it is poling hard. */
  set skiPoling(on: boolean) {
    this.skiHard = on
  }

  /** Whether what is in hand is up instead of the poles: a gun always, anything else while aiming. */
  private get gunUp() {
    return this.skiAiming || fillsHands(this.item)
  }

  /** Whether aim is held on skis. */
  set skiAim(on: boolean) {
    if (on === this.skiAiming) return
    this.skiAiming = on
    this.showGear()
  }

  private showGear() {
    if (this.gear) {
      for (const ski of this.gear.skis) ski.visible = this.onSkis
      for (const pole of this.gear.poles) pole.visible = this.onSkis && !this.gunUp
    }
    this.showWeapon()
  }

  /** Whether its headlamp is on. */
  set lamp(on: boolean) {
    if (on === this.lampOn) return
    this.lampOn = on
    if (on && !this.headlamp && this.head) {
      this.headlamp = new Headlamp()
      this.headlamp.root.quaternion.copy(this.headRest).invert()
      this.headlamp.root.position.applyQuaternion(this.headlamp.root.quaternion)
      this.head.add(this.headlamp.root)
    }
    if (this.headlamp) this.headlamp.lit = on
  }

  /** The lit headlamp's beam, wherever the head has it pointing this frame, ragdoll included; null when off or gone. */
  lampBeam(): { at: Vector3; direction: Vector3 } | null {
    const lamp = this.headlamp
    if (!lamp || !this.lampOn || this.headless || this.gibbed) return null
    this.object.updateWorldMatrix(true, true)
    lamp.root.getWorldPosition(this.beam.at)
    lamp.root.getWorldQuaternion(SCRATCH.q)
    this.beam.direction.set(0, -LAMP_DIP, 1).normalize().applyQuaternion(SCRATCH.q)
    return this.beam
  }

  /** Whether it carries the backpack on its back. */
  set pack(on: boolean) {
    this.carrying = on
    if (on && !this.backpack) {
      this.backpack = buildBackpack()
      this.object.add(this.backpack)
    }
  }

  get dead() {
    return this.down
  }

  /** Lying where it fell. */
  set dead(dead: boolean) {
    if (this.down === dead) return
    this.down = dead
    if (!dead) {
      // Respawned: whole again, upright and drawn.
      this.gibbed = false
      this.headless = false
      this.lostLimbs = 0
      this.limbsKnown = false
      this.hideLimbs()
      this.fling = null
      this.ragdoll = null
      this.rising = null
      this.tumble.identity()
      for (const spring of Object.values(this.springs)) spring.reset()
      this.struckCount = null
      this.bodyYaw = null
      this.fall = this.hang = 0
      this.head?.scale.setScalar(1)
      this.object.visible = this.shown && this.placed
    }
    const action = this.actions.get('death')
    if (!action) return
    // Dying plays the clip from its start; a respawn winds it back and holds it there.
    action.time = 0
    action.timeScale = dead ? 1 : 0
  }

  /** Hides the character: it sits inside armour, or its own scope is in the way. */
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
