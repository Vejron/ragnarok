import { COMMON_WGSL } from './common.wgsl'
import { MARSH_WGSL } from './marsh.wgsl'
import { STONES_WGSL } from './stones.wgsl'
import { SUN_SHADOW_WGSL } from './sunshadow.wgsl'
import { VERTEX_WGSL } from './vertex.wgsl'
import { WALLS_WGSL } from './walls.wgsl'
import { WORN_WGSL } from './worn.wgsl'
import { ASPHALT_WGSL } from './asphalt.wgsl'
import { WET_WGSL } from './wet.wgsl'

/**
 * The shader for every ordinary mesh. Toon lighting matches three's MeshToonMaterial with a gradient
 * map: base / π × (sun × ramp(N·L) × shadow + hemisphere), then linear fog. Unlit materials skip the
 * lighting. One shader for both keeps lit and unlit meshes in the same batches.
 */
export const STANDARD_WGSL = /* wgsl */ `
${COMMON_WGSL}
${VERTEX_WGSL}

// Group 1 holds what the shadow pass must not see bound: it renders into the shadow map.
@group(1) @binding(0) var ramps: texture_2d<f32>;
@group(1) @binding(1) var shadowMap: texture_depth_2d;
@group(1) @binding(2) var shadowSampler: sampler_comparison;

// Group 2 is the material's textures; materials without them share a white map and a flat normal map.
@group(2) @binding(0) var colorMap: texture_2d<f32>;
@group(2) @binding(1) var normalMap: texture_2d<f32>;
@group(2) @binding(2) var mapSampler: sampler;
@group(2) @binding(3) var stonesMap: texture_2d<f32>;

${SUN_SHADOW_WGSL}
${MARSH_WGSL}
${STONES_WGSL}
${WORN_WGSL}
${ASPHALT_WGSL}
${WALLS_WGSL}
${WET_WGSL}

// The craters' marks (linear colours): bare earth in the bowl, char, and the soil thrown out round the rim.
const CRATER_EARTH = vec3f(0.22, 0.14, 0.075);
const CRATER_CHAR = vec3f(0.03, 0.026, 0.022);
const CRATER_SOIL = vec3f(0.3, 0.22, 0.14);
// A fresh crater's embers, bright enough to bloom.
const CRATER_EMBER = vec3f(3.0, 0.8, 0.15);
// How much deeper than they are craters are drawn (their slope and shadow), and how dark a rim's ink line is.
const CRATER_SLOPE = 1.4;
const CRATER_INK = 0.35;

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  // Sampled before any branching: implicit-derivative sampling must be in uniform control flow.
  let texel = textureSample(colorMap, mapSampler, in.uv);
  let normalTexel = textureSample(normalMap, mapSampler, in.uv);
  let stonesTexel = textureSample(stonesMap, mapSampler, in.uv);
  // Metres a pixel spans on the surface, for the craters' rim lines: a derivative, so taken up here too.
  let pixel = length(fwidth(in.world.xz)) * 0.7071;
  // The same on any face, upright ones too, for walls' patterns.
  let facePixel = length(fwidth(in.world)) * 0.6;
  let material = materials[in.ids.x];
  let flags = material.params.x;
  let marshy = (flags & MAT_MARSH) != 0u;
  // A marsh's map has no room for the normal's y: it is the rest of a unit vector, as a heightfield's always points up.
  let packed = normalTexel.xy * 2.0 - 1.0;
  let mapped = select(normalTexel.xyz * 2.0 - 1.0, vec3f(packed.x, sqrt(max(0.0, 1.0 - dot(packed, packed))), packed.y), marshy);
  // Mirroring for a reflection: what is under the water plane is not part of the picture.
  if (frame.lightCount.z > 0.5 && in.world.y < frame.lightCount.y) {
    discard;
  }
  // A ribbon on the ground reads the ground's colour from its map, toned as the ground is, and keeps its own.
  let onGround = (flags & MAT_GROUND) != 0u;
  let ground = texel.rgb * in.custom.z;
  var rgb = select(in.color.rgb * texel.rgb, in.color.rgb, onGround);
  if ((flags & MAT_NORMAL_TONE) != 0u) {
    rgb *= normalTexel.a * 2.0;
  }
  if ((flags & MAT_WALLS) != 0u && in.custom.x > 0.5) {
    rgb = wallsLook(rgb, in.world, normalize(in.normal), in.custom.x, facePixel);
  }
  if ((flags & MAT_WORN) != 0u) {
    let worn = wornLook(rgb, in.world.xz, in.custom, pixel, ground, onGround);
    if (!worn.keep) {
      discard;
    }
    rgb = worn.rgb;
  }
  if ((flags & MAT_ASPHALT) != 0u) {
    let asphalt = asphaltLook(rgb, in.world.xz, in.custom.x, in.custom.w, pixel, ground, onGround);
    if (!asphalt.keep) {
      discard;
    }
    rgb = asphalt.rgb;
  }
  var marks = CraterMark(vec2f(0.0), 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
  if ((flags & MAT_CRATERS) != 0u) {
    marks = craterMarks(in.world.xz, pixel, CRATER_SLOPE);
    rgb = mix(rgb, CRATER_EARTH, marks.earth);
    rgb = mix(rgb, CRATER_SOIL, marks.soil * 0.5);
    rgb = mix(rgb, CRATER_CHAR, marks.char);
  }
  var marsh = MarshLook(rgb, 0.0, 0.0);
  if (marshy && normalTexel.b > 0.004) {
    marsh = marshLook(rgb, in.world.xz, normalTexel.b, pixel);
    // A crater's earth is thrown over the marsh; snow and ice cover the pools.
    let dry = select(1.0 - marks.earth, 0.0, (flags & MAT_SNOW) != 0u);
    rgb = mix(rgb, marsh.rgb, 1.0 - marks.earth);
    marsh.pool *= dry;
    marsh.sheen *= dry;
  }
  var stones = StonesLook(rgb, vec2f(0.0));
  if ((flags & MAT_STONES) != 0u && stonesTexel.r > 0.004) {
    // Where the client lays its stones over these (\`Stones\`, out to 16 m, flattening from 7 m).
    let carpet = 1.0 - smoothstep(7.0, 16.0, distance(in.world.xz, frame.cameraPos.xz));
    stones = stonesLook(rgb, in.world.xz, stonesTexel.r, stonesTexel.g > 0.5, pixel, carpet);
    // A crater's earth is thrown over the stones too.
    rgb = mix(stones.rgb, rgb, marks.earth);
    stones.tilt *= 1.0 - marks.earth;
  }
  // The shore: the stones' map has in blue how far over the water near it the ground is (0 to 1 m).
  // Up to about 0.3 m it is wet, darker and with a sheen, its edge uneven as the waves leave it.
  var wet = 0.0;
  if ((flags & MAT_STONES) != 0u && stonesTexel.b < 0.999) {
    let above = stonesTexel.b + (marshValue(in.world.xz * 0.9) - 0.5) * 0.14;
    wet = (1.0 - smoothstep(0.2, 0.36, above)) * (1.0 - marks.earth);
    rgb *= mix(1.0, 0.6, wet);
  }
  if ((flags & MAT_LIT) != 0u) {
    var n = normalize(in.normal);
    if ((flags & MAT_NORMAL_MAP) != 0u) {
      n = normalize(normalMatrix(objects[in.ids.z].model) * mapped);
    }
    // A heightfield's normal goes as (-dh/dx, 1, -dh/dz): the craters' slope tips it.
    n = normalize(n - vec3f(marks.slope.x, 0.0, marks.slope.y) * n.y + vec3f(stones.tilt.x, 0.0, stones.tilt.y) * n.y);
    if (!front) {
      n = -n;
    }
    if (marsh.pool > 0.0) {
      n = normalize(mix(n, marshRipple(in.world.xz, frame.fog.w), marsh.pool));
    }
    if ((flags & MAT_SNOW) != 0u) {
      // Keeps what the ground's tone did to the colour below it, so snow still shows the relief.
      let tone = select(1.0, mix(1.0, normalTexel.a * 2.0, 0.5), (flags & MAT_NORMAL_TONE) != 0u);
      // Ground the snow has slid off is bare rock, whatever the summer's cover was; a wall is its own colour.
      let bare = select(rgb, mix(SNOW_ROCK * tone, rgb, marks.earth), (flags & MAT_NORMAL_TONE) != 0u);
      rgb = mix(bare, SNOW_COLOR * tone, 0.94 * smoothstep(0.6, 0.82, n.y) * (1.0 - marks.earth));
    }
    // Rain: wet surfaces are darker, and puddles darker still, before they are lit.
    let rain = select(wetness(n, in.world), vec2f(0.0), (flags & MAT_SNOW) != 0u || !front);
    rgb *= (1.0 - 0.35 * rain.x) * (1.0 - 0.4 * rain.y);
    let dotNL = dot(n, frame.sunDir.xyz);
    let u = u32(clamp(dotNL * 0.5 + 0.5, 0.0, 1.0) * 255.0);
    let ramp = textureLoad(ramps, vec2u(u, material.params.y), 0).r;
    var shadow = marks.sun;
    if ((in.ids.y & OBJ_RECEIVE_SHADOW) != 0u) {
      shadow *= sunShadow(in.world, n) * cloudShade(in.world);
    }
    let hemi = mix(frame.hemiGround.rgb, frame.hemiSky.rgb, 0.5 * n.y + 0.5) * marks.sky;
    rgb = rgb / PI * (frame.sunColor.rgb * ramp * shadow + hemi + pointLight(in.world, n)) + material.emissive.rgb;
    if (wet > 0.0) {
      rgb += marshShine(n, in.world, 0.0, wet * 0.8, shadow);
    }
    if (rain.x + rain.y > 0.0) {
      rgb += wetSheen(n, in.world, rain);
    }
    if (marsh.pool + marsh.sheen > 0.0) {
      rgb += marshShine(n, in.world, marsh.pool, marsh.sheen, shadow);
    }
  }
  rgb = rgb * (1.0 - marks.ink * CRATER_INK) + CRATER_EMBER * marks.heat;
  return vec4f(applyFog(rgb, flags, in.viewDepth, in.world), in.color.a * texel.a);
}
`
