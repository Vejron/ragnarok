import { Vector3 } from '@ragnarok/engine'
import { AmvModel } from '@/render/amv'
import { Cb90Model } from '@/render/cb90'
import { HeliModel } from '@/render/heli'
import { AUTOCANNON, BEAM, BLACK_POWDER, CANNON, MORTAR, ROTARY, type MuzzleProfile } from '@/render/gunfire'
import { LaserEyes } from '@/render/lasereyes'
import { MortarModel } from '@/render/mortar'
import { PakModel } from '@/render/pak'
import { PbvModel } from '@/render/pbv'
import { ShipGunModel } from '@/render/shipgun'
import { PlaneModel } from '@/render/plane'
import { StrfModel } from '@/render/strf'
import { StrvModel } from '@/render/strvs'
import { TankModel } from '@/render/tank'
import { TechnicalModel } from '@/render/technical'
import { TitanModel } from '@/render/titan'
import { TuppModel } from '@/render/tupp'
import { WalkerModel } from '@/render/walker'
import { arcTime } from '@/render/rounds'
import { AMV_BARREL_LENGTH, CB90_CANNON_LENGTH, CB90_CANNON_RANGE, CB90_MG_LENGTH, CB90_MG_RANGE, HELI_GUN_LENGTH, MORTAR_BARREL_LENGTH, PAK_BARREL_LENGTH, PBV_BARREL_LENGTH, PBV_CANNON_RANGE, SHIP_GUN_BARREL_LENGTH, PLANE_GUN_RANGE, STRF_BARREL_LENGTH, STRV_BARREL_LENGTH, TANK_BARREL_LENGTH, TANK_MG_LENGTH, TANK_MG_RANGE, TECHNICAL_MG_LENGTH, TECHNICAL_MG_RANGE, TECHNICAL_MG_SEAT, TITAN_GUN_LENGTH, WALKER_GUN_LENGTH } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** Of every few rounds between two frames, at most this many are drawn: more is a gap in the stream, not a burst. */
const MAX_ROUNDS = 4

/** A gun that fires shells, as its shot is drawn: how far its muzzle is from its pivot (metres), how the shot looks, which seat fires it and whether it has a heavy gun's bang. */
interface Cannon {
  length: number
  profile: MuzzleProfile
  gunner: number
  heavy: boolean
  /**
   * How far its rounds fly, metres, for a gun whose rounds are bullets the server does not replicate:
   * each is drawn from the muzzle out to what it meets. Left out for a gun whose shells are projectiles.
   */
  range?: number
}

type CannonModel = AmvModel | TankModel | HeliModel | WalkerModel | MortarModel | PakModel | ShipGunModel | TitanModel

/** Who fires it: the driver of a walker, a Titan, a Strv S or a gun on a carriage, the gunner of the rest. */
function cannonOf(model: unknown): Cannon | null {
  if (model instanceof TitanModel) return { length: TITAN_GUN_LENGTH, profile: ROTARY, gunner: 0, heavy: true }
  if (model instanceof HeliModel) return { length: HELI_GUN_LENGTH, profile: AUTOCANNON, gunner: 1, heavy: false }
  if (model instanceof WalkerModel) return { length: WALKER_GUN_LENGTH, profile: BEAM, gunner: 0, heavy: false }
  if (model instanceof StrvModel) return { length: STRV_BARREL_LENGTH, profile: CANNON, gunner: 0, heavy: true }
  if (model instanceof StrfModel) return { length: STRF_BARREL_LENGTH, profile: AUTOCANNON, gunner: 1, heavy: false }
  if (model instanceof Cb90Model) return { length: CB90_CANNON_LENGTH, profile: AUTOCANNON, gunner: 1, heavy: false, range: CB90_CANNON_RANGE }
  if (model instanceof PbvModel) return { length: PBV_BARREL_LENGTH, profile: AUTOCANNON, gunner: 1, heavy: false, range: PBV_CANNON_RANGE }
  if (model instanceof AmvModel) return { length: AMV_BARREL_LENGTH, profile: MORTAR, gunner: 1, heavy: false }
  if (model instanceof MortarModel) return { length: MORTAR_BARREL_LENGTH, profile: MORTAR, gunner: 0, heavy: false }
  if (model instanceof PakModel) return { length: PAK_BARREL_LENGTH, profile: CANNON, gunner: 0, heavy: false }
  if (model instanceof ShipGunModel) return { length: SHIP_GUN_BARREL_LENGTH, profile: BLACK_POWDER, gunner: 0, heavy: false }
  if (model instanceof TankModel) return { length: TANK_BARREL_LENGTH, profile: CANNON, gunner: 1, heavy: true }
  return null
}

/**
 * A mounted gun (`turret`): every new shot on its counter is a shell or round leaving, with fire and
 * smoke at the muzzle (`Effects.gunfire`), a recoil and the bang, and a barrel that goes on smoking for
 * some seconds after. The shell itself is a projectile, and a plane's rear ring gun's and a technical's
 * machine gun's rounds are drawn as bullets in flight (`Rounds`).
 */
export class Turrets implements Presenter<Frame> {
  /** The shot counter as last seen, per gun. */
  private readonly shots = new Map<number, number>()
  /** How hot each barrel that has fired still is, 1 down to 0: what it smokes by. */
  private readonly heat = new Map<number, number>()
  private readonly laserEyes = new Map<number, LaserEyes>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.turret !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const model = this.vehicles.model(id)
    const placed = this.vehicles.placed.get(id)
    if (!model || !placed?.turret) return
    if (model instanceof TuppModel) return this.eyes(id, model, placed.turret.flags, placed.seats, frame)
    const shots = placed.turret.shots
    const before = this.shots.get(id)
    this.shots.set(id, shots)
    // The first sight of a gun is not a shot, nor is a counter that has not moved.
    const fired = before === undefined ? 0 : (shots - before + 256) % 256
    const cannon = cannonOf(model)
    if (cannon) return this.cannon(id, model as CannonModel, cannon, shots, fired, placed.seats, frame)
    if (!fired) return
    if (model instanceof PlaneModel) this.ring(model, fired)
    else if (model instanceof TechnicalModel) this.machineGun(model, fired, placed.seats, frame)
  }

  remove(id: number) {
    this.shots.delete(id)
    this.heat.delete(id)
    this.laserEyes.get(id)?.dispose()
    this.laserEyes.delete(id)
  }

  private eyes(id: number, tupp: TuppModel, flags: number, seats: readonly number[], frame: Frame) {
    let eyes = this.laserEyes.get(id)
    if (!eyes) this.laserEyes.set(id, (eyes = new LaserEyes()))
    const mine = frame.localId !== null && seats[0] === frame.localId
    eyes.update(tupp, flags, mine, frame.dt, { effects: this.world.effects, sounds: this.world.sounds, lights: this.world.lights, lit: this.world.ownShot })
  }

  /**
   * A gun that fires shells, this frame: the `fired` rounds that have left it since the last one, each
   * shot a blast at the muzzle, its light, a kick and the bang, and then the smoke its barrel gives off
   * while it is still hot, from wherever the muzzle now points.
   */
  private cannon(id: number, gun: CannonModel, cannon: Cannon, shots: number, fired: number, seats: readonly number[], frame: Frame) {
    let heat = this.heat.get(id) ?? 0
    if (!fired && heat <= 0) return
    // A walker's barrels and a mortar carrier's tubes fire in turn: the counter has already gone up for this shot.
    const side = (shots - 1) % 2 === 0 ? -1 : 1
    const { origin, direction } = gun instanceof WalkerModel || gun instanceof AmvModel ? gun.barrel(side) : gun.barrel()
    const muzzle = origin.clone().addScaledVector(direction, cannon.length)
    const { profile } = cannon
    const { effects, sounds } = this.world
    if (fired) {
      if (gun instanceof TitanModel) gun.kick()
      else gun.recoil()
      effects.gunfire(muzzle, direction, profile, fired, this.world.fx())
      this.world.lights.flash(muzzle.x, muzzle.y, muzzle.z, profile.light)
      if (gun instanceof WalkerModel) sounds.laser([muzzle.x, muzzle.y, muzzle.z])
      else sounds.cannon([muzzle.x, muzzle.y, muzzle.z], cannon.heavy)
      if (cannon.range) for (let k = 0; k < Math.min(MAX_ROUNDS, fired); k++) fireRound(this.world, muzzle, direction, cannon.range)
      if (frame.localId !== null && seats[cannon.gunner] === frame.localId) this.world.ownShot()
      heat = Math.min(1, heat + profile.heat * fired)
    }
    if (heat <= 0) return
    effects.barrelSmoke(muzzle, direction, heat, frame.dt, profile, this.world.fx())
    heat -= frame.dt / profile.linger
    if (heat > 0) this.heat.set(id, heat)
    else this.heat.delete(id)
  }

  /** A plane's rear ring gun: each new round a bullet out to what it meets. */
  private ring(plane: PlaneModel, fired: number) {
    const rounds = Math.min(MAX_ROUNDS, fired)
    const { direction } = plane.barrel()
    const muzzle = plane.muzzle()
    this.world.effects.muzzle(muzzle)
    for (let k = 0; k < rounds; k++) fireRound(this.world, muzzle, direction, PLANE_GUN_RANGE)
    plane.recoilRing()
    this.world.sounds.machineGun([muzzle.x, muzzle.y, muzzle.z], rounds, true)
  }

  /** A technical's machine gun: each new round a bullet out to what it meets, with a flash and the rattle. */
  private machineGun(truck: TechnicalModel, fired: number, seats: readonly number[], frame: Frame) {
    const rounds = Math.min(MAX_ROUNDS, fired)
    const { origin, direction } = truck.barrel()
    const muzzle = origin.addScaledVector(direction, TECHNICAL_MG_LENGTH)
    this.world.effects.muzzle(muzzle)
    for (let k = 0; k < rounds; k++) fireRound(this.world, muzzle, direction, TECHNICAL_MG_RANGE)
    truck.recoil()
    this.world.sounds.machineGun([muzzle.x, muzzle.y, muzzle.z], rounds)
    if (frame.localId !== null && seats[TECHNICAL_MG_SEAT] === frame.localId) this.world.ownShot()
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
      fireRound(this.world, origin, direction, PLANE_GUN_RANGE)
    }
    const { origin } = plane.forwardGun(0)
    this.world.sounds.machineGun([origin.x, origin.y, origin.z], rounds)
  }

  remove(id: number) {
    this.shots.delete(id)
  }
}

/**
 * A tank's roof gun or a Stridsbåt 90's machine guns (`pintle`): every new round on its counter is a bullet
 * from its muzzle out to what it meets, with a flash and the rattle; the boat's two barrels take turns.
 * Only rounds that hit something come back from the server.
 */
export class RoofGuns implements Presenter<Frame> {
  /** The counter as last drawn, per tank. */
  private readonly shots = new Map<number, number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.pintle !== undefined && entity.orientation !== undefined
  }

  update(id: number) {
    const tank = this.vehicles.model(id)
    const shots = this.vehicles.placed.get(id)?.pintle?.shots
    if (!(tank instanceof TankModel) || shots === undefined) return
    const before = this.shots.get(id)
    this.shots.set(id, shots)
    if (before === undefined || before === shots) return
    const rounds = Math.min(MAX_ROUNDS, (shots - before + 256) % 256)
    const [length, range] = tank instanceof Cb90Model ? [CB90_MG_LENGTH, CB90_MG_RANGE] : [TANK_MG_LENGTH, TANK_MG_RANGE]
    let last: Vector3 | null = null
    for (let k = 1; k <= rounds; k++) {
      // The counter had gone up for each round: an even count left the left barrel, as the server takes turns.
      const barrel = tank instanceof Cb90Model ? tank.machineGun((before + k - 1) % 2 === 0 ? -1 : 1) : tank.pintleBarrel()
      if (!barrel) return
      const muzzle = barrel.origin.addScaledVector(barrel.direction, length)
      this.world.effects.muzzle(muzzle)
      fireRound(this.world, muzzle, barrel.direction, range)
      last = muzzle
    }
    if (last) this.world.sounds.machineGun([last.x, last.y, last.z], rounds)
  }

  remove(id: number) {
    this.shots.delete(id)
  }
}

/** Sends a round from `origin` along `direction`, falling as it goes, to the first thing it meets or the end of its `range`. */
function fireRound(world: VehicleWorld, origin: Vector3, direction: Vector3, range: number) {
  const { time, hit } = arcTime(world.castRay, origin, direction, range)
  world.rounds.fire(origin, direction, time, hit)
}
