import * as THREE from 'three'
import {
  ARMS_CUT, ARMS_JAMMED, ARMS_REAR_JAMMED, DAMAGE_WHEELS, encodeInput, encodePing, KIND_HELI, KIND_PLANE, KIND_ROOSTER, KIND_SPORTS, KIND_STICKMAN, KIND_TANK, sceneStates, toAxis, toMrad, WEAPON_GRENADE,
  PROJECTILE_BULLET, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_CRASH, WEAPON_MG, WEAPON_ROCKET, WEAPON_SCRAPE, WEAPON_SHELL, WEAPON_SNIPER, WEAPON_WRECK,
  type Arms, type EntityState, type InputFrame, type PickupState, type RocketState, type ServerMessage, type ShotEvent, type Turret, type VehicleState,
} from './net/codec'
import { blendEntity, blendRocket, blendVehicle, Interpolator } from './net/interp'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport } from './net/transport'
import {
  BUTTON_CROUCH, BUTTON_FIRE, BUTTON_RELOAD, CROUCH_EYE_DROP, SLIDE_MIN_SPEED, ENTER_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, INPUT_REDUNDANCY, PLAYER_MAX_HP, PROTOCOL_VERSION, RESPAWN_TICKS,
  ROCKET_BLAST_RADIUS, SHELL_COOLDOWN_TICKS, STANDING_Y,
  CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS,
  HELI_GUN_COOLDOWN_TICKS, HELI_GUN_LENGTH, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT, CANNON_RADIUS,
  TANK_BARREL_LENGTH, TANK_BARREL_MAX, TANK_BARREL_MIN, TICK_MS, vehicleMaxHp, WRECK_BLAST_RADIUS,
  BOMB_RADIUS, PLANE_AIM_MAX_PITCH, PLANE_CRUISE_SPEED, PLANE_BOMB_MIN_HEIGHT, PLANE_BOMB_PIVOT, PLANE_GUN_RANGE, PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN, PLANE_STALL_SPEED,
} from './sim/consts'
import { vehicleDef } from './sim/vehicles'
import { aimSpeedScale, emptyHotbar, GRENADE_BLAST_RADIUS, HOTBAR_SLOTS, hitscanGunFor, ITEM_NONE, itemDef, PICKUP_RADIUS, type ItemDef, type ItemSlot } from './sim/items'
import { InputController } from './sim/input'
import { RapierMotor } from './sim/physics'
import { Terrain } from './sim/terrain'
import { VehicleDriver, type DrivenPose } from './sim/vehicle'
import { Predictor } from './sim/predict'
import type { Vec3 } from './sim/physics'
import { CarModel, type VehicleModel } from './render/car'
import { Effects } from './render/effects'
import { DamageNumbers } from './render/damagenumbers'
import { Particles } from './render/particles'
import { Skidmarks } from './render/skidmarks'
import { crashBurst, scrapeSparks, VehicleFx, type FxWorld } from './render/tyres'
import { Debris } from './render/debris'
import { Gore } from './render/gore'
import { Surfaces, SURFACE_GRASS } from './sim/surface'
import { Pickups } from './render/pickups'
import { TrajectoryView } from './render/trajectory'
import { dropTrajectory, throwTrajectory } from './sim/grenade'
import { Rockets } from './render/rockets'
import { saveLook, type Look } from './render/look'
import { halfExtentsOf } from './sim/kinds'
import { AIM_DISTANCE_SCALE, CAMERA_HEIGHT, ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, PLANE_CAMERA_HEIGHT, PLANE_DISTANCE, Scene } from './render/scene'
import { ChaseCam } from './render/chase'
import { HeliModel } from './render/heli'
import { wrapAngle } from './net/codec'
import { PlaneModel } from './render/plane'
import { TankModel } from './render/tank'
import { cameraDistance, createVehicleModel, loadVehicleModels } from './render/vehicles'
import { Character, Crowd } from './render/character'
import { loadRoosterModel, RoosterModel } from './render/rooster'
import { playCrow } from './audio'
import { VehicleSounds } from './vehicleaudio'
import { HIT_DEALT, HIT_KILL, HIT_ROOSTER, HIT_TAKEN, HIT_VEHICLE, HitFeedback } from './feedback'
import { buildGround } from './render/world'
import { updateWater } from './render/water'
import { Replica } from './net/replica'
import { TileStreamer } from './stream'
import { loadManifest, type WorldData } from './world'
import { WorldView } from './worldview'
import type { HudStats } from './hud'
import { Scoreboard } from './scoreboard'

// The overlay and its tests import these from here.
export type { HudStats } from './hud'
export type { FeedEntry, ScoreRow } from './scoreboard'

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

const riders = (cars: ReadonlyMap<number, CarPlacement>) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
const RECONNECT_MS = 3000
/** A crow can be heard this far off, metres. */
const ROOSTER_HEARING = 45
/** How long a kill stays in the feed. */
/** Mouse-look is this much slower down a scope, to match the narrower view. */
const SCOPE_LOOK_SCALE = 0.3
/** And a little slower aiming a gun over the shoulder. */
const AIM_LOOK_SCALE = 0.6

/**
 * The client game loop. Fixed 60 Hz ticks sample input, predict the local player and send inputs;
 * every animation frame renders the predicted local player and interpolated remote players.
 */
export class Game {
  private readonly scene: Scene
  readonly input: InputController
  private readonly transport: Transport
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
  /** True from the welcome until the joiner's catch-up `destroyed` arrives: that one is state, not news. */
  private awaitingCatchUp = false
  private readonly cars = new Map<number, VehicleModel>()
  private readonly effects = new Effects()
  private readonly damageNumbers = new DamageNumbers()
  /** Tyre smoke, dust, gravel and spray; sparks and flames glow on their own pool. */
  private readonly smoke = new Particles(3000)
  private readonly glow = new Particles(600, true)
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
  /** The tick a rocket was last listed in, once it stops being listed: it bursts then. */
  private readonly rocketEnds = new Map<number, number>()
  /** Shot and burst events wait here until the render tick reaches them, so they line up with what is drawn. */
  private pendingShots: { tick: number; shot: ShotEvent }[] = []
  private readonly smokeAt = new Map<number, number>()
  private readonly chase = new ChaseCam()
  private readonly pickupModels = new Pickups()
  /** The arc a grenade in hand would take, drawn while one is held. */
  private readonly trajectory = new TrajectoryView()
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
  /** The local player's health, from the newest snapshot. Zero is dead. */
  private hp = PLAYER_MAX_HP
  /** `performance.now()` when the local player died, or null while alive. */
  private diedAt: number | null = null
  private lastFireAt = -Infinity
  /** Kills and deaths of everyone, kept up to date by the server's kill events. */
  private readonly board = new Scoreboard()
  /** Hits dealt and taken, for the crosshair marker, the screen flash and the direction arcs. */
  private readonly hits = new HitFeedback()
  /** The vehicle seat the server says the local player sits in; null when on foot. */
  private seat: { vehicle: number; kind: number; index: number } | null = null
  private wasGunner = false
  private wasAviator = false
  /** The yaw and pitch a plane's camera has eased to, behind the mouse's aim. */
  private planeView: [number, number] = [0, 0]
  private predictor: Predictor | null = null
  private motor: RapierMotor | null = null
  /** Predicts the vehicle the local player drives. */
  private driver: VehicleDriver | null = null
  /** Vehicle poses of the newest snapshot, which the predicted stickman's frame is fixed to while riding. */
  private readonly refPoses = new Map<number, VehicleState>()
  private localId: number | null = null
  private worldHash = 0
  private seq = 0
  private recent: InputFrame[] = []
  private accumulator = 0
  private lastFrame = 0
  private frameHandle = 0
  private reconnectTimer = 0
  private pingTimer = 0
  private disposed = false
  /** Set when reconnecting cannot help (protocol mismatch); only a reload can. */
  private halted = false
  private warning: string | null = null

  /** Called every frame with the pilot's gunsight and mouse-aim circle on screen, or nulls. */
  onSights: ((nose: [number, number] | null, aim: [number, number] | null) => void) | null = null

  constructor(container: HTMLElement, private readonly onStats: (stats: HudStats) => void) {
    this.scene = new Scene(container)
    this.input = new InputController(this.scene.canvas)
    this.view = new WorldView(this.scene.scene)
    this.scene.scene.add(this.crowd.root)
    this.scene.scene.add(this.effects.root, this.view.damage.root, this.damageNumbers.root)
    this.scene.scene.add(this.rocketModels.root)
    this.scene.scene.add(this.pickupModels.root)
    this.scene.scene.add(this.trajectory.root)
    // Translucent smoke and flashes, and the grenade's arc, would show as solid shapes in the AO pass.
    this.scene.excludeFromAO(this.effects.root)
    this.scene.scene.add(this.smoke.root, this.glow.root, this.skids.root)
    for (const root of [this.smoke.root, this.glow.root, this.skids.root]) this.scene.excludeFromAO(root)
    this.smoke.groundAt = (x, z) => this.scene.groundAt(x, z)
    this.scene.scene.add(this.debris.root)
    this.debris.groundAt = (x, z) => this.scene.groundAt(x, z)
    this.scene.excludeFromAO(this.view.damage.root)
    this.scene.excludeFromAO(this.trajectory.root)
    // Water is translucent and drawn after everything solid; the AO pass would see it as a lid.
    this.scene.scene.add(this.view.waterRoot)
    this.scene.excludeFromAO(this.view.waterRoot)
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
    const { world, hash } = await loadManifest()
    this.worldHash = hash
    this.world = world
    this.view.world = world
    // On a hilly world the plane sits just under the terrain, so it only shows where a tile is late.
    this.scene.scene.add(buildGround(world.halfExtent, world.groundCell > 0 ? -0.05 : 0))
    this.terrain = this.view.terrain = new Terrain(world.halfExtent, world.tileSize)
    this.surfaces = this.view.surfaces = new Surfaces(world.halfExtent, world.tileSize)
    this.scene.groundAt = (x, z) => this.terrain?.heightAt(x, z) ?? 0
    const motor = await RapierMotor.create(world)
    this.motor = this.view.motor = motor
    this.driver = this.view.driver = await VehicleDriver.create(world)
    await Promise.all([this.crowd.load(), loadVehicleModels(), loadRoosterModel()])
    // The buildings arrive as tiles, around wherever the player is.
    this.tiles = new TileStreamer(world, { attach: (ref, tile) => this.view.attachTile(ref, tile) })
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    await this.tiles.prime(x, z)
    this.predictor = new Predictor(motor, { position: [x, this.terrain.heightAt(x, z) + STANDING_Y, z], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0 })
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
      await this.transport.connect()
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
    this.hits.clear()
    this.hp = PLAYER_MAX_HP
    this.diedAt = null
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
      this.reconnectTimer = window.setTimeout(() => void this.connect(), RECONNECT_MS)
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
        const rebuilt = this.replica.receive(message.tick, message.baseline, message.delta)
        if (!rebuilt) return
        const { entities, vehicles } = sceneStates(rebuilt.scene)
        this.clock.observe(message.tick, now)
        this.setHealth(message.hp, now)
        this.progress = message.progress / 255
        this.hotbar = [...message.inventory, ...emptyHotbar()].slice(0, HOTBAR_SLOTS)
        this.pickups = message.pickups
        for (const shot of message.shots) this.pendingShots.push({ tick: message.tick, shot })
        if (this.pendingShots.length > 64) this.pendingShots.splice(0, this.pendingShots.length - 64)
        this.trackRockets(message.tick, message.rockets)
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
        for (const vehicle of vehicles) for (const rider of vehicle.seats) if (rider !== 0) this.interp.remove(rider)
        const own = entities.find((e) => e.id === this.localId)
        this.driver?.setObstacles(vehicles)
        if (mount && this.seat?.index === 0 && message.drive) this.driver?.reconcile(message.ack, mount, message.drive)
        else {
          this.driver?.leave()
          this.driver?.acknowledge(message.ack)
        }
        if (this.seat) {
          // The stickman is parked inside the car. Inputs still go out; the server just applies them to the car.
          this.predictor?.acknowledge(message.ack)
        } else if (own && this.predictor) {
          this.predictor.reconcile(message.ack, { position: [own.x, own.y, own.z], yaw: own.yaw, vy: own.vy, grounded: own.grounded, knock: [own.knockX, own.knockZ], ride: message.ride })
        }
        break
      }
      case 'pong':
        this.rtt.pong(message.nonce, now)
        break
      case 'leave':
        this.interp.remove(message.id)
        this.board.forget(message.id)
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
      case 'scores':
        this.board.replace(message.scores)
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
      case 'kill': {
        const body = this.avatars.get(message.victim === this.localId ? 0 : message.victim)
        if (body?.visible) this.gore.die(body, message.style, message.yaw, message.power, (x, z) => this.scene.groundAt(x, z))
        this.board.kill(message, now)
        break
      }
    }
  }

  private tick() {
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
    const dt = Math.min(0.25, (now - this.lastFrame) / 1000)
    this.frameDt = dt
    this.lastFrame = now

    updateWater(now / 1000)
    this.view.update(dt, this.effects, this.scene.camera.position)
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
    const seated = this.seat !== null
    const driven = this.driver?.pose(this.accumulator / TICK_MS, dt) ?? null
    const cars = this.updateCars(renderTick, driven)

    const position = this.ridden(predictor.renderPosition(this.accumulator / TICK_MS, dt), predictor.state.ride, renderTick)
    const local = predictor.state
    const localVx = (local.position[0] - predictor.previous.position[0]) / (TICK_MS / 1000)
    const localVz = (local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)
    const me = this.avatar(0)
    // Aim held with a scoped gun in hand, on foot and alive: look down the scope.
    const scoped = this.localId !== null && !seated && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.scope === true
    this.scoped = scoped
    // Aim held with any other gun: half zoom over the shoulder.
    const aiming = !scoped && this.localId !== null && !seated && this.hp > 0 && this.input.aimHeld && itemDef(this.held().item)?.kind === 'gun'
    this.aiming = aiming
    this.aimBlend += ((aiming ? 1 : 0) - this.aimBlend) * Math.min(1, dt * 14)
    this.input.lookScale = scoped ? SCOPE_LOOK_SCALE : aiming ? AIM_LOOK_SCALE : 1
    const driving = seated ? cars.get(this.seat!.vehicle) : undefined
    const chase = driving && this.seat!.index === 0 && driving.kind !== KIND_HELI && driving.kind !== KIND_PLANE ? this.chase.update([driving.x, driving.y, driving.z], driving.yaw, dt, cameraDistance(driving.kind)) : null
    if (!chase) this.chase.reset()
    this.scene.zoomTo(scoped ? 'scope' : aiming ? 'aim' : 'none', dt, chase?.fovBoost ?? 0)
    me.hidden = seated || scoped
    me.dead = this.hp === 0
    me.held = this.hotbar[this.input.selectedSlot]?.item ?? ITEM_NONE
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    me.update(position, this.input.yaw, localVx, localVz, local.grounded, dt, this.input.pitch, this.crouching,
      this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED)
    const ride = seated ? cars.get(this.seat!.vehicle) : undefined
    const gunner = ride !== undefined && vehicleDef(ride.kind).gun && this.seat!.index === 1
    const turret = gunner ? this.cars.get(this.seat!.vehicle) : undefined
    const aviator = ride?.kind === KIND_PLANE && this.seat!.index === 0
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
      this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch)
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
    } else if (ride) {
      this.input.followYaw(ride.yaw, dt)
      const yaw = this.input.yaw + (chase?.lean ?? 0)
      this.scene.follow([ride.x, ride.y + 0.4, ride.z], yaw, this.input.pitch, cameraDistance(ride.kind) + (chase?.distance ?? 0), 0, CAMERA_HEIGHT, chase?.shake)
    } else if (scoped) {
      this.scene.scopeFrom([position[0]!, position[1]! + EYE_HEIGHT - (this.crouching ? CROUCH_EYE_DROP : 0), position[2]!], this.input.yaw, this.input.pitch)
    } else {
      // Only the distance changes: the pivot is what the server's crosshair ray is built from.
      this.scene.follow(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * this.aimBlend), ON_FOOT_SHOULDER, ON_FOOT_HEIGHT)
    }

    this.wasGunner = gunner
    this.wasAviator = aviator
    // Streaming follows what the camera is on: the car when seated, else the stickman.
    this.tiles?.update(ride ? ride.x : position[0], ride ? ride.z : position[2])
    this.playShots(renderTick)
    this.drawRockets(renderTick)
    this.pickupModels.set(this.pickups)
    this.pickupModels.update(now / 1000)
    this.aimGrenade(position, seated)
    this.effects.update(dt)
    this.damageNumbers.update(dt)
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
      const avatar = this.avatar(id)
      avatar.dead = state.dead
      avatar.held = state.held
      if (!state.dead) this.blobFoot(blobs, state.x, state.y, state.z)
      avatar.update([state.x, state.y, state.z], state.yaw, state.velocity[0], state.velocity[1], state.grounded, dt, 0, state.crouching,
        state.crouching && state.grounded && Math.hypot(state.knockX, state.knockZ) > SLIDE_MIN_SPEED)
    }
    for (const [id, avatar] of this.avatars) {
      if (!seen.has(id)) {
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

    blobs.end()
    this.scene.render()
    this.sights(aviator)
    const hitView = this.hits.view(now, this.input.yaw)
    this.onStats({
      status: this.transport.status,
      rtt: this.rtt.rtt,
      players: this.localId === null ? 1 : seen.size + riders(cars) - (seated ? 1 : 0),
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
          ? this.seat.kind === KIND_HELI ? 'pilot' : this.seat.kind === KIND_PLANE ? 'aviator' : 'driver'
          : vehicleDef(this.seat.kind).gun ? 'gunner' : 'passenger',
      canSwitchSeat: this.seat !== null && (cars.get(this.seat.vehicle)?.seats.includes(0) ?? false),
      reload: this.seat?.kind === KIND_PLANE ? 1 : Math.min(1, (now - this.lastShotAt) / ((this.seat?.kind === KIND_HELI ? HELI_GUN_COOLDOWN_TICKS : SHELL_COOLDOWN_TICKS) * TICK_MS)),
      ...this.cockpit(ride, aviator),
      hp: this.hp,
      maxHp: PLAYER_MAX_HP,
      dead: this.hp === 0,
      respawnIn: this.diedAt === null ? 0 : Math.max(0, (RESPAWN_TICKS * TICK_MS - (now - this.diedAt)) / 1000),
      scoped: this.scoped,
      aiming: this.aiming,
      armed: this.localId !== null && !seated && this.hp > 0 && itemDef(this.held().item)?.kind === 'gun',
      onFoot: this.localId !== null && !seated && this.hp > 0,
      hotbar: this.hotbar,
      selected: this.input.selectedSlot,
      progress: this.progress,
      kills: this.board.kills(this.localId),
      deaths: this.board.deaths(this.localId),
      feed: this.board.recent(now, this.localId),
      board: this.input.isHeld('Tab') ? this.board.rows(this.localId, this.interp.ids()) : [],
      hitMarker: hitView.marker,
      hurt: hitView.vignette,
      hurtArcs: hitView.arcs,
      x: position[0],
      z: position[2],
      yaw: this.input.yaw,
      halfExtent: this.world?.halfExtent ?? 0,
    })
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
      this.effects.burst(to, ROCKET_BLAST_RADIUS)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_GRENADE) {
      this.effects.burst(to, GRENADE_BLAST_RADIUS)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_SHELL) {
      this.effects.burst(to, EXPLOSION_RADIUS)
      this.sounds.blast([to.x, to.y, to.z], 'big')
    } else if (shot.weapon === WEAPON_WRECK) {
      this.effects.burst(to, WRECK_BLAST_RADIUS)
      this.sounds.explosion([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_AUTOCANNON) {
      this.effects.burst(to, CANNON_RADIUS)
      this.sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_MG) {
      // The tracer was drawn when the gun fired; this is a round striking something.
      this.effects.strike(to)
    } else if (shot.weapon === WEAPON_BOMB) {
      this.effects.burst(to, BOMB_RADIUS)
      this.sounds.explosion([to.x, to.y, to.z])
      if (to.distanceTo(this.scene.camera.position) < 40) this.chase.kick(0.3)
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
      this.effects.burst(to, 0.8)
    } else if ((gun = hitscanGunFor(shot.weapon))) {
      const from = this.muzzleOf(shot.shooter)
      this.avatars.get(shot.shooter === this.localId ? 0 : shot.shooter)?.fire()
      if (from) this.effects.tracer(from, to, from.distanceTo(to) < gun.range - 1)
      const at = from ?? to
      this.sounds.gunshot([at.x, at.y, at.z], gun.sound)
    }
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
    // A sniper round's crack is heard as it leaves, not where it lands (it may never land).
    const known = new Set(this.rocketInterp.ids())
    for (const rocket of rockets) {
      if (rocket.kind === PROJECTILE_BULLET && !known.has(rocket.id)) this.sounds.gunshot([rocket.x, rocket.y, rocket.z], 'sniper')
    }
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
        continue
      }
      const state = this.rocketInterp.sample(id, renderTick)
      if (state) this.rocketModels.place(id, state.kind, [state.x, state.y, state.z], [state.vx, state.vy, state.vz])
    }
  }

  private clearRockets() {
    for (const id of this.rocketInterp.ids()) this.rocketModels.remove(id)
    this.rocketInterp.clear()
    this.rocketEnds.clear()
  }

  /** Draws where a grenade thrown now would go, while one is in hand and the player is free to throw it. */
  private aimGrenade(position: readonly number[], seated: boolean) {
    const motor = this.motor
    const item = itemDef(this.held().item)
    if (motor && seated && this.seat?.index === 0 && this.seat.kind === KIND_PLANE) {
      this.trajectory.set(this.bombPath(motor))
      return
    }
    if (!motor || seated || this.hp === 0 || this.localId === null || item?.kind !== 'grenade') {
      this.trajectory.set(null)
      return
    }
    const eye: [number, number, number] = [position[0]!, position[1]! + EYE_HEIGHT - (this.crouching ? CROUCH_EYE_DROP : 0), position[2]!]
    this.trajectory.set(throwTrajectory((origin, dir, max) => motor.castRay(origin, dir, max), eye, this.input.yaw, this.input.pitch))
  }

  /** For a plane's pilot with bombs on the rack and room under them: where one let go now would land. */
  private bombPath(motor: RapierMotor) {
    const id = this.seat!.vehicle
    const plane = this.cars.get(id)
    const state = this.lastArms.get(id)
    const fx = this.vehicleFx.get(id)
    if (!(plane instanceof PlaneModel) || !fx || !state || state.bombs === 0) return null
    plane.root.updateMatrixWorld(true)
    const start = new THREE.Vector3(...PLANE_BOMB_PIVOT).applyMatrix4(plane.root.matrixWorld)
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
    if (this.hp === 0 || this.seat) return
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
    saveLook(look)
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
      car.update([x, y, z], [rotation[0], rotation[1], rotation[2], rotation[3]], steer, state.seats, state.turret, state.arms)
      placed.set(id, { kind: state.kind, turret: state.turret, arms: state.arms, x, y, z, yaw, seats: state.seats })
      this.smoulder(id, state, [x, y, z])
      this.tyres(id, state, [x, y, z], rotation, steer)
      if (car instanceof CarModel) car.wear(state.damage, state.hp, this.debris, this.vehicleFx.get(id)?.velocity ?? STILL)
      if ((car instanceof TankModel || car instanceof HeliModel) && state.turret) this.watchGun(id, car, state.turret, state.seats)
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
        this.smokeAt.delete(id)
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

  private fxWorldMade: FxWorld | null = null

  /** What vehicles' tyre effects draw into. */
  private get fxWorld(): FxWorld {
    this.fxWorldMade ??= {
      smoke: this.smoke,
      glow: this.glow,
      skids: this.skids,
      groundAt: (x, z) => this.scene.groundAt(x, z),
      surfaceAt: (x, z) => this.surfaces?.at(x, z) ?? SURFACE_GRASS,
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
    const powered = (state.seats[0] ?? 0) !== 0 && state.hp > 0
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
      },
      [position[0], position[1], position[2]],
      [fx.velocity.x, fx.velocity.y, fx.velocity.z],
      this.seat?.vehicle === id,
    )
  }

  /** Smoke from a damaged vehicle: a little under 40% health, a lot when it is a wreck. */
  private smoulder(id: number, state: VehicleState, [x, y, z]: readonly [number, number, number]) {
    if (state.hp >= vehicleMaxHp(state.kind) * 0.4) return
    const now = performance.now()
    if (now < (this.smokeAt.get(id) ?? 0)) return
    this.smokeAt.set(id, now + (state.hp === 0 ? 90 : 220))
    this.effects.smoke(new THREE.Vector3(x, y + 0.8, z), state.hp === 0)
  }

  /**
   * Points the look where the nose gun does. The gun's angles are in the hull frame, the look is in the
   * world's, so the hull's orientation turns one into the other.
   */
  private lookAlong(heli: { root: THREE.Object3D }, gun: Turret) {
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
  private limitGun(mount: { root: THREE.Object3D }, yawLimit: number, pitchMin: number, pitchMax: number) {
    const flat = Math.cos(this.input.pitch)
    const world = new THREE.Vector3(-Math.sin(this.input.yaw) * flat, Math.sin(this.input.pitch), -Math.cos(this.input.yaw) * flat)
    const local = world.applyQuaternion(mount.root.quaternion.clone().invert())
    const yaw = THREE.MathUtils.clamp(Math.atan2(-local.x, -local.z), -yawLimit, yawLimit)
    const pitch = THREE.MathUtils.clamp(Math.asin(THREE.MathUtils.clamp(local.y, -1, 1)), pitchMin, pitchMax)
    const limited = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)).applyQuaternion(mount.root.quaternion)
    this.input.yaw = Math.atan2(-limited.x, -limited.z)
    this.input.pitch = Math.asin(THREE.MathUtils.clamp(limited.y, -1, 1))
  }

  /** A change in the shot counter is a shell leaving: flash and recoil. The shell itself is a projectile. */
  private watchGun(id: number, tank: TankModel | HeliModel, turret: Turret, seats: readonly number[]) {
    const before = this.shots.get(id)
    this.shots.set(id, turret.shots)
    // The first sight of a gun is not a shot, nor is a counter that has not moved.
    if (before === undefined || before === turret.shots) return
    const { origin, direction } = tank.barrel()
    tank.recoil()
    const muzzle = origin.clone().addScaledVector(direction, tank instanceof HeliModel ? HELI_GUN_LENGTH : TANK_BARREL_LENGTH)
    this.effects.muzzleFlash(muzzle, direction)
    this.sounds.cannon([muzzle.x, muzzle.y, muzzle.z], tank instanceof TankModel)
    if (seats[1] === this.localId) this.lastShotAt = performance.now()
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
  private sights(aviator: boolean) {
    let nose: [number, number] | null = null
    let aim: [number, number] | null = null
    const plane = aviator && this.seat ? this.cars.get(this.seat.vehicle) : undefined
    if (plane instanceof PlaneModel) {
      const { origin, direction } = plane.forwardGun(0)
      const toScreen = (point: THREE.Vector3): [number, number] | null => {
        point.project(this.scene.camera)
        // Kept on the screen: the view may be a way behind the aim.
        return point.z < 1 ? [clamp01((point.x + 1) / 2), clamp01((1 - point.y) / 2)] : null
      }
      nose = toScreen(origin.clone().addScaledVector(direction, 150))
      const { yaw, pitch } = this.input
      const flat = Math.cos(pitch)
      const wanted = new THREE.Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat)
      aim = toScreen(plane.root.position.clone().addScaledVector(wanted, 150))
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
    if (this.seat) return 'E: get out'
    // The server hands over an item before it boards a car, so the hint follows the same order.
    const pickup = this.pickupInReach(position)
    if (pickup) return `E: take ${itemDef(pickup.item)?.name ?? 'item'}`
    let best: { distance: number; driverSeatFree: boolean; kind: number } | null = null
    for (const car of cars.values()) {
      const distance = Math.hypot(car.x - position[0]!, car.y - position[1]!, car.z - position[2]!)
      const hasFreeSeat = car.seats.includes(0)
      if (hasFreeSeat && distance <= ENTER_RADIUS && (!best || distance < best.distance)) best = { distance, driverSeatFree: car.seats[0] === 0, kind: car.kind }
    }
    if (!best) return null
    if (best.kind === KIND_TANK) return best.driverSeatFree ? 'E: drive the tank' : 'E: man the gun'
    if (best.kind === KIND_HELI) return best.driverSeatFree ? 'E: fly' : 'E: man the nose gun'
    if (best.kind === KIND_PLANE) return best.driverSeatFree ? 'E: fly the triplane' : 'E: man the rear gun'
    if (best.kind === KIND_SPORTS) return best.driverSeatFree ? 'E: drive the sports car' : 'E: ride along'
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
    cancelAnimationFrame(this.frameHandle)
    clearTimeout(this.reconnectTimer)
    clearInterval(this.pingTimer)
    this.transport.close()
    this.input.dispose()
    this.driver?.dispose()
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
    this.scene.dispose()
  }
}
