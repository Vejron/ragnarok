import * as THREE from 'three'
import {
  encodeInput, encodePing, KIND_HELI, KIND_SPORTS, KIND_STICKMAN, KIND_TANK, sceneStates, toAxis, toMrad, WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_RIFLE,
  WEAPON_AUTOCANNON, WEAPON_ROCKET, WEAPON_SHELL, WEAPON_WRECK,
  type EntityState, type InputFrame, type PickupState, type RocketState, type ServerMessage, type ShotEvent, type Turret, type VehicleState,
} from './net/codec'
import { blendEntity, blendRocket, blendVehicle, Interpolator } from './net/interp'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport, type ConnectionStatus } from './net/transport'
import {
  BUTTON_CROUCH, BUTTON_FIRE, BUTTON_RELOAD, CROUCH_EYE_DROP, SLIDE_MIN_SPEED, ENTER_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, INPUT_REDUNDANCY, PLAYER_MAX_HP, PROTOCOL_VERSION, RESPAWN_TICKS,
  ROCKET_BLAST_RADIUS, SHELL_COOLDOWN_TICKS, STANDING_Y,
  CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS,
  HELI_GUN_COOLDOWN_TICKS, HELI_GUN_LENGTH, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT, CANNON_RADIUS,
  TANK_BARREL_LENGTH, TANK_BARREL_MAX, TANK_BARREL_MIN, TICK_MS, vehicleMaxHp, WRECK_BLAST_RADIUS,
} from './sim/consts'
import { emptyHotbar, GRENADE_BLAST_RADIUS, HOTBAR_SLOTS, ITEM_AK5, ITEM_NONE, ITEM_REVOLVER, itemDef, PICKUP_RADIUS, type ItemSlot } from './sim/items'
import { InputController } from './sim/input'
import { RapierMotor } from './sim/physics'
import { groundOf, Terrain } from './sim/terrain'
import { VehicleDriver, type DrivenPose } from './sim/vehicle'
import { Predictor } from './sim/predict'
import type { Vec3 } from './sim/physics'
import { CarModel, loadCarModels, type VehicleModel } from './render/car'
import { Effects } from './render/effects'
import { Pickups } from './render/pickups'
import { TrajectoryView } from './render/trajectory'
import { throwTrajectory } from './sim/grenade'
import { Rockets } from './render/rockets'
import { saveLook, type Look } from './render/look'
import { halfExtentsOf } from './sim/kinds'
import { DRIVING_DISTANCE, HELI_DISTANCE, ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, Scene, TANK_DISTANCE } from './render/scene'
import { HeliModel, loadHeliModel } from './render/heli'
import { loadTankModel, TankModel } from './render/tank'
import { Character, Crowd } from './render/character'
import { buildGround, buildTile, disposeTile, restitchTerrain } from './render/world'
import { Replica } from './net/replica'
import { TileStreamer } from './stream'
import { loadManifest, type TileData, type TileRef, type WorldData } from './world'

export interface HudStats {
  status: ConnectionStatus
  rtt: number | null
  players: number
  interpDelayMs: number
  /** Map tiles loaded around the player. */
  tiles: number
  correction: number
  pending: number
  locked: boolean
  /** The player is on a touch screen. */
  touch: boolean
  warning: string | null
  /** A hint for the key that does something right now, or null. */
  prompt: string | null
  /** Set while in a vehicle: the seat's role. */
  seat: 'driver' | 'pilot' | 'passenger' | 'gunner' | null
  /** Seated with a free seat to move to: R switches to it. */
  canSwitchSeat: boolean
  /** For a gunner: how loaded the gun is, 0 (just fired) to 1 (ready). */
  reload: number
  hp: number
  maxHp: number
  /** Dead and waiting to respawn. */
  dead: boolean
  /** Seconds until respawn while dead. */
  respawnIn: number
  /** On foot with a gun in hand: show the crosshair. */
  armed: boolean
  /** Connected, standing and alive: the health bar and the hotbar are shown. */
  onFoot: boolean
  /** The five hotbar slots. */
  hotbar: ItemSlot[]
  /** The slot in hand. */
  selected: number
  /** How far a reload or a bandage has got, 0..1, or 0 when neither is going on. */
  progress: number
  kills: number
  deaths: number
  /** Recent kills, oldest first. */
  feed: FeedEntry[]
  /** Every player's tally, best first; empty unless the scoreboard key is held. */
  board: ScoreRow[]
}

export interface FeedEntry {
  /** 0 when the world did it. */
  killer: number
  victim: number
  weapon: number
  /** Whether the local player is the killer or the victim. */
  you: 'killer' | 'victim' | null
}

export interface ScoreRow {
  id: number
  kills: number
  deaths: number
  you: boolean
}

interface CarPlacement {
  kind: number
  turret: Turret | null
  x: number
  y: number
  z: number
  /** Heading of the nose, stickman convention. */
  yaw: number
  seats: readonly number[]
}

/** Players sitting in cars, which the stickman list does not include. */
/** How far behind a vehicle the camera sits. */
const cameraDistance = (kind: number) => (kind === KIND_TANK ? TANK_DISTANCE : kind === KIND_HELI ? HELI_DISTANCE : DRIVING_DISTANCE)

const riders = (cars: ReadonlyMap<number, CarPlacement>) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
const RECONNECT_MS = 3000
/** How long a kill stays in the feed. */
const FEED_MS = 6000
const FEED_LENGTH = 5

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
  /** Rebuilds snapshots from deltas, and says which one to diff the next against. */
  private readonly replica = new Replica()
  private tiles: TileStreamer | null = null
  private world: WorldData | null = null
  private terrain: Terrain | null = null
  /** Each streamed-in tile's drawing, by `row * 65536 + col`, so a new tile can re-shade its neighbours' seams. */
  private readonly tileGroups = new Map<number, THREE.Group>()
  private readonly cars = new Map<number, VehicleModel>()
  private readonly effects = new Effects()
  private readonly rocketInterp = new Interpolator<RocketState>(blendRocket)
  private readonly rocketModels = new Rockets()
  /** The tick a rocket was last listed in, once it stops being listed: it bursts then. */
  private readonly rocketEnds = new Map<number, number>()
  /** Shot and burst events wait here until the render tick reaches them, so they line up with what is drawn. */
  private pendingShots: { tick: number; shot: ShotEvent }[] = []
  private readonly smokeAt = new Map<number, number>()
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
  /** The last shot counter seen for each gun, so a change is a new shell. */
  private readonly shots = new Map<number, number>()
  private lastShotAt = -Infinity
  /** The local player's health, from the newest snapshot. Zero is dead. */
  private hp = PLAYER_MAX_HP
  /** `performance.now()` when the local player died, or null while alive. */
  private diedAt: number | null = null
  private lastFireAt = -Infinity
  /** Kills and deaths of everyone, kept up to date by the server's kill events. */
  private readonly scores = new Map<number, { kills: number; deaths: number }>()
  private feed: { killer: number; victim: number; weapon: number; at: number }[] = []
  /** The vehicle seat the server says the local player sits in; null when on foot. */
  private seat: { vehicle: number; kind: number; index: number } | null = null
  private wasGunner = false
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

  constructor(container: HTMLElement, private readonly onStats: (stats: HudStats) => void) {
    this.scene = new Scene(container)
    this.input = new InputController(this.scene.canvas)
    this.scene.scene.add(this.crowd.root)
    this.scene.scene.add(this.effects.root)
    this.scene.scene.add(this.rocketModels.root)
    this.scene.scene.add(this.pickupModels.root)
    this.scene.scene.add(this.trajectory.root)
    // Translucent smoke and flashes, and the grenade's arc, would show as solid shapes in the AO pass.
    this.scene.excludeFromAO(this.effects.root)
    this.scene.excludeFromAO(this.trajectory.root)
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
    // On a hilly world the plane sits just under the terrain, so it only shows where a tile is late.
    this.scene.scene.add(buildGround(world.halfExtent, world.groundCell > 0 ? -0.05 : 0))
    this.terrain = new Terrain(world.halfExtent, world.tileSize)
    this.scene.groundAt = (x, z) => this.terrain?.heightAt(x, z) ?? 0
    const motor = await RapierMotor.create(world)
    this.motor = motor
    this.driver = await VehicleDriver.create(world)
    await Promise.all([this.crowd.load(), loadCarModels(), loadTankModel(), loadHeliModel()])
    // The buildings arrive as tiles, around wherever the player is.
    this.tiles = new TileStreamer(world, { attach: (ref, tile) => this.attachTile(ref, tile) })
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    await this.tiles.prime(x, z)
    this.predictor = new Predictor(motor, { position: [x, this.terrain.heightAt(x, z) + STANDING_Y, z], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0 })
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

  /** Draws a streamed-in tile and makes it solid for both predicted simulations. Returns how to undo it. */
  private attachTile(ref: TileRef, tile: TileData) {
    const ground = this.world ? groundOf(this.world, ref, tile) : null
    const group = buildTile(tile, ref.col * 31 + ref.row * 17, ground, this.terrain ?? undefined)
    this.scene.scene.add(group)
    this.terrain?.set(ref, ground)
    const key = ref.row * 65536 + ref.col
    this.tileGroups.set(key, group)
    // Tiles already in place were shaded without this one beside them: close the seams.
    if (this.terrain && ground) {
      for (const [dc, dr] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]] as const) {
        const next = this.tileGroups.get((ref.row + dr) * 65536 + ref.col + dc)
        if (next) restitchTerrain(next, this.terrain)
      }
    }
    const walk = this.motor?.addTile(tile, ground) ?? []
    const drive = this.driver?.addTile(tile, ground) ?? []
    return () => {
      disposeTile(group)
      this.tileGroups.delete(key)
      this.terrain?.delete(ref)
      this.motor?.removeTile(walk)
      this.driver?.removeTile(drive)
    }
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
    this.seat = null
    this.scores.clear()
    this.feed = []
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
        this.seat = mount ? { vehicle: mount.id, kind: mount.kind, index: mount.seats.indexOf(this.localId!) } : null
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
        this.scores.delete(message.id)
        break
      case 'scores':
        this.scores.clear()
        for (const { id, kills, deaths } of message.scores) this.scores.set(id, { kills, deaths })
        break
      case 'kill': {
        if (message.killer !== 0) this.score(message.killer).kills = message.killerKills
        this.score(message.victim).deaths = message.victimDeaths
        this.feed.push({ killer: message.killer, victim: message.victim, weapon: message.weapon, at: now })
        if (this.feed.length > FEED_LENGTH) this.feed.shift()
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
    this.lastFrame = now

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
    me.hidden = seated
    me.dead = this.hp === 0
    me.held = this.hotbar[this.input.selectedSlot]?.item ?? ITEM_NONE
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    me.update(position, this.input.yaw, localVx, localVz, local.grounded, dt, this.input.pitch, this.crouching,
      this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED)
    const ride = seated ? cars.get(this.seat!.vehicle) : undefined
    const gunner = ride !== undefined && (ride.kind === KIND_TANK || ride.kind === KIND_HELI) && this.seat!.index === 1
    const turret = gunner ? this.cars.get(this.seat!.vehicle) : undefined
    if (gunner && (turret instanceof TankModel || turret instanceof HeliModel)) {
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
      else this.limitNoseGun(turret)
      const { origin } = turret.barrel()
      this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch)
    } else if (ride) {
      this.input.followYaw(ride.yaw, dt)
      this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, cameraDistance(ride.kind))
    } else {
      this.scene.follow(position, this.input.yaw, this.input.pitch, ON_FOOT_DISTANCE, ON_FOOT_SHOULDER, ON_FOOT_HEIGHT)
    }

    this.wasGunner = gunner
    // Streaming follows what the camera is on: the car when seated, else the stickman.
    this.tiles?.update(ride ? ride.x : position[0], ride ? ride.z : position[2])
    this.playShots(renderTick)
    this.drawRockets(renderTick)
    this.pickupModels.set(this.pickups)
    this.pickupModels.update(now / 1000)
    this.aimGrenade(position, seated)
    this.effects.update(dt)

    const blobs = this.scene.contact
    blobs.begin()
    if (!seated && this.hp > 0) this.blobFoot(blobs, position[0], position[1], position[2])
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = car.y - hy - this.scene.groundAt(car.x, car.z)
      blobs.add(car.x, car.z, hx * 1.2, hz * 1.1, car.yaw, Math.max(0, height))
    }

    const seen = new Set<number>([0])
    for (const id of this.interp.ids()) {
      const state = this.interp.sample(id, renderTick)
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

    blobs.end()
    this.scene.render()
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
      seat: !this.seat ? null : this.seat.index === 0 ? (this.seat.kind === KIND_HELI ? 'pilot' : 'driver') : this.seat.kind === KIND_TANK || this.seat.kind === KIND_HELI ? 'gunner' : 'passenger',
      canSwitchSeat: this.seat !== null && (cars.get(this.seat.vehicle)?.seats.includes(0) ?? false),
      reload: Math.min(1, (now - this.lastShotAt) / ((this.seat?.kind === KIND_HELI ? HELI_GUN_COOLDOWN_TICKS : SHELL_COOLDOWN_TICKS) * TICK_MS)),
      hp: this.hp,
      maxHp: PLAYER_MAX_HP,
      dead: this.hp === 0,
      respawnIn: this.diedAt === null ? 0 : Math.max(0, (RESPAWN_TICKS * TICK_MS - (now - this.diedAt)) / 1000),
      armed: this.localId !== null && !seated && this.hp > 0 && itemDef(this.held().item)?.kind === 'gun',
      onFoot: this.localId !== null && !seated && this.hp > 0,
      hotbar: this.hotbar,
      selected: this.input.selectedSlot,
      progress: this.progress,
      kills: this.localId === null ? 0 : (this.scores.get(this.localId)?.kills ?? 0),
      deaths: this.localId === null ? 0 : (this.scores.get(this.localId)?.deaths ?? 0),
      feed: this.recentFeed(now),
      board: this.input.isHeld('Tab') ? this.scoreboard() : [],
    })
  }

  private score(id: number) {
    let score = this.scores.get(id)
    if (!score) this.scores.set(id, (score = { kills: 0, deaths: 0 }))
    return score
  }

  private setHealth(hp: number, now: number) {
    if (hp === 0 && this.hp > 0) this.diedAt = now
    else if (hp > 0) this.diedAt = null
    this.hp = hp
  }

  private recentFeed(now: number): FeedEntry[] {
    this.feed = this.feed.filter((entry) => now - entry.at < FEED_MS)
    return this.feed.map(({ killer, victim, weapon }) => ({
      killer, victim, weapon,
      you: killer === this.localId ? 'killer' : victim === this.localId ? 'victim' : null,
    }))
  }

  private scoreboard(): ScoreRow[] {
    const ids = new Set<number>(this.scores.keys())
    if (this.localId !== null) ids.add(this.localId)
    for (const id of this.interp.ids()) ids.add(id)
    return [...ids]
      .map((id) => ({ id, kills: this.scores.get(id)?.kills ?? 0, deaths: this.scores.get(id)?.deaths ?? 0, you: id === this.localId }))
      .sort((a, b) => b.kills - a.kills || a.deaths - b.deaths || a.id - b.id)
  }

  /** Where a player's rifle leaves: the end of the barrel it is drawn holding, else its eye. */
  private muzzleOf(shooter: number): THREE.Vector3 | null {
    const avatar = this.avatars.get(shooter === this.localId ? 0 : shooter)
    if (!avatar || !avatar.visible) return null
    return avatar.muzzle() ?? avatar.position.clone().add(new THREE.Vector3(0, EYE_HEIGHT, 0))
  }

  private showShot(shot: ShotEvent) {
    const to = new THREE.Vector3(shot.x, shot.y, shot.z)
    if (shot.weapon === WEAPON_ROCKET) {
      this.effects.burst(to, ROCKET_BLAST_RADIUS)
    } else if (shot.weapon === WEAPON_GRENADE) {
      this.effects.burst(to, GRENADE_BLAST_RADIUS)
    } else if (shot.weapon === WEAPON_SHELL) {
      this.effects.burst(to, EXPLOSION_RADIUS)
    } else if (shot.weapon === WEAPON_WRECK) {
      this.effects.burst(to, WRECK_BLAST_RADIUS)
    } else if (shot.weapon === WEAPON_AUTOCANNON) {
      this.effects.burst(to, CANNON_RADIUS)
    } else if (shot.weapon === WEAPON_RIFLE || shot.weapon === WEAPON_REVOLVER) {
      const from = this.muzzleOf(shot.shooter)
      this.avatars.get(shot.shooter === this.localId ? 0 : shot.shooter)?.fire()
      const range = itemDef(shot.weapon === WEAPON_REVOLVER ? ITEM_REVOLVER : ITEM_AK5)!.range
      if (from) this.effects.tracer(from, to, from.distanceTo(to) < range - 1)
    }
  }

  /** Plays the events whose tick the picture has reached. */
  private playShots(renderTick: number) {
    let played = 0
    while (played < this.pendingShots.length && this.pendingShots[played]!.tick <= renderTick) this.showShot(this.pendingShots[played++]!.shot)
    if (played > 0) this.pendingShots.splice(0, played)
  }

  /** Buffers the rockets of a snapshot, and notes the ones it no longer lists as burst at that tick. */
  private trackRockets(tick: number, rockets: readonly RocketState[]) {
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
    if (!motor || seated || this.hp === 0 || this.localId === null || item?.kind !== 'grenade') {
      this.trajectory.set(null)
      return
    }
    const eye: [number, number, number] = [position[0]!, position[1]! + EYE_HEIGHT - (this.crouching ? CROUCH_EYE_DROP : 0), position[2]!]
    this.trajectory.set(throwTrajectory((origin, dir, max) => motor.castRay(origin, dir, max), eye, this.input.yaw, this.input.pitch))
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
    for (const id of this.vehicleInterp.ids()) {
      const state = this.vehicleInterp.sample(id, renderTick)
      if (!state) continue
      let car = this.cars.get(id)
      if (!car) {
        car = state.kind === KIND_TANK ? new TankModel(id) : state.kind === KIND_HELI ? new HeliModel(id) : new CarModel(state.kind, id)
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
      car.update([x, y, z], [rotation[0], rotation[1], rotation[2], rotation[3]], steer, state.seats, state.turret)
      placed.set(id, { kind: state.kind, turret: state.turret, x, y, z, yaw, seats: state.seats })
      this.smoulder(id, state, [x, y, z])
      if ((car instanceof TankModel || car instanceof HeliModel) && state.turret) this.watchGun(id, car, state.turret, state.seats)
    }
    for (const [id, car] of this.cars) {
      if (!placed.has(id)) {
        car.dispose()
        this.cars.delete(id)
        this.shots.delete(id)
        this.smokeAt.delete(id)
      }
    }
    return placed
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
  private lookAlong(heli: HeliModel, gun: Turret) {
    const flat = Math.cos(gun.pitch)
    const local = new THREE.Vector3(-Math.sin(gun.yaw) * flat, Math.sin(gun.pitch), -Math.cos(gun.yaw) * flat)
    const world = local.applyQuaternion(heli.root.quaternion)
    this.input.yaw = Math.atan2(-world.x, -world.z)
    this.input.pitch = Math.asin(THREE.MathUtils.clamp(world.y, -1, 1))
  }

  /**
   * Keeps the look within the nose gun's travel, which is in the hull's frame: the same limits the
   * server applies, so the view never points somewhere the gun cannot follow.
   */
  private limitNoseGun(heli: HeliModel) {
    const flat = Math.cos(this.input.pitch)
    const world = new THREE.Vector3(-Math.sin(this.input.yaw) * flat, Math.sin(this.input.pitch), -Math.cos(this.input.yaw) * flat)
    const local = world.applyQuaternion(heli.root.quaternion.clone().invert())
    const yaw = THREE.MathUtils.clamp(Math.atan2(-local.x, -local.z), -HELI_GUN_YAW_LIMIT, HELI_GUN_YAW_LIMIT)
    const pitch = THREE.MathUtils.clamp(Math.asin(THREE.MathUtils.clamp(local.y, -1, 1)), HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX)
    const limited = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)).applyQuaternion(heli.root.quaternion)
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
    if (seats[1] === this.localId) this.lastShotAt = performance.now()
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
    if (best.kind === KIND_SPORTS) return best.driverSeatFree ? 'E: drive the sports car' : 'E: ride along'
    return best.driverSeatFree ? 'E: drive the jeep' : 'E: ride along'
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
    this.rocketModels.dispose()
    this.pickupModels.dispose()
    this.trajectory.dispose()
    this.scene.dispose()
  }
}
