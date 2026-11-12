import type { Geometry } from '@ragnarok/engine'

/**
 * Tree impostors: pictures of each tree archetype from many directions, drawn from its own solid, so a
 * far tree can be one quad that shows the solid as it would look from there.
 *
 * The directions over the upper half of the sky are laid on a square by the hemi-octahedral map
 * (`encodeView`), and the square is cut into `IMPOSTOR_FRAMES` × `IMPOSTOR_FRAMES` views, its edge the
 * horizon and its middle straight down. Each view is an orthographic picture `IMPOSTOR_FRAME_PX` across of
 * the tree's bounding sphere, in the tree's own unit space (crown radius 1, height 1). The foliage shader
 * blends the three views nearest the camera's direction. Lighting is left to it: the pictures hold the
 * colour, the normal and the depth of the surface under each pixel.
 */

/** Views along each side of a kind's grid of pictures. */
export const IMPOSTOR_FRAMES = 8
/** Pixels across one picture. */
export const IMPOSTOR_FRAME_PX = 64
/** Kinds along each side of the atlas. */
export const IMPOSTOR_TILES = 3
/** Pixels across the atlas. */
export const IMPOSTOR_ATLAS_PX = IMPOSTOR_FRAMES * IMPOSTOR_FRAME_PX * IMPOSTOR_TILES
/** Pixels over which a picture's coverage goes from none to whole across the silhouette. */
export const IMPOSTOR_SPREAD = 3
/** How much wider a picture is than the bounding sphere it frames, so linear filtering leaves the silhouette whole. */
export const IMPOSTOR_MARGIN = 1.1

type V3 = [number, number, number]

/**
 * Where on the grid square (each -1 to 1) a direction falls, `y` up. A direction below the horizon
 * is taken as on it: a tree is not seen from beneath.
 */
export function encodeView(x: number, y: number, z: number): [number, number] {
  const sum = Math.abs(x) + Math.max(y, 0) + Math.abs(z) || 1
  const px = x / sum
  const pz = z / sum
  return [px + pz, px - pz]
}

/** The unit direction at a point of the grid square: the inverse of `encodeView`. */
export function decodeView(a: number, b: number): V3 {
  const px = (a + b) / 2
  const pz = (a - b) / 2
  const y = Math.max(0, 1 - Math.abs(px) - Math.abs(pz))
  const length = Math.hypot(px, y, pz)
  return [px / length, y / length, pz / length]
}

/** The direction picture `(i, j)` of a kind's grid is taken from. */
export function frameView(i: number, j: number): V3 {
  const step = 2 / (IMPOSTOR_FRAMES - 1)
  return decodeView(-1 + i * step, -1 + j * step)
}

/**
 * The picture's right and up for view `v`: right is level, up is square to it and to `v`. The foliage
 * shader works them out the same way.
 */
export function frameBasis(v: V3): { right: V3; up: V3 } {
  let rx = v[2], rz = -v[0]
  const length = Math.hypot(rx, rz)
  if (length < 1e-6) {
    rx = 1
    rz = 0
  } else {
    rx /= length
    rz /= length
  }
  // up = v × right
  const up: V3 = [v[1] * rz, v[2] * rx - v[0] * rz, -v[1] * rx]
  return { right: [rx, 0, rz], up }
}

/** What a kind's pictures frame: the centre and radius of its bounding sphere, in unit space. */
export interface ImpostorFrame {
  center: V3
  radius: number
}

/** The atlas: colour and coverage (sRGB), and normal and depth (linear), each `IMPOSTOR_ATLAS_PX` square, RGBA8. */
export interface ImpostorAtlas {
  albedo: Uint8Array
  normalDepth: Uint8Array
}

/** The pixel at which kind tile `tile`'s picture `(i, j)` starts, column and row. */
export function frameOrigin(tile: number, i: number, j: number): [number, number] {
  const kindPx = IMPOSTOR_FRAMES * IMPOSTOR_FRAME_PX
  return [(tile % IMPOSTOR_TILES) * kindPx + i * IMPOSTOR_FRAME_PX, Math.floor(tile / IMPOSTOR_TILES) * kindPx + j * IMPOSTOR_FRAME_PX]
}

/** A linear colour channel (0 to 1) as an sRGB byte: the inverse of the engine's `Color.hex`. */
function srgbByte(linear: number, channel: number): number {
  // A part is one colour: most calls repeat the channel's last.
  if (linear === lastLinear[channel]) return lastByte[channel]!
  const c = Math.min(1, Math.max(0, linear))
  lastLinear[channel] = linear
  lastByte[channel] = Math.round((c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055) * 255)
  return lastByte[channel]!
}
const lastLinear = new Float64Array([-1, -1, -1])
const lastByte = new Uint8Array(3)

/**
 * Takes kind tile `tile`'s pictures of `solid` into `atlas`, by drawing its own triangles, vertex colours
 * and smoothed normals, so the impostor shows just what the solid would and the fade between them is
 * hard to see.
 */
export function bakeImpostor(solid: Geometry, frame: ImpostorFrame, tile: number, atlas: ImpostorAtlas, cull = true) {
  const size = IMPOSTOR_ATLAS_PX
  const px = IMPOSTOR_FRAME_PX
  const extent = frame.radius * IMPOSTOR_MARGIN
  const [cx, cy, cz] = frame.center
  const positions = solid.positions
  const normals = solid.normals!
  const colors = solid.colors!
  const indices = solid.indices
  const triangles = indices ? indices.length / 3 : solid.vertexCount / 3
  const vertex = (n: number) => (indices ? indices[n]! : n)
  const count = solid.vertexCount
  // Each vertex in the picture: pixel column and row, and how far toward the viewer it is.
  const sx = new Float32Array(count), sy = new Float32Array(count), sz = new Float32Array(count)
  const depth = new Float32Array(px * px)
  const nearest = new Int32Array(px * px)
  // The nearest surface under each pixel's middle: a triangle's index (in `nearest`), and its weights.
  const w = new Float32Array(px * px * 3)
  const inside = new Uint8Array(px * px)
  const toInside = new Int32Array(px * px)
  const toOutside = new Int32Array(px * px)
  const signed = new Float32Array(px * px)
  const blurred = new Float32Array(px * px)
  for (let j = 0; j < IMPOSTOR_FRAMES; j++) {
    for (let i = 0; i < IMPOSTOR_FRAMES; i++) {
      const v = frameView(i, j)
      const { right, up } = frameBasis(v)
      for (let k = 0; k < count; k++) {
        const x = positions[k * 3]! - cx, y = positions[k * 3 + 1]! - cy, z = positions[k * 3 + 2]! - cz
        sx[k] = ((x * right[0] + y * right[1] + z * right[2]) / extent * 0.5 + 0.5) * px
        sy[k] = (0.5 - (x * up[0] + y * up[1] + z * up[2]) / extent * 0.5) * px
        sz[k] = x * v[0] + y * v[1] + z * v[2]
      }
      depth.fill(-Infinity)
      nearest.fill(-1)
      for (let t = 0; t < triangles; t++) {
        const a = vertex(t * 3), b = vertex(t * 3 + 1), c = vertex(t * 3 + 2)
        const ax = sx[a]!, ay = sy[a]!, bx = sx[b]!, by = sy[b]!, qx = sx[c]!, qy = sy[c]!
        const area = (bx - ax) * (qy - ay) - (qx - ax) * (by - ay)
        // Wound counter-clockwise seen from outside, so with rows downward a face toward the viewer has a negative area.
        if (Math.abs(area) < 1e-9 || (cull && area > 0)) continue
        const x0 = Math.max(0, Math.floor(Math.min(ax, bx, qx))), x1 = Math.min(px - 1, Math.ceil(Math.max(ax, bx, qx)))
        const y0 = Math.max(0, Math.floor(Math.min(ay, by, qy))), y1 = Math.min(px - 1, Math.ceil(Math.max(ay, by, qy)))
        for (let row = y0; row <= y1; row++) {
          const y = row + 0.5
          for (let col = x0; col <= x1; col++) {
            const x = col + 0.5
            const wa = ((bx - x) * (qy - y) - (qx - x) * (by - y)) / area
            const wb = ((qx - x) * (ay - y) - (ax - x) * (qy - y)) / area
            const wc = 1 - wa - wb
            if (wa < 0 || wb < 0 || wc < 0) continue
            const z = wa * sz[a]! + wb * sz[b]! + wc * sz[c]!
            const at = row * px + col
            if (z <= depth[at]!) continue
            depth[at] = z
            nearest[at] = t
            w[at * 3] = wa
            w[at * 3 + 1] = wb
            w[at * 3 + 2] = wc
          }
        }
      }
      const [ox, oy] = frameOrigin(tile, i, j)
      const albedo = atlas.albedo, nd = atlas.normalDepth
      for (let at = 0; at < px * px; at++) inside[at] = nearest[at]! >= 0 ? 1 : 0
      nearestOf(inside, 1, px, toInside)
      nearestOf(inside, 0, px, toOutside)
      // The coverage is a signed distance to the silhouette, so filtering keeps its edge smooth however
      // near the picture is drawn, and the mips do not thin it. Measured from whole pixels it steps along
      // a slanting edge; blurred once, the steps go and a smooth outline stays where it was (twice filled
      // the notches between a spruce's tiers).
      for (let at = 0; at < px * px; at++) {
        const covered = inside[at] === 1
        const other = covered ? toOutside[at]! : toInside[at]!
        const apart = other < 0 ? IMPOSTOR_SPREAD * 2 : Math.sqrt(((other % px) - (at % px)) ** 2 + (((other / px) | 0) - ((at / px) | 0)) ** 2) - 0.5
        signed[at] = covered ? apart : -apart
      }
      blur(signed, blurred, px)
      for (let at = 0; at < px * px; at++) {
        const out = ((oy + ((at / px) | 0)) * size + ox + (at % px)) * 4
        const covered = inside[at] === 1
        albedo[out + 3] = Math.round(Math.min(1, Math.max(0, 0.5 + blurred[at]! / (2 * IMPOSTOR_SPREAD))) * 255)
        // A pixel off the tree takes the colour, normal and depth of the nearest on it, so filtering at the
        // edge does not mix in something else.
        const from = covered ? at : toInside[at]!
        if (from < 0) {
          albedo[out] = albedo[out + 1] = albedo[out + 2] = 0
          nd[out] = Math.round((v[0] * 0.5 + 0.5) * 255)
          nd[out + 1] = Math.round((v[1] * 0.5 + 0.5) * 255)
          nd[out + 2] = Math.round((v[2] * 0.5 + 0.5) * 255)
          nd[out + 3] = 128
          continue
        }
        const t = nearest[from]!
        const a = vertex(t * 3) * 3, b = vertex(t * 3 + 1) * 3, c = vertex(t * 3 + 2) * 3
        const wa = w[from * 3]!, wb = w[from * 3 + 1]!, wc = w[from * 3 + 2]!
        const nx = wa * normals[a]! + wb * normals[b]! + wc * normals[c]!
        const ny = wa * normals[a + 1]! + wb * normals[b + 1]! + wc * normals[c + 1]!
        const nz = wa * normals[a + 2]! + wb * normals[b + 2]! + wc * normals[c + 2]!
        const scale = 0.5 / (Math.sqrt(nx * nx + ny * ny + nz * nz) || 1)
        // A triangle is all one part, so one colour.
        albedo[out] = srgbByte(colors[a]!, 0)
        albedo[out + 1] = srgbByte(colors[a + 1]!, 1)
        albedo[out + 2] = srgbByte(colors[a + 2]!, 2)
        nd[out] = Math.round((nx * scale + 0.5) * 255)
        nd[out + 1] = Math.round((ny * scale + 0.5) * 255)
        nd[out + 2] = Math.round((nz * scale + 0.5) * 255)
        // How far in front of the picture's middle plane the surface is, in picture half-widths.
        nd[out + 3] = Math.round(Math.min(1, Math.max(0, depth[from]! / extent * 0.5 + 0.5)) * 255)
      }
    }
  }
}

/** A 3×3 box blur of a `px` square, its edges held. */
function blur(from: Float32Array, to: Float32Array, px: number) {
  for (let y = 0; y < px; y++) {
    for (let x = 0; x < px; x++) {
      let sum = 0
      for (let dy = -1; dy <= 1; dy++) {
        const row = Math.min(px - 1, Math.max(0, y + dy)) * px
        for (let dx = -1; dx <= 1; dx++) sum += from[row + Math.min(px - 1, Math.max(0, x + dx))]!
      }
      to[y * px + x] = sum / 9
    }
  }
}

/**
 * For each pixel of a `px` square, the nearest pixel (as an index) whose `mask` is `want`, or -1 if
 * there is none: two sweeps that hand each pixel its neighbours' nearest, close to the true nearest.
 */
function nearestOf(mask: Uint8Array, want: number, px: number, out: Int32Array) {
  for (let at = 0; at < px * px; at++) out[at] = mask[at] === want ? at : -1
  for (let pass = 0; pass < 2; pass++) {
    const step = pass === 0 ? 1 : -1
    const first = pass === 0 ? 0 : px - 1
    for (let y = first; y >= 0 && y < px; y += step) {
      for (let x = first; x >= 0 && x < px; x += step) {
        const at = y * px + x
        let best = out[at]!
        let bestD = best < 0 ? Infinity : ((best % px) - x) ** 2 + (((best / px) | 0) - y) ** 2
        if (bestD === 0) continue
        // The neighbours already swept: the row behind, and the one before on this row.
        for (let n = 0; n < 4; n++) {
          const nx = n === 3 ? x - step : x + (n - 1)
          const ny = n === 3 ? y : y - step
          if (nx < 0 || nx >= px || ny < 0 || ny >= px) continue
          const seed = out[ny * px + nx]!
          if (seed < 0) continue
          const d = ((seed % px) - x) ** 2 + (((seed / px) | 0) - y) ** 2
          if (d < bestD) {
            bestD = d
            best = seed
          }
        }
        out[at] = best
      }
    }
  }
}

/** An empty atlas, to bake kinds into. */
export function emptyAtlas(): ImpostorAtlas {
  const bytes = IMPOSTOR_ATLAS_PX * IMPOSTOR_ATLAS_PX * 4
  return { albedo: new Uint8Array(bytes), normalDepth: new Uint8Array(bytes) }
}
