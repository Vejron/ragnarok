import { COMMON_WGSL } from './common.wgsl'

/** Depth-only: shadow casters seen from the sun. */
export const SHADOW_WGSL = /* wgsl */ `
${COMMON_WGSL}

@vertex
fn vs(
  @location(0) position: vec3f,
  @location(5) joints: vec4u,
  @location(6) weights: vec4f,
  @builtin(instance_index) instance: u32,
) -> @builtin(position) vec4f {
  let object = objects[drawList[instance]];
  return frame.shadowMatrix * modelMatrix(object, joints, weights) * vec4f(position, 1.0);
}
`
