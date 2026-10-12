import * as THREE from 'three'
import { DAMAGE_STAGES } from '@/sim/consts'
import type { TileData } from '@/world'
import type { Effects } from './effects'
import { Fracture, shardGeometry } from './fracture'
import { random } from './rubble'

/** Most scorch marks a building keeps; the oldest go first. */
const MAX_MARKS = 12
/** Most pieces thrown from a building at once, and how long they lie where they land, seconds. */
const MAX_FALLING = 14
const FALLEN_SECONDS = 10
const GRAVITY = 9.8
/** How far a mark floats off the wall so it does not z-fight with it, metres. */
const LIFT = 0.1
/** A building about to collapse sheds dust about this often, seconds, and only this near the camera, metres. */
const SHED_SECONDS = 0.45
const SHED_RANGE = 160

/** One mark at `(x, y, z)`, metres: a blast that landed there. */
type Mark = readonly [number, number, number]

interface Hurt {
  stage: number
  marks: Mark[]
}

/** A scorch decal, and the shard of wall it is on (once the wall is cut into shards), `-1` before. */
interface Scorch {
  mesh: THREE.Mesh
  at: THREE.Vector3
  shard: number
}

/** A shard that broke away, on its way down or lying where it fell. */
interface Falling {
  mesh: THREE.Mesh
  velocity: THREE.Vector3
  spin: THREE.Vector3
  age: number
  resting: boolean
}

interface Drawn {
  group: THREE.Group
  /** Scorch decals, oldest first. */
  scorches: Scorch[]
  /** The building's walls cut into shards, once it is hurt, and what is drawn of them. */
  fracture: Fracture | null
  body: THREE.Mesh | null
  core: THREE.Mesh | null
  /** Shards that have broken away for good. */
  gone: Set<number>
  falling: Falling[]
  shed: number
}

interface Loaded {
  tile: TileData
  salt: number
}

/** Dust to be raised where a shard just came away, drained by `update`. */
interface Puff {
  at: THREE.Vector3
  size: number
}

const planeGeometry = new THREE.PlaneGeometry(1, 1)

function paint(size: number, draw: (g: CanvasRenderingContext2D, size: number) => void): THREE.CanvasTexture | null {
  if (typeof document === 'undefined') return null
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = size
  const g = canvas.getContext('2d')
  if (!g) return null
  draw(g, size)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

/** A ragged black blotch, darkest at the middle, with soot thrown off it. */
function scorchTexture() {
  return paint(128, (g, size) => {
    const c = size / 2
    const gradient = g.createRadialGradient(c, c, 0, c, c, c)
    gradient.addColorStop(0, 'rgba(10,8,6,0.92)')
    gradient.addColorStop(0.4, 'rgba(14,11,8,0.7)')
    gradient.addColorStop(1, 'rgba(20,16,12,0)')
    g.fillStyle = gradient
    g.fillRect(0, 0, size, size)
    const rand = random(12345)
    for (let i = 0; i < 28; i++) {
      const angle = rand() * Math.PI * 2
      const reach = c * (0.25 + rand() * 0.55)
      const r = 3 + rand() * 9
      const x = c + Math.cos(angle) * reach
      const y = c + Math.sin(angle) * reach
      const spot = g.createRadialGradient(x, y, 0, x, y, r)
      spot.addColorStop(0, 'rgba(12,10,8,0.55)')
      spot.addColorStop(1, 'rgba(12,10,8,0)')
      g.fillStyle = spot
      g.fillRect(x - r, y - r, r * 2, r * 2)
    }
  })
}

let scorchMaterial: THREE.MeshBasicMaterial | null = null
const bodyMaterial = new THREE.MeshToonMaterial({ vertexColors: true })
const pieceMaterial = new THREE.MeshToonMaterial({ vertexColors: true, side: THREE.DoubleSide })
const coreMaterial = new THREE.MeshBasicMaterial({ color: 0x17120f, side: THREE.DoubleSide })

function decalMaterial(map: THREE.Texture | null, fallback: number) {
  return new THREE.MeshBasicMaterial({
    map,
    color: map ? 0xffffff : fallback,
    transparent: true,
    opacity: map ? 1 : 0.5,
    depthWrite: false,
    side: THREE.DoubleSide,
    polygonOffset: true,
    polygonOffsetFactor: -2,
    polygonOffsetUnits: -2,
  })
}

const scorchLook = () => (scorchMaterial ??= decalMaterial(scorchTexture(), 0x1a1510))

/** A wall triangle: the three corners and its unit normal (arbitrary side), metres. */
interface Wall {
  triangle: THREE.Triangle
  normal: THREE.Vector3
  area: number
}

function wallsOf(tile: TileData, index: number): Wall[] {
  const building = tile.buildings[index]
  if (!building) return []
  const { vertices, triangles } = tile.collision
  const walls: Wall[] = []
  for (let t = building.first; t < building.first + building.walls; t++) {
    const [a, b, c] = [0, 1, 2].map((k) => new THREE.Vector3().fromArray(vertices, triangles[t * 3 + k]! * 3)) as [THREE.Vector3, THREE.Vector3, THREE.Vector3]
    // The collision mesh is wound inwards (the renderer reverses it), so the outward normal is the
    // one of the reversed triangle.
    const triangle = new THREE.Triangle(a, c, b)
    const area = triangle.getArea()
    if (area < 0.01) continue
    walls.push({ triangle, normal: triangle.getNormal(new THREE.Vector3()), area })
  }
  return walls
}

/**
 * What a hurt building looks like beyond being whole. A scorch is left where each shell landed. Once
 * a building is hurt at all its walls are redrawn as shards that slip and tilt out of line as it
 * weakens, and break away where shells hit; just before it falls it sheds dust. Decals and shards
 * live in `root` in world coordinates; what a building has suffered is kept, so a tile that streams
 * back in is hurt as it was. Hurt buildings are to be left out of the tile's own drawing (`fractured`).
 */
export class BuildingDamage {
  readonly root = new THREE.Group()
  private readonly hurt = new Map<number, Map<number, Hurt>>()
  private readonly drawn = new Map<number, Map<number, Drawn>>()
  private readonly loaded = new Map<number, Loaded>()
  private readonly puffs: Puff[] = []
  private readonly rand = random(99)

  /**
   * Building `index` of tile `key` is now at damage `stage`, and a blast may have marked it at
   * `mark`. True when this is the first time the building is hurt enough to be redrawn as shards,
   * so the tile has to stop drawing it whole.
   */
  hurtBuilding(key: number, index: number, stage: number, mark: Mark | null): boolean {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, marks: [] }
    buildings.set(index, state)
    const before = state.stage
    state.stage = Math.max(state.stage, Math.min(stage, DAMAGE_STAGES))
    if (mark) {
      state.marks.push(mark)
      if (state.marks.length > MAX_MARKS) state.marks.shift()
    }
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, mark, true)
    return before === 0 && state.stage > 0
  }

  /** The buildings of tile `key` that are drawn here as shards and so are not to be drawn whole. */
  fractured(key: number): number[] {
    const out: number[] = []
    for (const [index, state] of this.hurt.get(key) ?? []) if (state.stage > 0) out.push(index)
    return out
  }

  /** Tile `key` is drawn now: show what its buildings have suffered so far. */
  attach(key: number, tile: TileData, salt = 0) {
    const loaded = { tile, salt }
    this.loaded.set(key, loaded)
    for (const [index, state] of this.hurt.get(key) ?? []) this.draw(key, index, loaded, state, null, false)
  }

  /** Tile `key` was unloaded. What was done to its buildings is remembered. */
  detach(key: number) {
    this.loaded.delete(key)
    for (const drawn of this.drawn.get(key)?.values() ?? []) this.release(drawn)
    this.drawn.delete(key)
  }

  /** Building `index` of tile `key` has collapsed: nothing is left to be hurt. */
  fell(key: number, index: number) {
    this.hurt.get(key)?.delete(index)
    const drawn = this.drawn.get(key)?.get(index)
    if (drawn) this.release(drawn)
    this.drawn.get(key)?.delete(index)
  }

  /** What building `index` of tile `key` shows, for tests. */
  shown(key: number, index: number): { scorches: number; shards: number; missing: number } {
    const drawn = this.drawn.get(key)?.get(index)
    if (!drawn) return { scorches: 0, shards: 0, missing: 0 }
    return {
      scorches: drawn.scorches.filter((s) => s.mesh.visible).length,
      shards: drawn.fracture?.count ?? 0,
      missing: drawn.gone.size,
    }
  }

  /** Pieces fall and rest, and buildings on the point of collapse shed dust when `near` (the camera) is close enough to see it. */
  update(dt: number, effects: Effects, near: THREE.Vector3) {
    for (const puff of this.puffs.splice(0)) effects.dust(puff.at, puff.size)
    for (const [key, buildings] of this.drawn) {
      const loaded = this.loaded.get(key)
      if (!loaded) continue
      for (const [index, drawn] of buildings) {
        this.fall(drawn, loaded.tile.buildings[index]?.base ?? 0, dt)
        if ((this.hurt.get(key)?.get(index)?.stage ?? 0) < DAMAGE_STAGES) continue
        drawn.shed -= dt
        if (drawn.shed > 0) continue
        drawn.shed = SHED_SECONDS * (0.6 + this.rand() * 0.8)
        const walls = wallsOf(loaded.tile, index)
        const wall = walls[Math.floor(this.rand() * walls.length)]
        if (!wall) continue
        const at = this.pointOn(wall)
        if (at.distanceToSquared(near) > SHED_RANGE * SHED_RANGE) continue
        effects.dust(at.addScaledVector(wall.normal, 0.3), 1.2 + this.rand() * 1.4)
      }
    }
  }

  dispose() {
    for (const buildings of this.drawn.values()) for (const drawn of buildings.values()) this.release(drawn)
    this.root.removeFromParent()
    this.hurt.clear()
    this.drawn.clear()
    this.loaded.clear()
  }

  /** Brings tile `key`'s building `index` up to `state`; `live` when it is happening now, to be seen. */
  private draw(key: number, index: number, loaded: Loaded, state: Hurt, fresh: Mark | null, live: boolean) {
    const buildings = this.drawn.get(key) ?? new Map<number, Drawn>()
    this.drawn.set(key, buildings)
    let drawn = buildings.get(index)
    const walls = wallsOf(loaded.tile, index)
    if (walls.length === 0) return
    if (!drawn) {
      drawn = { group: new THREE.Group(), scorches: [], fracture: null, body: null, core: null, gone: new Set(), falling: [], shed: SHED_SECONDS }
      buildings.set(index, drawn)
      this.root.add(drawn.group)
      // A tile coming back in draws every mark it remembers.
      for (const mark of state.marks) if (mark !== fresh) this.scorch(drawn, walls, mark)
    }
    if (fresh) this.scorch(drawn, walls, fresh)
    if (state.stage > 0) this.shatter(drawn, key, index, loaded, state, live)
  }

  /** Redraws the building as shards at its stage, dropping those that have broken away. */
  private shatter(drawn: Drawn, key: number, index: number, loaded: Loaded, state: Hurt, live: boolean) {
    const building = loaded.tile.buildings[index]!
    if (!drawn.fracture) {
      drawn.fracture = new Fracture(loaded.tile, index, key * 1_000_003 + index, loaded.salt)
      drawn.core = new THREE.Mesh(shardGeometry(drawn.fracture.core(), new Float32Array(drawn.fracture.core().length)), coreMaterial)
      drawn.group.add(drawn.core)
      for (const scorch of drawn.scorches) scorch.shard = drawn.fracture.nearest(scorch.at).shard
    }
    const built = drawn.fracture.build(state.stage, state.marks, drawn.gone)
    if (!drawn.body) {
      drawn.body = new THREE.Mesh(shardGeometry(built.positions, built.colors), bodyMaterial)
      drawn.body.castShadow = true
      drawn.body.receiveShadow = true
      drawn.group.add(drawn.body)
    } else {
      drawn.body.geometry.dispose()
      drawn.body.geometry = shardGeometry(built.positions, built.colors)
    }
    for (const scorch of drawn.scorches) scorch.mesh.visible = !drawn.gone.has(scorch.shard)
    if (!live) return
    for (const id of built.fresh) {
      const piece = drawn.fracture.piece(id)
      this.puffs.push({ at: piece.pivot.clone().addScaledVector(piece.normal, 0.5), size: 1.5 + this.rand() * 1.5 })
      if (drawn.falling.length >= MAX_FALLING) continue
      const mesh = new THREE.Mesh(shardGeometry(piece.positions, piece.colors), pieceMaterial)
      mesh.position.copy(piece.pivot)
      mesh.castShadow = true
      drawn.group.add(mesh)
      const away = 1 + this.rand() * 2.5
      drawn.falling.push({
        mesh,
        velocity: new THREE.Vector3(piece.normal.x * away, 0.5 + this.rand() * 2, piece.normal.z * away),
        spin: new THREE.Vector3((this.rand() - 0.5) * 5, (this.rand() - 0.5) * 5, (this.rand() - 0.5) * 5),
        age: 0,
        resting: mesh.position.y <= building.base + 0.15,
      })
    }
  }

  /** Moves a building's flung shards: they drop, tumble and lie on the ground a while, then go. */
  private fall(drawn: Drawn, floor: number, dt: number) {
    for (let i = drawn.falling.length - 1; i >= 0; i--) {
      const piece = drawn.falling[i]!
      piece.age += dt
      if (piece.age > FALLEN_SECONDS) {
        piece.mesh.geometry.dispose()
        piece.mesh.removeFromParent()
        drawn.falling.splice(i, 1)
        continue
      }
      if (piece.resting) continue
      piece.velocity.y -= GRAVITY * dt
      piece.mesh.position.addScaledVector(piece.velocity, dt)
      piece.mesh.rotation.x += piece.spin.x * dt
      piece.mesh.rotation.y += piece.spin.y * dt
      piece.mesh.rotation.z += piece.spin.z * dt
      if (piece.mesh.position.y <= floor + 0.15) {
        piece.mesh.position.y = floor + 0.15
        piece.resting = true
      }
    }
  }

  private release(drawn: Drawn) {
    for (const mesh of [drawn.body, drawn.core, ...drawn.falling.map((f) => f.mesh)]) mesh?.geometry.dispose()
    this.root.remove(drawn.group)
  }

  private scorch(drawn: Drawn, walls: Wall[], mark: Mark) {
    const point = new THREE.Vector3(...mark)
    // Where the blast met the house: the nearest spot on its walls, seen from the blast.
    let best: { at: THREE.Vector3; wall: Wall; distance: number } | null = null
    const closest = new THREE.Vector3()
    for (const wall of walls) {
      wall.triangle.closestPointToPoint(point, closest)
      const distance = closest.distanceTo(point)
      if (!best || distance < best.distance) best = { at: closest.clone(), wall, distance }
    }
    if (!best) return
    const size = THREE.MathUtils.clamp(3.4 - best.distance * 0.4, 1.2, 3.4)
    const mesh = this.decal(scorchLook(), best.at, best.wall.normal.clone(), size, this.rand() * Math.PI * 2)
    mesh.userData.decal = true
    const shard = drawn.fracture ? drawn.fracture.nearest(best.at).shard : -1
    drawn.scorches.push({ mesh, at: best.at, shard })
    drawn.group.add(mesh)
    if (drawn.scorches.length > MAX_MARKS) drawn.group.remove(drawn.scorches.shift()!.mesh)
  }

  /** A flat square `size` metres across lying on a wall at `at`, facing out along `normal`. */
  private decal(material: THREE.Material, at: THREE.Vector3, normal: THREE.Vector3, size: number, roll: number) {
    const mesh = new THREE.Mesh(planeGeometry, material)
    mesh.position.copy(at).addScaledVector(normal, LIFT)
    mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), normal)
    mesh.rotateZ(roll)
    mesh.scale.setScalar(size)
    mesh.userData.sharedGeometry = true
    return mesh
  }

  private pointOn(wall: Wall) {
    return pointInTriangle(wall.triangle, this.rand(), this.rand())
  }
}

/** A point of `triangle`, uniform for uniform `u` and `v`. */
function pointInTriangle(triangle: THREE.Triangle, u: number, v: number) {
  if (u + v > 1) [u, v] = [1 - u, 1 - v]
  return triangle.a.clone().addScaledVector(triangle.b.clone().sub(triangle.a), u).addScaledVector(triangle.c.clone().sub(triangle.a), v)
}
