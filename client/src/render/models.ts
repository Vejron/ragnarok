import { Geometry, Material, Mesh, parseGlb, type Node } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/**
 * Parses a `.glb` into a node tree whose materials are the game's flat toon ones, keeping each one's
 * name and base colour (so `paint`, `lamp` and the rest can be found and replaced). Every mesh casts a
 * shadow. `convert` can swap a material for something else (a headlight is unlit). `merge` joins plain
 * parts into fewer draws, except those whose material is `paint` or named in `keep`, so they can still be
 * recoloured.
 */
export function parseToonModel(
  data: ArrayBuffer,
  convert?: (source: Material) => Material | null,
  options: { merge?: boolean; keep?: readonly string[] } = {},
): Node {
  const { scene } = parseGlb(data)
  scene.traverse((node) => {
    if (!(node instanceof Mesh)) return
    const source = node.material
    node.material = convert?.(source) ?? toonMaterial({ color: source.color, name: source.name })
    node.castShadow = true
  })
  if (options.merge) {
    mergeParts(scene, options.keep ?? [])
    shareEqualGeometry(scene)
  }
  return scene
}

/** What every merged part is drawn with: the colour of each piece is in its vertices. */
const mergedMaterial = toonMaterial({ vertexColors: true, name: 'merged' })

/**
 * Cuts a model's draws down. Under every node that holds several meshes (a glTF node with several
 * materials), the plain lit ones are joined into one mesh whose vertices carry each piece's colour.
 * Left alone: the `paint` (every vehicle recolours it) and the materials in `keep`, unlit lamps and anything textured, double
 * sided or transparent, and the nodes themselves, so every part the game moves by name still moves.
 */
function mergeParts(root: Node, keep: readonly string[]) {
  const mergeable = (child: Node): child is Mesh => {
    if (child.constructor !== Mesh) return false
    const mesh = child as Mesh
    const m = mesh.material
    return (
      mesh.visible &&
      mesh.castShadow &&
      m.lit &&
      m.name !== 'paint' &&
      !keep.includes(m.name) &&
      !m.map &&
      !m.normalMap &&
      m.side === 'front' &&
      m.blend === 'opaque' &&
      m.opacity === 1 &&
      m.depthBias === 0 &&
      mesh.children.length === 0 &&
      mesh.position.lengthSq() === 0 &&
      mesh.scale.x === 1 &&
      mesh.scale.y === 1 &&
      mesh.scale.z === 1 &&
      mesh.quaternion.w === 1
    )
  }
  const parents: Node[] = []
  root.traverse((node) => {
    if (node.children.filter(mergeable).length >= 2) parents.push(node)
  })
  for (const parent of parents) {
    const parts = parent.children.filter(mergeable)
    const pieces = parts.map((mesh) => {
      const geometry = mesh.geometry.clone()
      const { r, g, b } = mesh.material.color
      const count = geometry.vertexCount
      const colors = new Float32Array(count * 3)
      for (let i = 0; i < count; i++) {
        const own = geometry.colors ? geometry.colors.length / count : 0
        colors[i * 3] = r * (own ? geometry.colors![i * own]! : 1)
        colors[i * 3 + 1] = g * (own ? geometry.colors![i * own + 1]! : 1)
        colors[i * 3 + 2] = b * (own ? geometry.colors![i * own + 2]! : 1)
      }
      geometry.colors = colors
      return geometry
    })
    const merged = new Mesh(Geometry.merge(pieces), mergedMaterial)
    merged.name = `${parent.name}_parts`
    merged.castShadow = true
    for (const mesh of parts) parent.remove(mesh)
    parent.add(merged)
  }
}

/** Meshes whose geometry holds the same numbers (the wheels of a tank) are pointed at one copy, so they draw as one instanced call. */
function shareEqualGeometry(root: Node) {
  const seen = new Map<string, Geometry>()
  root.traverse((node) => {
    if (node.constructor !== Mesh) return
    const mesh = node as Mesh
    const g = mesh.geometry
    let hash = 2166136261
    const mix = (array: ArrayLike<number> | undefined) => {
      if (!array) return
      for (let i = 0; i < array.length; i++) hash = Math.imul(hash ^ (Math.round(array[i]! * 1e5) | 0), 16777619)
    }
    mix(g.positions)
    mix(g.colors)
    mix(g.indices)
    const key = `${g.vertexCount}:${g.indexCount}:${hash >>> 0}`
    const found = seen.get(key)
    if (found && found !== g) mesh.geometry = found
    else seen.set(key, g)
  })
}

/** An unlit material of `source`'s colour: lamps and the like. */
export const unlitMaterial = (source: Material) => new Material({ color: source.color, name: source.name, lit: false })
