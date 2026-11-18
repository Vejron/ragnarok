/**
 * The sun's shadow, looked up in the shadow maps. Needs COMMON_WGSL before it, and `shadowMap` and
 * `farShadowMap` (`texture_depth_2d`) and `shadowSampler` (`sampler_comparison`, linear) bound by the
 * shader that includes it. Both functions are 0 in shadow and 1 in the light.
 *
 * The near map is sharp and covers the square around the player; towards its edge the shadow blends
 * into the far map's, which covers a wider square ahead of the camera and fades to light towards its own
 * edge. Neither ends in a line.
 */
export const SUN_SHADOW_WGSL = /* wgsl */ `
// How far towards a map's edge, as a share of the way from its centre, its shadow starts to fade out.
const SHADOW_FADE_FROM = 0.85;

// Where \`world\` falls in the map \`m\` with texels \`texel\` metres wide: uv, and depth in z.
fn shadowAt(m: mat4x4f, texel: f32, world: vec3f, normal: vec3f) -> vec3f {
  // Normal offset: sample from a point pushed off the surface, which beats acne on slopes.
  let p = m * vec4f(world + normal * texel * 1.5, 1.0);
  return vec3f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5, p.z - 0.0005);
}

// How far towards the map's edge \`c\` lies, across it or along the sun: 0 at its centre, 1 at its edge.
fn shadowEdge(c: vec3f) -> f32 {
  let d = abs(c * 2.0 - 1.0);
  return max(max(d.x, d.y), d.z);
}

fn shadowTap(map: texture_depth_2d, base: vec2f, offset: vec2f, inv: vec2f, depth: f32) -> f32 {
  return textureSampleCompareLevel(map, shadowSampler, base + offset * inv, depth);
}

// Castaño's filter over 3×3 texels in 4 bilinear taps, weighted so the kernel is a tent with no holes.
fn shadowFilter3(map: texture_depth_2d, c: vec3f) -> f32 {
  let size = vec2f(textureDimensions(map));
  let inv = 1.0 / size;
  let uv = c.xy * size;
  let centre = floor(uv + 0.5);
  let st = uv + 0.5 - centre;
  let base = (centre - 0.5) * inv;
  let w0 = 3.0 - 2.0 * st;
  let w1 = 1.0 + 2.0 * st;
  let o0 = (2.0 - st) / w0 - 1.0;
  let o1 = st / w1 + 1.0;
  var sum = w0.x * w0.y * shadowTap(map, base, vec2f(o0.x, o0.y), inv, c.z);
  sum += w1.x * w0.y * shadowTap(map, base, vec2f(o1.x, o0.y), inv, c.z);
  sum += w0.x * w1.y * shadowTap(map, base, vec2f(o0.x, o1.y), inv, c.z);
  sum += w1.x * w1.y * shadowTap(map, base, vec2f(o1.x, o1.y), inv, c.z);
  return sum / 16.0;
}

// The same over 5×5 texels in 9 bilinear taps: a softer edge.
fn shadowFilter5(map: texture_depth_2d, c: vec3f) -> f32 {
  let size = vec2f(textureDimensions(map));
  let inv = 1.0 / size;
  let uv = c.xy * size;
  let centre = floor(uv + 0.5);
  let st = uv + 0.5 - centre;
  let base = (centre - 0.5) * inv;
  let w0 = 4.0 - 3.0 * st;
  let w1 = vec2f(7.0);
  let w2 = 1.0 + 3.0 * st;
  let o0 = (3.0 - 2.0 * st) / w0 - 2.0;
  let o1 = (3.0 + st) / w1;
  let o2 = st / w2 + 2.0;
  var sum = w0.x * w0.y * shadowTap(map, base, vec2f(o0.x, o0.y), inv, c.z);
  sum += w1.x * w0.y * shadowTap(map, base, vec2f(o1.x, o0.y), inv, c.z);
  sum += w2.x * w0.y * shadowTap(map, base, vec2f(o2.x, o0.y), inv, c.z);
  sum += w0.x * w1.y * shadowTap(map, base, vec2f(o0.x, o1.y), inv, c.z);
  sum += w1.x * w1.y * shadowTap(map, base, vec2f(o1.x, o1.y), inv, c.z);
  sum += w2.x * w1.y * shadowTap(map, base, vec2f(o2.x, o1.y), inv, c.z);
  sum += w0.x * w2.y * shadowTap(map, base, vec2f(o0.x, o2.y), inv, c.z);
  sum += w1.x * w2.y * shadowTap(map, base, vec2f(o1.x, o2.y), inv, c.z);
  sum += w2.x * w2.y * shadowTap(map, base, vec2f(o2.x, o2.y), inv, c.z);
  return sum / 144.0;
}

// The near map, then the far one past it. \`soft\` filters (by the blur radius, and 4 taps in the far
// map, whose texels are already wide); otherwise one bilinear tap in each.
fn sunShadowFrom(world: vec3f, normal: vec3f, soft: bool) -> f32 {
  if (frame.shadow.z == 0.0) {
    return 1.0;
  }
  let near = shadowAt(frame.shadowMatrix, frame.shadow.x, world, normal);
  let nearEdge = shadowEdge(near);
  var lit = 1.0;
  if (nearEdge < 1.0) {
    if (!soft) {
      lit = textureSampleCompareLevel(shadowMap, shadowSampler, near.xy, near.z);
    } else if (frame.shadow.y > 2.0) {
      lit = shadowFilter5(shadowMap, near);
    } else {
      lit = shadowFilter3(shadowMap, near);
    }
  }
  let past = smoothstep(SHADOW_FADE_FROM, 1.0, nearEdge);
  if (past == 0.0) {
    return lit;
  }
  var far = 1.0;
  if (frame.farShadow.y > 0.5) {
    let c = shadowAt(frame.farShadowMatrix, frame.farShadow.x, world, normal);
    let edge = shadowEdge(c);
    if (edge < 1.0) {
      var shade = 1.0;
      if (soft) {
        shade = shadowFilter3(farShadowMap, c);
      } else {
        shade = textureSampleCompareLevel(farShadowMap, shadowSampler, c.xy, c.z);
      }
      far = mix(shade, 1.0, smoothstep(SHADOW_FADE_FROM, 1.0, edge));
    }
  }
  return mix(lit, far, past);
}

fn sunShadow(world: vec3f, normal: vec3f) -> f32 {
  return sunShadowFrom(world, normal, frame.shadow.w < 0.5);
}

// One hardware-compared tap (a 2×2 bilinear blend): harder edged, for surfaces that cover many pixels.
fn sunShadowTap(world: vec3f, normal: vec3f) -> f32 {
  return sunShadowFrom(world, normal, false);
}
`
