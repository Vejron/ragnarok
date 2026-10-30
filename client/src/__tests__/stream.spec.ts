import { describe, expect, it } from 'vitest'
import { LOAD_RADIUS, TileStreamer, UNLOAD_RADIUS } from '@/stream'
import type { TileData, TileRef, WorldData } from '@/world'

const empty: TileData = { buildings: [], roads: [], cover: [], trees: [], collision: { vertices: new Float32Array(), triangles: new Uint32Array() }, ground: new Float32Array(), water: new Float32Array(), streams: [], bridges: [], rails: [] }

/** A 10 × 10 grid of 100 m tiles, centred on the origin. */
function grid(): WorldData {
  const tiles: TileRef[] = []
  for (let col = 0; col < 10; col++) for (let row = 0; row < 10; row++) tiles.push({ col, row, hash: 0 })
  return { frame: [0, 0, 0], halfExtent: 500, tileSize: 100, spawns: [], tiles, groundCell: 0, collision: empty.collision }
}

function setup() {
  const attached = new Set<string>()
  const fetched: string[] = []
  const streamer = new TileStreamer(
    grid(),
    {
      attach(ref) {
        const name = `${ref.col}_${ref.row}`
        attached.add(name)
        return () => attached.delete(name)
      },
    },
    async (ref) => {
      fetched.push(`${ref.col}_${ref.row}`)
      return empty
    },
  )
  return { streamer, attached, fetched }
}

describe('TileStreamer', () => {
  it('attaches the same tiles afresh after a reload', async () => {
    const { streamer, attached } = setup()
    await streamer.prime(-450, -450)
    const before = new Set(attached)
    streamer.reload()
    expect(attached.size).toBe(0)
    expect(streamer.loaded).toBe(0)
    await streamer.prime(-450, -450)
    expect(attached).toEqual(before)
  })

  it('loads the tiles around the player and no others', async () => {
    const { streamer, attached } = setup()
    await streamer.prime(-450, -450)
    // A tile is wanted when its nearest edge is within the load radius of the player.
    const wanted = new Set<string>()
    for (let col = 0; col < 10; col++) {
      for (let row = 0; row < 10; row++) {
        const dx = Math.max(-500 + col * 100 - -450, 0, -450 - (-500 + col * 100 + 100))
        const dz = Math.max(-500 + row * 100 - -450, 0, -450 - (-500 + row * 100 + 100))
        if (Math.hypot(dx, dz) <= LOAD_RADIUS) wanted.add(`${col}_${row}`)
      }
    }
    expect(wanted.size).toBeGreaterThan(10)
    expect(wanted.size).toBeLessThan(100)
    expect(attached).toEqual(wanted)
    expect(streamer.loaded).toBe(wanted.size)
  })

  it('streams tiles in ahead of the player and out behind', async () => {
    const { streamer, attached } = setup()
    await streamer.prime(-450, 0)
    expect(attached.has('0_5')).toBe(true)
    expect(attached.has('9_5')).toBe(false)
    // Walk east across the world.
    for (let x = -450; x <= 450; x += 25) {
      streamer.update(x, 0)
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    await streamer.prime(450, 0)
    expect(attached.has('9_5')).toBe(true)
    expect(attached.has('0_5'), 'the far side is dropped again').toBe(false)
  })

  it('holds on to a tile until the player is beyond the unload radius', async () => {
    const { streamer, attached } = setup()
    await streamer.prime(0, 0)
    expect(attached.has('5_5')).toBe(true)
    // Tile 5_5 spans x 0..100. Standing at x = -(LOAD + 40) it is past the load radius but inside
    // the unload radius, so it stays; at x = -(UNLOAD + 40) it goes.
    streamer.update(-(LOAD_RADIUS + 40), 50)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(attached.has('5_5')).toBe(true)
    streamer.update(-(UNLOAD_RADIUS + 40), 50)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(attached.has('5_5')).toBe(false)
  })

  it('does not attach a tile that arrives after the player left', async () => {
    const attached: string[] = []
    let release: (() => void) | undefined
    const gate = new Promise<void>((resolve) => (release = resolve))
    const streamer = new TileStreamer(
      grid(),
      { attach: (ref) => (attached.push(`${ref.col}_${ref.row}`), () => {}) },
      async () => {
        await gate
        return empty
      },
    )
    streamer.update(-450, -450)
    streamer.update(450, 450)
    release!()
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(attached).not.toContain('0_0')
  })

  it('retries a tile that failed', async () => {
    let now = 0
    let calls = 0
    const attached: string[] = []
    const world = { ...grid(), tiles: [{ col: 5, row: 5, hash: 0 }] }
    const streamer = new TileStreamer(
      world,
      { attach: (ref) => (attached.push(`${ref.col}_${ref.row}`), () => {}) },
      async () => {
        if (++calls === 1) throw new Error('network')
        return empty
      },
      () => now,
    )
    streamer.update(0, 0)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(attached).toEqual([])
    now = 10_000
    streamer.update(0, 0)
    await new Promise((resolve) => setTimeout(resolve, 0))
    // Fetched; attached on the next frame.
    streamer.update(0, 0)
    expect(attached).toEqual(['5_5'])
  })

  it('attaches arrived tiles a few a frame, within the budget', async () => {
    let now = 0
    const attached: string[] = []
    const streamer = new TileStreamer(
      grid(),
      {
        attach: (ref) => {
          // Every tile takes 3 ms to attach, so the 4 ms budget fits two.
          now += 3
          attached.push(`${ref.col}_${ref.row}`)
          return () => {}
        },
      },
      async () => empty,
      () => now,
    )
    streamer.update(0, 0)
    for (let i = 0; i < 4; i++) await new Promise((resolve) => setTimeout(resolve, 0))
    expect(attached).toEqual([])
    streamer.update(0, 0)
    expect(attached.length).toBe(2)
    // Nearest first: the four tiles meeting at the player.
    expect(attached.every((name) => ['4_4', '4_5', '5_4', '5_5'].includes(name))).toBe(true)
  })

  it('prepares a tile before attaching it, and discards it if the player left meanwhile', async () => {
    const calls: string[] = []
    const world = { ...grid(), tiles: [{ col: 0, row: 0, hash: 0 }] }
    const streamer = new TileStreamer(
      world,
      {
        prepare: async (ref) => void calls.push(`prepare ${ref.col}_${ref.row}`),
        attach: (ref) => (calls.push(`attach ${ref.col}_${ref.row}`), () => {}),
        discard: (ref) => void calls.push(`discard ${ref.col}_${ref.row}`),
      },
      async () => empty,
    )
    streamer.update(-450, -450)
    for (let i = 0; i < 4; i++) await new Promise((resolve) => setTimeout(resolve, 0))
    expect(calls).toEqual(['prepare 0_0'])
    streamer.update(450, 450)
    expect(calls).toEqual(['prepare 0_0', 'discard 0_0'])
    expect(streamer.loaded).toBe(0)
  })
})
