import { Material, Mesh, parseGlb, type Node } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/**
 * Parses a `.glb` into a node tree whose materials are the game's flat toon ones, keeping each one's
 * name and base colour (so `paint`, `lamp` and the rest can be found and replaced). Every mesh casts a
 * shadow. `convert` can swap a material for something else (a headlight is unlit).
 */
export function parseToonModel(data: ArrayBuffer, convert?: (source: Material) => Material | null): Node {
  const { scene } = parseGlb(data)
  scene.traverse((node) => {
    if (!(node instanceof Mesh)) return
    const source = node.material
    node.material = convert?.(source) ?? toonMaterial({ color: source.color, name: source.name })
    node.castShadow = true
  })
  return scene
}

/** An unlit material of `source`'s colour: lamps and the like. */
export const unlitMaterial = (source: Material) => new Material({ color: source.color, name: source.name, lit: false })
