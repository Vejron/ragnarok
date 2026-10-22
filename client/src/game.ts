import type { Node } from '@ragnarok/engine'
import * as THREE from 'three'
import {
  ARMS_CUT, ARMS_JAMMED, ARMS_REAR_JAMMED, DAMAGE_TURRET, DAMAGE_WHEELS, encodeInput, encodePing, isDrone, KIND_DRONE, KIND_HELI, KIND_LOITER, KIND_HEN, KIND_PLANE, KIND_MORTAR, KIND_ROADSTER, KIND_ROOSTER, KIND_SPORTS, KIND_STICKMAN, KIND_STRV, KIND_TANK, KIND_WALKER, sceneStates, toAxis, toMrad, TURRET_GONE, WEAPON_GRENADE, WEAPON_MORTAR, WEAPON_RICOCHET, WEAPON_TURRET,
  PROJECTILE_BULLET, PROJECTILE_FLARE, WEAPON_BULLET, WEAPON_FLARE, WEAPON_AUTOCANNON, WEAPON_DRONE, WEAPON_LOITER, WEAPON_BOMB, WEAPON_EGG, WEAPON_CRASH, WEAPON_MG, WEAPON_PELLET, WEAPON_ROCKET, WEAPON_SCRAPE, WEAPON_SHELL, WEAPON_SHOTGUN, WEAPON_SNIPER, WEAPON_WRECK,
  type Arms, type BulletSpawn, type DriveState, type EntityState, type InputFrame, type PickupState, type RocketState, type Rider, type ServerMessage, type ShotEvent, type Turret, type VehicleState,
} from './net/codec'
import { blendEntity, blendRocket, blendVehicle, Interpolator } from './net/interp'
import { Backoff } from './net/backoff'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport } from './net/transport'
import {
  BUTTON_CROUCH, BUTTON_FIRE, BUTTON_RELOAD, CROUCH_EYE_DROP, PRONE_EYE_HEIGHT, SLIDE_MIN_SPEED, ENTER_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, INPUT_REDUNDANCY, PLAYER_MAX_HP, PROTOCOL_VERSION, RESPAWN_TICKS,
  ROCKET_BLAST_RADIUS, SHELL_COOLDOWN_TICKS, STANDING_Y,
  CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS,
  HELI_GUN_COOLDOWN_TICKS, HELI_GUN_LENGTH, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT, CANNON_RADIUS,
  SHELL_SPEED, GRAVITY, MORTAR_BARREL_LENGTH, MORTAR_COOLDOWN_TICKS, MORTAR_RADIUS, MORTAR_SHELL_SPEED, MORTAR_SHELL_TTL_TICKS, STRV_BARREL_LENGTH, STRV_BARREL_MAX, STRV_BARREL_MIN, TANK_BARREL_LENGTH, TANK_BARREL_MAX, TANK_BARREL_MIN, WALKER_GUN_COOLDOWN_TICKS, WALKER_GUN_LENGTH, WALKER_GUN_PITCH_MAX, WALKER_GUN_PITCH_MIN, WALKER_LEG_LENGTH, WALKER_TORSO_LIMIT, TICK_MS, TICK_RATE, vehicleMaxHp, WRECK_BLAST_RADIUS,
  PASSENGER_DISTANCE, PASSENGER_HEIGHT, PASSENGER_SHOULDER,
  BOMB_RADIUS, PLANE_AIM_MAX_PITCH, PLANE_CRUISE_SPEED, PLANE_BOMB_MIN_HEIGHT, PLANE_BOMB_PIVOT, PLANE_GUN_RANGE, PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN, PLANE_STALL_SPEED,
} from './sim/consts'
import { crewGun, isLegged, seatHead, vehicleDef } from './sim/vehicles'
import { aimSpeedScale, DRONE_BLAST_RADIUS, DRONE_RANGE, LOITER_BLAST_RADIUS, LOITER_RANGE, emptyHotbar, GRENADE_BLAST_RADIUS, HOTBAR_SLOTS, hitscanGunFor, ITEM_NONE, ITEM_SNIPER, itemDef, PICKUP_RADIUS, type ItemDef, type ItemSlot } from './sim/items'
import { InputController } from './sim/input'
import { SimMotor } from './sim/motor'
import { Terrain } from './sim/terrain'
import { VehicleDriver, type DrivenPose } from './sim/driver'
import { Predictor } from './sim/predict'
import type { Vec3 } from './sim/motor'
import { CarModel, type VehicleModel } from './render/car'
import { Effects } from './render/effects'
import { dayPhase } from './render/daynight'
import { DamageNumbers } from './render/damagenumbers'
import { Particles } from './render/particles'
import { Skidmarks } from './render/skidmarks'
import { coldGas, crashBurst, scrapeSparks, VehicleFx, type FxWorld } from './render/tyres'
import { Debris } from './render/debris'
import { damageSmoke, rocketTrail } from './render/vehiclesmoke'
import { BLASTS, explosion, type ExplosionProfile } from './render/explosion'
import { Gore } from './render/gore'
import { Surfaces, SURFACE_GRASS } from './sim/surface'
import { Waters } from './sim/waters'
import { splash } from './render/splash'
import { Pickups } from './render/pickups'
import { TrajectoryView } from './render/trajectory'
import { MortarArc } from './render/mortar-arc'
import { dropTrajectory, throwTrajectory } from './sim/grenade'
import { ProjectileTrails } from './render/trails'
import { Rockets } from './render/rockets'
import { bulletLifeTicks, bulletPosition, bulletTicksTo, bulletVelocity } from './sim/ballistics'
import { saveLook, type Look } from './render/look'
import { Grass, GRASS_FULL, GRASS_LITE, type Bender } from './render/grass'
import { halfExtentsOf } from './sim/kinds'
import { AIM_DISTANCE_SCALE, CAMERA_HEIGHT, ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, PLANE_CAMERA_HEIGHT, PLANE_DISTANCE, Scene, STRV_AIM_BACK, STRV_AIM_UP, WALKER_AIM_BACK, WALKER_AIM_UP } from './render/scene'
import { ChaseCam } from './render/chase'
import { PerfReadout } from './render/perf'
import { HeliModel } from './render/heli'
import { encodeRtc, wrapAngle } from './net/codec'
import { VoiceChat } from './voice'
import { PlaneModel } from './render/plane'
import { StrvModel } from './render/strvs'
import { MortarModel } from './render/mortar'
import type { Mark } from './render/gunsight'
import { TankModel } from './render/tank'
import { WalkerModel } from './render/walker'
import { HenModel } from './render/hen'
import { cameraDistance, createVehicleModel, loadVehicleModels } from './render/vehicles'
import { Character, Crowd } from './render/character'
import { loadRoosterModel, RoosterModel } from './render/rooster'
import { playCrow } from './audio'
import { VehicleSounds } from './vehicleaudio'
import { HIT_DEALT, HIT_KILL, HIT_ROOSTER, HIT_TAKEN, HIT_VEHICLE, HitFeedback } from './feedback'
import { buildGround } from './render/world'
import { v1Scene } from './net/adapter'
import { Replica } from './net/replica'
import { TileStreamer } from './stream'
import { loadManifest, NO_WATER, type WorldData } from './world'
import { WorldView } from './worldview'
import type { HudStats } from './hud'
import { Notices } from './notices'
import { Scoreboard, sides } from './scoreboard'
import { Blips } from './blips'
import { allies, colorFor, cssColor, forgetAllPlayers, forgetPlayer, isBot, loadProfile, profileQuery, setPlayer, teamFor } from './profile'

// The overlay and its tests import these from here.
export type { HudStats } from './hud'
export type { FeedEntry, ScoreRow, Side } from './scoreboard'
export type { Blip } from './blips'

/** How the minimap tells sides apart. */
/** Feet this deep (metres) under the surface are in the water. */
const WADE_DEPTH = 0.15
/** A burning flare's light: warm white, strong enough to light a field from where it hangs. */
const FLARE_LIGHT = 0xffe8c8
const FLARE_INTENSITY = 2
const FLARE_LIGHT_RADIUS = 120
/** Falling this high (metres) over the ground, a hint offers the parachute. */
const CHUTE_PROMPT_HEIGHT = 4
/** Wading ripples are drawn within this many metres of the camera. */
const WADE_RIPPLE_RANGE = 60
const MAP_SIDES = { colorOf: (id: number) => cssColor(colorFor(id)), isBot, allies }

interface CarPlacement {
  kind: number
  turret: Turret | null
  arms: Arms | null
  x: number
  y: number
  z: number
  /** Heading of the nose, stickman convention. */
  yaw: number
  seats: readonly number[]
  /** Where each rider looks and what they hold. */
  crew: readonly Rider[]
}

/** Players sitting in cars, which the stickman list does not include. */
/** How far behind a vehicle the camera sits. */
/** How quickly a plane's camera follows the mouse's aim, per second, how far it pulls back at cruise speed (m) and how much of the plane's roll it takes. */
const PLANE_VIEW_EASE = 5
const PLANE_PULLBACK = 3
const PLANE_VIEW_ROLL = 0.3
const clamp01 = (value: number) => Math.min(0.94, Math.max(0.06, value))
/** A vehicle with no motion on record. */
const STILL = new THREE.Vector3()

/** Players sitting in vehicles who are not already counted in `seen` (drawn sitting), nor `local`. */
const riders = (cars: ReadonlyMap<number, CarPlacement>, seen: ReadonlySet<number>, local: number | null) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0 && rider !== local && !seen.has(rider)) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
/** A crow can be heard this far off, metres. */
const ROOSTER_HEARING = 45
/** A bullet in flight on this client. */
interface LiveBullet {
  spawn: BulletSpawn
  /** Ticks it flies before it gives up. */
  life: number
  /** Ticks after firing at which its end event says it stopped, once known. */
  endTicks: number | null
  /** Its first frame has played: arm, flash and sound. */
  shown: boolean
  /** Muzzle minus eye, closing as it flies. */
  offset: THREE.Vector3 | null
}

/** Mouse-look is this much slower down a scope, to match the narrower view. */
const SCOPE_LOOK_SCALE = 0.3
/** Looking down a tank's sight is as slow as a rifle scope. */
const SIGHT_LOOK_SCALE = 0.35
/** The sight sits this far above the gun's pivot, on the mantlet. */
const SIGHT_EYE_UP = 0.5
/** Ranges (m) the sight's ladder is marked at. */
const SIGHT_RANGES = [100, 150, 200, 250, 300, 350, 400, 450, 500]
const SIGHT_RAY_START = 6
const SIGHT_RANGE_MAX = 1500
/** And a little slower aiming a gun over the shoulder. */
const AIM_LOOK_SCALE = 0.6
/** How long the static shows after a drone's feed cuts out, ms. */
const FEED_LOST_MS = 700
/** A loitering drone's look is kept between these pitches, radians: steep enough to dive, not to loop. */
const LOITER_PITCH_MIN = -1.2
const LOITER_PITCH_MAX = 0.6

/**
 * The client game loop. Fixed 60 Hz ticks sample input, predict the local player and send inputs;
 * every animation frame renders the predicted local player and interpolated remote players.
 */
export class Game {
  private readonly scene: Scene
  readonly input: InputController
  private readonly transport: Transport
  /** Proximity voice chat, peer to peer; the server only relays its signaling. */
  private readonly voice = new VoiceChat((peer, kind, json) => this.transport.sendReliable(encodeRtc(peer, kind, json)))
  private readonly clock = new ServerClock()
  private readonly rtt = new RttEstimator()
  private readonly interp = new Interpolator<EntityState>(blendEntity)
  private readonly vehicleInterp = new Interpolator<VehicleState>(blendVehicle)
  /** Every stickman is drawn by this one set of instanced meshes. */
  private readonly crowd = new Crowd()
  private readonly avatars = new Map<number, Character>()
  private readonly roosters = new Map<number, RoosterModel>()
  /** Rebuilds snapshots from deltas, and says which one to diff the next against. */
  private readonly replica = new Replica()
  private tiles: TileStreamer | null = null
  /** The streamed tiles, what is solid in them, and what the server has brought down. */
  private readonly view: WorldView
  private world: WorldData | null = null
  private terrain: Terrain | null = null
  private surfaces: Surfaces | null = null
  private grass: Grass | null = null
  private readonly benders: Bender[] = []
  private waters: Waters | null = null
  /** True from the welcome until the joiner's catch-up `destroyed` arrives: that one is state, not news. */
  private awaitingCatchUp = false
  private readonly cars = new Map<number, VehicleModel>()
  private readonly effects = new Effects()
  private readonly damageNumbers = new DamageNumbers()
  /** Tyre smoke, dust, gravel and spray; sparks and flames glow on their own pool. */
  private readonly smoke = new Particles(4096, false, true)
  private readonly glow = new Particles(2048, true)
  private readonly skids = new Skidmarks()
  private readonly vehicleFx = new Map<number, VehicleFx>()
  /** Engines, tyres, crashes and guns. */
  private readonly sounds = new VehicleSounds()
  /** Bumpers, doors and wheels that came off. */
  private readonly debris = new Debris(160)
  private readonly gore = new Gore(this.debris, this.smoke, this.effects)
  /** Seconds since the last frame, for what is drawn per vehicle. */
  private frameDt = 0
  private readonly rocketInterp = new Interpolator<RocketState>(blendRocket)
  private readonly rocketModels = new Rockets()
  /** Ribbons behind rockets, shells and bullets in flight. */
  private readonly trails = new ProjectileTrails()
  /** The tick a rocket was last listed in, once it stops being listed: it bursts then. */
  private readonly rocketEnds = new Map<number, number>()
  /** Shot and burst events wait here until the render tick reaches them, so they line up with what is drawn. */
  private pendingShots: { tick: number; shot: ShotEvent }[] = []
  /** Bullets in flight, flown from their spawn alone (`sim/ballistics.ts`); the server lists none after announcing them. */
  private readonly bullets = new Map<number, LiveBullet>()
  /** Bullets already seen, with the tick they were fired on: a spawn is sent twice and must not start a second round. */
  private readonly seenBullets = new Map<number, number>()
  /** Fraction of a smoke puff each damaged vehicle owes, carried between frames. */
  private readonly smokeOwed = new Map<number, number>()
  private readonly chase = new ChaseCam()
  private readonly pickupModels = new Pickups()
  /** The arc a grenade in hand would take, drawn while one is held. */
  private readonly trajectory = new TrajectoryView()
  private readonly mortarArc = new MortarArc()
  /** The items on the ground in the newest snapshot, for the prompt and the drawing. */
  private pickups: PickupState[] = []
  /** The local player's hotbar and reload/bandage progress, from the newest snapshot. */
  private hotbar: ItemSlot[] = emptyHotbar()
  private progress = 0
  private firedBefore = false
  /** Whether the last sampled input held crouch, for the local avatar. */
  private crouching = false
  /** Looking down a scope this frame. */
  private scoped = false
  /** Aiming a gun over the shoulder this frame, and how far the camera has eased in (0..1). */
  private aiming = false
  private aimBlend = 0
  /** The last shot counter seen for each gun, so a change is a new shell. */
  private readonly shots = new Map<number, number>()
  /** A plane's forward guns' shot counter as last drawn, by vehicle. */
  private readonly forwardShots = new Map<number, number>()
  /** A plane's arms as last drawn, by vehicle. */
  private readonly lastArms = new Map<number, Arms>()
  private lastShotAt = -Infinity
  /** Scratch for a seat's head point and its vehicle's turn. */
  private readonly seatAt = new THREE.Vector3()
  private readonly seatTurn = new THREE.Quaternion()
  /** Tanks whose turret has already been sent flying, so it is thrown once. */
  private readonly turretsShed = new Set<number>()
  /** The local player's health, from the newest snapshot. Zero is dead. */
  private hp = PLAYER_MAX_HP
  /** `performance.now()` when the local player died, or null while alive. */
  private diedAt: number | null = null
  private lastFireAt = -Infinity
  /** Kills and deaths of everyone, kept up to date by the server's kill events. */
  private readonly board = new Scoreboard()
  /** Allies and recent gunfire, for the minimap. */
  private readonly blips = new Blips()
  private readonly notices = new Notices()
  /** Hits dealt and taken, for the crosshair marker, the screen flash and the direction arcs. */
  private readonly hits = new HitFeedback()
  /** The vehicle seat the server says the local player sits in; null when on foot. */
  private seat: { vehicle: number; kind: number; index: number } | null = null
  private wasGunner = false
  private wasAviator = false
  private wasWalking = false
  private wasTanker = false
  private wasMortar = false
  private wasPiloting = false
  /** Which of a loitering drone's two cameras the local pilot watches; aim (right-click) swaps them. */
  private loiterCam: 'forward' | 'down' = 'forward'
  /** Aim was held last frame, so a fresh press swaps the loitering drone's camera once. */
  private wasAimHeld = false
  /** The local player's own drone, hidden from its own camera while they fly it. */
  private hiddenDrone: VehicleModel | null = null
  /** `performance.now()` when the local player's drone feed last cut out, for a burst of static. */
  private feedLostAt = -Infinity
  /** The yaw and pitch a plane's camera has eased to, behind the mouse's aim. */
  private planeView: [number, number] = [0, 0]
  private predictor: Predictor | null = null
  private motor: SimMotor | null = null
  /** Predicts the vehicle the local player drives. */
  private driver: VehicleDriver | null = null
  /** The newest snapshot of the driven vehicle, waiting for the next frame to be reconciled with. */
  private driverSnapshot: { ack: number; vehicle: VehicleState; drive: DriveState } | null = null
  /** Vehicle poses of the newest snapshot, which the predicted stickman's frame is fixed to while riding. */
  private readonly refPoses = new Map<number, VehicleState>()
  private localId: number | null = null
  private worldHash = 0
  private seq = 0
  private recent: InputFrame[] = []
  private accumulator = 0
  private lastFrame = 0
  private frameHandle = 0
  /** Frame-cost overlay, F3 or `?perf`. */
  private readonly perf: PerfReadout
  private reconnectTimer = 0
  private readonly backoff = new Backoff()
  private pingTimer = 0
  private disposed = false
  /** Set when reconnecting cannot help (protocol mismatch); only a reload can. */
  private halted = false
  private warning: string | null = null

  /** Called every frame with the pilot's gunsight and mouse-aim circle on screen, or nulls. */
  onSights: ((nose: [number, number] | null, aim: [number, number] | null) => void) | null = null

  constructor(container: HTMLElement, private readonly onStats: (stats: HudStats) => void) {
    this.scene = new Scene(container)
    this.perf = new PerfReadout(() => this.scene.renderer)
    this.input = new InputController(this.scene.canvas)
    this.view = new WorldView(this.scene.scene)
    this.view.onTreeNoise = ({ phase, at, size }) => {
      const where: [number, number, number] = [at.x, at.y, at.z]
      if (phase === 'hit') this.sounds.treeHit(where, size)
      else if (phase === 'crack') this.sounds.treeCrack(where, size)
      else {
        this.sounds.treeThud(where, size)
        if (this.scene.camera.position.distanceTo(new THREE.Vector3(at.x, at.y, at.z)) < 12) this.chase.kick(0.15)
      }
    }
    this.scene.scene.add(this.crowd.root)
    this.scene.scene.add(this.effects.root, this.view.damage.root, this.damageNumbers.root)
    this.scene.scene.add(this.rocketModels.root, this.trails.root)
    this.scene.scene.add(this.pickupModels.root)
    this.scene.scene.add(this.trajectory.root, this.mortarArc.root)
    this.scene.scene.add(this.smoke.root, this.glow.root, this.skids.root)
    this.smoke.groundAt = (x, z) => this.scene.groundAt(x, z)
    this.scene.scene.add(this.debris.root)
    this.debris.groundAt = (x, z) => this.scene.groundAt(x, z)
    this.scene.scene.add(this.view.waterRoot)
    this.transport = new Transport(
      (message) => this.handle(message),
      (status) => {
        if (status === 'OFFLINE') this.onDisconnect()
      },
    )
    // Dev-only handle for poking at the game from the console or a test driver.
    if (import.meta.env.DEV) (window as unknown as { __game?: Game }).__game = this
  }

  async start() {
    // The GPU first: with no WebGPU there is nothing to draw, and the error says so.
    await this.scene.init()
    const { world, hash } = await loadManifest()
    this.worldHash = hash
    this.world = world
    this.view.world = world
    // On a hilly world the plane sits just under the terrain, so it only shows where a tile is late.
    this.scene.scene.add(buildGround(world.halfExtent, world.groundCell > 0 ? -0.05 : 0))
    this.terrain = this.view.terrain = new Terrain(world.halfExtent, world.tileSize)
    this.surfaces = this.view.surfaces = new Surfaces(world.halfExtent, world.tileSize)
    this.waters = this.view.waters = new Waters(world.halfExtent, world.tileSize)
    this.scene.groundAt = (x, z) => this.terrain?.heightAt(x, z) ?? 0
    const terrain = this.terrain
    this.grass = this.view.grass = new Grass(world.halfExtent, world.tileSize, (x, z) => terrain.sample(x, z))
    this.scene.scene.add(this.grass.root)
    this.applyGrass(this.scene.look)
    if (!world.bytes) throw new Error('the manifest came without its bytes')
    const motor = await SimMotor.create(world.bytes)
    this.motor = this.view.motor = motor
    this.driver = new VehicleDriver(motor)
    await Promise.all([this.crowd.load(), loadVehicleModels(), loadRoosterModel()])
    // The buildings arrive as tiles, around wherever the player is.
    this.tiles = new TileStreamer(world, { attach: (ref, tile) => this.view.attachTile(ref, tile) })
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    await this.tiles.prime(x, z)
    this.predictor = new Predictor(motor.stepper, { position: [x, this.terrain.heightAt(x, z) + STANDING_Y, z], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0, prone: false, chute: false })
    this.predictor.speedScale = (input) => aimSpeedScale(this.hotbar[Math.min(input.slot, HOTBAR_SLOTS - 1)]?.item ?? ITEM_NONE, input.buttons)
    this.avatar(0)
    if (this.disposed) return
    this.lastFrame = performance.now()
    this.frameHandle = requestAnimationFrame((now) => this.frame(now))
    // A timer rather than the frame loop, so a background tab (no animation frames) still pings
    // and the server does not drop it as idle.
    this.pingTimer = window.setInterval(() => {
      if (this.localId !== null) this.transport.send(encodePing(this.rtt.next(performance.now())))
    }, PING_INTERVAL_MS)
    void this.connect()
  }

  private async connect() {
    if (this.disposed) return
    try {
      await this.transport.connect(profileQuery(loadProfile(), this.worldHash))
    } catch (error) {
      console.warn('connect failed', error)
      this.transport.close()
    }
  }

  private onDisconnect() {
    this.localId = null
    this.awaitingCatchUp = false
    this.seat = null
    this.board.clear()
    this.notices.clear()
    forgetAllPlayers()
    this.hits.clear()
    this.hp = PLAYER_MAX_HP
    this.diedAt = null
    this.driverSnapshot = null
    this.driver?.leave()
    this.interp.clear()
    this.vehicleInterp.clear()
    this.replica.reset()
    this.clearRockets()
    this.pickups = []
    this.pickupModels.clear()
    this.hotbar = emptyHotbar()
    this.progress = 0
    this.pendingShots = []
    this.blips.clear()
    this.motor?.setVehicles([])
    this.clock.reset()
    for (const [id, avatar] of this.avatars) {
      if (id !== 0) {
        avatar.dispose()
        this.avatars.delete(id)
      }
    }
    for (const model of this.roosters.values()) model.dispose()
    this.roosters.clear()
    if (!this.disposed && !this.halted) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = window.setTimeout(() => void this.connect(), this.backoff.next())
    }
  }

  private handle(message: ServerMessage) {
    const now = performance.now()
    switch (message.type) {
      case 'welcome':
        if (message.version !== PROTOCOL_VERSION) {
          this.warning = `Server speaks protocol ${message.version}, this page ${PROTOCOL_VERSION}. Reload.`
          this.halted = true
          this.transport.close()
          return
        }
        if (message.worldHash !== this.worldHash) this.warning = 'Server world differs from this page’s world.msgpack. Reload.'
        this.backoff.reset()
        this.localId = message.id
        this.awaitingCatchUp = true
        this.replica.reset()
        // Recreate the local stickman in this player's colour.
        this.avatars.get(0)?.dispose()
        this.avatars.delete(0)
        // Inputs made before joining were never sent; they must not be replayed on server state.
        this.predictor?.clearPending()
        this.driver?.clearPending()
        this.recent = []
        this.hp = PLAYER_MAX_HP
        this.diedAt = null
        break
      case 'snapshot': {
        if (this.localId === null) return
        // Snapshots list what changed since a baseline. One that names a baseline we do not hold
        // is dropped; the server has no acknowledgement for it and soon sends a whole one.
        const rebuilt = this.replica.receive(message.tick, message.baseline, message.updates, message.removed)
        if (!rebuilt) return
        // The components, as v1's states, until each has a presenter of its own (Phase 4).
        const { wire, rockets, pickups } = v1Scene(rebuilt.scene)
        const { entities, vehicles } = sceneStates(wire)
        this.clock.observe(message.tick, now)
        this.setHealth(message.hp, now)
        this.progress = message.progress / 255
        this.hotbar = [...message.inventory, ...emptyHotbar()].slice(0, HOTBAR_SLOTS)
        this.pickups = pickups
        for (const shot of message.shots) this.pendingShots.push({ tick: message.tick, shot })
        if (this.pendingShots.length > 64) this.pendingShots.splice(0, this.pendingShots.length - 64)
        this.trackRockets(message.tick, rockets)
        this.noteBullets(message.tick, message.bullets)
        const remote = entities.filter((e) => e.id !== this.localId)
        this.interp.push(message.tick, remote)
        this.vehicleInterp.push(message.tick, vehicles)
        if (rebuilt.latest) {
          // Out of range is not a leave: the server just stops listing them. Whatever is no longer
          // in the newest scene is dropped now instead of hanging in the air until it goes stale.
          const present = new Set(remote.map((e) => e.id))
          for (const id of this.interp.ids()) if (!present.has(id)) this.interp.remove(id)
          const parked = new Set(vehicles.map((v) => v.id))
          for (const id of this.vehicleInterp.ids()) if (!parked.has(id)) this.vehicleInterp.remove(id)
        }
        // Cars are solid to the predicted stickman, at the pose of the newest snapshot.
        this.refPoses.clear()
        this.motor?.setVehicles(
          vehicles.map((vehicle) => {
            this.refPoses.set(vehicle.id, vehicle)
            const [vx, vz] = this.vehicleInterp.latestVelocity(vehicle.id) ?? [0, 0]
            return { ...vehicle, vx, vz }
          }),
        )
        // Seated players are not listed as entities; whatever track they left behind is stale.
        const mount = vehicles.find((vehicle) => vehicle.seats.includes(this.localId!))
        const wasSeated = this.seat !== null
        this.seat = mount ? { vehicle: mount.id, kind: mount.kind, index: mount.seats.indexOf(this.localId!) } : null
        // A door closing, for getting in or out (not for hopping seats).
        if (wasSeated !== (this.seat !== null) && this.hp > 0) this.sounds.door()
        // A drone's pilot is still listed, kneeling where they launched it.
        for (const vehicle of vehicles) if (!isDrone(vehicle.kind)) for (const rider of vehicle.seats) if (rider !== 0) this.interp.remove(rider)
        const own = entities.find((e) => e.id === this.localId)
        if (mount && this.seat?.index === 0 && mount.kind !== KIND_MORTAR && message.drive) {
          // Replaying the unacknowledged inputs steps the whole physics world once each, so it waits
          // for the next frame: a backlog of snapshots (a busy main thread) is then one replay, not
          // one per snapshot, which would only make the thread busier.
          this.driver?.acknowledge(message.ack)
          this.driverSnapshot = { ack: message.ack, vehicle: mount, drive: message.drive }
        } else {
          this.driverSnapshot = null
          this.driver?.leave()
          this.driver?.acknowledge(message.ack)
        }
        if (this.seat) {
          // The stickman is parked inside the car. Inputs still go out; the server just applies them to the car.
          this.predictor?.acknowledge(message.ack)
        } else if (own && this.predictor) {
          this.predictor.reconcile(message.ack, { position: [own.x, own.y, own.z], yaw: own.yaw, vy: own.vy, grounded: own.grounded, knock: [own.knockX, own.knockZ], ride: message.ride, prone: own.prone, chute: own.chute })
        }
        break
      }
      case 'pong':
        this.rtt.pong(message.nonce, now)
        break
      case 'rtc':
        void this.voice.receive(message.from, message.kind, message.json)
        break
      case 'leave':
        this.voice.forget(message.id)
        this.interp.remove(message.id)
        this.board.forget(message.id)
        forgetPlayer(message.id)
        break
      case 'destroyed': {
        const animate = !this.awaitingCatchUp
        this.awaitingCatchUp = false
        this.view.destroyed(message.objects, animate)
        break
      }
      case 'damaged':
        this.view.damaged(message.buildings)
        break
      case 'carved':
        this.view.carved(message.buildings, message.live)
        break
      case 'scores':
        this.board.replace(message.scores)
        break
      case 'profiles':
        for (const { id, name, color, team, bot } of message.players) {
          setPlayer(id, name, color, team, bot)
          // Snapshots come by datagram and can overtake this, so a stickman may already be drawn.
          this.avatars.get(id === this.localId ? 0 : id)?.setColor(colorFor(id))
        }
        break
      case 'hit': {
        this.hits.receive(message, now)
        if (message.role === HIT_DEALT) {
          this.sounds.hit(message.kind === HIT_KILL)
          this.showHit(message)
        }
        else if (message.role === HIT_TAKEN) {
          this.sounds.hurt(message.amount)
          this.chase.kick(Math.min(0.5, message.amount / 80))
        }
        break
      }
      case 'notice':
        this.notices.receive(message.kind, message.text, now)
        break
      case 'kill': {
        const body = this.avatars.get(message.victim === this.localId ? 0 : message.victim)
        if (body?.visible) this.gore.die(body, message.style, message.yaw, message.power, (x, z) => this.scene.groundAt(x, z))
        this.board.kill(message, now)
        break
      }
    }
  }

  private tick() {
    // Dead or seated, nobody is lying down: respawning or stepping out starts on their feet.
    if (this.seat || this.hp === 0) this.input.clearProne()
    const sample = this.input.sample()
    this.crouching = (sample.buttons & BUTTON_CROUCH) !== 0
    const frame: InputFrame = {
      seq: ++this.seq,
      strafe: toAxis(sample.strafe),
      forward: toAxis(sample.forward),
      yaw: toMrad(sample.yaw),
      buttons: sample.buttons,
      pitch: toMrad(sample.pitch),
      view: this.viewTick(),
      slot: sample.slot,
    }
    // In a car the stickman does not walk, and nor does a dead one; the frame is kept for replay
    // in case it steps out or respawns.
    if (this.seat || this.hp === 0) this.predictor!.record(frame)
    else this.predictor!.apply(frame)
    if (this.localId !== null) this.fireFeedback(frame)
    // A driver's own vehicle is predicted too; otherwise the frame is only remembered for a replay.
    this.driver?.apply(frame)
    if (this.localId === null) return
    this.recent.push(frame)
    if (this.recent.length > INPUT_REDUNDANCY) this.recent.shift()
    this.transport.send(encodeInput(this.recent, this.replica.baseline))
  }

  /** The server tick the picture on screen shows, as the wire carries it: modulo 65536. */
  private viewTick() {
    const tick = Math.floor(this.clock.tickAt(performance.now()) - this.clock.interpDelay)
    return Number.isFinite(tick) && tick > 0 ? tick & 0xffff : 0
  }

  private frame(now: number) {
    if (this.disposed) return
    this.frameHandle = requestAnimationFrame((t) => this.frame(t))
    this.perf.begin(now)
    const dt = Math.min(0.25, (now - this.lastFrame) / 1000)
    this.frameDt = dt
    this.lastFrame = now

    if (this.driverSnapshot) {
      const { ack, vehicle, drive } = this.driverSnapshot
      this.driverSnapshot = null
      this.driver?.reconcile(ack, vehicle, drive)
    }
    this.view.update(dt, this.effects, this.cameraAt.set(this.scene.camera.position.x, this.scene.camera.position.y, this.scene.camera.position.z), this.fxWorld)
    this.accumulator += dt * 1000
    let ticks = 0
    while (this.accumulator >= TICK_MS && ticks < MAX_TICKS_PER_FRAME) {
      this.tick()
      this.accumulator -= TICK_MS
      ticks++
    }
    if (ticks === MAX_TICKS_PER_FRAME) this.accumulator = 0

    const predictor = this.predictor!
    const renderTick = this.clock.tickAt(now) - this.clock.interpDelay
    // The time of day runs on the server's clock, so every player sees the same sky.
    if (this.clock.synced) this.scene.setTimeOfDay(dayPhase(renderTick, TICK_RATE))
    const seated = this.seat !== null
    // Flying a drone: the body kneels where it launched it, and the view is the drone's camera.
    const flying = this.seat !== null && isDrone(this.seat.kind)
    const driven = this.driver?.pose(this.accumulator / TICK_MS, dt) ?? null
    const cars = this.updateCars(renderTick, driven)

    const position = this.ridden(predictor.renderPosition(this.accumulator / TICK_MS, dt), predictor.state.ride, renderTick)
    const local = predictor.state
    const localVx = (local.position[0] - predictor.previous.position[0]) / (TICK_MS / 1000)
    const localVz = (local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)
    const me = this.avatar(0)
    // A car's passenger fires their own guns from their seat, as one on foot does.
    const passenger = this.seat !== null && crewGun(this.seat.kind, this.seat.index)
    const hands = !seated || passenger
    // Where the local player sits, when they show in their seat: their head, and the vehicle's turn.
    const mount = seated && !flying ? this.cars.get(this.seat!.vehicle) : undefined
    const sitting = mount !== undefined && this.seatFrame(mount, this.seat!.kind, this.seat!.index, this.seatAt, this.seatTurn)
    // Aim held with a scoped gun in hand, alive and free to use it: look down the scope.
    const scoped = this.localId !== null && hands && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.scope === true
    this.scoped = scoped
    // Aim held with any other gun: half zoom over the shoulder.
    const aiming = !scoped && this.localId !== null && hands && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.kind === 'gun'
    this.aiming = aiming
    this.aimBlend += ((aiming ? 1 : 0) - this.aimBlend) * Math.min(1, dt * 14)
    const driving = seated ? cars.get(this.seat!.vehicle) : undefined
    // Aim held at a tank's gun, or a Strv S's: look down its sight, which shows where the barrel really points.
    const sighted = this.hp > 0 && this.input.aimHeld && driving !== undefined
      && (driving.kind === KIND_TANK ? this.seat!.index === 1 : driving.kind === KIND_STRV && this.seat!.index === 0)
    this.input.lookScale = sighted ? SIGHT_LOOK_SCALE : scoped ? SCOPE_LOOK_SCALE : aiming ? AIM_LOOK_SCALE : 1
    const chase = driving && this.seat!.index === 0 && driving.kind !== KIND_HELI && driving.kind !== KIND_PLANE && driving.kind !== KIND_MORTAR && !isDrone(driving.kind) ? this.chase.update([driving.x, driving.y, driving.z], driving.yaw, dt, cameraDistance(driving.kind)) : null
    if (!chase) this.chase.reset()
    // A loitering drone's pilot swaps between its nose camera and its belly camera on a fresh aim press.
    const aimHeld = this.input.aimHeld
    if (flying && this.seat!.kind === KIND_LOITER && aimHeld && !this.wasAimHeld) this.loiterCam = this.loiterCam === 'forward' ? 'down' : 'forward'
    this.wasAimHeld = aimHeld
    const looksDown = flying && this.seat!.kind === KIND_LOITER && this.loiterCam === 'down'
    this.scene.zoomTo(looksDown ? 'down' : flying ? 'fpv' : sighted ? 'sight' : scoped ? 'scope' : aiming ? 'aim' : 'none', dt, chase?.fovBoost ?? 0)
    me.hidden = (seated && !flying && !sitting) || scoped
    me.dead = this.hp === 0
    me.held = this.hotbar[this.input.selectedSlot]?.item ?? ITEM_NONE
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    if (sitting) me.seat(this.seatAt, this.seatTurn, this.input.yaw, this.input.pitch, dt)
    else me.update(position, flying ? local.yaw : this.input.yaw, localVx, localVz, local.grounded, dt, flying ? 0 : this.input.pitch, this.crouching || flying,
      this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED, local.prone, local.chute)
    if (!seated && this.hp > 0) this.wade(0, position[0], position[1], position[2], Math.hypot(localVx, localVz), dt)
    else this.wading.delete(0)
    const ride = seated ? cars.get(this.seat!.vehicle) : undefined
    const gunner = ride !== undefined && vehicleDef(ride.kind).gun && this.seat!.index === 1
    // A Strv S's driver fires the gun, which only the barrel's elevation aims: the hull turns to aim sideways.
    const tanker = ride?.kind === KIND_STRV && this.seat!.index === 0
    const mortar = ride?.kind === KIND_MORTAR && this.seat!.index === 0
    const turret = gunner ? this.cars.get(this.seat!.vehicle) : undefined
    const aviator = ride?.kind === KIND_PLANE && this.seat!.index === 0
    const walking = ride?.kind === KIND_WALKER && this.seat!.index === 0
    const piloting = ride !== undefined && isDrone(ride.kind) && this.seat!.index === 0
    this.scene.gunsight.hide()
    if (gunner && (turret instanceof TankModel || turret instanceof HeliModel || turret instanceof PlaneModel)) {
      if (!this.wasGunner && ride.turret) {
        // Take the seat looking where the barrel points.
        if (turret instanceof TankModel) {
          this.input.yaw = ride.yaw + ride.turret.yaw
          this.input.pitch = ride.turret.pitch
        } else {
          this.lookAlong(turret, ride.turret)
        }
      }
      // The camera sits behind and above the gun, looking along the barrel's line.
      if (turret instanceof TankModel) this.input.clampPitch(TANK_BARREL_MIN, TANK_BARREL_MAX)
      else if (turret instanceof HeliModel) this.limitGun(turret, HELI_GUN_YAW_LIMIT, HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX)
      else this.limitGun(turret, Math.PI, PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX)
      const { origin } = turret.barrel()
      if (sighted && turret instanceof TankModel) this.lookDownBarrel(turret)
      else this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch)
    } else if (piloting) {
      // The look is where the drone heads: it turns to follow, and the view is its nose camera, banking with it.
      const loiter = ride.kind === KIND_LOITER
      if (!this.wasPiloting) {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
        this.loiterCam = 'forward'
      }
      if (loiter) this.input.clampPitch(LOITER_PITCH_MIN, LOITER_PITCH_MAX)
      else this.input.clampPitch(-1.4, 1.0)
      const model = this.cars.get(this.seat!.vehicle)
      let bank = 0
      // A loitering drone's cameras sit in its nose and under its belly; an FPV drone's is at its centre.
      const eye = new THREE.Vector3(0, loiter ? (this.loiterCam === 'down' ? -0.35 : 0) : 0.05, loiter && this.loiterCam === 'forward' ? -1.7 : 0)
      if (model) {
        const right = new THREE.Vector3(1, 0, 0).applyQuaternion(model.root.quaternion)
        const top = new THREE.Vector3(0, 1, 0).applyQuaternion(model.root.quaternion)
        bank = -Math.atan2(-right.y, top.y)
        eye.applyQuaternion(model.root.quaternion)
        // Its own frame would fill the lens.
        model.root.visible = false
        this.hiddenDrone = model
      }
      const at = [ride.x + eye.x, ride.y + eye.y, ride.z + eye.z]
      // The belly camera looks straight down with the drone's heading up the screen; it does not bank.
      if (loiter && this.loiterCam === 'down') this.scene.downFrom(at, ride.yaw)
      else this.scene.fpvFrom(at, this.input.yaw, this.input.pitch, bank)
    } else if (aviator) {
      // The mouse is the aim and moves at once; the plane turns to follow it, and the camera follows
      // the aim at an easy pace, so the circle drifts across the screen as it catches up.
      if (!this.wasAviator) {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
        this.planeView = [ride.yaw, 0]
      }
      this.input.clampPitch(-PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)
      const ease = 1 - Math.exp(-PLANE_VIEW_EASE * dt)
      this.planeView[0] += wrapAngle(this.input.yaw - this.planeView[0]) * ease
      this.planeView[1] += (this.input.pitch - this.planeView[1]) * ease
      // A little further back the faster it goes, and only part of the plane's roll reaches the view.
      const flying = this.vehicleFx.get(this.seat!.vehicle)
      const fast = Math.min(1, Math.max(0, (flying?.forwardSpeed ?? 0) / PLANE_CRUISE_SPEED))
      this.scene.follow([ride.x, ride.y, ride.z], this.planeView[0], this.planeView[1], PLANE_DISTANCE + PLANE_PULLBACK * fast, 0, PLANE_CAMERA_HEIGHT)
      const model = this.cars.get(this.seat!.vehicle)
      if (model instanceof PlaneModel) {
        const right = new THREE.Vector3(1, 0, 0).applyQuaternion(model.root.quaternion)
        const top = new THREE.Vector3(0, 1, 0).applyQuaternion(model.root.quaternion)
        this.scene.camera.rotateZ(-Math.atan2(-right.y, top.y) * PLANE_VIEW_ROLL)
      }
    } else if (tanker) {
      // The gun points along the hull, which A/D turn. The mouse looks freely around it and the view
      // eases back behind the barrel when it rests, so the middle of the screen is where the gun points
      // again.
      if (!this.wasTanker) this.input.yaw = ride.yaw
      else if (!sighted) this.input.followYaw(ride.yaw, dt)
      this.input.clampPitch(STRV_BARREL_MIN, STRV_BARREL_MAX)
      const strv = this.cars.get(this.seat!.vehicle)
      if (strv instanceof StrvModel) {
        const { origin } = strv.barrel()
        if (sighted) this.input.yaw = this.lookDownBarrel(strv)
        else this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch, STRV_AIM_BACK, STRV_AIM_UP)
      } else {
        this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, cameraDistance(ride.kind), 0, CAMERA_HEIGHT)
      }
    } else if (walking) {
      // The look is the aim: free of the legs' heading, within the upper body's travel of it.
      if (!this.wasWalking) {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
      }
      const walker = this.cars.get(this.seat!.vehicle)
      if (walker instanceof WalkerModel) {
        this.limitGun(walker, WALKER_TORSO_LIMIT, WALKER_GUN_PITCH_MIN, WALKER_GUN_PITCH_MAX)
        // Behind and above the guns, looking along their line, so the middle of the screen is where they point.
        const { origin } = walker.barrel()
        this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch, WALKER_AIM_BACK, WALKER_AIM_UP)
      } else {
        this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, cameraDistance(ride.kind), 0, CAMERA_HEIGHT)
      }
    } else if (mortar) {
      // The mouse sets the bearing: the mount turns to face it, and the view sits behind the mount looking where it points. W and S set the range.
      if (!this.wasMortar && ride.turret) this.input.yaw = ride.yaw + ride.turret.yaw
      this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, cameraDistance(ride.kind), 0, CAMERA_HEIGHT)
    } else if (ride && passenger && sitting) {
      // Free to look all round and shoot: over their own shoulder, as on foot, from where they sit.
      const head = [this.seatAt.x, this.seatAt.y, this.seatAt.z]
      if (scoped) this.scene.scopeFrom(head, this.input.yaw, this.input.pitch)
      else this.scene.follow(head, this.input.yaw, this.input.pitch, PASSENGER_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * this.aimBlend), PASSENGER_SHOULDER, PASSENGER_HEIGHT)
    } else if (ride) {
      this.input.followYaw(ride.yaw, dt)
      const yaw = this.input.yaw + (chase?.lean ?? 0)
      this.scene.follow([ride.x, ride.y + 0.4, ride.z], yaw, this.input.pitch, cameraDistance(ride.kind) + (chase?.distance ?? 0), 0, CAMERA_HEIGHT, chase?.shake)
    } else if (scoped) {
      this.scene.scopeFrom([position[0]!, position[1]! + this.eyeHeight(), position[2]!], this.input.yaw, this.input.pitch)
    } else {
      // Only the distance changes: the pivot is what the server's crosshair ray is built from.
      this.scene.follow(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * this.aimBlend), ON_FOOT_SHOULDER, ON_FOOT_HEIGHT)
    }

    this.wasGunner = gunner
    this.wasAviator = aviator
    this.wasWalking = walking
    this.wasTanker = tanker
    this.wasMortar = mortar
    if (this.wasPiloting && !piloting) this.feedLostAt = now
    if (!piloting && this.hiddenDrone) {
      // The link is gone but the drone may still be falling: it shows again.
      this.hiddenDrone.root.visible = true
      this.hiddenDrone = null
    }
    this.wasPiloting = piloting
    // Streaming follows what the camera is on: the car when seated, else the stickman.
    this.tiles?.update(ride ? ride.x : position[0], ride ? ride.z : position[2])
    this.playShots(renderTick)
    this.drawRockets(renderTick)
    this.drawBullets(renderTick)
    this.pickupModels.set(this.pickups)
    this.pickupModels.update(now / 1000)
    this.aimGrenade(position, seated)
    this.effects.update(dt)
    this.trails.update(dt)
    this.scene.scene.lights.update(dt)
    this.damageNumbers.update(dt, this.scene.camera)
    const eye = this.scene.camera.position
    this.smoke.follow(eye.x, eye.z)
    this.glow.follow(eye.x, eye.z)
    this.smoke.update(dt)
    this.glow.update(dt)
    this.skids.update(dt)
    this.debris.update(dt)
    this.gore.update(dt)

    const blobs = this.scene.contact
    blobs.begin()
    if (!seated && this.hp > 0) this.blobFoot(blobs, position[0], position[1], position[2])
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = car.y - hy - this.scene.groundAt(car.x, car.z)
      blobs.add(car.x, car.z, hx * 1.2, hz * 1.1, car.yaw, Math.max(0, height))
    }

    const seen = new Set<number>([0])
    const nearby: Array<{ id: number; x: number; y: number; z: number }> = []
    const roosters = new Set<number>()
    for (const id of this.interp.ids()) {
      const state = this.interp.sample(id, renderTick)
      if (state?.kind === KIND_ROOSTER) {
        roosters.add(id)
        this.rooster(id, state, dt, position)
        if (!state.dead) this.blobFoot(blobs, state.x, state.y + CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS, state.z)
        continue
      }
      if (!state || state.kind !== KIND_STICKMAN) continue
      seen.add(id)
      nearby.push({ id, x: state.x, y: state.y, z: state.z })
      const avatar = this.avatar(id)
      avatar.dead = state.dead
      avatar.held = state.held
      if (!state.dead) this.blobFoot(blobs, state.x, state.y, state.z)
      avatar.update([state.x, state.y, state.z], state.yaw, state.velocity[0], state.velocity[1], state.grounded, dt, 0, state.crouching,
        state.crouching && state.grounded && Math.hypot(state.knockX, state.knockZ) > SLIDE_MIN_SPEED, state.prone, state.chute)
      if (!state.dead) this.wade(id, state.x, state.y, state.z, Math.hypot(state.velocity[0], state.velocity[1]), dt)
    }
    this.bendGrass(position, !seated, nearby, cars)
    // Everyone else sitting where they show: in their seat, turned where they look.
    for (const [id, car] of cars) {
      const model = this.cars.get(id)
      if (!model) continue
      car.seats.forEach((rider, index) => {
        if (rider === 0 || rider === this.localId || !this.seatFrame(model, car.kind, index, this.seatAt, this.seatTurn)) return
        const look = car.crew[index]
        seen.add(rider)
        nearby.push({ id: rider, x: this.seatAt.x, y: this.seatAt.y, z: this.seatAt.z })
        const avatar = this.avatar(rider)
        avatar.dead = false
        avatar.held = look?.held ?? ITEM_NONE
        avatar.seat(this.seatAt, this.seatTurn, look?.yaw ?? car.yaw, look?.pitch ?? 0, dt)
      })
    }
    for (const [id, avatar] of this.avatars) {
      if (!seen.has(id)) {
        this.wading.delete(id)
        avatar.dispose()
        this.avatars.delete(id)
      }
    }

    for (const [id, model] of this.roosters) {
      if (!roosters.has(id)) {
        model.dispose()
        this.roosters.delete(id)
      }
    }

    this.voice.setTalking(this.localId !== null && this.hp > 0 && this.input.isHeld('KeyV'))
    this.voice.update(this.localId, position, nearby)
    blobs.end()
    this.mirrorWater()
    this.perf.beforeRender()
    this.scene.render()
    this.perf.end(ticks)
    this.sights(aviator)
    const hitView = this.hits.view(now, this.input.yaw)
    this.onStats({
      status: this.transport.status,
      rtt: this.rtt.rtt,
      voice: this.voice.status,
      players: this.localId === null ? 1 : seen.size + riders(cars, seen, this.localId),
      interpDelayMs: (this.clock.interpDelay * TICK_MS),
      tiles: this.tiles?.loaded ?? 0,
      correction: this.driver?.driving ? this.driver.lastCorrection : predictor.lastCorrection,
      pending: predictor.pendingCount,
      locked: this.input.locked,
      touch: this.input.coarse,
      warning: this.warning,
      prompt: this.prompt(position, cars),
      seat: !this.seat
        ? null
        : this.seat.index === 0
          ? this.seat.kind === KIND_DRONE ? 'drone' : this.seat.kind === KIND_LOITER ? 'loiter' : this.seat.kind === KIND_HELI ? 'pilot' : this.seat.kind === KIND_PLANE ? 'aviator' : this.seat.kind === KIND_WALKER ? 'walker' : this.seat.kind === KIND_HEN ? 'hen' : this.seat.kind === KIND_STRV ? 'strv' : this.seat.kind === KIND_MORTAR ? 'mortar' : this.seat.kind === KIND_ROADSTER ? 'roadster' : 'driver'
          : vehicleDef(this.seat.kind).gun ? 'gunner' : 'passenger',
      canSwitchSeat: this.seat !== null && (cars.get(this.seat.vehicle)?.seats.includes(0) ?? false),
      range: this.seat?.kind === KIND_MORTAR ? (ride?.turret?.range ?? 0) : 0,
      reload: this.seat?.kind === KIND_PLANE ? 1 : Math.min(1, (now - this.lastShotAt) / ((this.seat?.kind === KIND_HELI ? HELI_GUN_COOLDOWN_TICKS : this.seat?.kind === KIND_WALKER ? WALKER_GUN_COOLDOWN_TICKS : this.seat?.kind === KIND_MORTAR ? MORTAR_COOLDOWN_TICKS : SHELL_COOLDOWN_TICKS) * TICK_MS)),
      ...this.cockpit(ride, aviator),
      air: this.seat?.index === 0 && (this.seat.kind === KIND_ROADSTER || this.seat.kind === KIND_HEN) ? (this.driver?.thrust?.air ?? null) : null,
      eggs: this.seat?.index === 0 && this.seat.kind === KIND_HEN ? (ride?.arms?.bombs ?? null) : null,
      drone: piloting
        ? {
          battery: this.refPoses.get(this.seat!.vehicle)?.thrust?.air ?? 1,
          signal: Math.max(0, 1 - Math.hypot(ride.x - position[0]!, ride.z - position[2]!) / (ride.kind === KIND_LOITER ? LOITER_RANGE : DRONE_RANGE)),
          camera: ride.kind === KIND_LOITER ? this.loiterCam : null,
        }
        : null,
      feedLost: Math.max(0, 1 - (now - this.feedLostAt) / FEED_LOST_MS),
      hp: this.hp,
      maxHp: PLAYER_MAX_HP,
      dead: this.hp === 0,
      respawnIn: this.diedAt === null ? 0 : Math.max(0, (RESPAWN_TICKS * TICK_MS - (now - this.diedAt)) / 1000),
      sight: sighted,
      scoped: this.scoped,
      aiming: this.aiming,
      armed: this.localId !== null && hands && this.hp > 0 && itemDef(this.held().item)?.kind === 'gun',
      hands: this.localId !== null && hands && this.hp > 0,
      hotbar: this.hotbar,
      selected: this.input.selectedSlot,
      progress: this.progress,
      kills: this.board.kills(this.localId),
      deaths: this.board.deaths(this.localId),
      feed: this.board.recent(now, this.localId),
      notices: this.notices.view(now),
      board: this.input.isHeld('Tab') ? sides(this.board.rows(this.localId, this.interp.ids()), teamFor) : [],
      hitMarker: hitView.marker,
      hurt: hitView.vignette,
      hurtArcs: hitView.arcs,
      // Seated, the on-foot body stays where it got in; the map follows the vehicle.
      x: ride?.x ?? position[0],
      z: ride?.z ?? position[2],
      yaw: this.input.yaw,
      halfExtent: this.world?.halfExtent ?? 0,
      blips: this.blips.view(now, this.localId, ride?.x ?? position[0]!, ride?.z ?? position[2]!, this.alliesInView(), MAP_SIDES),
    })
  }

  /** Where the local player's allies in view stand. */
  private alliesInView(): { id: number; x: number; z: number }[] {
    if (this.localId === null || teamFor(this.localId) === 0) return []
    const found: { id: number; x: number; z: number }[] = []
    for (const id of this.interp.ids()) {
      const avatar = this.avatars.get(id)
      if (avatar?.visible && allies(this.localId, id)) found.push({ id, x: avatar.position.x, z: avatar.position.z })
    }
    return found
  }

  private setHealth(hp: number, now: number) {
    if (hp === 0 && this.hp > 0) this.diedAt = now
    else if (hp > 0) this.diedAt = null
    this.hp = hp
  }

  /** Where a player's rifle leaves: the end of the barrel it is drawn holding, else its eye. */
  private muzzleOf(shooter: number): THREE.Vector3 | null {
    const avatar = this.avatars.get(shooter === this.localId ? 0 : shooter)
    if (!avatar || !avatar.visible) return null
    return avatar.muzzle() ?? avatar.position.clone().add(new THREE.Vector3(0, EYE_HEIGHT, 0))
  }

  private showShot(shot: ShotEvent) {
    let gun: ItemDef | undefined
    const to = new THREE.Vector3(shot.x, shot.y, shot.z)
    if (shot.weapon === WEAPON_ROCKET) {
      this.blast(to, ROCKET_BLAST_RADIUS, BLASTS.rocket)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_GRENADE) {
      this.blast(to, GRENADE_BLAST_RADIUS, BLASTS.grenade)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_SHELL) {
      this.blast(to, EXPLOSION_RADIUS, BLASTS.shell)
      this.sounds.blast([to.x, to.y, to.z], 'big')
    } else if (shot.weapon === WEAPON_MORTAR) {
      this.blast(to, MORTAR_RADIUS, BLASTS.shell)
      this.sounds.blast([to.x, to.y, to.z], 'big')
    } else if (shot.weapon === WEAPON_WRECK) {
      this.blast(to, WRECK_BLAST_RADIUS, BLASTS.wreck)
      this.sounds.explosion([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_DRONE) {
      this.blast(to, DRONE_BLAST_RADIUS, BLASTS.grenade)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_LOITER) {
      this.blast(to, LOITER_BLAST_RADIUS, BLASTS.shell)
      this.sounds.explosion([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_FLARE) {
      // The flare itself is a projectile; this is the gun going off.
      this.avatars.get(shot.shooter === this.localId ? 0 : shot.shooter)?.fire()
      const from = this.muzzleOf(shot.shooter) ?? to
      this.effects.muzzle(from)
      this.muzzleLight(from, 0xffa060, 2.5, 8)
      this.sounds.gunshot([from.x, from.y, from.z], 'revolver')
    } else if (shot.weapon === WEAPON_AUTOCANNON) {
      this.blast(to, CANNON_RADIUS, BLASTS.cannon)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_BULLET) {
      // A bullet ended here; `shooter` is its id. It is drawn until it gets there.
      const bullet = this.bullets.get(shot.shooter)
      if (bullet) bullet.endTicks = bulletTicksTo(bullet.spawn, [shot.x, shot.y, shot.z])
      if (this.watery(to, bullet?.spawn.item === ITEM_SNIPER ? 0.45 : 0.25)) { /* splashed */ }
      else if (bullet?.spawn.item === ITEM_SNIPER) this.effects.burst(to, 0.8)
      else this.effects.pelletStrike(to)
      if (bullet?.spawn.shooter === this.localId) this.view.chipped(to.x, to.y, to.z, itemDef(bullet.spawn.item)?.damage ?? 0)
    } else if (shot.weapon === WEAPON_MG) {
      // The tracer was drawn when the gun fired; this is a round striking something.
      if (!this.watery(to, 0.25)) this.effects.strike(to)
    } else if (shot.weapon === WEAPON_PELLET) {
      // A further pellet of a spread: the blast and the sound came with the first.
      const gun = hitscanGunFor(WEAPON_SHOTGUN)
      const from = this.muzzleOf(shot.shooter)
      if (!gun || (from && from.distanceTo(to) >= gun.range - 1)) return
      if (!this.watery(to, 0.2)) this.effects.pelletStrike(to)
      if (shot.shooter === this.localId) this.view.chipped(to.x, to.y, to.z, gun.damage)
    } else if (shot.weapon === WEAPON_BOMB || shot.weapon === WEAPON_EGG) {
      // A hen's egg goes off like a plane's bomb, and as wide.
      this.blast(to, BOMB_RADIUS, BLASTS.bomb)
      this.sounds.explosion([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_RICOCHET) {
      // A shell or round glanced off armor: sparks and a ping, and no blast.
      this.effects.strike(to)
      this.sounds.ricochet([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_TURRET) {
      const tank = this.cars.get(shot.shooter)
      if (tank instanceof TankModel) this.shedTurret(shot.shooter, tank)
      this.blast(to, CANNON_RADIUS, BLASTS.cannon)
      this.sounds.turretOff([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_CRASH) {
      const car = this.cars.get(shot.shooter)
      const velocity = this.vehicleFx.get(shot.shooter)?.velocity ?? STILL
      if (car instanceof CarModel) car.noteImpact(to)
      crashBurst(this.fxWorld, to, car instanceof CarModel ? car.paintColor : 0x555555, velocity)
      this.sounds.crash([to.x, to.y, to.z], this.vehicleFx.get(shot.shooter)?.impactSpeed ?? 0)
      // Felt hard by its driver, a little by anyone close by.
      if (this.driver?.driving === shot.shooter) this.chase.kick(0.7)
      else if (to.distanceTo(this.scene.camera.position) < 20) this.chase.kick(0.15)
    } else if (shot.weapon === WEAPON_SCRAPE) {
      scrapeSparks(this.fxWorld, to, this.vehicleFx.get(shot.shooter)?.velocity ?? STILL)
      this.sounds.scrape(shot.shooter)
    } else if (shot.weapon === WEAPON_SNIPER) {
      // The round is drawn in flight; this is where it landed.
      if (!this.watery(to, 0.45)) this.effects.burst(to, 0.8)
    } else if ((gun = hitscanGunFor(shot.weapon))) {
      const from = this.muzzleOf(shot.shooter)
      this.avatars.get(shot.shooter === this.localId ? 0 : shot.shooter)?.fire()
      if (from) {
        this.blips.fired(shot.shooter, from.x, from.z, performance.now())
        const landed = from.distanceTo(to) < gun.range - 1
        if (gun.pellets > 1) {
          // No rays for a spread: a blast at the muzzle, then each pellet is drawn where it landed.
          this.effects.blast(from, to.clone().sub(from).normalize())
          if (landed && !this.watery(to, 0.2)) this.effects.pelletStrike(to)
        } else {
          this.effects.tracer(from, to, landed)
          if (landed) this.watery(to, 0.25)
        }
        this.muzzleLight(from, 0xffd890, 1.6, 6)
      }
      const at = from ?? to
      this.sounds.gunshot([at.x, at.y, at.z], gun.sound)
      if (shot.shooter === this.localId && from && from.distanceTo(to) < gun.range - 1) this.view.chipped(to.x, to.y, to.z, gun.damage)
    }
  }

  /**
   * Splashes where something of about `size` metres struck, when `at` is at or under the water surface
   * (the server reports the carved bed). Returns whether it did.
   */
  private watery(at: THREE.Vector3, size: number, speed = 10): boolean {
    const level = this.waters?.levelAt(at.x, at.z) ?? NO_WATER
    if (level === NO_WATER || at.y > level + 0.05) return false
    splash(this.fxWorld, { x: at.x, y: level, z: at.z }, size, speed)
    return true
  }

  /** Picks the water level to mirror this frame, if any water is about the camera. */
  private mirrorWater() {
    const camera = this.scene.camera.position
    const look = this.scene.camera.getWorldDirection({ x: 0, y: 0, z: 0 })
    const level = this.waters?.planeNear(camera.x, camera.z, look.x, look.z) ?? null
    this.scene.setWaterPlane(level)
  }

  private readonly wading = new Map<number, { wet: boolean; ripple: number }>()

  /** Splashes when a person's feet go into water, and ripples while they wade. `speed` is how fast they move, m/s. */
  private wade(id: number, x: number, y: number, z: number, speed: number, dt: number) {
    const level = this.waters?.levelAt(x, z) ?? NO_WATER
    const wet = level !== NO_WATER && level - y > WADE_DEPTH
    const was = this.wading.get(id)
    if (!was) {
      // First seen: already wet is not a splash.
      this.wading.set(id, { wet, ripple: 0 })
      return
    }
    if (wet && !was.wet) splash(this.fxWorld, { x, y: level, z }, 0.45, 2 + Math.min(speed, 8))
    else if (!wet && was.wet && speed > 1) splash(this.fxWorld, { x, y: level === NO_WATER ? y : level, z }, 0.3, 1)
    if (wet && speed > 1) {
      was.ripple -= dt
      if (was.ripple <= 0 && Math.hypot(this.scene.camera.position.x - x, this.scene.camera.position.z - z) < WADE_RIPPLE_RANGE) {
        this.effects.ripple({ x, y: level + 0.02, z }, 0.3)
        was.ripple = 0.35
      }
    }
    was.wet = wet
  }

  /** A short flash of light at a gun's muzzle: it lights the barrel, the hull and the ground in front of it. */
  private muzzleLight(at: THREE.Vector3, color: number, intensity: number, radius: number) {
    this.scene.scene.lights.flash(at.x, at.y, at.z, { color, intensity, radius, life: 0.09 })
  }

  /** A heavy blast: the layered fire, sparks, dust and debris, a flash, and a shove of the camera if it is close. */
  private blast(at: THREE.Vector3, radius: number, profile: ExplosionProfile) {
    // On water it is a column of spray, not fire and dirt.
    if (this.watery(at, radius * 0.45, 14)) {
      const distance = at.distanceTo(this.scene.camera.position)
      if (distance < radius * 8) this.chase.kick(Math.min(0.4, (radius / 10) * (1 - distance / (radius * 8))))
      return
    }
    this.effects.burst(at, radius)
    explosion(this.fxWorld, this.debris, at, radius, profile)
    // A flash that lights the ground and what stands on it, and a shockwave racing out over the ground.
    const reach = Math.min(2, Math.max(0.6, radius / 5))
    this.scene.scene.lights.flash(at.x, at.y + 1.5, at.z, { color: 0xffa040, intensity: 5 * reach, radius: radius * 8, life: 0.6 })
    const ground = this.scene.groundAt(at.x, at.z)
    if (at.y - ground < radius) {
      this.effects.shockwave({ x: at.x, y: ground + 0.25, z: at.z }, radius)
      this.grass?.blast(at.x, at.z, radius)
    }
    const distance = at.distanceTo(this.scene.camera.position)
    if (distance < radius * 8) this.chase.kick(Math.min(0.6, (radius / 8) * (1 - distance / (radius * 8))))
  }

  /** What a hit you landed looks like in the world: a number, flying debris and a flash on the target. */
  private showHit(hit: { kind: number; amount: number; x: number; y: number; z: number; target: number }) {
    const at = new THREE.Vector3(hit.x, hit.y, hit.z)
    const kind = hit.kind === HIT_KILL ? 'kill' : hit.kind === HIT_VEHICLE ? 'vehicle' : hit.kind === HIT_ROOSTER ? 'rooster' : 'body'
    this.damageNumbers.spawn(at, hit.amount, kind, this.scene.camera)
    // Debris flies back towards the shooter, as it would from a round that came in.
    const back = this.scene.camera.position.clone().sub(at).normalize()
    this.effects.impact(at, kind === 'kill' ? 'kill' : kind === 'vehicle' ? 'vehicle' : 'body', back)
    this.avatars.get(hit.target)?.flash()
  }

  /** Plays the events whose tick the picture has reached. */
  private playShots(renderTick: number) {
    let played = 0
    while (played < this.pendingShots.length && this.pendingShots[played]!.tick <= renderTick) this.showShot(this.pendingShots[played++]!.shot)
    if (played > 0) this.pendingShots.splice(0, played)
  }

  /** Buffers the rockets of a snapshot, and notes the ones it no longer lists as burst at that tick. */
  private trackRockets(tick: number, rockets: readonly RocketState[]) {
    const known = new Set(this.rocketInterp.ids())
    // Nobody is named as a rocket's shooter: the map marks it as gunfire from where it was first seen.
    for (const rocket of rockets) if (!known.has(rocket.id)) this.blips.fired(0, rocket.x, rocket.z, performance.now())
    this.rocketInterp.push(tick, rockets)
    const listed = new Set(rockets.map((rocket) => rocket.id))
    for (const id of this.rocketInterp.ids()) {
      if (!listed.has(id) && !this.rocketEnds.has(id)) this.rocketEnds.set(id, tick)
    }
  }

  private drawRockets(renderTick: number) {
    for (const id of this.rocketInterp.ids()) {
      const ends = this.rocketEnds.get(id)
      if (ends !== undefined && renderTick >= ends) {
        this.rocketInterp.remove(id)
        this.rocketEnds.delete(id)
        this.rocketModels.remove(id)
        this.trails.end('r', id)
        this.trailOwed.delete(id)
        continue
      }
      const state = this.rocketInterp.sample(id, renderTick)
      if (!state) continue
      this.rocketModels.place(id, state.kind, [state.x, state.y, state.z], [state.vx, state.vy, state.vz])
      this.trails.follow('r', id, state.kind, state.x, state.y, state.z)
      if (state.kind === PROJECTILE_FLARE) this.burnFlare(id, state.x, state.y, state.z)
      // Only a motor-driven rocket (kind 0) leaves a flame and a smoke trail.
      if (state.kind === 0) {
        this.trailOwed.set(id, rocketTrail(this.fxWorld, [state.x, state.y, state.z], [state.vx, state.vy, state.vz], this.frameDt, this.trailOwed.get(id) ?? 0))
      }
    }
  }

  /** Starts the bullets of a snapshot that are new; each is sent in two, in case one is lost. */
  private noteBullets(tick: number, spawns: readonly BulletSpawn[]) {
    for (const spawn of spawns) {
      if (this.seenBullets.has(spawn.id)) continue
      this.seenBullets.set(spawn.id, spawn.tick)
      this.blips.fired(spawn.shooter, spawn.x, spawn.z, performance.now())
      this.bullets.set(spawn.id, { spawn, life: bulletLifeTicks(spawn), endTicks: null, shown: false, offset: null })
    }
    if (this.seenBullets.size > 256) {
      for (const [id, fired] of this.seenBullets) if (tick - fired > 300) this.seenBullets.delete(id)
    }
  }

  /** Draws every bullet where it has got to by `renderTick`, and ends the ones that have arrived. */
  private drawBullets(renderTick: number) {
    for (const [id, bullet] of this.bullets) {
      const n = renderTick - bullet.spawn.tick
      if (n < 0) continue
      if (n >= Math.min(bullet.life, bullet.endTicks ?? Infinity)) {
        this.rocketModels.remove(id)
        this.trails.end('b', id)
        this.bullets.delete(id)
        continue
      }
      const at = bulletPosition(bullet.spawn, n)
      if (!bullet.shown) {
        bullet.shown = true
        this.bulletLeft(bullet, at)
      }
      // It leaves the eye, but is drawn leaving the barrel: the difference closes over the first 25 m.
      if (bullet.offset) {
        const flown = n * Math.hypot(bullet.spawn.vx, bullet.spawn.vy, bullet.spawn.vz) / TICK_RATE
        const keep = Math.max(0, 1 - flown / 25)
        at[0] += bullet.offset.x * keep
        at[1] += bullet.offset.y * keep
        at[2] += bullet.offset.z * keep
      }
      this.rocketModels.place(id, PROJECTILE_BULLET, at, bulletVelocity(bullet.spawn, n))
      // The ribbon starts where the round is first drawn, so it grows out of the barrel.
      this.trails.follow('b', id, PROJECTILE_BULLET, at[0], at[1], at[2])
    }
  }

  /** The moment a bullet is first drawn: the shooter's arm, a flash at the muzzle and the shot's sound. */
  private bulletLeft(bullet: LiveBullet, at: readonly number[]) {
    const { shooter, item } = bullet.spawn
    const gun = itemDef(item)
    this.avatars.get(shooter === this.localId ? 0 : shooter)?.fire()
    const muzzle = this.muzzleOf(shooter)
    if (muzzle) {
      bullet.offset = muzzle.clone().sub(new THREE.Vector3(at[0], at[1], at[2]))
      this.effects.muzzle(muzzle)
    }
    const heard = muzzle ?? new THREE.Vector3(at[0], at[1], at[2])
    this.sounds.gunshot([heard.x, heard.y, heard.z], gun?.sound ?? 'rifle')
  }

  /**
   * A burning flare, drawn each frame it is listed: a blooming halo, sparks dripping off it, and a strong,
   * gently flickering light on everything below. The halo lives two frames, so it looks the same at any frame rate.
   */
  private burnFlare(id: number, x: number, y: number, z: number) {
    const now = performance.now()
    const flicker = 0.85 + 0.15 * Math.sin(now * 0.031 + id) * Math.sin(now * 0.0117 + id * 2.3)
    this.scene.scene.lights.hold(x, y, z, { color: FLARE_LIGHT, intensity: FLARE_INTENSITY * flicker, radius: FLARE_LIGHT_RADIUS })
    const life = Math.max(this.frameDt, 1 / 240) * 2
    this.glow.spawn({ position: [x, y, z], velocity: [0, 0, 0], life, size: [1.6 * flicker, 1.4], color: 0xfff2dc, alpha: 0.6, glow: 2 })
    this.glow.spawn({ position: [x, y, z], velocity: [0, 0, 0], life, size: [0.5, 0.45], color: 0xffffff, alpha: 1, glow: 4 })
    if (Math.random() < this.frameDt * 10) {
      this.glow.spawn({
        position: [x, y, z],
        velocity: [(Math.random() - 0.5) * 1.5, -0.5 - Math.random(), (Math.random() - 0.5) * 1.5],
        life: 0.6 + Math.random() * 0.6,
        size: [0.14, 0.04],
        color: 0xffc46a,
        alpha: 1,
        gravity: 6,
        drag: 0.5,
        glow: 3,
      })
    }
  }

  /** The unspent fraction of a smoke puff, per rocket in flight. */
  private readonly trailOwed = new Map<number, number>()

  private clearRockets() {
    this.trailOwed.clear()
    this.bullets.clear()
    this.seenBullets.clear()
    for (const id of this.rocketInterp.ids()) this.rocketModels.remove(id)
    this.rocketInterp.clear()
    this.rocketEnds.clear()
  }

  /** Draws where a grenade thrown now would go, while one is in hand and the player is free to throw it. */
  private aimGrenade(position: readonly number[], seated: boolean) {
    const motor = this.motor
    const item = itemDef(this.held().item)
    this.mortarArc.set(null)
    if (motor && seated && this.seat?.index === 0 && this.seat.kind === KIND_MORTAR) {
      this.trajectory.set(null)
      this.mortarArc.set(this.mortarPath(motor))
      return
    }
    if (motor && seated && this.seat?.index === 0 && this.seat.kind === KIND_PLANE) {
      this.trajectory.set(this.bombPath(motor))
      return
    }
    if (!motor || seated || this.hp === 0 || this.localId === null || item?.kind !== 'grenade') {
      this.trajectory.set(null)
      return
    }
    const eye: [number, number, number] = [position[0]!, position[1]! + this.eyeHeight(), position[2]!]
    this.trajectory.set(throwTrajectory((origin, dir, max) => motor.castRay(origin, dir, max), eye, this.input.yaw, this.input.pitch))
  }

  /** For a field mortar's gunner: where a bomb fired now would land, from the tube's muzzle at the mortar's muzzle speed. */
  private mortarPath(motor: SimMotor) {
    const mount = this.cars.get(this.seat!.vehicle)
    if (!(mount instanceof MortarModel)) return null
    const { origin, direction } = mount.barrel()
    const start = origin.clone().addScaledVector(direction, MORTAR_BARREL_LENGTH)
    const velocity = direction.clone().multiplyScalar(MORTAR_SHELL_SPEED)
    // Only where it first lands matters: the bomb flies its whole lifetime, as the real one does.
    return dropTrajectory((from, dir, max) => motor.castRay(from, dir, max), [start.x, start.y, start.z], [velocity.x, velocity.y, velocity.z], 3, MORTAR_SHELL_TTL_TICKS)
  }

  /** For a plane's pilot with bombs on the rack and room under them: where one let go now would land. */
  private bombPath(motor: SimMotor) {
    const id = this.seat!.vehicle
    const plane = this.cars.get(id)
    const state = this.lastArms.get(id)
    const fx = this.vehicleFx.get(id)
    if (!(plane instanceof PlaneModel) || !fx || !state || state.bombs === 0) return null
    plane.root.updateMatrixWorld(true)
    const start = new THREE.Vector3(...PLANE_BOMB_PIVOT).applyMatrix4(new THREE.Matrix4().fromArray(plane.root.matrixWorld))
    if (start.y - this.scene.groundAt(start.x, start.z) < PLANE_BOMB_MIN_HEIGHT) return null
    const velocity = fx.velocity
    return dropTrajectory((origin, dir, max) => motor.castRay(origin, dir, max), [start.x, start.y, start.z], [velocity.x, velocity.y, velocity.z])
  }

  /** The slot in hand. */
  private held(): ItemSlot {
    return this.hotbar[this.input.selectedSlot] ?? { item: ITEM_NONE, count: 0, ammo: 0 }
  }

  /** The local player's own muzzle flash, at once, instead of after the server's reply. */
  private fireFeedback(frame: InputFrame) {
    const now = performance.now()
    const pressed = (frame.buttons & BUTTON_FIRE) !== 0
    const fresh = pressed && !this.firedBefore
    this.firedBefore = pressed
    if (this.hp === 0 || (this.seat && !crewGun(this.seat.kind, this.seat.index))) return
    const slot = this.held()
    const item = itemDef(slot.item)
    // A local mirror of the server's rules for a shot, so the flash does not wait for its reply.
    if (!item || item.kind !== 'gun' || !pressed || (!item.auto && !fresh)) return
    if (slot.ammo === 0 || this.progress > 0 || (frame.buttons & BUTTON_RELOAD) !== 0) return
    if (now - this.lastFireAt < item.cooldownTicks * TICK_MS) return
    this.lastFireAt = now
    const [x, y, z] = this.predictor!.state.position
    const yaw = this.input.yaw
    const pitch = this.input.pitch
    const flat = Math.cos(pitch)
    const direction = new THREE.Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat)
    const me = this.avatars.get(0)
    me?.fire()
    const barrel = me?.muzzle()
    this.effects.muzzle(barrel ?? new THREE.Vector3(x, y + EYE_HEIGHT, z).addScaledVector(direction, 0.6))
  }

  /**
   * While standing on a car, the stickman is predicted as if the car stood still where the newest
   * snapshot left it. This carries that position along to wherever the car is drawn now, so the
   * stickman stays glued to the roof and still moves on it.
   */
  /**
   * Where the rider of seat `index` of vehicle `model` (a `kind`) sits: their head point in the world
   * into `head` and the vehicle's turn (the mortar's mount's) into `rotation`. False for a crew that
   * does not show, who are not drawn.
   */
  private seatFrame(model: VehicleModel, kind: number, index: number, head: THREE.Vector3, rotation: THREE.Quaternion): boolean {
    if (model instanceof MortarModel) {
      if (index !== 0) return false
      model.seatFrame(head, rotation)
      return true
    }
    const local = seatHead(kind, index)
    if (!local) return false
    const { position, quaternion } = model.root
    rotation.set(quaternion.x, quaternion.y, quaternion.z, quaternion.w)
    head.set(...local).applyQuaternion(rotation)
    head.x += position.x
    head.y += position.y
    head.z += position.z
    return true
  }

  /** Height of the local player's eye above the capsule centre, for the stance they are in. */
  private eyeHeight(): number {
    if (this.predictor?.state.prone) return PRONE_EYE_HEIGHT
    return EYE_HEIGHT - (this.crouching ? CROUCH_EYE_DROP : 0)
  }

  private ridden(position: Vec3, ride: number, renderTick: number): Vec3 {
    const ref = this.refPoses.get(ride)
    const now = ride === 0 ? null : this.vehicleInterp.sample(ride, renderTick)
    if (!ref || !now) return position
    const local = new THREE.Vector3(position[0] - ref.x, position[1] - ref.y, position[2] - ref.z)
    local.applyQuaternion(new THREE.Quaternion(ref.qx, ref.qy, ref.qz, ref.qw).invert())
    local.applyQuaternion(new THREE.Quaternion(now.qx, now.qy, now.qz, now.qw))
    return [now.x + local.x, now.y + local.y, now.z + local.z]
  }

  /** A blob under a stickman whose capsule is centred at `(x, y, z)`. */
  private blobFoot(blobs: Scene['contact'], x: number, y: number, z: number) {
    const height = y - (CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS) - this.scene.groundAt(x, z)
    blobs.add(x, z, 0.6, 0.6, 0, Math.max(0, height))
  }

  /** The soft-lighting options in force. */
  get look() {
    return this.scene.look
  }

  /** Switches the soft-lighting options and remembers them for next time. */
  setLook(look: Look) {
    this.scene.setLook(look)
    this.applyGrass(look)
    saveLook(look)
  }

  private applyGrass(look: Look) {
    const coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
    this.grass?.setDensity(coarse ? GRASS_LITE : GRASS_FULL)
    this.grass?.setEnabled(look.grass)
  }

  /** Who parts the grass this frame: feet, wheels and tracks on it, and the downwash of anything with rotors low over it. */
  private bendGrass(position: readonly number[], onFoot: boolean, others: readonly { x: number; z: number }[], cars: ReadonlyMap<number, CarPlacement>) {
    const grass = this.grass
    if (!grass) return
    const benders = this.benders
    benders.length = 0
    if (onFoot && this.hp > 0) benders.push({ x: position[0]!, z: position[2]!, radius: 0.7, strength: 1.1 })
    for (const other of others) benders.push({ x: other.x, z: other.z, radius: 0.7, strength: 1.1 })
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = Math.max(0, car.y - hy - this.scene.groundAt(car.x, car.z))
      if (car.kind === KIND_HELI || car.kind === KIND_DRONE) {
        // Downwash: wider and weaker the higher it hovers, gone past `wash` metres up.
        const wash = car.kind === KIND_HELI ? 18 : 4
        if (height < wash) {
          const k = 1 - height / wash
          benders.push({ x: car.x, z: car.z, radius: (car.kind === KIND_HELI ? 6 : 1.5) + height * 0.5, strength: 1.4 * k })
        }
      } else if (height < 1) {
        benders.push({ x: car.x, z: car.z, radius: Math.hypot(hx, hz) * 1.05, strength: 1.4 })
      }
    }
    const eye = this.scene.camera.position
    grass.update(eye.x, eye.z, benders)
  }

  /** Draws every vehicle at the render tick and returns where they are, by id. */
  private updateCars(renderTick: number, driven: DrivenPose | null) {
    const placed = new Map<number, CarPlacement>()
    this.listen()
    this.sounds.begin()
    for (const id of this.vehicleInterp.ids()) {
      const state = this.vehicleInterp.sample(id, renderTick)
      if (!state) continue
      let car = this.cars.get(id)
      if (!car) {
        car = createVehicleModel(state.kind, id)
        this.cars.set(id, car)
        this.scene.scene.add(car.root)
      }
      // The vehicle the local player drives is drawn where it was predicted, not where it was seen.
      const mine = driven !== null && this.driver?.driving === id
      const [x, y, z] = mine ? driven.position : [state.x, state.y, state.z]
      const rotation = mine ? driven.rotation : ([state.qx, state.qy, state.qz, state.qw] as const)
      const steer = mine ? driven.steer : state.steer
      const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(new THREE.Quaternion(...rotation))
      const yaw = Math.atan2(-forward.x, -forward.z)
      // A hen draws its wings and legs from its flags: the driver's own as predicted, at once.
      if (car instanceof HenModel) car.setFlags(mine ? (this.driver?.thrust?.flags ?? 0) : (state.thrust?.flags ?? 0))
      car.update([x, y, z], [rotation[0], rotation[1], rotation[2], rotation[3]], steer, state.seats, state.turret, state.arms)
      if (car instanceof WalkerModel || car instanceof HenModel) for (const step of car.takeSteps()) this.sounds.footstep(step.at, step.weight)
      if (car instanceof HenModel) {
        const { beats, leapt, eggs } = car.takeEvents()
        if (leapt) this.sounds.bawk([x, y + 2, z])
        for (let k = 0; k < Math.min(beats, 2); k++) this.sounds.wingBeat([x, y, z])
        if (eggs > 0) this.sounds.plop([x, y - 1, z])
      }
      placed.set(id, { kind: state.kind, turret: state.turret, arms: state.arms, x, y, z, yaw, seats: state.seats, crew: state.crew })
      this.smoulder(id, state, [x, y, z], rotation)
      this.tyres(id, state, [x, y, z], rotation, steer)
      if (car instanceof CarModel) car.wear(state.damage, state.hp, this.debris, this.vehicleFx.get(id)?.velocity ?? STILL)
      if (car instanceof CarModel && state.thrust) {
        // The driver sees their own nozzles fire as predicted, at once.
        const flags = mine ? (this.driver?.thrust?.flags ?? 0) : state.thrust.flags
        if (flags) coldGas(this.fxWorld, car.jets(flags), this.vehicleFx.get(id)?.velocity ?? STILL, this.frameDt)
      }
      if (car instanceof TankModel && state.kind === KIND_TANK) {
        if (state.damage[DAMAGE_TURRET] === TURRET_GONE) this.shedTurret(id, car)
        else if (this.turretsShed.delete(id)) car.setTurret(true)
      }
      if ((car instanceof TankModel || car instanceof HeliModel || car instanceof WalkerModel || car instanceof MortarModel) && state.turret) this.watchGun(id, car, state.turret, state.seats)
      if (car instanceof PlaneModel) this.watchPlaneGuns(id, car, state)
      if (state.arms) this.lastArms.set(id, state.arms)
    }
    for (const [id, car] of this.cars) {
      if (!placed.has(id)) {
        car.dispose()
        this.cars.delete(id)
        this.shots.delete(id)
        this.forwardShots.delete(id)
        this.lastArms.delete(id)
        this.smokeOwed.delete(id)
        this.turretsShed.delete(id)
        this.vehicleFx.get(id)?.lift(this.fxWorld)
        this.vehicleFx.delete(id)
        this.sounds.drop(id)
      }
    }
    this.sounds.end(this.frameDt)
    return placed
  }

  /** Puts the ears where the camera is, last frame's pose, and moving with the vehicle the player is in. */
  private listen() {
    const camera = this.scene.camera
    const at = camera.position
    const look = camera.getWorldDirection(this.listenForward)
    const moving = this.seat ? this.vehicleFx.get(this.seat.vehicle)?.velocity : undefined
    this.sounds.setListener([at.x, at.y, at.z], [look.x, look.y, look.z], moving ? [moving.x, moving.y, moving.z] : [0, 0, 0])
  }

  private readonly listenForward = new THREE.Vector3()
  private readonly cameraAt = new THREE.Vector3()

  private fxWorldMade: FxWorld | null = null

  /** What vehicles' tyre effects draw into. */
  private get fxWorld(): FxWorld {
    this.fxWorldMade ??= {
      smoke: this.smoke,
      glow: this.glow,
      skids: this.skids,
      groundAt: (x, z) => this.scene.groundAt(x, z),
      surfaceAt: (x, z) => this.surfaces?.at(x, z) ?? SURFACE_GRASS,
      waterAt: (x, z) => this.waters?.levelAt(x, z) ?? NO_WATER,
      ripple: (at, size) => this.effects.ripple(at, size),
      camera: this.scene.camera.position,
    }
    return this.fxWorldMade
  }

  /** Skidmarks, tyre smoke, dust and exhaust for a vehicle drawn at `position`. */
  private tyres(id: number, state: VehicleState, position: readonly [number, number, number], rotation: readonly [number, number, number, number], steer: number) {
    let fx = this.vehicleFx.get(id)
    if (!fx || fx.kind !== state.kind) {
      fx = new VehicleFx(id, state.kind)
      this.vehicleFx.set(id, fx)
    }
    const powered = (state.seats[0] ?? 0) !== 0 && state.hp > 0 && state.kind !== KIND_MORTAR
    fx.update(this.fxWorld, position, rotation, steer, powered, this.frameDt)
    // The driver's own car is drawn from its prediction, so what is heard follows the wheel at once.
    const landing = fx.takeLanding()
    if (landing > 0) this.sounds.thud([position[0], position[1], position[2]], landing)
    const hurt = state.damage.some((wear) => wear > 0)
    this.sounds.engine(
      id,
      {
        kind: state.kind,
        speed: fx.forwardSpeed,
        accel: fx.acceleration,
        climb: fx.velocity.y,
        powered,
        hp: state.hp,
        maxHp: vehicleMaxHp(state.kind),
        slip: fx.slip,
        surface: fx.surfaceUnder,
        grounded: fx.grounded,
        wheels: hurt ? state.damage.slice(DAMAGE_WHEELS, DAMAGE_WHEELS + 4) : null,
        cut: ((state.arms?.flags ?? 0) & ARMS_CUT) !== 0,
        jets: this.driver?.driving === id ? this.driver.thrust?.flags : state.thrust?.flags,
      },
      [position[0], position[1], position[2]],
      [fx.velocity.x, fx.velocity.y, fx.velocity.z],
      this.seat?.vehicle === id,
    )
  }

  /** A plume from a damaged vehicle: thin steam under 40% health, black smoke and flame as it dies, a burning column once a wreck. */
  private smoulder(id: number, state: VehicleState, position: readonly [number, number, number], rotation: readonly [number, number, number, number]) {
    const limit = vehicleMaxHp(state.kind) * 0.4
    if (state.hp >= limit) {
      this.smokeOwed.delete(id)
      return
    }
    const owed = damageSmoke(
      this.fxWorld,
      state.kind,
      position,
      new THREE.Quaternion(...rotation),
      this.vehicleFx.get(id)?.velocity ?? STILL,
      1 - state.hp / limit,
      state.hp === 0,
      this.frameDt,
      this.smokeOwed.get(id) ?? 0,
    )
    this.smokeOwed.set(id, owed)
    // A burning wreck lights its surroundings with a flicker. Each light lives a fraction of a second, so one per frame holds it.
    if (state.hp === 0) {
      const flicker = 0.7 + 0.3 * Math.sin(performance.now() * 0.021 + id * 1.7) * Math.sin(performance.now() * 0.0073 + id)
      this.scene.scene.lights.flash(position[0], position[1] + 1.5, position[2], { color: 0xff7a24, intensity: 2.2 * flicker, radius: 16, life: 0.08 })
    }
  }

  /**
   * Points the look where the nose gun does. The gun's angles are in the hull frame, the look is in the
   * world's, so the hull's orientation turns one into the other.
   */
  private lookAlong(heli: { root: Node }, gun: Turret) {
    const flat = Math.cos(gun.pitch)
    const local = new THREE.Vector3(-Math.sin(gun.yaw) * flat, Math.sin(gun.pitch), -Math.cos(gun.yaw) * flat)
    const world = local.applyQuaternion(heli.root.quaternion)
    this.input.yaw = Math.atan2(-world.x, -world.z)
    this.input.pitch = Math.asin(THREE.MathUtils.clamp(world.y, -1, 1))
  }

  /**
   * Keeps the look within a mounted gun's travel, which is in the hull's frame: the same limits the
   * server applies, so the view never points somewhere the gun cannot follow.
   */
  private limitGun(mount: { root: Node }, yawLimit: number, pitchMin: number, pitchMax: number) {
    const flat = Math.cos(this.input.pitch)
    const world = new THREE.Vector3(-Math.sin(this.input.yaw) * flat, Math.sin(this.input.pitch), -Math.cos(this.input.yaw) * flat)
    const local = world.applyQuaternion(new THREE.Quaternion().copy(mount.root.quaternion).invert())
    const yaw = THREE.MathUtils.clamp(Math.atan2(-local.x, -local.z), -yawLimit, yawLimit)
    const pitch = THREE.MathUtils.clamp(Math.asin(THREE.MathUtils.clamp(local.y, -1, 1)), pitchMin, pitchMax)
    const limited = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)).applyQuaternion(mount.root.quaternion)
    this.input.yaw = Math.atan2(-limited.x, -limited.z)
    this.input.pitch = Math.asin(THREE.MathUtils.clamp(limited.y, -1, 1))
  }

  /** Sends tank `id`'s turret flying, once: a copy of it tumbles off as debris and the tank is left with the bare ring. */
  private shedTurret(id: number, tank: TankModel) {
    if (this.turretsShed.has(id)) return
    this.turretsShed.add(id)
    const copy = tank.turretCopy()
    tank.setTurret(false)
    if (!copy) return
    const spread = () => (Math.random() - 0.5) * 8
    this.debris.throw(copy, new THREE.Vector3(spread(), 9 + Math.random() * 3, spread()), new THREE.Vector3(spread() / 2, spread() / 3, spread() / 2), 0.5, 25)
  }

  /** A change in the shot counter is a shell leaving: flash and recoil. The shell itself is a projectile. */
  private watchGun(id: number, tank: TankModel | HeliModel | WalkerModel | MortarModel, turret: Turret, seats: readonly number[]) {
    const before = this.shots.get(id)
    this.shots.set(id, turret.shots)
    // The first sight of a gun is not a shot, nor is a counter that has not moved.
    if (before === undefined || before === turret.shots) return
    // A walker's barrels fire in turn: the counter has already gone up for this shot.
    const { origin, direction } = tank instanceof WalkerModel ? tank.barrel((turret.shots - 1) % 2 === 0 ? -1 : 1) : tank.barrel()
    tank.recoil()
    const length = tank instanceof HeliModel ? HELI_GUN_LENGTH : tank instanceof WalkerModel ? WALKER_GUN_LENGTH : tank instanceof StrvModel ? STRV_BARREL_LENGTH : tank instanceof MortarModel ? MORTAR_BARREL_LENGTH : TANK_BARREL_LENGTH
    const muzzle = origin.clone().addScaledVector(direction, length)
    this.effects.muzzleFlash(muzzle, direction)
    this.muzzleLight(muzzle, 0xffc060, 4, 14)
    if (tank instanceof WalkerModel) this.sounds.laser([muzzle.x, muzzle.y, muzzle.z])
    else this.sounds.cannon([muzzle.x, muzzle.y, muzzle.z], tank instanceof TankModel)
    if (seats[tank instanceof WalkerModel || tank instanceof StrvModel || tank instanceof MortarModel ? 0 : 1] === this.localId) this.lastShotAt = performance.now()
  }

  /**
   * A plane's guns: every new round on either counter is drawn as a tracer from its muzzle, out to the
   * first wall or the end of the gun's range, with a flash, a kick and the rattle. Only rounds that hit
   * something come back from the server, as sparks.
   */
  private watchPlaneGuns(id: number, plane: PlaneModel, state: VehicleState) {
    const end = (origin: THREE.Vector3, direction: THREE.Vector3) => {
      const hit = this.motor?.castRay([origin.x, origin.y, origin.z], [direction.x, direction.y, direction.z], PLANE_GUN_RANGE)
      return origin.clone().addScaledVector(direction, hit ? hit.t : PLANE_GUN_RANGE)
    }
    const forward = state.arms?.shots
    const before = this.forwardShots.get(id)
    if (forward !== undefined) this.forwardShots.set(id, forward)
    if (forward !== undefined && before !== undefined && before !== forward) {
      // At most a few rounds pass between two frames; more is a gap in the stream, not a burst to draw.
      const rounds = Math.min(4, (forward - before + 256) % 256)
      for (let k = 1; k <= rounds; k++) {
        const side = (before + k) % 2
        const { origin, direction } = plane.forwardGun(side)
        plane.recoil(side)
        this.effects.round(origin, end(origin, direction))
      }
      const { origin } = plane.forwardGun(0)
      this.sounds.machineGun([origin.x, origin.y, origin.z], rounds)
    }
    const turret = state.turret
    if (!turret) return
    const ringBefore = this.shots.get(id)
    this.shots.set(id, turret.shots)
    if (ringBefore === undefined || ringBefore === turret.shots) return
    const rounds = Math.min(4, (turret.shots - ringBefore + 256) % 256)
    const { direction } = plane.barrel()
    const muzzle = plane.muzzle()
    for (let k = 0; k < rounds; k++) this.effects.round(muzzle, end(muzzle, direction))
    plane.recoilRing()
    this.sounds.machineGun([muzzle.x, muzzle.y, muzzle.z], rounds, true)
  }

  /** What a plane's crew sees on their gauges: the heat of their guns, the bombs, the stall warning and the gunsight. */
  private cockpit(ride: CarPlacement | undefined, aviator: boolean): Pick<HudStats, 'heat' | 'jammed' | 'bombs' | 'stall'> {
    const arms = ride?.kind === KIND_PLANE ? ride.arms : null
    if (!this.seat || !arms) return { heat: null, jammed: false, bombs: null, stall: false }
    if (!aviator) return { heat: arms.rearHeat, jammed: (arms.flags & ARMS_REAR_JAMMED) !== 0, bombs: null, stall: false }
    const fx = this.vehicleFx.get(this.seat.vehicle)
    const stall = fx !== undefined && !fx.grounded && fx.forwardSpeed < PLANE_STALL_SPEED * 1.1
    return { heat: arms.heat, jammed: (arms.flags & ARMS_JAMMED) !== 0, bombs: arms.bombs, stall }
  }

  /**
   * Where the pilot's two markers are on the screen, as fractions of its width and height, every frame:
   * the nose, which is where the guns point a little way out, and the aim, the heading the mouse asks for.
   * They are given apart from the HUD stats, which are throttled, so they move smoothly.
   */
  /**
   * Puts the camera on the barrel's line, looking exactly where a shell leaves, and lays out the
   * reticle over it: the crosshair is the barrel's line and the marks below it are where a shell lands
   * at each range. A shell leaves at `SHELL_SPEED` and falls under gravity, so `s` metres out it is
   * `g (s / v)² / 2` below the line. Returns the barrel's heading.
   */
  private lookDownBarrel(model: TankModel | StrvModel): number {
    const { origin, direction } = model.barrel()
    const yaw = Math.atan2(-direction.x, -direction.z)
    const pitch = Math.asin(Math.max(-1, Math.min(1, direction.y)))
    const eye = new THREE.Vector3(origin.x, origin.y + SIGHT_EYE_UP, origin.z)
    this.scene.scopeFrom([eye.x, eye.y, eye.z], yaw, pitch)
    const camera = this.scene.camera
    const right = new THREE.Vector3(Math.cos(yaw), 0, -Math.sin(yaw))
    const up = new THREE.Vector3().crossVectors(right, direction)
    const reach = Math.tan((camera.fov * Math.PI) / 360)
    const level = Math.hypot(direction.x, direction.z)
    const marks: Mark[] = []
    for (const range of SIGHT_RANGES) {
      if (level < 1e-3) break
      const along = range / level
      const fall = 0.5 * GRAVITY * (along / SHELL_SPEED) ** 2
      // From the eye, which sits a little above the muzzle's start.
      const to = direction.clone().multiplyScalar(along).add(new THREE.Vector3(0, -fall - SIGHT_EYE_UP, 0))
      const depth = to.dot(direction)
      if (depth <= 0) continue
      const y = to.dot(up) / depth
      if (y > -reach * 0.9) marks.push({ range, y })
    }
    // The ray starts past the muzzle, so it does not find the gun's own hull.
    const start = eye.clone().addScaledVector(direction, SIGHT_RAY_START)
    const hit = this.motor?.castRay([start.x, start.y, start.z], [direction.x, direction.y, direction.z], SIGHT_RANGE_MAX)
    this.scene.gunsight.show(camera.fov, marks, hit ? hit.t + SIGHT_RAY_START : null)
    return yaw
  }

  private sights(aviator: boolean) {
    let nose: [number, number] | null = null
    let aim: [number, number] | null = null
    const plane = aviator && this.seat ? this.cars.get(this.seat.vehicle) : undefined
    if (plane instanceof PlaneModel) {
      const { origin, direction } = plane.forwardGun(0)
      const toScreen = (point: THREE.Vector3): [number, number] | null => {
        // Kept on the screen: the view may be a way behind the aim.
        if (!this.scene.camera.project(point)) return null
        return [clamp01((point.x + 1) / 2), clamp01((1 - point.y) / 2)]
      }
      nose = toScreen(origin.clone().addScaledVector(direction, 150))
      const { yaw, pitch } = this.input
      const flat = Math.cos(pitch)
      const wanted = new THREE.Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat)
      aim = toScreen(new THREE.Vector3().copy(plane.root.position).addScaledVector(wanted, 150))
    }
    this.onSights?.(nose, aim)
  }

  /** The nearest item on the ground within reach of `position`, as the server will judge it. */
  private pickupInReach(position: readonly number[]): PickupState | null {
    let best: PickupState | null = null
    let bestDistance = PICKUP_RADIUS
    for (const pickup of this.pickups) {
      if (Math.abs(pickup.y - position[1]!) > 2.5) continue
      const distance = Math.hypot(pickup.x - position[0]!, pickup.z - position[2]!)
      if (distance <= bestDistance) {
        best = pickup
        bestDistance = distance
      }
    }
    return best
  }

  /** The key hint for what E does here. */
  private prompt(position: readonly number[], cars: ReadonlyMap<number, CarPlacement>): string | null {
    if (this.localId === null) return null
    if (this.seat) return isDrone(this.seat.kind) ? null : 'E: get out'
    const me = this.predictor?.state
    if (me && this.hp > 0 && !me.grounded && position[1]! - (this.terrain?.heightAt(position[0]!, position[2]!) ?? 0) > CHUTE_PROMPT_HEIGHT) {
      return me.chute ? 'G: cut the parachute' : 'G: open the parachute'
    }
    // The server hands over an item before it boards a car, so the hint follows the same order.
    const pickup = this.pickupInReach(position)
    if (pickup) return `E: take ${itemDef(pickup.item)?.name ?? 'item'}`
    let best: { distance: number; driverSeatFree: boolean; kind: number } | null = null
    for (const car of cars.values()) {
      // A walker or a hen is boarded from between its feet, as the server does (`boarding_point`).
      const y = isLegged(car.kind) ? car.y - WALKER_LEG_LENGTH : car.y
      const distance = Math.hypot(car.x - position[0]!, y - position[1]!, car.z - position[2]!)
      // A drone is only ever flown by whoever launched it.
      const hasFreeSeat = !isDrone(car.kind) && car.seats.includes(0)
      if (hasFreeSeat && distance <= ENTER_RADIUS && (!best || distance < best.distance)) best = { distance, driverSeatFree: car.seats[0] === 0, kind: car.kind }
    }
    if (!best) return null
    if (best.kind === KIND_MORTAR) return 'E: man the mortar'
    if (best.kind === KIND_TANK) return best.driverSeatFree ? 'E: drive the tank' : 'E: man the gun'
    if (best.kind === KIND_STRV) return 'E: drive the Strv S'
    if (best.kind === KIND_HELI) return best.driverSeatFree ? 'E: fly' : 'E: man the nose gun'
    if (best.kind === KIND_PLANE) return best.driverSeatFree ? 'E: fly the triplane' : 'E: man the rear gun'
    if (best.kind === KIND_SPORTS) return best.driverSeatFree ? 'E: drive the sports car' : 'E: ride along'
    if (best.kind === KIND_ROADSTER) return best.driverSeatFree ? 'E: drive the Roadster' : 'E: ride along'
    if (best.kind === KIND_WALKER) return 'E: drive the walker'
    if (best.kind === KIND_HEN) return 'E: drive the hen'
    return best.driverSeatFree ? 'E: drive the jeep' : 'E: ride along'
  }

  /** Places and animates rooster `id`, and lets it be heard when it crows: louder the nearer it is. */
  private rooster(id: number, state: { x: number; y: number; z: number; yaw: number; crowing: boolean }, dt: number, listener: readonly number[]) {
    let model = this.roosters.get(id)
    if (!model) {
      model = new RoosterModel()
      this.roosters.set(id, model)
      this.scene.scene.add(model.root)
    }
    if (model.update([state.x, state.y, state.z], state.yaw, state.crowing, dt)) {
      const distance = Math.hypot(state.x - listener[0]!, state.z - listener[2]!)
      playCrow((1 - Math.min(1, distance / ROOSTER_HEARING)) ** 2)
    }
  }

  /** The stickman for an entity; id 0 is the local player. */
  private avatar(id: number) {
    let avatar = this.avatars.get(id)
    if (!avatar) {
      avatar = new Character(this.crowd, id === 0 ? (this.localId ?? 0) : id)
      this.avatars.set(id, avatar)
    }
    return avatar
  }

  dispose() {
    this.disposed = true
    this.perf.dispose()
    cancelAnimationFrame(this.frameHandle)
    clearTimeout(this.reconnectTimer)
    clearInterval(this.pingTimer)
    this.transport.close()
    this.voice.dispose()
    this.input.dispose()
    this.driver?.dispose()
    this.motor?.dispose()
    this.tiles?.dispose()
    this.crowd.dispose()
    this.effects.dispose()
    this.smoke.dispose()
    this.glow.dispose()
    this.skids.dispose()
    this.sounds.dispose()
    this.debris.dispose()
    this.view.dispose()
    this.rocketModels.dispose()
    this.pickupModels.dispose()
    for (const model of this.roosters.values()) model.dispose()
    this.trajectory.dispose()
    this.mortarArc.dispose()
    this.scene.dispose()
  }
}
