import { describe, expect, it } from 'vitest'
import { Terrain, type TileGround } from '@/sim/terrain'
import { reliefPixels, terrainBuffers, terrainLevel, TERRAIN_LOD_ERRORS, TERRAIN_LOD_RANGES, type TerrainBuffers } from '@/render/terrainmesh'

/** A 65 × 65 grid 1 m apart: rolling ground with a ditch, a bank and a little noise, or flat. */
function ground(side = 65, shape: 'rolling' | 'flat' = 'rolling', x0 = 0): TileGround {
  const heights = new Float32Array(side * side)
  let seed = 7
  const noise = () => ((seed = (seed * 16807) % 2147483647) / 2147483647 - 0.5) * 0.02
  for (let row = 0; row < side; row++) {
    for (let column = 0; column < side; column++) {
      const [x, z] = [x0 + column, row]
      heights[row * side + column] =
        shape === 'flat' ? 3 : 2 * Math.sin(x / 9) + Math.cos(z / 13) - (Math.abs(x - 20) < 2 ? 1 : 0) + (z > 40 ? 0.8 : 0) + noise()
    }
  }
  return { x0, z0: 0, cell: 1, side, heights }
}

/** The triangles of `buffers` with every corner on the surface (not hung on a skirt), as vertex triples. */
function surface(buffers: TerrainBuffers, g: TileGround, cut = false) {
  const { positions, indices } = buffers
  // A skirt vertex is a surface vertex lowered: there is another at its place, above it.
  const highest = new Map<string, number>()
  const place = (v: number) => `${positions[v * 3]}|${positions[v * 3 + 2]}`
  for (let v = 0; v < positions.length / 3; v++) highest.set(place(v), Math.max(highest.get(place(v)) ?? -Infinity, positions[v * 3 + 1]!))
  const onGround = (v: number) => {
    const [x, y, z] = [(positions[v * 3]! - g.x0) / g.cell, positions[v * 3 + 1]!, (positions[v * 3 + 2]! - g.z0) / g.cell]
    if (y < highest.get(place(v))! - 0.5) return false
    // Off the samples only where a triangle was cut back to the tile.
    return cut || (Number.isInteger(x) && Number.isInteger(z) && Math.abs(y - g.heights[z * g.side + x]!) < 1e-6)
  }
  const top: [number, number, number][] = []
  const skirt: [number, number, number][] = []
  for (let i = 0; i < indices.length; i += 3) {
    const t: [number, number, number] = [indices[i]!, indices[i + 1]!, indices[i + 2]!]
    ;(t.every(onGround) ? top : skirt).push(t)
  }
  return { top, skirt }
}

/** The most any sample sits off the triangles `top` that cover it, metres. */
function worstError(buffers: TerrainBuffers, g: TileGround, top: [number, number, number][]) {
  let worst = 0
  for (const t of top) {
    const [a, b, c] = t.map((v) => corner(buffers, v))
    const [ax, az, bx, bz, cx, cz] = [a![0] - g.x0, a![2] - g.z0, b![0] - g.x0, b![2] - g.z0, c![0] - g.x0, c![2] - g.z0].map((m) => m / g.cell)
    const det = (bx! - ax!) * (cz! - az!) - (cx! - ax!) * (bz! - az!)
    for (let row = Math.ceil(Math.min(az!, bz!, cz!)); row <= Math.max(az!, bz!, cz!); row++) {
      for (let column = Math.ceil(Math.min(ax!, bx!, cx!)); column <= Math.max(ax!, bx!, cx!); column++) {
        const wb = ((column - ax!) * (cz! - az!) - (cx! - ax!) * (row - az!)) / det
        const wc = ((bx! - ax!) * (row - az!) - (column - ax!) * (bz! - az!)) / det
        const wa = 1 - wb - wc
        if (wa < -1e-9 || wb < -1e-9 || wc < -1e-9) continue
        worst = Math.max(worst, Math.abs(wa * a![1] + wb * b![1] + wc * c![1] - g.heights[row * g.side + column]!))
      }
    }
  }
  return worst
}

const corner = (b: TerrainBuffers, v: number) => [b.positions[v * 3]!, b.positions[v * 3 + 1]!, b.positions[v * 3 + 2]!] as const

/** Face normal of triangle `t`, not normalised. */
function faceNormal(b: TerrainBuffers, [i, j, k]: [number, number, number]) {
  const [a, p, q] = [corner(b, i), corner(b, j), corner(b, k)]
  const u = [p[0] - a[0], p[1] - a[1], p[2] - a[2]]
  const v = [q[0] - a[0], q[1] - a[1], q[2] - a[2]]
  return [u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!] as const
}

describe('terrain mesh simplification', () => {
  it('stays within the error of every sample, and covers the tile', () => {
    const g = ground()
    for (const maxError of [0, ...TERRAIN_LOD_ERRORS]) {
      const buffers = terrainBuffers(g, maxError)
      const { top } = surface(buffers, g)
      let area = 0
      for (const t of top) area += faceNormal(buffers, t)[1] / 2
      const worst = worstError(buffers, g, top)
      expect(area, `${maxError} m`).toBeCloseTo(64 * 64, 6)
      expect(worst, `${maxError} m`).toBeLessThanOrEqual(maxError + 1e-5)
    }
  })

  it('drops samples it can do without, more the coarser it is', () => {
    const g = ground()
    const counts = [0, ...TERRAIN_LOD_ERRORS].map((maxError) => surface(terrainBuffers(g, maxError), g).top.length)
    expect(counts[0]).toBe(64 * 64 * 2)
    for (let i = 1; i < counts.length; i++) expect(counts[i]).toBeLessThan(counts[i - 1]!)
    // Flat ground is two triangles, whatever the error.
    const flat = ground(65, 'flat')
    expect(surface(terrainBuffers(flat, 0), flat).top.length).toBe(2)
  })

  it('faces up, and hangs skirts facing out from every edge', () => {
    const g = ground()
    const buffers = terrainBuffers(g, TERRAIN_LOD_ERRORS[1])
    const { top, skirt } = surface(buffers, g)
    for (const t of top) expect(faceNormal(buffers, t)[1]).toBeGreaterThan(0)
    expect(skirt.length).toBeGreaterThan(0)
    const mid = 32
    for (const t of skirt) {
      const [nx, ny, nz] = faceNormal(buffers, t)
      expect(Math.abs(ny)).toBeLessThan(1e-6)
      // Out of the tile: away from its middle.
      const [x, , z] = corner(buffers, t[0])
      expect(nx * (x - mid) + nz * (z - mid)).toBeGreaterThan(0)
    }
    // Normals lean the way the ground falls.
    for (let v = 0; v < buffers.positions.length / 3; v++) expect(buffers.normals[v * 3 + 1]).toBeGreaterThan(0)
  })

  it('simplifies a grid of any size, cut back to the tile', () => {
    // 51 samples, as 250 m tiles at 5 m: carried on to 65 and cut back.
    const g = ground(51)
    for (const maxError of [0, ...TERRAIN_LOD_ERRORS]) {
      const buffers = terrainBuffers(g, maxError)
      const { top } = surface(buffers, g, true)
      let area = 0
      for (const t of top) {
        const n = faceNormal(buffers, t)
        expect(n[1]).toBeGreaterThan(0)
        area += n[1] / 2
      }
      expect(area, `${maxError} m`).toBeCloseTo(50 * 50, 6)
      for (let v = 0; v < buffers.positions.length / 3; v++) {
        expect(buffers.positions[v * 3]).toBeLessThanOrEqual(50)
        expect(buffers.positions[v * 3 + 2]).toBeLessThanOrEqual(50)
      }
      // Within the error at every sample, as a 2^k + 1 grid is.
      expect(worstError(buffers, g, top)).toBeLessThanOrEqual(maxError + 1e-5)
    }
    expect(surface(terrainBuffers(g, TERRAIN_LOD_ERRORS[1]), g, true).top.length).toBeLessThan(50 * 50 * 2)
  })
})

describe('terrain relief map', () => {
  it('has a texel a cell, facing up, with the tone at half scale', () => {
    const g = ground()
    const relief = reliefPixels(g)
    expect(relief.width).toBe(64)
    expect(relief.data.length).toBe(64 * 64 * 4)
    for (let k = 0; k < 64 * 64; k++) {
      expect(relief.data[k * 4 + 1]).toBeGreaterThan(128)
      // Tone 0.55..1.2, stored halved.
      expect(relief.data[k * 4 + 3]).toBeGreaterThanOrEqual(Math.floor(0.55 * 127.5))
      expect(relief.data[k * 4 + 3]).toBeLessThanOrEqual(Math.ceil(1.2 * 127.5))
    }
  })

  it('reworking only the edges once a neighbour arrives gives the same map as starting over', () => {
    const terrain = new Terrain(64, 64)
    const here = ground(65, 'rolling', -64)
    here.z0 = -64
    const next = ground(65, 'rolling', 0)
    next.z0 = -64
    terrain.set({ col: 0, row: 0, hash: 0 }, here)
    const alone = reliefPixels(here, terrain)
    terrain.set({ col: 1, row: 0, hash: 0 }, next)
    const restitched = reliefPixels(here, terrain, undefined, alone)
    const fresh = reliefPixels(here, terrain)
    expect(restitched.data).toEqual(fresh.data)
    expect(restitched.data).not.toEqual(alone.data)
  })
})

describe('terrain level of detail', () => {
  it('goes coarser with distance, and only once well past a range', () => {
    expect(terrainLevel(0)).toBe(0)
    expect(terrainLevel(TERRAIN_LOD_RANGES[0] + 1)).toBe(1)
    expect(terrainLevel(TERRAIN_LOD_RANGES[1] + 1)).toBe(2)
    expect(terrainLevel(TERRAIN_LOD_RANGES[0] + 5, 0)).toBe(0)
    expect(terrainLevel(TERRAIN_LOD_RANGES[0] + 30, 0)).toBe(1)
    expect(terrainLevel(TERRAIN_LOD_RANGES[0] - 5, 1)).toBe(1)
    expect(terrainLevel(TERRAIN_LOD_RANGES[0] - 30, 1)).toBe(0)
    expect(terrainLevel(1000, 0)).toBe(2)
  })
})
