import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { clone as cloneSkinned } from 'three/examples/jsm/utils/SkeletonUtils.js'

export const ROOSTER_URL = `${import.meta.env.BASE_URL}models/rooster.glb`

/** Ground speed above which the walk clip plays, m/s. The server walks it at 1.2 m/s. */
const WALK_MIN_SPEED = 0.15
/** How fast clip weights move to their targets, per second: a short crossfade. */
const BLEND_RATE = 8

interface Asset {
  scene: THREE.Object3D
  clips: Map<string, THREE.AnimationClip>
}

let asset: Asset | null = null

/**
 * Parses `rooster.glb` (built by `tools/npc/build_rooster.py`) for every `RoosterModel` to share.
 * Materials become flat toon ones to match the rest of the scene.
 */
export async function parseRoosterModel(data: ArrayBuffer) {
  const gltf = await new Promise<{ scene: THREE.Object3D; animations: THREE.AnimationClip[] }>((resolve, reject) => {
    new GLTFLoader().parse(data, '', (g) => resolve(g), reject)
  })
  gltf.scene.traverse((node) => {
    const mesh = node as THREE.Mesh
    if (!mesh.isMesh) return
    const source = mesh.material as THREE.MeshStandardMaterial
    mesh.material = new THREE.MeshToonMaterial({ color: source.color, name: source.name })
    source.dispose()
    mesh.castShadow = true
    // A skinned mesh's bounds are those of its rest pose: it would be culled while crowing.
    mesh.frustumCulled = false
  })
  asset = { scene: gltf.scene, clips: new Map(gltf.animations.map((clip) => [clip.name, clip])) }
}

export async function loadRoosterModel() {
  const response = await fetch(ROOSTER_URL)
  if (!response.ok) throw new Error(`could not load ${ROOSTER_URL}: ${response.status}`)
  await parseRoosterModel(await response.arrayBuffer())
}

/**
 * One rooster: the model with its `idle`, `walk` and `crow` clips. The walk and idle clips cross-fade
 * on the speed the interpolator reports; the crow plays once each time the server raises its crowing flag.
 */
export class RoosterModel {
  readonly root = new THREE.Group()
  private readonly mixer: THREE.AnimationMixer
  private readonly actions = new Map<string, THREE.AnimationAction>()
  private readonly weights = { idle: 1, walk: 0, crow: 0 }
  private readonly last = new THREE.Vector3()
  private placed = false
  private crowing = false
  private speed = 0

  constructor() {
    if (!asset) throw new Error('loadRoosterModel() must finish before roosters are created')
    const model = cloneSkinned(asset.scene)
    this.root.add(model)
    this.mixer = new THREE.AnimationMixer(model)
    for (const [name, clip] of asset.clips) {
      const action = this.mixer.clipAction(clip)
      action.play()
      action.setEffectiveWeight(name === 'idle' ? 1 : 0)
      if (name === 'crow') {
        action.setLoop(THREE.LoopOnce, 1)
        action.clampWhenFinished = true
      }
      this.actions.set(name, action)
    }
  }

  /**
   * Places and animates it. Returns true on the frame it starts to crow, so the caller can make the sound.
   * `yaw` is the heading in radians, 0 facing -Z, as for players.
   */
  update(position: readonly [number, number, number], yaw: number, crowing: boolean, dt: number): boolean {
    const [x, y, z] = position
    if (this.placed && dt > 0) {
      const speed = Math.hypot(x - this.last.x, z - this.last.z) / dt
      this.speed += (speed - this.speed) * Math.min(1, dt * 10)
    }
    this.last.set(x, y, z)
    this.placed = true
    this.root.position.set(x, y, z)
    this.root.rotation.y = yaw

    const started = crowing && !this.crowing
    if (started) {
      const action = this.actions.get('crow')
      action?.reset()
      action?.play()
    }
    this.crowing = crowing

    const walking = this.speed > WALK_MIN_SPEED
    const target = { idle: !crowing && !walking ? 1 : 0, walk: !crowing && walking ? 1 : 0, crow: crowing ? 1 : 0 }
    const k = Math.min(1, dt * BLEND_RATE)
    for (const name of ['idle', 'walk', 'crow'] as const) {
      this.weights[name] += (target[name] - this.weights[name]) * k
      this.actions.get(name)?.setEffectiveWeight(this.weights[name])
    }
    this.mixer.update(dt)
    return started
  }

  dispose() {
    this.mixer.stopAllAction()
    this.root.removeFromParent()
    this.root.traverse((node) => {
      const mesh = node as THREE.Mesh
      if (mesh.isMesh) (mesh.material as THREE.Material).dispose()
    })
  }
}
