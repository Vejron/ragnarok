import { KIND_DRONE, KIND_HELI, KIND_HEN, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_STRV, KIND_TANK, KIND_WALKER } from '@/net/codec'
import { CarModel, loadCarModels, type VehicleModel } from './car'
import { HeliModel, loadHeliModel } from './heli'
import { HenModel, loadHenModel } from './hen'
import { loadPlaneModel, PlaneModel } from './plane'
import { DroneModel } from './drone'
import { LoiterModel } from './loiter'
import { MortarModel } from './mortar'
import { loadStrvModel, StrvModel } from './strvs'
import { loadTankModel, TankModel } from './tank'
import { loadWalkerModel, WalkerModel } from './walker'

/**
 * Where each vehicle kind gets its model and its chase-camera distance. A new kind adds a branch
 * to each of the two functions below (and its model file); the game loop asks only these.
 */

/** Fetches the models every kind is built from. */
export const loadVehicleModels = () => Promise.all([loadCarModels(), loadTankModel(), loadHeliModel(), loadPlaneModel(), loadWalkerModel(), loadHenModel(), loadStrvModel()])

/** The drawn vehicle for `kind`. Unknown kinds are drawn as cars. */
export function createVehicleModel(kind: number, id: number): VehicleModel {
  if (kind === KIND_TANK) return new TankModel(id)
  if (kind === KIND_HELI) return new HeliModel(id)
  if (kind === KIND_PLANE) return new PlaneModel(id)
  if (kind === KIND_WALKER) return new WalkerModel(id)
  if (kind === KIND_HEN) return new HenModel(id)
  if (kind === KIND_STRV) return new StrvModel(id)
  if (kind === KIND_MORTAR) return new MortarModel(id)
  if (kind === KIND_DRONE) return new DroneModel(id)
  if (kind === KIND_LOITER) return new LoiterModel(id)
  return new CarModel(kind, id)
}

