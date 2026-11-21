import { COMMON_WGSL } from './common.wgsl'
import { MARSH_WGSL } from './marsh.wgsl'
import { ROCK_WGSL } from './rock.wgsl'
import { STONES_WGSL } from './stones.wgsl'
import { SUN_SHADOW_WGSL } from './sunshadow.wgsl'
import { vertexWgsl } from './vertex.wgsl'
import { WALLS_WGSL } from './walls.wgsl'
import { WORN_WGSL } from './worn.wgsl'
import { ASPHALT_WGSL } from './asphalt.wgsl'
import { WET_WGSL } from './wet.wgsl'

/**
 * The shader for every ordinary mesh. Toon lighting matches three's MeshToonMaterial with a gradient
 * map: base / π × (sun × ramp(N·L) × shadow + hemisphere), then linear fog. Unlit materials skip the
 * lighting. One shader for both keeps lit and unlit meshes in the same batches. With `vertexScars`,
 * scars press a damaged mesh's vertices in as well as shading its dents.
 *
 * A shader that can discard loses a tiled GPU's hidden-surface removal for
 * everything drawn with it (every hidden layer is shaded, and a constant that is false does not bring it
 * back: the statement must be gone), so what discards is written in only for the draws that need it:
 * `reflect` for the mirror image, cut off at the water, and `edges` for the ragged edges of worn paths
 * and asphalt.
 */
export const standardWgsl = (vertexScars: boolean, reflect = false, edges = false) => /* wgsl */ `
${COMMON_WGSL}
${vertexWgsl(false, vertexScars)}

// Group 1 holds what the shadow pass must not see bound: it renders into the shadow map.
@group(1) @binding(0) var ramps: texture_2d<f32>;
@group(1) @binding(1) var shadowMap: texture_depth_2d;
@group(1) @binding(2) var shadowSampler: sampler_comparison;
@group(1) @binding(3) var farShadowMap: texture_depth_2d;

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
${ROCK_WGSL}
${WALLS_WGSL}
${WET_WGSL}

// The craters' marks (linear colours): bare earth in the bowl, char, and the soil thrown out round the rim.
const CRATER_EARTH = vec3f(0.22, 0.14, 0.075);
// The same earth freshly turned up: dark and damp, until it dries.
const CRATER_EARTH_DAMP = vec3f(0.115, 0.07, 0.04);
const CRATER_CHAR = vec3f(0.03, 0.026, 0.022);
const CRATER_SOIL = vec3f(0.3, 0.22, 0.14);
// A fresh crater's embers, bright enough to bloom.
const CRATER_EMBER = vec3f(3.0, 0.8, 0.15);
// The light the embers throw on the bowl round them: too faint to see in daylight, plain in the dark.
const CRATER_GLOW = vec3f(0.1, 0.028, 0.006);
// How much deeper than they are craters are drawn (their slope and shadow), and how dark a rim's ink line is.
const CRATER_SLOPE = 1.4;
const CRATER_INK = 0.35;
// A scarred mesh's bare metal (where paint is torn off) and its char (scars.wgsl).
const SCAR_BARE = vec3f(0.46, 0.46, 0.48);
const SCAR_CHAR = vec3f(0.018, 0.016, 0.014);
const SCAR_HOLE_RGB = vec3f(0.004, 0.003, 0.003);
// How strongly a dent's slope shades it against the sun, beyond what the toon ramp's bands show.
const SCAR_RELIEF = 2.4;

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
${reflect ? `  if (frame.lightCount.z > 0.5 && dot(in.world, frame.mirror.xyz) < frame.mirror.w) {
    discard;
  }` : ''}
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
${edges ? `    if (!worn.keep) {
      discard;
    }` : ''}
    rgb = worn.rgb;
  }
  if ((flags & MAT_ASPHALT) != 0u) {
    // Asphalt carries where it is along and across the road in custom.y and z, and its tone only in its colour.
    let tone = in.color.g / max(material.color.g, 1e-3);
    let asphalt = asphaltLook(rgb, in.world.xz, in.custom.x, in.custom.y, in.custom.z, in.custom.w, pixel, texel.rgb * tone, onGround);
${edges ? `    if (!asphalt.keep) {
      discard;
    }` : ''}
    rgb = asphalt.rgb;
  }
  var marks = CraterMark(vec2f(0.0), 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0);
  if ((flags & MAT_CRATERS) != 0u) {
    marks = craterMarks(in.world.xz, pixel, CRATER_SLOPE);
    rgb = mix(rgb, mix(CRATER_EARTH, CRATER_EARTH_DAMP, marks.damp), marks.earth);
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
  // Bare rock (rock.wgsl): the stones' map has in alpha how much of the ground is rock.
  if ((flags & MAT_ROCK) != 0u && stonesTexel.a > 0.004) {
    let above = stonesTexel.b * 4.0 - 1.0;
    let rock = rockLook(rgb, in.world.xz, stonesTexel.a * (1.0 - marks.earth), pixel, above);
    rgb = rock.rgb;
    stones.tilt += rock.tilt;
  }
  // The shore: the stones' map has in blue how far over the water near it the ground is (-1 to 3 m,
  // the client's \`SHORE_ABOVE\`). Up to about 0.3 m it is wet, darker and with a sheen, its edge
  // uneven as the waves leave it.
  var wet = 0.0;
  if ((flags & MAT_STONES) != 0u && stonesTexel.b < 0.999) {
    let above = stonesTexel.b * 4.0 - 1.0 + (marshValue(in.world.xz * 0.9) - 0.5) * 0.14;
    wet = (1.0 - smoothstep(0.2, 0.36, above)) * (1.0 - marks.earth);
    rgb *= mix(1.0, 0.6, wet);
  }
  // Scars (scars.wgsl): a damaged mesh's dents, soot, holes and gouges, worked out in its own frame.
  let scarObject = objects[in.ids.z];
  let scarred = scarObject.misc.w != 0u && scarObject.misc.z == 0u;
  var scar = ScarLook(vec3f(0.0), 1.0, 0.0, 0.0, 0.0, 0.0, vec3f(0.0), 0.0, vec3f(0.0));
  // The dents' slope in the world: the cofactor matrix over the determinant is the inverse-transpose.
  var scarSlope = vec3f(0.0);
  if (scarred) {
    let m = scarObject.model;
    let worldNormal = normalize(in.normal) * select(-1.0, 1.0, front);
    let localNormal = normalize(vec3f(dot(m[0].xyz, worldNormal), dot(m[1].xyz, worldNormal), dot(m[2].xyz, worldNormal)));
    let localUp = normalize(vec3f(m[0].y / dot(m[0].xyz, m[0].xyz), m[1].y / dot(m[1].xyz, m[1].xyz), m[2].y / dot(m[2].xyz, m[2].xyz)));
    scar = scarLook(scarObject.misc.w, toObject(m, in.world), localNormal, localUp, frame.fog.w);
    let det = abs(dot(m[0].xyz, cross(m[1].xyz, m[2].xyz)));
    let g = normalMatrix(m) * scar.slope / max(det, 1e-8);
    scarSlope = g - worldNormal * dot(g, worldNormal);
    rgb = mix(rgb, SCAR_BARE, scar.bare);
    rgb *= 1.0 - scar.soot * 0.8;
    rgb = mix(rgb, SCAR_CHAR, scar.char);
    rgb = mix(rgb, scar.temper, scar.temperAmount);
    rgb = mix(rgb, SCAR_HOLE_RGB, scar.hole);
    // The dent's relief: the side of it turned to the sun lit, the far side in shade, past the ramp's bands.
    rgb *= scar.shade * clamp(1.0 - SCAR_RELIEF * dot(scarSlope, frame.sunDir.xyz), 0.35, 1.8);
  }
  if ((flags & MAT_LIT) != 0u) {
    var n = normalize(in.normal);
    if ((flags & MAT_NORMAL_MAP) != 0u) {
      n = normalize(normalMatrix(objects[in.ids.z].model) * mapped);
    }
    // The dents' slope tips the normal.
    n = normalize(n - scarSlope);
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
    if (scar.bare > 0.0) {
      // Bare metal glints in the sun where paint is torn off it.
      let toEye = normalize(frame.cameraPos.xyz - in.world);
      let half = normalize(frame.sunDir.xyz + toEye);
      rgb += frame.sunColor.rgb * pow(max(dot(n, half), 0.0), 48.0) * scar.bare * shadow * 0.3;
    }
    if (rain.x + rain.y > 0.0) {
      rgb += wetSheen(n, in.world, rain);
    }
    if (marsh.pool + marsh.sheen > 0.0) {
      rgb += marshShine(n, in.world, marsh.pool, marsh.sheen, shadow);
    }
  }
  rgb = rgb * (1.0 - marks.ink * CRATER_INK) + CRATER_EMBER * marks.heat + CRATER_GLOW * marks.glow + scar.heat;
  return vec4f(applyFog(rgb, flags, in.viewDepth, in.world), in.color.a * texel.a);
}
`
