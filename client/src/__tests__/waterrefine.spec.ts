import { afterEach, describe, expect, it } from 'vitest'
import { Geometry, Mesh, type MeshRefiner, type Node } from '@ragnarok/engine'
import { REFINE_SIZE, WaterRefine } from '@/render/waterrefine'
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
    refine.update({ x: 0, y: 0, z: 0 })
    expect(added).toHaveLength(0)
    lake(-100, -100, 200, 2)
    refine.update({ x: 0, y: 0, z: 0 })
    expect(added).toHaveLength(2)
    const cells = REFINE_SIZE / 2 + 2
    expect((added[0] as MeshRefiner).capacity).toBe(2 * cells * cells)
  })

  it('leaves the tiles their water until its mesh has a place to be written, then keeps the square on the cells\' lines', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    lake(-99, -101, 200, 1)
    refine.update({ x: 3.7, y: 0, z: -2.2 })
    expect(refine.area).toBeNull()
    const refiner = added[0] as MeshRefiner
    refiner.geometry.gpu = {}
    refine.update({ x: 3.7, y: 0, z: -2.2 })
    const [x0, z0, size] = refine.area!
    expect(size).toBe(REFINE_SIZE)
    // On the lattice of the lake's vertices, in steps of four of them, with the camera inside.
    expect((x0 + 99) % 4).toBe(0)
    expect((z0 + 101) % 4).toBe(0)
    expect(x0).toBeLessThanOrEqual(3.7 - REFINE_SIZE / 2)
    expect(x0 + 4).toBeGreaterThan(3.7 - REFINE_SIZE / 2)
  })

  it('takes only the surfaces that reach into its square', () => {
    const added: Node[] = []
    const refine = new WaterRefine((...nodes) => added.push(...nodes))
    const near = lake(-50, -50, 100, 1)
    lake(500, 500, 100, 1)
    refine.update({ x: 0, y: 0, z: 0 })
    expect((added[0] as MeshRefiner).sources).toEqual([near.geometry])
  })
})
