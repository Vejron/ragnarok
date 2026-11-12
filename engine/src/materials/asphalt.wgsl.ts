/**
 * Asphalt (`MAT_ASPHALT`): a paved road's ribbon, with the tarmac's broad mottling, darker patches
 * where it was mended, grit in it close up, the odd crack and edges that crumble a little. Everything
 * is laid out in world space, so it carries on unbroken across tiles and the discs at a road's bends.
 * Needs MARSH_WGSL before it, for its noise.
 */
export const ASPHALT_WGSL = /* wgsl */ `
// A mended patch's fresher tar, and the sealed seam round it, as shares of the colour.
const ASPHALT_PATCH = 0.74;
const ASPHALT_SEAM = 0.55;
// A crack, as a share of the colour.
const ASPHALT_CRACK = 0.5;
// On a tile's ground (\`MAT_GROUND\`), how much of the ground's colour the dust on the road gives it.
const ASPHALT_DUST = 0.08;
// Side of the grid the patches are scattered on, metres, and the share of its cells that have one.
const ASPHALT_PATCH_GRID = 6.0;
const ASPHALT_PATCH_SHARE = 0.12;

struct AsphaltLook {
  rgb: vec3f,
  // Whether the road covers the ground here, or its crumbled edge has left the verge showing.
  keep: bool,
}

// \`across\` runs -1 to 1 across the ribbon (in a disc, how far from the road it is), \`half\` is its half
// width, metres, and \`pixel\` the metres a pixel spans: grit and cracks fade out where they would be
// under a pixel, rather than shimmer. \`ground\` is the colour of the ground under it, where \`onGround\`
// says it is known: dust blown on from it.
fn asphaltLook(rgb: vec3f, world: vec2f, across: f32, half: f32, pixel: f32, ground: vec3f, onGround: bool) -> AsphaltLook {
  let grit = 1.0 - smoothstep(0.006, 0.03, pixel);
  let close = 1.0 - smoothstep(0.03, 0.12, pixel);
  // Broad mottling: older, greyer stretches and darker, newer ones.
  let broad = 0.6 * marshValue(world * 0.11 + vec2f(17.0, 5.0)) + 0.4 * marshValue(world * 0.37 + vec2f(3.0, 91.0));
  var c = rgb * mix(0.86, 1.12, broad);

  // A mended patch: a rounded box of its own size somewhere in some cells of a grid, wholly inside the cell.
  let g = world / ASPHALT_PATCH_GRID;
  let cell = floor(g);
  let ix = i32(cell.x);
  let iz = i32(cell.y);
  let has = step(1.0 - ASPHALT_PATCH_SHARE, marshHash(ix + 313, iz + 71));
  let centre = vec2f(marshHash(ix + 5, iz + 17), marshHash(ix + 29, iz + 3)) * 0.4 + 0.3;
  let size = vec2f(marshHash(ix + 11, iz + 43), marshHash(ix + 61, iz + 13)) * 0.18 + 0.08;
  let d = abs(g - cell - centre) - size;
  // Distance outside the box, metres (negative inside).
  let box = (length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0)) * ASPHALT_PATCH_GRID;
  let soft = 0.01 + pixel;
  let inside = (1.0 - smoothstep(-soft, soft, box)) * has;
  c = mix(c, c * ASPHALT_PATCH, inside);
  let seam = (1.0 - smoothstep(0.02, 0.02 + soft, abs(box + 0.03))) * has * close;
  c = mix(c, c * ASPHALT_SEAM, seam * 0.7);

  // Grit: the stones in the mix, pale and dark, only seen close up.
  let pale = smoothstep(0.7, 0.78, marshValue(world * 29.0 + vec2f(7.0, 31.0)));
  let dark = smoothstep(0.68, 0.76, marshValue(world * 53.0 + vec2f(61.0, 2.0)));
  c *= 1.0 + grit * (0.18 * pale - 0.14 * dark);

  // Cracks: thin wandering lines in about a third of the road, close up.
  let line = 0.75 * marshValue(world * 0.4 + vec2f(101.0, 37.0)) + 0.25 * marshValue(world * 2.1 + vec2f(9.0, 77.0));
  let cracked = smoothstep(0.58, 0.66, marshValue(world * 0.07 + vec2f(41.0, 113.0)));
  let width = 0.004 + pixel * 0.15;
  let crack = (1.0 - smoothstep(width * 0.5, width, abs(line - 0.5))) * cracked * close * (1.0 - inside);
  c = mix(c, c * ASPHALT_CRACK, crack);

  // The edge: dusty and crumbling over its last half metre, and broken unevenly into the shoulder.
  let inward = (1.0 - abs(across)) * half;
  let crumble = marshValue(world * 2.6 + vec2f(23.0, 53.0)) - 0.5;
  let keep = inward > crumble * 0.25;
  let dusty = 1.0 - smoothstep(0.0, 0.5, inward);
  c = mix(c, c * vec3f(1.12, 1.08, 1.0), dusty * 0.6);
  // Dust off the ground beside it, a little all over and more at the edges.
  if (onGround) {
    c = mix(c, ground * 0.6, ASPHALT_DUST + 0.25 * dusty);
  }
  return AsphaltLook(c, keep);
}
`
