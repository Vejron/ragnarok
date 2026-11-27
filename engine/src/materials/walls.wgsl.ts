/**
 * Building surfaces (`MAT_WALLS`): a pattern on each wall and roof, picked by the geometry's custom x
 * and laid out in world space along the face, so it needs no uvs and lines up across a building's
 * triangles. Every line is antialiased and, where its spacing nears a pixel, evens out into the tone
 * it averages to rather than shimmering. Needs MARSH_WGSL before it, for its hash and noise.
 */

/**
 * The colours a boarded house's corner boards are painted (linear `[r, g, b]`): the white of window
 * frames first, then cream, grey, dark green, oxblood and blue-grey. A wall's vertices name one by its
 * place here (`wallsLook`'s \`trim\`), so whoever paints the frames picks from this list too.
 */
export const WALL_TRIMS: readonly (readonly [number, number, number])[] = [
  [0.86, 0.84, 0.78],
  [0.8, 0.72, 0.52],
  [0.5, 0.51, 0.5],
  [0.05, 0.15, 0.085],
  [0.2, 0.035, 0.03],
  [0.17, 0.25, 0.33],
]

/** How many patterns a vertex's custom x can name: the corner boards' colour is in what is over this (`wallsLook`). */
export const WALL_KINDS = 16

export const WALLS_WGSL = /* wgsl */ `
// The kinds, as custom x: plain, upright boards (falu red houses), lap boards, brick, render, roof tiles, sheet metal, felt, cut stone, wooden shingles,
// and a bridge's deck (its roof): asphalt, timber planks or a railway's ballast.
const WALL_UPRIGHT = 1.0;
const WALL_LAP = 2.0;
const WALL_BRICK = 3.0;
const WALL_RENDER = 4.0;
const ROOF_TILES = 5.0;
const ROOF_SHEET = 6.0;
const ROOF_FELT = 7.0;
// Cut stone (French limestone and sandstone): a wall, numbered after the roofs.
const WALL_ASHLAR = 8.0;
// Wooden shingles: short courses of narrow, uneven shakes.
const ROOF_SHINGLES = 9.0;
// A bridge's deck: the road's asphalt (\`asphaltLook\`), or planks laid across a footbridge.
const ROOF_ASPHALT = 10.0;
const ROOF_PLANKS = 11.0;
// A railway bridge's deck: the bed of crushed stone its track lies on.
const ROOF_BALLAST = 12.0;
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

// Four numbers from 0 to 1 of building \`seed\`'s own (a whole number, 1 to 65535) for \`channel\`: the
// same over the whole building, and each says nothing of the others. One hash gives all four, so a
// building's eight numbers cost a pixel two.
fn wallOwn(seed: i32, channel: i32) -> vec4f {
  return fract(marshHash(seed, channel * 7919 + 13) * vec4f(1.0, 23.0, 517.0, 9001.0));
}

// A plinth: cast concrete, a little mottled. And the damp that darkens a wall's foot, its moss, the
// grey that paint weathers to and the rust that runs down a sheet roof (linear colours).
const WALL_PLINTH = vec3f(0.33, 0.32, 0.3);
const WALL_MOSS = vec3f(0.1, 0.15, 0.045);
const WALL_WEATHERED = vec3f(0.42, 0.4, 0.37);
const WALL_RUST = vec3f(0.23, 0.09, 0.035);

// The colours of corner boards (WALL_TRIMS), and how wide one is on each of the two walls it covers, metres.
const WALL_TRIMS = array<vec3f, ${WALL_TRIMS.length}>(${WALL_TRIMS.map(([r, g, b]) => `vec3f(${r}, ${g}, ${b})`).join(', ')});
const WALL_CORNER = 0.13;

// The pattern of wall \`kind\` on \`rgb\`, \`along\` the wall and \`up\` it (metres), for a building whose
// own numbers are \`build\` (where its boards start, their width, how far their shades spread, and its
// mortar or bond): so two houses side by side do not line up.
fn wallPattern(rgb: vec3f, along0: f32, up: f32, kind: f32, pixel: f32, build: vec4f) -> vec3f {
  let along = along0 + build.x * 7.0;
  if (kind < WALL_UPRIGHT + 0.5) {
    // Boards 16 to 22 cm wide, a dark gap between them, each its own shade and with a little grain.
    let width = 0.16 + 0.06 * build.y;
    let board = floor(along / width);
    let gap = wallSeam(along, width, 0.014, pixel);
    // The grain shows only from a few metres: further off its noise is not worked out at all.
    var grain = 1.0;
    if (pixel < 0.02) {
      grain = mix(1.0, 0.92 + 0.12 * marshValue(vec2f(along * 30.0, up * 0.8)), 1.0 - smoothstep(0.004, 0.02, pixel));
    }
    return rgb * wallTone(board, 0.0, 0.06 + 0.1 * build.z, width, pixel) * grain * (1.0 - 0.5 * gap);
  }
  if (kind < WALL_LAP + 0.5) {
    // Boards lapped 14 to 19 cm apart: lighter where each starts, a shadow under its edge.
    let lap = 0.14 + 0.05 * build.y;
    let row = floor(up / lap);
    let within = fract(up / lap);
    let shade = mix(1.0, 0.94 + 0.08 * within, 1.0 - smoothstep(0.03, 0.08, pixel));
    let edge = wallSeam(up, lap, 0.016, pixel);
    return rgb * shade * wallTone(row, 1.0, 0.03 + 0.07 * build.z, lap, pixel) * (1.0 - 0.45 * edge);
  }
  if (kind > WALL_ASHLAR - 0.5) {
    // Blocks 32 cm high and 60 cm long, every other course offset by a third; fine joints, each block a little its own shade and weathered.
    let course = floor(up / 0.32);
    let shifted = along + 0.2 * (course - 3.0 * floor(course / 3.0));
    let block = floor(shifted / 0.6);
    let joint = max(wallSeam(up, 0.32, 0.008, pixel), wallSeam(shifted, 0.6, 0.008, pixel));
    let weather = 0.95 + 0.08 * marshValue(vec2f(along, up) * 2.3);
    return rgb * wallTone(block, course, 0.05 + 0.08 * build.z, 0.32, pixel) * weather * (1.0 - 0.3 * joint);
  }
  if (kind < WALL_BRICK + 0.5) {
    // Courses 7.5 cm, bricks 25 cm, every other course half a brick along, or on some houses a third;
    // each brick its own shade, the mortar between lighter or darker from house to house.
    let course = floor(up / 0.075);
    var bond = 0.125;
    if (build.y > 0.7) {
      bond = 0.083;
    }
    let shifted = along + bond * (course - 2.0 * floor(course / 2.0));
    let brick = floor(shifted / 0.25);
    let mortar = max(wallSeam(up, 0.075, 0.011, pixel), wallSeam(shifted, 0.25, 0.011, pixel));
    let tone = wallTone(brick, course, 0.12 + 0.18 * build.z, 0.075, pixel);
    return mix(rgb * tone, mix(rgb, WALL_MORTAR * (0.7 + 0.6 * build.w), 0.6), mortar * 0.8);
  }
  // Render: mottled, a little streaked downwards.
  let mottle = 0.6 * marshValue(vec2f(along, up) * 1.3) + 0.4 * marshValue(vec2f(along * 4.0, up * 1.5) + vec2f(17.0, 3.0));
  return rgb * (0.93 + 0.11 * mottle);
}

// Moss on an old roof of tiles, shingles or felt: in patches, thicker on the slopes turned from the sun
// (north is -z), on a building of \`age\` past its middle years and near enough to show it.
fn roofMoss(rgb: vec3f, world: vec3f, n: vec3f, age: f32, own: i32, pixel: f32) -> vec3f {
  if (age < 0.55 || pixel > 0.3) {
    return rgb;
  }
  let patches = marshValue(world.xz * 0.8 + vec2f(f32(own & 255), 9.0));
  let shaded = 0.45 + 0.55 * smoothstep(-0.2, 0.5, -n.z);
  let moss = (age - 0.55) * 2.2 * shaded * smoothstep(0.4, 0.8, patches) * (1.0 - smoothstep(0.12, 0.3, pixel));
  return mix(rgb, WALL_MOSS, moss * 0.6);
}

// \`rgb\` on the face at \`world\` with unit normal \`n\`, of pattern \`kind\`; \`pixel\` the metres a pixel
// spans there. \`seed\` is the building's own number as a share of 65535 (0 for none), which gives its
// paint a tint of its own, its boards their width and its walls and roof their age; \`foot\` the height
// of the foot of its walls and \`plinth\` how far over that its plinth stands (metres, 0 for none).
// \`trim\` is 0, or one more than the place in WALL_TRIMS of the colour of the boards over the wall's
// corners, and \`ends\` then how far along the wall its two ends are from here (metres).
fn wallsLook(rgb0: vec3f, world: vec3f, n: vec3f, kind: f32, pixel: f32, seed: f32, foot: f32, plinth: f32, trim: f32, ends: vec2f) -> vec3f {
  let level = sqrt(n.x * n.x + n.z * n.z);
  let own = i32(seed * 65535.0 + 0.5);
  var rgb = rgb0;
  // How weathered the building is, 0 new to 1 old: most are in between.
  var age = 0.0;
  // Its own numbers: where its boards start, their width, how far their shades spread, its mortar or bond.
  var build = vec4f(0.0, 0.5, 0.5, 0.5);
  // The tone of its plinth.
  var base = 0.5;
  if (own != 0 && kind < ROOF_ASPHALT - 0.5) {
    let paint = wallOwn(own, 1);
    build = wallOwn(own, 2);
    age = paint.z;
    base = paint.w;
    // Its paint is a little lighter or darker, warmer or cooler than the next house's of the same colour.
    let warm = (paint.y - 0.5) * 0.08;
    rgb *= (0.92 + 0.16 * paint.x) * vec3f(1.0 + warm, 1.0, 1.0 - warm);
  }
  if (kind < ROOF_TILES - 0.5 || abs(kind - WALL_ASHLAR) < 0.5) {
    // A wall: along it horizontally, and up. Faces that are not upright stay plain.
    if (level < 0.7) {
      return rgb;
    }
    let along = dot(world.xz, vec2f(-n.z, n.x) / level);
    let up = world.y;
    // How far over the top of the plinth this is: under it the wall is the plinth's concrete.
    let over = up - foot - plinth;
    if (plinth > 0.0 && over < -pixel) {
      // Cast concrete: plain from afar, a little mottled near to.
      var poured = 1.0;
      if (pixel < 0.1) {
        poured = mix(1.0, 0.9 + 0.2 * marshValue(vec2f(along, up) * 2.1), 1.0 - smoothstep(0.03, 0.1, pixel));
      }
      return WALL_PLINTH * (0.85 + 0.3 * base) * poured;
    }
    var wall = wallPattern(rgb, along, up, kind, pixel, build);
    if (trim > 0.5) {
      // A board over each corner of a boarded house, from the plinth to the eaves, in the colour of its
      // window frames. From afar it thins out to the share of a pixel it covers.
      let end = min(ends.x, ends.y);
      if (end < WALL_CORNER + pixel) {
        let board = WALL_TRIMS[i32(trim - 0.5)] * (0.94 + 0.08 * build.z);
        let covered = 1.0 - smoothstep(WALL_CORNER - pixel * 0.5, WALL_CORNER + pixel * 0.5, end);
        // The shadow its edge throws on the wall beside it.
        wall *= 1.0 - 0.4 * (1.0 - smoothstep(0.0, 0.012 + pixel, abs(end - WALL_CORNER - 0.006))) * (1.0 - smoothstep(0.02, 0.06, pixel));
        wall = mix(wall, board, covered * min(1.0, WALL_CORNER / max(pixel, 0.001)));
      }
    }
    // Weather, on the older half of the houses and near enough to show: paint gone thin and grey in patches.
    if (age > 0.5 && pixel < 0.12) {
      let patches = marshValue(vec2f(along * 0.45, up * 0.3) + vec2f(f32(own & 255), 0.0));
      let thin = (age - 0.5) * smoothstep(0.45, 0.8, patches) * (1.0 - smoothstep(0.04, 0.12, pixel));
      wall = mix(wall, WALL_WEATHERED * dot(wall, vec3f(0.5)) * 2.0, thin * 0.8);
    }
    if (plinth > 0.0) {
      // Damp and dirt splashed up from the ground darken the first metre over the plinth, the more the older;
      // a line of shadow lies under the lowest board; and below it is the plinth.
      wall *= 1.0 - (0.08 + 0.2 * age) * (1.0 - smoothstep(0.0, 0.9, over));
      wall *= 1.0 - 0.4 * (1.0 - smoothstep(0.0, 0.03 + pixel, over));
      let concrete = WALL_PLINTH * (0.85 + 0.3 * base);
      wall = mix(concrete, wall, smoothstep(-pixel, pixel, over));
    }
    return wall;
  }
  // A bridge's roadway, as the road on the ground is drawn but with no crumbling edge: it runs between kerbs.
  if (kind > ROOF_ASPHALT - 0.5 && kind < ROOF_ASPHALT + 0.5) {
    // No line along the deck is known here: its patches lie square to the world, and it has no paint.
    return asphaltLook(rgb, world.xz, 0.0, world.x, world.z, 30.0, pixel, vec3f(0.0), false).rgb;
  }
  if (kind > ROOF_BALLAST - 0.5) {
    // Faint stones 7 cm across, evening out where they are under a pixel: the track's bed on the
    // ground either side is nearly plain, and the deck's must not show as a patch in it.
    let stone = floor(world.xz / 0.07);
    return rgb * wallTone(stone.x, stone.y, 0.06, 0.07, pixel);
  }
  if (kind > ROOF_PLANKS - 0.5) {
    // Planks 20 cm wide running along z, a dark gap between them; each 2.4 m long, the joints staggered,
    // each its own shade and with grain along it.
    let board = floor(world.x / 0.2);
    let plank = floor((world.z + 0.83 * board) / 2.4);
    let gap = wallSeam(world.x, 0.2, 0.014, pixel);
    let grain = mix(1.0, 0.9 + 0.14 * marshValue(vec2f(world.x * 30.0, world.z * 0.8)), 1.0 - smoothstep(0.004, 0.02, pixel));
    return rgb * wallTone(board, plank, 0.18, 0.2, pixel) * grain * (1.0 - 0.55 * gap);
  }
  // A roof: across the slope, and down it as far along the surface as it goes.
  if ((kind > ROOF_FELT - 0.5 && kind < ROOF_FELT + 0.5) || level < 0.08) {
    // Felt on a flat roof: patchy, a seam every metre.
    let felt = 0.6 * marshValue(world.xz * 0.9) + 0.4 * marshValue(world.xz * 3.1 + vec2f(5.0, 11.0));
    let seam = wallSeam(world.x, 1.0, 0.03, pixel);
    return roofMoss(rgb * (0.9 + 0.14 * felt) * (1.0 - 0.25 * seam), world, n, age, own, pixel);
  }
  let down2 = vec2f(n.x, n.z) / level;
  let across = dot(world.xz, vec2f(-down2.y, down2.x));
  let down = dot(world.xz, down2) / max(abs(n.y), 0.25);
  if (kind < ROOF_TILES + 0.5 || kind > ROOF_SHINGLES - 0.5) {
    // Courses of tiles 33 cm down the slope and 22 cm across (shingles 18 by 12), every other course
    // offset by half. Each tile's lower edge sits a little higher or lower than its neighbours', so
    // the courses run unevenly; a deep shadow under each course's edge and a lit lip on top of the one
    // below give the roof its steps.
    let shingles = kind > ROOF_SHINGLES - 0.5;
    let course = select(0.33, 0.18, shingles);
    let width = select(0.22, 0.12, shingles);
    let row0 = floor(down / course);
    let shifted = across + 0.5 * width * (row0 - 2.0 * floor(row0 / 2.0));
    let tile = floor(shifted / width);
    let wobble = (marshHash(i32(tile), i32(row0) + 131) - 0.5) * select(0.04, 0.03, shingles) * (1.0 - smoothstep(0.02, 0.08, pixel));
    let d = down + wobble;
    let row = floor(d / course);
    let within = fract(d / course);
    let lip = wallSeam(d, course, 0.05, pixel);
    let lit = wallSeam(d - course * 0.17, course, 0.02, pixel);
    let side = wallSeam(shifted, width, 0.012, pixel);
    let shade = mix(1.0, 1.06 - 0.16 * within, 1.0 - smoothstep(0.05, 0.15, pixel));
    let spread = select(0.14, 0.2, shingles) * (0.6 + 0.8 * build.z);
    let laid = rgb * shade * wallTone(tile, row, spread, width, pixel) * (1.0 - 0.6 * lip) * (1.0 + 0.15 * lit) * (1.0 - 0.3 * side);
    return roofMoss(laid, world, n, age, own, pixel);
  }
  // Sheet metal: a standing seam every 50 cm down the slope, lit on one side and shadowed on the other.
  let lit = wallSeam(across - 0.012, 0.5, 0.016, pixel);
  let dark = wallSeam(across + 0.012, 0.5, 0.016, pixel);
  var sheet = rgb * (1.0 + 0.22 * lit) * (1.0 - 0.35 * dark);
  // An old sheet roof rusts in streaks down the slope.
  if (age > 0.6 && pixel < 0.3) {
    let streak = marshValue(vec2f(across * 2.6, down * 0.22) + vec2f(f32(own & 255), 5.0));
    sheet = mix(sheet, WALL_RUST, (age - 0.6) * 1.8 * smoothstep(0.55, 0.9, streak) * (1.0 - smoothstep(0.12, 0.3, pixel)));
  }
  return sheet;
}
`
