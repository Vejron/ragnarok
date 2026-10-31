import { Matrix4, Mesh, Quaternion, Vector3, type Material, type Node } from '@ragnarok/engine'
import { HenModel, type Breed } from './hen'
import { parseToonModel, unlitMaterial } from './models'
import { toonMaterial } from './toon'
import { TUPP_EYE_AHEAD, TUPP_EYE_X } from '@/sim/consts'
import type { Arms, Turret } from '@/entities/units'

/** The Hedemora's two commonest looks, handed out by vehicle id: black with a gold hackle, and lavender with a pale one. */
const BREEDS: readonly (Breed & { hackle: number })[] = [
  { paint: 0x1e1b1d, comb: null, hackle: 0xd9962e },
  { paint: 0x83808f, comb: null, hackle: 0xcfc6da },
]
export const TUPP_MODEL_URL = `${import.meta.env.BASE_URL}models/tupp.glb`
const X = new Vector3(1, 0, 0)
const Y = new Vector3(0, 1, 0)
/** How far the skull shakes while the beam burns, metres. */
const SHUDDER = 0.05
/** How far the wings swing out as it rears to fire, radians. */
const REAR = 1.2

let asset: Node | null = null

/**
 * Parses `tupp.glb` (built by `tools/vehicles/build_tupp.py`) for every `TuppModel` to share. As a hen's,
 * but its `hackle`, not its comb, is replaced per tupp.
 */
export async function parseTuppModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => (source.name === 'lamp' ? unlitMaterial(source) : null), { merge: true, keep: ['hackle'] })
}

export async function loadTuppModel(url = TUPP_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseTuppModel(await response.arrayBuffer())
}

/** A giant robot Hedemora rooster: a hen in everything but its skull, which turns to where its laser eyes aim (`LaserEyes` draws them firing). */
export class TuppModel extends HenModel {
  private readonly skull: Node
  private readonly hackle: Material
  private readonly skullRest: Vector3
  /** Where the beam last ended, from the shot events; null until the first of a burn. */
  beamEnd: Vector3 | null = null
  /** Where the beam last burnt the ground, this burn: the next burn joins on to it. */
  scorched: Vector3 | null = null
  /** How hard the skull shakes, 0..1. */
  shudder = 0
  /** How far it throws its wings out as it fires, 0..1. */
  set rearing(share: number) {
    this.brace = share * REAR
  }

  constructor(id: number) {
    super(id, asset, BREEDS)
    this.hackle = toonMaterial({ color: BREEDS[id % BREEDS.length]!.hackle })
    this.root.traverse((node) => {
      if (node instanceof Mesh && node.material.name === 'hackle') node.material = this.hackle
    })
    this.skull = this.root.getObjectByName('skull')!
    this.skullRest = new Vector3().copy(this.skull.position)
  }

  /** Where the eyes aim from and which way, in the world, for the camera, the sound and the beams. */
  barrel() {
    const matrix = this.skullMatrix()
    return { origin: new Vector3().applyMatrix4(matrix), direction: new Vector3(0, 0, -1).transformDirection(matrix) }
  }

  /** The two lenses, in the world, where the beams start. */
  eyes() {
    const matrix = this.skullMatrix()
    return [-1, 1].map((side) => new Vector3(side * TUPP_EYE_X, 0, -TUPP_EYE_AHEAD).applyMatrix4(matrix))
  }

  override update(
    position: readonly [number, number, number],
    rotation: readonly [number, number, number, number],
    steer: number,
    seats: readonly number[],
    turret?: Turret | null,
    arms?: Arms | null,
  ) {
    super.update(position, rotation, steer, seats, turret, arms)
    const pitch = new Quaternion().setFromAxisAngle(X, turret?.pitch ?? 0)
    this.skull.quaternion.setFromAxisAngle(Y, turret?.yaw ?? 0).multiply(pitch)
    const shake = () => (Math.random() - 0.5) * SHUDDER * this.shudder
    this.skull.position.set(this.skullRest.x + shake(), this.skullRest.y + shake(), this.skullRest.z + shake())
  }

  override dispose() {
    this.hackle.dispose()
    super.dispose()
  }

  private skullMatrix() {
    this.skull.updateWorldMatrix(true, false)
    return new Matrix4().fromArray(this.skull.matrixWorld)
  }
}
