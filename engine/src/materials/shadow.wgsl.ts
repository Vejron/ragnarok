import { COMMON_WGSL } from './common.wgsl'

/** Depth-only: shadow casters seen from the sun. */
export const SHADOW_WGSL = /* wgsl */ `
${COMMON_WGSL}

@vertex
fn vs(@location(0) position: vec3f, @builtin(instance_index) instance: u32) -> @builtin(position) vec4f {
  return frame.shadowMatrix * objects[instance].model * vec4f(position, 1.0);
}
`
