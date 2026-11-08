/**
 * Building surfaces (`MAT_WALLS`): a pattern on each wall and roof, picked by the geometry's custom x
 * and laid out in world space along the face, so it needs no uvs and lines up across a building's
 * triangles. Every line is antialiased and, where its spacing nears a pixel, evens out into the tone
 * it averages to rather than shimmering. Needs MARSH_WGSL before it, for its hash and noise.
 */
export const WALLS_WGSL = /* wgsl */ `
// The kinds, as custom x: plain, upright boards (falu red houses), lap boards, brick, render, roof tiles, sheet metal, felt.
const WALL_UPRIGHT = 1.0;
const WALL_LAP = 2.0;
const WALL_BRICK = 3.0;
const WALL_RENDER = 4.0;
const ROOF_TILES = 5.0;
const ROOF_SHEET = 6.0;
const ROOF_FELT = 7.0;
// Mortar, a share of the brick's colour pulled towards this grey.
const WALL_MORTAR = vec3f(0.5, 0.48, 0.44);

// How much of a pixel at \`x\` (metres along) is covered by lines \`width\` wide every \`period\`: antialiased,
// and evening out into width / period as the lines close up under a pixel.
fn wallSeam(x: f32, period: f32, width: f32, pixel: f32) -> f32 {
  let d = abs(fract(x / period + 0.5) - 0.5) * period;
  let sharp = 1.0 - smoothstep(width * 0.5 - pixel * 0.5, width * 0.5 + pixel * 0.5, d);
  return mix(sharp, width / period, smoothstep(period * 0.07, period * 0.25, pixel));
}

// A tone a little either side of 1 for each whole cell \`(i, j)\`, fading to 1 where cells are under a pixel.
fn wallTone(i: f32, j: f32, spread: f32, size: f32, pixel: f32) -> f32 {
  let t = marshHash(i32(i), i32(j) + 7919) - 0.5;
  return 1.0 + t * spread * (1.0 - smoothstep(size * 0.1, size * 0.35, pixel));
}

// \`rgb\` on the face at \`world\` with unit normal \`n\`, of pattern \`kind\`; \`pixel\` the metres a pixel spans there.
fn wallsLook(rgb: vec3f, world: vec3f, n: vec3f, kind: f32, pixel: f32) -> vec3f {
  let level = sqrt(n.x * n.x + n.z * n.z);
  if (kind < ROOF_TILES - 0.5) {
    // A wall: along it horizontally, and up. Faces that are not upright stay plain.
    if (level < 0.7) {
      return rgb;
    }
    let along = dot(world.xz, vec2f(-n.z, n.x) / level);
    let up = world.y;
    if (kind < WALL_UPRIGHT + 0.5) {
      // Boards 20 cm wide, a dark gap between them, each its own shade and with a little grain.
      let board = floor(along / 0.2);
      let gap = wallSeam(along, 0.2, 0.014, pixel);
      let grain = mix(1.0, 0.92 + 0.12 * marshValue(vec2f(along * 30.0, up * 0.8)), 1.0 - smoothstep(0.004, 0.02, pixel));
      return rgb * wallTone(board, 0.0, 0.1, 0.2, pixel) * grain * (1.0 - 0.5 * gap);
    }
    if (kind < WALL_LAP + 0.5) {
      // Boards lapped 17 cm apart: lighter where each starts, a shadow under its edge.
      let row = floor(up / 0.17);
      let within = fract(up / 0.17);
      let shade = mix(1.0, 0.94 + 0.08 * within, 1.0 - smoothstep(0.03, 0.08, pixel));
      let edge = wallSeam(up, 0.17, 0.016, pixel);
      return rgb * shade * wallTone(row, 1.0, 0.06, 0.17, pixel) * (1.0 - 0.45 * edge);
    }
    if (kind < WALL_BRICK + 0.5) {
      // Courses 7.5 cm, bricks 25 cm, every other course half a brick along; each brick its own shade, light mortar between.
      let course = floor(up / 0.075);
      let shifted = along + 0.125 * (course - 2.0 * floor(course / 2.0));
      let brick = floor(shifted / 0.25);
      let mortar = max(wallSeam(up, 0.075, 0.011, pixel), wallSeam(shifted, 0.25, 0.011, pixel));
      let tone = wallTone(brick, course, 0.22, 0.075, pixel);
      return mix(rgb * tone, mix(rgb, WALL_MORTAR, 0.6), mortar * 0.8);
    }
    // Render: mottled, a little streaked downwards, and a darker plinth would need the ground, so none.
    let mottle = 0.6 * marshValue(vec2f(along, up) * 1.3) + 0.4 * marshValue(vec2f(along * 4.0, up * 1.5) + vec2f(17.0, 3.0));
    return rgb * (0.93 + 0.11 * mottle);
  }
  // A roof: across the slope, and down it as far along the surface as it goes.
  if (kind > ROOF_FELT - 0.5 || level < 0.08) {
    // Felt on a flat roof: patchy, a seam every metre.
    let felt = 0.6 * marshValue(world.xz * 0.9) + 0.4 * marshValue(world.xz * 3.1 + vec2f(5.0, 11.0));
    let seam = wallSeam(world.x, 1.0, 0.03, pixel);
    return rgb * (0.9 + 0.14 * felt) * (1.0 - 0.25 * seam);
  }
  let down2 = vec2f(n.x, n.z) / level;
  let across = dot(world.xz, vec2f(-down2.y, down2.x));
  let down = dot(world.xz, down2) / max(abs(n.y), 0.25);
  if (kind < ROOF_TILES + 0.5) {
    // Rows of tiles 33 cm down the slope, 22 cm across, every other row offset: a shadow under each row's edge.
    let row = floor(down / 0.33);
    let within = fract(down / 0.33);
    let shifted = across + 0.11 * (row - 2.0 * floor(row / 2.0));
    let tile = floor(shifted / 0.22);
    let lip = wallSeam(down, 0.33, 0.035, pixel);
    let side = wallSeam(shifted, 0.22, 0.012, pixel);
    let shade = mix(1.0, 1.04 - 0.1 * within, 1.0 - smoothstep(0.05, 0.15, pixel));
    return rgb * shade * wallTone(tile, row, 0.14, 0.22, pixel) * (1.0 - 0.45 * lip) * (1.0 - 0.3 * side);
  }
  // Sheet metal: a standing seam every 50 cm down the slope, lit on one side and shadowed on the other.
  let lit = wallSeam(across - 0.012, 0.5, 0.016, pixel);
  let dark = wallSeam(across + 0.012, 0.5, 0.016, pixel);
  return rgb * (1.0 + 0.22 * lit) * (1.0 - 0.35 * dark);
}
`
