import { AnimationClip, type Interpolation, type Track, type TrackPath } from '../animation/clip'
import { Color } from '../math/vec'
import { Geometry } from '../scene/geometry'
import { Material } from '../scene/material'
import { Mesh } from '../scene/mesh'
import { Node } from '../scene/node'
import { SkinnedMesh, Skeleton } from '../scene/skin'

/** What a GLB holds: its scene as a node tree, and its animations. */
export interface Gltf {
  scene: Node
  clips: AnimationClip[]
}

interface Accessor {
  bufferView?: number
  byteOffset?: number
  componentType: number
  normalized?: boolean
  count: number
  type: string
}

interface Primitive {
  attributes: Record<string, number>
  indices?: number
  material?: number
  mode?: number
}

interface GltfJson {
  scene?: number
  scenes?: { nodes?: number[] }[]
  nodes: {
    name?: string
    mesh?: number
    skin?: number
    children?: number[]
    matrix?: number[]
    translation?: number[]
    rotation?: number[]
    scale?: number[]
  }[]
  meshes?: { name?: string; primitives: Primitive[] }[]
  materials?: {
    name?: string
    doubleSided?: boolean
    alphaMode?: string
    pbrMetallicRoughness?: { baseColorFactor?: number[] }
    emissiveFactor?: number[]
  }[]
  accessors: Accessor[]
  bufferViews: { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number }[]
  skins?: { joints: number[]; inverseBindMatrices?: number }[]
  animations?: { name?: string; channels: { sampler: number; target: { node?: number; path: string } }[]; samplers: { input: number; output: number; interpolation?: string }[] }[]
}

const COMPONENTS: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 }
const ARRAYS: Record<number, { new (buffer: ArrayBuffer, offset: number, length: number): ArrayLike<number>; BYTES_PER_ELEMENT: number }> = {
  5120: Int8Array, 5121: Uint8Array, 5122: Int16Array, 5123: Uint16Array, 5125: Uint32Array, 5126: Float32Array,
}
/** Divisors for normalised integer components. */
const NORMALISE: Record<number, number> = { 5120: 127, 5121: 255, 5122: 32767, 5123: 65535 }

/**
 * Reads a binary glTF (.glb): meshes as Mesh (or SkinnedMesh) nodes with a toon-lit Material per glTF
 * material (base colour, emissive, double-sided, blend), node transforms, skins and animations. Textures
 * and morph targets are not read. A node with one primitive becomes the mesh itself; with several, a
 * node holding one mesh per primitive (as three's GLTFLoader does), so lookups by name find the same.
 */
export function parseGlb(data: ArrayBuffer): Gltf {
  const view = new DataView(data)
  if (view.getUint32(0, true) !== 0x46546c67) throw new Error('Not a GLB file')
  let offset = 12
  let json: GltfJson | null = null
  let bin: ArrayBuffer | null = null
  while (offset < data.byteLength) {
    const length = view.getUint32(offset, true)
    const type = view.getUint32(offset + 4, true)
    const chunk = data.slice(offset + 8, offset + 8 + length)
    if (type === 0x4e4f534a) json = JSON.parse(new TextDecoder().decode(chunk)) as GltfJson
    else if (type === 0x004e4942) bin = chunk
    offset += 8 + length
  }
  if (!json) throw new Error('GLB has no JSON chunk')
  return build(json, bin ?? new ArrayBuffer(0))
}

/** Fetches and parses a .glb. */
export async function loadGlb(url: string): Promise<Gltf> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)
  return parseGlb(await response.arrayBuffer())
}

function build(json: GltfJson, bin: ArrayBuffer): Gltf {
  const cache = new Map<number, Float32Array | Uint8Array | Uint16Array | Uint32Array>()

  /** An accessor's data as a tightly packed typed array (floats when normalised). */
  const read = (index: number) => {
    const hit = cache.get(index)
    if (hit) return hit
    const a = json.accessors[index]!
    const components = COMPONENTS[a.type]!
    const Array = ARRAYS[a.componentType]!
    const n = a.count * components
    let out: Float32Array | Uint8Array | Uint16Array | Uint32Array
    const target = a.normalized || a.componentType === 5126 ? new Float32Array(n) : new (Array as unknown as new (n: number) => Uint8Array | Uint16Array | Uint32Array)(n)
    if (a.bufferView !== undefined) {
      const bv = json.bufferViews[a.bufferView]!
      const start = (bv.byteOffset ?? 0) + (a.byteOffset ?? 0)
      const size = Array.BYTES_PER_ELEMENT
      const stride = bv.byteStride && bv.byteStride !== components * size ? bv.byteStride : components * size
      const divisor = a.normalized ? NORMALISE[a.componentType] ?? 1 : 1
      if (stride === components * size && start % size === 0) {
        const src = new Array(bin, start, n)
        for (let i = 0; i < n; i++) target[i] = src[i]! / divisor
      } else {
        // Interleaved or unaligned: copy element by element.
        const bytes = new Uint8Array(bin)
        const tmp = new Uint8Array(size)
        const one = new Array(tmp.buffer, 0, 1)
        for (let e = 0; e < a.count; e++) {
          for (let c = 0; c < components; c++) {
            tmp.set(bytes.subarray(start + e * stride + c * size, start + e * stride + (c + 1) * size))
            target[e * components + c] = one[0]! / divisor
          }
        }
      }
    }
    out = target
    cache.set(index, out)
    return out
  }

  const materials = (json.materials ?? []).map((m) => {
    const base = m.pbrMetallicRoughness?.baseColorFactor ?? [1, 1, 1, 1]
    const emissive = m.emissiveFactor ?? [0, 0, 0]
    // glTF factors are linear already.
    const material = new Material({
      color: new Color(base[0], base[1], base[2]),
      opacity: base[3] ?? 1,
      emissive: new Color(emissive[0], emissive[1], emissive[2]),
      blend: m.alphaMode === 'BLEND' ? 'alpha' : 'opaque',
      side: m.doubleSided ? 'double' : 'front',
    })
    material.name = m.name ?? ''
    return material
  })
  const fallback = new Material()

  const geometries = new Map<string, Geometry>()
  const geometryOf = (p: Primitive) => {
    const key = JSON.stringify([p.attributes, p.indices])
    let geometry = geometries.get(key)
    if (geometry) return geometry
    const at = p.attributes
    const color = at.COLOR_0 !== undefined ? read(at.COLOR_0) : undefined
    const colorAccessor = at.COLOR_0 !== undefined ? json.accessors[at.COLOR_0]! : undefined
    geometry = new Geometry({
      positions: read(at.POSITION!) as Float32Array,
      normals: at.NORMAL !== undefined ? (read(at.NORMAL) as Float32Array) : undefined,
      uvs: at.TEXCOORD_0 !== undefined ? toFloat(read(at.TEXCOORD_0)) : undefined,
      colors: color ? toFloat(color, colorAccessor!.type === 'VEC3' ? 3 : 4) : undefined,
      joints: at.JOINTS_0 !== undefined ? (read(at.JOINTS_0) as Uint8Array | Uint16Array) : undefined,
      weights: at.WEIGHTS_0 !== undefined ? toFloat(read(at.WEIGHTS_0)) : undefined,
      indices: p.indices !== undefined ? (read(p.indices) as Uint16Array | Uint32Array) : undefined,
    })
    if (!geometry.normals) geometry.computeNormals()
    geometries.set(key, geometry)
    return geometry
  }

  // Nodes first, skeletons after (they refer to nodes anywhere in the tree).
  const nodes: Node[] = []
  const skinned: { mesh: SkinnedMesh; skin: number }[] = []
  json.nodes.forEach((n, i) => {
    const name = n.name ?? `node${i}`
    const primitives = n.mesh !== undefined ? json.meshes![n.mesh]!.primitives.filter((p) => (p.mode ?? 4) === 4) : []
    const makeMesh = (p: Primitive) => {
      const geometry = geometryOf(p)
      const material = p.material !== undefined ? materials[p.material]! : fallback
      if (p.attributes.COLOR_0 !== undefined && !material.vertexColors) material.vertexColors = true
      if (n.skin !== undefined && p.attributes.JOINTS_0 !== undefined) {
        const mesh = new SkinnedMesh(geometry, material, new Skeleton([], new Float32Array(0)))
        skinned.push({ mesh, skin: n.skin })
        return mesh
      }
      return new Mesh(geometry, material)
    }
    let node: Node
    if (primitives.length === 1) node = makeMesh(primitives[0]!)
    else {
      node = new Node()
      for (const p of primitives) {
        const mesh = makeMesh(p)
        mesh.name = json.meshes![n.mesh!]!.name ?? name
        node.add(mesh)
      }
    }
    node.name = name
    if (n.matrix) node.setMatrix(n.matrix)
    else {
      if (n.translation) node.position.set(n.translation[0]!, n.translation[1]!, n.translation[2]!)
      if (n.rotation) node.quaternion.set(n.rotation[0]!, n.rotation[1]!, n.rotation[2]!, n.rotation[3]!)
      if (n.scale) node.scale.set(n.scale[0]!, n.scale[1]!, n.scale[2]!)
    }
    nodes.push(node)
  })
  json.nodes.forEach((n, i) => {
    for (const child of n.children ?? []) nodes[i]!.add(nodes[child]!)
  })
  for (const { mesh, skin } of skinned) {
    const s = json.skins![skin]!
    const inverse = s.inverseBindMatrices !== undefined ? (read(s.inverseBindMatrices) as Float32Array) : identities(s.joints.length)
    mesh.skeleton = new Skeleton(s.joints.map((j) => nodes[j]!), inverse)
  }

  const scene = new Node()
  scene.name = 'scene'
  const roots = json.scenes?.[json.scene ?? 0]?.nodes ?? json.nodes.map((_, i) => i).filter((i) => !nodes[i]!.parent)
  for (const r of roots) scene.add(nodes[r]!)

  const clips = (json.animations ?? []).map((a, i) => {
    let duration = 0
    const tracks: Track[] = []
    for (const channel of a.channels) {
      const path = channel.target.path as TrackPath
      if (channel.target.node === undefined || !['translation', 'rotation', 'scale'].includes(path)) continue
      const sampler = a.samplers[channel.sampler]!
      const times = toFloat(read(sampler.input))
      duration = Math.max(duration, times[times.length - 1] ?? 0)
      tracks.push({
        node: nodes[channel.target.node]!.name,
        path,
        interpolation: (sampler.interpolation ?? 'LINEAR') as Interpolation,
        times,
        values: toFloat(read(sampler.output)),
      })
    }
    return new AnimationClip(a.name ?? `animation${i}`, tracks, duration)
  })
  return { scene, clips }
}

/** As floats; widens RGB colours to RGBA when `size` is 3. */
function toFloat(a: ArrayLike<number>, size = 0): Float32Array {
  if (size === 3) {
    const n = a.length / 3
    const out = new Float32Array(n * 4)
    for (let i = 0; i < n; i++) out.set([a[i * 3]!, a[i * 3 + 1]!, a[i * 3 + 2]!, 1], i * 4)
    return out
  }
  return a instanceof Float32Array ? a : Float32Array.from(a)
}

function identities(n: number) {
  const out = new Float32Array(n * 16)
  for (let i = 0; i < n; i++) out[i * 16] = out[i * 16 + 5] = out[i * 16 + 10] = out[i * 16 + 15] = 1
  return out
}
