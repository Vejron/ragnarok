import { describe, expect, it } from 'vitest'
import { KIND_HEN, KIND_JEEP, KIND_STRV, KIND_TANK, KIND_WALKER } from '../net/codec'
import { TICK_RATE } from '../sim/consts'
import * as items from '../sim/items'
import tables from '../sim/tables.generated.json'
import { VEHICLES, isLegged, isTracked, vehicleDef } from '../sim/vehicles'
import { TREE_BIRCH, TREE_BUSH, TREE_KINDS, TREE_PINE, TREE_ROCK_BOULDER, TREE_ROCK_SLAB, treeKindDef, trunkRadiusCm } from '../world'

// `sim/tables.generated.json` is written from the Rust tables by `cargo test -p shared parity`
// (`UPDATE_PARITY=1` rewrites it), which fails while it is stale. The client builds its item, vehicle
// and tree tables from it, so what is left to check here is what the client adds and its constants.
describe('gameplay tables come from the server', () => {
  it('has every item the server does, each with how it is held and sounds', () => {
    for (const row of tables.items) {
      const def = items.itemDef(row.id)
      expect(def, `item ${row.id}`).toEqual(expect.objectContaining({ name: row.name, damage: row.damage }))
      expect(def!.stance && def!.sound, `item ${row.id}`).toBeTruthy()
    }
    for (let id = 1; id < 32; id++) {
      expect(items.itemDef(id) !== undefined, `item ${id}`).toBe(tables.items.some((row) => row.id === id))
    }
  })

  it('has every vehicle kind the server does, the jeep first as the fallback', () => {
    expect(VEHICLES.map((def) => def.kind)).toEqual(tables.vehicles.map((row) => row.kind))
    expect(vehicleDef(999).kind).toBe(KIND_JEEP)
    expect(VEHICLES.filter((def) => isLegged(def.kind)).map((def) => def.kind)).toEqual([KIND_WALKER, KIND_HEN])
    expect(VEHICLES.filter((def) => isTracked(def.kind)).map((def) => def.kind)).toEqual([KIND_TANK, KIND_STRV])
    expect(isLegged(undefined) || isTracked(undefined)).toBe(false)
  })

  it('has every tree and rock archetype the server does, the birch first as the fallback', () => {
    expect(TREE_KINDS.map((def) => def.kind)).toEqual(tables.treeKinds.map((row) => row.kind))
    expect(treeKindDef(999).kind).toBe(TREE_BIRCH)
  })

  it('sizes colliders like Tree::trunk_radius: trees by height, rocks by their crown', () => {
    // The same cases are asserted in crates/shared/src/world.rs.
    expect(trunkRadiusCm(TREE_PINE, 1600, 300)).toBe(20)
    expect(trunkRadiusCm(TREE_PINE, 4000, 300)).toBe(50)
    expect(trunkRadiusCm(TREE_BIRCH, 100, 100)).toBe(5)
    expect(trunkRadiusCm(TREE_ROCK_BOULDER, 150, 100)).toBe(80)
    expect(trunkRadiusCm(TREE_ROCK_SLAB, 60, 200)).toBe(170)
    expect(trunkRadiusCm(TREE_ROCK_BOULDER, 150, 1000)).toBe(200)
    expect(trunkRadiusCm(TREE_BUSH, 100, 80)).toBe(10)
  })

  it('shares the scalar constants', () => {
    const mirrored: Record<string, number> = { ...items, TICK_RATE } as never
    for (const [name, value] of Object.entries(tables.consts)) {
      expect(mirrored[name], name).toBe(value)
    }
  })
})

describe('falloff', () => {
  it('keeps full damage up close, thins out with range and never reaches zero', () => {
    const ak5 = items.itemDef(items.ITEM_AK5)!
    expect(items.falloffDamage(ak5, 10)).toBe(ak5.damage)
    expect(items.falloffDamage(ak5, ak5.range)).toBe(ak5.damage * ak5.falloffMin)
    const shotgun = items.itemDef(items.ITEM_SHOTGUN)!
    expect(shotgun.pellets * items.falloffDamage(shotgun, 3)).toBeGreaterThanOrEqual(100)
    expect(items.falloffDamage(shotgun, 1000)).toBeGreaterThanOrEqual(1)
  })
})
