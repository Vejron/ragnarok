/**
 * The sun's shadow, looked up in the shadow map. Needs COMMON_WGSL before it, and `shadowMap`
 * (`texture_depth_2d`) and `shadowSampler` (`sampler_comparison`, linear) bound by the shader that
 * includes it. Both functions are 0 in shadow and 1 in the light.
 */
export const SUN_SHADOW_WGSL = /* wgsl */ `
// Where \`world\` falls in the shadow map: uv, and depth in z; z past 1 where the map does not reach.
fn shadowCoord(world: vec3f, normal: vec3f) -> vec3f {
  // Normal offset: sample from a point pushed off the surface, which beats acne on slopes.
  let offset = normal * frame.shadow.x * 1.5;
  let p = frame.shadowMatrix * vec4f(world + offset, 1.0);
  let uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  let depth = p.z - 0.0005;
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0))) {
    return vec3f(uv, 2.0);
  }
  return vec3f(uv, depth);
}

fn sunShadow(world: vec3f, normal: vec3f) -> f32 {
  if (frame.shadow.z == 0.0) {
    return 1.0;
  }
  let c = shadowCoord(world, normal);
  if (c.z > 1.0) {
    return 1.0;
  }
  // PCF: 3×3 hardware-compared taps, spread by the blur radius.
  let texel = 1.0 / vec2f(textureDimensions(shadowMap));
  let spread = max(frame.shadow.y, 1.0) * 0.5;
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      lit += textureSampleCompareLevel(shadowMap, shadowSampler, c.xy + vec2f(f32(x), f32(y)) * texel * spread, c.z);
    }
  }
  return lit / 9.0;
}

// One hardware-compared tap (a 2×2 bilinear blend): harder edged, for surfaces that cover many pixels.
fn sunShadowTap(world: vec3f, normal: vec3f) -> f32 {
  if (frame.shadow.z == 0.0) {
    return 1.0;
  }
  let c = shadowCoord(world, normal);
  if (c.z > 1.0) {
    return 1.0;
  }
  return textureSampleCompareLevel(shadowMap, shadowSampler, c.xy, c.z);
}
`
