import { describe, expect, it, vi } from 'vitest'
import { GeometryArena } from '../src/gpu/arena'
import { Geometry } from '../src/scene/geometry'

vi.stubGlobal('GPUBufferUsage', { VERTEX: 1, INDEX: 2, COPY_DST: 4, COPY_SRC: 8 })

/** Just enough of a device for the arena: buffers that remember their size, and a queue that does nothing. */
function fakeDevice(maxBufferSize = 1 << 30) {
  return {
    limits: { maxBufferSize },
    createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
    createCommandEncoder: () => ({ copyBufferToBuffer() {}, finish: () => ({}) }),
    queue: { writeBuffer() {}, submit() {} },
  } as unknown as GPUDevice
}

function triangle() {
  return new Geometry({ positions: new Float32Array(9) })
}

describe('GeometryArena', () => {
  it('frees a disposed geometry on collect, though nothing syncs it again', () => {
    const arena = new GeometryArena(fakeDevice())
    const geometry = triangle()
    arena.sync(geometry)
    const used = arena.usedBytes
    expect(used).toBeGreaterThan(0)
    geometry.dispose()
    arena.collect()
    expect(arena.usedBytes).toBe(0)
    expect(geometry.gpu).toBeUndefined()
  })

  it('keeps live geometries on collect', () => {
    const arena = new GeometryArena(fakeDevice())
    arena.sync(triangle())
    const used = arena.usedBytes
    arena.collect()
    expect(arena.usedBytes).toBe(used)
  })

  it('uploads again a geometry whose slot belongs to another arena', () => {
    const geometry = triangle()
    new GeometryArena(fakeDevice()).sync(geometry)
    const arena = new GeometryArena(fakeDevice())
    arena.sync(geometry)
    expect(arena.usedBytes).toBeGreaterThan(0)
  })

  it('throws rather than growing past the device limit', () => {
    const arena = new GeometryArena(fakeDevice((1 << 18) * 64))
    expect(() => arena.sync(new Geometry({ positions: new Float32Array(3 * ((1 << 18) + 1)) }))).toThrow(/over the GPU/)
  })
})
