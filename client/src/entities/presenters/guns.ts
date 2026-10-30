import { Vector3 } from '@ragnarok/engine'
import { HeliModel } from '@/render/heli'
import { MortarModel } from '@/render/mortar'
import { PakModel } from '@/render/pak'
import { PlaneModel } from '@/render/plane'
import { StrvModel } from '@/render/strvs'
import { TankModel } from '@/render/tank'
import { WalkerModel } from '@/render/walker'
import { HELI_GUN_LENGTH, MORTAR_BARREL_LENGTH, PAK_BARREL_LENGTH, PLANE_GUN_RANGE, STRV_BARREL_LENGTH, TANK_BARREL_LENGTH, WALKER_GUN_LENGTH } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** Of every few rounds between two frames, at most this many are drawn: more is a gap in the stream, not a burst. */
const MAX_ROUNDS = 4

/**
 * A mounted gun (`turret`): every new shot on its counter is a shell or round leaving, with a flash at
 * the muzzle, a recoil and the bang. The shell itself is a projectile, and a plane's rear ring gun's
 * rounds are drawn as bullets in flight (`Rounds`).
 */
export class Turrets implements Presenter<Frame> {
  /** The shot counter as last seen, per gun. */
  private readonly shots = new Map<number, number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.turret !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!model || !placed?.turret) return
    const shots = placed.turret.shots
    const before = this.shots.get(id)
    this.shots.set(id, shots)
    // The first sight of a gun is not a shot, nor is a counter that has not moved.
    if (before === undefined || before === shots) return
    if (model instanceof PlaneModel) this.ring(model, (shots - before + 256) % 256)
    else if (model instanceof TankModel || model instanceof HeliModel || model instanceof WalkerModel || model instanceof MortarModel || model instanceof PakModel) this.fired(model, shots, placed.seats, frame)
  }

  remove(id: number) {
    this.shots.delete(id)
  }

  private fired(gun: TankModel | HeliModel | WalkerModel | MortarModel | PakModel, shots: number, seats: readonly number[], frame: Frame) {
    // A walker's barrels fire in turn: the counter has already gone up for this shot.
    const { origin, direction } = gun instanceof WalkerModel ? gun.barrel((shots - 1) % 2 === 0 ? -1 : 1) : gun.barrel()
    gun.recoil()
    const length = gun instanceof HeliModel ? HELI_GUN_LENGTH : gun instanceof WalkerModel ? WALKER_GUN_LENGTH : gun instanceof StrvModel ? STRV_BARREL_LENGTH : gun instanceof MortarModel ? MORTAR_BARREL_LENGTH : gun instanceof PakModel ? PAK_BARREL_LENGTH : TANK_BARREL_LENGTH
    const muzzle = origin.clone().addScaledVector(direction, length)
    this.world.effects.muzzleFlash(muzzle, direction)
    this.world.lights.flash(muzzle.x, muzzle.y, muzzle.z, { color: 0xffc060, intensity: 4, radius: 14, life: 0.09 })
    if (gun instanceof WalkerModel) this.world.sounds.laser([muzzle.x, muzzle.y, muzzle.z])
    else this.world.sounds.cannon([muzzle.x, muzzle.y, muzzle.z], gun instanceof TankModel)
    // Who fires it: the driver of a walker, a Strv S or a towed gun, the gunner of the rest.
    const gunner = gun instanceof WalkerModel || gun instanceof StrvModel || gun instanceof MortarModel || gun instanceof PakModel ? 0 : 1
    if (frame.localId !== null && seats[gunner] === frame.localId) this.world.ownShot()
  }

  /** A plane's rear ring gun: each new round a bullet out to what it meets. */
  private ring(plane: PlaneModel, fired: number) {
    const rounds = Math.min(MAX_ROUNDS, fired)
    const { direction } = plane.barrel()
    const muzzle = plane.muzzle()
    this.world.effects.muzzle(muzzle)
    for (let k = 0; k < rounds; k++) fireRound(this.world, muzzle, direction)
    plane.recoilRing()
    this.world.sounds.machineGun([muzzle.x, muzzle.y, muzzle.z], rounds, true)
  }
}

/**
 * A plane's forward guns (`arms`): every new round on their counter is drawn as a bullet from its
 * muzzle, out to the first wall or the end of the guns' range, with a flash, a kick and the rattle.
 * Only rounds that hit something come back from the server, and spark where they land.
 */
export class PlaneGuns implements Presenter<Frame> {
  /** The forward guns' counter as last drawn, per plane. */
  private readonly shots = new Map<number, number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.arms !== undefined && entity.orientation !== undefined
  }

  update(id: number) {
    const plane = this.vehicles.model(id)
    const shots = this.vehicles.placed.get(id)?.arms?.shots
    if (!(plane instanceof PlaneModel) || shots === undefined) return
    const before = this.shots.get(id)
    this.shots.set(id, shots)
    if (before === undefined || before === shots) return
    const rounds = Math.min(MAX_ROUNDS, (shots - before + 256) % 256)
    for (let k = 1; k <= rounds; k++) {
      const side = (before + k) % 2
      const { origin, direction } = plane.forwardGun(side)
      plane.recoil(side)
      this.world.effects.muzzle(origin)
      fireRound(this.world, origin, direction)
    }
    const { origin } = plane.forwardGun(0)
    this.world.sounds.machineGun([origin.x, origin.y, origin.z], rounds)
  }

  remove(id: number) {
    this.shots.delete(id)
  }
}

/** Sends a round from `origin` along `direction` out to the first thing it meets, or the end of the guns' range. */
function fireRound(world: VehicleWorld, origin: Vector3, direction: Vector3) {
  const hit = world.castRay([origin.x, origin.y, origin.z], [direction.x, direction.y, direction.z], PLANE_GUN_RANGE)
  world.rounds.fire(origin, origin.clone().addScaledVector(direction, hit ? hit.t : PLANE_GUN_RANGE), hit !== null)
}
