import { afterEach, describe, expect, it, vi } from 'vitest'
import { Geometry, Mesh, MeshRefiner, type EffectContext, type Node } from '@ragnarok/engine'
import { REFINE_HEIGHT, REFINE_SIZE, WaterRefine } from '@/render/waterrefine'
import { waterMaterial, waterSurfaces } from '@/render/water'

/** A flat square of lake `size` metres a side from `(x0, z0)`, a vertex every `spacing` metres. */
function lake(x0: number, z0: number, size: number, spacing: number) {
  const n = Math.round(size / spacing) + 1
  const positions = new Float32Array(n * n * 3)
  for (let j = 0; j < n; j++) for (let i = 0; i < n; i++) positions.set([x0 + i * spacing, 1, z0 + j * spacing], (j * n + i) * 3)
  const indices: number[] = []
  for (let j = 0; j + 1 < n; j++) for (let i = 0; i + 1 < n; i++) indices.push(j * n + i, (j + 1) * n + i, j * n + i + 1)
  const mesh = new Mesh(new Geometry({ positions, custom: new Float32Array(n * n * 4), indices: Uint32Array.from(indices) }), waterMaterial())
  waterSurfaces.add(mesh)
  return mesh
}

describe('the refined water', () => {
  afterEach(() => waterSurfaces.clear())

  it('waits for water, then sizes itself to the water\'s vertex spacing', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    refine.update({ x: 0, y: 0, z: 0 }, 1)
    expect(added).toHaveLength(0)
    lake(-100, -100, 200, 2)
    refine.update({ x: 0, y: 0, z: 0 }, 1)
    expect(added).toHaveLength(2)
    const cells = REFINE_SIZE / 2 + 2
    expect((added[0] as MeshRefiner).capacity).toBe(2 * cells * cells)
  })

  it('leaves the tiles their water until its mesh has a place to be written, then keeps the square on the cells\' lines', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    lake(-99, -101, 200, 1)
    refine.update({ x: 3.7, y: 0, z: -2.2 }, 1)
    expect(refine.area).toBeNull()
    const refiner = added[0] as MeshRefiner
    refiner.geometry.gpu = {}
    refine.update({ x: 3.7, y: 0, z: -2.2 }, 1)
    const [x0, z0, size] = refine.area!
    expect(size).toBe(REFINE_SIZE)
    // On the lattice of the lake's vertices, in steps of four of them, with the camera inside.
    expect((x0 + 99) % 4).toBe(0)
    expect((z0 + 101) % 4).toBe(0)
    expect(x0).toBeLessThanOrEqual(3.7 - REFINE_SIZE / 2)
    expect(x0 + 4).toBeGreaterThan(3.7 - REFINE_SIZE / 2)
  })

  it('stands down high over the water, or with none about, leaving the tiles their water', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    lake(-100, -100, 200, 1)
    refine.update({ x: 0, y: 5, z: 0 }, 1)
    const refiner = added[0] as MeshRefiner
    const mesh = added[1] as Mesh
    refiner.geometry.gpu = {}
    refine.update({ x: 0, y: 5, z: 0 }, 1)
    expect(refine.area).not.toBeNull()
    expect(mesh.visible).toBe(true)
    refine.update({ x: 0, y: 1 + REFINE_HEIGHT + 1, z: 0 }, 1)
    expect(refine.area).toBeNull()
    expect(mesh.visible).toBe(false)
    refine.update({ x: 0, y: 5, z: 0 }, null)
    expect(refine.area).toBeNull()
    refine.update({ x: 0, y: 5, z: 0 }, 1)
    expect(mesh.visible).toBe(true)
  })

  it('refines a cell past its square each way, which the water’s shader trims back to it', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    lake(-100, -100, 200, 1)
    refine.update({ x: 0, y: 0, z: 0 }, 1)
    const refiner = added[0] as MeshRefiner
    refiner.geometry.gpu = {}
    const set: number[][] = []
    const setArea = refiner.setArea.bind(refiner)
    refiner.setArea = (x0, z0, size) => { set.push([x0, z0, size]); setArea(x0, z0, size) }
    refine.update({ x: 0, y: 0, z: 0 }, 1)
    const [x0, z0, size] = refine.area!
    expect(set[set.length - 1]).toEqual([x0 - 1, z0 - 1, size + 2])
  })

  it('takes only the surfaces that reach into its square', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    const near = lake(-50, -50, 100, 1)
    lake(500, 500, 100, 1)
    refine.update({ x: 0, y: 0, z: 0 }, 1)
    expect((added[0] as MeshRefiner).sources).toEqual([near.geometry])
  })
})

describe('the refiner', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('copies its square in again when the arena grows, so the square never stands empty', () => {
    vi.stubGlobal('GPUBufferUsage', { STORAGE: 1, COPY_DST: 2, COPY_SRC: 4, UNIFORM: 8 })
    vi.stubGlobal('GPUShaderStage', { COMPUTE: 4 })
    const stub = () => ({})
    const device = { createBuffer: (d: { size: number }) => ({ size: d.size, destroy() {} }), createBindGroupLayout: stub, createShaderModule: stub, createComputePipeline: stub, createPipelineLayout: stub, createBindGroup: stub, queue: { writeBuffer() {} } }
    // Which buffers each frame's copies went into.
    let into: unknown[] = []
    const encoder = { clearBuffer() {}, beginComputePass: () => ({ setPipeline() {}, setBindGroup() {}, dispatchWorkgroups() {}, end() {} }), copyBufferToBuffer: (_from: unknown, _at: number, to: unknown) => into.push(to) }
    const frame = (vertices: object, indices: object) => {
      into = []
      refiner.prepare({ device, vertices, indices } as unknown as EffectContext, encoder as unknown as GPUCommandEncoder)
      return into
    }
    const refiner = new MeshRefiner(8)
    refiner.setSources([lake(0, 0, 4, 1).geometry])
    refiner.setArea(0, 0, 4)
    refiner.geometry.gpu = { baseVertex: 0, firstIndex: 0 } as never
    const [vertices, indices] = [{}, {}]
    expect(frame(vertices, indices)).toEqual([vertices, indices])
    expect(frame(vertices, indices)).toEqual([])
    // The arena grew: its buffers are new, and whatever went into the old ones this frame is lost.
    const [moreVertices, moreIndices] = [{}, {}]
    expect(frame(moreVertices, moreIndices)).toEqual([moreVertices, moreIndices])
    expect(frame(moreVertices, moreIndices)).toEqual([])
  })
})
