import { Vector3, type Node } from '@ragnarok/engine'
import { DEATH_BLAST, DEATH_OVERKILL, DEATH_ROADKILL, FLAG_DEAD, FLAG_GRAZING } from '@/net/codec'
import { ANIMAL_URLS, AnimalModel } from '@/render/animal'
import type { Gore } from '@/render/gore'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** A wounded animal drips this often, ms, after a hit of `DRIP_FULL` or more; slower after a lighter one. */
const DRIP_MS = 250
const DRIP_SLOW_MS = 700
const DRIP_FULL = 40
/** How thick a deer's body is, for where a round comes out of it, metres. */
const THROUGH = 0.45

export interface AnimalWorld {
  /** Where models go. */
  root: Node
  /** A contact shadow under a soldier-sized body centred at `(x, y, z)`. */
  blobFoot(x: number, y: number, z: number): void
  /** Blood. */
  gore: Gore
}

/** One animal drawn: its model, the hits it has been seen to take, and whether it was seen alive. */
interface Shown {
  model: AnimalModel
  /** `struck.count` last seen; undefined before the first hit of a life. */
  hits: number | undefined
  /** The last hit's size, for how fast it drips. */
  hurt: number
  dead: boolean
  drippedAt: number
}

const torn = (style: number | undefined) => style === DEATH_BLAST || style === DEATH_ROADKILL || style === DEATH_OVERKILL

/**
 * Wild animals, deer and stags: grazing, walking and galloping off, as the server moves them. Shot,
 * they bleed as a soldier does and leave a trail of drops while wounded; killed, they fall and lie in
 * a pool of blood, or are blown into pieces, until they come back.
 */
export class Animals implements Presenter<Frame> {
  private readonly shown = new Map<number, Shown>()

  constructor(private readonly world: AnimalWorld) {}

  wants(entity: Entity) {
    return entity.kind !== undefined && ANIMAL_URLS.has(entity.kind.kind) && entity.orientation === undefined && entity.place !== undefined
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { x, y, z } = entity.place!
    const flags = entity.flags?.flags ?? 0
    const kind = entity.kind!.kind
    const dead = (flags & FLAG_DEAD) !== 0
    const struck = entity.struck
    let shown = this.shown.get(id)
    // First drawn: whatever it took before then is not shown again, and a dead one is already down.
    const first = shown?.model.kind !== kind
    if (first) {
      shown?.model.dispose()
      const model = new AnimalModel(kind)
      this.world.root.add(model.root)
      shown = { model, hits: struck?.count, hurt: struck?.amount ?? 0, dead, drippedAt: frame.now }
      this.shown.set(id, shown)
      if (dead && torn(entity.fell?.style)) model.root.visible = false
    }
    const s = shown!
    const { model } = s
    model.update([x, y, z], entity.heading?.yaw ?? 0, (flags & FLAG_GRAZING) !== 0, frame.dt, dead, first && dead)
    const { gore } = this.world
    const centre = new Vector3()

    if (struck && struck.count !== s.hits) {
      const at = new Vector3(x, y, z)
      if (struck.x || struck.y || struck.z) at.add(new Vector3(struck.x, struck.y, struck.z))
      else at.y += model.bodyHeight
      const offset = at.clone().sub(model.centre(centre))
      if (model.root.visible) gore.wound(at, struck.yaw, struck.amount, THROUGH, (out) => model.centre(out).add(offset))
      s.hurt = struck.amount
    }
    s.hits = struck?.count

    if (dead && !s.dead) {
      const style = entity.fell?.style ?? 0
      const body = new Vector3()
      gore.fell(() => model.centre(body), style, struck?.yaw ?? 0, torn(style) ? model.tear() : [])
    }
    // Back: whole and unhurt.
    if (!dead && s.dead) s.hurt = 0
    s.dead = dead

    if (!dead && s.hits !== undefined) {
      const every = DRIP_SLOW_MS - (DRIP_SLOW_MS - DRIP_MS) * Math.min(1, s.hurt / DRIP_FULL)
      if (frame.now - s.drippedAt > every) {
        s.drippedAt = frame.now
        gore.dripAt(() => model.centre(centre))
      }
    }
    if (!dead) this.world.blobFoot(x, y + CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS, z)
  }

  remove(id: number) {
    this.shown.get(id)?.model.dispose()
    this.shown.delete(id)
  }
}
