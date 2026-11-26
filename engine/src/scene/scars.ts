/** Meshes that can carry scars at once: a slot each. */
export const SCAR_SLOTS = 512
/** Scars one mesh keeps: a ring, the oldest overwritten. */
export const SCAR_POINTS = 24
/**
 * Of those, how many are kept for light scars (a bullet's chip): a ring of their own, first in the slot,
 * so a machine gun's hail never wipes out a shell's dent.
 */
export const SCAR_LIGHT_POINTS = 6
/** vec4s a scar takes: where it is and its reach, the way the round came in, then its style, birth, dent depth and seed. */
export const SCAR_VEC4S = 3
/** vec4s a slot takes: a header (`count, grime, unused, unused`), then its scars. The shader's `SCAR_STRIDE`. */
export const SCAR_STRIDE = 1 + SCAR_POINTS * SCAR_VEC4S
const SLOT_FLOATS = SCAR_STRIDE * 4

/** The plate is pressed in round the point, as deep as the scar's `depth`. */
export const SCAR_DENT = 1
/** A hole through it: black, with a ring of tempered steel round it. */
export const SCAR_HOLE = 2
/** A gouge torn along the surface the way the round went, down to bare metal: a round that glanced off. */
export const SCAR_GOUGE = 4
/** Soot and char spread round it, more upward, as smoke rises. */
export const SCAR_SOOT = 8
/** Glowing hot at first, cooling over `SCAR_COOL` seconds. */
export const SCAR_HOT = 16
/** Glass broken by a round: a hole crushed white at its edge, cracks running out from it in a star and a ring of them round it. Its pixels are drawn opaque on see-through glass. */
export const SCAR_CRACK = 32
/** Seconds a hot scar takes to cool from white-orange to dark: the shader's `SCAR_COOL`. */
export const SCAR_COOL = 7

/** One scar, in the frame of the mesh it is on (the mesh's own units: a scaled mesh's are not metres). */
export interface Scar {
  /** Where it is. */
  x: number
  y: number
  z: number
  /** How far its dent and soot reach from it. */
  radius: number
  /** The way the round was going (unit). */
  dx: number
  dy: number
  dz: number
  /** `SCAR_*` flags. */
  style: number
  /** How deep the plate is pressed in at the middle. */
  depth: number
  /** When it was made, seconds on the renderer's clock (`performance.now() / 1000`). */
  birth: number
}

/**
 * Damage drawn on meshes' own surfaces (HULL-DAMAGE.md): each mesh given a slot (`Mesh.scars`) is dented,
 * scorched, holed and gouged where its scars are, worked out in its shader from this list, so the marks
 * move with it (a turret, a Titan's arm) and bend its lighting. Kept by the scene; the renderer uploads
 * the slots that changed.
 */
export class Scars {
  readonly data = new Float32Array(SCAR_SLOTS * SLOT_FLOATS)
  /** Bumped by every change. */
  version = 0
  private readonly free: number[] = []
  /** Scars added to each slot ever, heavy and light. */
  private readonly heavy = new Uint32Array(SCAR_SLOTS)
  private readonly light = new Uint32Array(SCAR_SLOTS)
  private readonly dirty = new Set<number>()

  constructor() {
    for (let slot = SCAR_SLOTS - 1; slot >= 0; slot--) this.free.push(slot)
  }

  /** A slot for one mesh, as `Mesh.scars` takes it (the slot + 1), or 0 when every slot is taken. */
  alloc(): number {
    const slot = this.free.pop()
    if (slot === undefined) return 0
    this.reset(slot)
    return slot + 1
  }

  /** Gives slot `handle` back, its scars forgotten. Its mesh must stop naming it. */
  release(handle: number) {
    if (handle <= 0) return
    this.reset(handle - 1)
    this.free.push(handle - 1)
  }

  /** Adds `scar` to slot `handle`, overwriting its oldest of the same weight when they are all used: `light` for a bullet's chip. */
  add(handle: number, scar: Scar, light = false) {
    if (handle <= 0) return
    const slot = handle - 1
    const index = light ? this.light[slot]!++ % SCAR_LIGHT_POINTS : SCAR_LIGHT_POINTS + (this.heavy[slot]!++ % (SCAR_POINTS - SCAR_LIGHT_POINTS))
    const o = slot * SLOT_FLOATS
    // The shader looks at the first `count`: the light ring, then as many heavy scars as there are.
    const heavy = Math.min(this.heavy[slot]!, SCAR_POINTS - SCAR_LIGHT_POINTS)
    this.data[o] = heavy > 0 ? SCAR_LIGHT_POINTS + heavy : Math.min(this.light[slot]!, SCAR_LIGHT_POINTS)
    const p = o + 4 + index * SCAR_VEC4S * 4
    this.data.set([scar.x, scar.y, scar.z, scar.radius, scar.dx, scar.dy, scar.dz, 0, scar.style, scar.birth, scar.depth, Math.random()], p)
    this.touch(slot)
  }

  /** How worn the whole of slot `handle` looks, 0 to 1: sooted and dulled all over, as a battered vehicle is. */
  setGrime(handle: number, grime: number) {
    if (handle <= 0) return
    const o = (handle - 1) * SLOT_FLOATS + 1
    if (Math.abs(this.data[o]! - grime) < 0.01) return
    this.data[o] = grime
    this.touch(handle - 1)
  }

  /** The slots changed since the last call. */
  takeDirty(): number[] {
    const slots = [...this.dirty]
    this.dirty.clear()
    return slots
  }

  private reset(slot: number) {
    this.heavy[slot] = 0
    this.light[slot] = 0
    this.data.fill(0, slot * SLOT_FLOATS, (slot + 1) * SLOT_FLOATS)
    this.touch(slot)
  }

  private touch(slot: number) {
    this.dirty.add(slot)
    this.version++
  }
}

/** Floats a slot takes in `Scars.data`. */
export const SCAR_SLOT_FLOATS = SLOT_FLOATS
