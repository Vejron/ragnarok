import { PointLights } from '../scene/lights'
import type { Scene } from '../scene/scene'

/** Spot lights that show their beams at once: the strongest near the camera. */
export const MAX_BEAMS = 4
/** Steps along a view ray through each beam. */
const STEPS = 14
const FORMAT: GPUTextureFormat = 'rgba16float'
/** Floats in the uniform: the inverse view-projection, the camera and its forward, the clip, then four per-beam vec4s × 3. */
const FLOATS = 16 + 4 + 4 + 4 + MAX_BEAMS * 12

/**
 * Beams of light in the air: the cones of headlights, a headlamp and a searchlight seen in mist, rain
 * and the dark. At half the screen's size, each pixel's view ray is marched through the few strongest
 * spot lights near the camera, from the eye to whatever the depth buffer says it meets, gathering the
 * light the air scatters back toward the eye. The output pass adds the result.
 */
function beamsWgsl(multisampled: boolean) {
  return /* wgsl */ `
struct Beam {
  // Where the lamp is (world), how far it reaches (m).
  place: vec4f,
  // Unit direction it points, the cosine of its outer half-angle.
  aim: vec4f,
  // Its colour times strength (linear), the cosine of its inner half-angle.
  color: vec4f,
}

struct Beams {
  inverseViewProj: mat4x4f,
  // The camera (world), how dense the air is (light scattered per metre).
  camera: vec4f,
  // The camera's forward unit vector, the number of beams.
  forward: vec4f,
  // Camera near, camera far.
  clip: vec4f,
  beams: array<Beam, ${MAX_BEAMS}>,
}

@group(0) @binding(0) var<uniform> b: Beams;
@group(0) @binding(1) var depth: ${multisampled ? 'texture_depth_multisampled_2d' : 'texture_depth_2d'};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

// Henyey-Greenstein: the air scatters a lamp's light a little forward, so a beam coming toward the eye
// is brighter than one seen from behind its lamp, but not by so much that the second is lost.
fn phase(cosine: f32) -> f32 {
  let g = 0.15;
  return (1.0 - g * g) / (4.0 * 3.14159265 * pow(1.0 + g * g - 2.0 * g * cosine, 1.5));
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let full = vec2i(textureDimensions(depth));
  let p = min(vec2i(pos.xy) * 2, full - 1);
  let size = vec2f(full);
  let uv = (vec2f(p) + 0.5) / size;
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let near = b.inverseViewProj * vec4f(ndc, 1.0, 1.0);
  let dir = normalize(near.xyz / near.w - b.camera.xyz);
  // How far the ray goes before it meets something: reversed depth, 0 the far plane.
  let d = textureLoad(depth, p, 0);
  let z = b.clip.x * b.clip.y / (d * (b.clip.y - b.clip.x) + b.clip.x);
  let reach = z / max(dot(dir, b.forward.xyz), 0.05);
  let jitter = fract(52.9829189 * fract(dot(pos.xy, vec2f(0.06711056, 0.00583715))));
  var light = vec3f(0.0);
  for (var i = 0u; i < u32(b.forward.w); i++) {
    let beam = b.beams[i];
    let radius = beam.place.w;
    // Only the part of the ray inside the lamp's reach, and before what the ray meets.
    let o = b.camera.xyz - beam.place.xyz;
    let along = dot(o, dir);
    let disc = along * along - (dot(o, o) - radius * radius);
    if (disc <= 0.0) {
      continue;
    }
    let root = sqrt(disc);
    let enter = max(-along - root, 0.0);
    let leave = min(-along + root, reach);
    if (leave <= enter) {
      continue;
    }
    let step = (leave - enter) / ${STEPS}.0;
    var sum = 0.0;
    for (var s = 0; s < ${STEPS}; s++) {
      let q = b.camera.xyz + dir * (enter + (f32(s) + jitter) * step);
      let v = q - beam.place.xyz;
      let dist = max(length(v), 0.05);
      let toward = v / dist;
      let cone = smoothstep(beam.aim.w, beam.color.w, dot(toward, beam.aim.xyz));
      let fall = 1.0 - min(dist / radius, 1.0);
      sum += cone * fall * fall / (1.0 + 0.01 * dist * dist) * phase(dot(toward, -dir));
    }
    light += beam.color.rgb * sum * step;
  }
  return vec4f(light * b.camera.w, 1.0);
}
`
}

export class LampBeams {
  private texture: GPUTexture | null = null
  private group: GPUBindGroup | null = null
  private readonly pipeline: GPURenderPipeline
  private readonly uniform: GPUBuffer
  private readonly data = new Float32Array(FLOATS)
  /** The beams' light, half the screen's size, for the output pass. Valid after `resize`. */
  view: GPUTextureView | null = null
  /** Spot lights picked this frame, as indexes into the scene's live lights, and their scores. */
  private readonly picked: number[] = []
  private readonly scores: number[] = []

  constructor(private readonly device: GPUDevice, multisampled: boolean) {
    const module = device.createShaderModule({ label: 'beams', code: beamsWgsl(multisampled) })
    this.pipeline = device.createRenderPipeline({
      label: 'beams',
      layout: 'auto',
      vertex: { module, entryPoint: 'vs' },
      fragment: { module, entryPoint: 'fs', targets: [{ format: FORMAT }] },
      primitive: { topology: 'triangle-list' },
    })
    this.uniform = device.createBuffer({ label: 'beams', size: FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST })
  }

  /** (Re)makes the half-size target for a screen of `width`×`height`, reading `depth`. */
  resize(width: number, height: number, depth: GPUTexture) {
    this.texture?.destroy()
    this.texture = this.device.createTexture({
      label: 'beams',
      size: [Math.max(1, Math.ceil(width / 2)), Math.max(1, Math.ceil(height / 2))],
      format: FORMAT,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    })
    this.view = this.texture.createView()
    this.group = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: this.uniform } },
        { binding: 1, resource: depth.createView() },
      ],
    })
  }

  /**
   * Draws the beams of the strongest spot lights near the camera, if the scene asks for beams and has
   * any. Returns whether it drew (when not, the output pass must leave the target out).
   */
  encode(
    encoder: GPUCommandEncoder, scene: Scene, inverseViewProj: ArrayLike<number>, camera: readonly [number, number, number],
    forward: readonly [number, number, number], near: number, far: number, timestampWrites?: GPURenderPassTimestampWrites,
  ): boolean {
    const settings = scene.beams
    if (!settings || settings.density <= 0 || !this.group) return false
    const count = this.pick(scene.lights, camera)
    if (!count) return false
    const d = this.data
    d.set(inverseViewProj, 0)
    d[16] = camera[0]; d[17] = camera[1]; d[18] = camera[2]; d[19] = settings.density
    d[20] = forward[0]; d[21] = forward[1]; d[22] = forward[2]; d[23] = count
    d[24] = near; d[25] = far
    const live = scene.lights.live
    for (let i = 0; i < count; i++) {
      const l = live[this.picked[i]!]!
      const o = 28 + i * 12
      const strength = l.intensity * PointLights.decay(l)
      d[o] = l.x; d[o + 1] = l.y; d[o + 2] = l.z; d[o + 3] = l.radius
      d[o + 4] = l.dx; d[o + 5] = l.dy; d[o + 6] = l.dz; d[o + 7] = l.cosOuter
      d[o + 8] = l.r * strength; d[o + 9] = l.g * strength; d[o + 10] = l.b * strength; d[o + 11] = l.cosInner
    }
    this.device.queue.writeBuffer(this.uniform, 0, d)
    const pass = encoder.beginRenderPass({
      label: 'beams',
      colorAttachments: [{ view: this.view!, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      timestampWrites,
    })
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.group)
    pass.draw(3)
    pass.end()
    return true
  }

  /** Picks up to `MAX_BEAMS` spot lights by strength over distance from the camera; returns how many. */
  private pick(lights: PointLights, camera: readonly [number, number, number]): number {
    const picked = this.picked
    const scores = this.scores
    picked.length = 0
    scores.length = 0
    const live = lights.live
    for (let i = 0; i < live.length; i++) {
      const l = live[i]!
      // Spot lights only: a point light's glow is the bloom's.
      if (l.cosOuter <= -1.5) continue
      const distance = Math.hypot(l.x - camera[0], l.y - camera[1], l.z - camera[2])
      if (distance > l.radius + 60) continue
      const score = (l.intensity * PointLights.decay(l) * (l.r + l.g + l.b)) / (1 + (distance * distance) / 400)
      if (score <= 0) continue
      let at = scores.length
      while (at > 0 && scores[at - 1]! < score) at--
      if (at >= MAX_BEAMS) continue
      scores.splice(at, 0, score)
      picked.splice(at, 0, i)
      if (scores.length > MAX_BEAMS) {
        scores.pop()
        picked.pop()
      }
    }
    return picked.length
  }

  destroy() {
    this.texture?.destroy()
    this.uniform.destroy()
  }
}

export const BEAMS_WGSL = beamsWgsl
