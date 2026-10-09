import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { decodeWorld, worldHash } from '@/world'

describe('world file', () => {
  it('hashes with FNV-1a like the server', () => {
    expect(worldHash(new Uint8Array())).toBe(0x811c9dc5)
    expect(worldHash(new TextEncoder().encode('a'))).toBe(0xe40c292c)
  })

  it('decodes the generated world', () => {
    const bytes = readFileSync(resolve(process.cwd(), 'public/world.msgpack'))
    const world = decodeWorld(new Uint8Array(bytes))
    expect(world.halfExtent).toBeGreaterThan(50)
    expect(world.buildings.length).toBeGreaterThan(0)
    expect(world.spawns.length).toBeGreaterThan(0)
    expect(world.collision.triangles.length % 3).toBe(0)
    expect(Math.max(...world.collision.triangles)).toBeLessThan(world.collision.vertices.length / 3)
  })
})
