import { Group, Mesh } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import { DAMAGE_STAGES } from '@/sim/consts'
import type { TileData } from '@/world'
import { shellOf, type Shell } from '@/sim/carve'
import type { Effects } from './effects'
import { Fracture, shardGeometry } from './fracture'
import { random } from './rubble'

/** Most pieces thrown from a building at once, and how long they lie where they land, seconds. */
const MAX_FALLING = 32
const FALLEN_SECONDS = 10
const GRAVITY = 9.8
/** A building about to collapse sheds dust about this often, seconds, and only this near the camera, metres. */
const SHED_SECONDS = 0.45
const SHED_RANGE = 160

/** A point `(x, y, z)`, metres. */
type Mark = readonly [number, number, number]

interface Hurt {
  stage: number
  /** The cells a blast took out of it, by `Shell` number: gone for good. */
  removed: Set<number>
  /** Damage this player's own rounds have done to cells that still stand: they are drawn pushed in. */
  dents: Map<number, number>
}

/** A bite that just happened: what to throw, and from where. */
interface Burst {
  blasted: number[]
  fell: number[]
  point: Mark | null
}

/** The most cells in one piece thrown from a blast, and in one chunk that falls whole. */
const THROWN_CELLS = 320
const FALLEN_CELLS = 2560
/** How hard a broken piece is thrown from the blast, m/s. */
const THROW = 3

/** A shard that broke away, on its way down or lying where it fell. */
interface Falling {
  mesh: Mesh
  velocity: THREE.Vector3
  spin: THREE.Vector3
  age: number
  resting: boolean
}

interface Drawn {
  group: Group
  /** The building's walls and roof as cells, once it is hurt, and what is drawn of them. */
  fracture: Fracture | null
  /** What is drawn of the cells still standing, a mesh a brick (`Fracture.bricksNear`). */
  bricks: Map<number, Mesh>
  /** The state the bricks in `dirty` are to be redrawn for at the next `update`: a barrage redraws a building once a frame, not once a hit. */
  stale: Hurt | null
  dirty: Set<number>
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

const pieceMaterial = toonMaterial({ vertexColors: true, side: 'double' })
// Double-sided: through a hole you see the inside of the far wall and the underside of the roof.
const hurtMaterial = toonMaterial({ vertexColors: true, side: 'double' })

/** A wall triangle: the three corners and its unit normal (arbitrary side), metres. */
interface Wall {
  triangle: THREE.Triangle
  normal: THREE.Vector3
  area: number
}

const cubeWalls = new WeakMap<Shell, Wall[]>()

/** The sides of building `index`'s cubes (`Shell.faces`, wound outwards), or its own wall triangles when it has no cubes. */
function wallsOf(tile: TileData, index: number): Wall[] {
  const building = tile.buildings[index]
  if (!building) return []
  const shell = shellOf(tile, index)
  if (shell) {
    let walls = cubeWalls.get(shell)
    if (!walls) {
      const faces = shell.faces(new Set())
      walls = []
      for (let o = 0; o < faces.length; o += 9) {
        const [a, b, c] = [0, 3, 6].map((k) => new THREE.Vector3(faces[o + k]! / 100, faces[o + k + 1]! / 100, faces[o + k + 2]! / 100)) as [THREE.Vector3, THREE.Vector3, THREE.Vector3]
        const triangle = new THREE.Triangle(a, b, c)
        const normal = triangle.getNormal(new THREE.Vector3())
        // Sides only: the tops and bottoms of the cubes are roof and floor.
        if (Math.abs(normal.y) > 0.5) continue
        walls.push({ triangle, normal, area: triangle.getArea() })
      }
      cubeWalls.set(shell, walls)
    }
    return walls
  }
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
 * What a hurt building looks like beyond being whole. Just before it falls it sheds dust. Once cells have been blasted out of it, or this player's rounds have
 * dented some, its walls and roof are redrawn from their cells (`Fracture`) without those that are
 * gone, and the pieces fly and fall. Pieces live in `root` in world coordinates; what a
 * building has suffered is kept, so a tile that streams back in is hurt as it was. Buildings drawn
 * from their cells are to be left out of the tile's own drawing (`fractured`).
 */
export class BuildingDamage {
  readonly root = new Group()
  private readonly hurt = new Map<number, Map<number, Hurt>>()
  private readonly drawn = new Map<number, Map<number, Drawn>>()
  private readonly loaded = new Map<number, Loaded>()
  private readonly puffs: Puff[] = []
  private readonly rand = random(99)

  /**
   * Building `index` of tile `key` is now at damage `stage`; one about to fall sheds dust.
   */
  hurtBuilding(key: number, index: number, stage: number) {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>() }
    buildings.set(index, state)
    state.stage = Math.max(state.stage, Math.min(stage, DAMAGE_STAGES))
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, true, null)
  }

  /**
   * A round of `damage` from this player struck cell `cell` of building `index` of tile `key`, which
   * still stands: it is drawn pushed in by what it has taken. Nothing is replicated: the server
   * breaks the cell out when its health is gone, and then it is simply removed. True when this is
   * the first time the building is drawn as pieces.
   */
  dent(key: number, index: number, cell: number, damage: number): boolean {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>() }
    buildings.set(index, state)
    const wasWhole = state.removed.size === 0 && state.dents.size === 0
    if (state.removed.has(cell)) return false
    state.dents.set(cell, (state.dents.get(cell) ?? 0) + damage)
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, false, null, [cell])
    return wasWhole
  }

  /**
   * A blast took the cells `blasted` out of building `index` of tile `key`, and left `fell` with
   * nothing under them. `live` is false for the catch-up on joining. True when this is the first time
   * the building is drawn as pieces, so the tile has to stop drawing it whole.
   */
  carve(key: number, index: number, blasted: readonly number[], fell: readonly number[], point: Mark | null, live: boolean): boolean {
    const buildings = this.hurt.get(key) ?? new Map<number, Hurt>()
    this.hurt.set(key, buildings)
    const state = buildings.get(index) ?? { stage: 0, removed: new Set<number>(), dents: new Map<number, number>() }
    buildings.set(index, state)
    const wasWhole = state.removed.size === 0 && state.dents.size === 0
    const fresh = { blasted: blasted.filter((c) => !state.removed.has(c)), fell: fell.filter((c) => !state.removed.has(c)), point }
    for (const cell of [...blasted, ...fell]) state.removed.add(cell)
    const loaded = this.loaded.get(key)
    if (loaded) this.draw(key, index, loaded, state, live, fresh, [...fresh.blasted, ...fresh.fell])
    for (const cell of state.removed) state.dents.delete(cell)
    return wasWhole && state.removed.size > 0
  }

  /** The cells of each hurt building of tile `key` that are gone, by building index. */
  carved(key: number): Map<number, ReadonlySet<number>> {
    const out = new Map<number, ReadonlySet<number>>()
    for (const [index, state] of this.hurt.get(key) ?? []) if (state.removed.size > 0) out.set(index, state.removed)
    return out
  }

  /** The buildings of tile `key` that are drawn here from their cells and so are not to be drawn whole. */
  fractured(key: number): number[] {
    const out: number[] = []
    for (const [index, state] of this.hurt.get(key) ?? []) if (state.removed.size > 0 || state.dents.size > 0) out.push(index)
    return out
  }

  /** Tile `key` is drawn now: show what its buildings have suffered so far. */
  attach(key: number, tile: TileData, salt = 0) {
    const loaded = { tile, salt }
    this.loaded.set(key, loaded)
    for (const [index, state] of this.hurt.get(key) ?? []) this.draw(key, index, loaded, state, false, null)
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
  shown(key: number, index: number): { cells: number } {
    return { cells: this.hurt.get(key)?.get(index)?.removed.size ?? 0 }
  }

  /** Pieces fall and rest, and buildings on the point of collapse shed dust when `near` (the camera) is close enough to see it. */
  update(dt: number, effects: Effects, near: THREE.Vector3) {
    for (const puff of this.puffs.splice(0)) effects.dust(puff.at, puff.size)
    for (const [key, buildings] of this.drawn) {
      const loaded = this.loaded.get(key)
      if (!loaded) continue
      for (const [index, drawn] of buildings) {
        if (drawn.stale) this.redraw(drawn, drawn.stale)
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
  private draw(key: number, index: number, loaded: Loaded, state: Hurt, live: boolean, burst: Burst | null, cells: readonly number[] = []) {
    const buildings = this.drawn.get(key) ?? new Map<number, Drawn>()
    this.drawn.set(key, buildings)
    let drawn = buildings.get(index)
    const walls = wallsOf(loaded.tile, index)
    if (walls.length === 0) return
    if (!drawn) {
      drawn = { group: new Group(), fracture: null, bricks: new Map(), stale: null, dirty: new Set(), falling: [], shed: SHED_SECONDS }
      buildings.set(index, drawn)
      this.root.add(drawn.group)
    }
    if (state.removed.size > 0 || state.dents.size > 0) this.shatter(drawn, key, index, loaded, state, live, burst, cells)
  }

  /** Redraws the building without the cells that are gone and with the ones that were shot pushed in. */
  private shatter(drawn: Drawn, key: number, index: number, loaded: Loaded, state: Hurt, live: boolean, burst: Burst | null, cells: readonly number[]) {
    const building = loaded.tile.buildings[index]!
    // Cubes: through a hole you see the inner faces of the walls round it, so no room needs drawing.
    drawn.fracture ??= new Fracture(loaded.tile, index, loaded.salt)
    // The first time it is drawn now, so the tile's own drawing of the building can stop; after that a live hit waits for the frame's end.
    for (const brick of drawn.fracture.bricksNear(cells)) drawn.dirty.add(brick)
    if (drawn.bricks.size > 0 && live) drawn.stale = state
    else this.redraw(drawn, state)
    const shell = drawn.fracture.shell
    if (!live || !burst || !shell) return
    this.throwCells(drawn, drawn.fracture, shell, building.base, burst.blasted, burst.point, false)
    this.throwCells(drawn, drawn.fracture, shell, building.base, burst.fell, burst.point, true)
  }

  /** Draws the bricks of `drawn` that changed, as `state` has the cells; all of them the first time. */
  private redraw(drawn: Drawn, state: Hurt) {
    const fracture = drawn.fracture
    if (!fracture) return
    drawn.stale = null
    const first = drawn.bricks.size === 0
    const todo = first ? fracture.allBricks() : drawn.dirty
    for (const key of todo) {
      const built = fracture.buildBrick(key, state.removed, state.dents)
      const old = drawn.bricks.get(key)
      if (!built) {
        if (old) {
          old.geometry.dispose()
          old.removeFromParent()
          drawn.bricks.delete(key)
        }
        continue
      }
      if (old) {
        old.geometry.dispose()
        old.geometry = shardGeometry(built.positions, built.colors)
        continue
      }
      const mesh = new Mesh(shardGeometry(built.positions, built.colors), hurtMaterial)
      mesh.castShadow = true
      mesh.receiveShadow = true
      drawn.group.add(mesh)
      drawn.bricks.set(key, mesh)
    }
    drawn.dirty.clear()
  }

  /**
   * Throws the wall and roof in `cells` out of the building as chunks: blasted ones fly away from
   * `point`, ones that were left hanging (`falls`) drop where they were.
   */
  private throwCells(drawn: Drawn, fracture: Fracture, shell: NonNullable<Fracture['shell']>, floor: number, cells: readonly number[], point: Mark | null, falls: boolean) {
    for (const group of shell.clusters(cells, falls ? FALLEN_CELLS : THROWN_CELLS)) {
      const chunk = fracture.chunk(new Set(group))
      if (!chunk) continue
      this.puffs.push({ at: chunk.pivot.clone(), size: falls ? 3 + this.rand() * 2 : 1.4 + this.rand() * 1.4 })
      if (drawn.falling.length >= MAX_FALLING) continue
      const mesh = new Mesh(shardGeometry(chunk.positions, chunk.colors), pieceMaterial)
      mesh.position.copy(chunk.pivot)
      mesh.castShadow = true
      drawn.group.add(mesh)
      const velocity = new THREE.Vector3()
      if (!falls && point) {
        velocity.set(chunk.pivot.x - point[0], chunk.pivot.y - point[1], chunk.pivot.z - point[2])
        if (velocity.lengthSq() < 1e-4) velocity.set(0, 1, 0)
        velocity.normalize().multiplyScalar(THROW * (0.5 + this.rand())).y += 1 + this.rand() * 2
      }
      const tumble = falls ? 0.6 : 5
      drawn.falling.push({
        mesh,
        velocity,
        spin: new THREE.Vector3((this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble, (this.rand() - 0.5) * tumble),
        age: 0,
        resting: mesh.position.y <= floor + 0.15,
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
    for (const mesh of [...drawn.bricks.values(), ...drawn.falling.map((f) => f.mesh)]) mesh.geometry.dispose()
    this.root.remove(drawn.group)
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
