import init, { ClientSim } from './wasm/sim'
import { DAMAGE_TURRET, TURRET_GONE, type InputFrame } from '@/net/codec'
import type { DriveState } from '@/entities/units'
import type { TileRef } from '@/world'
import { ROADSTER_AIR } from './consts'

export type Vec3 = [number, number, number]

/** Mirrors `sim::Character`. Position is the capsule centre. */
export interface CharacterState {
  position: Vec3
  yaw: number
  vy: number
  grounded: boolean
  /** Leftover knockback velocity `[x, z]`, m/s. */
  knock: [number, number]
  /** Id of the vehicle stood on, or 0. The predicted frame is then the one where that vehicle is still. */
  ride: number
  /** Lying down. */
  prone: boolean
  /** Hanging under an open parachute; `knock` is then the glide. */
  chute: boolean
  /** Stamina used up, 0 (rested) ..= `STAMINA_FULL`, in whole points. */
  spent: number
  /** Ran out of stamina and has not got enough back: running is walking, jumps are low. */
  winded: boolean
  /** The limbs lost (`LIMB_*`): one leg hops, none crawls. The server's to change. */
  lost: number
}

/** One tick of a stickman: what `Predictor` runs, and replays on reconciling. */
export type Stepper = (state: CharacterState, input: InputFrame, dt: number, speedScale: number) => CharacterState

/** What a driver asks of a vehicle this tick, as `sim::CarControls`. */
export interface CarControls {
  /** -1 (reverse) ..= 1 (forward). */
  throttle: number
  /** -1 (left) ..= 1 (right). */
  steer: number
  handbrake: boolean
  /** -1 (sink) ..= 1 (climb). Only a helicopter has it. */
  lift: number
  /** A pilot is aboard and the rotor or propeller turns. */
  powered: boolean
  /** Where a plane's or a drone's pilot looks, `[yaw, pitch]`: it turns towards it. Null flies straight on. */
  aim?: [number, number] | null
  /** A Roadster's rear nozzles are held open. */
  boost?: boolean
  /** A Roadster hops, then hovers; a hen leaps, then flaps or glides. */
  hop?: boolean
}

/** A simulated vehicle's state, as `ClientSim::car_state` gives it. */
export interface CarBodyState {
  position: [number, number, number]
  /** Unit quaternion `[x, y, z, w]`. */
  rotation: [number, number, number, number]
  linvel: [number, number, number]
  angvel: [number, number, number]
  /** Front wheel angle, radians, and the yaw rate a tank has ramped to. */
  steer: number
  spin: number
  /** A Roadster's air or a hen's stamina (`ROADSTER_AIR` and `HEN_STAMINA` full); null for a kind without either. */
  tank: number | null
  /** The `THRUST_*` nozzles that fired in the last step. */
  firing: number
  /** Steering as a fraction of the lock, for drawing the front wheels. */
  steerFraction: number
}

/** Physics time on this client since the debug readout last reset it (see `render/perf.ts`). */
export const physicsClock = { ms: 0, calls: 0 }

/** Runs `work` and adds its time to `physicsClock`. */
export function timedPhysics<T>(work: () => T): T {
  const started = performance.now()
  try {
    return work()
  } finally {
    physicsClock.ms += performance.now() - started
    physicsClock.calls++
  }
}
const timed = timedPhysics

const flag = (b: boolean | undefined) => (b ? 1 : 0)

/** What a driver's `input` asks of a vehicle of `kind`, read as the server reads it (`CarControls::driven`). */
export function controlsOf(input: InputFrame, kind: number): CarControls {
  const c = ClientSim.controlsFor(kind, input.strafe, input.forward, input.yaw, input.pitch, input.buttons)
  return { throttle: c[0]!, steer: c[1]!, handbrake: c[2] === 1, lift: c[3]!, powered: c[4] === 1, aim: c[5] === 1 ? [c[6]!, c[7]!] : null, boost: c[8] === 1, hop: c[9] === 1 }
}

/** A vehicle as the newest snapshot has it, which its driver's prediction starts from. Metres. */
export interface VehiclePose {
  id: number
  kind: number
  x: number
  y: number
  z: number
  qx: number
  qy: number
  qz: number
  qw: number
  hp: number
  /** `VehicleState::damage`'s parts, all 0 while unhurt. */
  damage: readonly number[]
  /** Air, stamina or battery left, 0..1, for a vehicle that has any. */
  air: number | null
}

/** A vehicle of the newest snapshot, as a solid for the stickman, with its horizontal velocity. */
export type Obstacle = Pick<VehiclePose, 'id' | 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'> & { kind?: number; vx?: number; vz?: number; damage?: readonly number[] }

/**
 * The local stickman's world: the server's own physics (`crates/sim`, built to WASM by
 * `npm run build:wasm`), fed the tiles the client streams and the vehicles of the newest snapshot,
 * stepping the stickman exactly as the server does. It replaces the hand-written mirror of the
 * server's character controller.
 */
export class SimMotor {
  /** The character as the WASM side reads and writes it (`sim-wasm`'s layout). */
  private readonly scratch = new Float64Array(14)

  private constructor(private readonly sim: ClientSim) {}

  /** A world from its `world.msgpack` bytes, with no tile in it yet. */
  static async create(manifest: Uint8Array): Promise<SimMotor> {
    await init()
    return new SimMotor(new ClientSim(manifest))
  }

  /** Adds a streamed tile from its file's bytes. */
  addTile(ref: Pick<TileRef, 'col' | 'row'>, bytes: Uint8Array) {
    timed(() => this.sim.addTile(ref.col, ref.row, bytes))
  }

  /** Gives the tile its ground after a crater: `heights` in whole centimetres, as the tile file has them. */
  setTileGround(ref: Pick<TileRef, 'col' | 'row'>, heights: Int32Array) {
    timed(() => this.sim.setTileGround(ref.col, ref.row, heights))
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    timed(() => this.sim.removeTile(ref.col, ref.row))
  }

  /** Fells tree `index` of tile `(col, row)`. */
  fellTree(col: number, row: number, index: number) {
    timed(() => {
      if (this.sim.fellTree(col, row, index)) this.sim.refresh()
    })
  }

  /**
   * Brings the tile's buildings `down` and gives each of the `carved` ones its shape without the
   * cells listed for it, as the server has them.
   */
  setBuildings(col: number, row: number, down: Iterable<number>, carved: ReadonlyMap<number, ReadonlySet<number>>) {
    timed(() => {
      for (const index of down) this.sim.fellBuilding(col, row, index)
      for (const [index, cells] of carved) this.sim.carveBuilding(col, row, index, Uint32Array.from(cells))
      this.sim.refresh()
    })
  }

  /**
   * Gives each chunk of placed cubes in `chunks` the shape of its cubes (`cells`, one byte a cube,
   * 0 for none), as the server has them; a chunk with none stops being solid.
   */
  setConstruct(chunks: Iterable<{ chunk: readonly [number, number, number]; cells: Uint8Array }>) {
    timed(() => {
      for (const { chunk, cells } of chunks) this.sim.setConstructChunk(chunk[0], chunk[1], chunk[2], cells)
      this.sim.refresh()
    })
  }

  /** Whether a cube of half side `half` centred at `centre` would overlap a building, tree, vehicle or placed cubes. */
  cubeBlocked(centre: Vec3, half: number): boolean {
    return this.sim.cubeBlocked(centre[0], centre[1], centre[2], half)
  }

  /** Whether a building's standing cells reach within `radius` of `point`. */
  buildingNear(point: Vec3, radius: number): boolean {
    return this.sim.buildingNear(point[0], point[1], point[2], radius)
  }

  /**
   * Makes the vehicles solid for the stickman at the poses of the newest snapshot; the ones missing
   * from `vehicles` go. The server runs them as dynamic bodies; here they only need to block the
   * predicted stickman like they block the real one, and carry it off at their speed when it leaves them.
   */
  setVehicles(vehicles: readonly Obstacle[]) {
    timed(() => {
      const alive = new Set<number>()
      for (const car of vehicles) {
        alive.add(car.id)
        const turret = car.damage?.[DAMAGE_TURRET] !== TURRET_GONE
        this.sim.setObstacle(car.id, car.kind ?? 0, car.x, car.y, car.z, car.qx, car.qy, car.qz, car.qw, car.vx ?? 0, 0, car.vz ?? 0, turret)
      }
      for (const id of this.sim.obstacleIds()) if (!alive.has(id)) this.sim.removeObstacle(id)
      this.sim.refresh()
    })
  }

  /**
   * How far a ray travels before it meets the world or a vehicle, and the surface normal there; null
   * when it reaches `max` clear. Used to draw where a grenade will go.
   */
  castRay(origin: Vec3, dir: Vec3, max: number): { t: number; normal: Vec3 } | null {
    const hit = this.sim.castRay(origin[0], origin[1], origin[2], dir[0], dir[1], dir[2], max)
    if (hit.length < 4) return null
    return { t: hit[0]!, normal: [hit[1]!, hit[2]!, hit[3]!] }
  }

  /** Starts simulating vehicle `id` of `kind` here in place of its stand-in: the player drives it. */
  startCar(id: number, kind: number, position: Vec3, yaw = 0) {
    this.sim.startCar(id, kind, position[0], position[1], position[2], yaw)
  }

  stopCar(id: number) {
    this.sim.stopCar(id)
  }

  /**
   * Puts simulated vehicle `id` where the server says it is: its pose, the driver's motion, and the
   * air, wear and turret the snapshot shows.
   */
  setCarState(id: number, vehicle: VehiclePose, drive: DriveState) {
    // The wire carries a share of full (`air_byte` on the server).
    const tank = vehicle.air !== null ? vehicle.air * ROADSTER_AIR : Number.NaN
    const turret = vehicle.damage[DAMAGE_TURRET] !== TURRET_GONE
    this.sim.setCarState(id, vehicle.x, vehicle.y, vehicle.z, vehicle.qx, vehicle.qy, vehicle.qz, vehicle.qw, drive.vx, drive.vy, drive.vz, drive.wx, drive.wy, drive.wz, drive.steer, drive.spin, tank, vehicle.hp, Uint8Array.from(vehicle.damage), turret)
  }

  /** Puts simulated vehicle `id` in a state of the caller's making, the rest as it is when new; for tests. */
  placeCar(id: number, state: { position: Vec3; rotation?: [number, number, number, number]; linvel?: Vec3; angvel?: Vec3; hp?: number; damage?: readonly number[]; turret?: boolean }) {
    const { position, rotation = [0, 0, 0, 1], linvel = [0, 0, 0], angvel = [0, 0, 0], hp = 255, damage = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], turret = true } = state
    this.sim.setCarState(id, ...position, ...rotation, ...linvel, ...angvel, 0, 0, Number.NaN, hp, Uint8Array.from(damage), turret)
  }

  /** One tick of simulated vehicle `id` under `controls`: the server's `drive_car` then `step_cars`. */
  driveCar(id: number, controls: CarControls, dt: number) {
    const aim = controls.aim ?? null
    const c = [controls.throttle, controls.steer, flag(controls.handbrake), controls.lift, flag(controls.powered), flag(aim !== null), aim?.[0] ?? 0, aim?.[1] ?? 0, flag(controls.boost), flag(controls.hop)]
    timed(() => this.sim.driveCar(id, Float32Array.from(c), dt))
  }

  /** Simulated vehicle `id`'s state, or null when it is not simulated here. */
  carState(id: number): CarBodyState | null {
    const s = this.sim.carState(id)
    if (s.length < 18) return null
    return {
      position: [s[0]!, s[1]!, s[2]!],
      rotation: [s[3]!, s[4]!, s[5]!, s[6]!],
      linvel: [s[7]!, s[8]!, s[9]!],
      angvel: [s[10]!, s[11]!, s[12]!],
      steer: s[13]!,
      spin: s[14]!,
      tank: Number.isNaN(s[15]!) ? null : s[15]!,
      firing: s[16]!,
      steerFraction: s[17]!,
    }
  }

  /** Whether the standing body fits with its centre at `position`. */
  fits(position: Vec3): boolean {
    return this.sim.fits(position[0], position[1], position[2])
  }

  /** One tick of the stickman, as `Physics::step_character` does it on the server. */
  step(state: CharacterState, input: InputFrame, dt: number, speedScale = 1): CharacterState {
    return timed(() => {
      const s = this.scratch
      s[0] = state.position[0]
      s[1] = state.position[1]
      s[2] = state.position[2]
      s[3] = state.yaw
      s[4] = state.vy
      s[5] = state.grounded ? 1 : 0
      s[6] = state.knock[0]
      s[7] = state.knock[1]
      s[8] = state.ride
      s[9] = state.prone ? 1 : 0
      s[10] = state.chute ? 1 : 0
      s[11] = state.spent
      s[12] = state.winded ? 1 : 0
      s[13] = state.lost
      this.sim.stepCharacter(s, input.seq, input.strafe, input.forward, input.yaw, input.buttons, input.pitch, input.view, input.slot, dt, speedScale)
      return { position: [s[0]!, s[1]!, s[2]!], yaw: s[3]!, vy: s[4]!, grounded: s[5] === 1, knock: [s[6]!, s[7]!], ride: s[8]!, prone: s[9] === 1, chute: s[10] === 1, spent: s[11]!, winded: s[12] === 1, lost: s[13]! }
    })
  }

  /** The stepper `Predictor` runs. */
  get stepper(): Stepper {
    return (state, input, dt, speedScale) => this.step(state, input, dt, speedScale)
  }

  dispose() {
    this.sim.free()
  }
}
