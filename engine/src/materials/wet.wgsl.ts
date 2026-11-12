/**
 * Rain on the standard shader's surfaces (`Frame.weather.x`): darker where wet, a sheen of the sky at a
 * grazing look, and puddles in the hollows of level ground that mirror it. Needs SKY_WGSL (through
 * COMMON_WGSL) and MARSH_WGSL (its noise) before it.
 */
export const WET_WGSL = /* wgsl */ `
// How wet a surface facing \`n\` at \`world\` is in the rain, 0 to 1: the flat soaks most, and on level
// ground puddles lie in hollows of a broad noise.
fn wetness(n: vec3f, world: vec3f) -> vec2f {
  let wet = frame.weather.x;
  if (wet <= 0.0) {
    return vec2f(0.0);
  }
  let facing = mix(0.35, 1.0, smoothstep(0.3, 0.9, n.y));
  let level = smoothstep(0.93, 0.99, n.y);
  let puddle = level * smoothstep(0.63, 0.69, marshValue(world.xz * 0.32)) * smoothstep(0.5, 1.0, wet);
  return vec2f(wet * facing, puddle);
}

// A wet surface's sheen: a little of the sky mirrored, more at a grazing look, most in a puddle. Held
// well short of a mirror: wet grass and earth only glisten.
fn wetSheen(n: vec3f, world: vec3f, wet: vec2f) -> vec3f {
  let view = normalize(frame.cameraPos.xyz - world);
  let ripple = vec3f(sin(world.x * 9.0 + frame.fog.w * 7.0), 0.0, cos(world.z * 8.0 - frame.fog.w * 6.0)) * 0.008 * wet.y;
  let normal = normalize(mix(n, vec3f(0.0, 1.0, 0.0), wet.y) + ripple);
  let ndv = clamp(dot(normal, view), 0.0, 1.0);
  let fresnel = min(0.03 + 0.97 * pow(1.0 - ndv, 5.0), 0.4);
  let bounce = reflect(-view, normal);
  return skyHaze(bounce) * fresnel * (wet.x * 0.12 + wet.y * 0.55);
}
`
