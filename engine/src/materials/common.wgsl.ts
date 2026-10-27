import { CRATERS_WGSL } from './craters.wgsl'

/**
 * Declarations shared by every scene shader: the per-frame uniforms and the object and material records.
 * The layouts must match their writers: FRAME_FLOATS in renderer.ts, OBJECT_FLOATS in gpu/objects.ts and
 * MATERIAL_FLOATS in gpu/materials.ts.
 */
export const COMMON_WGSL = /* wgsl */ `
struct Frame {
  viewProj: mat4x4f,
  view: mat4x4f,
  cameraPos: vec4f,
  // Unit vector toward the sun.
  sunDir: vec4f,
  // Colour times intensity.
  sunColor: vec4f,
  hemiSky: vec4f,
  hemiGround: vec4f,
  fogColor: vec4f,
  // near, far, on (0 or 1), time in seconds.
  fog: vec4f,
  // width, height, 1/width, 1/height in pixels.
  viewport: vec4f,
  shadowMatrix: mat4x4f,
  // texel size, blur radius in texels, on (0 or 1), unused.
  shadow: vec4f,
  // camera near, camera far, seconds since the last frame, unused.
  clip: vec4f,
  // Number of point lights, the height under which a reflection pass cuts the scene away, whether it does (0 or 1), unused.
  lightCount: vec4f,
  // Per light, two vec4s: xyz position and radius, then linear rgb times intensity.
  lights: array<vec4f, 16>,
}

struct Object {
  model: mat4x4f,
  tint: vec4f,
  // material index, flags (OBJ_*), first bone + 1 (0 when not skinned), unused.
  misc: vec4u,
}

struct Material {
  // linear rgb, opacity.
  color: vec4f,
  emissive: vec4f,
  // flags (MAT_*), ramp row, unused, unused.
  params: vec4u,
}

const MAT_LIT = 1u;
const MAT_VERTEX_COLORS = 2u;
const MAT_FOG = 4u;
const MAT_NORMAL_MAP = 8u;
const MAT_NORMAL_TONE = 16u;
const MAT_CRATERS = 32u;
const OBJ_RECEIVE_SHADOW = 1u;
const PI = 3.141592653589793;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> objects: array<Object>;
@group(0) @binding(2) var<storage, read> materials: array<Material>;
// This frame's draws, in draw order: instance_index picks an entry, the entry picks the object record.
@group(0) @binding(3) var<storage, read> drawList: array<u32>;
// Skinning matrices (bone world × inverse bind) of every skinned mesh drawn this frame.
@group(0) @binding(4) var<storage, read> bones: array<mat4x4f>;

// Light from this frame's point lights on a surface at \`world\` facing \`n\` (wrapped, so it still reads on the far side).
fn pointLight(world: vec3f, n: vec3f) -> vec3f {
  var sum = vec3f(0.0);
  let count = u32(frame.lightCount.x);
  for (var i = 0u; i < count; i++) {
    let to = frame.lights[i * 2u].xyz - world;
    let d = length(to);
    let radius = frame.lights[i * 2u].w;
    if (d < radius) {
      let fall = 1.0 - d / radius;
      let facing = dot(n, to / max(d, 1e-3)) * 0.6 + 0.4;
      sum += frame.lights[i * 2u + 1u].rgb * fall * fall * max(facing, 0.0);
    }
  }
  return sum;
}

// The object's model matrix, or for a skinned object the weighted blend of its vertex's bones.
fn modelMatrix(object: Object, joints: vec4u, weights: vec4f) -> mat4x4f {
  if (object.misc.z == 0u) {
    return object.model;
  }
  let b = object.misc.z - 1u;
  return bones[b + joints.x] * weights.x + bones[b + joints.y] * weights.y + bones[b + joints.z] * weights.z + bones[b + joints.w] * weights.w;
}

// The inverse-transpose of the model's upper 3×3, up to scale: the cofactor matrix. Normals only need
// direction, so this handles non-uniform scale without a per-object normal matrix.
fn normalMatrix(m: mat4x4f) -> mat3x3f {
  let c0 = m[0].xyz;
  let c1 = m[1].xyz;
  let c2 = m[2].xyz;
  let cof = mat3x3f(cross(c1, c2), cross(c2, c0), cross(c0, c1));
  return cof * sign(dot(c0, cof[0]));
}

${CRATERS_WGSL}
`
