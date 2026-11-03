/**
 * Worn paths (`MAT_WORN`): a path's ribbon whose edges break up into the ground beside it, with mottled
 * dirt and the odd stone in it. Needs MARSH_WGSL before it, for its noise.
 */
export const WORN_WGSL = /* wgsl */ `
// What the cover trodden flat at a path's edges does to its colour: greener and darker than the dirt.
const WORN_VERGE = vec3f(0.62, 0.8, 0.5);
// A stone or root showing through the dirt, as a share of the colour.
const WORN_STONE = 0.55;

struct WornLook {
  rgb: vec3f,
  // Whether the path covers the ground here, or the ground's own cover has grown over it.
  keep: bool,
}

// \`across\` runs -1 to 1 across the ribbon, \`ragged\` is how unevenly its edges break up (0 to 1) and
// \`pixel\` the metres a pixel spans: far off, where a clump is under a pixel, the edge evens out rather than shimmers.
fn wornLook(rgb: vec3f, world: vec2f, across: f32, ragged: f32, pixel: f32) -> WornLook {
  let near = 1.0 - smoothstep(0.05, 0.3, pixel);
  // Clumps of the ground's cover reach in over the edges, the small ones only seen close up.
  let clumps = 0.65 * marshValue(world * 1.1 + vec2f(41.0, 7.0)) + 0.35 * mix(0.5, marshValue(world * 3.7 + vec2f(3.0, 59.0)), near);
  let lie = abs(across) + (clumps - 0.5) * ragged;
  let keep = lie < 1.0 - 0.3 * ragged;
  // Damp in the low patches, dry and pale on the high ones.
  let damp = marshValue(world * 0.4 + vec2f(13.0, 77.0));
  var c = rgb * mix(0.78, 1.1, smoothstep(0.25, 0.75, damp));
  // Towards the edges it is trampled cover rather than bare dirt.
  c = mix(c, rgb * WORN_VERGE, smoothstep(0.3, 0.75, lie) * ragged);
  // A stone or root in about one cell in eight of a third-metre grid, gone where it is under a pixel.
  let p = world * 3.0;
  let cell = floor(p);
  let ix = i32(cell.x);
  let iz = i32(cell.y);
  let spot = vec2f(marshHash(ix + 101, iz + 57), marshHash(ix + 7, iz + 211)) * 0.6 + 0.2;
  let r = length(p - cell - spot);
  let stone = (1.0 - smoothstep(0.1, 0.1 + pixel * 3.0, r)) * step(0.88, marshHash(ix, iz)) * near;
  c = mix(c, c * WORN_STONE, stone);
  return WornLook(c, keep);
}
`
