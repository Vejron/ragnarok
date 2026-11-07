/*
 * Broken pieces of wall: low, flat-faced lumps rather than cubes. Each is an icosahedron with its
 * corners pulled in and out, squashed into a slab (a wall breaks into plates, not balls), in one
 * colour a little varied by face so the facets read under the cel ramp. A few shapes a building
 * colour are made and kept; a piece is one of them, scaled and turned.
 */

const T = (1 + Math.sqrt(5)) / 2
const ICO: readonly (readonly [number, number, number])[] = [
  [-1, T, 0], [1, T, 0], [-1, -T, 0], [1, -T, 0],
  [0, -1, T], [0, 1, T], [0, -1, -T], [0, 1, -T],
  [T, 0, -1], [T, 0, 1], [-T, 0, -1], [-T, 0, 1],
]
const FACES: readonly (readonly [number, number, number])[] = [
  [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
  [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
  [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
  [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
]

function random(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export interface ShardSoup {
  positions: Float32Array
  colors: Float32Array
}

/**
 * A shard about 1 m across (half-extents near `[0.5, 0.5 × flat, 0.5]`, before turning), centred on
 * the origin, in linear `[r, g, b]`, the same for the same `seed`.
 */
export function shardSoup(seed: number, rgb: readonly [number, number, number], flat = 0.35): ShardSoup {
  const next = random(seed)
  const corners = ICO.map(([x, y, z]) => {
    const l = Math.hypot(x, y, z)
    const r = (0.32 + next() * 0.25) / l
    return [x * r, y * r * flat * 2, z * r * (0.7 + next() * 0.5)] as const
  })
  const positions = new Float32Array(FACES.length * 9)
  const colors = new Float32Array(FACES.length * 9)
  FACES.forEach(([a, b, c], f) => {
    const tone = 0.82 + next() * 0.22
    ;[a, c, b].forEach((v, k) => {
      const p = corners[v]!
      positions.set(p, f * 9 + k * 3)
      colors.set([rgb[0] * tone, rgb[1] * tone, rgb[2] * tone], f * 9 + k * 3)
    })
  })
  return { positions, colors }
}

/** Ground cells rubble is heaped on, metres a side. */
const HEAP_CELL = 1.5
/** Rubble a fallen cell leaves on the ground, cubic metres (a wall is far thinner than its cell). */
const HEAP_PER_CELL = 0.12

/**
 * The rubble left by the cells at `centres` (metres; `roofs[i]` when that one was roof) heaped on the
 * ground (`groundAt`, metres, under a mound's middle): a low mound for each `HEAP_CELL` square they stood over, sized by how many stood
 * there, with a few broken plates on it. In world coordinates; the same for the same cells.
 */
export function rubbleSoup(centres: readonly (readonly [number, number, number])[], roofs: readonly boolean[], groundAt: (x: number, z: number) => number, wall: readonly [number, number, number], roof: readonly [number, number, number]): ShardSoup {
  const bins = new Map<string, { x: number; z: number; n: number; roofs: number }>()
  centres.forEach(([x, , z], i) => {
    const key = `${Math.floor(x / HEAP_CELL)},${Math.floor(z / HEAP_CELL)}`
    const bin = bins.get(key) ?? { x: 0, z: 0, n: 0, roofs: 0 }
    bin.x += x
    bin.z += z
    bin.n++
    if (roofs[i]) bin.roofs++
    bins.set(key, bin)
  })
  const positions: number[] = []
  const colors: number[] = []
  const dark = (c: readonly [number, number, number], f: number) => [c[0] * f, c[1] * f, c[2] * f] as const
  const put = (soup: ShardSoup, at: readonly [number, number, number], scale: readonly [number, number, number], yaw: number) => {
    const [c, s] = [Math.cos(yaw), Math.sin(yaw)]
    for (let i = 0; i < soup.positions.length; i += 3) {
      const [x, y, z] = [soup.positions[i]! * scale[0], soup.positions[i + 1]! * scale[1], soup.positions[i + 2]! * scale[2]]
      positions.push(at[0] + x * c - z * s, at[1] + y, at[2] + x * s + z * c)
      colors.push(soup.colors[i]!, soup.colors[i + 1]!, soup.colors[i + 2]!)
    }
  }
  for (const [key, bin] of [...bins].sort(([a], [b]) => (a < b ? -1 : 1))) {
    const [x, z] = [bin.x / bin.n, bin.z / bin.n]
    const next = random(Math.floor(x * 7919) ^ Math.floor(z * 104729) ^ key.length)
    const volume = bin.n * HEAP_PER_CELL
    const radius = Math.min(2.4, 0.5 + Math.sqrt(volume) * 0.9)
    const height = Math.min(1.4, Math.max(0.12, volume / (radius * radius * 1.1)))
    // On a slope the mound sits at the lowest ground round it, so no edge of it hangs in the air.
    const floor = Math.min(groundAt(x, z), ...[0, 1, 2, 3].map((k) => groundAt(x + Math.cos(k * 1.571) * radius * 0.8, z + Math.sin(k * 1.571) * radius * 0.8)))
    // The mound: a squashed lump, half sunk into the ground.
    put(shardSoup(next() * 1e9, dark(wall, 0.72), 0.5), [x, floor, z], [radius * 2, height * 2, radius * 2], next() * 6.28)
    // Plates on and round it, a few of them roof.
    const plates = Math.min(6, 1 + Math.floor(bin.n / 3))
    for (let i = 0; i < plates; i++) {
      const isRoof = next() * bin.n < bin.roofs
      const size = 0.25 + next() * 0.45
      const angle = next() * 6.28
      const out = next() * radius * 0.8
      const [px, pz] = [x + Math.cos(angle) * out, z + Math.sin(angle) * out]
      put(shardSoup(next() * 1e9, dark(isRoof ? roof : wall, 0.9), isRoof ? 0.18 : 0.32), [px, Math.max(groundAt(px, pz), floor) + height * (1 - out / radius) * 0.8, pz], [size, size, size], next() * 6.28)
    }
  }
  return { positions: Float32Array.from(positions), colors: Float32Array.from(colors) }
}
