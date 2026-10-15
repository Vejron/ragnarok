import { COMMON_WGSL } from './common.wgsl'
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

fn sunShadow(world: vec3f, normal: vec3f) -> f32 {
  if (frame.shadow.z == 0.0) {
    return 1.0;
  }
  // Normal offset: sample from a point pushed off the surface, which beats acne on slopes.
  let offset = normal * frame.shadow.x * 1.5;
  let p = frame.shadowMatrix * vec4f(world + offset, 1.0);
  let uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  let depth = p.z - 0.0005;
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || depth > 1.0) {
    return 1.0;
  }
  // PCF: 3×3 hardware-compared taps, spread by the blur radius.
  let texel = 1.0 / vec2f(textureDimensions(shadowMap));
  let spread = max(frame.shadow.y, 1.0) * 0.5;
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      lit += textureSampleCompareLevel(shadowMap, shadowSampler, uv + vec2f(f32(x), f32(y)) * texel * spread, depth);
    }
  }
  return lit / 9.0;
}

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  // Sampled before any branching: implicit-derivative sampling must be in uniform control flow.
  let texel = textureSample(colorMap, mapSampler, in.uv);
  let mapped = textureSample(normalMap, mapSampler, in.uv).xyz * 2.0 - 1.0;
  let material = materials[in.ids.x];
  let flags = material.params.x;
  var rgb = in.color.rgb * texel.rgb;
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
    rgb = rgb / PI * (frame.sunColor.rgb * ramp * shadow + hemi) + material.emissive.rgb;
  }
  return vec4f(applyFog(rgb, flags, in.viewDepth), in.color.a * texel.a);
}
`
