import { Geometry, Group, Mesh, type Color, type Material, type Node } from '@ragnarok/engine'
import * as THREE from 'three'
import { toonMaterial } from './toon'
import { DAMAGE_FRONT, DAMAGE_LEFT, DAMAGE_REAR, DAMAGE_RIGHT, DAMAGE_WHEELS, WHEEL_GONE } from '@/net/codec'
import type { Debris } from './debris'

/**
 * How a car shows the crash damage the server reports: dents pushed into the body where it was
 * hit, lights that break, a cracked windscreen, paint that dulls, bumpers and doors that come off,
 * and wheels that lean, wobble and finally roll away. Everything follows the vehicle's `damage`
 * (see `DAMAGE_*`), so a player who joins late sees a wreck as battered as everyone else does.
 */

/** Side damage at which the lights on that end break, and the windscreen cracks. */
export const LIGHTS_BREAK = 90
const GLASS_CRACKS = 120
/** Side damage at which a bumper, and a door, comes off. */
const BUMPER_FALLS = 160
const DOOR_FALLS = 200
/** How deep a side's full damage dents it, metres, and how far round the hit a dent reaches. */
const MAX_DENT = 0.3
const DENT_REACH = 0.9
/** How far a wheel leans at full damage, radians. Rolling, the lean shows as a wobble. */
const MAX_CAMBER = 0.32
/** How long a damage increase waits for the crash event that says where it hit, ms. */
const IMPACT_WAIT_MS = 150
/** How long a crash event's point stays the place to dent, ms. */
const IMPACT_KEEP_MS = 1500

const BROKEN = toonMaterial({ color: 0x262626, name: 'broken' })
const CRACKED = toonMaterial({ color: 0xd9e3e8, name: 'cracked' })
const DULL = new THREE.Color(0x4a4744)
const CHARRED = new THREE.Color(0x1a1817)

const WHEELS = ['wheel_FL', 'wheel_FR', 'wheel_RL', 'wheel_RR'] as const

interface BodyPart {
  mesh: Mesh
  /** The geometry every car of the kind shares; put back on a repair. */
  shared: Geometry
  /** This car's own copy, made on the first dent. */
  own: Geometry | null
  toRoot: THREE.Matrix4
  fromRoot: THREE.Matrix4
  /** What it was drawn with before a light broke or the glass cracked. */
  material: Material
}

/** A piece of the body that comes off, as a box in the car's frame. */
interface Panel {
  name: string
  side: number
  threshold: number
  inside: (p: THREE.Vector3) => boolean
  /** Which way it flies off, in the car's frame. */
  away: THREE.Vector3
}

/** Which side of a car a point in its frame is on, by how far along each half extent it lies. */
export function sideOf(point: THREE.Vector3, half: readonly [number, number, number]): number {
  const nx = point.x / half[0]
  const nz = point.z / half[2]
  if (Math.abs(nz) >= Math.abs(nx)) return nz < 0 ? DAMAGE_FRONT : DAMAGE_REAR
  return nx < 0 ? DAMAGE_LEFT : DAMAGE_RIGHT
}

export class CarWear {
  private readonly parts: BodyPart[] = []
  private readonly wheels: (Node | null)[]
  private readonly wheelRadius: number
  private readonly panels: Panel[]
  private readonly applied = [0, 0, 0, 0]
  private readonly waiting: { side: number; amount: number; since: number }[] = []
  private readonly impacts: { point: THREE.Vector3; at: number }[] = []
  private readonly gone = new Set<string>()
  private readonly wheelGone = [false, false, false, false]
  private readonly baseColor: Color
  private hurt = false

  constructor(
    private readonly root: Node,
    model: Node,
    private readonly half: readonly [number, number, number],
    wheelRadius: number,
    private readonly paint: Material,
  ) {
    this.wheelRadius = wheelRadius
    this.baseColor = paint.color.clone()
    model.updateMatrixWorld(true)
    const toModel = new THREE.Matrix4().fromArray(model.matrixWorld).invert()
    model.getObjectByName('body')?.traverse((node) => {
      if (!(node instanceof Mesh)) return
      // The model sits at the root's origin, so its frame is the car's.
      const toRoot = new THREE.Matrix4().multiplyMatrices(toModel, new THREE.Matrix4().fromArray(node.matrixWorld))
      this.parts.push({ mesh: node, shared: node.geometry, own: null, toRoot, fromRoot: toRoot.clone().invert(), material: node.material })
    })
    this.wheels = WHEELS.map((name) => model.getObjectByName(name) ?? null)
    const [hx, hy, hz] = half
    this.panels = [
      { name: 'bumper_F', side: DAMAGE_FRONT, threshold: BUMPER_FALLS, inside: (p) => p.z < -hz + 0.3 && p.y < hy * 0.2, away: new THREE.Vector3(0, 0.4, -1) },
      { name: 'bumper_R', side: DAMAGE_REAR, threshold: BUMPER_FALLS, inside: (p) => p.z > hz - 0.3 && p.y < hy * 0.2, away: new THREE.Vector3(0, 0.4, 1) },
      { name: 'door_L', side: DAMAGE_LEFT, threshold: DOOR_FALLS, inside: (p) => p.x < -hx + 0.18 && Math.abs(p.z) < hz * 0.4 && p.y > -hy, away: new THREE.Vector3(-1, 0.4, 0) },
      { name: 'door_R', side: DAMAGE_RIGHT, threshold: DOOR_FALLS, inside: (p) => p.x > hx - 0.18 && Math.abs(p.z) < hz * 0.4 && p.y > -hy, away: new THREE.Vector3(1, 0.4, 0) },
    ]
  }

  /** A crash event at `world`: the next dent on that side goes there. */
  noteImpact(world: THREE.Vector3, now = performance.now()) {
    this.root.updateMatrixWorld()
    const point = this.root.worldToLocal(world.clone())
    this.impacts.push({ point, at: now })
    if (this.impacts.length > 8) this.impacts.shift()
    // A dent that arrived before its event goes where the event says.
    const side = sideOf(point, this.half)
    for (let i = this.waiting.length - 1; i >= 0; i--) {
      if (this.waiting[i]!.side !== side) continue
      this.dent(point, this.waiting[i]!.amount)
      this.waiting.splice(i, 1)
    }
  }

  /**
   * Shows `damage` (as the snapshot carries it) and `hp`. Pieces that come off are thrown into
   * `debris`, moving with the car's `velocity`.
   */
  update(damage: readonly number[], hp: number, debris: Debris | null, velocity: THREE.Vector3, now = performance.now()) {
    const total = damage.slice(0, 4).reduce((sum, value) => sum + value, 0) + damage.slice(DAMAGE_WHEELS).reduce((sum, value) => sum + value, 0)
    if (total === 0) {
      if (this.hurt) this.repair()
      this.tint(hp, 0)
      return
    }
    this.hurt = true
    for (let side = 0; side < 4; side++) {
      const value = damage[side] ?? 0
      if (value > this.applied[side]!) {
        this.waiting.push({ side, amount: (value - this.applied[side]!) / 255, since: now })
        this.applied[side] = value
      }
    }
    // Waited long enough for a crash event: dent where it last hit that side, or mid-side.
    for (let i = this.waiting.length - 1; i >= 0; i--) {
      const { side, amount, since } = this.waiting[i]!
      const recent = this.impacts.filter((impact) => now - impact.at < IMPACT_KEEP_MS && sideOf(impact.point, this.half) === side).pop()
      if (!recent && now - since < IMPACT_WAIT_MS) continue
      this.dent(recent?.point ?? this.middleOf(side), amount)
      this.waiting.splice(i, 1)
    }

    const front = damage[DAMAGE_FRONT] ?? 0
    const rear = damage[DAMAGE_REAR] ?? 0
    for (const part of this.parts) {
      const name = part.material.name
      const broken = (name === 'lamp' && front >= LIGHTS_BREAK) || (name === 'red' && rear >= LIGHTS_BREAK)
      const cracked = name === 'glass' && front >= GLASS_CRACKS
      part.mesh.material = broken ? BROKEN : cracked ? CRACKED : part.material
    }
    for (const panel of this.panels) {
      if (!this.gone.has(panel.name) && (damage[panel.side] ?? 0) >= panel.threshold) this.detach(panel, debris, velocity)
    }
    this.wheels.forEach((wheel, index) => {
      if (!wheel) return
      const value = damage[DAMAGE_WHEELS + index] ?? 0
      // Left wheels lean out to the left, right ones to the right.
      wheel.rotation.z = (index % 2 === 0 ? 1 : -1) * MAX_CAMBER * (value / WHEEL_GONE)
      if (value >= WHEEL_GONE && !this.wheelGone[index]) this.lose(index, debris, velocity)
    })
    this.tint(hp, total / (8 * 255))
  }

  private middleOf(side: number): THREE.Vector3 {
    const [hx, , hz] = this.half
    return side === DAMAGE_FRONT ? new THREE.Vector3(0, 0, -hz) : side === DAMAGE_REAR ? new THREE.Vector3(0, 0, hz) : new THREE.Vector3(side === DAMAGE_LEFT ? -hx : hx, 0, 0)
  }

  /** This car's own copy of a body part's geometry, to bend or cut. */
  private own(part: BodyPart): Geometry {
    if (!part.own) {
      part.own = part.shared.clone()
      if (!part.own.indices) part.own.indices = Uint32Array.from({ length: part.own.vertexCount }, (_, i) => i)
      part.mesh.geometry = part.own
    }
    return part.own
  }

  /** Pushes the body in around `point` (the car's frame) by `amount` of a full dent. */
  private dent(point: THREE.Vector3, amount: number) {
    const depth = MAX_DENT * Math.min(1, amount)
    // Inwards, level: towards the middle of the car from where it was hit.
    const inward = new THREE.Vector3(-point.x, 0, -point.z)
    if (inward.lengthSq() < 1e-6) return
    inward.normalize()
    const v = new THREE.Vector3()
    for (const part of this.parts) {
      const geometry = this.own(part)
      const positions = geometry.positions
      let moved = false
      for (let i = 0; i < geometry.vertexCount; i++) {
        v.fromArray(positions, i * 3).applyMatrix4(part.toRoot)
        const distance = v.distanceTo(point)
        if (distance >= DENT_REACH) continue
        const falloff = (1 - distance / DENT_REACH) ** 2
        // A little crumple, the same for a vertex every time, so shared corners stay together.
        const crumple = 0.75 + 0.5 * Math.abs(Math.sin(v.x * 12.9898 + v.y * 78.233 + v.z * 37.719))
        v.addScaledVector(inward, depth * falloff * crumple).applyMatrix4(part.fromRoot)
        positions[i * 3] = v.x
        positions[i * 3 + 1] = v.y
        positions[i * 3 + 2] = v.z
        moved = true
      }
      if (moved) {
        // New normals re-send the whole geometry, bounds included.
        geometry.computeNormals()
        geometry.computeBounds()
      }
    }
  }

  /** Cuts `panel` out of the body and throws it. */
  private detach(panel: Panel, debris: Debris | null, velocity: THREE.Vector3) {
    this.gone.add(panel.name)
    const piece = new Group()
    const owned: (Geometry | Material)[] = []
    const a = new THREE.Vector3()
    const b = new THREE.Vector3()
    const c = new THREE.Vector3()
    const centroid = new THREE.Vector3()
    for (const part of this.parts) {
      const geometry = this.own(part)
      const positions = geometry.positions
      const index = geometry.indices!
      const kept: number[] = []
      const cut: number[] = []
      for (let t = 0; t + 2 < index.length; t += 3) {
        const [i, j, k] = [index[t]!, index[t + 1]!, index[t + 2]!]
        a.fromArray(positions, i * 3).applyMatrix4(part.toRoot)
        b.fromArray(positions, j * 3).applyMatrix4(part.toRoot)
        c.fromArray(positions, k * 3).applyMatrix4(part.toRoot)
        centroid.copy(a).add(b).add(c).divideScalar(3)
        if (panel.inside(centroid)) cut.push(a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z)
        else kept.push(i, j, k)
      }
      if (cut.length === 0) continue
      geometry.indices = Uint32Array.from(kept)
      geometry.needsUpdate()
      const loose = new Geometry({ positions: Float32Array.from(cut) })
      loose.computeNormals()
      owned.push(loose)
      // The car's own paint goes when the car does; the piece keeps a copy.
      let material = part.mesh.material
      if (material === this.paint) {
        material = this.paint.clone()
        owned.push(material)
      }
      piece.add(new Mesh(loose, material))
    }
    if (piece.children.length === 0 || !debris) {
      for (const thing of owned) thing.dispose()
      return
    }
    this.throwPiece(piece, panel.away, debris, velocity, 0.08, owned)
  }

  /** Hides wheel `index` and sends a copy of it rolling off. */
  private lose(index: number, debris: Debris | null, velocity: THREE.Vector3) {
    this.wheelGone[index] = true
    const wheel = this.wheels[index]!
    wheel.visible = false
    if (!debris) return
    const loose = wheel.clone()
    loose.visible = true
    const holder = new Group()
    wheel.updateWorldMatrix(true, false)
    // In the car's frame, so `throwPiece` can place it with the car.
    holder.add(loose)
    const at = this.root.worldToLocal(wheel.getWorldPosition(new THREE.Vector3()))
    loose.position.set(at.x, at.y, at.z)
    loose.quaternion.copy(this.root.quaternion).invert().multiply(wheel.getWorldQuaternion(new THREE.Quaternion()))
    const away = new THREE.Vector3(index % 2 === 0 ? -1 : 1, 0.6, 0)
    this.throwPiece(holder, away, debris, velocity, this.wheelRadius, [])
  }

  /** Places `piece` (built in the car's frame) where the car is and throws it towards `away`. */
  private throwPiece(piece: Node, away: THREE.Vector3, debris: Debris, velocity: THREE.Vector3, rest: number, owned: (Geometry | Material)[]) {
    this.root.updateMatrixWorld()
    // Recentre on its own middle, so it tumbles about itself.
    const middle = centreOf(piece)
    for (const child of piece.children) child.position.sub(middle)
    const holder = new Group()
    holder.add(piece)
    const at = middle.applyMatrix4(new THREE.Matrix4().fromArray(this.root.matrixWorld))
    holder.position.set(at.x, at.y, at.z)
    holder.quaternion.copy(this.root.quaternion)
    const throwDir = away.clone().applyQuaternion(this.root.quaternion)
    const fling = velocity.clone().multiplyScalar(0.7).addScaledVector(throwDir, 2.5 + Math.random() * 2)
    const spin = new THREE.Vector3(Math.random() - 0.5, Math.random() - 0.5, Math.random() - 0.5).multiplyScalar(10)
    debris.throw(holder, fling, spin, rest, 9, owned)
  }

  /** Paint dulls with damage; a wreck is charred. */
  private tint(hp: number, worn: number) {
    this.paint.color.copy(this.baseColor).lerp(DULL, Math.min(0.5, worn * 0.9))
    if (hp === 0) this.paint.color.lerp(CHARRED, 0.85)
    this.paint.needsUpdate()
  }

  /** Back to a fresh car, as a respawn brings. */
  private repair() {
    this.hurt = false
    this.applied.fill(0)
    this.waiting.length = 0
    this.impacts.length = 0
    this.gone.clear()
    for (const part of this.parts) {
      part.own?.dispose()
      part.own = null
      part.mesh.geometry = part.shared
      part.mesh.material = part.material
    }
    this.wheels.forEach((wheel, index) => {
      if (!wheel) return
      wheel.visible = true
      wheel.rotation.z = 0
      this.wheelGone[index] = false
    })
  }

  /** How many vertices the dents have moved, and what has come off: for tests. */
  get state() {
    return { gone: [...this.gone], wheelsGone: [...this.wheelGone], owned: this.parts.filter((part) => part.own).length }
  }

  dispose() {
    for (const part of this.parts) part.own?.dispose()
  }
}

/** The middle of the box around every vertex under `root`, in `root`'s own frame. */
function centreOf(root: Node): THREE.Vector3 {
  root.updateMatrixWorld(true)
  const min = new THREE.Vector3(Infinity, Infinity, Infinity)
  const max = new THREE.Vector3(-Infinity, -Infinity, -Infinity)
  const v = new THREE.Vector3()
  const m = new THREE.Matrix4()
  root.traverse((node) => {
    if (!(node instanceof Mesh)) return
    m.fromArray(node.matrixWorld)
    const p = node.geometry.positions
    for (let i = 0; i < p.length; i += 3) {
      v.fromArray(p, i).applyMatrix4(m)
      min.min(v)
      max.max(v)
    }
  })
  return min.add(max).multiplyScalar(0.5)
}
