import { GunSightView } from './gunsight'
import * as THREE from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { GTAOPass } from 'three/examples/jsm/postprocessing/GTAOPass.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { ContactShadows } from './contact'
import { setWaterLight } from './water'
import { CLASSIC, SOFT, loadLook, type Look } from './look'
import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '../sim/consts'

// On foot the camera is over the right shoulder and a little above the head; the server shares these.
export { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER }

/** Third-person camera distance behind the player, metres. */
export const ON_FOOT_DISTANCE = 3.2
export const DRIVING_DISTANCE = 9
export const TANK_DISTANCE = 13
export const HELI_DISTANCE = 16
export const WALKER_DISTANCE = 15
/** The Strv S driver's camera: behind the gun's trunnion on the aim line and a little above it, as a gunner's, a bit further back. */
export const STRV_DISTANCE = 12
export const STRV_AIM_BACK = 9
export const STRV_AIM_UP = 3.2
/** The walker's aim camera: behind the guns' pivot on the aim line, and this far above it, to see over the cab. */
export const WALKER_AIM_BACK = 12
export const WALKER_AIM_UP = 5
/** A plane's pilot looks where they fly, from behind and above, so the plane sits below the middle of the screen. */
export const PLANE_DISTANCE = 12
export const PLANE_CAMERA_HEIGHT = 2.4
/** The gunner's camera sits this far behind the turret, on the line of the barrel. */
export const GUNNER_BACK = 6.5
/** ...and this far above it, so the turret doesn't hide the view. */
export const GUNNER_UP = 2.6
/** The camera looks at the point this far along the aim line, so the screen centre is where the barrel points. */
export const GUNNER_FOCUS = 60
export const CAMERA_HEIGHT = 1.2

/**
 * Where the sun sits relative to what it follows, metres: about 42° above the horizon. Higher and
 * hillsides all fall in the same lit band; lower and the terrain's toon ramp shows its form.
 */
const SUN_OFFSET = [70, 80, 55] as const

/** The sun's shadow map covers a square this many metres to a side around the player, and its resolution. */
const SHADOW_NEAR = { span: 100, size: 2048, near: 10, far: 300 }
const SHADOW_FAR = { span: 300, size: 4096, near: 1, far: 450 }

/** The AO pass renders at most this many pixels per CSS pixel; it is soft anyway and costs by area. */
const AO_PIXEL_RATIO = 1.5

/** Vertical field of view, degrees: normal, aiming a gun, and looking down a scope. */
const DEFAULT_FOV = 75
const AIM_FOV = 50
const SCOPE_FOV = 18
/** A gunner's sight: a tank gun's optic, wide enough to hold a range ladder out to 400 m. */
const SIGHT_FOV = 20
/** How far in the third-person camera pulls while aiming, as a fraction of its distance. */
export const AIM_DISTANCE_SCALE = 0.7
/** How fast the zoom eases in and out, per second. */
const ZOOM_RATE = 14

export class Scene {
  readonly scene = new THREE.Scene()
  readonly camera = new THREE.PerspectiveCamera(DEFAULT_FOV, 1, 0.25, 2000)
  readonly renderer: THREE.WebGLRenderer
  /** A tank gunner's reticle, drawn over the finished frame. */
  readonly gunsight = new GunSightView()
  private readonly sun = new THREE.DirectionalLight(0xffffff, 2.4)
  private readonly resizeObserver: ResizeObserver
  private readonly hemisphere = new THREE.HemisphereLight()
  /** Blob shadows; the game fills them in each frame. */
  readonly contact: ContactShadows
  private currentLook: Look = loadLook()
  /** Built the first time AO is switched on. */
  private composer: EffectComposer | null = null
  private gtao: GTAOPass | null = null
  /** Parts of the scene the AO pass must not see: translucent effects, decals and lines. */
  private readonly aoHidden: THREE.Object3D[] = []

  constructor(private readonly container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true })
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    this.renderer.shadowMap.enabled = true
    this.renderer.shadowMap.type = THREE.PCFShadowMap
    container.appendChild(this.renderer.domElement)

    this.scene.background = new THREE.Color()
    this.scene.fog = new THREE.Fog(0xffffff, 150, 600)
    this.scene.add(this.hemisphere)
    this.contact = new ContactShadows((x, z) => this.groundAt(x, z))
    this.scene.add(this.contact.root)
    this.excludeFromAO(this.contact.root)
    this.sun.position.set(...SUN_OFFSET)
    this.sun.castShadow = true
    this.sun.shadow.bias = -0.001
    this.sun.shadow.normalBias = 0.15
    this.scene.add(this.sun, this.sun.target)

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.resize()
    this.setLook(this.currentLook)
  }

  get look(): Readonly<Look> {
    return this.currentLook
  }

  /** Switches the soft-lighting options; takes effect on the next frame. */
  setLook(look: Look) {
    this.currentLook = { ...look }
    const palette = look.soft ? SOFT : CLASSIC
    ;(this.scene.background as THREE.Color).set(palette.sky)
    this.scene.fog!.color.set(palette.sky)
    this.hemisphere.color.set(palette.hemiSky)
    this.hemisphere.groundColor.set(palette.hemiGround)
    this.hemisphere.intensity = palette.hemiIntensity
    this.sun.color.set(palette.sun)
    this.sun.intensity = palette.sunIntensity
    this.sun.shadow.radius = palette.shadowRadius
    setWaterLight(palette.sky, palette.sun, new THREE.Vector3(...SUN_OFFSET))
    this.renderer.toneMapping = palette.toneMapping
    this.renderer.toneMappingExposure = palette.exposure
    this.contact.active = look.contact
    this.setShadowReach(look.far)
    if (look.ao && !this.composer) this.buildComposer()
    this.resize()
  }

  private setShadowReach(far: boolean) {
    const { span, size, near, far: depth } = far ? SHADOW_FAR : SHADOW_NEAR
    const shadow = this.sun.shadow
    const camera = shadow.camera
    camera.left = camera.bottom = -span / 2
    camera.right = camera.top = span / 2
    camera.near = near
    camera.far = depth
    camera.updateProjectionMatrix()
    if (shadow.mapSize.x !== size) {
      shadow.mapSize.set(size, size)
      // The old map is the wrong size; the renderer makes a new one on the next frame.
      shadow.map?.dispose()
      shadow.map = null
    }
  }

  /** Keeps `object` out of the AO pass's depth and normal picture, though it is still drawn. */
  excludeFromAO(object: THREE.Object3D) {
    this.aoHidden.push(object)
  }

  private buildComposer() {
    const target = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType, samples: 4 })
    const composer = new EffectComposer(this.renderer, target)
    composer.addPass(new RenderPass(this.scene, this.camera))
    const gtao = new GTAOPass(this.scene, this.camera, 1, 1)
    // Tight and gentle: it should seat things into the ground and darken creases, not outline them.
    gtao.updateGtaoMaterial({ radius: 1.1, distanceExponent: 1.5, thickness: 2, scale: 1.2, samples: 12, distanceFallOff: 1, screenSpaceRadius: false })
    gtao.updatePdMaterial({ lumaPhi: 10, depthPhi: 2, normalPhi: 3, radius: 6, rings: 2, samples: 12 })
    gtao.blendIntensity = 0.85
    // The pass renders the whole scene again for depth and normals; keep translucent things out of it.
    const render = gtao.render.bind(gtao)
    gtao.render = (...args: Parameters<GTAOPass['render']>) => {
      const hidden = this.aoHidden.filter((object) => object.visible)
      for (const object of hidden) object.visible = false
      try {
        render(...args)
      } finally {
        for (const object of hidden) object.visible = true
      }
    }
    composer.addPass(gtao)
    // Tone mapping and the sRGB encode happen here, since the passes work in linear light.
    composer.addPass(new OutputPass())
    this.composer = composer
    this.gtao = gtao
  }

  get canvas() {
    return this.renderer.domElement
  }

  private resize() {
    const { clientWidth: width, clientHeight: height } = this.container
    if (width === 0 || height === 0) return
    this.renderer.setSize(width, height)
    if (this.composer) {
      this.composer.setPixelRatio(Math.min(this.renderer.getPixelRatio(), AO_PIXEL_RATIO))
      this.composer.setSize(width, height)
    }
    this.camera.aspect = width / height
    this.camera.updateProjectionMatrix()
  }

  /** Height of the ground under `(x, z)`, so the camera stays above hills. Set by the game. */
  groundAt: (x: number, z: number) => number = () => 0

  /**
   * Orbits the camera behind `target` by yaw/pitch, and keeps the shadow frustum around it.
   * A `shoulder` shifts the pivot to the right of the target, so the target sits left of screen centre.
   * A `shake` moves the camera but not where it looks, so the view jolts.
   */
  follow(target: readonly number[], yaw: number, pitch: number, distance = ON_FOOT_DISTANCE, shoulder = 0, height = CAMERA_HEIGHT, shake?: readonly number[]) {
    const x = target[0]! + Math.cos(yaw) * shoulder
    const y = target[1]! + height
    const z = target[2]! - Math.sin(yaw) * shoulder
    const horizontal = Math.cos(pitch) * distance
    this.camera.position.set(x + Math.sin(yaw) * horizontal, y - Math.sin(pitch) * distance, z + Math.cos(yaw) * horizontal)
    if (shake) this.camera.position.add(new THREE.Vector3(shake[0], shake[1], shake[2]))
    this.camera.position.y = Math.max(this.groundAt(this.camera.position.x, this.camera.position.z) + 0.3, this.camera.position.y)
    this.camera.lookAt(x, y, z)
    // The shadow frustum follows the target's height too, so it covers hills.
    this.sun.target.position.set(target[0]!, target[1]!, target[2]!)
    this.sun.position.set(target[0]! + SUN_OFFSET[0], target[1]! + SUN_OFFSET[1], target[2]! + SUN_OFFSET[2])
  }

  /**
   * A gunner's view: behind and above `origin` (the turret ring), looking along the aim direction.
   * It focuses on a far point of the barrel's line, so the centre of the screen is where the barrel
   * points even though the camera is higher than the gun. A shell falls from that line.
   */
  aimFrom(origin: readonly number[], yaw: number, pitch: number, back = GUNNER_BACK, up = GUNNER_UP) {
    const dir = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch))
    const from = new THREE.Vector3(origin[0]!, origin[1]!, origin[2]!)
    this.camera.position.copy(from).addScaledVector(dir, -back).add(new THREE.Vector3(0, up, 0))
    this.camera.position.y = Math.max(this.groundAt(this.camera.position.x, this.camera.position.z) + 0.3, this.camera.position.y)
    this.camera.lookAt(from.addScaledVector(dir, GUNNER_FOCUS))
    this.sun.target.position.set(origin[0]!, origin[1]!, origin[2]!)
    this.sun.position.set(origin[0]! + SUN_OFFSET[0], origin[1]! + SUN_OFFSET[1], origin[2]! + SUN_OFFSET[2])
  }

  /**
   * A first-person view from `eye` along yaw/pitch, for looking down a scope. The shot leaves the
   * eye along this line, which is what the server fires when aim is held.
   */
  scopeFrom(eye: readonly number[], yaw: number, pitch: number) {
    this.camera.position.set(eye[0]!, eye[1]!, eye[2]!)
    this.camera.lookAt(
      eye[0]! - Math.sin(yaw) * Math.cos(pitch),
      eye[1]! + Math.sin(pitch),
      eye[2]! - Math.cos(yaw) * Math.cos(pitch),
    )
    this.sun.target.position.set(eye[0]!, eye[1]!, eye[2]!)
    this.sun.position.set(eye[0]! + SUN_OFFSET[0], eye[1]! + SUN_OFFSET[1], eye[2]! + SUN_OFFSET[2])
  }

  /** Eases the field of view towards the scope or aim zoom, or back, `dt` seconds on. `boost` widens the plain view, e.g. with speed. */
  zoomTo(zoom: 'none' | 'aim' | 'scope' | 'sight', dt: number, boost = 0) {
    const target = zoom === 'scope' ? SCOPE_FOV : zoom === 'sight' ? SIGHT_FOV : zoom === 'aim' ? AIM_FOV : DEFAULT_FOV + boost
    const fov = this.camera.fov + (target - this.camera.fov) * Math.min(1, dt * ZOOM_RATE)
    if (Math.abs(fov - this.camera.fov) < 1e-3) return
    this.camera.fov = Math.abs(fov - target) < 0.05 ? target : fov
    this.camera.updateProjectionMatrix()
  }

  render() {
    if (this.currentLook.ao && this.composer) this.composer.render()
    else this.renderer.render(this.scene, this.camera)
    if (this.gunsight.visible) {
      // Over the frame, never behind a wall: its own pass with a cleared depth buffer.
      const clear = this.renderer.autoClear
      this.renderer.autoClear = false
      this.renderer.clearDepth()
      this.renderer.render(this.gunsight.scene, this.gunsight.camera)
      this.renderer.autoClear = clear
    }
  }

  dispose() {
    this.resizeObserver.disconnect()
    this.gunsight.dispose()
    this.gtao?.dispose()
    this.composer?.dispose()
    this.contact.dispose()
    this.renderer.dispose()
    this.renderer.domElement.remove()
  }
}
