import { decode } from '@msgpack/msgpack'
import { describe, expect, it } from 'vitest'
import { v1Rows, v1Scene } from '../net/adapter'
import { sceneStates } from '../net/codec'
import type { Components } from '../net/components.generated'
import fixture from './adapter.fixture.json'

// `adapter.fixture.json` is written by the Rust tests: a scene with every family and the edge cases of
// each, and the v1 rows of it as the server's v1 encoder puts them on the wire
// (`shared::replicated::v1_rows`, which the server's own v1 snapshot code is held to).
const scene = new Map(fixture.scene as [number, Components][])
const [entities, vehicles, rockets, pickups] = decode(new Uint8Array(fixture.v1)) as unknown[][]

describe('the v1 adapter', () => {
  it('turns a v2 scene into exactly the rows v1 sent', () => {
    expect(v1Rows(scene)).toEqual({ entities, vehicles, rockets, pickups })
  })

  it('gives the game the states the v1 decoder gives, in its units', () => {
    const { wire, rockets: flying, pickups: lying } = v1Scene(scene)
    const { entities: people, vehicles: cars } = sceneStates(wire)
    expect(people.map((e) => e.id)).toEqual([1, 2])
    expect(people[0]).toMatchObject({ prone: true, crouching: true, grounded: true, knockX: 1.5, held: 2 })
    expect(people[1]).toMatchObject({ kind: 5, crowing: true, vy: 0 })
    expect(cars.find((v) => v.kind === 9)?.turret?.stowed).toBe(true)
    expect(cars.find((v) => v.kind === 6)).toMatchObject({ arms: { bombs: 4 }, crew: [{ held: 0 }, { held: 2 }] })
    expect(cars.find((v) => v.kind === 1)?.hp).toBe(0)
    expect(flying).toEqual([{ id: 9, kind: 0, x: 0.01, y: 20, z: 0.03, vx: -40, vy: 0.5, vz: 0 }])
    expect(lying).toEqual([{ id: 10, item: 7, x: -0.05, y: 0.3, z: 0.05 }])
  })
})
