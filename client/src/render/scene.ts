import { Bloom, Color, Fog, Ink, PerspectiveCamera, Renderer, Scene as EngineScene } from '@ragnarok/engine'
import { GunSightView } from './gunsight'
import { ContactShadows } from './contact'
import { setWaterLight, setWaterPlane, setWaterReflection } from './water'
import { CLASSIC, SOFT, loadLook, type Look, type Palette } from './look'
import { createSky, skyAt } from './daynight'
import { clearArm, lookDir } from '../aimline'
import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '../sim/consts'

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
/** Not used for flying it (that is first person, `fpvFrom`), only by anything that asks every kind. */
export const DRONE_DISTANCE = 3
export const LOITER_DISTANCE = 10

/** How far from what it follows the sun (or moon) is put, metres, along its direction. */
const SUN_DISTANCE = 120
/** The time of day until the game says otherwise (the editor never does): noon. */
const NOON = 0.5

/**
 * The sun's shadow map covers a square this many metres to a side around the player, at this resolution,
 * with casters taken from this far either side of the player along the sun's direction.
 */
const SHADOW_NEAR = { span: 100, size: 2048, depth: 200 }
const SHADOW_FAR = { span: 300, size: 4096, depth: 300 }

/** Radians a scoped view swings per metre of camera shake: the zoomed view magnifies it, so a little goes a long way. */
const SCOPE_SHAKE = 0.25
/** Vertical field of view, degrees: normal, aiming a gun, and looking down a scope. */
const DEFAULT_FOV = 75
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
/** How far in the third-person camera pulls while aiming, as a fraction of its distance. */
export const AIM_DISTANCE_SCALE = 0.7
/** How fast the zoom eases in and out, per second. */
const ZOOM_RATE = 14

/** The pixel ratio the picture renders at: the screen's, to at most this. */
const MAX_PIXEL_RATIO = 2

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
  private readonly resizeObserver: ResizeObserver
  /** Blob shadows; the game fills them in each frame. */
  readonly contact: ContactShadows
  private currentLook: Look = loadLook()
  private palette: Palette = CLASSIC
  private dayPhase = NOON
  private readonly sky = createSky()
  private readonly glint = new Color()
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

    this.scene.fog = new Fog(150, 600)
    this.scene.sun.shadow.enabled = true
    this.contact = new ContactShadows((x, z) => this.groundAt(x, z))
    this.scene.add(this.contact.root, this.gunsight.root)

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.setLook(this.currentLook)
  }

  /** Gets the GPU and sizes the picture. Rejects where WebGPU is missing. */
  async init() {
    this.renderer = await this.createRenderer()
    this.resize()
  }

  private async createRenderer(): Promise<Renderer> {
    const renderer = await Renderer.create(this.canvas, { onDeviceLost: (info) => this.deviceLost(info) })
    setWaterReflection(renderer.reflection)
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

  get look(): Readonly<Look> {
    return this.currentLook
  }

  /** Switches the soft-lighting options; takes effect on the next frame. */
  setLook(look: Look) {
    this.currentLook = { ...look }
    const palette = look.soft ? SOFT : CLASSIC
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
    const { span, size, depth } = far ? SHADOW_FAR : SHADOW_NEAR
    const shadow = this.scene.sun.shadow
    shadow.span = span
    shadow.mapSize = size
    shadow.depth = depth
  }

  private resize() {
    const { clientWidth: width, clientHeight: height } = this.container
    if (width === 0 || height === 0) return
    this.renderer?.setSize(width, height, Math.min(window.devicePixelRatio, MAX_PIXEL_RATIO))
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
   * A gunner's view: behind and above `origin` (the turret ring), looking along the aim direction.
   * It focuses on a far point of the barrel's line, so the centre of the screen is where the barrel
   * points even though the camera is higher than the gun. A shell falls from that line.
   */
  aimFrom(origin: readonly number[], yaw: number, pitch: number, back = GUNNER_BACK, up = GUNNER_UP) {
    const dx = -Math.sin(yaw) * Math.cos(pitch)
    const dy = Math.sin(pitch)
    const dz = -Math.cos(yaw) * Math.cos(pitch)
    const camera = this.camera
    camera.position.set(origin[0]! - dx * back, origin[1]! - dy * back + up, origin[2]! - dz * back)
    camera.position.y = Math.max(this.groundAt(camera.position.x, camera.position.z) + 0.3, camera.position.y)
    camera.lookAt(origin[0]! + dx * GUNNER_FOCUS, origin[1]! + dy * GUNNER_FOCUS, origin[2]! + dz * GUNNER_FOCUS)
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

  render() {
    if (!this.renderer) return
    // The reticle is laid out in view space; it goes where the camera is, just before drawing.
    this.camera.updateWorld()
    this.gunsight.follow(this.camera)
    this.renderer.render(this.scene, this.camera)
  }

  dispose() {
    this.disposed = true
    this.resizeObserver.disconnect()
    this.gunsight.dispose()
    this.contact.dispose()
    this.renderer?.destroy()
    this.canvas.remove()
  }
}
