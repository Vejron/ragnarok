import * as THREE from 'three'
import { toonMaterial } from './toon'
import { ITEM_AK5, ITEM_BANDAGE, ITEM_GOLDEN_EGG, ITEM_GRENADE, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER } from '@/sim/items'

const GUN_METAL = 0x3b3f47
const GUN_TRIM = 0xf2a33a
const WOOD = 0x7a4a2a
const OLIVE = 0x4f5d2e
const CLOTH = 0xf2f2ee
const CROSS = 0xd93a3a
const GOLD = 0xffc933

const materials = new Map<number, THREE.MeshToonMaterial>()
const boxes = new Map<string, THREE.BoxGeometry>()
let sphere: THREE.SphereGeometry | null = null
let shiny: THREE.MeshToonMaterial | null = null

/** One shared toon material per colour: items are small and many. */
const material = (color: number) => {
  let m = materials.get(color)
  if (!m) materials.set(color, (m = toonMaterial({ color })))
  return m
}

const box = (group: THREE.Group, size: [number, number, number], at: [number, number, number], color: number) => {
  const key = size.join(',')
  let geometry = boxes.get(key)
  if (!geometry) boxes.set(key, (geometry = new THREE.BoxGeometry(...size)))
  const mesh = new THREE.Mesh(geometry, material(color))
  mesh.position.set(...at)
  group.add(mesh)
  return mesh
}

/**
 * A low-poly model of `item`, about as long as it is in the player's hands, lying along -Z with
 * its grip at the origin: the same pose whether it sits on the ground or sits in a hand.
 */
export function buildItem(item: number): THREE.Group {
  const group = new THREE.Group()
  switch (item) {
    case ITEM_REVOLVER:
      box(group, [0.04, 0.05, 0.2], [0, 0.04, -0.1], GUN_METAL)
      box(group, [0.05, 0.065, 0.07], [0, 0.035, -0.02], GUN_METAL)
      box(group, [0.04, 0.11, 0.05], [0, -0.03, 0.04], WOOD).rotation.x = -0.25
      break
    case ITEM_AK5:
      box(group, [0.05, 0.07, 0.62], [0, 0.04, -0.2], GUN_METAL)
      box(group, [0.045, 0.045, 0.3], [0, 0.04, -0.66], GUN_METAL)
      box(group, [0.05, 0.14, 0.05], [0, -0.07, -0.18], GUN_TRIM).rotation.x = 0.2
      box(group, [0.05, 0.11, 0.05], [0, -0.05, 0.05], WOOD).rotation.x = -0.3
      box(group, [0.045, 0.1, 0.22], [0, 0.0, 0.3], WOOD)
      break
    case ITEM_SNIPER:
      box(group, [0.045, 0.065, 0.7], [0, 0.04, -0.22], GUN_METAL)
      box(group, [0.03, 0.03, 0.4], [0, 0.04, -0.8], GUN_METAL)
      box(group, [0.05, 0.05, 0.3], [0, 0.12, -0.22], GUN_TRIM)
      box(group, [0.05, 0.11, 0.05], [0, -0.05, 0.05], WOOD).rotation.x = -0.3
      box(group, [0.05, 0.1, 0.24], [0, 0.0, 0.32], WOOD)
      break
    case ITEM_SHOTGUN:
      // A fat double barrel and a long wooden fore-end under it.
      box(group, [0.06, 0.06, 0.62], [0, 0.04, -0.28], GUN_METAL)
      box(group, [0.07, 0.05, 0.3], [0, -0.02, -0.34], WOOD)
      box(group, [0.05, 0.11, 0.05], [0, -0.05, 0.05], WOOD).rotation.x = -0.3
      box(group, [0.045, 0.1, 0.26], [0, 0.0, 0.3], WOOD)
      break
    case ITEM_GRENADE: {
      sphere ??= new THREE.SphereGeometry(0.06, 10, 8)
      const body = new THREE.Mesh(sphere, material(OLIVE))
      body.scale.set(1, 1.15, 1)
      group.add(body)
      box(group, [0.03, 0.025, 0.05], [0, 0.07, 0], GUN_METAL)
      box(group, [0.015, 0.02, 0.07], [0.03, 0.075, 0], GUN_TRIM)
      break
    }
    case ITEM_BANDAGE:
      box(group, [0.16, 0.07, 0.1], [0, 0, 0], CLOTH)
      box(group, [0.1, 0.075, 0.025], [0, 0, 0], CROSS)
      box(group, [0.025, 0.075, 0.07], [0, 0, 0], CROSS)
      break
    case ITEM_GOLDEN_EGG: {
      sphere ??= new THREE.SphereGeometry(0.06, 10, 8)
      // Shiny and a little glowing, so it is seen across a field.
      shiny ??= toonMaterial({ color: GOLD, emissive: 0x8a5a00 })
      const egg = new THREE.Mesh(sphere, shiny)
      egg.scale.set(0.9, 1.25, 0.9)
      group.add(egg)
      break
    }
  }
  group.traverse((node) => {
    node.castShadow = true
  })
  return group
}

export function disposeItems() {
  for (const m of materials.values()) m.dispose()
  for (const g of boxes.values()) g.dispose()
  sphere?.dispose()
  shiny?.dispose()
  shiny = null
  materials.clear()
  boxes.clear()
  sphere = null
}
