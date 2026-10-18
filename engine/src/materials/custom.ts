import { Color } from '../math/vec'
import type { ShaderMaterial, UniformValue } from '../scene/material'
import { COMMON_WGSL } from './common.wgsl'
import { VERTEX_WGSL } from './vertex.wgsl'

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
${VERTEX_WGSL}

struct Params {
${struct}
}
@group(1) @binding(0) var<uniform> params: Params;
@group(1) @binding(1) var materialSampler: sampler;
${textures}

struct Surface {
  world: vec3f,
  normal: vec3f,
  color: vec4f,
  uv: vec2f,
  custom: vec4f,
  front: bool,
  viewDepth: f32,
}

${material.wgsl}

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
  var n = normalize(in.normal);
  if (!front) {
    n = -n;
  }
  let flags = materials[in.ids.x].params.x;
  let c = surface(Surface(in.world, n, in.color, in.uv, in.custom, front, in.viewDepth));
  if (frame.lightCount.z > 0.5 && in.world.y < frame.lightCount.y) {
    discard;
  }
  return vec4f(applyFog(c.rgb, flags, in.viewDepth), c.a);
}
`
  const key = `${material.wgsl}|${fields.map((f) => `${f.name}:${f.type}`).join(',')}|${textureNames.join(',')}`
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
