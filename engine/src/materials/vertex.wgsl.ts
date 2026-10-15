/**
 * The vertex stage every mesh shader shares: it fetches the object record through the draw list,
 * transforms to clip space, and passes the material colour (× tint × vertex colour) along.
 * Needs COMMON_WGSL before it.
 */
export const VERTEX_WGSL = /* wgsl */ `
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
}

@vertex
fn vs(in: VertexIn) -> VertexOut {
  let record = drawList[in.instance];
  let object = objects[record];
  let material = materials[object.misc.x];
  let model = modelMatrix(object, in.joints, in.weights);
  let world = model * vec4f(in.position, 1.0);
  var out: VertexOut;
  out.clip = frame.viewProj * world;
  out.world = world.xyz;
  out.normal = normalMatrix(model) * in.normal;
  var color = material.color * object.tint;
  if ((material.params.x & MAT_VERTEX_COLORS) != 0u) {
    color *= in.color;
  }
  out.color = color;
  out.uv = in.uv;
  out.viewDepth = -(frame.view * world).z;
  out.custom = in.custom;
  out.ids = vec3u(object.misc.x, object.misc.y, record);
  return out;
}

fn applyFog(rgb: vec3f, flags: u32, viewDepth: f32) -> vec3f {
  if ((flags & MAT_FOG) == 0u || frame.fog.z == 0.0) {
    return rgb;
  }
  return mix(rgb, frame.fogColor.rgb, smoothstep(frame.fog.x, frame.fog.y, viewDepth));
}
`
