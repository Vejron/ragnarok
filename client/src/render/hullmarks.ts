import { Material, Mesh, Quat, Vec3, ringGeometry, type Geometry, type Node } from '@ragnarok/engine'
import { vehicleDef } from '@/sim/vehicles'
import { nearestSurface, raySurface } from './surface'

/** Marks on one vehicle at most: the oldest goes when another is made. */
const MAX_MARKS = 40
/** A hole's glowing rim cools over this long, seconds. */
const GLOW_SECONDS = 5
/** Marks sit this far off the surface, so they never sink into it. */
const LIFT = 0.012
/** The physics' solids are boxes and drums round the model: a mark finds the model's own surface this near, metres. */
const SNAP = 0.6
/** Holes a hurt vehicle smokes and burns from at most: the newest. */
export const MAX_VENTS = 3

/** A hole smoke and fire come out of: where it is in the world, which way is out of it, and how long since it was made, seconds. */
export interface Vent {
  x: number
  y: number
  z: number
  out: Vec3
  age: number
}

type V = { x: number; y: number; z: number }

interface Mark {
  meshes: Mesh[]
  /** The hole's hot rim, which cools; none for a scuff. */
  glow: Mesh | null
  /** Out of the surface, in the frame of the mesh it is on; none for a scuff, which nothing comes out of. */
  normal: Vec3 | null
  /** Its round hurt a part inside: smoke and fire come out of it first. */
  vital: boolean
  age: number
}

const scorch = new Material({ color: 0x141210, lit: false, blend: 'alpha', opacity: 0.92, depthWrite: false, depthBias: 4, depthBiasSlope: 4 })
const rim = new Material({ color: 0xffa040, emissive: 0xff7a20, lit: false, blend: 'additive', opacity: 1, depthWrite: false, depthBias: 5, depthBiasSlope: 5, fog: false })
const scuff = new Material({ color: 0xb8b4ac, lit: false, blend: 'alpha', opacity: 0.7, depthWrite: false, depthBias: 4, depthBiasSlope: 4 })
/** The marks' own materials: a mark is never laid on another. */
const MARKS = new Set<Material>([scorch, rim, scuff])
/** Marks something else lays on a vehicle (a slit) that no round's mark should be laid on. */
export const notSurface = (material: Material) => void MARKS.add(material)
let shapes: { hole: Geometry; rim: Geometry; scuff: Geometry } | null = null

const FORWARD = new Vec3(0, 0, 1)
/** The outward normal of the face of a box of half extents `half` nearest to `p`. */
function boxNormal(p: Vec3, [hx, hy, hz]: readonly number[]): Vec3 {
  const gaps = [hx! - Math.abs(p.x), hy! - Math.abs(p.y), hz! - Math.abs(p.z)]
  const axis = gaps.indexOf(Math.min(...gaps))
  if (axis === 0) return new Vec3(Math.sign(p.x) || 1, 0, 0)
  if (axis === 1) return new Vec3(0, Math.sign(p.y) || 1, 0)
  return new Vec3(0, 0, Math.sign(p.z) || 1)
}

/** A ray along the round's path starts this far before the physics' point and looks this far past it, metres. */
const BEFORE = 0.8
const PAST = 0.8

/** Where on a vehicle a round struck: the node it is on, the point and normal in that node's frame (and how that frame is scaled), and both in the world. */
export interface Struck {
  parent: Node
  local: Vec3
  normal: Vec3
  unit: number
  world: Vec3
  outward: Vec3
}

/** `v` in the world from the frame of `node`: a point, or with `direction` a direction (unnormalised). */
function toWorld(node: Node, v: V, direction = false): Vec3 {
  const m = node.matrixWorld
  const w = direction ? 0 : 1
  return new Vec3(m[0]! * v.x + m[4]! * v.y + m[8]! * v.z + m[12]! * w, m[1]! * v.x + m[5]! * v.y + m[9]! * v.z + m[13]! * w, m[2]! * v.x + m[6]! * v.y + m[10]! * v.z + m[14]! * w)
}

const unitOf = (v: Vec3) => {
  const length = Math.hypot(v.x, v.y, v.z) || 1
  return new Vec3(v.x / length, v.y / length, v.z / length)
}

/** Whether `node` still hangs, however deep, under `root`. */
function attached(node: Node, root: Node) {
  for (let at: Node | null = node; at; at = at.parent) if (at === root) return true
  return false
}

/**
 * Where on the model under `root` (a `kind`) a round going along `dir` struck it at `point`, both in the
 * vehicle's own frame as the server reported them. The server's point is on its boxes and drums, which
 * are only near the model: the round's path is followed into the model's triangles from a little before
 * the point (`before` it, looking `past` it), and the first it meets is where it went in. Failing that,
 * the model's nearest surface; failing that (no model yet), the hull box's face.
 */
export function locate(kind: number, root: Node, point: V, dir: V, before = BEFORE, past = PAST): Struck {
  root.updateWorldMatrix(true, true)
  const at = toWorld(root, point)
  const way = unitOf(toWorld(root, dir, true))
  const skip = (mesh: Mesh) => MARKS.has(mesh.material)
  const from = { x: at.x - way.x * before, y: at.y - way.y * before, z: at.z - way.z * before }
  const surface = raySurface(root, from, way, before + past, skip) ?? nearestSurface(root, at, SNAP, skip)
  let parent: Node, local: Vec3, normal: Vec3, unit = 1
  if (surface) {
    parent = surface.mesh
    local = new Vec3(...surface.point)
    normal = unitOf(new Vec3(...surface.normal))
    unit = 1 / surface.scale
  } else {
    parent = root
    local = new Vec3(point.x, point.y, point.z)
    normal = boxNormal(local, vehicleDef(kind).halfExtents)
  }
  return { parent, local, normal, unit, world: toWorld(parent, local), outward: unitOf(toWorld(parent, normal, true)) }
}

/**
 * Where rounds have struck armoured vehicles: a dark hole with a rim that glows and cools where one
 * went through, a pale scuff where the plate stopped it. Each lies on the model's own surface where the
 * round's path met it (`locate`), parented to the mesh it is on, so it slopes with the glacis and turns
 * with the turret or the gun. Gone when the vehicle is put back as new.
 */
export class HullMarks {
  private readonly marks = new Map<number, { root: Node; marks: Mark[] }>()
  /** Each vehicle's health last frame: a wreck coming back as new loses its marks. */
  private readonly lastHp = new Map<number, number>()

  /** Marks vehicle `id`, drawn by `root`, where a round `struck` it (`locate`). */
  mark(id: number, root: Node, struck: Struck, pierced: boolean, calibre = pierced ? 1 : 0.6, vital = false) {
    const { parent, local, normal, unit } = struck
    shapes ??= { hole: ringGeometry(0, 0.06, 14), rim: ringGeometry(0.05, 0.11, 18), scuff: ringGeometry(0, 0.07, 10) }
    const facing = new Quat().setFromUnitVectors(FORWARD, normal)
    const lift = LIFT * unit
    const at = new Vec3(local.x + normal.x * lift, local.y + normal.y * lift, local.z + normal.z * lift)
    const place = (geometry: Geometry, material: Material, scale: number) => {
      const mesh = new Mesh(geometry, material)
      mesh.position.copy(at)
      mesh.quaternion.copy(facing)
      mesh.scale.set(scale * unit, scale * unit, scale * unit)
      parent.add(mesh)
      return mesh
    }
    const mark: Mark = pierced
      ? (() => {
          const hole = place(shapes.hole, scorch, calibre)
          const glow = place(shapes.rim, rim, calibre)
          return { meshes: [hole, glow], glow, normal, vital, age: 0 }
        })()
      : { meshes: [place(shapes.scuff, scuff, calibre)], glow: null, normal: null, vital: false, age: 0 }
    let entry = this.marks.get(id)
    if (!entry || entry.root !== root) {
      if (entry) this.clear(id)
      entry = { root, marks: [] }
      this.marks.set(id, entry)
    }
    entry.marks.push(mark)
    if (entry.marks.length > MAX_MARKS) for (const mesh of entry.marks.shift()!.meshes) mesh.removeFromParent()
  }

  /** Cools the glowing rims, and forgets the marks of vehicles put back as new (`placed` by id, with their health). */
  update(dt: number, placed: ReadonlyMap<number, { hp: number }>) {
    for (const [id, entry] of this.marks) {
      const now = placed.get(id)?.hp
      if (now === undefined || (this.lastHp.get(id) === 0 && now > 0)) {
        this.clear(id)
        continue
      }
      for (const mark of entry.marks) {
        mark.age += dt
        if (!mark.glow) continue
        const heat = Math.max(0, 1 - mark.age / GLOW_SECONDS)
        mark.glow.tint[3] = heat * heat
        if (heat === 0) {
          mark.glow.removeFromParent()
          mark.glow = null
        }
      }
    }
    for (const [id, { hp }] of placed) this.lastHp.set(id, hp)
  }

  /**
   * The holes in vehicle `id` its smoke and fire come out of, where they are as it is drawn now:
   * `MAX_VENTS` at most, those whose round hurt something inside first, newest first. A hole on a part
   * that has come off (a turret thrown by its ammunition) is not one.
   */
  vents(id: number): Vent[] {
    const entry = this.marks.get(id)
    const vents: Vent[] = []
    if (!entry) return vents
    for (const vital of [true, false]) {
      for (let i = entry.marks.length - 1; i >= 0 && vents.length < MAX_VENTS; i--) {
        const mark = entry.marks[i]!
        const hole = mark.meshes[0]!
        if (!mark.normal || mark.vital !== vital || !hole.parent || !attached(hole, entry.root)) continue
        hole.updateWorldMatrix(true, false)
        const m = hole.matrixWorld
        vents.push({ x: m[12]!, y: m[13]!, z: m[14]!, out: unitOf(toWorld(hole.parent, mark.normal, true)), age: mark.age })
      }
    }
    return vents
  }

  clear(id?: number) {
    for (const [key, entry] of this.marks) {
      if (id !== undefined && key !== id) continue
      for (const mark of entry.marks) for (const mesh of mark.meshes) mesh.removeFromParent()
      this.marks.delete(key)
    }
  }
}
