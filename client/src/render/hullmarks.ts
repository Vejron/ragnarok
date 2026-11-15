import { InstancedMesh, Mesh, SCAR_DENT, SCAR_GOUGE, SCAR_HOLE, SCAR_HOT, SCAR_SOOT, Scars, Vec3, type Material, type Node } from '@ragnarok/engine'
import { vehicleDef } from '@/sim/vehicles'
import { nearestSurface, raySurface } from './surface'

/** Marks kept on one vehicle for its smoke and fire to come out of: the oldest is forgotten. */
const MAX_MARKS = 40
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

/** What a round left: a hole through, a scuff where the plate stopped it, a glance it tore along, or a blast's scorch beside it. */
export type MarkKind = 'hole' | 'scuff' | 'glance' | 'blast'

interface Mark {
  /** The node it is on, where it is there, and out of the surface there; none for anything but a hole, which nothing comes out of. */
  parent: Node
  local: Vec3
  normal: Vec3 | null
  /** Its round hurt a part inside: smoke and fire come out of it first. */
  vital: boolean
  age: number
}

/**
 * What each kind of mark is drawn as (scene/scars.ts), for a round of calibre 1 on a vehicle of size 1:
 * its reach and its dent's depth, metres, and its `SCAR_*` style. A bullet's (`light`) is a chip.
 */
const LOOKS: Record<MarkKind | 'chip', { radius: number; depth: number; style: number }> = {
  hole: { radius: 0.3, depth: 0.12, style: SCAR_DENT | SCAR_HOLE | SCAR_SOOT | SCAR_HOT },
  scuff: { radius: 0.34, depth: 0.16, style: SCAR_DENT | SCAR_SOOT | SCAR_HOT },
  glance: { radius: 0.22, depth: 0.02, style: SCAR_GOUGE | SCAR_SOOT | SCAR_HOT },
  blast: { radius: 0.9, depth: 0, style: SCAR_SOOT },
  chip: { radius: 0.05, depth: 0.004, style: SCAR_GOUGE | SCAR_DENT },
}

/** Marks something else lays on a vehicle (a slit) that no round's mark should be laid on. */
const MARKS = new Set<Material>()
export const notSurface = (material: Material) => void MARKS.add(material)

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

/**
 * Where on a vehicle a round struck: the node it is on, the point and normal in that node's frame (and
 * how that frame is scaled: metres × `unit` is its own units), the way the round went there (unit), and
 * the point and normal in the world.
 */
export interface Struck {
  parent: Node
  local: Vec3
  normal: Vec3
  way: Vec3
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

/** Direction `v` in the world in the frame of `node`, whose axes are square to one another (unnormalised). */
function toLocalDir(node: Node, v: V): Vec3 {
  const m = node.matrixWorld
  const axis = (i: number) => (m[i]! * v.x + m[i + 1]! * v.y + m[i + 2]! * v.z) / (m[i]! ** 2 + m[i + 1]! ** 2 + m[i + 2]! ** 2 || 1)
  return new Vec3(axis(0), axis(4), axis(8))
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
  return { parent, local, normal, way: unitOf(toLocalDir(parent, way)), unit, world: toWorld(parent, local), outward: unitOf(toWorld(parent, normal, true)) }
}

/**
 * Where rounds have struck armoured vehicles (HULL-DAMAGE.md): each mark is a scar on the mesh it struck
 * (scene/scars.ts), drawn by that mesh's own shader, so it slopes with the glacis and turns with the turret
 * or a Titan's arm. A hole is pressed in, black at its middle, ringed with blued steel and glowing as it
 * cools; a scuff is a scorched dent; a glance a gouge to bare metal; a blast beside it soot. A bullet's is
 * a chip in the paint, kept apart so it never wipes out a shell's. Gone when the vehicle is put back as new.
 */
export class HullMarks {
  private readonly marks = new Map<number, { root: Node; marks: Mark[]; slots: Map<Mesh, number> }>()
  /** Each vehicle's health last frame: a wreck coming back as new loses its marks. */
  private readonly lastHp = new Map<number, number>()

  /** Draws the marks as `scars`: the scene's, or a list of its own in a test. */
  constructor(private readonly scars: Scars = new Scars()) {}

  /**
   * Marks vehicle `id`, drawn by `root`, where a round `struck` it (`locate`): a `kind` of mark, as big as
   * `calibre` (1 an ordinary shell's) on a vehicle of `size` (1 a tank, larger for a Titan). `light` is a
   * bullet's, `vital` a hole whose round hurt a part inside.
   */
  mark(id: number, root: Node, struck: Struck, kind: MarkKind, calibre = 1, vital = false, size = 1, light = false, now = performance.now() / 1000) {
    const { parent, local, normal, way, unit } = struck
    let entry = this.marks.get(id)
    if (!entry || entry.root !== root) {
      if (entry) this.clear(id)
      entry = { root, marks: [], slots: new Map() }
      this.marks.set(id, entry)
    }
    if (parent instanceof Mesh && !(parent instanceof InstancedMesh)) {
      let slot = entry.slots.get(parent) ?? 0
      if (!slot && (slot = this.scars.alloc())) {
        entry.slots.set(parent, slot)
        parent.scars = slot
      }
      const look = LOOKS[light && kind !== 'hole' && kind !== 'blast' ? 'chip' : kind]
      const reach = look.radius * calibre * size * unit
      // A blast's soot has no way it came; it spreads round the nearest face.
      const dir = kind === 'blast' ? new Vec3(-normal.x, -normal.y, -normal.z) : way
      this.scars.add(slot, { x: local.x, y: local.y, z: local.z, radius: reach, dx: dir.x, dy: dir.y, dz: dir.z, style: look.style, depth: look.depth * calibre * size * unit, birth: now }, light)
    }
    if (kind === 'blast') return
    entry.marks.push({ parent, local, normal: kind === 'hole' ? normal : null, vital: kind === 'hole' && vital, age: 0 })
    if (entry.marks.length > MAX_MARKS) entry.marks.shift()
  }

  /** Ages the marks, wears each vehicle's parts with its hurt, and forgets the marks of vehicles put back as new (`placed` by id). */
  update(dt: number, placed: ReadonlyMap<number, { hp: number; kind?: number }>) {
    for (const [id, entry] of this.marks) {
      const now = placed.get(id)
      if (now === undefined || (this.lastHp.get(id) === 0 && now.hp > 0)) {
        this.clear(id)
        continue
      }
      for (const mark of entry.marks) mark.age += dt
      // A battered vehicle is grimed all over, a burnt-out one most of all.
      const full = now.kind === undefined ? 0 : vehicleDef(now.kind).hp
      const grime = now.hp === 0 ? 1 : full > 0 ? Math.min(1, Math.max(0, 1 - now.hp / full)) * 0.8 : 0
      for (const slot of entry.slots.values()) this.scars.setGrime(slot, grime)
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
        if (!mark.normal || mark.vital !== vital || !attached(mark.parent, entry.root)) continue
        mark.parent.updateWorldMatrix(true, false)
        const at = toWorld(mark.parent, mark.local)
        vents.push({ x: at.x, y: at.y, z: at.z, out: unitOf(toWorld(mark.parent, mark.normal, true)), age: mark.age })
      }
    }
    return vents
  }

  clear(id?: number) {
    for (const [key, entry] of this.marks) {
      if (id !== undefined && key !== id) continue
      for (const [mesh, slot] of entry.slots) {
        if (mesh.scars === slot) mesh.scars = 0
        this.scars.release(slot)
      }
      this.marks.delete(key)
    }
  }
}
