/**
 * Makes a colour read from the HDR image safe to add up, for the exposure meter and the shafts' mask.
 * A shader that divides by zero or overflows half precision leaves a pixel
 * that is not a number, or infinite, and a pass that averages pixels spreads it over all it covers: the
 * exposure eases from its last value, so a NaN there would never leave (RENDERING.md, "Measuring, and
 * traps"). Read through `finite`,
 * a NaN or a negative infinity is black and a positive infinity is `top`; every finite value is as it was.
 *
 * Tested on the bits, as a shader compiler may assume a float is never NaN and drop `x != x`.
 */
export const FINITE_WGSL = /* wgsl */ `
fn finite(c: vec3f, top: f32) -> vec3f {
  let bits = bitcast<vec3u>(c);
  let special = (bits & vec3u(0x7f800000u)) == vec3u(0x7f800000u);
  let nan = (bits & vec3u(0x007fffffu)) != vec3u(0u);
  let positive = (bits >> vec3u(31u)) == vec3u(0u);
  return select(c, select(vec3f(0.0), vec3f(top), positive & !nan), special);
}
`
