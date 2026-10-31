import { Quaternion, Vector3 } from '@ragnarok/engine'
import {
  FLAG_GROUNDED, NOTICE_SAY, encodeInput, encodePing, toAxis, toMrad, type InputFrame, type ServerMessage } from './net/codec'
import type { DriveState, Entity } from './entities/units'
import { Backoff } from './net/backoff'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport } from './net/transport'
import {
  BUTTON_CROUCH, BUTTON_FIRE, BUTTON_RELOAD, CROUCH_EYE_DROP, PRONE_EYE_HEIGHT, SLIDE_MIN_SPEED, EYE_HEIGHT, INPUT_REDUNDANCY, PLAYER_MAX_HP, PROTOCOL_VERSION,
  SHELL_COOLDOWN_TICKS, STANDING_Y, STANCE_CHUTE, STANCE_DOWN, STANCE_PRONE,
  TICK_MS, TICK_RATE, STAMINA_FULL, ONE_ARM_SWAY, BACKPACK_SPEED } from './sim/consts'
import { easeSway, sway, swayAmplitude } from './sway'
import { mayPlace } from './sim/bricks'
import { crewGun, vehicleDef } from './sim/vehicles'
import { aimSpeedScale, HOTBAR_SLOTS, itemDef, type ItemSlot } from './sim/items'
import { InputController } from './sim/input'
import { SimMotor, type CharacterState, type VehiclePose } from './sim/motor'
import { Terrain } from './sim/terrain'
import { VehicleDriver, type DrivenPose } from './sim/driver'
import { Predictor } from './sim/predict'
import type { Vec3 } from './sim/motor'
import { Effects } from './render/effects'
import { dayPhase } from './render/daynight'
import { DamageNumbers } from './render/damagenumbers'
import { Particles } from './render/particles'
import { Skidmarks } from './render/skidmarks'
import { Bloodstains } from './render/bloodstains'
import { Scorchmarks } from './render/scorchmarks'
import { type FxWorld } from './render/tyres'
import { Debris } from './render/debris'
import { Gore } from './render/gore'
import { Ragdolls } from './sim/ragdoll'
import { Surfaces, SURFACE_GRASS } from './sim/surface'
import { Waters } from './sim/waters'
import { Pickups } from './render/pickups'
import { TrajectoryView } from './render/trajectory'
import { MortarArc } from './render/mortar-arc'
import { ProjectileTrails } from './render/trails'
import { Rockets } from './render/rockets'
import { Rounds } from './render/rounds'
import { HullMarks } from './render/hullmarks'
import { Callouts } from './callouts'
import { saveLook, type Look } from './render/look'
import { Grass, GRASS_FULL, GRASS_LITE } from './render/grass'
import { AIM_DISTANCE_SCALE, ON_FOOT_DISTANCE, ON_FOOT_HEIGHT, ON_FOOT_SHOULDER, Scene } from './render/scene'
import { ChaseCam } from './render/chase'
import { PerfReadout } from './render/perf'
import { encodeRtc, KIND_STRV, KIND_TANK, LIMB_ALL, LIMB_ARMS, LIMB_LEGS, TURRET_ALT } from './net/codec'
import { TankMusic } from './music'
import { VoiceChat } from './voice'
import { PlaneModel } from './render/plane'
import { loadVehicleModels } from './kinds'
import { Crowd } from './render/character'
import { Events } from './events'
import { Arcs } from './arcs'
import { cockpit, pilotSights, roofGun } from './gauges'
import { prompt } from './prompt'
import { Surroundings } from './surroundings'
import { Cameras } from './cameras'
import { rideView, seatView, type SeatView } from './seats'
import { loadRoosterModel } from './render/rooster'
import { VehicleSounds } from './vehicleaudio'
import { HIT_DEALT, HIT_KILL, HIT_TAKEN, HitFeedback } from './feedback'
import { buildGround } from './render/world'
import { Replica } from './net/replica'
import { EntityStore, inGameUnits } from './entities/store'
import { Presenters, type Frame } from './entities/presenter'
import { Avatars } from './entities/avatars'
import { Projectiles } from './entities/presenters/projectiles'
import { Pickups as LyingItems } from './entities/presenters/pickups'
import { Backpacks } from './entities/presenters/backpacks'
import { LyingBackpacks } from './render/backpack'
import { Soldiers } from './entities/presenters/people'
import { Riders, seatFrame } from './entities/presenters/riders'
import { Seating } from './entities/presenters/seating'
import { poseOf, Vehicles, type Placed, type VehicleWorld } from './entities/presenters/vehicles'
import { PlaneGuns, RoofGuns, Turrets } from './entities/presenters/guns'
import { Wear } from './entities/presenters/wear'
import { Smoke } from './entities/presenters/smoke'
import { Slits } from './entities/presenters/slits'
import { Jets } from './entities/presenters/jets'
import { Headlights } from './entities/presenters/headlights'
import { Roosters } from './entities/presenters/roosters'
import { TileStreamer } from './stream'
import { loadManifest, NO_WATER, type WorldData } from './world'
import { WorldView } from './worldview'
import type { HudStats } from './hud'
import { Notices } from './notices'
import { Scoreboard, sides } from './scoreboard'
import { Blips } from './blips'
import { BackpackTracker } from './backpack'
import { Bodies } from './bodies'
import { Bricklayer } from './bricklayer'
import { LocalPlayer } from './localplayer'
import { allies, colorFor, colorIndexFor, cssColor, forgetAllPlayers, forgetPlayer, isBot, loadProfile, nameFor, profileQuery, setPlayer, teamFor } from './profile'

// The overlay and its tests import these from here.
export type { HudStats } from './hud'
export type { FeedEntry, ScoreRow, Side } from './scoreboard'
export type { Blip } from './blips'

/** How the minimap tells sides apart. */
const MAP_SIDES = { colorOf: (id: number) => cssColor(colorFor(id)), isBot, allies }

/** Players sitting in vehicles who are not already counted in `seen` (drawn sitting), nor `local`. */
const riders = (cars: ReadonlyMap<number, Placed>, seen: ReadonlySet<number>, local: number | null) => {
  let count = 0
  for (const car of cars.values()) for (const rider of car.seats) if (rider !== 0 && rider !== local && !seen.has(rider)) count++
  return count
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
/** Mouse-look is this much slower down a scope, to match the narrower view. */
const SCOPE_LOOK_SCALE = 0.3
/** Looking down a tank's sight is as slow as a rifle scope. */
const SIGHT_LOOK_SCALE = 0.35
/** And a little slower aiming a gun over the shoulder. */
const AIM_LOOK_SCALE = 0.6
/** How long the static shows after a drone's feed cuts out, ms. */
const FEED_LOST_MS = 700

/** What the local player is up to this frame: worked out once, read by the stages of the frame. */
interface Moment {
  now: number
  dt: number
  seat: { vehicle: number; kind: number; index: number } | null
  view: SeatView | null
  ride: Placed | undefined
  /** Where the local soldier is drawn. */
  position: Vec3
  local: CharacterState
  /** Across the ground, m/s. */
  velocity: [number, number]
  seated: boolean
  /** Flying a drone: the view is its camera. */
  flying: boolean
  /** Their hands are free: on foot, or a car's passenger. */
  hands: boolean
  /** In the game and alive. */
  able: boolean
  /** Able, with their hands free. */
  handy: boolean
  /** Shown in their seat, at `seatAt` and `seatTurn`. */
  sitting: boolean
  scoped: boolean
  aiming: boolean
  sighted: boolean
  item: ReturnType<typeof itemDef>
  /** The look shown and sent, the sway in it. */
  lookYaw: number
  lookPitch: number
  /** How far a chase camera stands behind what they ride. */
  distance: number
}

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
  /** Every soldier is drawn by this one set of instanced meshes. */
  private readonly crowd = new Crowd()
  /** Every player's soldier, whoever draws it. */
  private readonly avatars = new Avatars(this.crowd)
  /** Rebuilds snapshots from deltas, and says which one to diff the next against. */
  private readonly replica = new Replica()
  /** Splashes, wading, grass and contact shadows, for everyone alike. */
  private readonly surroundings: Surroundings
  /** Shots, blasts and bullets, played when the picture reaches them. */
  private readonly events: Events
  /** Every replicated entity in the game's units, sampled at the render tick for the presenters. */
  private readonly store = new EntityStore()
  /** What is drawn in flight and on the ground, before the lights are updated: a flare holds one. */
  private readonly flying = new Presenters<Frame>()
  /** Everyone else on foot, and the creatures, where the contact shadows are gathered. */
  private readonly people = new Presenters<Frame>()
  private readonly soldiers: Soldiers
  private readonly riders: Riders
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
  private waters: Waters | null = null
  /** True from the welcome until the joiner's catch-up `destroyed` arrives: that one is state, not news. */
  private awaitingCatchUp = false
  private readonly damageNumbers = new DamageNumbers()
  /** Tyre smoke, dust, gravel and spray; sparks and flames glow on their own pool. */
  private readonly smoke = new Particles(4096, false, true)
  private readonly glow = new Particles(2048, true)
  /** Blood has a pool of its own, so a firefight's spray never leaves smoke and dust without room. */
  private readonly blood = new Particles(1536)
  private readonly effects = new Effects({ smoke: this.smoke, glow: this.glow, blood: this.blood })
  private readonly skids = new Skidmarks()
  /** Engines, tyres, crashes and guns. */
  private readonly sounds = new VehicleSounds()
  private readonly music = new TankMusic()
  /** Bumpers, doors and wheels that came off. */
  private readonly debris = new Debris(160)
  private readonly stains = new Bloodstains()
  /** Burns on the ground where a laser has been. */
  private readonly scorches = new Scorchmarks((x, z) => this.scene.groundAt(x, z))
  private readonly gore = new Gore(this.debris, this.blood, this.effects, this.stains, (x, z) => this.scene.groundAt(x, z))
  /** Seconds since the last frame, for what is drawn per vehicle. */
  private frameDt = 0
  private readonly rocketModels = new Rockets()
  /** Ribbons behind rockets, shells and bullets in flight. */
  private readonly trails = new ProjectileTrails()
  /** A plane's machine gun rounds in flight, drawn as bullets. */
  private readonly rounds = new Rounds(this.rocketModels, this.trails, (at) => this.effects.pelletStrike(at))
  /** Holes and scuffs where rounds struck armoured vehicles. */
  private readonly hullMarks = new HullMarks()
  /** What the armour made of the player's rounds, and of the rounds that struck their vehicle. */
  private readonly callouts = new Callouts()
  /** The backpack's circle and carrier (BACKPACK.md). */
  private readonly backpack = new BackpackTracker()
  private readonly chase = new ChaseCam()
  private readonly pickupModels = new Pickups()
  /** Items on the ground, as drawn. */
  private readonly lying = new LyingItems(this.pickupModels)
  /** The backpack lying on the ground, as drawn (BACKPACK.md). */
  private readonly backpackModels = new LyingBackpacks()
  /** The arc a grenade in hand would take, drawn while one is held. */
  private readonly trajectory = new TrajectoryView()
  private readonly mortarArc = new MortarArc()
  private readonly arcs = new Arcs(this.trajectory, this.mortarArc)
  private firedBefore = false
  /** Whether the last sampled input held crouch, for the local avatar. */
  private crouching = false
  /** Looking down a scope this frame. */
  private scoped = false
  /** Aiming a gun over the shoulder this frame, and how far the camera has eased in (0..1). */
  private aiming = false
  private aimBlend = 0
  /** A tired player's gun wandering this frame, `[yaw, pitch]` radians: added to the look shown and sent. */
  private sway: [number, number] = [0, 0]
  private swaySize = 0
  private lastShotAt = -Infinity
  /** Scratch for a seat's head point and its vehicle's turn. */
  private readonly seatAt = new Vector3()
  private readonly seatTurn = new Quaternion()
  private lastFireAt = -Infinity
  /** Kills and deaths of everyone, kept up to date by the server's kill events. */
  private readonly board = new Scoreboard()
  /** Allies and recent gunfire, for the minimap. */
  private readonly blips = new Blips()
  private readonly notices = new Notices()
  /** Hits dealt and taken, for the crosshair marker, the screen flash and the direction arcs. */
  private readonly hits = new HitFeedback()
  /** What the server tells the local player of themself. */
  private readonly player = new LocalPlayer()
  /** Limbs coming off and soldiers thrown down, as drawn. */
  private readonly bodies: Bodies
  /** The local player's bricks, at once. */
  private readonly bricks: Bricklayer
  /** The vehicle seat the server says the local player sits in; null when on foot. */
  private seat: { vehicle: number; kind: number; index: number } | null = null
  /** Where the camera goes while seated, by the seat's rig. */
  private readonly cameras: Cameras
  private predictor: Predictor | null = null
  private motor: SimMotor | null = null
  /** Knocked-down soldiers tumbling as ragdolls (KNOCKDOWN.md), once the world's physics is up. */
  private ragdolls: Ragdolls | null = null
  /** Where the local soldier is drawn this frame. */
  private localAt: readonly number[] | null = null
  /** Predicts the vehicle the local player drives. */
  private driver: VehicleDriver | null = null
  /** The newest snapshot of the driven vehicle, waiting for the next frame to be reconciled with. */
  private driverSnapshot: { ack: number; vehicle: VehiclePose; drive: DriveState } | null = null
  /** Vehicle poses of the newest snapshot, which the predicted soldier's frame is fixed to while riding. */
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
    this.bodies = new Bodies(this.avatars, this.gore, this.callouts, this.chase)
    this.perf = new PerfReadout(() => this.scene.renderer)
    this.input = new InputController(this.scene.canvas)
    this.cameras = new Cameras(this.scene, this.input, (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null)
    this.view = new WorldView(this.scene.scene)
    this.bricks = new Bricklayer(this.view, () => this.motor, () => this.eyeHeight())
    this.view.scorch = this.scene.look.scorch
    this.scorches.root.visible = this.scene.look.scorch
    this.view.onTreeNoise = ({ phase, at, size }) => {
      const where: [number, number, number] = [at.x, at.y, at.z]
      if (phase === 'hit') this.sounds.treeHit(where, size)
      else if (phase === 'crack') this.sounds.treeCrack(where, size)
      else {
        this.sounds.treeThud(where, size)
        if (this.scene.camera.position.distanceTo(new Vector3(at.x, at.y, at.z)) < 12) this.chase.kick(0.15)
      }
    }
    this.scene.scene.add(this.crowd.root)
    this.scene.scene.add(this.effects.root, this.view.damage.root, this.view.cubes.root, this.damageNumbers.root)
    this.scene.scene.add(this.rocketModels.root, this.trails.root)
    this.scene.scene.add(this.pickupModels.root, this.backpackModels.root)
    this.scene.scene.add(this.trajectory.root, this.mortarArc.root)
    this.scene.scene.add(this.smoke.root, this.glow.root, this.blood.root, this.stains.root, this.scorches.root, this.skids.root)
    this.smoke.groundAt = (x, z) => this.scene.groundAt(x, z)
    this.glow.groundAt = (x, z) => this.scene.groundAt(x, z)
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
      root: this.scene.scene, sounds: this.sounds, effects: this.effects, rounds: this.rounds, debris: this.debris, lights: this.scene.scene.lights, fx: () => this.fxWorld,
      castRay: (origin, direction, max) => this.motor?.castRay(origin, direction, max) ?? null,
      driven: () => (this.driven && this.driver?.driving ? { id: this.driver.driving, pose: this.driven, jets: this.driver.thrust?.flags ?? 0 } : null),
      seated: () => this.seat?.vehicle ?? null,
      listen: () => this.listen(),
      ownShot: () => (this.lastShotAt = performance.now()),
    }
    this.surroundings = new Surroundings(this.scene, this.effects, () => this.fxWorld)
    this.vehicles = this.machines.add(new Vehicles(world, this.machines.add(new Seating())))
    this.wear = this.machines.add(new Wear(world, this.vehicles))
    this.events = new Events({
      scene: this.scene, effects: this.effects, scorches: this.scorches, sounds: this.sounds, debris: this.debris, damageNumbers: this.damageNumbers, rockets: this.rocketModels, trails: this.trails, rounds: this.rounds,
      hullMarks: this.hullMarks,
      blips: this.blips, avatars: this.avatars, vehicles: this.vehicles, wear: this.wear, view: this.view, chase: this.chase, surroundings: this.surroundings,
      fx: () => this.fxWorld, localId: () => this.localId, driving: () => this.driver?.driving ?? null,
    })
    this.machines.add(new Smoke(world, this.vehicles, (id) => this.hullMarks.vents(id)))
    this.machines.add(new Slits(this.vehicles))
    this.machines.add(new Jets(world, this.vehicles))
    this.machines.add(new Headlights(world, this.vehicles))
    this.machines.add(new Turrets(world, this.vehicles))
    this.machines.add(new PlaneGuns(world, this.vehicles))
    this.machines.add(new RoofGuns(world, this.vehicles))
    this.flying.add(new Projectiles({
      models: this.rocketModels, trails: this.trails, glow: this.glow, lights: this.scene.scene.lights, fx: () => this.fxWorld, blips: this.blips,
    }))
    this.flying.add(this.lying)
    this.flying.add(new Backpacks(this.backpackModels))
    const blobFoot = (x: number, y: number, z: number) => this.surroundings.blobFoot(x, y, z)
    this.soldiers = this.people.add(new Soldiers({ avatars: this.avatars, blobFoot, wade: (id, x, y, z, speed, dt) => this.surroundings.wade(id, x, y, z, speed, dt) }))
    this.riders = this.people.add(new Riders(this.avatars, this.vehicles))
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
    this.waters = this.view.waters = this.surroundings.waters = new Waters(world.halfExtent, world.tileSize)
    this.scene.groundAt = (x, z) => this.terrain?.heightAt(x, z) ?? 0
    const terrain = this.terrain
    this.grass = this.view.grass = this.surroundings.grass = new Grass(world.halfExtent, world.tileSize, (x, z) => terrain.sample(x, z))
    this.scene.scene.add(this.grass.root)
    this.applyGrass(this.scene.look)
    if (!world.bytes) throw new Error('the manifest came without its bytes')
    const motor = await SimMotor.create(world.bytes)
    this.motor = this.view.motor = motor
    this.ragdolls = new Ragdolls(motor)
    this.driver = new VehicleDriver(motor)
    await Promise.all([this.crowd.load(), loadVehicleModels(), loadRoosterModel()])
    // The buildings arrive as tiles, around wherever the player is.
    this.tiles = new TileStreamer(world, {
      prepare: (ref, tile) => this.view.prepareTile(ref, tile),
      attach: (ref, tile) => this.view.attachTile(ref, tile),
      discard: (ref) => this.view.discardTile(ref),
    })
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    this.view.focus(x, z)
    await this.tiles.prime(x, z)
    this.predictor = new Predictor(motor.stepper, { position: [x, this.terrain.heightAt(x, z) + STANDING_Y, z], yaw: 0, vy: 0, grounded: true, knock: [0, 0], ride: 0, prone: false, chute: false, spent: 0, winded: false, lost: 0, down: 0 })
    this.predictor.speedScale = (input) => aimSpeedScale(this.player.slot(Math.min(input.slot, HOTBAR_SLOTS - 1)).item, input.buttons) * (this.player.carrying ? BACKPACK_SPEED : 1)
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
    this.player.reset()
    this.bodies.clear()
    this.board.clear()
    this.notices.clear()
    this.backpack.clear()
    forgetAllPlayers()
    this.hits.clear()
    this.driverSnapshot = null
    this.driver?.leave()
    this.replica.reset()
    this.flying.clear()
    this.rounds.clear()
    this.hullMarks.clear()
    this.callouts.clear()
    this.people.clear()
    this.machines.clear()
    this.store.clear()
    this.events.clear()
    this.blips.clear()
    this.refPoses.clear()
    this.motor?.setVehicles([])
    this.clock.reset()
    this.avatars.forgetOthers()
    // The next session's catch-up is everything there is to know, and that server may have started
    // over: what this one said must not be left on the ground.
    if (!this.view.pristine) {
      this.tiles?.reload()
      this.view.reset()
    }
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
        // Recreate the local soldier in this player's colour.
        this.avatars.forgetLocal()
        // Inputs made before joining were never sent; they must not be replayed on server state.
        this.predictor?.clearPending()
        this.driver?.clearPending()
        this.recent = []
        this.player.reset()
        break
      case 'snapshot':
        this.onSnapshot(message, now)
        break
      case 'pong':
        this.rtt.pong(message.nonce, now)
        break
      case 'rtc':
        void this.voice.receive(message.peer, message.kind, message.json)
        break
      case 'leave':
        this.voice.forget(message.id)
        this.store.remove(message.id)
        this.board.forget(message.id)
        this.bodies.forget(message.id)
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
      case 'cratered':
        this.view.cratered(message.craters, message.live)
        break
      case 'dug':
        this.view.dug(message.tiles)
        break
      case 'built':
        this.view.built(message)
        break
      case 'scores':
        this.board.replace(message.scores)
        break
      case 'profiles':
        for (const { id, name, color, team, bot } of message.players) {
          setPlayer(id, name, color, team, bot)
          // Snapshots come by datagram and can overtake this, so a soldier may already be drawn.
          this.avatars.of(id)?.setColor(colorFor(id))
        }
        break
      case 'hit': {
        this.hits.receive(message, now)
        if (message.role === HIT_DEALT) {
          this.bodies.dealt(message.target, now)
          this.sounds.hit(message.kind === HIT_KILL)
          this.events.hit(message)
        }
        else if (message.role === HIT_TAKEN) {
          this.sounds.hurt(message.amount)
          this.chase.kick(Math.min(0.5, message.amount / 80))
        }
        break
      }
      case 'armor':
        this.callouts.receive(message, now)
        break
      case 'marks':
        this.events.receiveMarks(message.tick, message.marks)
        break
      case 'notice':
        this.notices.receive(message.kind, message.text, now)
        break
      case 'backpack': {
        if (message.carrier !== 0) this.board.held(message.carrier, message.held)
        const news = this.backpack.receive(message, this.localId, nameFor)
        if (news) this.notices.receive(NOTICE_SAY, news, now)
        break
      }
      case 'kill': {
        const body = this.avatars.of(message.victim)
        if (body?.visible) this.gore.die(body, message.style, message.yaw, message.power, (x, z) => this.scene.groundAt(x, z))
        this.board.kill(message, now)
        break
      }
    }
  }

  private onSnapshot(message: Extract<ServerMessage, { type: 'snapshot' }>, now: number) {
    if (this.localId === null) return
    // Snapshots list what changed since a baseline. One that names a baseline we do not hold
    // is dropped; the server has no acknowledgement for it and soon sends a whole one.
    const rebuilt = this.replica.receive(message.tick, message.baseline, message.updates, message.removed)
    if (!rebuilt) return
    this.store.push(message.tick, rebuilt.scene, rebuilt.latest)
    this.clock.observe(message.tick, now)
    // What only we are told comes as our own entity's components, sent to us wherever we are.
    const listed = rebuilt.scene.get(this.localId)
    const own = listed && inGameUnits(listed)
    if (own) this.player.receive(own, now)
    this.events.receive(message.tick, message.shots, message.bullets)
    // Vehicles are solid to the predicted soldier, at the pose of the newest snapshot.
    this.refPoses.clear()
    let drive: DriveState | undefined
    for (const [id, wire] of rebuilt.scene) {
      if (!wire.orientation || !wire.place) continue
      const vehicle = inGameUnits(wire)
      this.refPoses.set(id, poseOf(id, vehicle))
      if (id === own?.seated?.vehicle) drive = vehicle.drive
    }
    this.motor?.setVehicles([...this.refPoses.values()].map((pose) => {
      const [vx, vz] = this.store.latestVelocity(pose.id) ?? [0, 0]
      return { ...pose, vx, vz }
    }))
    // Where the local player sits, if they do: their own `seated`, once their vehicle is in view.
    const mount = (own?.seated && this.refPoses.get(own.seated.vehicle)) || null
    const wasSeated = this.seat !== null
    this.seat = mount ? { vehicle: mount.id, kind: mount.kind, index: own?.seated?.seat ?? -1 } : null
    // A door closing, for getting in or out (not for hopping seats).
    if (wasSeated !== (this.seat !== null) && this.player.alive) this.sounds.door()
    if (mount && this.seat?.index === 0 && !vehicleDef(mount.kind).towed && drive) {
      // Replaying the unacknowledged inputs steps the whole physics world once each, so it waits
      // for the next frame: a backlog of snapshots (a busy main thread) is then one replay, not
      // one per snapshot, which would only make the thread busier.
      this.driver?.acknowledge(message.ack)
      this.driverSnapshot = { ack: message.ack, vehicle: mount, drive }
    } else {
      this.driverSnapshot = null
      this.driver?.leave()
      this.driver?.acknowledge(message.ack)
    }
    if (this.seat) {
      // The soldier is parked inside the car. Inputs still go out; the server just applies them to the car.
      this.predictor?.acknowledge(message.ack)
    } else if (own) {
      this.reconcile(own, message.ack, message.ride)
    }
  }

  /** Puts the predicted soldier where their own entity says they are, on foot, as of input `ack`. */
  private reconcile(own: Entity, ack: number, ride: number) {
    if (!own.place || !own.flags || !this.predictor) return
    const { x, y, z } = own.place
    const { flags, stance } = own.flags
    const lost = (own.limbs?.lost ?? 0) & LIMB_ALL
    // Knocked down, a soldier with no legs is still lying as they were.
    const prone = stance === STANCE_PRONE || (stance === STANCE_DOWN && (lost & LIMB_LEGS) === LIMB_LEGS)
    this.predictor.reconcile(ack, {
      position: [x, y, z], yaw: own.heading?.yaw ?? 0, vy: own.motion?.vy ?? 0, grounded: (flags & FLAG_GROUNDED) !== 0,
      knock: [own.motion?.knock_x ?? 0, own.motion?.knock_z ?? 0], ride, prone, chute: stance === STANCE_CHUTE,
      spent: own.vitals?.spent ?? 0, winded: own.vitals?.winded === 1, lost, down: own.vitals?.down ?? 0,
    })
  }

  private tick() {
    // Dead or seated, nobody is lying down: respawning or stepping out starts on their feet.
    if (this.seat || !this.player.alive) this.input.clearProne()
    const sample = this.input.sample()
    this.crouching = (sample.buttons & BUTTON_CROUCH) !== 0
    const frame: InputFrame = {
      seq: ++this.seq,
      strafe: toAxis(sample.strafe),
      forward: toAxis(sample.forward),
      yaw: toMrad(sample.yaw + this.sway[0]),
      buttons: sample.buttons,
      pitch: toMrad(sample.pitch + this.sway[1]),
      view: this.viewTick(),
      slot: sample.slot,
    }
    // In a car the soldier does not walk, and nor does a dead one; the frame is kept for replay
    // in case it steps out or respawns.
    if (this.seat || !this.player.alive) this.predictor!.record(frame)
    else this.predictor!.apply(frame)
    if (this.localId !== null) {
      this.fireFeedback(frame)
      const onFoot = !this.seat && this.player.alive && itemDef(this.held().item)?.kind === 'build'
      this.bricks.feedback(frame, onFoot, this.predictor!.state.position, colorIndexFor(this.localId), this.rtt.rtt)
    }
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
    const eye = this.scene.camera.position
    this.view.update(dt, this.effects, this.cameraAt.set(eye.x, eye.y, eye.z), this.fxWorld)
    const ticks = this.runTicks(dt)
    const renderTick = this.clock.tickAt(now) - this.clock.interpDelay
    this.followClock(renderTick)

    const seat = this.seat
    const view = seat ? seatView(seat.kind, seat.index) : null
    this.music.want(seat !== null && this.player.alive && (seat.kind === KIND_TANK || seat.kind === KIND_STRV))
    this.driven = this.driver?.pose(this.accumulator / TICK_MS, dt) ?? null
    // Everything replicated, sampled once at the render tick; each group of presenters draws its part.
    const sampled = this.store.sampleAll(renderTick)
    const entityFrame: Frame = { tick: renderTick, dt, now, localId: this.localId }
    this.machines.run(sampled, entityFrame)

    const moment = this.momentOf(now, dt, renderTick, seat, view)
    this.poseLocal(moment)
    this.aimCamera(moment)
    // Streaming follows what the camera is on: the car when seated, else the soldier.
    const { ride, position } = moment
    this.tiles?.update(ride ? ride.x : position[0], ride ? ride.z : position[2])
    this.events.play(renderTick)
    this.flying.run(sampled, entityFrame)
    this.events.drawBullets(renderTick)
    this.drawAims(moment)
    this.updateEffects(dt)

    const cars = this.vehicles.placed
    const standing = !moment.seated && this.player.alive
    this.scene.contact.begin()
    if (standing) this.surroundings.blobFoot(position[0], position[1], position[2])
    this.surroundings.blobVehicles(cars)
    this.people.run(sampled, entityFrame)
    this.bodies.limbs(sampled, this.localId, now)
    if (this.ragdolls) {
      const at = this.scene.camera.position
      this.bodies.knockdowns(this.ragdolls, sampled, { id: this.localId, state: this.predictor?.state, at: this.localAt, seated: moment.seated, dead: !this.player.alive }, [at.x, at.y, at.z], dt)
    }
    this.surroundings.bendGrass(standing ? position : null, this.soldiers.nearby, cars)
    const nearby = [...this.soldiers.nearby, ...this.riders.seated]
    this.avatars.sweep((key) => this.surroundings.dry(key))
    this.voice.setTalking(this.localId !== null && this.player.alive && this.input.isHeld('KeyV'))
    this.voice.update(this.localId, position, nearby)
    this.scene.contact.end()
    this.surroundings.mirrorWater()
    this.perf.beforeRender()
    this.scene.render()
    this.perf.end(ticks)
    this.report(moment)
  }

  /** Runs the fixed ticks `dt` has brought due, and says how many. */
  private runTicks(dt: number) {
    this.accumulator += dt * 1000
    let ticks = 0
    while (this.accumulator >= TICK_MS && ticks < MAX_TICKS_PER_FRAME) {
      this.tick()
      this.accumulator -= TICK_MS
      ticks++
    }
    if (ticks === MAX_TICKS_PER_FRAME) this.accumulator = 0
    return ticks
  }

  /** The time of day runs on the server's clock, so every player sees the same sky. */
  private followClock(renderTick: number) {
    if (!this.clock.synced) return
    this.scene.setTimeOfDay(dayPhase(renderTick, TICK_RATE))
    this.smoke.light.copy(this.scene.groundLight)
    this.blood.light.copy(this.scene.groundLight)
  }

  /** Works out what the local player is up to this frame, for the stages of the frame that follow. */
  private momentOf(now: number, dt: number, renderTick: number, seat: Game['seat'], view: SeatView | null): Moment {
    const predictor = this.predictor!
    const local = predictor.state
    const position = this.ridden(predictor.renderPosition(this.accumulator / TICK_MS, dt), local.ride, renderTick)
    this.localAt = position
    const velocity: [number, number] = [(local.position[0] - predictor.previous.position[0]) / (TICK_MS / 1000), (local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)]
    const seated = seat !== null
    // Flying a drone: the body kneels where it launched it, and the view is the drone's camera.
    const flying = view?.rig.name === 'fpv'
    // A car's passenger fires their own guns from their seat, as one on foot does.
    const hands = !seated || (seat !== null && crewGun(seat.kind, seat.index))
    const able = this.localId !== null && this.player.alive
    const handy = able && hands
    // Where the local player sits, when they show in their seat: their head, and the vehicle's turn.
    const mount = seated && !flying ? this.vehicles.model(seat.vehicle) : undefined
    const sitting = mount !== undefined && seatFrame(mount, seat!.kind, seat!.index, this.seatAt, this.seatTurn, this.vehicles.placed.get(seat!.vehicle)?.turret?.yaw)
    const item = itemDef(this.held().item)
    // Aim held with a scoped gun in hand, alive and free to use it: look down the scope. With any other gun: half zoom over the shoulder.
    const scoped = handy && this.input.aimHeld && item?.scope === true
    const aiming = !scoped && handy && this.input.aimHeld && item?.kind === 'gun'
    this.scoped = scoped
    this.aiming = aiming
    this.aimBlend += ((aiming ? 1 : 0) - this.aimBlend) * Math.min(1, dt * 14)
    // Out of breath, a gun in hand on foot wanders. The look sent with each input carries it, so shots go where it is shown.
    const steadying = !seated && able && (item?.kind === 'gun' || item?.kind === 'launcher')
    // Seated it stops at once: a vehicle's aim must not pick up a fading wobble.
    // A gun held in one hand wanders however rested.
    const arms = local.lost & LIMB_ARMS
    const oneArm = arms !== 0 && arms !== LIMB_ARMS ? ONE_ARM_SWAY : 0
    this.swaySize = seated ? 0 : easeSway(this.swaySize, steadying ? Math.max(swayAmplitude(local.spent, local.winded), oneArm) : 0, dt)
    this.sway = sway(now / 1000, this.swaySize)
    const ride = seat ? this.vehicles.placed.get(seat.vehicle) : undefined
    return {
      now, dt, seat, view, ride, position, local, velocity, seated, flying, hands, able, handy, sitting, scoped, aiming, item,
      lookYaw: this.input.yaw + this.sway[0],
      lookPitch: this.input.pitch + this.sway[1],
      distance: ride ? rideView(ride.kind).distance : 0,
      // Aim held at a gun with a sight (a tank's, a Strv S's, a Pak's): look down it, which shows where the barrel really points.
      sighted: this.player.alive && this.input.aimHeld && ride !== undefined && view?.sight === true,
    }
  }

  /** Draws the local soldier. */
  private poseLocal({ dt, seated, flying, sitting, scoped, position, local, velocity, lookYaw, lookPitch }: Moment) {
    const [vx, vz] = velocity
    this.avatars.begin()
    const me = this.avatars.local()
    me.hidden = (seated && !flying && !sitting) || scoped
    me.dead = !this.player.alive
    me.held = this.player.slot(this.input.selectedSlot).item
    me.pack = this.player.carrying
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    if (sitting) me.seat(this.seatAt, this.seatTurn, this.input.yaw, this.input.pitch, dt)
    else me.update(position, flying ? local.yaw : lookYaw, vx, vz, local.grounded, dt, flying ? 0 : lookPitch, this.crouching || flying,
      this.crouching && local.grounded && Math.hypot(local.knock[0], local.knock[1]) > SLIDE_MIN_SPEED, local.prone || local.down > 0, local.chute, local.vy)
    if (!seated && this.player.alive) this.surroundings.wade(0, position[0], position[1], position[2], Math.hypot(vx, vz), dt)
    else this.surroundings.dry(0)
  }

  /** Puts the camera where this frame wants it. */
  private aimCamera({ now, dt, seat, view, ride, position, distance, sighted, scoped, aiming, lookYaw, lookPitch, sitting }: Moment) {
    this.input.lookScale = sighted ? SIGHT_LOOK_SCALE : scoped ? SCOPE_LOOK_SCALE : aiming ? AIM_LOOK_SCALE : 1
    const chase = ride && view?.chase ? this.chase.update([ride.x, ride.y, ride.z], ride.yaw, dt, distance) : null
    if (!chase) this.chase.reset()
    // A kick shakes the on-foot view too; a car's chase camera shakes with its own.
    const shake = chase ? undefined : this.chase.tremble(dt)
    this.scene.zoomTo(this.cameras.zoom(view, this.input.aimHeld, sighted, scoped, aiming), dt, chase?.fovBoost ?? 0)
    if (seat && ride && view) {
      this.cameras.seated({
        now, dt, vehicle: seat.vehicle, seat: view, ride, model: this.vehicles.model(seat.vehicle), motion: this.vehicles.fx(seat.vehicle),
        distance, sighted, scoped, aimBlend: this.aimBlend, chase, head: sitting ? this.seatAt : null,
      }, seat.index)
      return
    }
    this.cameras.leave(now)
    if (scoped) this.scene.scopeFrom([position[0]!, position[1]! + this.eyeHeight(), position[2]!], lookYaw, lookPitch, shake)
    // Only the distance changes: the pivot is what the server's crosshair ray is built from.
    else this.scene.follow(position, lookYaw, lookPitch, ON_FOOT_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * this.aimBlend), ON_FOOT_SHOULDER, ON_FOOT_HEIGHT, shake)
  }

  /** The arcs of what is in hand or in the rack, and the ghost of the cube bricks in hand would place or dig. */
  private drawAims({ seat, view, ride, position, seated, able, item }: Moment) {
    this.arcs.update({
      motor: this.motor, arc: view?.arc ?? null, groundAt: (x, z) => this.scene.groundAt(x, z),
      vehicle: seat && ride ? { model: this.vehicles.model(seat.vehicle), motion: this.vehicles.fx(seat.vehicle), bombs: ride.arms?.bombs ?? 0 } : null,
      grenade: !seated && able && item?.kind === 'grenade' ? { eye: [position[0]!, position[1]! + this.eyeHeight(), position[2]!], yaw: this.input.yaw, pitch: this.input.pitch } : null,
    })
    const brick = !seated && able && item?.kind === 'build' ? this.bricks.aim(position, this.input.yaw, this.input.pitch) : null
    const digging = this.input.aimHeld
    const place = brick && !digging ? brick.place : null
    const world = place ? this.bricks.world(position) : null
    this.view.cubes.aim(place, brick && digging && this.view.construct.has(brick.dig) ? brick.dig : null, place !== null && world !== null && mayPlace(world, place))
  }

  private updateEffects(dt: number) {
    this.effects.update(dt)
    this.rounds.update(dt)
    this.hullMarks.update(dt, this.vehicles.placed)
    this.trails.update(dt)
    this.scene.scene.lights.update(dt)
    this.damageNumbers.update(dt, this.scene.camera)
    const eye = this.scene.camera.position
    this.smoke.follow(eye.x, eye.z)
    this.glow.follow(eye.x, eye.z)
    this.smoke.update(dt)
    this.glow.update(dt)
    this.blood.update(dt)
    this.skids.update(dt)
    this.stains.update(dt)
    this.scorches.update(dt)
    this.debris.update(dt)
    this.gore.update(dt)
  }

  /** Tells the overlay how things stand. */
  private report({ now, seat, view, ride, position, local, seated, hands, able, handy, sighted }: Moment) {
    const { player, predictor } = this
    const cars = this.vehicles.placed
    const aviator = view?.rig.name === 'aviator'
    const plane = aviator && seat ? this.vehicles.model(seat.vehicle) : undefined
    const sights = plane instanceof PlaneModel ? pilotSights(this.scene.camera, plane, this.input.yaw, this.input.pitch) : { nose: null, aim: null }
    this.onSights?.(sights.nose, sights.aim)
    const hitView = this.hits.view(now, this.input.yaw)
    const gauges = view?.gauges
    const link = view?.rig.name === 'fpv' ? view.rig : null
    this.onStats({
      status: this.transport.status,
      rtt: this.rtt.rtt,
      voice: this.voice.status,
      players: this.localId === null ? 1 : this.avatars.drawn.size + riders(cars, this.avatars.drawn, this.localId),
      interpDelayMs: (this.clock.interpDelay * TICK_MS),
      tiles: this.tiles?.loaded ?? 0,
      correction: this.driver?.driving ? this.driver.lastCorrection : predictor!.lastCorrection,
      pending: predictor!.pendingCount,
      locked: this.input.locked,
      touch: this.input.coarse,
      warning: this.warning,
      prompt: this.localId === null ? null : prompt({
        position, ground: this.terrain?.heightAt(position[0]!, position[2]!) ?? 0, alive: player.alive, grounded: local.grounded, chute: local.chute, seat,
      }, this.lying.lying, cars),
      seat: view?.role ?? null,
      canSwitchSeat: seat !== null && (cars.get(seat.vehicle)?.seats.includes(0) ?? false),
      range: gauges?.includes('range') ? (ride?.turret?.range ?? 0) : 0,
      callouts: this.callouts.view(now),
      limbs: player.body(),
      ammo: gauges?.includes('ammo') ? ((ride?.turret?.flags ?? 0) & TURRET_ALT ? 'HE' : 'AP') : null,
      reload: this.reloaded(now, ride ? rideView(ride.kind).reload : SHELL_COOLDOWN_TICKS),
      ...(gauges?.includes('pintle') ? roofGun(ride?.pintle ?? null) : cockpit(gauges?.includes('cockpit') ? (ride?.arms ?? null) : null, aviator, seat ? this.vehicles.fx(seat.vehicle) : undefined)),
      air: gauges?.includes('air') ? (this.driver?.thrust?.air ?? null) : null,
      stamina: !seated && able ? { left: 1 - local.spent / STAMINA_FULL, winded: local.winded } : null,
      eggs: gauges?.includes('eggs') ? (ride?.arms?.bombs ?? null) : null,
      drone: link && ride
        ? {
          battery: this.refPoses.get(seat!.vehicle)?.air ?? 1,
          signal: Math.max(0, 1 - Math.hypot(ride.x - position[0]!, ride.z - position[2]!) / link.link),
          camera: link.belly ? this.cameras.loiterCam : null,
        }
        : null,
      feedLost: Math.max(0, 1 - (now - this.cameras.feedLostAt) / FEED_LOST_MS),
      hp: player.hp,
      maxHp: PLAYER_MAX_HP,
      dead: !player.alive,
      respawnIn: player.respawnIn(now),
      sight: sighted,
      scoped: this.scoped,
      aiming: this.aiming,
      armed: handy && itemDef(this.held().item)?.kind === 'gun',
      hands: this.localId !== null && hands && player.alive,
      hotbar: player.hotbar,
      selected: this.input.selectedSlot,
      progress: player.progress,
      kills: this.board.kills(this.localId),
      deaths: this.board.deaths(this.localId),
      feed: this.board.recent(now, this.localId),
      notices: this.notices.view(now),
      board: this.input.isHeld('Tab') ? sides(this.board.rows(this.localId, this.soldiers.inView), teamFor) : [],
      hitMarker: hitView.marker,
      hurt: hitView.vignette,
      hurtArcs: hitView.arcs,
      // Seated, the on-foot body stays where it got in; the map follows the vehicle.
      x: ride?.x ?? position[0],
      z: ride?.z ?? position[2],
      yaw: this.input.yaw,
      halfExtent: this.world?.halfExtent ?? 0,
      blips: this.blips.view(now, this.localId, ride?.x ?? position[0]!, ride?.z ?? position[2]!, this.alliesInView(), MAP_SIDES),
      backpack: this.backpack.view(this.localId),
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
    for (const id of this.soldiers.inView) {
      const avatar = this.avatars.of(id)
      if (avatar?.visible && allies(this.localId, id)) found.push({ id, x: avatar.position.x, z: avatar.position.z })
    }
    return found
  }

  /** The slot in hand. */
  private held(): ItemSlot {
    return this.player.slot(this.input.selectedSlot)
  }

  /** The local player's own muzzle flash, at once, instead of after the server's reply. */
  private fireFeedback(frame: InputFrame) {
    const now = performance.now()
    const pressed = (frame.buttons & BUTTON_FIRE) !== 0
    const fresh = pressed && !this.firedBefore
    this.firedBefore = pressed
    if (!this.player.alive || (this.seat && !crewGun(this.seat.kind, this.seat.index))) return
    const slot = this.held()
    const item = itemDef(slot.item)
    // A local mirror of the server's rules for a shot, so the flash does not wait for its reply.
    if (!item || item.kind !== 'gun' || !pressed || (!item.auto && !fresh)) return
    if (slot.ammo === 0 || this.player.progress > 0 || (frame.buttons & BUTTON_RELOAD) !== 0) return
    if (now - this.lastFireAt < item.cooldownTicks * TICK_MS) return
    this.lastFireAt = now
    const [x, y, z] = this.predictor!.state.position
    const yaw = frame.yaw / 1000
    const pitch = frame.pitch / 1000
    const flat = Math.cos(pitch)
    const direction = new Vector3(-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat)
    const me = this.avatars.local()
    me.fire()
    const barrel = me.muzzle()
    this.effects.muzzle(barrel ?? new Vector3(x, y + EYE_HEIGHT, z).addScaledVector(direction, 0.6))
  }

  /** Height of the local player's eye above the capsule centre, for the stance they are in. */
  private eyeHeight(): number {
    if (this.predictor?.state.prone) return PRONE_EYE_HEIGHT
    return EYE_HEIGHT - (this.crouching ? CROUCH_EYE_DROP : 0)
  }

  /**
   * While standing on a car, the soldier is predicted as if the car stood still where the newest
   * snapshot left it. This carries that position along to wherever the car is drawn now, so the
   * soldier stays glued to the roof and still moves on it.
   */
  private ridden(position: Vec3, ride: number, renderTick: number): Vec3 {
    const ref = this.refPoses.get(ride)
    const now = ride === 0 ? null : this.store.sample(ride, renderTick)?.entity
    if (!ref || !now?.place || !now.orientation) return position
    const local = new Vector3(position[0] - ref.x, position[1] - ref.y, position[2] - ref.z)
    local.applyQuaternion(new Quaternion(ref.qx, ref.qy, ref.qz, ref.qw).invert())
    const { qx, qy, qz, qw } = now.orientation
    local.applyQuaternion(new Quaternion(qx, qy, qz, qw))
    return [now.place.x + local.x, now.place.y + local.y, now.place.z + local.z]
  }

  /** The soft-lighting options in force. */
  get look() {
    return this.scene.look
  }

  /** Switches the soft-lighting options and remembers them for next time. */
  setLook(look: Look) {
    this.scene.setLook(look)
    this.view.scorch = look.scorch
    this.scorches.root.visible = look.scorch
    this.applyGrass(look)
    saveLook(look)
  }

  private applyGrass(look: Look) {
    const coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
    this.grass?.setDensity(coarse ? GRASS_LITE : GRASS_FULL)
    this.grass?.setEnabled(look.grass)
  }

  /** Puts the ears where the camera is, last frame's pose, and moving with the vehicle the player is in. */
  private listen() {
    const camera = this.scene.camera
    const at = camera.position
    const look = camera.getWorldDirection(this.listenForward)
    const moving = this.seat ? this.vehicles.fx(this.seat.vehicle)?.velocity : undefined
    this.sounds.setListener([at.x, at.y, at.z], [look.x, look.y, look.z], moving ? [moving.x, moving.y, moving.z] : [0, 0, 0])
  }

  private readonly listenForward = new Vector3()
  private readonly cameraAt = new Vector3()

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
    this.blood.dispose()
    this.skids.dispose()
    this.stains.dispose()
    this.scorches.dispose()
    this.sounds.dispose()
    this.music.dispose()
    this.debris.dispose()
    this.view.dispose()
    this.rocketModels.dispose()
    this.pickupModels.dispose()
    this.people.clear()
    this.trails.dispose()
    this.damageNumbers.dispose()
    this.grass?.dispose()
    this.trajectory.dispose()
    this.mortarArc.dispose()
    this.scene.dispose()
  }
}
