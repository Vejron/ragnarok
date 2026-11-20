import { NO_WATER, type TileRef } from '@/world'
import type { TileGround } from './terrain'
import { churn, FLOW_STEP, REACH_STEP, shore, wave } from './waves'

/** One tile's water surface heights, laid out like its ground. */
interface TileWater {
  ground: TileGround
  levels: Float32Array
  /** How wide the open water is at each sample, in `REACH_STEP`s (`TileData.reach`); null for none. */
  reach: Uint8Array | null
  /** Which way and how fast the water runs at each sample, `x` then `z` in `FLOW_STEP`s (`TileData.flow`); null where it is still. */
  flow: Int8Array | null
}

/** A boulder just under a river's surface (`Waters.rocksNear`): where it is, how deep its top lies (metres) and the current over it (m/s). */
export interface Rock {
  x: number
  z: number
  depth: number
  flow: [number, number]
}

/** How far from the camera, metres, and in how many directions `planeNear` looks for water. */
const PLANE_RADII = [6, 20, 45, 90, 160, 320, 640]
const PLANE_RAYS = 12
/**
 * Water within this of the mirrored plane, metres, shows the scene in it (the water shader fades it out
 * from 0.2 m to 0.5 m off): `planeNear` counts every level within it as the same water, so a river
 * falling gently past the camera is one surface and not a slice every 10 cm.
 */
const PLANE_BAND = 0.3
/** The plane mirrored last frame stays while it has at least this share of the best level's votes. */
const PLANE_KEEP = 0.8
/** How many times `planeNear` fits its plane to the water near it. */
const PLANE_FITS = 3
/**
 * How strongly the fit holds the plane level, metres squared: the tilt it takes is as if this were
 * added to the spread of the water about the camera. Water spread over a few metres barely tilts it.
 */
const PLANE_STIFFNESS = 4
/** The steepest the mirrored plane leans, metres a metre: steeper than a rapid's even fall. */
const PLANE_TILT_MAX = 0.1

/** The plane of water to mirror (`Waters.planeNear`): `y` metres high at `(x, z)`, rising `slopeX` metres a metre along x and `slopeZ` along z. */
export interface WaterPlane {
  x: number
  y: number
  z: number
  slopeX: number
  slopeZ: number
}

/** How quickly the mirror shown follows the plane fitted under it, per second: a change of water slides over about half a second. */
const MIRROR_FOLLOW = 6
/** How much of the mirror fades in or out a second, where water comes into reach or goes out of it. */
const MIRROR_FADE = 2

/** The mirror as it is shown (`easeMirror`): its plane and how much of it shows, 0 to 1. */
export interface ShownMirror {
  plane: WaterPlane
  strength: number
}

/**
 * The mirror shown, `dt` seconds after `shown`, where `planeNear` fitted `fit` (null for no water about):
 * the plane slides towards the fit and the mirror fades in or out, so it never jumps or pops. With no
 * water about the last plane stays while it fades; null once it has gone.
 */
export function easeMirror(shown: ShownMirror | null, fit: WaterPlane | null, dt: number): ShownMirror | null {
  if (!fit) {
    if (!shown) return null
    const strength = shown.strength - MIRROR_FADE * dt
    return strength > 0 ? { plane: shown.plane, strength } : null
  }
  if (!shown) return { plane: { ...fit }, strength: Math.min(1, MIRROR_FADE * dt) }
  const k = 1 - Math.exp(-MIRROR_FOLLOW * dt)
  const p = shown.plane
  // The shown plane's height where the fit is given, so the two are compared at one place.
  const y = p.y + p.slopeX * (fit.x - p.x) + p.slopeZ * (fit.z - p.z)
  return {
    plane: {
      x: fit.x,
      y: y + (fit.y - y) * k,
      z: fit.z,
      slopeX: p.slopeX + (fit.slopeX - p.slopeX) * k,
      slopeZ: p.slopeZ + (fit.slopeZ - p.slopeZ) * k,
    },
    strength: Math.min(1, shown.strength + MIRROR_FADE * dt),
  }
}

/** Solves the 3×3 system `m · out = b` (`m` by rows), or null where it has no single answer. */
function solve3(m: number[], b: number[]): [number, number, number] | null {
  const det = (a: number[]) => a[0]! * (a[4]! * a[8]! - a[5]! * a[7]!) - a[1]! * (a[3]! * a[8]! - a[5]! * a[6]!) + a[2]! * (a[3]! * a[7]! - a[4]! * a[6]!)
  const whole = det(m)
  if (Math.abs(whole) < 1e-12) return null
  const column = (c: number) => det(m.map((value, i) => (i % 3 === c ? b[Math.floor(i / 3)]! : value)))
  return [column(0) / whole, column(1) / whole, column(2) / whole]
}

/**
 * The streamed-in tiles' water surface, to put splashes at the surface and not on the carved bed,
 * with or without its waves (`surfaceAt`). The sim has its own copy, which vehicles ride.
 */
export class Waters {
  private readonly tiles = new Map<number, TileWater>()
  /** No waves: the water is ice. */
  calm = false

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
  ) {}

  private key(col: number, row: number) {
    return row * 65536 + col
  }

  set(ref: TileRef, ground: TileGround | null, levels: Float32Array, reach?: Uint8Array, flow?: Int8Array) {
    if (ground && levels.length === ground.heights.length) {
      this.tiles.set(this.key(ref.col, ref.row), { ground, levels, reach: reach && reach.length === levels.length ? reach : null, flow: flow && flow.length === 2 * levels.length ? flow : null })
    }
  }

  /**
   * The current at `(x, z)`, `[x, z]` m/s, between the four samples around it, as the sim has it (`Waters::flow_at`):
   * what carries a swimmer, so what is taken off their speed to pace their strokes. Still where the tile has none.
   */
  flowAt(x: number, z: number): [number, number] {
    const tile = this.tileAt(x, z)
    if (!tile?.flow) return [0, 0]
    const { ground, flow } = tile
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    const out: [number, number] = [0, 0]
    for (let k = 0; k < 2; k++) {
      const at = (i: number, j: number) => flow[2 * (Math.min(j, last) * ground.side + Math.min(i, last)) + k]! * FLOW_STEP
      const near = at(i, j) + (at(i + 1, j) - at(i, j)) * fu
      const far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu
      out[k] = near + (far - near) * fv
    }
    return out
  }

  /**
   * The boulders that lie just under a river's surface within `reach` metres of `(x, z)`: the samples
   * where the water is no deeper than `depth` metres and no deeper than at any sample beside them, and
   * runs at `speed` m/s or more. Each with how deep its top lies and the current over it. None on ice.
   */
  rocksNear(x: number, z: number, reach: number, depth: number, speed: number): Rock[] {
    const out: Rock[] = []
    if (this.calm) return out
    for (const { ground, levels, flow } of this.tiles.values()) {
      if (!flow) continue
      const { x0, z0, cell, side, heights } = ground
      const from = (v: number, v0: number) => Math.max(1, Math.ceil((v - reach - v0) / cell))
      const to = (v: number, v0: number) => Math.min(side - 2, Math.floor((v + reach - v0) / cell))
      const deep = (at: number) => (levels[at]! !== NO_WATER && levels[at]! > heights[at]! ? levels[at]! - heights[at]! : Infinity)
      for (let row = from(z, z0); row <= to(z, z0); row++) {
        for (let column = from(x, x0); column <= to(x, x0); column++) {
          const at = row * side + column
          const top = deep(at)
          if (top > depth) continue
          const flowX = flow[at * 2]! * FLOW_STEP
          const flowZ = flow[at * 2 + 1]! * FLOW_STEP
          if (Math.hypot(flowX, flowZ) < speed) continue
          let lowest = true
          for (const beside of [at - 1, at + 1, at - side, at + side, at - side - 1, at - side + 1, at + side - 1, at + side + 1]) {
            // Of two as shallow, the first counts.
            if (deep(beside) < top || (deep(beside) === top && beside < at)) lowest = false
          }
          if (lowest) out.push({ x: x0 + column * cell, z: z0 + row * cell, depth: top, flow: [flowX, flowZ] })
        }
      }
    }
    return out
  }

  delete(ref: TileRef) {
    this.tiles.delete(this.key(ref.col, ref.row))
  }

  /**
   * The plane of the water worth mirroring from the camera at `(x, z)` looking along `(dx, dz)`, given at
   * the camera. It starts from the level with the most water within `PLANE_BAND` of it among sample
   * points around the camera and ahead, nearer ones counting more, and is then fitted to that water and
   * tilted with it, so a river falling past the camera is mirrored along its length and not in a strip
   * where it crosses one level. `kept`, the plane mirrored last frame, is fitted afresh instead while it
   * is nearly as good, so on a river that steps down through rapids the mirror does not flip between two
   * levels as the camera turns. Null when none of them is wet, so a mirrored pass is only drawn where
   * there is water to see.
   */
  planeNear(x: number, z: number, dx: number, dz: number, kept: WaterPlane | null = null): WaterPlane | null {
    const us: number[] = []
    const vs: number[] = []
    const levels: number[] = []
    const surfaces: number[] = []
    const weights: number[] = []
    for (const radius of PLANE_RADII) {
      for (let i = 0; i < PLANE_RAYS; i++) {
        const angle = (i / PLANE_RAYS) * Math.PI * 2
        const px = Math.cos(angle)
        const pz = Math.sin(angle)
        const level = this.sampleAt(x + px * radius, z + pz * radius)
        if (level === NO_WATER) continue
        us.push(px * radius)
        vs.push(pz * radius)
        levels.push(level)
        const surface = this.levelAt(x + px * radius, z + pz * radius)
        surfaces.push(surface === NO_WATER ? level : surface)
        // What lies ahead matters more than what is behind.
        weights.push((0.5 + 0.5 * Math.max(0, px * dx + pz * dz)) / radius)
      }
    }
    if (levels.length === 0) return null
    // How far each sample lies off a plane through `y` over the camera, rising `sx` and `sz` a metre: by the
    // nearest sample's level when voting, so a rapid's ramp counts as the levels above and below it, and
    // by the surface read between the samples when fitting, so the plane glides as the camera moves.
    const off = (heights: number[], y: number, sx: number, sz: number, k: number) => heights[k]! - y - sx * us[k]! - sz * vs[k]!
    const score = (y: number, sx: number, sz: number) => levels.reduce((sum, _, k) => sum + (Math.abs(off(levels, y, sx, sz, k)) <= PLANE_BAND ? weights[k]! : 0), 0)
    let best = levels[0]!
    let most = -1
    for (const level of levels) {
      const s = score(level, 0, 0)
      if (s > most) {
        best = level
        most = s
      }
    }
    let y = best
    let sx = 0
    let sz = 0
    if (kept !== null) {
      const ky = kept.y + kept.slopeX * (x - kept.x) + kept.slopeZ * (z - kept.z)
      if (score(ky, kept.slopeX, kept.slopeZ) >= PLANE_KEEP * most) {
        y = ky
        sx = kept.slopeX
        sz = kept.slopeZ
      }
    }
    // Fitted by weighted least squares to the water within `PLANE_BAND` of the plane so far, a few times
    // over, so the plane reaches along a falling river as far as it falls evenly. The tilt is held back by
    // `PLANE_STIFFNESS`, so a narrow strip of water does not tip the plane across it.
    for (let round = 0; round < PLANE_FITS; round++) {
      let w = 0, wu = 0, wv = 0, wl = 0, wuu = 0, wvv = 0, wuv = 0, wul = 0, wvl = 0
      surfaces.forEach((level, k) => {
        if (Math.abs(off(surfaces, y, sx, sz, k)) > PLANE_BAND) return
        const weight = weights[k]!
        const u = us[k]!
        const v = vs[k]!
        w += weight; wu += weight * u; wv += weight * v; wl += weight * level
        wuu += weight * u * u; wvv += weight * v * v; wuv += weight * u * v
        wul += weight * u * level; wvl += weight * v * level
      })
      if (w === 0) break
      const ridge = PLANE_STIFFNESS * w
      const fitted = solve3([w, wu, wv, wu, wuu + ridge, wuv, wv, wuv, wvv + ridge], [wl, wul, wvl])
      if (!fitted) break
      const tilt = Math.hypot(fitted[1], fitted[2])
      const scale = tilt > PLANE_TILT_MAX ? PLANE_TILT_MAX / tilt : 1
      sx = fitted[1] * scale
      sz = fitted[2] * scale
      // With the tilt held back, the height is the inliers' mean height off the tilted plane.
      y = (wl - sx * wu - sz * wv) / w
    }
    return { x, y, z, slopeX: sx, slopeZ: sz }
  }

  /**
   * The water surface height at `(x, z)` with its waves, `time` seconds into their loop (`waveTime`), as
   * the sim has it (`Waters::surface`): as big as the water is wide and dying away towards the shore,
   * with the surge of a rapid's standing waves on top. `NO_WATER` where there is none.
   */
  surfaceAt(x: number, z: number, time: number): number {
    const level = this.levelAt(x, z)
    if (level === NO_WATER || this.calm) return level
    const [flowX, flowZ] = this.flowAt(x, z)
    return level + wave(x, z, time, this.reachAt(x, z), shore(this.depthAt(x, z))).height + churn(x, z, time, flowX, flowZ).height
  }

  /** How wide the open water is at `(x, z)`, metres, from the nearest ground sample, as the sim has it; 0 where it is not known. */
  private reachAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile?.reach) return 0
    const { ground, reach } = tile
    const column = Math.min(Math.max(Math.round((x - ground.x0) / ground.cell), 0), ground.side - 1)
    const row = Math.min(Math.max(Math.round((z - ground.z0) / ground.cell), 0), ground.side - 1)
    return reach[row * ground.side + column]! * REACH_STEP
  }

  /** How deep the water is at `(x, z)`, metres, between the four samples around it; 0 where it is dry. */
  private depthAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return 0
    const { ground, levels } = tile
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    const at = (i: number, j: number) => {
      const index = Math.min(j, last) * ground.side + Math.min(i, last)
      const level = levels[index]!
      return level !== NO_WATER && level > ground.heights[index]! ? level - ground.heights[index]! : 0
    }
    const near = at(i, j) + (at(i + 1, j) - at(i, j)) * fu
    const far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu
    return near + (far - near) * fv
  }

  private tileAt(x: number, z: number): TileWater | undefined {
    const grid = Math.max(1, Math.ceil((2 * this.halfExtent) / this.tileSize))
    const cell = (v: number) => Math.min(Math.max(Math.floor((v + this.halfExtent) / this.tileSize), 0), grid - 1)
    return this.tiles.get(this.key(cell(x), cell(z)))
  }

  /**
   * The water level of the ground sample nearest `(x, z)`, or `NO_WATER` where it is dry: what `planeNear`
   * votes on, so a rapid's ramp counts as the levels above and below it and not as a level of its own.
   */
  private sampleAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return NO_WATER
    const { ground, levels } = tile
    const column = Math.round((x - ground.x0) / ground.cell)
    const row = Math.round((z - ground.z0) / ground.cell)
    if (column < 0 || row < 0 || column >= ground.side || row >= ground.side) return NO_WATER
    const index = row * ground.side + column
    const level = levels[index]!
    return level !== NO_WATER && level > ground.heights[index]! ? level : NO_WATER
  }

  /**
   * The still water surface's height at `(x, z)`, or `NO_WATER` where the nearest sample is dry (or the tile is
   * not loaded). A river's surface falls along it and is shaped by its current, so the height is read between
   * the wet samples of the four around the point, as the sim has it (`Waters::level_at`).
   */
  levelAt(x: number, z: number): number {
    const tile = this.tileAt(x, z)
    if (!tile) return NO_WATER
    const { ground, levels } = tile
    const column = Math.round((x - ground.x0) / ground.cell)
    const row = Math.round((z - ground.z0) / ground.cell)
    if (column < 0 || row < 0 || column >= ground.side || row >= ground.side) return NO_WATER
    // Water at or under the bed is dry land that the importer left a level on.
    const wet = (index: number) => levels[index]! !== NO_WATER && levels[index]! > ground.heights[index]!
    if (!wet(row * ground.side + column)) return NO_WATER
    const last = ground.side - 1
    const u = Math.max(0, (x - ground.x0) / ground.cell)
    const v = Math.max(0, (z - ground.z0) / ground.cell)
    const i = Math.min(Math.floor(u), Math.max(0, last - 1))
    const j = Math.min(Math.floor(v), Math.max(0, last - 1))
    const fu = Math.min(1, u - i)
    const fv = Math.min(1, v - j)
    let sum = 0
    let weight = 0
    for (const [di, dj, share] of [[0, 0, (1 - fu) * (1 - fv)], [1, 0, fu * (1 - fv)], [0, 1, (1 - fu) * fv], [1, 1, fu * fv]] as const) {
      const index = Math.min(j + dj, last) * ground.side + Math.min(i + di, last)
      if (wet(index)) {
        sum += levels[index]! * share
        weight += share
      }
    }
    return weight > 0 ? sum / weight : levels[row * ground.side + column]!
  }
}
