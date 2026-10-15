import { compileCustom, type CustomShader } from '../materials/custom'
import type { Material, ShaderMaterial } from '../scene/material'
import type { TextureStore } from './textures'

/** A compiled ShaderMaterial program: shared by every material with the same code and resource layout. */
export interface CustomProgram {
  shader: CustomShader
  module: GPUShaderModule
  groupLayout: GPUBindGroupLayout
}

interface TextureGroup {
  id: number
  group: GPUBindGroup
  used: number
}

interface CustomState {
  program: CustomProgram
  buffer: GPUBuffer
  data: Float32Array<ArrayBuffer>
  version: number
  texturesKey: string
  id: number
  group: GPUBindGroup | null
}

/** Frames an unused texture bind group is kept before it is dropped. */
const RETAIN_FRAMES = 300

/**
 * The bind groups that differ between materials: textures for standard materials (shared by every
 * material with the same maps, so they still batch), and uniforms plus textures for ShaderMaterials.
 * Each group has a small integer id the renderer sorts and batches by; id 0 is "no textures".
 */
export class MaterialResources {
  readonly texturesLayout: GPUBindGroupLayout
  private readonly groups = new Map<number, GPUBindGroup>()
  private readonly textureGroups = new Map<string, TextureGroup>()
  private readonly customs = new Map<ShaderMaterial, CustomState>()
  private readonly programs = new Map<string, CustomProgram>()
  private readonly compiled = new WeakMap<ShaderMaterial, CustomShader>()
  private nextId = 1
  private frame = 0

  constructor(private readonly device: GPUDevice, private readonly textures: TextureStore) {
    const fragment = GPUShaderStage.FRAGMENT
    this.texturesLayout = device.createBindGroupLayout({
      label: 'material textures',
      entries: [
        { binding: 0, visibility: fragment, texture: {} },
        { binding: 1, visibility: fragment, texture: {} },
        { binding: 2, visibility: fragment, sampler: {} },
      ],
    })
    this.groups.set(0, this.makeTextureGroup(null, null))
  }

  group(id: number): GPUBindGroup {
    return this.groups.get(id)!
  }

  /** The group id for a standard material's textures, uploading textures as needed. */
  standard(material: Material): number {
    const map = material.map
    const normal = material.normalMap
    if (!map && !normal) return 0
    const key = `${map ? `${map.id}:${map.version}` : '-'}|${normal ? `${normal.id}:${normal.version}` : '-'}`
    let entry = this.textureGroups.get(key)
    if (!entry) {
      const id = this.nextId++
      entry = { id, group: this.makeTextureGroup(map, normal), used: this.frame }
      this.textureGroups.set(key, entry)
      this.groups.set(id, entry.group)
    }
    entry.used = this.frame
    return entry.id
  }

  /** The program a ShaderMaterial compiles to. */
  program(material: ShaderMaterial): CustomProgram {
    let shader = this.compiled.get(material)
    if (!shader) {
      shader = compileCustom(material)
      this.compiled.set(material, shader)
    }
    let program = this.programs.get(shader.key)
    if (!program) {
      const fragment = GPUShaderStage.FRAGMENT
      program = {
        shader,
        module: this.device.createShaderModule({ label: 'shader material', code: shader.code }),
        groupLayout: this.device.createBindGroupLayout({
          label: 'shader material',
          entries: [
            { binding: 0, visibility: fragment, buffer: { type: 'uniform' } },
            { binding: 1, visibility: fragment, sampler: {} },
            ...shader.textureNames.map((_, i) => ({ binding: i + 2, visibility: fragment, texture: {} })),
          ],
        }),
      }
      this.programs.set(shader.key, program)
    }
    return program
  }

  /** The group id for a ShaderMaterial: its uniforms (re-sent when its version changes) and textures. */
  custom(material: ShaderMaterial): number {
    let state = this.customs.get(material)
    if (!state) {
      const program = this.program(material)
      state = {
        program,
        buffer: this.device.createBuffer({ label: 'shader material params', size: program.shader.uniformBytes, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST }),
        data: new Float32Array(program.shader.uniformBytes / 4),
        version: -1,
        texturesKey: '',
        id: this.nextId++,
        group: null,
      }
      this.customs.set(material, state)
    }
    if (material.disposed) {
      state.buffer.destroy()
      this.customs.delete(material)
      this.groups.delete(state.id)
      return 0
    }
    if (state.version !== material.version) {
      state.version = material.version
      state.program.shader.pack(material, state.data)
      this.device.queue.writeBuffer(state.buffer, 0, state.data)
    }
    const names = state.program.shader.textureNames
    const texturesKey = names.map((name) => {
      const t = material.textures[name]
      return t ? `${t.id}:${t.version}` : '-'
    }).join('|')
    if (!state.group || texturesKey !== state.texturesKey) {
      state.texturesKey = texturesKey
      const first = names.length ? material.textures[names[0]!] ?? null : null
      state.group = this.device.createBindGroup({
        label: 'shader material',
        layout: state.program.groupLayout,
        entries: [
          { binding: 0, resource: { buffer: state.buffer } },
          { binding: 1, resource: this.textures.sampler(first) },
          ...names.map((name, i) => {
            const t = material.textures[name]
            return { binding: i + 2, resource: t ? this.textures.view(t) : this.textures.white }
          }),
        ],
      })
      this.groups.set(state.id, state.group)
    }
    return state.id
  }

  /** Drops texture groups nobody has used for a while. Call once per frame. */
  sweep() {
    this.frame++
    if (this.frame % 60) return
    for (const [key, entry] of this.textureGroups) {
      if (this.frame - entry.used < RETAIN_FRAMES) continue
      this.textureGroups.delete(key)
      this.groups.delete(entry.id)
    }
  }

  private makeTextureGroup(map: Material['map'], normal: Material['normalMap']): GPUBindGroup {
    return this.device.createBindGroup({
      label: 'material textures',
      layout: this.texturesLayout,
      entries: [
        { binding: 0, resource: map ? this.textures.view(map) : this.textures.white },
        { binding: 1, resource: normal ? this.textures.view(normal) : this.textures.flat },
        { binding: 2, resource: this.textures.sampler(map ?? normal) },
      ],
    })
  }
}

