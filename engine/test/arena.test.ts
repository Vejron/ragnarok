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

  it('lets the arrays of an upload-once geometry go, keeping its counts and bounds', () => {
    const arena = new GeometryArena(fakeDevice())
    const geometry = new Geometry({ positions: new Float32Array([0, 0, 0, 2, 0, 0, 0, 2, 0, 2, 2, 0]), indices: new Uint16Array([0, 1, 2, 1, 3, 2]), uploadOnce: true })
    const bounds = { ...geometry.bounds }
    const slot = arena.sync(geometry)
    expect(geometry.released).toBe(true)
    expect(geometry.positions.length).toBe(0)
    expect(geometry.indices).toBeUndefined()
    expect([geometry.vertexCount, geometry.indexCount]).toEqual([4, 6])
    expect(geometry.bounds).toEqual(bounds)
    // Drawn again from the copy the arena holds.
    expect(arena.sync(geometry)).toBe(slot)
    expect(() => geometry.needsUpdate()).toThrow(/uploadOnce/)
  })

  it('does not draw a released geometry on another arena, which holds no copy of it', () => {
    const geometry = new Geometry({ positions: new Float32Array(9), uploadOnce: true })
    new GeometryArena(fakeDevice()).sync(geometry)
    const arena = new GeometryArena(fakeDevice())
    expect(arena.sync(geometry)).toBeNull()
    expect(arena.usedBytes).toBe(0)
  })

  it('collapses a range of a released geometry by writing over the copy it holds', () => {
    const writes: [number, number][] = []
    const device = fakeDevice()
    device.queue.writeBuffer = ((_buffer: GPUBuffer, offset: number, _data: ArrayBuffer, _from?: number, size?: number) => void writes.push([offset, size ?? -1])) as GPUQueue['writeBuffer']
    const arena = new GeometryArena(device)
    const geometry = new Geometry({ positions: new Float32Array(18), uploadOnce: true })
    const slot = arena.sync(geometry)!
    writes.length = 0
    geometry.collapseRange(3, 3)
    arena.sync(geometry)
    expect(writes).toEqual([[(slot.baseVertex + 3) * 64, 3 * 64]])
    // Once only.
    arena.sync(geometry)
    expect(writes.length).toBe(1)
  })

  it('throws rather than growing past the device limit', () => {
    const arena = new GeometryArena(fakeDevice((1 << 18) * 64))
    expect(() => arena.sync(new Geometry({ positions: new Float32Array(3 * ((1 << 18) + 1)) }))).toThrow(/over the GPU/)
  })
})
