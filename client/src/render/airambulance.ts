import { Vector3, type Node } from '@ragnarok/engine'
import { AIR_AMBULANCE_LAMP } from '@/sim/consts'
import { parseToonModel } from './models'
import { RotorModel, type RotorLook } from './rotor'
import { toonMaterial } from './toon'

export const AIR_AMBULANCE_MODEL_URL = `${import.meta.env.BASE_URL}models/air_ambulance.glb`

/**
 * The fan's ten blades turn slower than the main rotor here, where a real one's turn nine times as
 * fast: at that speed they would seem to stand still or run backwards at sixty frames a second.
 */
const ROTORS: RotorLook = { speed: 28, tail: 0.55 }
/** Clearer than the gunship's, so that those aboard show through it. */
const GLASS_OPACITY = 0.3
/** How far the searchlight is dipped below the nose: a rise over a run of 1. */
const LAMP_DIP = 0.25
/** How far ahead of the lamp's middle the beam starts, metres, so it does not light the nose over it. */
const LAMP_AHEAD = 0.4
let asset: Node | null = null

/**
 * Parses `air_ambulance.glb` (built by `tools/vehicles/build_air_ambulance.py`) for every
 * `AirAmbulanceModel` to share. They all wear the one livery, so there is no paint to replace.
 */
export async function parseAirAmbulanceModel(data: ArrayBuffer) {
  asset = parseToonModel(data, (source) => {
    if (source.name !== 'glass') return null
    return toonMaterial({ color: source.color, name: source.name, blend: 'alpha', opacity: GLASS_OPACITY, depthWrite: false })
  }, { merge: true })
}

export async function loadAirAmbulanceModel(url = AIR_AMBULANCE_MODEL_URL) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
  await parseAirAmbulanceModel(await response.arrayBuffer())
}

/**
 * An EC135 in Swedish ambulance yellow: a glazed cabin its four aboard show in, skids, four blades, a
 * fan in the fin and a searchlight fixed under the nose. The cabin's box and the tail's solids are the
 * physics hull; the rotor is drawn only.
 */
export class AirAmbulanceModel extends RotorModel {
  private readonly beam = { at: new Vector3(), direction: new Vector3() }

  constructor() {
    if (!asset) throw new Error('loadAirAmbulanceModel() must finish before air ambulances are created')
    super(asset.clone(), ROTORS)
  }

  /** From just ahead of the lamp under the nose, along it and dipped towards the ground ahead. */
  searchlight() {
    const { position, quaternion } = this.root
    const [x, y, z] = AIR_AMBULANCE_LAMP
    this.beam.at.set(x, y, z - LAMP_AHEAD).applyQuaternion(quaternion)
    this.beam.at.x += position.x
    this.beam.at.y += position.y
    this.beam.at.z += position.z
    this.beam.direction.set(0, -LAMP_DIP, -1).normalize().applyQuaternion(quaternion)
    return this.beam
  }
}
