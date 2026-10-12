import * as THREE from 'three'
import type { TileData } from '@/world'
import { random } from './rubble'
import { buildingColors } from './world'

/** The longest edge of a wall piece before it is cut in two, metres. */
const CELL = 2.4
/** About how much wall one shard covers, square metres. */
const SHARD_AREA = 8
/** How far behind the wall the dark inside of the building sits, metres. */
const INSET = 0.4
/** How much of the wall's brightness the inside keeps. */
const INSIDE = 0.6
/** The share of shards knocked out of true, and the share broken out entirely, at each damage stage. */
const DISPLACED = [0, 0.3, 0.55, 0.8]
const GONE = [0, 0, 0.1, 0.28]
/** A blast breaks out the shard it hit once the building is at least this hurt. */
const BREAKS_AT_STAGE = 2
/** Shards this close to where a shell landed fall sooner, metres. */
const BLAST_REACH = 5

/** One ragged piece of wall: a few of the small triangles the walls are cut into. */
interface Shard {
  pivot: THREE.Vector3
  normal: THREE.Vector3
  /** Which pieces go first: 0 first, 1 last. */
  rank: number
  /** The in-plane line it tilts about. */
  axis: THREE.Vector3
  /** Which way and how far it tilts at full damage, radians. */
  spin: number
  /** How far it sinks into the wall and how far it sags, metres, at full damage. */
  slide: number
  sag: number
  shade: number
}

export interface Fractured {
  /** Walls and roof, triangle soup, with a colour per vertex. */
  positions: Float32Array
  colors: Float32Array
  /** Shards that came away on this build and were not gone before. */
  fresh: number[]
}

export interface Piece {
  /** Triangle soup around `pivot`. */
  positions: Float32Array
  colors: Float32Array
  pivot: THREE.Vector3
  normal: THREE.Vector3
}

/**
 * A building's walls cut into shards, to be redrawn broken as it weakens: shards tilt and sink out
 * of line so seams open, show the dark inside of the house, and fall away where shells hit. Which
 * shards are cut and in what order is fixed by the building's identity alone, so every client
 * and a late joiner see the same house. The server's collision shape is unchanged: this is a picture.
 */
export class Fracture {
  private readonly shards: Shard[] = []
  /** The small wall triangles, nine numbers each, wound to face out. */
  private readonly small: number[] = []
  /** The shard each small triangle belongs to. */
  private readonly owner: number[] = []
  private readonly roof: number[] = []
  private readonly wall: THREE.Color
  private readonly roofColor: THREE.Color
  private readonly middle = new THREE.Vector3()

  constructor(tile: TileData, index: number, seed: number, salt: number) {
    const building = tile.buildings[index]!
    const { vertices, triangles } = tile.collision
    const colors = buildingColors(tile, index, salt)
    this.wall = new THREE.Color(colors.wall)
    this.roofColor = new THREE.Color(colors.roof)
    const rand = random(seed)
    const corner = (t: number, k: number) => new THREE.Vector3().fromArray(vertices, triangles[t * 3 + k]! * 3)
    // The collision mesh is wound inwards and drawn reversed (a, c, b), so that is the order kept.
    for (let t = building.first; t < building.first + building.walls; t++) this.cut(corner(t, 0), corner(t, 2), corner(t, 1))
    for (let t = building.first + building.walls; t < building.first + building.walls + building.roof; t++) {
      this.roof.push(...corner(t, 0).toArray(), ...corner(t, 2).toArray(), ...corner(t, 1).toArray())
    }
    for (const p of building.footprint) this.middle.x += p[0]
    for (const p of building.footprint) this.middle.z += p[1]
    this.middle.divideScalar(Math.max(1, building.footprint.length))
    this.carve(rand)
  }

  /** The number of shards. */
  get count() {
    return this.shards.length
  }

  /** The shard whose wall lies nearest `point`, and how far that is, metres. */
  nearest(point: THREE.Vector3): { shard: number; distance: number } {
    let best = { shard: -1, distance: Infinity }
    const centre = new THREE.Vector3()
    for (let i = 0; i < this.small.length; i += 9) {
      centre.set(
        (this.small[i]! + this.small[i + 3]! + this.small[i + 6]!) / 3,
        (this.small[i + 1]! + this.small[i + 4]! + this.small[i + 7]!) / 3,
        (this.small[i + 2]! + this.small[i + 5]! + this.small[i + 8]!) / 3,
      )
      const distance = centre.distanceTo(point)
      if (distance < best.distance) best = { shard: this.owner[i / 9]!, distance }
    }
    return best
  }

  /**
   * The building as it looks at damage `stage` after the blasts at `marks`, leaving out the shards
   * in `gone` and adding to it any that break away now.
   */
  build(stage: number, marks: readonly (readonly [number, number, number])[], gone: Set<number>): Fractured {
    const level = Math.min(stage, GONE.length - 1)
    const fresh: number[] = []
    const rank = this.ranks(level, marks)
    this.shards.forEach((_, i) => {
      if (!gone.has(i) && rank[i]! < GONE[level]!) {
        gone.add(i)
        fresh.push(i)
      }
    })
    const positions: number[] = []
    const colors: number[] = []
    const tint = new THREE.Color()
    const p = new THREE.Vector3()
    const q = new THREE.Quaternion()
    const severity = level / (DISPLACED.length - 1)
    for (let t = 0; t < this.owner.length; t++) {
      const id = this.owner[t]!
      if (gone.has(id)) continue
      const shard = this.shards[id]!
      const loose = rank[id]! < DISPLACED[level]!
      if (loose) q.setFromAxisAngle(shard.axis, shard.spin * (0.25 + 0.75 * severity))
      tint.copy(this.wall).multiplyScalar(loose ? shard.shade : 1)
      for (let k = 0; k < 3; k++) {
        p.fromArray(this.small, t * 9 + k * 3)
        if (loose) {
          p.sub(shard.pivot).applyQuaternion(q).add(shard.pivot)
          p.addScaledVector(shard.normal, -shard.slide * severity)
          p.y -= shard.sag * severity
        }
        positions.push(p.x, p.y, p.z)
        colors.push(tint.r, tint.g, tint.b)
      }
    }
    for (let i = 0; i < this.roof.length; i += 3) {
      positions.push(this.roof[i]!, this.roof[i + 1]!, this.roof[i + 2]!)
      colors.push(this.roofColor.r, this.roofColor.g, this.roofColor.b)
    }
    return { positions: Float32Array.from(positions), colors: Float32Array.from(colors), fresh }
  }

  /** What is inside the walls: the same walls drawn a little way in, dark, for the seams and holes to show. */
  core(): Float32Array {
    const out = new Float32Array(this.small.length)
    const p = new THREE.Vector3()
    const toward = new THREE.Vector3()
    for (let i = 0; i < this.small.length; i += 3) {
      p.fromArray(this.small, i)
      toward.set(this.middle.x - p.x, 0, this.middle.z - p.z)
      const reach = toward.length()
      if (reach > 1e-3) p.addScaledVector(toward.divideScalar(reach), Math.min(INSET, reach))
      p.toArray(out, i)
    }
    return out
  }

  /** The inside's colour per vertex of `core()`: the wall's own, a little dimmer, so a hole shows a room and not a void. */
  coreColors(): Float32Array {
    const out = new Float32Array(this.small.length)
    for (let i = 0; i < out.length; i += 3) {
      out[i] = this.wall.r * INSIDE
      out[i + 1] = this.wall.g * INSIDE
      out[i + 2] = this.wall.b * INSIDE
    }
    return out
  }

  /** Shard `id` on its own, to be thrown, with its centre at the origin. */
  piece(id: number): Piece {
    const shard = this.shards[id]!
    const positions: number[] = []
    const colors: number[] = []
    for (let t = 0; t < this.owner.length; t++) {
      if (this.owner[t] !== id) continue
      for (let k = 0; k < 3; k++) {
        positions.push(this.small[t * 9 + k * 3]! - shard.pivot.x, this.small[t * 9 + k * 3 + 1]! - shard.pivot.y, this.small[t * 9 + k * 3 + 2]! - shard.pivot.z)
        colors.push(this.wall.r * shard.shade, this.wall.g * shard.shade, this.wall.b * shard.shade)
      }
    }
    return { positions: Float32Array.from(positions), colors: Float32Array.from(colors), pivot: shard.pivot.clone(), normal: shard.normal.clone() }
  }

  /** Each shard's place in the order of breaking, after the blasts: the ones a shell hit go first. */
  private ranks(level: number, marks: readonly (readonly [number, number, number])[]): number[] {
    const out = this.shards.map((shard) => shard.rank)
    const at = new THREE.Vector3()
    for (const mark of marks) {
      at.set(...mark)
      const hit = this.nearest(at)
      this.shards.forEach((shard, i) => {
        const factor = 0.4 + 0.6 * Math.min(1, shard.pivot.distanceTo(at) / BLAST_REACH)
        out[i] = Math.min(out[i]!, shard.rank * factor)
      })
      if (level >= BREAKS_AT_STAGE && hit.shard >= 0 && hit.distance < BLAST_REACH) out[hit.shard] = 0
    }
    return out
  }

  /** Cuts one wall triangle into pieces no longer than `CELL`, keeping its winding. */
  private cut(a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3) {
    const stack: THREE.Vector3[][] = [[a, b, c]]
    while (stack.length > 0 && this.small.length < 9 * 4000) {
      const tri = stack.pop()!
      let longest = 0
      let length = 0
      for (let i = 0; i < 3; i++) {
        const l = tri[i]!.distanceTo(tri[(i + 1) % 3]!)
        if (l > length) [longest, length] = [i, l]
      }
      const [p, q, r] = [tri[longest]!, tri[(longest + 1) % 3]!, tri[(longest + 2) % 3]!]
      if (length <= CELL) {
        if (new THREE.Triangle(p, q, r).getArea() > 1e-4) this.small.push(...p.toArray(), ...q.toArray(), ...r.toArray())
        continue
      }
      const m = p.clone().add(q).multiplyScalar(0.5)
      stack.push([p, m, r], [m, q, r])
    }
  }

  /** Groups the small triangles into ragged shards around randomly chosen seeds. */
  private carve(rand: () => number) {
    const triangles = this.small.length / 9
    if (triangles === 0) return
    const centres: THREE.Vector3[] = []
    let area = 0
    for (let t = 0; t < triangles; t++) {
      const [a, b, c] = [0, 1, 2].map((k) => new THREE.Vector3().fromArray(this.small, t * 9 + k * 3)) as [THREE.Vector3, THREE.Vector3, THREE.Vector3]
      centres.push(a.clone().add(b).add(c).divideScalar(3))
      area += new THREE.Triangle(a, b, c).getArea()
    }
    const wanted = Math.min(triangles, Math.max(4, Math.min(48, Math.round(area / SHARD_AREA))))
    const seeds = Array.from({ length: wanted }, () => ({
      at: centres[Math.floor(rand() * triangles)]!,
      // Shards come tall and thin or wide and flat, and some are bigger than others.
      weight: rand() * 1.4,
      stretch: 0.5 + rand() * 1.3,
    }))
    const members: number[][] = seeds.map(() => [])
    for (let t = 0; t < triangles; t++) {
      const c = centres[t]!
      let best = 0
      let nearest = Infinity
      seeds.forEach((seed, i) => {
        const dy = (c.y - seed.at.y) * seed.stretch
        const d = Math.sqrt((c.x - seed.at.x) ** 2 + (c.z - seed.at.z) ** 2 + dy * dy) - seed.weight
        if (d < nearest) [best, nearest] = [i, d]
      })
      members[best]!.push(t)
    }
    const up = new THREE.Vector3(0, 1, 0)
    this.owner.length = triangles
    for (const list of members) {
      if (list.length === 0) continue
      const id = this.shards.length
      const pivot = new THREE.Vector3()
      const normal = new THREE.Vector3()
      for (const t of list) {
        this.owner[t] = id
        pivot.add(centres[t]!)
        const [a, b, c] = [0, 1, 2].map((k) => new THREE.Vector3().fromArray(this.small, t * 9 + k * 3)) as [THREE.Vector3, THREE.Vector3, THREE.Vector3]
        const tri = new THREE.Triangle(a, b, c)
        normal.addScaledVector(tri.getNormal(new THREE.Vector3()), tri.getArea())
      }
      pivot.divideScalar(list.length)
      if (normal.lengthSq() < 1e-9) normal.set(0, 0, 1)
      normal.normalize()
      const side = new THREE.Vector3().crossVectors(normal, up)
      if (side.lengthSq() < 1e-6) side.set(1, 0, 0)
      side.normalize()
      const rise = new THREE.Vector3().crossVectors(normal, side).normalize()
      const angle = rand() * Math.PI * 2
      this.shards.push({
        pivot,
        normal,
        rank: rand(),
        axis: side.multiplyScalar(Math.cos(angle)).addScaledVector(rise, Math.sin(angle)).normalize(),
        spin: (0.12 + rand() * 0.2) * (rand() < 0.5 ? -1 : 1),
        slide: 0.04 + rand() * 0.14,
        sag: rand() * 0.14,
        shade: 0.78 + rand() * 0.2,
      })
    }
  }
}

/** A flat-shaded geometry from triangle soup with a colour per vertex. */
export function shardGeometry(positions: Float32Array, colors: Float32Array): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  geometry.computeVertexNormals()
  geometry.computeBoundingSphere()
  return geometry
}
