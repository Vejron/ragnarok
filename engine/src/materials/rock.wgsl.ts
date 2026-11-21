/**
 * Bare rock (`MAT_ROCK`): the joints, grain and waterline of a rock slab, drawn over the ground's
 * painted colour near the eye. The slab's colour, its paler and darker stone and its lichen are in the
 * painted cover (the client's `COVER_LAYERS`), which is all there is far off: what is added here
 * lightens as much as it darkens and fades as a pixel grows to span it, so nothing changes tone or
 * shimmers with distance. Needs MARSH_WGSL and ASPHALT_WGSL before it, for their noise.
 */
export const ROCK_WGSL = /* wgsl */ `
// The metres a pixel spans past which nothing of the rock's detail is drawn.
const ROCK_FAR = 0.12;
// The metres a pixel spans past which the stone's grain is not drawn.
const ROCK_NEAR = 0.04;
// Half the width of a joint, metres.
const ROCK_JOINT = 0.014;
// What has gathered in a joint: earth, and the moss that grows in the wider ones (linear colours).
const ROCK_JOINT_EARTH = vec3f(0.03, 0.027, 0.022);
const ROCK_JOINT_MOSS = vec3f(0.06, 0.1, 0.03);
// What a crust of pale lichen adds to the stone under it (linear).
const ROCK_CRUST = vec3f(0.03, 0.035, 0.028);
// The zone the sea's black lichen keeps, metres above the water, and how dark it is.
const ROCK_TIDE_LOW = 0.25;
const ROCK_TIDE_HIGH = 1.1;
const ROCK_TIDE_DARK = 0.55;

struct RockLook {
  rgb: vec3f,
  // What the grain and the joints do to the ground's slope, for its normal: (dh/dx, dh/dz) turned outward.
  tilt: vec2f,
}

// How much of a joint covers a pixel \`distance\` metres from its line: a line never thinner than the
// pixel, and fainter as the pixel grows past the joint's own width.
fn rockJoint(distance: f32, pixel: f32) -> f32 {
  let half = max(ROCK_JOINT, pixel * 0.6);
  return (1.0 - smoothstep(half * 0.5, half, distance)) * min(1.0, ROCK_JOINT / half);
}

// How far \`p\` is from the nearest joint of a set, metres: the set's joints run along y about
// \`spacing\` metres apart, each wandering as it goes (\`wander\`, in spacings) and broken off in places.
fn rockJointSet(p: vec2f, spacing: f32, wander: f32, seed: vec2f) -> f32 {
  let u = p.x / spacing + wander;
  // A joint runs for a length of ten metres or so, each length there or not by its own chance.
  let whole = marshHash(i32(floor(u)) * 7 + i32(seed.x), i32(floor(p.y * 0.09 + seed.y)));
  return abs(fract(u) - 0.5) * spacing + step(whole, 0.3) * 9.0;
}

// \`rock\` 0 to 1 is how much of the ground is rock, \`pixel\` the metres a pixel spans, \`above\` how far
// over the water near it the ground is, metres (3 and more where there is none).
fn rockLook(rgb: vec3f, world: vec2f, rock: f32, pixel: f32, above: f32) -> RockLook {
  // One noise, half a metre a cell, for the rock's ragged edge (turf and moss lie over it in tongues),
  // its crusts of lichen and the waterline's uneven edge.
  let blotch = marshValue(world * 1.9 + vec2f(43.0, 11.0));
  let on = smoothstep(0.25, 0.6, rock + (blotch - 0.5) * 0.5);
  var stone = rgb;
  var tilt = vec2f(0.0);
  if (pixel < ROCK_FAR) {
    // Two sets of joints, each running its own way as the rock split, and crossing. One slow noise
    // bends both.
    let bend = marshValue(world * 0.11 + vec2f(7.0, 61.0));
    let first = rockJointSet(vec2f(world.x * 0.94 + world.y * 0.34, world.y * 0.94 - world.x * 0.34), 2.3, bend * 2.6 + blotch * 0.16, vec2f(7.0, 61.0));
    let second = rockJointSet(vec2f(world.x * 0.36 - world.y * 0.93, world.x * 0.93 + world.y * 0.36), 3.7, 2.1 - bend * 2.2 + blotch * 0.1, vec2f(83.0, 19.0));
    let joint = max(rockJoint(first, pixel), rockJoint(second, pixel));
    let away = 1.0 - smoothstep(ROCK_FAR * 0.5, ROCK_FAR, pixel);
    // Crusts of pale lichen, a hand or two across, their edges sharp where the painted ones are soft.
    let crust = smoothstep(0.68, 0.71, blotch) * (1.0 - smoothstep(0.03, 0.12, pixel));
    stone = mix(stone, stone * 1.12 + ROCK_CRUST, crust * 0.6);
    // The grain: the crystals and pits of the stone, a few centimetres across, seen only close up.
    if (pixel < ROCK_NEAR) {
      let near = 1.0 - smoothstep(ROCK_NEAR * 0.25, ROCK_NEAR, pixel);
      let grain = asphaltNoiseSlope(world * 14.0);
      stone *= 1.0 + (grain.x - 0.5) * 0.3 * near;
      tilt = grain.yz * 0.06 * near;
    }
    // Moss in the joints here and there, earth in the rest.
    stone = mix(stone, mix(ROCK_JOINT_EARTH, ROCK_JOINT_MOSS, smoothstep(0.4, 0.6, bend)), joint * 0.85);
    stone = mix(rgb, stone, away);
  }
  // The black lichen's zone over the waterline, seen from far off too, its upper edge uneven.
  if (above < ROCK_TIDE_HIGH + 0.3) {
    let level = above + (blotch - 0.5) * 0.5;
    let tide = smoothstep(ROCK_TIDE_LOW - 0.15, ROCK_TIDE_LOW, level) * (1.0 - smoothstep(ROCK_TIDE_HIGH - 0.4, ROCK_TIDE_HIGH, level));
    stone *= 1.0 - ROCK_TIDE_DARK * tide;
  }
  return RockLook(mix(rgb, stone, on), tilt * on);
}
`
