import { KIND_HELI, KIND_PLANE, KIND_TANK } from '@/net/codec'
import { CarModel, loadCarModels, type VehicleModel } from './car'
import { HeliModel, loadHeliModel } from './heli'
import { loadPlaneModel, PlaneModel } from './plane'
import { DRIVING_DISTANCE, HELI_DISTANCE, PLANE_DISTANCE, TANK_DISTANCE } from './scene'
import { loadTankModel, TankModel } from './tank'

/**
 * Where each vehicle kind gets its model and its chase-camera distance. A new kind adds a branch
 * to each of the two functions below (and its model file); the game loop asks only these.
 */

/** Fetches the models every kind is built from. */
export const loadVehicleModels = () => Promise.all([loadCarModels(), loadTankModel(), loadHeliModel(), loadPlaneModel()])

/** The drawn vehicle for `kind`. Unknown kinds are drawn as cars. */
export function createVehicleModel(kind: number, id: number): VehicleModel {
  if (kind === KIND_TANK) return new TankModel(id)
  if (kind === KIND_HELI) return new HeliModel(id)
  if (kind === KIND_PLANE) return new PlaneModel(id)
  return new CarModel(kind, id)
}

/** How far behind a vehicle of `kind` the camera sits. */
export function cameraDistance(kind: number): number {
  if (kind === KIND_TANK) return TANK_DISTANCE
  if (kind === KIND_HELI) return HELI_DISTANCE
  if (kind === KIND_PLANE) return PLANE_DISTANCE
  return DRIVING_DISTANCE
}
