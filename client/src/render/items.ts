import { Group, Material, Mesh, boxGeometry, sphereGeometry, type Geometry } from '@ragnarok/engine'
import { toonMaterial } from './toon'
import { ITEM_AK5, ITEM_AT_RIFLE, ITEM_BANDAGE, ITEM_BRICKS, ITEM_CARL_GUSTAF, ITEM_DRONE, ITEM_FLARE, ITEM_GOLDEN_EGG, ITEM_LOITER, ITEM_GRENADE, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER } from '@/sim/items'

const GUN_METAL = 0x3b3f47
const GUN_TRIM = 0xf2a33a
const WOOD = 0x7a4a2a
const OLIVE = 0x4f5d2e
const CLOTH = 0xf2f2ee
const CROSS = 0xd93a3a
const GOLD = 0xffc933
const FLARE_ORANGE = 0xf26a1b
const BRICK = 0xc0583e
const TUBE = 0x55613a

const materials = new Map<number, Material>()
const boxes = new Map<string, Geometry>()
let sphere: Geometry | null = null
let shiny: Material | null = null

/** One shared toon material per colour: items are small and many. */
const material = (color: number) => {
  let m = materials.get(color)
  if (!m) materials.set(color, (m = toonMaterial({ color })))
  return m
}

const box = (group: Group, size: [number, number, number], at: [number, number, number], color: number) => {
  const key = size.join(',')
  let geometry = boxes.get(key)
  if (!geometry) boxes.set(key, (geometry = boxGeometry(...size)))
  const mesh = new Mesh(geometry, material(color))
  mesh.position.set(...at)
  group.add(mesh)
  return mesh
}

/**
 * A low-poly model of `item`, about as long as it is in the player's hands, lying along -Z with
 * its grip at the origin: the same pose whether it sits on the ground or sits in a hand.
 */
export function buildItem(item: number): Group {
  const group = new Group()
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
    case ITEM_AT_RIFLE:
      // A long heavy barrel with a muzzle brake, a bipod folded under it and a plain wooden stock.
      box(group, [0.05, 0.07, 0.55], [0, 0.04, -0.2], GUN_METAL)
      box(group, [0.035, 0.035, 0.7], [0, 0.04, -0.82], GUN_METAL)
      box(group, [0.06, 0.05, 0.08], [0, 0.04, -1.2], GUN_TRIM)
      box(group, [0.08, 0.02, 0.3], [0, -0.01, -0.7], GUN_TRIM)
      box(group, [0.05, 0.11, 0.05], [0, -0.05, 0.05], WOOD).rotation.x = -0.3
      box(group, [0.05, 0.1, 0.26], [0, 0.0, 0.33], WOOD)
      break
    case ITEM_SHOTGUN:
      // A fat double barrel and a long wooden fore-end under it.
      box(group, [0.06, 0.06, 0.62], [0, 0.04, -0.28], GUN_METAL)
      box(group, [0.07, 0.05, 0.3], [0, -0.02, -0.34], WOOD)
      box(group, [0.05, 0.11, 0.05], [0, -0.05, 0.05], WOOD).rotation.x = -0.3
      box(group, [0.045, 0.1, 0.26], [0, 0.0, 0.3], WOOD)
      break
    case ITEM_GRENADE: {
      sphere ??= sphereGeometry(0.06, 10, 8)
      const body = new Mesh(sphere, material(OLIVE))
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
    case ITEM_DRONE:
      // A folded quad: a body, two crossed arms with a motor at each end, and the charge under it.
      box(group, [0.1, 0.04, 0.14], [0, 0.03, 0], GUN_METAL)
      box(group, [0.04, 0.03, 0.12], [0, -0.01, 0], OLIVE)
      for (const angle of [Math.PI / 4, -Math.PI / 4]) box(group, [0.025, 0.015, 0.4], [0, 0.04, 0], GUN_METAL).rotation.y = angle
      for (const [x, z] of [[-0.14, -0.14], [0.14, -0.14], [-0.14, 0.14], [0.14, 0.14]] as const) box(group, [0.04, 0.035, 0.04], [x, 0.055, z], GUN_TRIM)
      break
    case ITEM_LOITER:
      // A launch crate with the drone's swept wing and fins showing on top.
      box(group, [0.3, 0.14, 0.42], [0, 0, 0], OLIVE)
      box(group, [0.5, 0.02, 0.16], [0, 0.08, 0.1], GUN_METAL)
      box(group, [0.28, 0.02, 0.14], [0, 0.08, -0.04], GUN_METAL)
      for (const x of [-0.24, 0.24]) box(group, [0.015, 0.08, 0.1], [x, 0.12, 0.12], GUN_TRIM)
      break
    case ITEM_FLARE:
      // A stubby orange pistol with a fat barrel and a dark breech.
      box(group, [0.06, 0.06, 0.2], [0, 0.045, -0.11], FLARE_ORANGE)
      box(group, [0.05, 0.065, 0.06], [0, 0.04, -0.01], GUN_METAL)
      box(group, [0.045, 0.11, 0.05], [0, -0.03, 0.04], FLARE_ORANGE).rotation.x = -0.25
      break
    case ITEM_BRICKS:
      // Two bricks, the top one turned across the bottom one.
      box(group, [0.1, 0.06, 0.2], [0, 0, 0], BRICK)
      box(group, [0.2, 0.06, 0.1], [0, 0.065, 0], BRICK)
      break
    case ITEM_CARL_GUSTAF:
      // A long olive tube, its venturi flaring behind, with the grips and the sight on its left.
      box(group, [0.1, 0.1, 1.0], [0, 0.2, 0.2], TUBE)
      box(group, [0.15, 0.15, 0.1], [0, 0.2, 0.74], GUN_METAL)
      box(group, [0.115, 0.115, 0.06], [0, 0.2, -0.31], GUN_METAL)
      box(group, [0.04, 0.15, 0.05], [0, 0.07, 0.02], GUN_TRIM)
      box(group, [0.035, 0.12, 0.04], [0, 0.1, -0.2], GUN_METAL)
      box(group, [0.04, 0.05, 0.12], [-0.08, 0.23, 0.06], GUN_METAL)
      break
    case ITEM_GOLDEN_EGG: {
      sphere ??= sphereGeometry(0.06, 10, 8)
      // Shiny and a little glowing, so it is seen across a field.
      shiny ??= toonMaterial({ color: GOLD, emissive: 0x8a5a00 })
      const egg = new Mesh(sphere, shiny)
      egg.scale.set(0.9, 1.25, 0.9)
      group.add(egg)
      break
    }
  }
  group.traverse((node) => {
    if (node instanceof Mesh) node.castShadow = true
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
