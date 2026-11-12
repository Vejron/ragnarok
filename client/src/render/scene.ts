import { Bloom, Color, Fog, Ink, PerspectiveCamera, Renderer, Scene as EngineScene } from '@ragnarok/engine'
import { GunSightView } from './gunsight'
import { HeldMapView } from './heldmap'
import { ContactShadows } from './contact'
import { setWaterLight, setWaterPlane, setWaterReflection } from './water'
import { CLASSIC, SOFT, loadLook, wintry, type Look, type Palette } from './look'
import { winter } from './season'
import { loadQuality, PRESETS, resolveQuality, type Preset } from './quality'
import { createSky, nightAt, skyAt } from './daynight'
import { clearArm, lookDir } from '../aimline'
import { GUNNER_BACK, GUNNER_UP, MOUNT_EYE_CLEARANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '../sim/consts'

// On foot the camera is over the right shoulder and a little above the head; the server shares these.
export { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER }

/** Third-person camera distance behind the player, metres. */
export const ON_FOOT_DISTANCE = 3.2
export const DRIVING_DISTANCE = 9
export const TANK_DISTANCE = 13
export const HELI_DISTANCE = 16
export const WALKER_DISTANCE = 15
/** The hen is as big as the walker, and its leaps and glides want room to see where it lands. */
export const HEN_DISTANCE = 17
/** Behind the field mortar, which is low and wide. */
export const MORTAR_DISTANCE = 9
/** The Strv S driver's camera, when not aiming (`STRV_AIM_BACK` and `STRV_AIM_UP` when aiming). */
export const STRV_DISTANCE = 12
/** Behind the Strf 90, which is longer than the tank but lower. */
export const STRF_DISTANCE = 13
/** A plane's pilot looks where they fly, from behind and above, so the plane sits below the middle of the screen. */
export const PLANE_DISTANCE = 12
export const PLANE_CAMERA_HEIGHT = 2.4
export const CAMERA_HEIGHT = 1.2
/** Not used for flying it (that is first person, `fpvFrom`), only by anything that asks every kind. */
export const DRONE_DISTANCE = 3
export const LOITER_DISTANCE = 10

/** How far from what it follows the sun (or moon) is put, metres, along its direction. */
const SUN_DISTANCE = 120
/** The time of day until the game says otherwise (the editor never does): noon. */
const NOON = 0.5

/**
 * The sun's shadow map covers a square this many metres to a side around the player, with casters
 * taken from this far either side of the player along the sun's direction. Its resolution is the
 * quality preset's.
 */
const SHADOW_NEAR = { span: 100, depth: 200 }
const SHADOW_FAR = { span: 300, depth: 300 }

/** Radians a scoped view swings per metre of camera shake: the zoomed view magnifies it, so a little goes a long way. */
const SCOPE_SHAKE = 0.25
/** Vertical field of view, degrees: normal, aiming a gun, and looking down a scope. */
export const DEFAULT_FOV = 65
/** An FPV camera's wide lens. */
const FPV_FOV = 100
/** A loitering drone's belly camera: narrower, to pick out targets from 100 m up. */
const DOWN_FOV = 55
const AIM_FOV = 50
const SCOPE_FOV = 18
/** A gunner's sight: a tank gun's optic, wide enough to hold a range ladder out to 400 m. */
const SIGHT_FOV = 20
/** How far above the ground a shoulder camera stays (m). */
const CAMERA_CLEARANCE = 0.3
/** The colour of the water around a camera under the surface, at noon (sRGB). */
const MURK = 0x1f5f6e
/** Under water, fog from this far to all murk at `MURK_FAR`, metres. */
const MURK_NEAR = 0.5
const MURK_FAR = 22
/** How far in the third-person camera pulls while aiming, as a fraction of its distance. */
export const AIM_DISTANCE_SCALE = 0.7
/** How fast the zoom eases in and out, per second. */
const ZOOM_RATE = 14

/**
 * The picture: the engine's scene, the camera, the renderer and the lights. Everything else adds a
 * `root` node to `scene`. The renderer needs the GPU, which is asked for asynchronously, so `init`
 * must finish before the first `render`; until then `render` does nothing.
 */
export class Scene {
  readonly scene = new EngineScene()
  readonly camera = new PerspectiveCamera(DEFAULT_FOV, 1, 0.25, 2000)
  readonly canvas = document.createElement('canvas')
  renderer: Renderer | null = null
  /** A tank gunner's reticle, drawn over the finished frame. */
  readonly gunsight = new GunSightView()
  /** The paper map in the soldier's hands, in first person. */
  readonly heldMap = new HeldMapView()
  private readonly resizeObserver: ResizeObserver
  /** Blob shadows; the game fills them in each frame. */
  readonly contact: ContactShadows
  private currentLook: Look = loadLook()
  private currentPreset: Preset = PRESETS[resolveQuality(loadQuality())]
  /** The share of the preset's pixel ratio drawn at: lowered while frames are slow (`AdaptiveScale`). */
  private scale = 1
  /** Bumped by each renderer swap, so a slow one that is overtaken is thrown away. */
  private renderers = 0
  private palette: Palette = CLASSIC
  private dayPhase = NOON
  private readonly sky = createSky()
  private readonly glint = new Color()
  /** Whether the camera is under the water's surface this frame (see `setUnderwater`). */
  private underwater = false
  private readonly murk = new Color()
  private readonly saved = { fog: new Color(), background: new Color(), near: 0, far: 0 }
  /** The light on flat ground against noon's, per channel: white at noon, dark blue at midnight (see `Sky.tint`). */
  get groundLight(): Readonly<Color> {
    return this.sky.tint
  }

  /** Where the sun stands relative to what it follows, metres; moves with the time of day. */
  private readonly sunOffset = [0, SUN_DISTANCE, 0]
  private disposed = false

  constructor(private readonly container: HTMLElement) {
    this.canvas.style.cssText = 'display:block;width:100%;height:100%'
    container.appendChild(this.canvas)

    this.scene.fog = new Fog(this.currentPreset.fogNear, this.currentPreset.fogFar)
    this.scene.sun.shadow.enabled = true
    this.contact = new ContactShadows((x, z) => this.groundAt(x, z))
    this.scene.add(this.contact.root, this.gunsight.root, this.heldMap.root)

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.setQuality(this.currentPreset)
    this.setLook(this.currentLook)
  }

  /** Gets the GPU and sizes the picture. Rejects where WebGPU is missing. */
  async init() {
    this.renderer = await this.createRenderer()
    this.resize()
  }

  private async createRenderer(): Promise<Renderer> {
    const renderer = await Renderer.create(this.canvas, { sampleCount: this.currentPreset.msaa, onDeviceLost: (info) => this.deviceLost(info) })
    setWaterReflection(renderer.reflection)
    renderer.detail = this.currentPreset.treeDetail
    return renderer
  }

  /**
   * The GPU can drop the device at any time (sleep and wake, a driver reset, a switch between GPUs), after
   * which nothing draws and the canvas stays black. Ask for a new one; everything the scene holds is
   * uploaded again on the next frame, since the engine's GPU copies belong to the device that made them.
   */
  private async deviceLost(info: GPUDeviceLostInfo) {
    if (info.reason === 'destroyed' || this.disposed) return
    console.warn(`The graphics device was lost (${info.message}); getting a new one`)
    this.renderer = null
    for (let attempt = 0; !this.disposed; attempt++) {
      try {
        const renderer = await this.createRenderer()
        if (this.disposed) {
          renderer.destroy()
          return
        }
        this.renderer = renderer
        this.resize()
        return
      } catch (error) {
        console.error('Could not get the graphics device back:', error)
        await new Promise((resolve) => setTimeout(resolve, Math.min(1000 * 2 ** attempt, 10000)))
      }
    }
  }

  get quality(): Readonly<Preset> {
    return this.currentPreset
  }

  get renderScale() {
    return this.scale
  }

  /** Applies a quality preset. A change of multisampling needs a new renderer, which takes a moment. */
  setQuality(preset: Preset) {
    const msaa = preset.msaa !== this.currentPreset.msaa
    this.currentPreset = preset
    this.scene.fog!.near = preset.fogNear
    this.scene.fog!.far = preset.fogFar
    this.scene.sun.shadow.taps = preset.shadowTaps
    this.heldMap.redrawMs = preset.heldMapMs
    if (this.renderer) this.renderer.detail = preset.treeDetail
    this.setShadowReach(this.currentLook.far)
    if (msaa && this.renderer) void this.replaceRenderer()
    else this.resize()
  }

  /** Draws at `scale` of the preset's pixel ratio. Each change reallocates the render targets. */
  setRenderScale(scale: number) {
    if (scale === this.scale) return
    this.scale = scale
    this.resize()
  }

  private async replaceRenderer() {
    const generation = ++this.renderers
    this.renderer?.destroy()
    this.renderer = null
    try {
      const renderer = await this.createRenderer()
      if (this.disposed || generation !== this.renderers) return renderer.destroy()
      this.renderer = renderer
      this.resize()
    } catch (error) {
      console.error('Could not make a new renderer:', error)
    }
  }

  /** The time of day (`dayPhase`): 0 midnight, 0.5 noon. */
  get phase() {
    return this.dayPhase
  }

  /** How dark it is now for lamps (`nightAt`). */
  get night() {
    return nightAt(this.dayPhase)
  }

  get look(): Readonly<Look> {
    return this.currentLook
  }

  /** Switches the soft-lighting options; takes effect on the next frame. */
  setLook(look: Look) {
    this.currentLook = { ...look }
    const base = look.soft ? SOFT : CLASSIC
    const palette = winter ? wintry(base) : base
    this.palette = palette
    const scene = this.scene
    scene.sun.shadow.radius = palette.shadowRadius
    scene.toneMapping = palette.toneMapping
    scene.exposure = palette.exposure
    this.setTimeOfDay(this.dayPhase)
    this.contact.active = look.contact
    this.setShadowReach(look.far)
    scene.ink = look.ink ? (scene.ink ?? new Ink()) : null
    scene.bloom = look.bloom ? (scene.bloom ?? new Bloom()) : null
    scene.craters.visible = look.scorch
  }

  /**
   * Moves the sun (or by night the moon) and colours the sky, fog and fill for `phase` of the day
   * (`dayPhase`: 0 midnight, 0.5 noon). The sun's position follows on the next `follow`/`aimFrom`/... call.
   */
  setTimeOfDay(phase: number) {
    this.dayPhase = phase
    const sky = skyAt(phase, this.palette, this.sky)
    const scene = this.scene
    scene.background.copy(sky.sky)
    scene.fog!.color.copy(sky.sky)
    scene.hemisphere.sky.copy(sky.hemiSky)
    scene.hemisphere.ground.copy(sky.hemiGround)
    scene.hemisphere.intensity = sky.hemiIntensity
    scene.sun.color.copy(sky.lightColor)
    scene.sun.intensity = sky.lightIntensity
    for (let i = 0; i < 3; i++) this.sunOffset[i] = sky.lightDir[i]! * SUN_DISTANCE
    // The water's glint is the light's colour scaled by its strength against the noon sun's.
    this.glint.copy(sky.lightColor).multiplyScalar(sky.lightIntensity / this.palette.sunIntensity)
    // Clouds mirrored in the water are lit as the ground is.
    setWaterLight(sky.sky.getHex(), this.glint.getHex(), sky.tint)
    const target = scene.sun.target
    this.sunOver(target.x, target.y, target.z)
  }

  private setShadowReach(far: boolean) {
    const { span, depth } = far ? SHADOW_FAR : SHADOW_NEAR
    const shadow = this.scene.sun.shadow
    shadow.span = span
    shadow.mapSize = far ? this.currentPreset.shadowFar : this.currentPreset.shadowNear
    shadow.depth = depth
  }

  private resize() {
    const { clientWidth: width, clientHeight: height } = this.container
    if (width === 0 || height === 0) return
    this.renderer?.setSize(width, height, Math.min(window.devicePixelRatio, this.currentPreset.maxPixelRatio) * this.scale)
    this.camera.aspect = width / height
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
    const camera = this.camera
    camera.position.set(x + Math.sin(yaw) * horizontal, y - Math.sin(pitch) * distance, z + Math.cos(yaw) * horizontal)
    if (shake) camera.position.set(camera.position.x + shake[0]!, camera.position.y + shake[1]!, camera.position.z + shake[2]!)
    camera.position.y = Math.max(this.groundAt(camera.position.x, camera.position.z) + 0.3, camera.position.y)
    camera.lookAt(x, y, z)
    // The shadow frustum follows the target's height too, so it covers hills.
    this.sunOver(target[0]!, target[1]!, target[2]!)
  }

  /**
   * Over a shoulder, where a crosshair must mean what the server's ray does: the camera stays on the
   * line through the pivot along the look, pulled in rather than lifted where the ground would come
   * through it, and looks along that line. Shake moves the view without turning it.
   */
  overShoulder(target: readonly number[], yaw: number, pitch: number, distance: number, shoulder: number, height: number, shake?: readonly number[]) {
    const look = lookDir(yaw, pitch)
    const pivot = [target[0]! + Math.cos(yaw) * shoulder, target[1]! + height, target[2]! - Math.sin(yaw) * shoulder]
    const arm = clearArm(pivot, [-look[0], -look[1], -look[2]], distance, this.groundAt, CAMERA_CLEARANCE)
    const camera = this.camera
    camera.position.set(pivot[0]! - look[0] * arm + (shake?.[0] ?? 0), pivot[1]! - look[1] * arm + (shake?.[1] ?? 0), pivot[2]! - look[2] * arm + (shake?.[2] ?? 0))
    // Only on a bank so steep the pivot itself is in it.
    camera.position.y = Math.max(this.groundAt(camera.position.x, camera.position.z) + CAMERA_CLEARANCE, camera.position.y)
    camera.lookAt(camera.position.x + look[0] * 10, camera.position.y + look[1] * 10, camera.position.z + look[2] * 10)
    this.sunOver(target[0]!, target[1]!, target[2]!)
  }

  /**
   * A gunner's view: `back` behind `origin` (the gun's pivot) on the look's line and `up` above that,
   * looking along the look, so the centre of the screen is the ray the server points the gun at what
   * it meets (`gunnery::wanted`, from the same numbers in `MountDef::eye`). A shell falls from there.
   */
  aimFrom(origin: readonly number[], yaw: number, pitch: number, back = GUNNER_BACK, up = GUNNER_UP) {
    const [dx, dy, dz] = lookDir(yaw, pitch)
    const camera = this.camera
    camera.position.set(origin[0]! - dx * back, origin[1]! - dy * back + up, origin[2]! - dz * back)
    camera.position.y = Math.max(this.groundAt(camera.position.x, camera.position.z) + MOUNT_EYE_CLEARANCE, camera.position.y)
    camera.lookAt(camera.position.x + dx * 10, camera.position.y + dy * 10, camera.position.z + dz * 10)
    this.sunOver(origin[0]!, origin[1]!, origin[2]!)
  }

  /**
   * A first-person view from `eye` along yaw/pitch, for looking down a scope. The shot leaves the
   * eye along this line, which is what the server fires when aim is held.
   */
  scopeFrom(eye: readonly number[], yaw: number, pitch: number, shake?: readonly number[]) {
    this.camera.position.set(eye[0]!, eye[1]!, eye[2]!)
    // A shake swings the view rather than moving the eye: offsetting a point a metre ahead turns it.
    const swing = (axis: number) => (shake ? shake[axis]! * SCOPE_SHAKE : 0)
    this.camera.lookAt(
      eye[0]! - Math.sin(yaw) * Math.cos(pitch) + swing(0),
      eye[1]! + Math.sin(pitch) + swing(1),
      eye[2]! - Math.cos(yaw) * Math.cos(pitch) + swing(2),
    )
    this.sunOver(eye[0]!, eye[1]!, eye[2]!)
  }

  /**
   * An FPV drone's camera at `eye`: looking along yaw/pitch (where its pilot looks, which the drone turns
   * to follow), rolled with the drone's bank.
   */
  fpvFrom(eye: readonly number[], yaw: number, pitch: number, roll: number) {
    this.scopeFrom(eye, yaw, pitch)
    this.camera.rotateZ(roll)
  }

  /**
   * A loitering drone's belly camera at `eye`: straight down, with the drone's heading `yaw` up the
   * screen, and no bank, so the ground below reads like a map.
   */
  downFrom(eye: readonly number[], yaw: number) {
    this.camera.position.set(eye[0]!, eye[1]!, eye[2]!)
    // Turned to the heading, then tipped from looking along it to looking down, which puts the heading at the top.
    this.camera.quaternion.setFromEuler(-Math.PI / 2, yaw, 0, 'YXZ')
    this.sunOver(eye[0]!, eye[1]!, eye[2]!)
  }

  /** The camera at `eye`, looking at `at`, with the sun's shadow centred on what it looks at. */
  lookFromTo(eye: readonly number[], at: readonly number[]) {
    this.camera.position.set(eye[0]!, eye[1]!, eye[2]!)
    this.camera.lookAt(at[0]!, at[1]!, at[2]!)
    this.sunOver(at[0]!, at[1]!, at[2]!)
  }

  /** Centres the sun's shadow on `(x, y, z)`. */
  private sunOver(x: number, y: number, z: number) {
    const sun = this.scene.sun
    sun.target.set(x, y, z)
    sun.position.set(x + this.sunOffset[0]!, y + this.sunOffset[1]!, z + this.sunOffset[2]!)
  }

  /** Eases the field of view towards the scope or aim zoom, or back, `dt` seconds on. `boost` widens the plain view, e.g. with speed. */
  zoomTo(zoom: 'none' | 'aim' | 'scope' | 'sight' | 'fpv' | 'down', dt: number, boost = 0) {
    const target = zoom === 'scope' ? SCOPE_FOV : zoom === 'sight' ? SIGHT_FOV : zoom === 'aim' ? AIM_FOV : zoom === 'fpv' ? FPV_FOV : zoom === 'down' ? DOWN_FOV : DEFAULT_FOV + boost
    const fov = this.camera.fov + (target - this.camera.fov) * Math.min(1, dt * ZOOM_RATE)
    if (Math.abs(fov - this.camera.fov) < 1e-3) return
    this.camera.fov = Math.abs(fov - target) < 0.05 ? target : fov
  }

  /**
   * The height of the water to mirror this frame, or null where there is none in sight (the mirrored
   * pass is then skipped altogether).
   */
  setWaterPlane(height: number | null) {
    if (!this.renderer) return
    const on = this.currentLook.reflect && height !== null
    this.renderer.reflectionHeight = on ? height : null
    setWaterPlane(on ? height : null)
  }

  /** Whether the camera is under the water's surface: the view then closes in to the water's murk. */
  setUnderwater(under: boolean) {
    this.underwater = under
  }

  render() {
    if (!this.renderer) return
    // The reticle is laid out in view space; it goes where the camera is, just before drawing.
    this.camera.updateWorld()
    this.gunsight.follow(this.camera)
    this.heldMap.follow(this.camera)
    if (!this.underwater) return this.renderer.render(this.scene, this.camera)
    // Under water the fog is the water's own colour, lit as the ground is, and close; the sky's comes back after.
    const { fog, background } = this.scene
    const saved = this.saved
    saved.fog.copy(fog!.color)
    saved.background.copy(background)
    saved.near = fog!.near
    saved.far = fog!.far
    const murk = this.murk.setHex(MURK)
    const tint = this.sky.tint
    murk.set(murk.r * tint.r, murk.g * tint.g, murk.b * tint.b)
    fog!.color.copy(murk)
    background.copy(murk)
    fog!.near = MURK_NEAR
    fog!.far = MURK_FAR
    try {
      this.renderer.render(this.scene, this.camera)
    } finally {
      fog!.color.copy(saved.fog)
      background.copy(saved.background)
      fog!.near = saved.near
      fog!.far = saved.far
    }
  }

  dispose() {
    this.disposed = true
    this.resizeObserver.disconnect()
    this.gunsight.dispose()
    this.heldMap.dispose()
    this.contact.dispose()
    this.renderer?.destroy()
    this.canvas.remove()
  }
}
