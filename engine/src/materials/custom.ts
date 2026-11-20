import { Color } from '../math/vec'
import type { ShaderMaterial, UniformValue } from '../scene/material'
import { COMMON_WGSL } from './common.wgsl'
import { SUN_SHADOW_WGSL } from './sunshadow.wgsl'
import { vertexWgsl } from './vertex.wgsl'

/** What a ShaderMaterial compiles to: its shader, and how its uniforms pack into a buffer. */
export interface CustomShader {
  /** Identifies the shader and its resource layout: materials with equal keys share a pipeline. */
  key: string
  code: string
  textureNames: string[]
  uniformBytes: number
  /** Packs the material's current uniform values. */
  pack(material: ShaderMaterial, out: Float32Array): void
}

interface Field {
  name: string
  type: 'f32' | 'vec2f' | 'vec3f' | 'vec4f'
  offset: number
}

const SIZE = { f32: 4, vec2f: 8, vec3f: 12, vec4f: 16 }
const ALIGN = { f32: 4, vec2f: 8, vec3f: 16, vec4f: 16 }

function typeOf(value: UniformValue): Field['type'] {
  if (typeof value === 'number') return 'f32'
  if (value instanceof Color) return 'vec3f'
  return (['vec2f', 'vec3f', 'vec4f'] as const)[value.length - 2]!
}

/** A material with `shadows` takes the renderer's lighting group (see standard.wgsl) at group 2. */
const SHADOW_BINDINGS = /* wgsl */ `
@group(2) @binding(1) var shadowMap: texture_depth_2d;
@group(2) @binding(2) var shadowSampler: sampler_comparison;
@group(2) @binding(3) var farShadowMap: texture_depth_2d;
${SUN_SHADOW_WGSL}
`

/** Where a shader material's code takes the mirror image's clip (`mirrorCode`), or nothing. */
const MIRROR_CLIP_MARK = '  // (mirror clip)'

/**
 * `code` (a `CustomShader`'s) for the mirror image's draw, which is cut off at the water. Only that draw
 * has it: a shader that can discard is shaded for every hidden layer on a tiled GPU, whether or not it
 * ever does, so the main view's must not hold the statement at all.
 */
export function mirrorCode(code: string): string {
  return code.replace(MIRROR_CLIP_MARK, `  if (frame.lightCount.z > 0.5 && dot(in.world, frame.mirror.xyz) < frame.mirror.w) {
    discard;
  }`)
}

/** Builds the WGSL and uniform packing for a ShaderMaterial (fields laid out by WGSL's uniform rules). */
export function compileCustom(material: ShaderMaterial): CustomShader {
  const fields: Field[] = []
  let offset = 0
  for (const [name, value] of Object.entries(material.uniforms)) {
    const type = typeOf(value)
    offset = Math.ceil(offset / ALIGN[type]) * ALIGN[type]
    fields.push({ name, type, offset })
    offset += SIZE[type]
  }
  if (!fields.length) {
    fields.push({ name: '_unused', type: 'vec4f', offset: 0 })
    offset = 16
  }
  const uniformBytes = Math.ceil(offset / 16) * 16
  const textureNames = Object.keys(material.textures)
  const struct = fields.map((f) => `  ${f.name}: ${f.type},`).join('\n')
  const textures = textureNames.map((name, i) => `@group(1) @binding(${i + 2}) var ${name}: texture_2d<f32>;`).join('\n')
  const code = /* wgsl */ `
${COMMON_WGSL}

struct Params {
${struct}
}
@group(1) @binding(0) var<uniform> params: Params;
@group(1) @binding(1) var materialSampler: sampler;
${textures}
${material.shadows ? SHADOW_BINDINGS : ''}

${material.vertex ?? ''}
${vertexWgsl(material.vertex !== undefined)}

struct Surface {
  world: vec3f,
  normal: vec3f,
  color: vec4f,
  uv: vec2f,
  custom: vec4f,
  front: bool,
  viewDepth: f32,
  extra: array<vec4f, 3>,
  // The pixel's place in the target, from its top left corner, at the pixel's middle.
  pixel: vec2f,
}

// Which draw of a material with a depth pre-pass this is: 0 the only one, 1 depth only, 2 shaded after it.
override PREPASS: u32 = 0u;
${material.coverage ? `// How many samples a pixel has, and which of them the surface covers (\`ShaderMaterial.coverage\`).
override SAMPLES: u32 = 4u;
var<private> surfaceMask: u32 = 0xffffffffu;` : ''}

struct FragmentOut {
  @location(0) color: vec4f,
${material.coverage ? '  @builtin(sample_mask) mask: u32,' : ''}
}

${material.wgsl}

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> FragmentOut {
  var n = normalize(in.normal);
  if (!front) {
    n = -n;
  }
  let flags = materials[in.ids.x].params.x;
  let extra = ${material.vertex !== undefined ? 'array<vec4f, 3>(in.extra0, in.extra1, in.extra2)' : 'array<vec4f, 3>()'};
  let c = surface(Surface(in.world, n, in.color, in.uv, in.custom, front, in.viewDepth, extra, in.clip.xy));
  var out: FragmentOut;
${material.coverage ? '  out.mask = surfaceMask;' : ''}
  if (PREPASS == 1u) {
    return out;
  }
${MIRROR_CLIP_MARK}
  out.color = vec4f(applyFog(c.rgb, flags, in.viewDepth, in.world), c.a);
  return out;
}
`
  const key = `${material.wgsl}|${material.vertex ?? ''}|${material.shadows}|${material.coverage}|${fields.map((f) => `${f.name}:${f.type}`).join(',')}|${textureNames.join(',')}`
  return {
    key,
    code,
    textureNames,
    uniformBytes,
    pack(m, out) {
      for (const field of fields) {
        const value = m.uniforms[field.name]
        if (value === undefined) continue
        const o = field.offset / 4
        if (typeof value === 'number') out[o] = value
        else if (value instanceof Color) out.set([value.r, value.g, value.b], o)
        else out.set(value, o)
      }
    },
  }
}
