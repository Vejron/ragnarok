import init, { ClientSim } from './wasm/sim'
import { DAMAGE_TURRET, TURRET_GONE, type InputFrame, type VehicleState } from '@/net/codec'
import type { TileRef } from '@/world'
import { timedPhysics as timed } from './physics'

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
}

/** One tick of a stickman: what `Predictor` runs, and replays on reconciling. */
export type Stepper = (state: CharacterState, input: InputFrame, dt: number, speedScale: number) => CharacterState

/** A vehicle of the newest snapshot, as a solid for the stickman, with its horizontal velocity. */
export type Obstacle = Pick<VehicleState, 'id' | 'x' | 'y' | 'z' | 'qx' | 'qy' | 'qz' | 'qw'> & { kind?: number; vx?: number; vz?: number; damage?: readonly number[] }

/**
 * The local stickman's world: the server's own physics (`crates/sim`, built to WASM by
 * `npm run build:wasm`), fed the tiles the client streams and the vehicles of the newest snapshot,
 * stepping the stickman exactly as the server does. It replaces the hand-written mirror of the
 * server's character controller.
 */
export class SimMotor {
  /** The character as the WASM side reads and writes it (`sim-wasm`'s layout). */
  private readonly scratch = new Float64Array(11)

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
      this.sim.stepCharacter(s, input.seq, input.strafe, input.forward, input.yaw, input.buttons, input.pitch, input.view, input.slot, dt, speedScale)
      return { position: [s[0]!, s[1]!, s[2]!], yaw: s[3]!, vy: s[4]!, grounded: s[5] === 1, knock: [s[6]!, s[7]!], ride: s[8]!, prone: s[9] === 1, chute: s[10] === 1 }
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
