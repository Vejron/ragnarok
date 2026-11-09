import type { Sphere } from '@ragnarok/engine'
import { add, dot, hash, sub, unit, type P } from './geom'
import { PANE_OUT, facing, type Opening } from './windows'

/*
 * The glass in a house's windows: a translucent sheet over each window's dark pane, drawn in one
 * mesh a tile apart from the walls (`glassBuffers`), so that a pane can be broken on its own: its
 * sheet collapses and a few jagged teeth are left round the frame (`writePane`). Whether a round
 * or a blast breaks a pane is decided here from the server's own numbers (`paneStruck`,
 * `paneBlasted`), so every client breaks the same glass. Pure, so it runs in the tile worker too.
 */

/** How far the sheet stands out from the wall, metres: a hair in front of the dark pane, inside the frame. */
export const GLASS_OUT = PANE_OUT + 0.01
/** How far inside the frame's inner edge the sheet stops, metres. */
export const GLASS_INSET = 0.01
/** Vertices a pane takes, whole or broken: a sheet of two triangles, or up to four teeth. */
export const PANE_VERTICES = 18
/**
 * How far a round's end may be from the sheet along its normal to have gone through it, metres,
 * behind and in front: it stops on the wall's cell box, up to a cell (72 cm) in front of the wall.
 */
export const PANE_NEAR: readonly [number, number] = [-0.12, 0.77]
/** How far outside a pane's rectangle a round still counts as through it, metres. */
export const PANE_SLACK = 0.08
/** A blast breaks every pane within its radius and some out to this many radii, fewer the further. */
export const GLASS_REACH = 1.6

export interface GlassBuffers {
  positions: Float32Array
  normals: Float32Array
  bounds: Sphere
  /** Two numbers a pane: the building's index in the tile, and the opening's index in `openingsOf` it. */
  panes: Uint32Array
}

/** A window's frame: its corner `a`, unit `along` the wall and `up`, its width and height, and the wall's normal. */
function frameOf(o: Opening) {
  const [a, b, , d] = o.corners
  const along = unit([b[0] - a[0], 0, b[2] - a[2]])
  return { a, along, width: Math.hypot(b[0] - a[0], b[2] - a[2]), height: d[1] - a[1], normal: o.normal }
}

/** The middle of the pane, on the sheet. */
export function paneCentre(o: Opening): P {
  const { a, along, width, height, normal } = frameOf(o)
  return add(add(add(a, along, width / 2), [0, height / 2, 0]), normal, GLASS_OUT)
}

/** The sheet's four corners, in order round it and wound to face out of the wall. */
export function paneSheet(o: Opening): [P, P, P, P] {
  const { a, along, width, height, normal } = frameOf(o)
  const at = (u: number, v: number): P => add(add(add(a, along, u), [0, v, 0]), normal, GLASS_OUT)
  const [i, w, h] = [GLASS_INSET, width, height]
  return facing([at(i, i), at(w - i, i), at(w - i, h - i), at(i, h - i)], normal)
}

/**
 * The teeth of glass left in a broken pane's frame: two to four triangles with their base on a side
 * of the frame and their point reaching into the pane, the same for the same pane.
 */
export function teethOf(o: Opening): [P, P, P][] {
  const { a, along, width, height, normal } = frameOf(o)
  const c = paneCentre(o)
  const h = (salt: number) => hash(c[0], c[1], c[2], salt)
  const at = (u: number, v: number): P => add(add(add(a, along, u), [0, v, 0]), normal, GLASS_OUT)
  const count = 2 + Math.floor(h(1) * 3)
  const teeth: [P, P, P][] = []
  for (let k = 0; k < count; k++) {
    const side = Math.floor(h(10 + k) * 4) % 4
    const t = 0.15 + 0.7 * h(20 + k)
    const half = 0.07 + 0.1 * h(30 + k)
    const reach = 0.12 + 0.18 * h(40 + k)
    const lean = (h(50 + k) - 0.5) * 0.1
    // Along the side, from its start: (u, v) on the frame's inner edge, and the way into the pane.
    const length = side % 2 === 0 ? width : height
    const s = t * length
    const edge = (p: number): [number, number] => (side === 0 ? [p, GLASS_INSET] : side === 1 ? [width - GLASS_INSET, p] : side === 2 ? [p, height - GLASS_INSET] : [GLASS_INSET, p])
    const into: [number, number] = side === 0 ? [0, 1] : side === 1 ? [-1, 0] : side === 2 ? [0, -1] : [1, 0]
    const [b0, b1] = [edge(Math.max(GLASS_INSET, s - half)), edge(Math.min(length - GLASS_INSET, s + half))]
    const tip = edge(s).map((v, i) => v + into[i]! * reach + (i === 0 ? into[1]! : -into[0]!) * lean) as [number, number]
    const [p, q, r] = [at(...b0), at(...b1), at(...tip)]
    // Wound to face out of the wall, as the sheet is.
    const e = sub(q, p)
    const f = sub(r, p)
    const n: P = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]]
    teeth.push(dot(n, normal) >= 0 ? [p, q, r] : [q, p, r])
  }
  return teeth
}

/**
 * Writes pane `pane` of `o` into `positions` and `normals`: its sheet when whole, its teeth when
 * `broken`, the slots it does not need collapsed onto a corner so they draw nothing.
 */
export function writePane(positions: Float32Array, normals: Float32Array, pane: number, o: Opening, broken: boolean, gone = false) {
  const base = pane * PANE_VERTICES * 3
  const { a, normal } = frameOf(o)
  for (let v = 0; v < PANE_VERTICES; v++) {
    positions.set(a, base + v * 3)
    normals.set(normal, base + v * 3)
  }
  // Gone with its wall: nothing is left, not a tooth.
  if (gone) return
  const put = (slot: number, p: P) => positions.set(p, base + slot * 3)
  if (!broken) {
    const [p, q, r, s] = paneSheet(o)
    ;[p, q, r, p, r, s].forEach((corner, slot) => put(slot, corner))
    return
  }
  teethOf(o).forEach((tooth, k) => tooth.forEach((corner, i) => put(6 + k * 3 + i, corner)))
}

/** The glass of the buildings' windows (`openings[building]`, null for one without), or null when there is none. */
export function glassBuffers(openings: readonly (readonly Opening[] | null)[]): GlassBuffers | null {
  let count = 0
  openings.forEach((list) => list?.forEach((o) => !o.door && count++))
  if (count === 0) return null
  const positions = new Float32Array(count * PANE_VERTICES * 3)
  const normals = new Float32Array(count * PANE_VERTICES * 3)
  const panes = new Uint32Array(count * 2)
  const box = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity]
  let pane = 0
  openings.forEach((list, building) =>
    list?.forEach((o, opening) => {
      if (o.door) return
      writePane(positions, normals, pane, o, false)
      panes[pane * 2] = building
      panes[pane * 2 + 1] = opening
      for (const p of o.corners) for (let k = 0; k < 3; k++) {
        box[k] = Math.min(box[k]!, p[k]!)
        box[k + 3] = Math.max(box[k + 3]!, p[k]!)
      }
      pane++
    }),
  )
  const bounds = { x: (box[0]! + box[3]!) / 2, y: (box[1]! + box[4]!) / 2, z: (box[2]! + box[5]!) / 2, radius: Math.hypot(box[3]! - box[0]!, box[4]! - box[1]!, box[5]! - box[2]!) / 2 + 0.1 }
  return { positions, normals, bounds, panes }
}

/** Whether a round that ended at `at` went through pane `o`: at the sheet, give or take the wall's cell box in front of it. */
export function paneStruck(o: Opening, at: P): boolean {
  const { a, along, width, height, normal } = frameOf(o)
  const d = sub(at, a)
  const out = dot(d, normal) - GLASS_OUT
  if (out < PANE_NEAR[0] || out > PANE_NEAR[1]) return false
  const u = dot(d, along)
  const v = d[1]
  return u >= -PANE_SLACK && u <= width + PANE_SLACK && v >= -PANE_SLACK && v <= height + PANE_SLACK
}

/**
 * Whether a blast at `at` of `radius` breaks pane `o`: every pane within the radius, none past
 * `GLASS_REACH` radii or on the far side of its house, and between those fewer the further, decided
 * by the pane and the blast alone so it is the same on every client.
 */
export function paneBlasted(o: Opening, at: P, radius: number): boolean {
  const c = paneCentre(o)
  const d = Math.hypot(c[0] - at[0], c[1] - at[1], c[2] - at[2])
  if (d < radius) return true
  const reach = GLASS_REACH * radius
  if (d > reach) return false
  if (dot(sub(at, c), o.normal) < -0.3 * d) return false
  const t = (d - radius) / (reach - radius)
  const salt = Math.round(at[0] * 100) * 0.0113 + Math.round(at[1] * 100) * 0.0071 + Math.round(at[2] * 100) * 0.0137
  return hash(c[0], c[1], c[2], salt) < 1 - t * t
}

/** Where `count` shards of pane `o` start from, scattered over it, with a size each; the same for the same pane. */
export function glassShards(o: Opening, count: number): { at: P; size: number }[] {
  const { a, along, width, height, normal } = frameOf(o)
  const c = paneCentre(o)
  const out: { at: P; size: number }[] = []
  for (let k = 0; k < count; k++) {
    const u = 0.1 + 0.8 * hash(c[0], c[1], c[2], 100 + k)
    const v = 0.1 + 0.8 * hash(c[0], c[1], c[2], 200 + k)
    out.push({ at: add(add(add(a, along, u * width), [0, v * height, 0]), normal, GLASS_OUT), size: 0.08 + 0.12 * hash(c[0], c[1], c[2], 300 + k) })
  }
  return out
}

/** One number for pane `opening` of building `building`. */
export function paneKey(building: number, opening: number): number {
  return building * 4096 + opening
}

