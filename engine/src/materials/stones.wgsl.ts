/**
 * Stony ground (`MAT_STONES`): cobbles, or a boulder field's blocks, laid over the ground in the
 * shader. The ground is cut into a jittered grid of cells (a Voronoi diagram), and as many of the cells
 * hold a stone as the ground is stony: on sparse ground small stones lie apart, on the stoniest they
 * pack edge to edge. Each stone has its own tone, and a dome (a cobble) or a tipped face (a block) for
 * the light to fall on. Far off, where a cell is a few pixels, the stones even out to their average.
 * `stonesHash` must stay the same as the client's (`client/src/render/stones.ts`), which lays the stones
 * near the eye on the same cells.
 */
export const STONES_WGSL = /* wgsl */ `
// Linear colours: the darkest and palest stone, and a warmer one (pink granite) now and then.
const STONE_DARK = vec3f(0.12, 0.118, 0.11);
const STONE_LIGHT = vec3f(0.4, 0.39, 0.37);
const STONE_WARM = vec3f(0.38, 0.3, 0.25);
// The grit between stones, and the moss and low green that grow in it.
const STONE_GRIT = vec3f(0.075, 0.068, 0.055);
const STONE_MOSS = vec3f(0.07, 0.11, 0.035);
// Metres across a cell: fist-to-head cobbles, and a boulder field's blocks.
const STONE_COBBLE = 0.34;
const STONE_BLOCK = 0.85;
// How much of the ground is under stones, for each grade of stony ground (\`Tile::stones\`).
const STONES_SHOWN = 1.6;

fn stonesHash(ix: i32, iz: i32, salt: u32) -> f32 {
  var h = (bitcast<u32>(ix) * 0x9e3779b1u) ^ (bitcast<u32>(iz) * 0x85ebca77u) ^ (salt * 0xc2b2ae3du);
  h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
  h = (h ^ (h >> 12u)) * 0x297a2d39u;
  h = h ^ (h >> 15u);
  return f32(h >> 8u) / 16777216.0;
}

struct StonesLook {
  rgb: vec3f,
  // What the stones do to the ground's slope, for its normal: (dh/dx, dh/dz) turned outward.
  tilt: vec2f,
}

// \`density\` 0 to 1, \`pixel\` the metres a pixel spans (edges soften with it, and far stones even out),
// \`carpet\` how much the client's stones stand up off these (\`STONES_REACH\`, flattening toward its
// edge), 0 past them: as a stone stands up, its painted one fades into the ground between the stones,
// leaving a soft shade where the stone meets it.
fn stonesLook(rgb: vec3f, world: vec2f, stony: f32, angular: bool, pixel: f32, carpet: f32) -> StonesLook {
  // The map's grades go by how slow the going is: slow running is already ground well covered.
  let density = min(1.0, stony * STONES_SHOWN);
  let size = select(STONE_COBBLE, STONE_BLOCK, angular);
  let q = world / size;
  let base = vec2i(floor(q));
  let f = q - floor(q);
  var best = 9.0;
  var second = 9.0;
  var toCentre = vec2f(0.0);
  var cell = base;
  for (var j = -1; j <= 1; j++) {
    for (var i = -1; i <= 1; i++) {
      let c = base + vec2i(i, j);
      let r = vec2f(f32(i), f32(j)) + vec2f(stonesHash(c.x, c.y, 1u), stonesHash(c.x, c.y, 2u)) * 0.8 + 0.1 - f;
      let d = dot(r, r);
      if (d < best) {
        second = best;
        best = d;
        toCentre = r;
        cell = c;
      } else if (d < second) {
        second = d;
      }
    }
  }
  // About the distance to the cell's edge, in cells.
  let edge = (sqrt(second) - sqrt(best)) * 0.5;
  let present = step(stonesHash(cell.x, cell.y, 3u), density * 1.25);
  // The gap between stones: wide on sparse ground, a crack where they pack.
  let gap = mix(0.2, 0.03, density);
  let aa = max(0.02, pixel / size * 0.7);
  let body = present * smoothstep(gap, gap + aa, edge);
  let tone = stonesHash(cell.x, cell.y, 4u);
  var stone = mix(STONE_DARK, STONE_LIGHT, 0.3 + 0.55 * tone);
  stone = mix(stone, STONE_WARM, step(0.82, stonesHash(cell.x, cell.y, 5u)) * 0.7);
  // Shaded toward its foot, where it meets the ground.
  stone *= mix(0.65, 1.0, smoothstep(gap, gap + 0.15, edge));
  // The ground between stones: the ground's own colour in their shade, mossy and with a little grit,
  // so stony ground runs into what is round it.
  let shaded = mix(rgb * 0.55, STONE_MOSS, 0.35) * 0.8 + STONE_GRIT * 0.2;
  let between = mix(rgb, shaded, smoothstep(0.1, 0.55, density));
  // Under a stone that stands up, the ground in its shade.
  // Soft, toward its middle, so no outline of its cell is left on the ground.
  let under = mix(between, between * 0.75, present * smoothstep(gap, gap + 0.3, edge) * carpet);
  let near = mix(under, stone, body * (1.0 - carpet));
  // Far off: the ground, as much of it under stones as it is stony, in the stones' average tone.
  let covered = min(1.0, density * 1.25) * mix(0.45, 0.75, density);
  let far = mix(between, mix(STONE_DARK, STONE_LIGHT, 0.55), covered);
  let away = smoothstep(0.12, 0.45, pixel / size);
  // A cobble's dome leans away from its middle; a block's face is tipped its own way.
  var tilt = -toCentre * 1.3;
  if (angular) {
    tilt = (vec2f(stonesHash(cell.x, cell.y, 6u), stonesHash(cell.x, cell.y, 7u)) - 0.5) * 1.1;
  }
  return StonesLook(mix(near, far, away), tilt * body * (1.0 - away) * (1.0 - carpet));
}
`
