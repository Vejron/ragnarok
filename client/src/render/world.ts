import * as THREE from 'three'
import { ROAD_MAJOR, ROAD_MINOR, type WorldData } from '@/world'

/** Candy-coloured buildings. Picked per building so the palette stays stable between reloads. */
const BUILDING_COLORS = [0xffd6a5, 0xfdffb6, 0xcaffbf, 0x9bf6ff, 0xa0c4ff, 0xbdb2ff, 0xffc6ff, 0xffadad]
const ROAD_COLORS: Record<number, number> = { [ROAD_MAJOR]: 0x7d7f8c, [ROAD_MINOR]: 0x9a9caa }
const PATH_COLOR = 0xe8d9b5

/** Builds the static scene: grass, roads as flat ribbons and extruded buildings. */
export function buildWorld(data: WorldData): THREE.Group {
  const group = new THREE.Group()
  const half = data.halfExtent

  const ground = new THREE.Mesh(new THREE.PlaneGeometry(half * 2, half * 2), new THREE.MeshLambertMaterial({ color: 0x8fd16a }))
  ground.rotation.x = -Math.PI / 2
  ground.receiveShadow = true
  group.add(ground)

  // Roads: one quad per segment, plus a disc per joint so corners have no gaps.
  const roadGeometries = new Map<number, THREE.BufferGeometry[]>()
  data.roads.forEach((road, index) => {
    const color = ROAD_COLORS[road.kind] ?? PATH_COLOR
    const list = roadGeometries.get(color) ?? []
    // Paths draw on top of roads, roads on top of grass; the tiny lift avoids z-fighting.
    const lift = 0.02 + (road.kind === ROAD_MAJOR ? 0 : road.kind === ROAD_MINOR ? 0.005 : 0.01) + (index % 7) * 0.0003
    for (let i = 0; i + 1 < road.points.length; i++) {
      const [ax, az] = road.points[i]!
      const [bx, bz] = road.points[i + 1]!
      const length = Math.hypot(bx - ax, bz - az)
      if (length < 0.01) continue
      const quad = new THREE.PlaneGeometry(length, road.width)
      quad.rotateX(-Math.PI / 2)
      quad.rotateY(-Math.atan2(bz - az, bx - ax))
      quad.translate((ax + bx) / 2, lift, (az + bz) / 2)
      list.push(quad)
    }
    for (const [x, z] of road.points) {
      const disc = new THREE.CircleGeometry(road.width / 2, 12)
      disc.rotateX(-Math.PI / 2)
      disc.translate(x, lift, z)
      list.push(disc)
    }
    roadGeometries.set(color, list)
  })
  for (const [color, geometries] of roadGeometries) {
    const merged = mergeGeometries(geometries)
    const mesh = new THREE.Mesh(merged, new THREE.MeshLambertMaterial({ color }))
    mesh.receiveShadow = true
    group.add(mesh)
  }

  // Buildings: extrude each footprint upwards. Shape space is (x, -z) so the extrusion along +Z
  // becomes +Y after rotating the geometry -90° about X.
  const byColor = new Map<number, THREE.BufferGeometry[]>()
  data.buildings.forEach((building, index) => {
    const shape = new THREE.Shape(building.footprint.map(([x, z]) => new THREE.Vector2(x, -z)))
    const geometry = new THREE.ExtrudeGeometry(shape, { depth: building.height, bevelEnabled: false })
    geometry.rotateX(-Math.PI / 2)
    const color = BUILDING_COLORS[index % BUILDING_COLORS.length]!
    const list = byColor.get(color) ?? []
    list.push(geometry)
    byColor.set(color, list)
  })
  for (const [color, geometries] of byColor) {
    const mesh = new THREE.Mesh(mergeGeometries(geometries), new THREE.MeshToonMaterial({ color }))
    mesh.castShadow = true
    mesh.receiveShadow = true
    group.add(mesh)
  }
  return group
}

/** Concatenates non-indexed copies of geometries that share attributes (position, normal, uv). */
function mergeGeometries(geometries: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const parts = geometries.map((g) => (g.index ? g.toNonIndexed() : g))
  const merged = new THREE.BufferGeometry()
  for (const name of ['position', 'normal'] as const) {
    const total = parts.reduce((sum, g) => sum + g.getAttribute(name).array.length, 0)
    const array = new Float32Array(total)
    let offset = 0
    for (const g of parts) {
      const source = g.getAttribute(name).array
      array.set(source, offset)
      offset += source.length
    }
    merged.setAttribute(name, new THREE.BufferAttribute(array, 3))
  }
  for (const g of geometries) g.dispose()
  for (const g of parts) g.dispose()
  merged.computeBoundingSphere()
  return merged
}
