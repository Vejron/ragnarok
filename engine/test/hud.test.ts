import { describe, expect, it } from 'vitest'
import { HudLayer } from '../src/hud/layer'
import { Texture } from '../src/scene/texture'

describe('HudLayer', () => {
  it('batches consecutive sprites sharing a texture, in submission order', () => {
    const layer = new HudLayer()
    const icon = new Texture({ data: new Uint8Array(4), width: 1, height: 1 })
    layer.begin(800, 600)
    layer.quad(0, 0, 10, 10)
    layer.quad(10, 0, 10, 10, 0xff0000)
    layer.quad(0, 0, 4, 4, 0xffffff, 1, icon)
    layer.quad(20, 0, 10, 10)
    expect(layer.batches.map((b) => [b.texture === icon, b.first, b.count])).toEqual([[false, 0, 2], [true, 2, 1], [false, 3, 1]])
  })

  it('stores centre, half size, uv and colour per sprite', () => {
    const layer = new HudLayer()
    layer.begin(100, 100)
    layer.quad(10, 20, 30, 40, 0x336699, 0.5, null, [0.25, 0, 0.5, 1])
    expect(Array.from(layer.instances.subarray(0, 4))).toEqual([25, 40, 15, 20])
    expect(Array.from(layer.instances.subarray(8, 12))).toEqual([0.25, 0, 0.5, 1])
    expect(layer.instances[12]).toBeCloseTo(0x33 / 255)
    expect(layer.instances[15]).toBe(0.5)
  })

  it('skips invisible sprites and grows past its first buffer', () => {
    const layer = new HudLayer()
    layer.begin(100, 100)
    layer.quad(0, 0, 1, 1, 0, 0)
    expect(layer.size).toBe(0)
    for (let i = 0; i < 1000; i++) layer.quad(i, 0, 1, 1)
    expect(layer.size).toBe(1000)
    expect(layer.batches).toHaveLength(1)
  })

  it('draws a line as a rotated quad along it', () => {
    const layer = new HudLayer()
    layer.begin(100, 100)
    layer.line(0, 0, 0, 10, 2)
    expect(Array.from(layer.instances.subarray(0, 4))).toEqual([0, 5, 5, 1])
    expect(layer.instances[4]).toBeCloseTo(Math.PI / 2)
  })
})
