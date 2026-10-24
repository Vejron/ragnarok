import { COMMON_WGSL } from './common.wgsl'
import { SUN_SHADOW_WGSL } from './sunshadow.wgsl'
import { VERTEX_WGSL } from './vertex.wgsl'

/**
 * The shader for every ordinary mesh. Toon lighting matches three's MeshToonMaterial with a gradient
 * map: base / π × (sun × ramp(N·L) × shadow + hemisphere), then linear fog. Unlit materials skip the
 * lighting. One shader for both keeps lit and unlit meshes in the same batches.
 */
export const STANDARD_WGSL = /* wgsl */ `
${COMMON_WGSL}
${VERTEX_WGSL}

// Group 1 holds what the shadow pass must not see bound: it renders into the shadow map.
@group(1) @binding(0) var ramps: texture_2d<f32>;
@group(1) @binding(1) var shadowMap: texture_depth_2d;
@group(1) @binding(2) var shadowSampler: sampler_comparison;

// Group 2 is the material's textures; materials without them share a white map and a flat normal map.
@group(2) @binding(0) var colorMap: texture_2d<f32>;
@group(2) @binding(1) var normalMap: texture_2d<f32>;
@group(2) @binding(2) var mapSampler: sampler;

${SUN_SHADOW_WGSL}

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  // Sampled before any branching: implicit-derivative sampling must be in uniform control flow.
  let texel = textureSample(colorMap, mapSampler, in.uv);
  let normalTexel = textureSample(normalMap, mapSampler, in.uv);
  let mapped = normalTexel.xyz * 2.0 - 1.0;
  let material = materials[in.ids.x];
  let flags = material.params.x;
  // Mirroring for a reflection: what is under the water plane is not part of the picture.
  if (frame.lightCount.z > 0.5 && in.world.y < frame.lightCount.y) {
    discard;
  }
  var rgb = in.color.rgb * texel.rgb;
  if ((flags & MAT_NORMAL_TONE) != 0u) {
    rgb *= normalTexel.a * 2.0;
  }
  if ((flags & MAT_LIT) != 0u) {
    var n = normalize(in.normal);
    if ((flags & MAT_NORMAL_MAP) != 0u) {
      n = normalize(normalMatrix(objects[in.ids.z].model) * mapped);
    }
    if (!front) {
      n = -n;
    }
    let dotNL = dot(n, frame.sunDir.xyz);
    let u = u32(clamp(dotNL * 0.5 + 0.5, 0.0, 1.0) * 255.0);
    let ramp = textureLoad(ramps, vec2u(u, material.params.y), 0).r;
    var shadow = 1.0;
    if ((in.ids.y & OBJ_RECEIVE_SHADOW) != 0u) {
      shadow = sunShadow(in.world, n);
    }
    let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * n.y + 0.5);
    rgb = rgb / PI * (frame.sunColor.rgb * ramp * shadow + hemi + pointLight(in.world, n)) + material.emissive.rgb;
  }
  return vec4f(applyFog(rgb, flags, in.viewDepth), in.color.a * texel.a);
}
`
