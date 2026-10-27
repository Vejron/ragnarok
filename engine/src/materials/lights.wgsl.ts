import { LIGHT_CELL_MAX, LIGHT_GRID } from '../scene/lightgrid'

/**
 * The frame's point and spot lights, for any scene shader: the light records and their grid over the
 * ground (scene/lightgrid.ts) at group 0, and `pointLight`, the light they add to a surface. Needs
 * COMMON_WGSL's group 0 before it.
 */
export const LIGHTS_WGSL = /* wgsl */ `
const LIGHT_GRID = ${LIGHT_GRID}i;
const LIGHT_CELL_MAX = ${LIGHT_CELL_MAX}u;

// Three vec4s a light: xyz position and radius; linear rgb times strength and the cosine of a spot's
// outer half-angle (-2 for a point light); a spot's unit direction and the cosine of its inner half-angle.
@group(0) @binding(7) var<storage, read> lightRecords: array<vec4f>;
// Per cell, offset << 8 | count; then, from LIGHT_GRID², every cell's light indices.
@group(0) @binding(8) var<storage, read> lightCells: array<u32>;

// Light from this frame's point and spot lights on a surface at \`world\` facing \`n\` (wrapped, so it
// still reads on the far side). With \`frame.lightGrid.w\` on, the cell's light count instead, as a heat map.
fn pointLight(world: vec3f, n: vec3f) -> vec3f {
  if (frame.lightCount.x < 0.5) {
    return vec3f(0.0);
  }
  let cell = vec2i(floor((world.xz - frame.lightGrid.xy) * frame.lightGrid.z));
  if (any(cell < vec2i(0)) || any(cell >= vec2i(LIGHT_GRID))) {
    return vec3f(0.0);
  }
  let packed = lightCells[u32(cell.y * LIGHT_GRID + cell.x)];
  let first = u32(LIGHT_GRID * LIGHT_GRID) + (packed >> 8u);
  let count = min(packed & 0xffu, LIGHT_CELL_MAX);
  if (frame.lightGrid.w > 0.5) {
    let heat = f32(count) / 8.0;
    return vec3f(heat, max(1.0 - abs(heat - 1.0), 0.0), max(1.0 - heat * 2.0, 0.0) * 0.4) * PI * 2.0;
  }
  var sum = vec3f(0.0);
  for (var k = 0u; k < count; k++) {
    let i = lightCells[first + k] * 3u;
    let at = lightRecords[i];
    let to = at.xyz - world;
    let d = length(to);
    if (d < at.w) {
      let color = lightRecords[i + 1u];
      let spot = lightRecords[i + 2u];
      let l = to / max(d, 1e-3);
      let fall = 1.0 - d / at.w;
      let facing = dot(n, l) * 0.6 + 0.4;
      // A point light's cosines (-2 and -1) let every direction through.
      let cone = smoothstep(color.w, spot.w, dot(-l, spot.xyz));
      sum += color.rgb * fall * fall * max(facing, 0.0) * cone;
    }
  }
  return sum;
}
`
