import * as THREE from 'three'
import {
  ARMS_JAMMED, FLAG_GROUNDED, ARMS_REAR_JAMMED, encodeInput, encodePing, KIND_MORTAR, toAxis, toMrad, WEAPON_GRENADE, WEAPON_MORTAR, WEAPON_RICOCHET, WEAPON_TURRET,
  PROJECTILE_BULLET, WEAPON_BULLET, WEAPON_FLARE, WEAPON_AUTOCANNON, WEAPON_DRONE, WEAPON_LOITER, WEAPON_BOMB, WEAPON_EGG, WEAPON_CRASH, WEAPON_MG, WEAPON_PELLET, WEAPON_ROCKET, WEAPON_SCRAPE, WEAPON_SHELL, WEAPON_SHOTGUN, WEAPON_SNIPER, WEAPON_WRECK,
  type BulletSpawn, type DriveState, type InputFrame, type ServerMessage, type ShotEvent,
} from './net/codec'
import { Backoff } from './net/backoff'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport } from './net/transport'
import {
  BUTTON_CROUCH, BUTTON_FIRE, BUTTON_RELOAD, CROUCH_EYE_DROP, PRONE_EYE_HEIGHT, SLIDE_MIN_SPEED, ENTER_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, INPUT_REDUNDANCY, PLAYER_MAX_HP, PROTOCOL_VERSION, RESPAWN_TICKS,
  ROCKET_BLAST_RADIUS, SHELL_COOLDOWN_TICKS, STANDING_Y, STANCE_CHUTE, STANCE_PRONE,
  CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS,
  CANNON_RADIUS,
  MORTAR_BARREL_LENGTH, MORTAR_RADIUS, MORTAR_SHELL_SPEED, MORTAR_SHELL_TTL_TICKS, WALKER_LEG_LENGTH, TICK_MS, TICK_RATE, WRECK_BLAST_RADIUS,
  BOMB_RADIUS, PLANE_BOMB_MIN_HEIGHT, PLANE_BOMB_PIVOT, PLANE_STALL_SPEED,
} from './sim/consts'
import { crewGun, isLegged, seatHead } from './sim/vehicles'
import { aimSpeedScale, DRONE_BLAST_RADIUS, LOITER_BLAST_RADIUS, emptyHotbar, GRENADE_BLAST_RADIUS, HOTBAR_SLOTS, hitscanGunFor, ITEM_NONE, ITEM_SNIPER, itemDef, PICKUP_RADIUS, type ItemDef, type ItemSlot } from './sim/items'
import { InputController } from './sim/input'
import { SimMotor, type VehiclePose } from './sim/motor'
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
import { crashBurst, scrapeSparks, type FxWorld } from './render/tyres'
import { Debris } from './render/debris'
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
import { AIM_DISTANCE_SCALE, ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, Scene } from './render/scene'
import { ChaseCam } from './render/chase'
import { PerfReadout } from './render/perf'
import { encodeRtc } from './net/codec'
import { VoiceChat } from './voice'
import { PlaneModel } from './render/plane'
import { MortarModel } from './render/mortar'
import { loadVehicleModels } from './render/vehicles'
import { Crowd } from './render/character'
import { Cameras } from './cameras'
import { rideView, seatView } from './seats'
import { loadRoosterModel } from './render/rooster'
import { VehicleSounds } from './vehicleaudio'
import { HIT_DEALT, HIT_KILL, HIT_ROOSTER, HIT_TAKEN, HIT_VEHICLE, HitFeedback } from './feedback'
import { buildGround } from './render/world'
import { Replica } from './net/replica'
import { EntityStore, inGameUnits } from './entities/store'
import { Presenters, type Frame } from './entities/presenter'
import { Avatars } from './entities/avatars'
import { Projectiles } from './entities/presenters/projectiles'
import { Pickups as LyingItems, type Lying } from './entities/presenters/pickups'
import { Stickmen } from './entities/presenters/people'
import { poseOf, Vehicles, type Placed, type VehicleWorld } from './entities/presenters/vehicles'
import { PlaneGuns, Turrets } from './entities/presenters/guns'
import { Wear } from './entities/presenters/wear'
import { Smoke } from './entities/presenters/smoke'
import { Jets } from './entities/presenters/jets'
import { Roosters } from './entities/presenters/roosters'
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
/** Falling this high (metres) over the ground, a hint offers the parachute. */
const CHUTE_PROMPT_HEIGHT = 4
/** Wading ripples are drawn within this many metres of the camera. */
const WADE_RIPPLE_RANGE = 60
const MAP_SIDES = { colorOf: (id: number) => cssColor(colorFor(id)), isBot, allies }

/** Players sitting in cars, which the stickman list does not include. */
/** How far behind a vehicle the camera sits. */
const clamp01 = (value: number) => Math.min(0.94, Math.max(0.06, value))
/** A vehicle with no motion on record. */
const STILL = new THREE.Vector3()

/** Players sitting in vehicles who are not already counted in `seen` (drawn sitting), nor `local`. */
const riders = (cars: ReadonlyMap<number, Placed>, seen: ReadonlySet<number>, local: number | null) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0 && rider !== local && !seen.has(rider)) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
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
/** And a little slower aiming a gun over the shoulder. */
const AIM_LOOK_SCALE = 0.6
/** How long the static shows after a drone's feed cuts out, ms. */
const FEED_LOST_MS = 700

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
  /** Every stickman is drawn by this one set of instanced meshes. */
  private readonly crowd = new Crowd()
  /** Every player's stickman, whoever draws it. */
  private readonly avatars = new Avatars(this.crowd)
  /** Rebuilds snapshots from deltas, and says which one to diff the next against. */
  private readonly replica = new Replica()
  /** Every replicated entity in the game's units, sampled at the render tick for the presenters. */
  private readonly store = new EntityStore()
  /** What is drawn in flight and on the ground, before the lights are updated: a flare holds one. */
  private readonly flying = new Presenters<Frame>()
  /** Everyone else on foot, and the creatures, where the contact shadows are gathered. */
  private readonly people = new Presenters<Frame>()
  private readonly stickmen: Stickmen
  /** The vehicles and what they carry, first in the frame: the camera follows them. */
  private readonly machines = new Presenters<Frame>()
  private readonly vehicles: Vehicles
  private readonly wear: Wear
  /** The vehicle the local player drives, as predicted this frame. */
  private driven: DrivenPose | null = null
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
  private readonly effects = new Effects()
  private readonly damageNumbers = new DamageNumbers()
  /** Tyre smoke, dust, gravel and spray; sparks and flames glow on their own pool. */
  private readonly smoke = new Particles(4096, false, true)
  private readonly glow = new Particles(2048, true)
  private readonly skids = new Skidmarks()
  /** Engines, tyres, crashes and guns. */
  private readonly sounds = new VehicleSounds()
  /** Bumpers, doors and wheels that came off. */
  private readonly debris = new Debris(160)
  private readonly gore = new Gore(this.debris, this.smoke, this.effects)
  /** Seconds since the last frame, for what is drawn per vehicle. */
  private frameDt = 0
  private readonly rocketModels = new Rockets()
  /** Ribbons behind rockets, shells and bullets in flight. */
  private readonly trails = new ProjectileTrails()
  /** Shot and burst events wait here until the render tick reaches them, so they line up with what is drawn. */
  private pendingShots: { tick: number; shot: ShotEvent }[] = []
  /** Bullets in flight, flown from their spawn alone (`sim/ballistics.ts`); the server lists none after announcing them. */
  private readonly bullets = new Map<number, LiveBullet>()
  /** Bullets already seen, with the tick they were fired on: a spawn is sent twice and must not start a second round. */
  private readonly seenBullets = new Map<number, number>()
  private readonly chase = new ChaseCam()
  private readonly pickupModels = new Pickups()
  /** Items on the ground, as drawn. */
  private readonly lying = new LyingItems(this.pickupModels)
  /** The arc a grenade in hand would take, drawn while one is held. */
  private readonly trajectory = new TrajectoryView()
  private readonly mortarArc = new MortarArc()
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
  private lastShotAt = -Infinity
  /** Scratch for a seat's head point and its vehicle's turn. */
  private readonly seatAt = new THREE.Vector3()
  private readonly seatTurn = new THREE.Quaternion()
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
  /** Where the camera goes while seated, by the seat's rig. */
  private readonly cameras: Cameras
  private predictor: Predictor | null = null
  private motor: SimMotor | null = null
  /** Predicts the vehicle the local player drives. */
  private driver: VehicleDriver | null = null
  /** The newest snapshot of the driven vehicle, waiting for the next frame to be reconciled with. */
  private driverSnapshot: { ack: number; vehicle: VehiclePose; drive: DriveState } | null = null
  /** Vehicle poses of the newest snapshot, which the predicted stickman's frame is fixed to while riding. */
  private readonly refPoses = new Map<number, VehiclePose>()
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
    this.cameras = new Cameras(this.scene, this.input, (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null)
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
    const world: VehicleWorld = {
      root: this.scene.scene, sounds: this.sounds, effects: this.effects, debris: this.debris, lights: this.scene.scene.lights, fx: () => this.fxWorld,
      castRay: (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null,
      driven: () => (this.driven && this.driver?.driving ? { id: this.driver.driving, pose: this.driven, jets: this.driver.thrust?.flags ?? 0 } : null),
      seated: () => this.seat?.vehicle ?? null,
      listen: () => this.listen(),
      ownShot: () => (this.lastShotAt = performance.now()),
    }
    this.vehicles = this.machines.add(new Vehicles(world))
    this.wear = this.machines.add(new Wear(world, this.vehicles))
    this.machines.add(new Smoke(world, this.vehicles))
    this.machines.add(new Jets(world, this.vehicles))
    this.machines.add(new Turrets(world, this.vehicles))
    this.machines.add(new PlaneGuns(world, this.vehicles))
    this.flying.add(new Projectiles({
      models: this.rocketModels, trails: this.trails, glow: this.glow, lights: this.scene.scene.lights, fx: () => this.fxWorld, blips: this.blips,
    }))
    this.flying.add(this.lying)
    const blobFoot = (x: number, y: number, z: number) => this.blobFoot(this.scene.contact, x, y, z)
    this.stickmen = this.people.add(new Stickmen({ avatars: this.avatars, blobFoot, wade: (id, x, y, z, speed, dt) => this.wade(id, x, y, z, speed, dt) }))
    this.people.add(new Roosters({ root: this.scene.scene, listener: () => this.predictor?.state.position ?? [0, 0, 0], blobFoot }))
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
    this.avatars.local()
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
    this.replica.reset()
    this.flying.clear()
    this.people.clear()
    this.machines.clear()
    this.store.clear()
    this.clearBullets()
    this.hotbar = emptyHotbar()
    this.progress = 0
    this.pendingShots = []
    this.blips.clear()
    this.motor?.setVehicles([])
    this.clock.reset()
    this.avatars.forgetOthers()
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
        this.localId = this.avatars.localId = message.id
        this.awaitingCatchUp = true
        this.replica.reset()
        // Recreate the local stickman in this player's colour.
        this.avatars.forgetLocal()
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
        this.store.push(message.tick, rebuilt.scene, rebuilt.latest)
        this.clock.observe(message.tick, now)
        this.setHealth(message.hp, now)
        this.progress = message.progress / 255
        this.hotbar = [...message.inventory, ...emptyHotbar()].slice(0, HOTBAR_SLOTS)
        for (const shot of message.shots) this.pendingShots.push({ tick: message.tick, shot })
        if (this.pendingShots.length > 64) this.pendingShots.splice(0, this.pendingShots.length - 64)
        this.noteBullets(message.tick, message.bullets)
        // Vehicles are solid to the predicted stickman, at the pose of the newest snapshot.
        this.refPoses.clear()
        let mount: VehiclePose | null = null
        let index = -1
        for (const [id, wire] of rebuilt.scene) {
          if (!wire.orientation || !wire.place) continue
          const vehicle = inGameUnits(wire)
          const pose = poseOf(id, vehicle)
          this.refPoses.set(id, pose)
          const seat = vehicle.seats?.riders.indexOf(this.localId) ?? -1
          if (seat >= 0) [mount, index] = [pose, seat]
        }
        this.motor?.setVehicles([...this.refPoses.values()].map((pose) => {
          const [vx, vz] = this.store.latestVelocity(pose.id) ?? [0, 0]
          return { ...pose, vx, vz }
        }))
        const wasSeated = this.seat !== null
        this.seat = mount ? { vehicle: mount.id, kind: mount.kind, index } : null
        // A door closing, for getting in or out (not for hopping seats).
        if (wasSeated !== (this.seat !== null) && this.hp > 0) this.sounds.door()
        const listed = rebuilt.scene.get(this.localId)
        const own = listed && inGameUnits(listed)
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
        } else if (own?.place && own.flags && this.predictor) {
          const { x, y, z } = own.place
          const { flags, stance } = own.flags
          this.predictor.reconcile(message.ack, {
            position: [x, y, z], yaw: own.heading?.yaw ?? 0, vy: own.motion?.vy ?? 0, grounded: (flags & FLAG_GROUNDED) !== 0,
            knock: [own.motion?.knock_x ?? 0, own.motion?.knock_z ?? 0], ride: message.ride, prone: stance === STANCE_PRONE, chute: stance === STANCE_CHUTE,
          })
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
        this.store.remove(message.id)
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
          this.avatars.of(id)?.setColor(colorFor(id))
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
        const body = this.avatars.of(message.victim)
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
    const seat = this.seat
    const seated = seat !== null
    const view = seat ? seatView(seat.kind, seat.index) : null
    // Flying a drone: the body kneels where it launched it, and the view is the drone's camera.
    const flying = view?.rig.name === 'fpv'
    this.driven = this.driver?.pose(this.accumulator / TICK_MS, dt) ?? null
    // Everything replicated, sampled once at the render tick; each group of presenters draws its part.
    const sampled = this.store.sampleAll(renderTick)
    const frame: Frame = { tick: renderTick, dt, now, localId: this.localId }
    this.machines.run(sampled, frame)
    const cars = this.vehicles.placed

    const position = this.ridden(predictor.renderPosition(this.accumulator / TICK_MS, dt), predictor.state.ride, renderTick)
    const local = predictor.state
    const localVx = (local.position[0] - predictor.previous.position[0]) / (TICK_MS / 1000)
    const localVz = (local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)
    this.avatars.begin()
    const me = this.avatars.local()
    // A car's passenger fires their own guns from their seat, as one on foot does.
    const passenger = seat !== null && crewGun(seat.kind, seat.index)
    const hands = !seated || passenger
    // Where the local player sits, when they show in their seat: their head, and the vehicle's turn.
    const mount = seated && !flying ? this.vehicles.model(seat.vehicle) : undefined
    const sitting = mount !== undefined && this.seatFrame(mount, seat!.kind, seat!.index, this.seatAt, this.seatTurn)
    // Aim held with a scoped gun in hand, alive and free to use it: look down the scope.
    const scoped = this.localId !== null && hands && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.scope === true
    this.scoped = scoped
    // Aim held with any other gun: half zoom over the shoulder.
    const aiming = !scoped && this.localId !== null && hands && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.kind === 'gun'
    this.aiming = aiming
    this.aimBlend += ((aiming ? 1 : 0) - this.aimBlend) * Math.min(1, dt * 14)
    const ride = seat ? cars.get(seat.vehicle) : undefined
    const distance = ride ? rideView(ride.kind).distance : 0
    // Aim held at a gun with a sight (a tank's, a Strv S's): look down it, which shows where the barrel really points.
    const sighted = this.hp > 0 && this.input.aimHeld && ride !== undefined && view?.sight === true
    this.input.lookScale = sighted ? SIGHT_LOOK_SCALE : scoped ? SCOPE_LOOK_SCALE : aiming ? AIM_LOOK_SCALE : 1
    const chase = ride && view?.chase ? this.chase.update([ride.x, ride.y, ride.z], ride.yaw, dt, distance) : null
    if (!chase) this.chase.reset()
    this.scene.zoomTo(this.cameras.zoom(view, this.input.aimHeld, sighted, scoped, aiming), dt, chase?.fovBoost ?? 0)
    me.hidden = (seated && !flying && !sitting) || scoped
    me.dead = this.hp === 0
    me.held = this.hotbar[this.input.selectedSlot]?.item ?? ITEM_NONE
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    if (sitting) me.seat(this.seatAt, this.seatTurn, this.input.yaw, this.input.pitch, dt)
    else me.update(position, flying ? local.yaw : this.input.yaw, localVx, localVz, local.grounded, dt, flying ? 0 : this.input.pitch, this.crouching || flying,
      this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED, local.prone, local.chute)
    if (!seated && this.hp > 0) this.wade(0, position[0], position[1], position[2], Math.hypot(localVx, localVz), dt)
    else this.wading.delete(0)
    const aviator = view?.rig.name === 'aviator'
    const link = view?.rig.name === 'fpv' ? view.rig : null
    if (seat && ride && view) {
      this.cameras.seated({
        now, dt, vehicle: seat.vehicle, seat: view, ride, model: this.vehicles.model(seat.vehicle), motion: this.vehicles.fx(seat.vehicle),
        distance, sighted, scoped, aimBlend: this.aimBlend, chase, head: sitting ? this.seatAt : null,
      }, seat.index)
    } else {
      this.cameras.leave(now)
      if (scoped) this.scene.scopeFrom([position[0]!, position[1]! + this.eyeHeight(), position[2]!], this.input.yaw, this.input.pitch)
      // Only the distance changes: the pivot is what the server's crosshair ray is built from.
      else this.scene.follow(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * this.aimBlend), ON_FOOT_SHOULDER, ON_FOOT_HEIGHT)
    }
    // Streaming follows what the camera is on: the car when seated, else the stickman.
    this.tiles?.update(ride ? ride.x : position[0], ride ? ride.z : position[2])
    this.playShots(renderTick)
    this.flying.run(sampled, frame)
    this.drawBullets(renderTick)
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

    this.people.run(sampled, frame)
    const nearby = this.stickmen.nearby
    this.bendGrass(position, !seated, nearby, cars)
    // Everyone else sitting where they show: in their seat, turned where they look.
    for (const [id, car] of cars) {
      const model = this.vehicles.model(id)
      if (!model) continue
      car.seats.forEach((rider, index) => {
        if (rider === 0 || rider === this.localId || !this.seatFrame(model, car.kind, index, this.seatAt, this.seatTurn)) return
        const look = car.crew[index]
        nearby.push({ id: rider, x: this.seatAt.x, y: this.seatAt.y, z: this.seatAt.z })
        const avatar = this.avatars.draw(rider)
        avatar.dead = false
        avatar.held = look?.held ?? ITEM_NONE
        avatar.seat(this.seatAt, this.seatTurn, look?.yaw ?? car.yaw, look?.pitch ?? 0, dt)
      })
    }
    this.avatars.sweep((key) => this.wading.delete(key))

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
      players: this.localId === null ? 1 : this.avatars.drawn.size + riders(cars, this.avatars.drawn, this.localId),
      interpDelayMs: (this.clock.interpDelay * TICK_MS),
      tiles: this.tiles?.loaded ?? 0,
      correction: this.driver?.driving ? this.driver.lastCorrection : predictor.lastCorrection,
      pending: predictor.pendingCount,
      locked: this.input.locked,
      touch: this.input.coarse,
      warning: this.warning,
      prompt: this.prompt(position, cars),
      seat: view?.role ?? null,
      canSwitchSeat: this.seat !== null && (cars.get(this.seat.vehicle)?.seats.includes(0) ?? false),
      range: view?.gauges?.includes('range') ? (ride?.turret?.range ?? 0) : 0,
      reload: this.reloaded(now, ride ? rideView(ride.kind).reload : SHELL_COOLDOWN_TICKS),
      ...this.cockpit(view?.gauges?.includes('cockpit') ? ride : undefined, aviator),
      air: view?.gauges?.includes('air') ? (this.driver?.thrust?.air ?? null) : null,
      eggs: view?.gauges?.includes('eggs') ? (ride?.arms?.bombs ?? null) : null,
      drone: link && ride
        ? {
          battery: this.refPoses.get(seat!.vehicle)?.air ?? 1,
          signal: Math.max(0, 1 - Math.hypot(ride.x - position[0]!, ride.z - position[2]!) / link.link),
          camera: link.belly ? this.cameras.loiterCam : null,
        }
        : null,
      feedLost: Math.max(0, 1 - (now - this.cameras.feedLostAt) / FEED_LOST_MS),
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
      board: this.input.isHeld('Tab') ? sides(this.board.rows(this.localId, this.stickmen.inView), teamFor) : [],
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

  /** How far the gun the local player last fired has reloaded, 0..1, for one taking `ticks` (null: never reloads). */
  private reloaded(now: number, ticks: number | null): number {
    return ticks === null ? 1 : Math.min(1, (now - this.lastShotAt) / (ticks * TICK_MS))
  }

  /** Where the local player's allies in view stand. */
  private alliesInView(): { id: number; x: number; z: number }[] {
    if (this.localId === null || teamFor(this.localId) === 0) return []
    const found: { id: number; x: number; z: number }[] = []
    for (const id of this.stickmen.inView) {
      const avatar = this.avatars.of(id)
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
    const avatar = this.avatars.of(shooter)
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
      this.avatars.of(shot.shooter)?.fire()
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
      this.wear.shedTurret(shot.shooter)
      this.blast(to, CANNON_RADIUS, BLASTS.cannon)
      this.sounds.turretOff([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_CRASH) {
      const car = this.vehicles.model(shot.shooter)
      const velocity = this.vehicles.fx(shot.shooter)?.velocity ?? STILL
      if (car instanceof CarModel) car.noteImpact(to)
      crashBurst(this.fxWorld, to, car instanceof CarModel ? car.paintColor : 0x555555, velocity)
      this.sounds.crash([to.x, to.y, to.z], this.vehicles.fx(shot.shooter)?.impactSpeed ?? 0)
      // Felt hard by its driver, a little by anyone close by.
      if (this.driver?.driving === shot.shooter) this.chase.kick(0.7)
      else if (to.distanceTo(this.scene.camera.position) < 20) this.chase.kick(0.15)
    } else if (shot.weapon === WEAPON_SCRAPE) {
      scrapeSparks(this.fxWorld, to, this.vehicles.fx(shot.shooter)?.velocity ?? STILL)
      this.sounds.scrape(shot.shooter)
    } else if (shot.weapon === WEAPON_SNIPER) {
      // The round is drawn in flight; this is where it landed.
      if (!this.watery(to, 0.45)) this.effects.burst(to, 0.8)
    } else if ((gun = hitscanGunFor(shot.weapon))) {
      const from = this.muzzleOf(shot.shooter)
      this.avatars.of(shot.shooter)?.fire()
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
    this.avatars.of(hit.target)?.flash()
  }

  /** Plays the events whose tick the picture has reached. */
  private playShots(renderTick: number) {
    let played = 0
    while (played < this.pendingShots.length && this.pendingShots[played]!.tick <= renderTick) this.showShot(this.pendingShots[played++]!.shot)
    if (played > 0) this.pendingShots.splice(0, played)
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
    this.avatars.of(shooter)?.fire()
    const muzzle = this.muzzleOf(shooter)
    if (muzzle) {
      bullet.offset = muzzle.clone().sub(new THREE.Vector3(at[0], at[1], at[2]))
      this.effects.muzzle(muzzle)
    }
    const heard = muzzle ?? new THREE.Vector3(at[0], at[1], at[2])
    this.sounds.gunshot([heard.x, heard.y, heard.z], gun?.sound ?? 'rifle')
  }

  private clearBullets() {
    this.bullets.clear()
    this.seenBullets.clear()
  }

  /** Draws where a grenade thrown now would go, while one is in hand and the player is free to throw it. */
  private aimGrenade(position: readonly number[], seated: boolean) {
    const motor = this.motor
    const item = itemDef(this.held().item)
    this.mortarArc.set(null)
    const arc = this.seat ? seatView(this.seat.kind, this.seat.index).arc : undefined
    if (motor && seated && arc === 'mortar') {
      this.trajectory.set(null)
      this.mortarArc.set(this.mortarPath(motor))
      return
    }
    if (motor && seated && arc === 'bombs') {
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
    const mount = this.vehicles.model(this.seat!.vehicle)
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
    const plane = this.vehicles.model(id)
    const state = this.vehicles.placed.get(id)?.arms
    const fx = this.vehicles.fx(id)
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
    const me = this.avatars.local()
    me.fire()
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
    const now = ride === 0 ? null : this.store.sample(ride, renderTick)?.entity
    if (!ref || !now?.place || !now.orientation) return position
    const local = new THREE.Vector3(position[0] - ref.x, position[1] - ref.y, position[2] - ref.z)
    local.applyQuaternion(new THREE.Quaternion(ref.qx, ref.qy, ref.qz, ref.qw).invert())
    const { qx, qy, qz, qw } = now.orientation
    local.applyQuaternion(new THREE.Quaternion(qx, qy, qz, qw))
    return [now.place.x + local.x, now.place.y + local.y, now.place.z + local.z]
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
  private bendGrass(position: readonly number[], onFoot: boolean, others: readonly { x: number; z: number }[], cars: ReadonlyMap<number, Placed>) {
    const grass = this.grass
    if (!grass) return
    const benders = this.benders
    benders.length = 0
    if (onFoot && this.hp > 0) benders.push({ x: position[0]!, z: position[2]!, radius: 0.7, strength: 1.1 })
    for (const other of others) benders.push({ x: other.x, z: other.z, radius: 0.7, strength: 1.1 })
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = Math.max(0, car.y - hy - this.scene.groundAt(car.x, car.z))
      const wash = rideView(car.kind).downwash
      if (wash) {
        // Downwash: wider and weaker the higher it hovers, gone past `reach` metres up.
        if (height < wash.reach) benders.push({ x: car.x, z: car.z, radius: wash.radius + height * 0.5, strength: 1.4 * (1 - height / wash.reach) })
      } else if (height < 1) {
        benders.push({ x: car.x, z: car.z, radius: Math.hypot(hx, hz) * 1.05, strength: 1.4 })
      }
    }
    const eye = this.scene.camera.position
    grass.update(eye.x, eye.z, benders)
  }

  /** Puts the ears where the camera is, last frame's pose, and moving with the vehicle the player is in. */
  private listen() {
    const camera = this.scene.camera
    const at = camera.position
    const look = camera.getWorldDirection(this.listenForward)
    const moving = this.seat ? this.vehicles.fx(this.seat.vehicle)?.velocity : undefined
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



  /** What a plane's crew sees on their gauges: the heat of their guns, the bombs, the stall warning and the gunsight. */
  private cockpit(ride: Placed | undefined, aviator: boolean): Pick<HudStats, 'heat' | 'jammed' | 'bombs' | 'stall'> {
    const arms = ride?.arms ?? null
    if (!this.seat || !arms) return { heat: null, jammed: false, bombs: null, stall: false }
    if (!aviator) return { heat: arms.rear_heat, jammed: (arms.flags & ARMS_REAR_JAMMED) !== 0, bombs: null, stall: false }
    const fx = this.vehicles.fx(this.seat.vehicle)
    const stall = fx !== undefined && !fx.grounded && fx.forwardSpeed < PLANE_STALL_SPEED * 1.1
    return { heat: arms.heat, jammed: (arms.flags & ARMS_JAMMED) !== 0, bombs: arms.bombs, stall }
  }

  /**
   * Where the pilot's two markers are on the screen, as fractions of its width and height, every frame:
   * the nose, which is where the guns point a little way out, and the aim, the heading the mouse asks for.
   * They are given apart from the HUD stats, which are throttled, so they move smoothly.
   */

  private sights(aviator: boolean) {
    let nose: [number, number] | null = null
    let aim: [number, number] | null = null
    const plane = aviator && this.seat ? this.vehicles.model(this.seat.vehicle) : undefined
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
  private pickupInReach(position: readonly number[]): Lying | null {
    let best: Lying | null = null
    let bestDistance = PICKUP_RADIUS
    for (const pickup of this.lying.lying) {
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
  private prompt(position: readonly number[], cars: ReadonlyMap<number, Placed>): string | null {
    if (this.localId === null) return null
    // A drone is flown from afar: there is no getting out of it.
    if (this.seat) return seatView(this.seat.kind, this.seat.index).board ? 'E: get out' : null
    const me = this.predictor?.state
    if (me && this.hp > 0 && !me.grounded && position[1]! - (this.terrain?.heightAt(position[0]!, position[2]!) ?? 0) > CHUTE_PROMPT_HEIGHT) {
      return me.chute ? 'G: cut the parachute' : 'G: open the parachute'
    }
    // The server hands over an item before it boards a car, so the hint follows the same order.
    const pickup = this.pickupInReach(position)
    if (pickup) return `E: take ${itemDef(pickup.item)?.name ?? 'item'}`
    // The nearest vehicle in reach with a seat free, which the server boards into its lowest free seat.
    let best: { distance: number; board: string } | null = null
    for (const car of cars.values()) {
      // A walker or a hen is boarded from between its feet, as the server does (`boarding_point`).
      const y = isLegged(car.kind) ? car.y - WALKER_LEG_LENGTH : car.y
      const distance = Math.hypot(car.x - position[0]!, y - position[1]!, car.z - position[2]!)
      const free = car.seats.indexOf(0)
      // A drone's seat is boarded by nobody: it is flown by whoever launched it.
      const board = free < 0 ? undefined : seatView(car.kind, free).board
      if (board && distance <= ENTER_RADIUS && (!best || distance < best.distance)) best = { distance, board }
    }
    return best?.board ?? null
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
    this.people.clear()
    this.trajectory.dispose()
    this.mortarArc.dispose()
    this.scene.dispose()
  }
}
