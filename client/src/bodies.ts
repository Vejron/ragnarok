import type { Avatars } from './entities/avatars'
import type { Sampled } from './entities/store'
import type { Callouts } from './callouts'
import { FLAG_DEAD, LIMB_ALL, LIMB_LEGS } from './net/codec'
import { STANCE_DOWN } from './sim/consts'
import type { ChaseCam } from './render/chase'
import type { Gore } from './render/gore'
import { bleedingOf, limbName, lostOf } from './sim/limbs'
import type { CharacterState, Vec3 } from './sim/motor'
import type { Knockable, Ragdolls } from './sim/ragdoll'

/** A limb that comes off this soon after the local player hurt its owner is called out as theirs, ms. */
const LIMB_CREDIT_MS = 1500
/** A bleeding stump drips this often, ms. */
const DRIP_MS = 250

/** The local soldier this frame, for what is done to everyone drawn. */
export interface LocalBody {
  id: number | null
  state: CharacterState | undefined
  at: readonly number[] | null
  seated: boolean
  dead: boolean
}

/** What happens to soldiers' bodies, as drawn: limbs coming off, stumps bleeding, and being thrown down. */
export class Bodies {
  /** When the local player last hurt each player, by id: a limb that comes off soon after was their doing. */
  private readonly dealtAt = new Map<number, number>()
  /** When each soldier's stumps last dripped, by avatar key. */
  private readonly drippedAt = new Map<number, number>()
  /** Whether the local soldier was knocked down last frame. */
  private wasDown = false

  constructor(
    private readonly avatars: Avatars,
    private readonly gore: Gore,
    private readonly callouts: Callouts,
    private readonly chase: ChaseCam,
  ) {}

  clear() {
    this.dealtAt.clear()
    this.drippedAt.clear()
    this.wasDown = false
  }

  /** The local player just hurt player `id`. */
  dealt(id: number, now: number) {
    this.dealtAt.set(id, now)
  }

  /** Player `id` left. */
  forget(id: number) {
    this.dealtAt.delete(id)
    this.drippedAt.delete(id)
  }

  /**
   * Tells every soldier drawn this frame which limbs it has lost. One that has just come off is thrown,
   * and called out to the local player if it was theirs or they did it; a stump that still bleeds drips.
   */
  limbs(sampled: ReadonlyMap<number, Sampled>, localId: number | null, now: number) {
    for (const key of this.avatars.drawn) {
      const id = key === 0 ? localId : key
      const avatar = id === null ? undefined : this.avatars.of(id)
      if (id === null || !avatar) continue
      const entity = sampled.get(id)?.entity
      const struck = entity?.struck
      if (avatar.setStruck(struck) && struck && avatar.visible) this.gore.hit(avatar, struck.part, struck.yaw, struck.amount, struck)
      const wire = entity?.limbs?.lost ?? 0
      const gained = avatar.setLimbs(wire)
      if (gained) {
        if (avatar.visible) this.gore.sever(avatar, gained)
        const name = limbName(gained)
        if (key === 0) this.callouts.say({ text: `${name} gone! Bandage it`, tone: 'bad' }, now)
        else if (now - (this.dealtAt.get(id) ?? -Infinity) < LIMB_CREDIT_MS) this.callouts.say({ text: `${name} off!`, tone: 'good' }, now)
      }
      const bleeding = bleedingOf(wire)
      if (bleeding && avatar.visible && now - (this.drippedAt.get(key) ?? -Infinity) > DRIP_MS) {
        this.gore.drip(avatar, bleeding)
        this.drippedAt.set(key, now)
      }
      // A soldier with no legs crawls, and while a stump bleeds it drags a smear behind it.
      const legless = (lostOf(wire) & LIMB_LEGS) === LIMB_LEGS
      if (legless && bleeding && avatar.visible && !avatar.dead) this.gore.drag(`crawl${key}`, avatar)
      else this.gore.lift(`crawl${key}`)
    }
  }

  /**
   * Hands every soldier drawn this frame that is knocked down to a ragdoll, and back when they are up
   * (`Ragdolls`). The local player's own knockdown also jolts the camera, as hard as they were thrown.
   */
  knockdowns(ragdolls: Ragdolls, sampled: ReadonlyMap<number, Sampled>, me: LocalBody, eye: Vec3, dt: number) {
    const soldiers: Knockable[] = []
    for (const key of this.avatars.drawn) {
      const id = key === 0 ? me.id : key
      const body = id === null ? undefined : this.avatars.of(id)
      if (id === null || !body) continue
      if (key === 0) {
        const local = me.state
        if (!local || !me.at) continue
        const down = local.down > 0 && !me.seated
        if (down && !this.wasDown) this.chase.kick(Math.min(0.6, Math.hypot(local.knock[0], local.knock[1]) / 30))
        this.wasDown = down
        const [x, y, z] = me.at
        soldiers.push({ key, body, down, dead: me.dead, position: [x!, y!, z!], velocity: [local.knock[0], local.vy, local.knock[1]], lost: local.lost })
        continue
      }
      const entity = sampled.get(id)?.entity
      if (!entity?.place) continue
      const { x, y, z } = entity.place
      soldiers.push({
        key, body,
        down: entity.flags?.stance === STANCE_DOWN,
        dead: ((entity.flags?.flags ?? 0) & FLAG_DEAD) !== 0,
        position: [x, y, z],
        velocity: [entity.motion?.knock_x ?? 0, entity.motion?.vy ?? 0, entity.motion?.knock_z ?? 0],
        lost: (entity.limbs?.lost ?? 0) & LIMB_ALL,
      })
    }
    ragdolls.update(soldiers, eye, dt)
  }
}
