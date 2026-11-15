/**
 * A VR headset, through WebXR on WebGPU (VR.md). The engine is WebGPU only, so this needs the
 * WebXR/WebGPU binding (`XRGPUBinding`), which is still a draft: Chrome has it behind a flag. Where it
 * is missing nothing here is offered.
 *
 * Each frame the headset gives a pose for each eye and a part of a texture to draw it into, and where
 * the pilot's two controllers are. The types are declared here, as far as they are used: the binding
 * has none published.
 */

import type { EyeTarget } from '@ragnarok/engine'
import { reversedDepth, type PilotHand } from './hands'

interface XRRigidTransform {
  position: { x: number; y: number; z: number }
  orientation: { x: number; y: number; z: number; w: number }
}
interface XRView {
  transform: XRRigidTransform
  projectionMatrix: Float32Array
}
interface XRPose {
  transform: XRRigidTransform
}
interface XRInputSource {
  handedness: 'left' | 'right' | 'none'
  gripSpace?: object
  gamepad?: { axes: readonly number[]; buttons: readonly { pressed: boolean; value: number }[] }
}
interface XRFrame {
  getViewerPose(space: object): (XRPose & { views: XRView[] }) | null
  getPose(space: object, base: object): XRPose | null
}
interface XRSession extends EventTarget {
  inputSources: Iterable<XRInputSource>
  requestReferenceSpace(type: string): Promise<object>
  updateRenderState(state: { layers?: object[]; depthNear?: number; depthFar?: number }): void
  requestAnimationFrame(callback: (time: number, frame: XRFrame) => void): number
  cancelAnimationFrame(handle: number): void
  end(): Promise<void>
}
interface XRSystem {
  isSessionSupported(mode: string): Promise<boolean>
  requestSession(mode: string, init: { requiredFeatures: string[] }): Promise<XRSession>
}
interface XRGPUSubImage {
  colorTexture: GPUTexture
  viewport: { x: number; y: number; width: number; height: number }
  getViewDescriptor(): GPUTextureViewDescriptor
}
interface XRGPUBinding {
  getPreferredColorFormat(): GPUTextureFormat
  createProjectionLayer(init: { colorFormat: GPUTextureFormat }): object
  getViewSubImage(layer: object, view: XRView): XRGPUSubImage
}

const xr = (): XRSystem | undefined => (navigator as unknown as { xr?: XRSystem }).xr
const Binding = (): (new (session: XRSession, device: GPUDevice) => XRGPUBinding) | undefined => (globalThis as unknown as { XRGPUBinding?: new (session: XRSession, device: GPUDevice) => XRGPUBinding }).XRGPUBinding

/** One eye of this frame: where it is and how it looks in the headset's space, its lens, and where to draw it. */
export interface Eye {
  position: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  /** Its projection, in the engine's reversed depth. */
  lens: Float32Array
  /** Roughly what it covers: the vertical field of view, degrees, and its width over its height. */
  fov: number
  aspect: number
  target: EyeTarget
  width: number
  height: number
}

/** What the headset says this frame. */
export interface HeadsetFrame {
  eyes: Eye[]
  /** The pilot's eyes, in the headset's space: the origin is where they were when it started. */
  head: readonly [number, number, number]
  /** The left and the right controller. */
  hands: [PilotHand, PilotHand]
  /** The left and the right thumbstick, each `[x, y]` with + right and + forward. */
  sticks: [readonly [number, number], readonly [number, number]]
  /** The right controller's lower face button (A) is down. */
  jump: boolean
  /** The left and the right controller's index trigger is pulled. */
  triggers: [boolean, boolean]
}

/** The index trigger, the squeeze button and the thumbstick's axes of a controller, in the `xr-standard` layout. */
const TRIGGER = 0
const SQUEEZE = 1
/** The lower face button (A on the right hand, X on the left) in the same layout. */
const FACE = 4
const STICK_X = 2
const STICK_Y = 3

export class Headset {
  /** Whether this browser can show the game in a headset at all: WebXR, with the WebGPU binding. */
  static async supported(): Promise<boolean> {
    try {
      return Binding() !== undefined && (await xr()?.isSessionSupported('immersive-vr')) === true
    } catch {
      return false
    }
  }

  /**
   * Starts a session drawing with `device` into textures of `format`, the renderer's own, between
   * `near` and `far` metres. Must be called from a click. `ended` is called when the session is over,
   * by the pilot's doing or the headset's.
   */
  static async enter(device: GPUDevice, format: GPUTextureFormat, near: number, far: number, ended: () => void): Promise<Headset> {
    const system = xr()
    const Bound = Binding()
    if (!system || !Bound) throw new Error('This browser has no WebXR on WebGPU (XRGPUBinding)')
    const session = await system.requestSession('immersive-vr', { requiredFeatures: ['webgpu'] })
    try {
      const binding = new Bound(session, device)
      if (binding.getPreferredColorFormat() !== format) console.warn(`the headset prefers ${binding.getPreferredColorFormat()}, and is given ${format}`)
      const layer = binding.createProjectionLayer({ colorFormat: format })
      session.updateRenderState({ layers: [layer], depthNear: near, depthFar: far })
      const space = await session.requestReferenceSpace('local')
      const headset = new Headset(session, binding, layer, space)
      session.addEventListener('end', () => {
        headset.over = true
        ended()
      })
      return headset
    } catch (error) {
      await session.end().catch(() => {})
      throw error
    }
  }

  /** The session has ended. */
  over = false
  private frame: XRFrame | null = null

  private constructor(private readonly session: XRSession, private readonly binding: XRGPUBinding, private readonly layer: object, private readonly space: object) {}

  /** Calls `callback` for the headset's next frame, as `requestAnimationFrame` does for the screen's. */
  requestFrame(callback: (time: number) => void): number {
    return this.session.requestAnimationFrame((time, frame) => {
      this.frame = frame
      try {
        callback(time)
      } finally {
        this.frame = null
      }
    })
  }

  cancelFrame(handle: number) {
    this.session.cancelAnimationFrame(handle)
  }

  /** What the headset says in the frame being drawn, or null when it has lost track of itself. */
  read(): HeadsetFrame | null {
    const frame = this.frame
    const pose = frame?.getViewerPose(this.space)
    if (!frame || !pose) return null
    const eyes = pose.views.map((view, index): Eye => {
      const image = this.binding.getViewSubImage(this.layer, view)
      const { x, y, width, height } = image.viewport
      const { position: p, orientation: q } = view.transform
      const projection = view.projectionMatrix
      return {
        position: [p.x, p.y, p.z],
        orientation: [q.x, q.y, q.z, q.w],
        lens: reversedDepth(projection),
        fov: (2 * Math.atan(1 / projection[5]!) * 180) / Math.PI,
        aspect: projection[5]! / projection[0]!,
        target: { view: image.colorTexture.createView(image.getViewDescriptor()), x, y, again: index > 0 },
        width,
        height,
      }
    })
    const hands: [PilotHand, PilotHand] = [{ at: null, closed: false }, { at: null, closed: false }]
    const sticks: [[number, number], [number, number]] = [[0, 0], [0, 0]]
    let jump = false
    const triggers: [boolean, boolean] = [false, false]
    for (const source of this.session.inputSources) {
      if (source.handedness === 'none') continue
      const index: 0 | 1 = source.handedness === 'left' ? 0 : 1
      const grip = source.gripSpace ? frame.getPose(source.gripSpace, this.space) : null
      const pad = source.gamepad
      const at = grip?.transform.position
      hands[index] = { at: at ? [at.x, at.y, at.z] : null, closed: pad?.buttons[SQUEEZE]?.pressed === true }
      // A stick pushed away is a negative y.
      sticks[index] = [pad?.axes[STICK_X] ?? 0, -(pad?.axes[STICK_Y] ?? 0)]
      if (index === 1) jump = pad?.buttons[FACE]?.pressed === true
      triggers[index] = pad?.buttons[TRIGGER]?.pressed === true
    }
    const head = pose.transform.position
    return { eyes, head: [head.x, head.y, head.z], hands, sticks, jump, triggers }
  }

  /** Ends the session. */
  async leave() {
    if (!this.over) await this.session.end().catch(() => {})
  }
}
