import { describe, expect, it, vi } from 'vitest'
import { GeometryArena, packVertices } from '../src/gpu/arena'
import { Geometry } from '../src/scene/geometry'

vi.stubGlobal('GPUBufferUsage', { VERTEX: 1, INDEX: 2, COPY_DST: 4, COPY_SRC: 8, MAP_WRITE: 16 })
vi.stubGlobal('GPUMapMode', { WRITE: 2 })

/** What a fake device was asked to do, in order. */
type Event =
  | { kind: 'write'; offset: number; size: number }
  | { kind: 'copy'; from: FakeBuffer; to: FakeBuffer; offset: number; size: number }
  | { kind: 'submit'; copies: number }
  | { kind: 'create'; buffer: FakeBuffer }

interface FakeBuffer { size: number; label?: string; mapped: boolean; destroyed: boolean }

/**
 * Just enough of a device for the arena: buffers that remember their size and can be mapped, and a
 * queue that records what it is asked to do.
 */
function fakeDevice(maxBufferSize = 1 << 30) {
  const events: Event[] = []
  const device = {
    events,
    limits: { maxBufferSize },
    createBuffer: ({ size, label, mappedAtCreation }: GPUBufferDescriptor) => {
      let range: ArrayBuffer | null = mappedAtCreation ? new ArrayBuffer(size) : null
      const buffer = {
        size, label, mapped: !!mappedAtCreation, destroyed: false,
        destroy() { buffer.destroyed = true },
        getMappedRange() { return range! },
        unmap() { range = null; buffer.mapped = false },
        mapAsync() { return Promise.resolve().then(() => { range = new ArrayBuffer(size); buffer.mapped = true }) },
      }
      events.push({ kind: 'create', buffer })
      return buffer
    },
    createCommandEncoder: () => {
      let copies = 0
      return {
        copyBufferToBuffer(from: FakeBuffer, _at: number, to: FakeBuffer, offset: number, size: number) {
          copies++
          events.push({ kind: 'copy', from, to, offset, size })
        },
        finish: () => ({ copies }),
      }
    },
    queue: {
      writeBuffer(_buffer: GPUBuffer, offset: number, data: ArrayBufferView | ArrayBuffer, _from?: number, size?: number) {
        events.push({ kind: 'write', offset, size: size ?? data.byteLength })
      },
      submit([commands]: { copies: number }[]) {
        events.push({ kind: 'submit', copies: commands!.copies })
      },
    },
  }
  return device as unknown as GPUDevice & { events: Event[] }
}

/** A geometry of `vertices` vertices, 64 bytes each in the arena, and no indices. */
function mesh(vertices: number) {
  return new Geometry({ positions: new Float32Array(vertices * 3) })
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

  it('gives a released geometry that is filled on the GPU room on another arena', () => {
    const geometry = new Geometry({ positions: new Float32Array(9), uploadOnce: true })
    geometry.gpuFilled = true
    const first = new GeometryArena(fakeDevice())
    const old = first.sync(geometry)
    expect(geometry.released).toBe(true)
    const arena = new GeometryArena(fakeDevice())
    const slot = arena.sync(geometry)
    expect(slot).not.toBeNull()
    expect(slot).not.toBe(old)
    expect(slot!.owner).toBe(arena)
    expect(slot!.vertexCount).toBe(3)
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

  it('sends a small upload with writeBuffer and nothing to flush', () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    arena.sync(mesh(100))
    arena.flush()
    expect(device.events.filter((e) => e.kind === 'write').length).toBeGreaterThan(0)
    expect(device.events.some((e) => e.kind === 'copy' || e.kind === 'submit')).toBe(false)
  })

  it('stages a big upload in a mapped buffer and copies it in on flush', () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    const slot = arena.sync(mesh(4096))!
    expect(device.events.some((e) => e.kind === 'submit')).toBe(false)
    arena.flush()
    const copies = device.events.filter((e) => e.kind === 'copy')
    expect(copies.map((e) => [e.offset, e.size])).toEqual([[slot.baseVertex * 64, 4096 * 64], [slot.firstIndex * 4, 4096 * 4]])
    expect(copies.every((e) => !e.from.mapped)).toBe(true)
    expect(device.events.at(-1)).toEqual({ kind: 'submit', copies: 2 })
  })

  it('sends small uploads through the ring once a copy waits, so they cannot land before it', () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    arena.sync(mesh(4096))
    const writes = device.events.filter((e) => e.kind === 'write').length
    arena.sync(mesh(10))
    expect(device.events.filter((e) => e.kind === 'write').length).toBe(writes)
    arena.flush()
    expect(device.events.at(-1)).toEqual({ kind: 'submit', copies: 4 })
  })

  it('maps its staging buffer again and uses it for the next frame', async () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    arena.sync(mesh(4096))
    arena.flush()
    const staging = () => device.events.filter((e) => e.kind === 'create' && e.buffer.label === 'arena staging')
    expect(staging().length).toBe(1)
    await Promise.resolve()
    await Promise.resolve()
    arena.sync(mesh(4096))
    arena.flush()
    expect(staging().length).toBe(1)
  })

  it('gives an upload too big for the ring a buffer of its own, destroyed once its copy is submitted', () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    arena.sync(mesh(100_000))
    arena.flush()
    const own = device.events.find((e) => e.kind === 'create' && e.buffer.size === 100_000 * 64)
    expect(own?.kind === 'create' && own.buffer.destroyed).toBe(true)
  })

  it('submits the staged copies before growing, so the growth copies them over', () => {
    const device = fakeDevice()
    const arena = new GeometryArena(device)
    arena.sync(mesh(4096))
    const old = arena.vertexBuffer as unknown as FakeBuffer
    arena.sync(mesh(1 << 18))
    const growth = device.events.findIndex((e) => e.kind === 'copy' && e.from === old && e.to !== old)
    const first = device.events.findIndex((e) => e.kind === 'submit')
    expect(first).toBeGreaterThan(-1)
    expect(first).toBeLessThan(growth)
    expect(device.events[first]).toEqual({ kind: 'submit', copies: 2 })
  })

  it('throws rather than growing past the device limit', () => {
    const arena = new GeometryArena(fakeDevice((1 << 18) * 64))
    expect(() => arena.sync(new Geometry({ positions: new Float32Array(3 * ((1 << 18) + 1)) }))).toThrow(/over the GPU/)
  })
})

describe('packed vertices', () => {
  /** The bytes of the first upload an arena makes of `geometry`, small enough to be written straight. */
  const uploaded = (geometry: Geometry): Uint8Array => {
    const device = fakeDevice()
    const bytes: Uint8Array[] = []
    device.queue.writeBuffer = ((_buffer: GPUBuffer, _offset: number, data: ArrayBuffer, from = 0, size?: number) => {
      bytes.push(new Uint8Array(data, from, size ?? data.byteLength).slice())
    }) as GPUQueue['writeBuffer']
    new GeometryArena(device).sync(geometry)
    return bytes[0]!
  }
  const arrays = () => ({
    positions: Float32Array.from([0, 1, 2, 3, 4, 5, 6, 7, 8.5]),
    normals: Float32Array.from([0, 1, 0, 1, 0, 0, 0, 0, 1]),
    uvs: Float32Array.from([0, 0, 1, 0, 0.5, 1]),
    colors: Float32Array.from([1, 0.5, 0.25, 0.1, 0.2, 0.3, 2, -1, 0]),
    custom: Float32Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
  })

  it('uploads a packed geometry as the same bytes as its arrays', () => {
    const plain = new Geometry(arrays())
    const packed = new Geometry({ packed: packVertices(arrays()) })
    expect(packed.vertexCount).toBe(plain.vertexCount)
    expect(packed.bounds).toEqual(plain.bounds)
    expect(Array.from(uploaded(packed))).toEqual(Array.from(uploaded(plain)))
  })

  it('collapses a range of a packed geometry before it is uploaded as the arena collapses one after', () => {
    const packed = new Geometry({ packed: packVertices(arrays()) })
    packed.collapseRange(1, 1)
    const bytes = uploaded(packed)
    expect(Array.from(bytes.slice(64, 128)).every((b) => b === 0)).toBe(true)
    expect(Array.from(bytes.slice(0, 64)).some((b) => b !== 0)).toBe(true)
  })
})
