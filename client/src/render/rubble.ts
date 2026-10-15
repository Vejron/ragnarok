import { Color, Group, InstancedMesh, Mesh, boxGeometry, icosphereGeometry, mat4 } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import type { TileData } from '@/world'
import type { Effects } from './effects'
import { buildingColors, collapsingBuilding } from './world'

/** How long the walls take to come down, seconds. */
const COLLAPSE_SECONDS = 1.8
/** How long the flung pieces lie about before they are gone, seconds. */
const CHUNK_SECONDS = 9
const CHUNK_FADE_SECONDS = 1
const GRAVITY = 9.8
/** The pile left behind: no taller than this, metres, however tall the building was. */
const HEAP_MAX_HEIGHT = 3

const heapMaterial = toonMaterial({ color: 0x8d8478 })
// Flat faces, as the heap was before: each triangle gets its own normal. Shared by every heap, never disposed.
const heapGeometry = (() => {
  const geometry = icosphereGeometry(1, 1).toNonIndexed()
  geometry.computeNormals()
  return geometry
})()
const chunkMaterial = toonMaterial()
const chunkGeometry = boxGeometry(1, 1, 1)

/** A small deterministic generator, so every client throws the same pieces the same way. */
export function random(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

interface Chunk {
  position: THREE.Vector3
  velocity: THREE.Vector3
  size: THREE.Vector3
  spin: THREE.Vector3
  rotation: THREE.Euler
  resting: boolean
}

interface Collapse {
  body: Group
  heap: Mesh
  heapHeight: number
  chunks: InstancedMesh
  pieces: Chunk[]
  ground: (x: number, z: number) => number
  age: number
  puffs: number
  centre: THREE.Vector3
  spread: number
}

/** The pile a building leaves, at full height (or `0` high, to be grown): flat-shaded and low. */
function heapOf(tile: TileData, index: number, scale: number): { heap: Mesh; height: number } {
  const building = tile.buildings[index]!
  const xs = building.footprint.map((p) => p[0])
  const zs = building.footprint.map((p) => p[1])
  const [minX, maxX, minZ, maxZ] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)]
  const height = Math.min(HEAP_MAX_HEIGHT, Math.max(1, building.height * 0.2))
  const heap = new Mesh(heapGeometry, heapMaterial)
  heap.position.set((minX + maxX) / 2, building.base, (minZ + maxZ) / 2)
  heap.scale.set(Math.max(1, (maxX - minX) * 0.46), Math.max(0.01, height * scale), Math.max(1, (maxZ - minZ) * 0.46))
  heap.castShadow = true
  heap.receiveShadow = true
  return { heap, height }
}

/**
 * Buildings coming down. A destroyed building's walls sink into the ground as a pile of rubble
 * rises where it stood, pieces are flung away from the blast, and dust billows. It is only a
 * picture: the server's only word is that the building is gone, and the pieces collide with
 * nothing but the ground. Each lasts a few seconds, then only the pile remains, drawn with the
 * tile it is in.
 */
export class Collapses {
  private readonly active: Collapse[] = []

  /**
   * Starts building `index` of `tile` (drawn in `group`) coming down. `yaw` is the way the
   * blast came from, as the server says (0 faces -Z): pieces fly the other way.
   */
  start(group: Group, tile: TileData, index: number, salt: number, yaw: number, ground: (x: number, z: number) => number, seed: number) {
    const building = tile.buildings[index]!
    const body = collapsingBuilding(tile, index, salt)
    group.add(body)
    const { heap, height } = heapOf(tile, index, 0)
    group.add(heap)

    const random_ = random(seed)
    const xs = building.footprint.map((p) => p[0])
    const zs = building.footprint.map((p) => p[1])
    const [minX, maxX, minZ, maxZ] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)]
    const area = (maxX - minX) * (maxZ - minZ)
    const count = Math.round(Math.min(40, Math.max(12, area / 4)))
    // The way the pieces are thrown: yaw 0 faces -Z, and the blast came from there.
    const away = new THREE.Vector3(Math.sin(yaw), 0, Math.cos(yaw))
    const { wall, roof } = buildingColors(tile, index, salt)
    const chunks = new InstancedMesh(chunkGeometry, chunkMaterial, count)
    chunks.userData.sharedGeometry = true
    chunks.castShadow = true
    chunks.frustumCulled = false
    const pieces: Chunk[] = []
    const color = new Color()
    for (let i = 0; i < count; i++) {
      const size = 0.4 + random_() * 0.9
      const start = new THREE.Vector3(minX + random_() * (maxX - minX), building.base + random_() * building.height, minZ + random_() * (maxZ - minZ))
      const speed = 1.5 + random_() * 5
      pieces.push({
        position: start,
        velocity: new THREE.Vector3(
          away.x * speed + (random_() - 0.5) * 4,
          1 + random_() * 5,
          away.z * speed + (random_() - 0.5) * 4,
        ),
        size: new THREE.Vector3(size, size * (0.4 + random_() * 0.6), size * (0.6 + random_() * 0.6)),
        spin: new THREE.Vector3((random_() - 0.5) * 8, (random_() - 0.5) * 8, (random_() - 0.5) * 8),
        rotation: new THREE.Euler(random_() * 3, random_() * 3, random_() * 3),
        resting: false,
      })
      color.setHex(random_() < 0.7 ? wall : roof)
      chunks.setColorAt(i, color.r, color.g, color.b)
    }
    group.add(chunks)
    this.active.push({
      body,
      heap,
      heapHeight: height,
      chunks,
      pieces,
      ground,
      age: 0,
      puffs: 0,
      centre: new THREE.Vector3((minX + maxX) / 2, building.base, (minZ + maxZ) / 2),
      spread: Math.max(maxX - minX, maxZ - minZ),
    })
    this.place(this.active[this.active.length - 1]!)
  }

  /** The pile alone, at full size, for a building that was already gone when its tile came in. */
  static settled(group: Group, tile: TileData, index: number) {
    group.add(heapOf(tile, index, 1).heap)
  }

  update(dt: number, effects: Effects) {
    for (let i = this.active.length - 1; i >= 0; i--) {
      const c = this.active[i]!
      c.age += dt
      // A tile that unloaded took its drawing with it.
      if (!c.heap.parent?.parent) {
        this.active.splice(i, 1)
        continue
      }
      const p = Math.min(1, c.age / COLLAPSE_SECONDS)
      if (p < 1) {
        // Ease in: the roof gives, then everything follows. A little shudder on the way down.
        c.body.scale.y = Math.max(0.04, 1 - p * p)
        c.body.position.x = Math.sin(c.age * 60) * 0.12 * (1 - p)
        c.heap.scale.y = Math.max(0.01, c.heapHeight * p)
        // Dust rolls out over the collapse, wider than the building.
        while (c.puffs < 3 + Math.floor(p * 9)) {
          c.puffs++
          const angle = c.puffs * 2.4
          const reach = c.spread * 0.45
          effects.dust(new THREE.Vector3(c.centre.x + Math.cos(angle) * reach, c.centre.y + 0.5, c.centre.z + Math.sin(angle) * reach), 3 + c.spread * 0.3)
        }
      } else if (c.body.parent) {
        c.heap.scale.y = c.heapHeight
        c.body.traverse((o) => o instanceof Mesh && o.geometry.dispose())
        c.body.removeFromParent()
      }
      this.step(c, dt)
      if (c.age >= CHUNK_SECONDS + CHUNK_FADE_SECONDS && !c.body.parent) {
        c.chunks.removeFromParent()
        this.active.splice(i, 1)
      }
    }
  }

  private step(c: Collapse, dt: number) {
    const fade = Math.min(1, Math.max(0, (CHUNK_SECONDS + CHUNK_FADE_SECONDS - c.age) / CHUNK_FADE_SECONDS))
    for (const piece of c.pieces) {
      if (piece.resting) continue
      piece.velocity.y -= GRAVITY * dt
      piece.position.addScaledVector(piece.velocity, dt)
      piece.rotation.x += piece.spin.x * dt
      piece.rotation.y += piece.spin.y * dt
      piece.rotation.z += piece.spin.z * dt
      const floor = c.ground(piece.position.x, piece.position.z) + piece.size.y / 2
      if (piece.position.y <= floor) {
        piece.position.y = floor
        // A bounce that loses most of it, and a skid that stops.
        if (Math.abs(piece.velocity.y) > 1.5) {
          piece.velocity.y *= -0.3
          piece.velocity.x *= 0.6
          piece.velocity.z *= 0.6
          piece.spin.multiplyScalar(0.5)
        } else {
          piece.resting = true
        }
      }
    }
    this.place(c, fade)
  }

  private place(c: Collapse, fade = 1) {
    const matrix = mat4.create()
    const quaternion = new THREE.Quaternion()
    c.pieces.forEach((piece, i) => {
      quaternion.setFromEuler(piece.rotation)
      const { position: p, size } = piece
      c.chunks.setMatrixAt(i, mat4.compose(matrix, p.x, p.y, p.z, quaternion.x, quaternion.y, quaternion.z, quaternion.w, size.x * fade, size.y * fade, size.z * fade))
    })
    c.chunks.needsUpdate()
  }
}
