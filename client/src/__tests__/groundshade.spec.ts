import { describe, expect, it } from 'vitest'
import { heightIn, type TileGround } from '@/sim/terrain'
import { GroundShade, heightSampler, normalAt, ringMean, toneAt, toneOf } from '@/render/shade'

/** 64 m of 1 m ground: rolling, with a sharp ditch along z = 30, the worst case for blended normals. */
function bumpy(): TileGround {
  const side = 65
  const heights = new Float32Array(side * side)
  for (let j = 0; j < side; j++) {
    for (let i = 0; i < side; i++) {
      const ditch = Math.abs(j - 30) <= 1 ? -1.2 : 0
      heights[j * side + i] = 3 * Math.sin(i * 0.11) * Math.cos(j * 0.07) + 0.4 * Math.sin(i * 0.9 + j * 0.6) + ditch
    }
  }
  return { x0: 100, z0: -50, cell: 1, side, heights }
}

describe('GroundShade', () => {
  const ground = bumpy()
  const sample = heightSampler(ground)
  const shade = new GroundShade(sample, ground.cell, [100, -50, 164, 14])
  // Points inside the tile and in the margin past its edge, off the nodes.
  const points = Array.from({ length: 4000 }, (_, k) => [96 + ((k * 7.31) % 72), -54 + ((k * 3.17) % 72)] as const)

  it('puts a point on the collider’s triangles, as heightIn does', () => {
    for (const [x, z] of points) expect(shade.height(x, z)).toBeCloseTo(sample(x, z), 4)
    expect(shade.height(110.25, -40.75)).toBeCloseTo(heightIn(ground, 110.25, -40.75), 4)
  })

  it('is normalAt and ringMean at the nodes', () => {
    for (const [x, z] of [[110, -40], [130, -20], [150, -21]] as const) {
      const [a, b] = [shade.normal(x, z), normalAt(sample, x, z, ground.cell)]
      for (let k = 0; k < 3; k++) expect(a[k]).toBeCloseTo(b[k]!, 5)
    }
    // The lattice of ring means starts 16 m before the border, every 3 m.
    expect(shade.mean(84 + 3 * 11, -66 + 3 * 12)).toBeCloseTo(ringMean(sample, 117, -30), 4)
  })

  it('shades every point within a hair of working it out directly', () => {
    let [worstTone, worstAngle, sumTone] = [0, 0, 0]
    for (const [x, z] of points) {
      const exact = normalAt(sample, x, z, ground.cell)
      const near = shade.normal(x, z)
      const dot = exact[0] * near[0] + exact[1] * near[1] + exact[2] * near[2]
      worstAngle = Math.max(worstAngle, Math.acos(Math.min(1, dot)))
      const tone = toneOf(near, shade.height(x, z), shade.mean(x, z), x, z)
      const off = Math.abs(tone - toneAt(sample, x, z, exact))
      worstTone = Math.max(worstTone, off)
      sumTone += off
    }
    // Tone runs 0.55 ..= 1.2. The normals are blended between nodes, so they part from the central
    // differences most at the ditch's edge: there about 0.06 of tone and 3°, elsewhere far less.
    expect(sumTone / points.length).toBeLessThan(0.01)
    expect(worstTone).toBeLessThan(0.07)
    expect((worstAngle * 180) / Math.PI).toBeLessThan(4)
  })
})
