import { Color, Geometry, InstancedMesh, boxGeometry, cylinderGeometry, mat4, type Material } from '@ragnarok/engine'
import { GRAVE_CROSS, GRAVE_HEADSTONE, GRAVE_KINDS, GRAVE_LOW, graveKindDef, type Grave, type GraveKindDef } from '@/world'
import { painted } from './lines'
import { hashAt } from './rocks'
import { winter } from './season'
import { toonMaterial } from './toon'

/*
 * The tombstones of the graveyards (`TileData.graves`): the importer stands them in rows along a
 * yard's paths, or in rows of their own where it has none, each turned to face its path. They are
 * world data like the rocks: solid (`grave_collider` in `crates/sim`), and nothing breaks them. Each
 * kind is one shape, made once to the size of its collider (`GRAVE_KINDS`), and a tile draws its
 * stones as one instanced mesh a kind, each culled on its own and tinted as its own stone.
 */

/** How deep a stone reaches into the ground, metres, so it stands on a slope (`GRAVE_SUNK` in `crates/sim`). */
const SUNK = 0.3

/** The stones they are cut from: grey and dark granite, a red one and a pale limestone, by how common. */
const STONES: readonly { color: Color; weight: number }[] = [
  { color: Color.hex(0x8f918d), weight: 0.4 },
  { color: Color.hex(0x6b6e71), weight: 0.25 },
  { color: Color.hex(0x8e6f66), weight: 0.15 },
  { color: Color.hex(0xb9b5ab), weight: 0.2 },
]

/** A part of a stone: white, for the instance's tint to colour. */
const part = (geometry: Geometry): Geometry => painted(geometry, 0xffffff)

/**
 * The shape of a kind of stone, its foot at the origin and its face looking along +Z, filling the
 * box of `def`: `halfWidth` to each side along X, `halfDepth` along Z and `height` tall.
 */
export function graveGeometry(def: GraveKindDef): Geometry {
  const [width, depth, height] = [def.halfWidth * 2, def.halfDepth * 2, def.height]
  if (def.kind === GRAVE_CROSS) {
    const [plinth, bar] = [0.22, 0.16]
    return Geometry.merge([
      part(boxGeometry(width, plinth + SUNK, depth).translate(0, (plinth - SUNK) / 2, 0)),
      part(boxGeometry(bar, height - plinth, depth * 0.7).translate(0, (height + plinth) / 2, 0)),
      part(boxGeometry(width * 0.9, bar, depth * 0.7).translate(0, height * 0.72, 0)),
    ])
  }
  if (def.kind === GRAVE_LOW) {
    // A slab leaning back on a low base, its face turned up to whoever stands before it.
    const [base, lean, thick] = [0.2, 0.25, 0.12]
    const slab = (height - thick * Math.sin(lean)) / Math.cos(lean)
    return Geometry.merge([
      part(boxGeometry(width, base + SUNK, depth).translate(0, (base - SUNK) / 2, 0)),
      part(boxGeometry(width * 0.92, slab, thick).translate(0, slab / 2, 0).rotateX(-lean).translate(0, 0, (depth - thick) / 2)),
    ])
  }
  // An upright slab on a plinth, its top a half round as wide as the slab.
  const [plinth, slabWidth, slabDepth] = [0.12, width * 0.86, depth * 0.7]
  const shoulder = height - slabWidth / 2
  return Geometry.merge([
    part(boxGeometry(width, plinth + SUNK, depth).translate(0, (plinth - SUNK) / 2, 0)),
    part(boxGeometry(slabWidth, shoulder - plinth, slabDepth).translate(0, (shoulder + plinth) / 2, 0)),
    part(cylinderGeometry(slabWidth / 2, slabWidth / 2, slabDepth, 14).rotateX(Math.PI / 2).translate(0, shoulder, 0)),
  ])
}

let shared: { shapes: Map<number, Geometry>; material: Material } | undefined

function prototypes() {
  shared ??= {
    shapes: new Map(GRAVE_KINDS.map((def) => [def.kind, graveGeometry(def)])),
    material: toonMaterial({ vertexColors: true, snow: winter }),
  }
  return shared
}

/** One tile's tombstones: one instanced mesh a kind, each stone where the world stood it and turned to its heading. */
export function buildGraves(graves: readonly Grave[] | undefined): InstancedMesh[] {
  if (!graves || graves.length === 0) return []
  const { shapes, material } = prototypes()
  const byKind = new Map<number, Grave[]>()
  for (const grave of graves) {
    const kind = graveKindDef(grave.kind).kind
    const list = byKind.get(kind) ?? []
    list.push(grave)
    byKind.set(kind, list)
  }
  const matrix = mat4.create()
  const meshes: InstancedMesh[] = []
  for (const [kind, list] of byKind) {
    const mesh = new InstancedMesh(shapes.get(kind) ?? shapes.get(GRAVE_HEADSTONE)!, material, list.length)
    list.forEach((grave, i) => {
      // Turned about the vertical: at 0 its face looks along +Z, and towards +X as the angle grows.
      mesh.setMatrixAt(i, mat4.compose(matrix, grave.x, grave.base, grave.z, 0, Math.sin(grave.yaw / 2), 0, Math.cos(grave.yaw / 2), 1, 1, 1))
      let pick = hashAt(grave.x, grave.z, 21)
      const stone = STONES.find((s) => (pick -= s.weight) < 0) ?? STONES[0]!
      const shade = 0.92 + hashAt(grave.x, grave.z, 22) * 0.16
      mesh.setColorAt(i, stone.color.r * shade, stone.color.g * shade, stone.color.b * shade)
    })
    mesh.needsUpdate()
    // No coarser shapes, but each stone is culled on its own, as the rocks are.
    mesh.levels = []
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.sharedGeometry = true
    mesh.userData.graves = kind
    meshes.push(mesh)
  }
  return meshes
}
