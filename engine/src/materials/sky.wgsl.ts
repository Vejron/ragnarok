import { ATMOSPHERE_WGSL } from '../scene/atmosphere'

/** Metres above the world's zero at which the cloud layer lies. */
export const CLOUD_HEIGHT = 1800
/** Metres the cloud map spans before it repeats. */
export const CLOUD_TILE = 6000
/** Texels a side of the cloud map's top level. */
export const CLOUD_MAP_SIZE = 256

/**
 * The sky as every scene shader sees it (included in COMMON_WGSL): the atmosphere's radiance read from
 * the sky-view table (`passes/sky.ts`), the haze that colours the fog, the cloud layer and its shadow
 * on the ground, the sun, the moon and the stars. Needs the `Frame` struct before it.
 */
export const SKY_WGSL = /* wgsl */ `
${ATMOSPHERE_WGSL}

// The atmosphere's radiance around the eye for a sun of illuminance 1 (Hillaire's sky-view table).
@group(0) @binding(9) var skyView: texture_2d<f32>;
// The cloud layer's shapes: red is the broad cover, evened out so its values are spread uniformly over
// 0 to 1 and the cover can cut it at any level; green is finer detail. Both repeat.
@group(0) @binding(10) var cloudMap: texture_2d<f32>;
@group(0) @binding(11) var skySampler: sampler;
@group(0) @binding(12) var cloudSampler: sampler;

const CLOUD_HEIGHT = ${CLOUD_HEIGHT.toFixed(1)};
const CLOUD_TILE = ${CLOUD_TILE.toFixed(1)};
const CLOUD_MAP_SIZE = ${CLOUD_MAP_SIZE.toFixed(1)};
// Twice the cloud layer's height over the radius of the world it curves around: a world much smaller
// than the Earth, so the clouds visibly bow down to the horizon some 40 km out.
const CLOUD_CURVE = 0.0045;
// The sun's and the moon's angular radii, radians: about twice and five times the real ones, so they read on screen.
const SUN_RADIUS = 0.0095;
const MOON_RADIUS = 0.022;
const MOON_COLOR = vec3f(0.92, 0.94, 1.0);
const LUMA = vec3f(0.2126, 0.7152, 0.0722);
// The haze's and the clouds' brightness past which their glare is held back, and how far above it they
// may still rise (see softCap): a large patch of sky over the bloom's threshold floods the screen.
const HAZE_KNEE = 0.85;
const HAZE_ROOM = 0.35;
const CLOUD_KNEE = 1.0;
const CLOUD_ROOM = 0.45;

// \`c\` with its brightest channel above \`knee\` eased toward at most \`knee + room\`, keeping its hue. The
// brightest channel, not the luminance, as the bloom's threshold reads it: a warm glow is mostly red.
fn softCap(c: vec3f, knee: f32, room: f32) -> vec3f {
  let l = max(c.r, max(c.g, c.b));
  if (l <= knee) {
    return c;
  }
  return c * (knee + room * (1.0 - exp(-(l - knee) / room))) / l;
}

// Where the sky-view table holds direction \`dir\`: across, the angle from the sun's bearing (fine near
// the sun); down, the angle from the zenith, fine near the horizon on both sides of it.
fn skyViewUv(dir: vec3f) -> vec2f {
  let r = ATMO_BOTTOM + ATMO_EYE;
  let horizon = sqrt(r * r - ATMO_BOTTOM * ATMO_BOTTOM);
  let beta = acos(horizon / r);
  let zenithHorizon = PI - beta;
  let zenith = acos(clamp(dir.y, -1.0, 1.0));
  var v: f32;
  if (zenith < zenithHorizon) {
    v = (1.0 - sqrt(max(1.0 - zenith / zenithHorizon, 0.0))) * 0.5;
  } else {
    v = sqrt(max((zenith - zenithHorizon) / beta, 0.0)) * 0.5 + 0.5;
  }
  let flatSun = frame.skySun.xz;
  let flatView = dir.xz;
  let lengths = length(flatSun) * length(flatView);
  let cosPhi = select(1.0, dot(flatSun, flatView) / lengths, lengths > 1e-6);
  return vec2f(sqrt(clamp(-cosPhi * 0.5 + 0.5, 0.0, 1.0)), v);
}

// The atmosphere along \`dir\`, tinted, with the night sky's glow under it.
fn skyAtmosphere(dir: vec3f) -> vec3f {
  let lut = textureSampleLevel(skyView, skySampler, skyViewUv(dir), 0.0).rgb;
  return lut * frame.skyTint.rgb + frame.skyNight.rgb;
}

// How grey the cloud cover makes the whole sky, 0 to 1.
fn overcast() -> f32 {
  return smoothstep(0.55, 1.0, frame.clouds.x) * 0.85;
}

// The sky toward \`dir\` with nothing in it, never looking below the horizon: what the fog fades to.
// The haze's glare round a low sun is many times brighter than the rest of the sky; past \`HAZE_KNEE\` it
// is held back (softCap), so it glows without washing the world out through the bloom.
fn skyHaze(dir: vec3f) -> vec3f {
  let level = normalize(vec3f(dir.x, max(dir.y, 0.0), dir.z) + vec3f(0.0, 1e-4, 0.0));
  let c = softCap(skyAtmosphere(level), HAZE_KNEE, HAZE_ROOM);
  let grey = vec3f(dot(c, LUMA)) * vec3f(0.97, 0.99, 1.03);
  return mix(c, grey, overcast());
}

// The cloud layer's thickness at world \`xz\`, 0 to 1, reading the map at mip level \`lod\`.
fn cloudDensity(xz: vec2f, lod: f32) -> f32 {
  let uv = (xz + frame.clouds.yz) / CLOUD_TILE;
  let broad = textureSampleLevel(cloudMap, cloudSampler, uv, lod).r;
  let fine = textureSampleLevel(cloudMap, cloudSampler, uv * 3.0 + vec2f(0.37, 0.71), lod).g;
  let n = broad + (fine - 0.5) * 0.16;
  let edge = 1.0 - frame.clouds.x;
  return smoothstep(edge - 0.02, edge + 0.16, n);
}

// How much of the light reaches \`world\` through the clouds, 0 to 1: the drifting shadows of the cloud
// layer, cast along the light (the sun, or the moon at night).
fn cloudShade(world: vec3f) -> f32 {
  let strength = frame.clouds.w;
  let l = frame.sunDir.xyz;
  if (strength <= 0.0 || frame.clouds.x <= 0.0 || l.y < 0.02) {
    return 1.0;
  }
  let t = max(CLOUD_HEIGHT - world.y, 0.0) / l.y;
  let xz = world.xz + l.xz * t;
  let uv = (xz + frame.clouds.yz) / CLOUD_TILE;
  // The broad cover alone, a little blurred: a shadow's edge is soft that far below its cloud.
  let n = textureSampleLevel(cloudMap, cloudSampler, uv, 1.0).r;
  let edge = 1.0 - frame.clouds.x;
  return 1.0 - smoothstep(edge - 0.02, edge + 0.2, n) * strength;
}

// How far along \`dir\` the eye meets the cloud layer, metres, bowed over a small world (see CLOUD_CURVE).
fn cloudDistance(dir: vec3f) -> f32 {
  let h = max(CLOUD_HEIGHT - frame.cameraPos.y, 50.0);
  let mu = max(dir.y, 0.0);
  return 2.0 * h / (mu + sqrt(mu * mu + CLOUD_CURVE));
}

// Where along the cloud map \`dir\` meets the layer, in texels of its top level: for the mip level, by derivatives.
fn cloudTexel(dir: vec3f) -> vec2f {
  let xz = frame.cameraPos.xz + dir.xz * cloudDistance(dir);
  return (xz + frame.clouds.yz) / CLOUD_TILE * CLOUD_MAP_SIZE;
}

// The cloud layer along \`dir\`, lit by the sun and the moon over the sky's glow: colour and cover.
// \`behind\` is the sky it is drawn over, which the furthest clouds fade into.
fn cloudLayer(dir: vec3f, lod: f32, behind: vec3f) -> vec4f {
  if (dir.y <= 0.0 || frame.clouds.x <= 0.0) {
    return vec4f(0.0);
  }
  let t = cloudDistance(dir);
  let xz = frame.cameraPos.xz + dir.xz * t;
  let d = cloudDensity(xz, lod);
  if (d <= 0.002) {
    return vec4f(0.0);
  }
  // A cloud thick on the side toward the light is in its own shade on the side the eye sees.
  let sun = frame.skySun.xyz;
  let sunFlat = select(vec2f(0.0), normalize(sun.xz), dot(sun.xz, sun.xz) > 1e-6);
  let toSun = cloudDensity(xz + sunFlat * 220.0, lod);
  // A low sun lights the clouds from underneath, all of them: the glow of a sunset.
  let low = 1.0 - smoothstep(0.04, 0.4, sun.y);
  let lit = mix(mix(0.45, 0.8, low), 1.0, exp(-toSun * 2.2));
  // Light scattered forward through a cloud's edge: the silver lining toward the sun.
  let nu = max(dot(dir, sun), 0.0);
  let lining = 1.0 + (1.0 - d) * (1.6 * pow(nu, 12.0) + 0.5 * pow(nu, 3.0));
  // A little less saturated than the light itself: a red sun makes pink and orange clouds, not pure red ones.
  let light = mix(vec3f(dot(frame.skyLight.rgb, LUMA)), frame.skyLight.rgb, 0.82);
  let sunlight = light * lit * lining * (1.0 + 1.3 * low);
  let moonlight = frame.sunColor.rgb * frame.skyLight.w * 0.35 * lit;
  let ambient = skyAtmosphere(vec3f(0.0, 1.0, 0.0)) * 1.1 + frame.skyNight.rgb * 1.5;
  // Thick clouds are darker underneath; overcast is a flat grey.
  var c = (sunlight + moonlight + ambient) * mix(1.0, 0.72, d * d);
  c = mix(c, vec3f(dot(c, LUMA)), overcast() * 0.5);
  // Bright edges round the sun are held back as the haze is, so they glow without flooding the bloom.
  c = softCap(c, CLOUD_KNEE, CLOUD_ROOM);
  // Far ones sink into the haze along the horizon.
  let far = smoothstep(4000.0, 60000.0, t);
  c = mix(c, behind, far * 0.8);
  return vec4f(c, d * mix(1.0, 0.55, far) * smoothstep(0.0, 0.04, dir.y));
}

fn starHash(p: vec3f) -> vec3f {
  var q = fract(p * vec3f(0.1031, 0.1030, 0.0973));
  q += dot(q, q.yxz + 33.33);
  return fract((q.xxy + q.yxx) * q.zyx);
}

// The night sky toward \`dir\`: stars that twinkle, and the faint band of the Milky Way. They turn with the
// night about a pole tilted 30° off the zenith toward -z.
fn starField(dir: vec3f) -> vec3f {
  let pole = vec3f(0.0, 0.866, -0.5);
  let a = frame.skyNight.w;
  let v = dir * cos(a) + cross(pole, dir) * sin(a) + pole * dot(pole, dir) * (1.0 - cos(a));
  let p = v * 110.0;
  let cell = floor(p);
  let h = starHash(cell);
  var c = vec3f(0.0);
  if (h.z < 0.22) {
    let centre = cell + 0.3 + 0.4 * starHash(cell + 17.0);
    let off = centre - p;
    let across = off - v * dot(off, v);
    let glow = exp(-dot(across, across) / 0.012);
    let bright = 0.05 + pow(h.x, 7.0) * 4.0;
    let twinkle = 0.75 + 0.25 * sin(frame.fog.w * (1.5 + h.y * 4.0) + h.x * 61.0);
    c = mix(vec3f(0.72, 0.84, 1.12), vec3f(1.12, 0.95, 0.75), h.y) * bright * twinkle * glow;
  }
  // The Milky Way: a band about a great circle, clumped by the cloud map.
  let band = dot(v, normalize(vec3f(0.35, 0.55, 0.76)));
  let dust = textureSampleLevel(cloudMap, cloudSampler, v.xz * 0.45 + v.y * 0.3, 1.0).r;
  c += vec3f(0.025, 0.03, 0.045) * exp(-band * band / 0.025) * (0.3 + dust * 1.2);
  return c;
}

// The sun's disc, darker at its limb, and the glare close round it, through the atmosphere's colour.
fn sunDisc(dir: vec3f) -> vec3f {
  let sun = frame.skySun.xyz;
  let c = dot(dir, sun);
  if (c <= 0.0) {
    return vec3f(0.0);
  }
  // The sine of the angle off the sun, exact where an arccosine of the dot product would be ragged.
  let s = length(cross(dir, sun)) / SUN_RADIUS;
  let disc = 1.0 - smoothstep(0.85, 1.0, s);
  let limb = 1.0 - 0.45 * (1.0 - sqrt(max(1.0 - s * s, 0.0)));
  let glare = pow(c, 3000.0) * 0.8 + pow(c, 220.0) * 0.06;
  return frame.skyLight.rgb * (disc * limb * frame.skySun.w + glare);
}

// The moon's disc lit from the side its phase says, with darker seas and a faint halo.
fn moonDisc(dir: vec3f) -> vec3f {
  let m = frame.moon.xyz;
  let c = dot(dir, m);
  if (c <= 0.0) {
    return vec3f(0.0);
  }
  let halo = MOON_COLOR * (pow(c, 400.0) * 0.12 + pow(c, 30.0) * 0.012);
  let s = length(cross(dir, m)) / MOON_RADIUS;
  if (s > 1.0) {
    return halo;
  }
  let side = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(m.y) < 0.99);
  let right = normalize(cross(m, side));
  let upward = cross(right, m);
  let q = vec2f(dot(dir, right), dot(dir, upward)) / MOON_RADIUS;
  let z = sqrt(max(1.0 - dot(q, q), 0.0));
  let n = right * q.x + upward * q.y - m * z;
  let phase = frame.moon.w;
  let light = -m * cos(phase) + right * sin(phase);
  let lit = smoothstep(-0.04, 0.12, dot(n, light));
  let seas = textureSampleLevel(cloudMap, cloudSampler, q * 0.16 + vec2f(0.31, 0.57), 0.0).r;
  let albedo = mix(0.6, 1.0, smoothstep(0.3, 0.7, seas));
  let edge = 1.0 - smoothstep(0.9, 1.0, s);
  return MOON_COLOR * albedo * (lit * 2.2 + 0.03) * edge + halo;
}

// Everything in the sky toward \`dir\`: the atmosphere, the stars, moon and sun, and the clouds over them.
// \`lod\` is the cloud map's mip level there.
fn skyColor(dir: vec3f, lod: f32) -> vec3f {
  let haze = skyHaze(dir);
  var c = haze;
  let above = smoothstep(-0.005, 0.02, dir.y);
  let clear = 1.0 - overcast();
  if (frame.skyTint.w > 0.0) {
    c += starField(dir) * frame.skyTint.w * above * clear;
  }
  if (frame.skyLight.w > 0.0) {
    c += moonDisc(dir) * frame.skyLight.w * above * mix(0.25, 1.0, clear);
  }
  c += sunDisc(dir) * above * mix(0.1, 1.0, clear);
  let cloud = cloudLayer(dir, lod, haze);
  return mix(c, cloud.rgb, cloud.a);
}

// How much of the view to \`world\` is fog, 0 to 1: the distance fog that hides the world's edge, with
// the low mist (exponential in height, integrated along the view) over it.
fn fogAmount(world: vec3f, viewDepth: f32) -> f32 {
  var f = smoothstep(frame.fog.x, frame.fog.y, viewDepth);
  let density = frame.heightFog.x;
  if (density > 0.0) {
    let v = world - frame.cameraPos.xyz;
    let k = frame.heightFog.z;
    let a = density * exp(-max(k * (frame.cameraPos.y - frame.heightFog.y), -8.0));
    let ky = k * v.y;
    let integral = a * length(v) * select((1.0 - exp(-ky)) / ky, 1.0, abs(ky) < 1e-3);
    f = 1.0 - (1.0 - f) * exp(-integral);
  }
  return f;
}

// What the fog toward \`world\` is coloured: the sky behind it when there is one, else the flat fog colour.
fn fogTint(world: vec3f) -> vec3f {
  if (frame.heightFog.w < 0.5) {
    return frame.fogColor.rgb;
  }
  return skyHaze(normalize(world - frame.cameraPos.xyz));
}
`
