import { encodeInput, encodePing, KIND_STICKMAN, toAxis, toMrad, type InputFrame, type ServerMessage } from './net/codec'
import { Interpolator } from './net/interp'
import { RttEstimator, ServerClock } from './net/clock'
import { Transport, type ConnectionStatus } from './net/transport'
import { INPUT_REDUNDANCY, PROTOCOL_VERSION, STANDING_Y, TICK_MS } from './sim/consts'
import { InputController } from './sim/input'
import { RapierMotor } from './sim/physics'
import { Predictor } from './sim/predict'
import { Scene } from './render/scene'
import { Stickman } from './render/stickman'
import { buildWorld } from './render/world'
import { loadWorld } from './world'

export interface HudStats {
  status: ConnectionStatus
  rtt: number | null
  players: number
  interpDelayMs: number
  correction: number
  pending: number
  locked: boolean
  warning: string | null
}

/** Fixed ticks run per animation frame at most, so a backgrounded tab does not spiral. */
const MAX_TICKS_PER_FRAME = 8
const PING_INTERVAL_MS = 1000
const RECONNECT_MS = 3000

/**
 * The client game loop. Fixed 60 Hz ticks sample input, predict the local player and send inputs;
 * every animation frame renders the predicted local player and interpolated remote players.
 */
export class Game {
  private readonly scene: Scene
  private readonly input: InputController
  private readonly transport: Transport
  private readonly clock = new ServerClock()
  private readonly rtt = new RttEstimator()
  private readonly interp = new Interpolator()
  private readonly avatars = new Map<number, Stickman>()
  private predictor: Predictor | null = null
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
    const { world, hash } = await loadWorld()
    this.worldHash = hash
    this.scene.scene.add(buildWorld(world))
    const motor = await RapierMotor.create(world)
    // Until the server places us, walk around solo from the first spawn point.
    const [x, z] = world.spawns[0] ?? [0, 0]
    this.predictor = new Predictor(motor, { position: [x, STANDING_Y, z], yaw: 0, vy: 0, grounded: true })
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
    this.interp.clear()
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
        // Recreate the local stickman in this player's colour.
        this.avatars.get(0)?.dispose()
        this.avatars.delete(0)
        // Inputs made before joining were never sent; they must not be replayed on server state.
        this.predictor?.clearPending()
        this.recent = []
        break
      case 'snapshot': {
        if (this.localId === null) return
        this.clock.observe(message.tick, now)
        const remote = message.entities.filter((e) => e.id !== this.localId)
        this.interp.push(message.tick, remote)
        const own = message.entities.find((e) => e.id === this.localId)
        if (own && this.predictor) {
          this.predictor.reconcile(message.ack, { position: [own.x, own.y, own.z], yaw: own.yaw, vy: own.vy, grounded: own.grounded })
        }
        break
      }
      case 'pong':
        this.rtt.pong(message.nonce, now)
        break
      case 'leave':
        this.interp.remove(message.id)
        break
    }
  }

  private tick() {
    const sample = this.input.sample()
    const frame: InputFrame = {
      seq: ++this.seq,
      strafe: toAxis(sample.strafe),
      forward: toAxis(sample.forward),
      yaw: toMrad(sample.yaw),
      buttons: sample.buttons,
    }
    this.predictor!.apply(frame)
    if (this.localId === null) return
    this.recent.push(frame)
    if (this.recent.length > INPUT_REDUNDANCY) this.recent.shift()
    this.transport.send(encodeInput(this.recent))
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
    const position = predictor.renderPosition(this.accumulator / TICK_MS, dt)
    const local = predictor.state
    const localSpeed = Math.hypot(local.position[0] - predictor.previous.position[0], local.position[2] - predictor.previous.position[2]) / (TICK_MS / 1000)
    // Face where the camera looks, immediately, rather than the last ticked yaw.
    this.avatar(0).update(position, this.input.yaw, localSpeed, local.grounded, dt)
    this.scene.follow(position, this.input.yaw, this.input.pitch)

    const renderTick = this.clock.tickAt(now) - this.clock.interpDelay
    const seen = new Set<number>([0])
    for (const id of this.interp.ids()) {
      const state = this.interp.sample(id, renderTick)
      if (!state || state.kind !== KIND_STICKMAN) continue
      seen.add(id)
      this.avatar(id).update([state.x, state.y, state.z], state.yaw, state.speed, state.grounded, dt)
    }
    for (const [id, avatar] of this.avatars) {
      if (!seen.has(id)) {
        avatar.dispose()
        this.avatars.delete(id)
      }
    }

    this.scene.render()
    this.onStats({
      status: this.transport.status,
      rtt: this.rtt.rtt,
      players: this.localId === null ? 1 : seen.size,
      interpDelayMs: (this.clock.interpDelay * TICK_MS),
      correction: predictor.lastCorrection,
      pending: predictor.pendingCount,
      locked: this.input.locked,
      warning: this.warning,
    })
  }

  /** The stickman for an entity; id 0 is the local player. */
  private avatar(id: number) {
    let avatar = this.avatars.get(id)
    if (!avatar) {
      avatar = new Stickman(id === 0 ? (this.localId ?? 0) : id)
      this.avatars.set(id, avatar)
      this.scene.scene.add(avatar.root)
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
    this.scene.dispose()
  }
}
