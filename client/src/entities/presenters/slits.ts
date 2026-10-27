import * as THREE from 'three'
import { Material, Mesh, Quat, quadGeometry, type Geometry, type Node } from '@ragnarok/engine'
import { locate, notSurface } from '@/render/hullmarks'
import { vehicleDef, type Slit } from '@/sim/vehicles'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { Vehicles } from './vehicles'

/** The gap sits this far off the plate, so it never sinks into it; the glint a little further. */
const LIFT = 0.01
const GLINT_LIFT = 0.016
/** How far before and past a slit's box its plate on the model is looked for along the way it looks out, metres. */
const REACH_BEFORE = 1
const REACH_PAST = 3
/** A model that has not loaded its triangles yet is tried again for this many frames before the slits go on its boxes. */
const PATIENCE = 120

const gap = new Material({ color: 0x050505, lit: false, blend: 'alpha', opacity: 0.95, depthWrite: false, depthBias: 4, depthBiasSlope: 4 })
const shine = new Material({ color: 0xffe2a8, emissive: 0xffd890, lit: false, blend: 'additive', opacity: 1, depthWrite: false, depthBias: 5, depthBiasSlope: 5 })
notSurface(gap)
notSurface(shine)
let quad: Geometry | null = null

interface Drawn {
  root: Node
  /** Each slit's glint and the seat whose occupant makes it, or none until they are placed. */
  glints: { mesh: Mesh; seat: number | null }[] | null
  tries: number
}

/** Direction `v` in the frame of `node`, in the world, unit length. */
const toWorldDirection = (node: Node, [x, y, z]: readonly number[]) => {
  const m = node.matrixWorld
  return new THREE.Vector3(m[0]! * x! + m[4]! * y! + m[8]! * z!, m[1]! * x! + m[5]! * y! + m[9]! * z!, m[2]! * x! + m[6]! * y! + m[10]! * z!).normalize()
}

/** `v` turned by `yaw` about the vertical, as a turret turns it (yaw 0 faces -Z, + turns left). */
const yawed = ([x, y, z]: readonly number[], yaw: number): [number, number, number] => {
  const sin = Math.sin(yaw), cos = Math.cos(yaw)
  return [x! * cos + z! * sin, y!, -x! * sin + z! * cos]
}

/**
 * The vision slits on an armoured vehicle (`VehicleDef::slits`), which a round goes straight through to
 * whoever looks out of them: a dark gap on the plate where each is, and behind it a faint glint while
 * that seat is taken, so a marksman can see there is someone there. Each lies on the model's own surface,
 * found as a round's mark is (`locate`), and goes with the mesh it is on: it turns with the turret.
 */
export class Slits implements Presenter<Frame> {
  private readonly drawn = new Map<number, Drawn>()

  constructor(private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.kind !== undefined && vehicleDef(entity.kind.kind).slits.length > 0
  }

  update(id: number, _: unknown, frame: Frame) {
    const placed = this.vehicles.placed.get(id)
    const model = this.vehicles.model(id)
    if (!placed || !model) return
    let drawn = this.drawn.get(id)
    if (!drawn || drawn.root !== model.root) {
      drawn = { root: model.root, glints: null, tries: 0 }
      this.drawn.set(id, drawn)
    }
    drawn.glints ??= this.place(placed.kind, model.root, placed.turret?.yaw ?? 0, drawn.tries++ >= PATIENCE)
    if (!drawn.glints) return
    // Someone behind the slit: an eye's glint, never quite still, that now and then catches the light.
    const t = frame.now / 1000
    drawn.glints.forEach(({ mesh, seat }, i) => {
      const there = seat !== null && placed.hp > 0 && (placed.seats[seat] ?? 0) !== 0
      const flicker = 0.55 + 0.2 * Math.sin(t * 2.3 + id + i * 1.9) + 0.25 * Math.max(0, Math.sin(t * 0.7 + id * 0.37 + i)) ** 12
      mesh.tint[3] = there ? flicker : 0
    })
  }

  remove(id: number) {
    this.drawn.delete(id)
  }

  /**
   * Lays a kind's slits on its model, with its turret turned to `yaw`; null while the model has no
   * triangles to lay them on, unless `anyway`.
   */
  private place(kind: number, root: Node, yaw: number, anyway: boolean) {
    const slits = vehicleDef(kind).slits
    const found = slits.map((slit) => [slit, this.find(kind, root, slit, yaw)] as const)
    if (!anyway && found.some(([, struck]) => struck.parent === root)) return null
    quad ??= quadGeometry(1, 1)
    return found.map(([slit, struck]) => {
      const { parent, local, normal, unit } = struck
      // Its long side level: across the plate, square to the vehicle's up as it is in the mesh's frame.
      parent.updateWorldMatrix(true, false)
      const toLocal = new THREE.Matrix4().fromArray(parent.matrixWorld).invert()
      const up = new THREE.Vector3(root.matrixWorld[4]!, root.matrixWorld[5]!, root.matrixWorld[6]!).transformDirection(toLocal)
      const z = new THREE.Vector3(normal.x, normal.y, normal.z)
      const x = new THREE.Vector3().crossVectors(up, z)
      if (x.lengthSq() < 1e-6) x.set(1, 0, 0)
      x.normalize()
      const y = new THREE.Vector3().crossVectors(z, x)
      const basis = new THREE.Matrix4().makeBasis(x, y, z)
      const facing = new Quat().setFromRotationMatrix(basis.elements)
      const [hx, hy, hz] = slit.half
      // Drawn where a round going straight in would cross the plate: a strip as long as the slope stretches it.
      const along = toWorldDirection(root, slit.turret ? yawed(slit.out, yaw) : slit.out)
      const square = Math.max(0.25, Math.abs(along.dot(new THREE.Vector3(struck.outward.x, struck.outward.y, struck.outward.z))))
      const width = 2 * (slit.out[0] !== 0 ? hz : hx)
      const height = (2 * (slit.out[1] !== 0 ? hz : hy)) / square
      const lay = (material: Material, lift: number, w: number, h: number) => {
        const mesh = new Mesh(quad!, material)
        mesh.position.set(local.x + normal.x * lift * unit, local.y + normal.y * lift * unit, local.z + normal.z * lift * unit)
        mesh.quaternion.copy(facing)
        mesh.scale.set(w * unit, h * unit, unit)
        parent.add(mesh)
        return mesh
      }
      lay(gap, LIFT, width, height)
      const mesh = lay(shine, GLINT_LIFT, Math.min(width * 0.5, 0.07), height * 0.5)
      mesh.tint[3] = 0
      return { mesh, seat: slit.seat }
    })
  }

  /** Where `slit` is on the model: the plate a round coming straight in at it would meet. */
  private find(kind: number, root: Node, slit: Slit, yaw: number) {
    const turn = (v: readonly number[]) => (slit.turret ? yawed(v, yaw) : ([v[0]!, v[1]!, v[2]!] as [number, number, number]))
    const { centre, pivot, out } = slit
    const [x, y, z] = turn([centre[0] - pivot[0], centre[1] - pivot[1], centre[2] - pivot[2]])
    const at = slit.turret ? { x: x + pivot[0], y: y + pivot[1], z: z + pivot[2] } : { x: centre[0], y: centre[1], z: centre[2] }
    const [dx, dy, dz] = turn(out)
    // The physics' boxes stand off the model, a long way in front of a shallow glacis: look well past.
    return locate(kind, root, at, { x: -dx, y: -dy, z: -dz }, REACH_BEFORE, REACH_PAST)
  }
}
