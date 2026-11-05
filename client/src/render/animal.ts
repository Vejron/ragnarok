import { Animator, boxGeometry, Color, Group, Mesh, parseGlb, type AnimationClip, type Geometry, type Material, type Node, Vector3 } from '@ragnarok/engine'
import { KIND_DEER, KIND_STAG } from '@/net/codec'
import { toonMaterial } from './toon'

/** The wild animals' models, by kind: Quaternius' deer and stag, brought in by `tools/npc/import_deer.py`. */
export const ANIMAL_URLS: ReadonlyMap<number, string> = new Map([
  [KIND_DEER, `${import.meta.env.BASE_URL}models/deer.glb`],
  [KIND_STAG, `${import.meta.env.BASE_URL}models/stag.glb`],
])

/** Ground speed above which it walks, and above which it gallops, m/s. The server walks them at 1.3 m/s and gallops them at 9. */
const WALK_MIN_SPEED = 0.15
const GALLOP_MIN_SPEED = 3
/** The ground speed each clip's stride matches at a time scale of 1, m/s; and how far the scale may go either way. */
const WALK_CLIP_SPEED = 1.3
const GALLOP_CLIP_SPEED = 7.5
const SCALE_RANGE: [number, number] = [0.6, 1.6]
/** How fast clip weights move to their targets, per second: a short crossfade; quicker into the death. */
const BLEND_RATE = 6
const DEATH_BLEND_RATE = 20
/** Height of the middle of its body above its hooves, metres, by kind: where a hit with no spot strikes. */
const BODY_HEIGHT: ReadonlyMap<number, number> = new Map([
  [KIND_DEER, 0.95],
  [KIND_STAG, 1.1],
])
/**
 * What a deer blown apart is thrown about as, about its middle as it stands facing -Z: a box's size
 * and where it starts, metres, and which of the model's colours it takes (0 the coat, 1 the belly, 2
 * the darkest). The stag's are as much bigger as it is.
 */
const PIECES: { size: [number, number, number]; at: [number, number, number]; colour: number }[] = [
  { size: [0.4, 0.38, 0.55], at: [0, 0, 0.2], colour: 0 },
  { size: [0.36, 0.34, 0.45], at: [0, 0, -0.3], colour: 0 },
  { size: [0.2, 0.22, 0.32], at: [0, 0.45, -0.7], colour: 0 },
  { size: [0.24, 0.12, 0.3], at: [0, -0.2, 0], colour: 1 },
  { size: [0.08, 0.5, 0.08], at: [0.15, -0.55, -0.45], colour: 2 },
  { size: [0.08, 0.5, 0.08], at: [-0.15, -0.55, -0.45], colour: 2 },
  { size: [0.09, 0.5, 0.09], at: [0.15, -0.55, 0.4], colour: 2 },
  { size: [0.09, 0.5, 0.09], at: [-0.15, -0.55, 0.4], colour: 2 },
  { size: [0.12, 0.1, 0.14], at: [0.1, 0.1, 0.1], colour: 1 },
  { size: [0.1, 0.12, 0.1], at: [-0.12, 0.05, -0.1], colour: 0 },
]

const CLIPS = ['idle', 'graze', 'walk', 'gallop', 'death'] as const
type Clip = (typeof CLIPS)[number]

interface Asset {
  scene: Node
  clips: Map<string, AnimationClip>
  /** The coat, the belly and the darkest of its colours, for the pieces it is blown into. */
  colours: [Color, Color, Color]
}

const assets = new Map<number, Asset>()

/** Parses one animal's GLB for every `AnimalModel` of `kind` to share. Materials become flat toon ones to match the rest of the scene. */
export function parseAnimalModel(kind: number, data: ArrayBuffer) {
  const gltf = parseGlb(data)
  const colours = new Map<string, Color>()
  gltf.scene.traverse((node) => {
    if (!(node instanceof Mesh)) return
    const source = node.material
    colours.set(source.name, new Color(source.color.r, source.color.g, source.color.b))
    node.material = toonMaterial({ color: new Color(source.color.r, source.color.g, source.color.b), name: source.name })
    node.castShadow = true
    // A skinned mesh's bounds are those of its rest pose: it would be culled mid-stride.
    node.frustumCulled = false
  })
  // By how light they are: the coat is the middle one of the hide's, which is what most of it is.
  const shades = [...colours.values()].sort((a, b) => a.r + a.g + a.b - (b.r + b.g + b.b))
  const pick = (share: number) => shades[Math.min(shades.length - 1, Math.floor(shades.length * share))] ?? new Color(0.24, 0.1, 0.05)
  assets.set(kind, { scene: gltf.scene, clips: new Map(gltf.clips.map((clip) => [clip.name, clip])), colours: [pick(0.6), pick(0.9), pick(0.15)] })
}

export async function loadAnimalModels() {
  await Promise.all(
    [...ANIMAL_URLS].map(async ([kind, url]) => {
      const response = await fetch(url)
      if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
      parseAnimalModel(kind, await response.arrayBuffer())
    }),
  )
}

/** Which clip an animal moving at `speed` m/s plays, grazing (its head down) or not, or dead. */
export function clipFor(speed: number, grazing: boolean, dead = false): Clip {
  if (dead) return 'death'
  if (speed > GALLOP_MIN_SPEED) return 'gallop'
  if (speed > WALK_MIN_SPEED) return 'walk'
  return grazing ? 'graze' : 'idle'
}

/**
 * One deer or stag: the model with its `idle`, `graze`, `walk` and `gallop` clips, cross-faded on the
 * speed the interpolator reports and the server's grazing flag. The walk and the gallop play faster
 * or slower with the speed, so the hooves keep up with the ground. Dead, it plays `death` once and
 * lies as that leaves it.
 */
export class AnimalModel {
  readonly root = new Group()
  private readonly animator: Animator
  private readonly weights: Record<Clip, number> = { idle: 1, graze: 0, walk: 0, gallop: 0, death: 0 }
  private readonly colours: [Color, Color, Color]
  private dead = false
  private readonly last = new Vector3()
  private placed = false
  private speed = 0

  constructor(readonly kind: number) {
    const asset = assets.get(kind)
    if (!asset) throw new Error(`loadAnimalModels() must finish before animals of kind ${kind} are created`)
    const model = asset.scene.clone()
    this.root.add(model)
    this.colours = asset.colours
    this.animator = new Animator(model, [...asset.clips.values()])
    for (const [name, action] of this.animator.actions) {
      action.weight = name === 'idle' ? 1 : 0
      // Not all in step: a herd that grazes as one looks wound up.
      action.time = Math.random() * action.clip.duration
      if (name === 'death') action.loop = 'once'
    }
  }

  /** Height of the middle of its body above its hooves, metres. */
  get bodyHeight() {
    return BODY_HEIGHT.get(this.kind) ?? 1
  }

  /** The middle of its body in the world, into `out`: lower once it lies dead. */
  centre(out: Vector3) {
    return out.copy(this.root.position).setY(this.root.position.y + this.bodyHeight * (this.dead ? 0.35 : 1))
  }

  /**
   * Hides it and makes the pieces it is blown into, in the world where it stood, for `Debris` to
   * throw: each piece's mesh, and what it owns for whoever lets go of it to dispose of.
   */
  tear(): { object: Node; owned: (Geometry | Material)[] }[] {
    this.root.visible = false
    const scale = this.bodyHeight / BODY_HEIGHT.get(KIND_DEER)!
    const yaw = this.root.rotation.y
    const [cos, sin] = [Math.cos(yaw), Math.sin(yaw)]
    const centre = this.centre(new Vector3())
    return PIECES.map(({ size, at, colour }) => {
      const geometry = boxGeometry(size[0] * scale, size[1] * scale, size[2] * scale)
      const material = toonMaterial({ color: this.colours[colour]!.clone(), name: 'gib' })
      const mesh = new Mesh(geometry, material)
      const [x, y, z] = at.map((v) => v * scale) as [number, number, number]
      mesh.position.set(centre.x + x * cos + z * sin, centre.y + y, centre.z - x * sin + z * cos)
      mesh.rotation.y = yaw
      mesh.castShadow = true
      return { object: mesh, owned: [geometry, material] }
    })
  }

  /** The clip it is mostly playing now. */
  get playing(): Clip {
    return CLIPS.reduce((best, name) => (this.weights[name] > this.weights[best] ? name : best), 'idle')
  }

  /**
   * Places and animates it. `yaw` is the heading in radians, 0 facing -Z, as for players. `dead` with
   * `fallen` it has lain dead since before it was first drawn, so it is already down.
   */
  update(position: readonly [number, number, number], yaw: number, grazing: boolean, dt: number, dead = false, fallen = false) {
    if (dead !== this.dead) {
      const death = this.animator.actions.get('death')
      if (death) death.time = dead && fallen ? death.clip.duration : 0
      if (dead && fallen) for (const name of CLIPS) this.weights[name] = name === 'death' ? 1 : 0
      // Back from the dead: whole again, wherever it was torn apart.
      if (!dead) this.root.visible = true
      this.dead = dead
    }
    const [x, y, z] = position
    if (this.placed && dt > 0) {
      const speed = Math.hypot(x - this.last.x, z - this.last.z) / dt
      this.speed += (speed - this.speed) * Math.min(1, dt * 10)
    }
    this.last.set(x, y, z)
    this.placed = true
    this.root.position.set(x, y, z)
    this.root.rotation.y = yaw

    const want = clipFor(this.speed, grazing, dead)
    const k = Math.min(1, dt * (dead ? DEATH_BLEND_RATE : BLEND_RATE))
    const [low, high] = SCALE_RANGE
    for (const name of CLIPS) {
      this.weights[name] += ((name === want ? 1 : 0) - this.weights[name]) * k
      const action = this.animator.actions.get(name)
      if (!action) continue
      action.weight = this.weights[name]
      if (name === 'walk') action.timeScale = Math.min(high, Math.max(low, this.speed / WALK_CLIP_SPEED))
      if (name === 'gallop') action.timeScale = Math.min(high, Math.max(low, this.speed / GALLOP_CLIP_SPEED))
    }
    this.animator.update(dt)
  }

  dispose() {
    this.root.removeFromParent()
    this.root.traverse((node) => {
      if (node instanceof Mesh) node.material.dispose()
    })
  }
}
