/**
 * The vertex stage every mesh shader shares: it fetches the object record through the draw list,
 * transforms to clip space, and passes the material colour (× tint × vertex colour) along.
 * Needs COMMON_WGSL before it. With `displace`, the shader must also define
 * `fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f`, which moves each vertex in world
 * space before it is projected (a ShaderMaterial's `vertex` code: grass bending in the wind). `color` is
 * the vertex's colour as the fragment gets it: material × instance tint × vertex colour. `displace`
 * may also read `vertexModel`, the object's (or instance's) model matrix, and `vertexUv`, the vertex's uv,
 * and may write `vertexExtra`, three vec4s handed to the fragment stage interpolated (`Surface.extra`).
 */
export function vertexWgsl(displace: boolean) {
  return /* wgsl */ `
var<private> vertexModel: mat4x4f;
var<private> vertexUv: vec2f;
${displace ? 'var<private> vertexExtra: array<vec4f, 3>;' : ''}

struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) color: vec4f,
  @location(4) custom: vec4f,
  @location(5) joints: vec4u,
  @location(6) weights: vec4f,
  @builtin(instance_index) instance: u32,
}

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) color: vec4f,
  @location(3) uv: vec2f,
  @location(4) viewDepth: f32,
  @location(5) custom: vec4f,
  // material index, object flags, object record.
  @location(6) @interpolate(flat) ids: vec3u,
${displace ? '  @location(7) extra0: vec4f,\n  @location(8) extra1: vec4f,\n  @location(9) extra2: vec4f,' : ''}
}

@vertex
fn vs(in: VertexIn) -> VertexOut {
  let record = drawList[in.instance];
  let object = objects[record];
  let material = materials[object.misc.x];
  let model = modelMatrix(object, in.joints, in.weights);
  var color = material.color * object.tint;
  if ((material.params.x & MAT_VERTEX_COLORS) != 0u) {
    color *= in.color;
  }
  vertexModel = model;
  vertexUv = in.uv;
  ${displace ? 'let world = vec4f(displace((model * vec4f(in.position, 1.0)).xyz, in.custom, color), 1.0);' : 'let world = model * vec4f(in.position, 1.0);'}
  var out: VertexOut;
  out.clip = frame.viewProj * world;
  out.world = world.xyz;
  out.normal = normalMatrix(model) * in.normal;
  out.color = color;
  out.uv = in.uv;
  out.viewDepth = -(frame.view * world).z;
  out.custom = in.custom;
  out.ids = vec3u(object.misc.x, object.misc.y, record);
${displace ? '  out.extra0 = vertexExtra[0];\n  out.extra1 = vertexExtra[1];\n  out.extra2 = vertexExtra[2];' : ''}
  return out;
}

// Fades \`rgb\`, seen at \`world\`, into the fog: the sky behind it far off, and low mist (see SKY_WGSL).
fn applyFog(rgb: vec3f, flags: u32, viewDepth: f32, world: vec3f) -> vec3f {
  if ((flags & MAT_FOG) == 0u || frame.fog.z == 0.0) {
    return rgb;
  }
  return mix(rgb, fogTint(world), fogAmount(world, viewDepth));
}
`
}

export const VERTEX_WGSL = vertexWgsl(false)
