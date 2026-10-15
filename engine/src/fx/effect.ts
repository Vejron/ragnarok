import { Node } from '../scene/node'

/** What the renderer lends an effect each frame. */
export interface EffectContext {
  device: GPUDevice
  /** Group 0 of every scene shader: frame uniforms, object records, materials, draw list. */
  sceneLayout: GPUBindGroupLayout
  sceneGroup: GPUBindGroup
  /** The scene's depth, readable while effects draw (the effects pass holds it read-only). */
  depthView: GPUTextureView
  sampleCount: number
  colorFormat: GPUTextureFormat
  depthFormat: GPUTextureFormat
  /** Changes when the render targets are recreated (a resize): bind groups holding `depthView` must be rebuilt. */
  targetsVersion: number
}

/**
 * Something drawn by its own GPU code rather than as meshes: particles, trails, analytic effects. The
 * renderer calls `prepare` before the main pass (for compute work and uploads) and `draw` in the effects
 * pass, which comes after all meshes, transparent ones included, and can read the scene's depth.
 * Effects draw in `renderOrder`.
 */
export abstract class Effect extends Node {
  renderOrder = 0

  abstract prepare(ctx: EffectContext, encoder: GPUCommandEncoder): void

  /** Records the draws; returns how many draw calls it made. */
  abstract draw(ctx: EffectContext, pass: GPURenderPassEncoder): number

  /** Called once the frame is submitted, for reading results back. */
  afterSubmit() {}
}
