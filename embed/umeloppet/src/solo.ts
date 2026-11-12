import { Quaternion, Vector3 } from '@ragnarok/engine'
import { toAxis, toMrad, type InputFrame } from '@/net/codec'
import type { Components } from '@/net/components.generated'
import {
  BUTTON_CROUCH, BUTTON_RUN, BUTTON_SKIS, BUTTON_USE, KIND_SNOWMOBILE, SKI_CLIP_SPEED, SLIDE_MIN_SPEED, STANDING_Y, TICK_MS, vehicleMaxHp,
} from '@/sim/consts'
import { SimMotor, type CharacterState, type Obstacle, type VehiclePose } from '@/sim/motor'
import { Predictor } from '@/sim/predict'
import { VehicleDriver, type DrivenPose } from '@/sim/driver'
import { InputController } from '@/sim/input'
import { Terrain } from '@/sim/terrain'
import { Surfaces, SURFACE_GRASS } from '@/sim/surface'
import { Waters } from '@/sim/waters'
import { ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, Scene } from '@/render/scene'
import { Effects } from '@/render/effects'
import { Particles } from '@/render/particles'
import { Skidmarks } from '@/render/skidmarks'
import { SkiTracks } from '@/render/skitracks'
import { Snowfall } from '@/render/snowfall'
import { SnowRoost } from '@/render/snowroost'
import { setWinter } from '@/render/season'
import { freezeWater } from '@/render/water'
import { buildGround, snowOnBuildings } from '@/render/world'
import { Debris } from '@/render/debris'
import { Rockets } from '@/render/rockets'
import { ProjectileTrails } from '@/render/trails'
import { Rounds } from '@/render/rounds'
import { Grass, GRASS_LITE } from '@/render/grass'
import { Stones } from '@/render/stones'
import { LookWorker } from '@/render/lookworker'
import { ChaseCam } from '@/render/chase'
import type { FxWorld } from '@/render/tyres'
import { Crowd } from '@/render/character'
import { Ragdolls } from '@/sim/ragdoll'
import { groomCourse } from './trails'
import { Avatars } from '@/entities/avatars'
import { EntityStore } from '@/entities/store'
import { Presenters, type Frame } from '@/entities/presenter'
import { Seating } from '@/entities/presenters/seating'
import { seatFrame } from '@/entities/presenters/riders'
import { Vehicles, type VehicleWorld } from '@/entities/presenters/vehicles'
import { parseCarModel } from '@/render/car'
import { Cameras } from '@/cameras'
import { rideView, seatView } from '@/seats'
import { Surroundings } from '@/surroundings'
import { VehicleSounds } from '@/vehicleaudio'
import { audioContext } from '@/audio'
import { TileStreamer } from '@/stream'
import { WorldView } from '@/worldview'
import { decodeManifest, NO_WATER, THEME_WINTER, type WorldData } from '@/world'
import { CourseView } from './course3d'
import { Progress, RaceClock, type ClockState, type Course, type Facing } from './course'

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
/** The local skier's id, as the presenters see it; the snowmobiles' ids follow it. */
const LOCAL_ID = 1
const FIRST_SLED = 100
/** Key for the local skier's tracks. */
const LOCAL_TRACK = 0
/** A snowmobile is close enough to get on within this many metres. */
const REACH_SLED = 3.5
/** Getting off, the skier is put this far to the sled's right, metres. */
const STEP_OFF = 1.8
/** The skier starts this far behind the start line, metres. */
const BEHIND_START = 8
/** Physics steps a parked snowmobile is given to settle onto the snow before it is shown. */
const SETTLE_STEPS = 90
/** A fine winter's day: a little after noon. */
const TIME_OF_DAY = 0.53
/** The skier's camera widens past this speed, by this many degrees per m/s (as the game's does). */
const SKI_FOV_FROM = 9
const SKI_FOV_PER_SPEED = 0.7
/** How often the readout is redrawn, ms. */
const REPORT_EVERY = 100

/** What the element shows over the 3D view, a few times a second. */
export interface Report {
  /** The skier (or their snowmobile) on the map. */
  at: Facing
  /** Metres along the course, and how far off the line. */
  along: number
  off: number
  /** Ground speed, km/h. */
  speed: number
  riding: boolean
  skis: boolean
  /** A snowmobile is near enough to get on. */
  nearSled: boolean
  snowmobiles: Facing[]
  /** The race clock, from the start gate to the finish. */
  clock: ClockState
}

interface Sled {
  id: number
  pose: VehiclePose
}

const STILL_DRIVE = { vx: 0, vy: 0, vz: 0, wx: 0, wy: 0, wz: 0, steer: 0, spin: 0 }

/**
 * The game, offline and on its own: one skier on the course in the world, and a few snowmobiles to ride.
 * It is put together from the client's own parts (the world view, the sim in WASM, the soldier, the
 * vehicle presenters) as `Game` is, but with no server: the skier and the snowmobile ridden are stepped
 * here and never corrected, and the snowmobiles are fed to the presenters as if replicated.
 */
export class SoloGame {
  private readonly scene: Scene
  readonly input: InputController
  private readonly crowd = new Crowd()
  private readonly avatars = Object.assign(new Avatars(this.crowd), { localId: LOCAL_ID })
  private readonly store = new EntityStore()
  private readonly machines = new Presenters<Frame>()
  private readonly vehicles: Vehicles
  private readonly view: WorldView
  private readonly debris = new Debris(40)
  private readonly smoke = new Particles(2048, false, true)
  private readonly glow = new Particles(1024, true)
  private readonly blood = new Particles(64)
  private readonly effects = new Effects({ smoke: this.smoke, glow: this.glow, blood: this.blood })
  private readonly skids = new Skidmarks()
  private readonly rockets = new Rockets()
  private readonly trails = new ProjectileTrails()
  private readonly rounds = new Rounds(this.rockets, this.trails, (at) => this.effects.pelletStrike(at))
  private readonly sounds = new VehicleSounds()
  private readonly chase = new ChaseCam()
  private readonly cameras: Cameras
  private readonly surroundings: Surroundings
  private readonly progress: Progress
  /** The race clock, and its own follower along the course, moved on every tick. */
  private readonly clock: RaceClock
  private readonly clockProgress: Progress
  private clockAt: [number, number] | null = null
  private readonly courseView: CourseView
  private world: WorldData | null = null
  private terrain: Terrain | null = null
  private surfaces: Surfaces | null = null
  private waters: Waters | null = null
  private grass: Grass | null = null
  private stones: Stones | null = null
  private motor: SimMotor | null = null
  private predictor: Predictor | null = null
  private driver: VehicleDriver | null = null
  /** Falls, tumbled as ragdolls in a physics world of their own. */
  private ragdolls: Ragdolls | null = null
  private wasDown = false
  private tiles: TileStreamer | null = null
  private skiTracks: SkiTracks | null = null
  private snowfall: Snowfall | null = null
  private snowRoost: SnowRoost | null = null
  private readonly sleds: Sled[] = []
  /** The snowmobile ridden, by id, or 0 on foot. */
  private riding = 0
  private driven: DrivenPose | null = null
  private seq = 0
  /** Ticks run so far: the store's clock. */
  private tick = 0
  private accumulator = 0
  private lastFrame = 0
  private frameHandle = 0
  private reportedAt = 0
  private crouching = false
  private polingHard = false
  private disposed = false
  private readonly seatAt = new Vector3()
  private readonly seatTurn = new Quaternion()
  private readonly cameraAt = new Vector3()
  private fxWorldMade: FxWorld | null = null
  /** Whether the visitor's keys and mouse go to the game: the element is in use. */
  active = false

  constructor(
    container: HTMLElement,
    private readonly base: string,
    private readonly course: Course,
    private readonly onReport: (report: Report) => void,
  ) {
    this.scene = new Scene(container)
    this.input = new InputController(this.scene.canvas)
    this.input.yaw = course.start.yaw
    this.cameras = new Cameras(this.scene, this.input, (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null)
    this.view = new WorldView(this.scene.scene, this.debris)
    this.view.damage.groundAt = (x, z) => (this.terrain ? this.terrain.heightAt(x, z) : null)
    this.view.damage.looks = LookWorker.create()
    this.surroundings = new Surroundings(this.scene, this.effects, () => this.fxWorld)
    this.progress = new Progress(course)
    this.clock = new RaceClock(course)
    this.clockProgress = new Progress(course)
    this.courseView = new CourseView(course, (x, z) => this.terrain?.sample(x, z) ?? null)
    const scene = this.scene.scene
    scene.add(this.crowd.root, this.effects.root, this.view.damage.root, this.rockets.root, this.trails.root)
    scene.add(this.smoke.root, this.glow.root, this.blood.root, this.skids.root, this.debris.root, this.view.waterRoot, this.courseView.root)
    this.smoke.groundAt = this.glow.groundAt = this.debris.groundAt = (x, z) => this.scene.groundAt(x, z)
    const world: VehicleWorld = {
      root: scene, sounds: this.sounds, effects: this.effects, rounds: this.rounds, debris: this.debris, lights: scene.lights, fx: () => this.fxWorld,
      castRay: (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null,
      driven: () => (this.driven && this.riding ? { id: this.riding, pose: this.driven, jets: 0 } : null),
      seated: () => this.riding || null,
      listen: () => this.listen(),
      ownShot: () => {},
    }
    this.vehicles = this.machines.add(new Vehicles(world, this.machines.add(new Seating())))
  }

  async start() {
    await this.scene.init()
    if (this.disposed) return
    const response = await fetch(`${this.base}world.msgpack`)
    if (!response.ok) throw new Error(`world.msgpack: ${response.status}`)
    const bytes = new Uint8Array(await response.arrayBuffer())
    const world = decodeManifest(bytes)
    world.bytes = bytes
    this.world = world
    if (world.theme === THEME_WINTER) this.startWinter()
    this.view.world = world
    const backdrop = buildGround(world.halfExtent, world.groundCell > 0 ? -0.05 : 0)
    this.scene.scene.add(backdrop)
    if (world.groundCell > 0) this.view.backdrop = backdrop
    this.terrain = this.view.terrain = new Terrain(world.halfExtent, world.tileSize)
    this.surfaces = this.view.surfaces = new Surfaces(world.halfExtent, world.tileSize)
    this.waters = this.view.waters = this.surroundings.waters = new Waters(world.halfExtent, world.tileSize)
    this.waters.calm = true
    this.scene.groundAt = (x, z) => this.terrain?.heightAt(x, z) ?? 0
    const terrain = this.terrain
    this.grass = this.view.grass = this.surroundings.grass = new Grass(world.halfExtent, world.tileSize, (x, z) => terrain.sample(x, z))
    this.grass.setDensity(GRASS_LITE)
    this.grass.setEnabled(world.theme !== THEME_WINTER && this.scene.look.grass)
    this.scene.scene.add(this.grass.root)
    this.stones = this.view.stones = new Stones(world.halfExtent, world.tileSize, (x, z) => terrain.sample(x, z))
    this.scene.scene.add(this.stones.root)
    const created = await SimMotor.create(bytes)
    // Left while the physics was loading: it is not wanted.
    if (this.disposed) return created.dispose()
    const motor = (this.motor = this.view.motor = created)
    this.driver = new VehicleDriver(motor)
    this.ragdolls = new Ragdolls(motor)
    await Promise.all([this.crowd.load(), this.loadSnowmobile()])
    if (this.disposed) return
    this.tiles = new TileStreamer(world, groomCourse(this.course, world, {
      prepare: (ref, tile) => this.view.prepareTile(ref, tile),
      attach: (ref, tile) => this.view.attachTile(ref, tile),
      discard: (ref) => this.view.discardTile(ref),
    }))
    this.tiles.setReach(this.scene.quality.loadRadius, this.scene.quality.unloadRadius)
    // A few metres behind the start line, so the arch is in view ahead.
    const { yaw } = this.course.start
    const x = this.course.start.x + Math.sin(yaw) * BEHIND_START
    const z = this.course.start.z + Math.cos(yaw) * BEHIND_START
    this.view.focus(x, z)
    await this.tiles.prime(x, z)
    if (this.disposed) return
    this.parkSleds()
    this.predictor = new Predictor(motor.stepper, {
      position: [x, terrain.heightAt(x, z) + STANDING_Y, z], yaw, vy: 0, grounded: true, knock: [0, 0], ride: 0, prone: false, chute: false,
      spent: 0, winded: false, lost: 0, down: 0, skis: true, skiStrain: 0, swimming: false,
    })
    this.avatars.local()
    this.scene.setTimeOfDay(TIME_OF_DAY)
    this.lastFrame = performance.now()
    this.frameHandle = requestAnimationFrame((now) => this.frame(now))
  }

  /** The snowmobile's model alone: the game's own loader fetches every car's. */
  private async loadSnowmobile() {
    const url = `${this.base}models/snowmobile.glb`
    const response = await fetch(url)
    if (!response.ok) throw new Error(`could not load ${url}: ${response.status}`)
    await parseCarModel('snowmobile', await response.arrayBuffer())
  }

  /** Called on the click that starts the game: browsers only let sound start from one. */
  unlockAudio() {
    audioContext()
  }

  private startWinter() {
    setWinter(true)
    snowOnBuildings()
    freezeWater()
    this.scene.setLook(this.scene.look)
    this.snowfall = new Snowfall()
    this.skiTracks = new SkiTracks((x, z) => this.scene.groundAt(x, z), this.smoke)
    this.snowRoost = new SnowRoost(this.glow, (x, z) => this.scene.groundAt(x, z))
    if (this.fxWorldMade) this.fxWorldMade.roost = this.snowRoost
    this.scene.scene.add(this.snowfall.root, this.skiTracks.marks.root, this.snowRoost.root)
  }

  /**
   * Puts the snowmobiles where the course says, each settled onto the snow by the physics first, and
   * makes them solid to the skier.
   */
  private parkSleds() {
    const motor = this.motor!
    this.course.snowmobiles.forEach((place, index) => {
      const id = FIRST_SLED + index
      const ground = this.terrain!.heightAt(place.x, place.z)
      motor.startCar(id, KIND_SNOWMOBILE, [place.x, ground + 1, place.z], place.yaw)
      for (let step = 0; step < SETTLE_STEPS; step++) motor.driveCar(id, { throttle: 0, steer: 0, handbrake: true, lift: 0, powered: false }, TICK_MS / 1000)
      const state = motor.carState(id)
      motor.stopCar(id)
      const position = state?.position ?? [place.x, ground, place.z]
      const rotation = state?.rotation ?? yawQuaternion(place.yaw)
      this.sleds.push({ id, pose: poseOfSled(id, position, rotation) })
    })
    this.updateObstacles()
  }

  /** The parked snowmobiles are solid to the skier and the one ridden; the one ridden is simulated. */
  private updateObstacles() {
    const parked: Obstacle[] = this.sleds.filter((sled) => sled.id !== this.riding).map(({ pose }) => ({ ...pose }))
    this.motor?.setVehicles(parked)
  }

  private frame(now: number) {
    if (this.disposed) return
    this.frameHandle = requestAnimationFrame((t) => this.frame(t))
    const dt = Math.min(0.25, (now - this.lastFrame) / 1000)
    this.lastFrame = now
    this.runTicks(dt)
    const alpha = this.accumulator / TICK_MS
    const eye = this.scene.camera.position
    this.view.update(dt, this.effects, this.cameraAt.set(eye.x, eye.y, eye.z), this.fxWorld)

    this.driven = this.riding ? (this.driver?.pose(alpha, dt) ?? null) : null
    const renderTick = this.tick - 1 + alpha
    const sampled = this.store.sampleAll(renderTick)
    const entityFrame: Frame = { tick: renderTick, dt, now, localId: LOCAL_ID }
    this.machines.run(sampled, entityFrame)

    const predictor = this.predictor!
    const local = predictor.state
    const position = predictor.renderPosition(alpha, dt)
    const velocity: [number, number] = [(local.position[0] - predictor.previous.position[0]) / (TICK_MS / 1000), (local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)]
    const ride = this.riding ? this.vehicles.placed.get(this.riding) : undefined
    this.poseLocal(dt, position, local, velocity, ride !== undefined)
    this.aimCamera(now, dt, position, local)
    const [fx, fz] = ride ? [ride.x, ride.z] : [position[0], position[2]]
    this.tiles?.update(fx, fz)
    this.courseView.update(fx, fz, dt)
    this.updateEffects(dt)

    this.scene.contact.begin()
    if (!ride) this.surroundings.blobFoot(position[0], position[1], position[2])
    this.surroundings.blobVehicles(this.vehicles.placed)
    this.stones?.update(eye.x, eye.z)
    this.tumble(position, local, ride !== undefined, dt)
    this.avatars.sweep((key) => this.surroundings.dry(key))
    this.scene.contact.end()
    this.surroundings.mirrorWater()
    this.scene.render()

    if (now - this.reportedAt > REPORT_EVERY) {
      this.reportedAt = now
      this.report(fx, fz, ride ? ride.yaw : this.input.yaw, ride ? null : velocity, local)
    }
  }

  private runTicks(dt: number) {
    this.accumulator += dt * 1000
    let ticks = 0
    while (this.accumulator >= TICK_MS && ticks < MAX_TICKS_PER_FRAME) {
      this.step()
      this.accumulator -= TICK_MS
      ticks++
    }
    if (ticks === MAX_TICKS_PER_FRAME) this.accumulator = 0
  }

  /** One fixed tick: the visitor's input to the skier or the snowmobile, and the snowmobiles to the store. */
  private step() {
    const sample = this.input.sample()
    // Keys pressed while the visitor is elsewhere on the page are not the game's.
    const live = this.active
    const buttons = live ? sample.buttons : 0
    this.crouching = (buttons & BUTTON_CROUCH) !== 0
    this.polingHard = live && sample.forward > 0 && (buttons & BUTTON_RUN) !== 0
    const frame: InputFrame = {
      seq: ++this.seq,
      strafe: live ? toAxis(sample.strafe) : 0,
      forward: live ? toAxis(sample.forward) : 0,
      yaw: toMrad(sample.yaw),
      buttons,
      pitch: toMrad(sample.pitch),
      view: 0,
      slot: sample.slot,
    }
    const predictor = this.predictor!
    if (buttons & BUTTON_USE) this.toggleRide()
    if (buttons & BUTTON_SKIS && !this.riding) this.switchSkis()
    if (this.riding) this.driver!.apply(frame)
    else predictor.apply(frame)
    // Nothing is ever acknowledged by a server: forget the inputs as they are made.
    predictor.acknowledge(frame.seq)
    this.driver!.acknowledge(frame.seq)
    this.tick++
    this.pushScene()
    this.time()
  }

  /** Moves the race clock on with where the skier, or their snowmobile, is after this tick. */
  private time() {
    const sled = this.riding ? this.sleds.find((s) => s.id === this.riding) : undefined
    const [x, z] = sled ? [sled.pose.x, sled.pose.z] : [this.predictor!.state.position[0], this.predictor!.state.position[2]]
    if (this.clockAt) {
      const { along } = this.clockProgress.at(x, z)
      this.clock.update(this.clockAt[0], this.clockAt[1], x, z, along, this.seconds(), this.riding !== 0)
    }
    this.clockAt = [x, z]
  }

  /** Seconds of play, counted in ticks, so a run is timed by the sim and not by the frame rate. */
  private seconds(): number {
    return (this.tick * TICK_MS) / 1000
  }

  /**
   * Skis on or off, as the server allows it (`mechanics/movement.rs`): only nearly still, and only with
   * legs. Off, the skier walks.
   */
  private switchSkis() {
    const state = this.predictor!.state
    if (Math.hypot(state.knock[0], state.knock[1]) >= SKI_CLIP_SPEED) return
    state.skis = !state.skis
    state.knock = [0, 0]
  }

  /** The nearest parked snowmobile within reach of the skier, if any. */
  private sledNear(): Sled | undefined {
    const [x, , z] = this.predictor!.state.position
    let best: Sled | undefined
    let bestDistance = REACH_SLED
    for (const sled of this.sleds) {
      const d = Math.hypot(sled.pose.x - x, sled.pose.z - z)
      if (d < bestDistance) {
        bestDistance = d
        best = sled
      }
    }
    return best
  }

  /** E: gets on the snowmobile in reach, or off the one ridden, beside it. */
  private toggleRide() {
    const driver = this.driver!
    const predictor = this.predictor!
    if (this.riding) {
      const sled = this.sleds.find((s) => s.id === this.riding)!
      const state = this.motor!.carState(sled.id)
      driver.leave()
      if (state) sled.pose = poseOfSled(sled.id, state.position, state.rotation)
      this.riding = 0
      this.updateObstacles()
      const turn = new Quaternion(sled.pose.qx, sled.pose.qy, sled.pose.qz, sled.pose.qw)
      const side = new Vector3(STEP_OFF, 0, 0).applyQuaternion(turn)
      const x = sled.pose.x + side.x
      const z = sled.pose.z + side.z
      const standing: CharacterState = {
        ...predictor.state, position: [x, this.terrain!.heightAt(x, z) + STANDING_Y, z], vy: 0, knock: [0, 0], grounded: true, ride: 0,
      }
      predictor.state = standing
      predictor.previous = { ...standing, position: [...standing.position], knock: [0, 0] }
      return
    }
    const sled = this.sledNear()
    if (!sled) return
    this.riding = sled.id
    this.updateObstacles()
    driver.reconcile(this.seq, sled.pose, STILL_DRIVE)
  }

  /**
   * The snowmobiles, and the skier sitting on one, as a snapshot would bring them: the presenters draw
   * them from the store. Pushed every tick, since the store forgets what it has not heard of lately.
   */
  private pushScene() {
    const scene = new Map<number, Components>()
    for (const sled of this.sleds) {
      if (sled.id === this.riding) {
        const state = this.motor!.carState(sled.id)
        if (state) sled.pose = poseOfSled(sled.id, state.position, state.rotation)
      }
      scene.set(sled.id, wireOf(sled.pose, sled.id === this.riding ? (this.driven?.steer ?? 0) : 0))
    }
    if (this.riding) scene.set(LOCAL_ID, { seated: { vehicle: this.riding, seat: 0 } })
    this.store.push(this.tick, scene, true)
  }

  /** Draws the local skier: on their skis, or sat on the snowmobile. */
  private poseLocal(dt: number, position: readonly number[], local: CharacterState, [vx, vz]: [number, number], seated: boolean) {
    this.avatars.begin()
    const me = this.avatars.local()
    me.dead = false
    me.held = 0
    me.swim = 0
    me.skis = local.skis && !seated
    me.skiAim = false
    me.skiPoling = this.polingHard && !this.crouching && !local.winded && local.grounded
    if (seated) {
      const model = this.vehicles.model(this.riding)
      const ride = this.vehicles.placed.get(this.riding)
      const shown = model !== undefined && ride !== undefined && seatFrame(model, ride.kind, 0, this.seatAt, this.seatTurn)
      me.hidden = !shown
      if (shown) me.seat(this.seatAt, this.seatTurn, this.input.yaw, this.input.pitch, dt, seatView(KIND_SNOWMOBILE, 0).stand === true)
      this.skiTracks?.lift(LOCAL_TRACK)
      this.surroundings.dry(0)
      this.sounds.glide(0)
      return
    }
    me.hidden = false
    const sliding = this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED
    me.update(position, this.input.yaw, vx, vz, local.grounded, dt, this.input.pitch, this.crouching, sliding, local.prone || local.down > 0, local.chute, local.vy)
    this.surroundings.wade(0, position[0]!, position[1]!, position[2]!, Math.hypot(vx, vz), dt)
    if (local.skis) this.skiTracks?.run(LOCAL_TRACK, position[0]!, position[2]!, vx, vz, local.grounded, dt)
    else this.skiTracks?.lift(LOCAL_TRACK)
    this.sounds.glide(local.skis && local.grounded ? Math.hypot(local.knock[0], local.knock[1]) : 0)
  }

  /**
   * A fall (the sim's knockdown, `down`) thrown as a ragdoll that tumbles in the snow, as the game draws
   * it (`Bodies.knockdowns`), with the camera kicked as it starts.
   */
  private tumble(position: readonly number[], local: CharacterState, seated: boolean, dt: number) {
    const body = this.avatars.of(LOCAL_ID)
    if (!this.ragdolls || !body) return
    const down = local.down > 0 && !seated
    if (down && !this.wasDown) this.chase.kick(Math.min(0.6, Math.hypot(local.knock[0], local.knock[1]) / 30))
    this.wasDown = down
    const eye = this.scene.camera.position
    this.ragdolls.update([{
      key: 0, body, down, dead: false, position: [position[0]!, position[1]!, position[2]!], velocity: [local.knock[0], local.vy, local.knock[1]], lost: local.lost,
    }], [eye.x, eye.y, eye.z], dt)
  }

  /** Behind the skier's shoulder on foot, the game's chase camera on a snowmobile. */
  private aimCamera(now: number, dt: number, position: readonly number[], local: CharacterState) {
    const ride = this.riding ? this.vehicles.placed.get(this.riding) : undefined
    if (ride) {
      const view = seatView(ride.kind, 0)
      const distance = rideView(ride.kind).distance
      const chase = view.chase ? this.chase.update([ride.x, ride.y, ride.z], ride.yaw, dt, distance) : null
      this.scene.zoomTo('none', dt, chase?.fovBoost ?? 0)
      this.cameras.seated({
        now, dt, vehicle: this.riding, seat: view, ride, model: this.vehicles.model(this.riding), motion: this.vehicles.fx(this.riding),
        distance, sighted: false, scoped: false, aimBlend: 0, chase, aimHeld: false, head: null,
      }, 0)
      return
    }
    this.chase.reset()
    this.cameras.leave(now)
    const skiing = local.skis ? Math.hypot(local.knock[0], local.knock[1]) : 0
    this.scene.zoomTo('none', dt, Math.max(0, skiing - SKI_FOV_FROM) * SKI_FOV_PER_SPEED)
    this.scene.overShoulder(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE, ON_FOOT_SHOULDER, ON_FOOT_HEIGHT, this.chase.tremble(dt))
  }

  private updateEffects(dt: number) {
    const eye = this.scene.camera.position
    this.effects.update(dt)
    this.rounds.update(dt)
    this.trails.update(dt)
    this.scene.scene.lights.update(dt)
    this.snowfall?.update(eye, dt)
    this.snowRoost?.update(eye, dt)
    this.skiTracks?.update(dt)
    this.smoke.follow(eye.x, eye.z)
    this.glow.follow(eye.x, eye.z)
    this.smoke.update(dt)
    this.glow.update(dt)
    this.blood.update(dt)
    this.skids.update(dt)
    this.debris.update(dt)
  }

  private report(x: number, z: number, yaw: number, velocity: [number, number] | null, local: CharacterState) {
    const { along, off } = this.progress.at(x, z)
    const ride = this.riding ? this.vehicles.fx(this.riding) : undefined
    const speed = ride ? Math.hypot(ride.velocity.x, ride.velocity.z) : Math.hypot(...(velocity ?? [0, 0]))
    this.onReport({
      at: { x, z, yaw }, along, off, speed: speed * 3.6, riding: this.riding !== 0, skis: local.skis, nearSled: !this.riding && this.sledNear() !== undefined,
      snowmobiles: this.sleds.filter((s) => s.id !== this.riding).map(({ pose }) => ({ x: pose.x, z: pose.z, yaw: 0 })),
      clock: this.clock.read(this.seconds()),
    })
  }

  /** Puts the ears where the camera is. */
  private listen() {
    const camera = this.scene.camera
    const at = camera.position
    const look = camera.getWorldDirection(new Vector3())
    const moving = this.riding ? this.vehicles.fx(this.riding)?.velocity : undefined
    this.sounds.setListener([at.x, at.y, at.z], [look.x, look.y, look.z], moving ? [moving.x, moving.y, moving.z] : [0, 0, 0])
  }

  /** What the snowmobiles' track and spray effects draw into. */
  private get fxWorld(): FxWorld {
    this.fxWorldMade ??= {
      smoke: this.smoke,
      glow: this.glow,
      skids: this.skids,
      groundAt: (x, z) => this.scene.groundAt(x, z),
      surfaceAt: (x, z) => this.surfaces?.at(x, z) ?? SURFACE_GRASS,
      waterAt: (x, z) => this.waters?.surfaceAt(x, z, 0) ?? NO_WATER,
      camera: this.scene.camera.position,
      roost: this.snowRoost,
    }
    return this.fxWorldMade
  }

  /** The canvas the game draws on, to lock the pointer to. */
  get canvas(): HTMLCanvasElement {
    return this.scene.canvas
  }

  dispose() {
    this.disposed = true
    cancelAnimationFrame(this.frameHandle)
    this.input.dispose()
    // The tiles first: letting them go takes them out of the physics, which must still be there.
    this.tiles?.dispose()
    this.driver?.dispose()
    this.motor?.dispose()
    this.motor = null
    this.crowd.dispose()
    this.effects.dispose()
    this.smoke.dispose()
    this.glow.dispose()
    this.blood.dispose()
    this.snowfall?.dispose()
    this.snowRoost?.dispose()
    this.skiTracks?.dispose()
    this.skids.dispose()
    this.sounds.dispose()
    this.debris.dispose()
    this.view.dispose()
    this.rockets.dispose()
    this.trails.dispose()
    this.grass?.dispose()
    this.stones?.dispose()
    this.scene.dispose()
  }
}

/** A turn of `yaw` about the vertical, soldier convention. */
function yawQuaternion(yaw: number): [number, number, number, number] {
  return [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)]
}

function poseOfSled(id: number, position: readonly number[], rotation: readonly number[]): VehiclePose {
  const [qx, qy, qz, qw] = rotation as [number, number, number, number]
  return { id, kind: KIND_SNOWMOBILE, x: position[0]!, y: position[1]!, z: position[2]!, qx, qy, qz, qw, hp: vehicleMaxHp(KIND_SNOWMOBILE), damage: new Array(11).fill(0), air: null }
}

/** A snowmobile's pose as the wire carries it (centimetres, a quaternion with its `w` dropped), for the store. */
function wireOf(pose: VehiclePose, steer: number): Components {
  // The wire's quaternion has a non-negative `w`, rebuilt from the other three.
  const sign = pose.qw < 0 ? -1 : 1
  return {
    kind: { kind: KIND_SNOWMOBILE },
    place: { x: Math.round(pose.x * 100), y: Math.round(pose.y * 100), z: Math.round(pose.z * 100) },
    orientation: { qx: Math.round(sign * pose.qx * 32767), qy: Math.round(sign * pose.qy * 32767), qz: Math.round(sign * pose.qz * 32767) },
    steer: { steer: Math.round(steer * 127) },
    health: { hp: pose.hp },
  }
}
