let nextId = 1

/** A bounding sphere in the geometry's own space. */
export interface Sphere {
  x: number
  y: number
  z: number
  radius: number
}

export interface GeometryData {
  positions: Float32Array
  normals?: Float32Array
  uvs?: Float32Array
  /** Linear RGB (3 per vertex) or RGBA (4 per vertex) colours. */
  colors?: Float32Array
  /** Four free floats per vertex, read by ShaderMaterials as `custom`. */
  custom?: Float32Array
  /** Skinning: four bone indices per vertex (into the skeleton's bones) and their four weights. */
  joints?: Uint8Array | Uint16Array
  weights?: Float32Array
  /** Without indices the vertices are drawn as a triangle list in order. */
  indices?: Uint16Array | Uint32Array
  /** The bounding sphere, when the caller already knows it; worked out from `positions` otherwise. */
  bounds?: Sphere
  /** Lets the arrays go once the renderer holds a copy (`Geometry.uploadOnce`). */
  uploadOnce?: boolean
}

const NO_FLOATS = new Float32Array(0)

/**
 * Vertex data on the CPU. The renderer uploads it into its shared vertex arena the first time it is
 * drawn, and again after `needsUpdate()`. Geometry can be shared by any number of meshes, and meshes that
 * share geometry and pipeline are drawn as one instanced call.
 *
 * The arrays stay here after the upload, which doubles what a mesh costs in memory, so that they can
 * be read, changed in place and sent again (and sent to a new device when the old one is lost). A big
 * mesh nobody reads again sets `uploadOnce` to keep the GPU's copy only.
 */
export class Geometry {
  readonly id = nextId++
  positions: Float32Array
  normals: Float32Array | undefined
  uvs: Float32Array | undefined
  colors: Float32Array | undefined
  custom: Float32Array | undefined
  joints: Uint8Array | Uint16Array | undefined
  weights: Float32Array | undefined
  indices: Uint16Array | Uint32Array | undefined
  bounds: Sphere = { x: 0, y: 0, z: 0, radius: 0 }
  /** Bumped by `needsUpdate`; the renderer re-uploads when it differs from what it has. */
  version = 0
  /** @internal vertex ranges (first, count pairs) changed in place since the last upload. */
  dirtyRanges: number[] = []
  /** @internal the renderer's handle to the uploaded copy. */
  gpu: unknown = undefined
  disposed = false
  /**
   * Drops the arrays once the renderer has uploaded them: they are then empty, the geometry cannot be
   * changed but for `collapseRange`, and it is not drawn on another device (after a lost one) until
   * whoever made it makes it again. Its counts and bounds stay.
   */
  uploadOnce: boolean
  /**
   * Its vertices and indices are written on the GPU, not from its arrays (`MeshRefiner`). So on another
   * device (after a lost one, or a change of multisampling) it is given room again though its arrays were
   * let go, and whoever fills it fills it again.
   */
  gpuFilled = false
  /** @internal vertex ranges (first, count pairs) collapsed since the last upload. */
  collapsedRanges: number[] = []
  /** The vertices and indices it had when its arrays were let go; -1 while it holds them. */
  private releasedVertices = -1
  private releasedIndices = -1

  constructor(data: GeometryData) {
    this.positions = data.positions
    this.normals = data.normals
    this.uvs = data.uvs
    this.colors = data.colors
    this.custom = data.custom
    this.joints = data.joints
    this.weights = data.weights
    this.indices = data.indices
    this.uploadOnce = data.uploadOnce ?? false
    if (data.bounds) this.bounds = { ...data.bounds }
    else this.computeBounds()
  }

  get vertexCount() {
    return this.releasedVertices >= 0 ? this.releasedVertices : this.positions.length / 3
  }

  get indexCount() {
    return this.releasedIndices >= 0 ? this.releasedIndices : this.indices ? this.indices.length : this.vertexCount
  }

  /** Whether its arrays were let go after the upload (`uploadOnce`). */
  get released() {
    return this.releasedVertices >= 0
  }

  /** @internal lets the arrays go, keeping the counts: the renderer calls it once it holds a copy. */
  release() {
    if (this.released) return
    this.releasedIndices = this.indexCount
    this.releasedVertices = this.vertexCount
    this.positions = NO_FLOATS
    this.normals = this.uvs = this.colors = this.custom = this.joints = this.weights = this.indices = undefined
  }

  /**
   * Draws nothing of vertices `first` to `first + count - 1` from now on: every one is put on the same
   * point, so the triangles between them have no area. For triangles that use no vertex outside the
   * range. Works on a geometry whose arrays were let go, which `needsUpdateRange` cannot.
   */
  collapseRange(first: number, count: number) {
    if (!this.released) this.positions.fill(0, first * 3, (first + count) * 3)
    this.collapsedRanges.push(first, count)
  }

  /**
   * Call after changing vertices `first` to `first + count - 1` in place: only they are re-sent. Indices
   * and array sizes must not have changed (use `needsUpdate` for that). Bounds are not recomputed.
   */
  needsUpdateRange(first: number, count: number) {
    this.mustHold()
    this.dirtyRanges.push(first, count)
  }

  /** Call after changing any of the arrays (in place or by replacing them). */
  needsUpdate() {
    this.mustHold()
    this.dirtyRanges.length = 0
    this.collapsedRanges.length = 0
    this.version++
    this.computeBounds()
  }

  /** A geometry that let its arrays go has nothing to send again: changing it is a mistake, said at once. */
  private mustHold() {
    if (this.released) throw new Error('This geometry let its arrays go after the upload (uploadOnce), so it cannot be changed')
  }

  computeBounds() {
    if (this.released) return
    const p = this.positions
    let minX = Infinity, minY = Infinity, minZ = Infinity
    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity
    for (let i = 0; i < p.length; i += 3) {
      const x = p[i]!, y = p[i + 1]!, z = p[i + 2]!
      if (x < minX) minX = x
      if (y < minY) minY = y
      if (z < minZ) minZ = z
      if (x > maxX) maxX = x
      if (y > maxY) maxY = y
      if (z > maxZ) maxZ = z
    }
    if (!p.length) {
      this.bounds = { x: 0, y: 0, z: 0, radius: 0 }
      return
    }
    const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2, cz = (minZ + maxZ) / 2
    let r2 = 0
    for (let i = 0; i < p.length; i += 3) {
      const dx = p[i]! - cx, dy = p[i + 1]! - cy, dz = p[i + 2]! - cz
      r2 = Math.max(r2, dx * dx + dy * dy + dz * dz)
    }
    this.bounds = { x: cx, y: cy, z: cz, radius: Math.sqrt(r2) }
  }

  /** Flat-ish smooth normals from the triangles, area weighted. */
  computeNormals() {
    this.mustHold()
    const p = this.positions
    const n = new Float32Array(p.length)
    const tri = this.indexCount / 3
    const idx = this.indices
    for (let t = 0; t < tri; t++) {
      const a = idx ? idx[t * 3]! : t * 3
      const b = idx ? idx[t * 3 + 1]! : t * 3 + 1
      const c = idx ? idx[t * 3 + 2]! : t * 3 + 2
      const ux = p[b * 3]! - p[a * 3]!, uy = p[b * 3 + 1]! - p[a * 3 + 1]!, uz = p[b * 3 + 2]! - p[a * 3 + 2]!
      const vx = p[c * 3]! - p[a * 3]!, vy = p[c * 3 + 1]! - p[a * 3 + 1]!, vz = p[c * 3 + 2]! - p[a * 3 + 2]!
      const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx
      for (const v of [a, b, c]) {
        n[v * 3] = n[v * 3]! + nx
        n[v * 3 + 1] = n[v * 3 + 1]! + ny
        n[v * 3 + 2] = n[v * 3 + 2]! + nz
      }
    }
    for (let i = 0; i < n.length; i += 3) {
      const len = Math.hypot(n[i]!, n[i + 1]!, n[i + 2]!) || 1
      n[i] = n[i]! / len
      n[i + 1] = n[i + 1]! / len
      n[i + 2] = n[i + 2]! / len
    }
    this.normals = n
    this.version++
  }

  /** An independent copy: its own arrays, not yet uploaded. */
  clone(): Geometry {
    return new Geometry({
      positions: this.positions.slice(),
      normals: this.normals?.slice(),
      uvs: this.uvs?.slice(),
      colors: this.colors?.slice(),
      custom: this.custom?.slice(),
      joints: this.joints?.slice(),
      weights: this.weights?.slice(),
      indices: this.indices?.slice(),
    })
  }

  /** The same triangles with each corner its own vertex, so `computeNormals` gives flat faces. */
  toNonIndexed(): Geometry {
    const idx = this.indices
    if (!idx) return this.clone()
    const take = (a: Float32Array | undefined, size: number) => {
      if (!a) return undefined
      const out = new Float32Array(idx.length * size)
      for (let i = 0; i < idx.length; i++) for (let c = 0; c < size; c++) out[i * size + c] = a[idx[i]! * size + c]!
      return out
    }
    const colorSize = this.colors ? this.colors.length / this.vertexCount : 0
    return new Geometry({
      positions: take(this.positions, 3)!,
      normals: take(this.normals, 3),
      uvs: take(this.uvs, 2),
      colors: take(this.colors, colorSize),
      custom: take(this.custom, 4),
    })
  }

  /** Moves every vertex by (x, y, z). Call `needsUpdate()` after, if it was already drawn. */
  translate(x: number, y: number, z: number): this {
    const p = this.positions
    for (let i = 0; i < p.length; i += 3) {
      p[i] = p[i]! + x
      p[i + 1] = p[i + 1]! + y
      p[i + 2] = p[i + 2]! + z
    }
    this.computeBounds()
    return this
  }

  /** Scales every vertex about the origin; normals follow for a uniform scale or a non-uniform one alike. */
  scale(x: number, y: number, z: number): this {
    const p = this.positions
    for (let i = 0; i < p.length; i += 3) {
      p[i] = p[i]! * x
      p[i + 1] = p[i + 1]! * y
      p[i + 2] = p[i + 2]! * z
    }
    const n = this.normals
    if (n) {
      for (let i = 0; i < n.length; i += 3) {
        let nx = n[i]! / x, ny = n[i + 1]! / y, nz = n[i + 2]! / z
        const l = Math.hypot(nx, ny, nz) || 1
        nx /= l; ny /= l; nz /= l
        n[i] = nx; n[i + 1] = ny; n[i + 2] = nz
      }
    }
    this.computeBounds()
    return this
  }

  /** Turns every vertex (and normal) about the X axis. */
  rotateX(angle: number): this {
    return this.rotateAbout(0, angle)
  }

  rotateY(angle: number): this {
    return this.rotateAbout(1, angle)
  }

  rotateZ(angle: number): this {
    return this.rotateAbout(2, angle)
  }

  private rotateAbout(axis: 0 | 1 | 2, angle: number): this {
    const c = Math.cos(angle), s = Math.sin(angle)
    // The two axes the rotation mixes, in right-handed order.
    const a = (axis + 1) % 3, b = (axis + 2) % 3
    for (const array of [this.positions, this.normals]) {
      if (!array) continue
      for (let i = 0; i < array.length; i += 3) {
        const u = array[i + a]!, v = array[i + b]!
        array[i + a] = u * c - v * s
        array[i + b] = u * s + v * c
      }
    }
    this.computeBounds()
    return this
  }

  /**
   * One geometry from several, in order. Attributes that only some have are zero-filled for the rest
   * (colours default to white). Indices are offset; a mix of indexed and plain geometry is made indexed.
   */
  static merge(list: readonly Geometry[]): Geometry {
    const vertices = list.reduce((n, g) => n + g.vertexCount, 0)
    const withNormals = list.some((g) => g.normals)
    const withUvs = list.some((g) => g.uvs)
    const colorSize = list.some((g) => g.colors && g.colors.length === g.vertexCount * 4) ? 4 : list.some((g) => g.colors) ? 3 : 0
    const positions = new Float32Array(vertices * 3)
    const normals = withNormals ? new Float32Array(vertices * 3) : undefined
    const uvs = withUvs ? new Float32Array(vertices * 2) : undefined
    const custom = list.some((g) => g.custom) ? new Float32Array(vertices * 4) : undefined
    const colors = colorSize ? new Float32Array(vertices * colorSize).fill(1) : undefined
    const indices = vertices > 65535 ? new Uint32Array(list.reduce((n, g) => n + g.indexCount, 0)) : new Uint16Array(list.reduce((n, g) => n + g.indexCount, 0))
    let v = 0
    let i = 0
    for (const g of list) {
      positions.set(g.positions, v * 3)
      if (normals && g.normals) normals.set(g.normals, v * 3)
      if (uvs && g.uvs) uvs.set(g.uvs, v * 2)
      if (custom && g.custom) custom.set(g.custom, v * 4)
      if (colors && g.colors) {
        const size = g.colors.length / g.vertexCount
        if (size === colorSize) colors.set(g.colors, v * colorSize)
        else for (let k = 0; k < g.vertexCount; k++) for (let c = 0; c < 3; c++) colors[(v + k) * colorSize + c] = g.colors[k * size + c]!
      }
      if (g.indices) for (let k = 0; k < g.indices.length; k++) indices[i++] = g.indices[k]! + v
      else for (let k = 0; k < g.vertexCount; k++) indices[i++] = v + k
      v += g.vertexCount
    }
    return new Geometry({ positions, normals, uvs, colors, custom, indices })
  }

  /** Frees the GPU copy. Meshes still using it stop drawing. */
  dispose() {
    this.disposed = true
    this.version++
  }
}
