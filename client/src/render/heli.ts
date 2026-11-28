import { Mesh, type Material, type Node, Matrix4, Vector3 } from '@ragnarok/engine'
import { parseToonModel } from './models'
import { toonMaterial } from './toon'
import { HELI_GUN_LENGTH } from '@/sim/consts'
import type { Turret } from '@/entities/units'
import { RotorModel, type RotorLook } from './rotor'
import { assetUrl } from '@/assets'

export const HELI_MODEL_URL = assetUrl('models/heli.glb')

const ROTORS: RotorLook = { speed: 28, tail: 1.6 }
const RECOIL_STROKE = 0.08
const RECOIL_SECONDS = 0.06
/** Paint colours handed out by vehicle id, so neighbouring helicopters are told apart. */
const PAINTS = [0x6b7a4f, 0x8a8456, 0x5d6b52, 0x7a7048] as const
const GLASS_OPACITY = 0.45
let asset: Node | null = null

/**
 * Parses `heli.glb` (built by `tools/vehicles/build_heli.py`) for every `HeliModel` to share. Its
 * materials become flat toon ones to match the rest of the scene; `paint` is replaced per helicopter.
 */
export async function parseHeliModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => {
    if (source.name !== 'glass') return null
    return toonMaterial({ color: source.color, name: source.name, blend: 'alpha', opacity: GLASS_OPACITY, depthWrite: false })
  }, { merge: true })
}

export async function loadHeliModel(url = HELI_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseHeliModel(await response.arrayBuffer())
}

/**
 * A Mi-24 in silhouette: a double cockpit with a pilot's head behind and above the gunner's, stub
 * wings with pods, a five-blade main rotor that spins up while someone sits in the pilot's seat, and a
 * chin gun that the gunner (seat 1) swings and nods. The airframe box is the physics hull; the rotor
 * and the wings are bigger than it, as a cartoon's are.
 */
export class HeliModel extends RotorModel {
  private readonly turret: Node
  private readonly gun: Node
  private readonly gunRest: number
  private readonly paint: Material
  /** `performance.now()` of the last round, for the barrels' kick. */
  private firedAt = -Infinity
  /** Whoever sits at the gun, 0 for nobody. */
  gunner = 0
  private readonly pivot = new Vector3()
  private readonly aim = new Vector3()
  private readonly matrix = new Matrix4()
  private readonly beam = { at: new Vector3(), direction: new Vector3() }

  constructor(id: number) {
    if (!asset) throw new Error('loadHeliModel() must finish before helicopters are created')
    const model = asset.clone()
    super(model, ROTORS)
    this.paint = toonMaterial({ color: PAINTS[id % PAINTS.length]! })
    model.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'paint') node.material = this.paint
    })
    this.turret = model.getObjectByName('turret')!
    this.gun = model.getObjectByName('gun')!
    this.gunRest = this.gun.position.z
  }

  /** Places the helicopter, turns its rotors and aims the nose gun: `turret` is its angles in the hull frame. */
  override update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    seats: readonly number[],
    turret?: Turret | null,
  ) {
    super.update(position, rotation, steer, seats)
    this.turret.rotation.y = turret?.yaw ?? 0
    this.gun.rotation.x = turret?.pitch ?? 0
    const since = (performance.now() - this.firedAt) / 1000
    this.gun.position.z = this.gunRest + (since < RECOIL_SECONDS ? RECOIL_STROKE * (1 - since / RECOIL_SECONDS) : 0)
    this.gunner = seats[1] ?? 0
  }

  /** A round was just fired: the barrels kick back. */
  recoil() {
    this.firedAt = performance.now()
  }

  /** The gun in world space as of the last `update`: its pivot and the unit aim direction. */
  barrel() {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.pivot)
    this.aim.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.gun.matrixWorld))
    return { origin: this.pivot.clone(), direction: this.aim.clone() }
  }

  /**
   * The searchlight under the nose gun, which turns with it: from just past the muzzle (so it does not
   * light the gun), along the barrel, as of the last `update`. The vectors are reused by the next call.
   */
  override searchlight(): { at: Vector3; direction: Vector3 } {
    this.root.updateMatrixWorld(true)
    this.gun.getWorldPosition(this.beam.at)
    this.beam.direction.set(0, 0, -1).transformDirection(this.matrix.fromArray(this.gun.matrixWorld))
    this.beam.at.addScaledVector(this.beam.direction, HELI_GUN_LENGTH + 0.2)
    return this.beam
  }

  /** Where rounds leave the gun in world space. */
  muzzle() {
    const { origin, direction } = this.barrel()
    return origin.addScaledVector(direction, HELI_GUN_LENGTH)
  }

  /** Frees what this helicopter owns; the model's geometry and other materials are shared by every one. */
  override dispose() {
    this.paint.dispose()
    super.dispose()
  }
}
