import type { Entity } from './entities/units'
import type { HudStats } from './hud'
import { HEAD_HP, LIMB_HP, PLAYER_MAX_HP, RESPAWN_TICKS, TICK_MS } from './sim/consts'
import { emptyHotbar, HOTBAR_SLOTS, ITEM_NONE, type ItemSlot } from './sim/items'
import { bleedingOf, lostOf } from './sim/limbs'

const NOTHING: ItemSlot = { item: ITEM_NONE, count: 0, ammo: 0 }

/**
 * What the server tells the local player of themself alone, from their own entity's `[owner]`
 * components: their health, what they carry and what is left of them. One `reset` puts it all back.
 */
export class LocalPlayer {
  /** Zero is dead. */
  hp = PLAYER_MAX_HP
  /** `performance.now()` when they died, or null while alive. */
  diedAt: number | null = null
  hotbar: ItemSlot[] = emptyHotbar()
  /** How far a reload or a bandage has got. */
  progress = 0
  /** Their own `limbs`, as last sent. */
  limbs = 0
  /** What their head and limbs have left of their health (`Vitals::parts`). */
  parts: number[] = [HEAD_HP, LIMB_HP, LIMB_HP, LIMB_HP, LIMB_HP]
  /** They carry the backpack, as their own `carry` last said: they walk faster. */
  carrying = false

  get alive() {
    return this.hp > 0
  }

  reset() {
    this.hp = PLAYER_MAX_HP
    this.diedAt = null
    this.hotbar = emptyHotbar()
    this.progress = 0
    this.limbs = 0
    this.parts = [HEAD_HP, LIMB_HP, LIMB_HP, LIMB_HP, LIMB_HP]
    this.carrying = false
  }

  /** Takes what their own entity says now. */
  receive(own: Entity, now: number) {
    if (own.vitals) {
      this.setHealth(own.vitals.hp, now)
      this.progress = own.vitals.progress
      this.parts = own.vitals.parts
    }
    if (own.hotbar) this.hotbar = [...own.hotbar.slots, ...emptyHotbar()].slice(0, HOTBAR_SLOTS)
    this.limbs = own.limbs?.lost ?? 0
    this.carrying = own.carry !== undefined
  }

  private setHealth(hp: number, now: number) {
    if (hp === 0 && this.hp > 0) this.diedAt = now
    else if (hp > 0) this.diedAt = null
    this.hp = hp
  }

  /** What is in hotbar slot `slot`. */
  slot(slot: number): ItemSlot {
    return this.hotbar[slot] ?? NOTHING
  }

  /** Seconds until they are back, 0 while alive. */
  respawnIn(now: number) {
    return this.diedAt === null ? 0 : Math.max(0, (RESPAWN_TICKS * TICK_MS - (now - this.diedAt)) / 1000)
  }

  /** Their body for the HUD while any of it is hurt and they live. */
  body(): HudStats['limbs'] {
    const [head, ...limbs] = this.parts
    const hurt = this.limbs !== 0 || this.hp < PLAYER_MAX_HP || head! < HEAD_HP || limbs.some((left) => left < LIMB_HP)
    if (!hurt || this.hp <= 0) return null
    return { lost: lostOf(this.limbs), bleeding: bleedingOf(this.limbs), head: head! / HEAD_HP, torso: this.hp / PLAYER_MAX_HP, parts: limbs.map((left) => left / LIMB_HP) }
  }
}
