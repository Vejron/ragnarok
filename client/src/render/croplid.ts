import { Color, Geometry, Group, Mesh, ShaderMaterial } from '@ragnarok/engine'
import { CROP_NONE, type TileData, type TileRef } from '@/world'
import { CROP_HEIGHT } from '@/sim/consts.generated'
import { WIND_HEADING } from '@/sim/wind'
import type { TileGround } from '@/sim/terrain'
import { GROUND_LIGHT_WGSL } from './groundlight'
import { WHEAT_DENSITY, type GrassDensity } from './grass'
import { CraterGrid } from './cratergrid'

/*
 * The far side of a field of wheat. The stalks (`grass.ts`, `WHEAT`) only stand near the camera, and
 * past them the field would be bare ground with whoever is in it in plain view. So each tile's crops
 * also make a lid: a solid slab as high as the wheat, in its colour. Under the camera's stalks it lies
 * sunk in the ground, and it rises out of it as they thin with distance, so a soldier far off in a
 * field is hidden to the height the wheat would hide them. It is opaque and discards nothing, which on
 * a tile-based GPU costs next to nothing.
 */

/** How high the lid stands, metres: just under the wheat's ears, which show over it where both are drawn. */
export const LID_HEIGHT = CROP_HEIGHT * 0.95
/** How far under the ground the lid lies where the stalks stand instead, metres. */
const LID_SUNK = 0.3
/**
 * The lid's colour. Measured, not derived: the colour at which the lid, drawn over the same pixels as
 * the stalks 60 to 100 m off (their leaves, ears, inked edges and the straw between them, from 3 m up
 * and from 14 m), comes out the same on the screen, so that where the one takes over from the other
 * no line shows. Measure it again after changing `WHEAT`'s colours or `CROP_GROUND_COLOR`.
 */
const LID_COLOR = 0xd2bc69
/** A camera this far over the ground, metres, is on foot or in a car, and sees the field from the side. */
const LID_EYE = 5
/** Metres nearer the lid comes for each metre the camera is higher than that: up everywhere from some 30 m over the field. */
const LID_PER_HEIGHT = 4.5
/** The share of the stalks' reach (`GrassDensity.far`) at which the lid starts to rise, and at which it is up. */
const LID_RISE_FROM = 0.55
const LID_RISE_TO = 0.8

/**
 * How much of the ground round each of a tile's crop cells is sown, 0 to 1: the mean of the cell and
 * the eight about it, those past the tile's edge taken to be as the cell at the edge is. The lid's
 * outline is where this crosses a half, which follows a field's slanting edge as a line and not as
 * the steps its cells make. A crater (`bare`, the tile's corner at `(x0, z0)` and `cell` metres a
 * cell) takes the share away toward its middle, down to a half at `CRATER_BARE` of its radius, where
 * the stalks stop too (`Grass.dent`): the lid has a hole over the bowl.
 */
function sownShare(crops: Uint8Array, n: number, bare: readonly Crater[], x0: number, z0: number, cell: number): Float32Array {
  const share = new Float32Array(n * n)
  const at = (i: number, j: number) => (crops[Math.min(n - 1, Math.max(0, j)) * n + Math.min(n - 1, Math.max(0, i))] === CROP_NONE ? 0 : 1)
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      let sum = 0
      for (let dj = -1; dj <= 1; dj++) for (let di = -1; di <= 1; di++) sum += at(i + di, j + dj)
      share[j * n + i] = sum / 9
    }
  }
  for (const { x, z, radius } of bare) {
    const from = (v: number, origin: number) => Math.max(0, Math.floor((v - radius - origin) / cell))
    const to = (v: number, origin: number) => Math.min(n - 1, Math.floor((v + radius - origin) / cell))
    for (let j = from(z, z0); j <= to(z, z0); j++) {
      for (let i = from(x, x0); i <= to(x, x0); i++) {
        const d = Math.hypot(x0 + (i + 0.5) * cell - x, z0 + (j + 0.5) * cell - z) / radius
        // A half at CRATER_BARE of the radius, none inside the bowl, all of it at the rim.
        const t = Math.min(1, Math.max(0, (d - (2 * CRATER_BARE - 1)) / (2 - 2 * CRATER_BARE)))
        share[j * n + i]! *= t
      }
    }
  }
  return share
}

/** A crater a shell dug: where, and how far its rim is from its middle, metres. */
export interface Crater {
  x: number
  z: number
  radius: number
}

/** The share of a crater's radius that is bare earth, as `grass.ts` has it for the stalks. */
const CRATER_BARE = 0.8

/**
 * The lid of one tile's `crops` (a square grid over the tile from `(x0, z0)`, `size` metres a side):
 * a top over the sown ground, with a skirt down to the ground along its outline and along the
 * tile's own edge, where the next tile's lid may not be. Its corners are the middles of the crop
 * cells and, round the tile's edge, the points of the edge beside them; between two corners of which
 * one is sown (`sownShare` over a half) the outline crosses where the share does, so it runs straight
 * along a field's edge whichever way that lies. Every vertex is on the ground, and `custom.x` is how
 * far the shader lifts it: `LID_HEIGHT` for the top, 0 for a skirt's foot. `bare` are the craters
 * dug in the field, which the lid is open over. Null when nothing is sown, or the ground is not there.
 */
export function lidGeometry(crops: Uint8Array, x0: number, z0: number, size: number, heightAt: (x: number, z: number) => number | null, bare: readonly Crater[] = []): Geometry | null {
  const n = Math.round(Math.sqrt(crops.length))
  if (n === 0 || !crops.some((cell) => cell !== CROP_NONE)) return null
  const cell = size / n
  const share = sownShare(crops, n, bare, x0, z0, cell)
  // Corners 0..n+1 a side: the cells' middles, with the tile's edge either end.
  const side = n + 2
  const place = (k: number) => Math.min(size, Math.max(0, (k - 0.5) * cell))
  const sown = (i: number, j: number) => share[Math.min(n - 1, Math.max(0, j - 1)) * n + Math.min(n - 1, Math.max(0, i - 1))]!
  const positions: number[] = []
  const custom: number[] = []
  const indices: number[] = []
  let missing = false
  const vertex = (x: number, z: number, lift: number) => {
    const y = heightAt(x, z)
    if (y === null) missing = true
    positions.push(x, y ?? 0, z)
    custom.push(lift, 0, 0, 0)
    return positions.length / 3 - 1
  }
  // The top's vertices, made when first used: one a corner, and one where the outline crosses each
  // edge between corners, those running along x and then those along z.
  const corners = new Int32Array(side * side).fill(-1)
  const crossings = new Int32Array(2 * side * side).fill(-1)
  const corner = (i: number, j: number) => (corners[j * side + i] = corners[j * side + i]! >= 0 ? corners[j * side + i]! : vertex(x0 + place(i), z0 + place(j), LID_HEIGHT))
  const crossing = (i: number, j: number, along: 0 | 1) => {
    const key = along * side * side + j * side + i
    if (crossings[key]! >= 0) return crossings[key]!
    const a = sown(i, j), b = along === 0 ? sown(i + 1, j) : sown(i, j + 1)
    const t = (0.5 - a) / (b - a)
    const x = along === 0 ? place(i) + (place(i + 1) - place(i)) * t : place(i)
    const z = along === 0 ? place(j) : place(j) + (place(j + 1) - place(j)) * t
    return (crossings[key] = vertex(x0 + x, z0 + z, LID_HEIGHT))
  }
  // A skirt under the top's edge from vertex `a` to vertex `b`: their feet, on the ground.
  const skirt = (a: number, b: number) => {
    const foot = (v: number) => vertex(positions[v * 3]!, positions[v * 3 + 2]!, 0)
    const fa = foot(a), fb = foot(b)
    indices.push(a, b, fb, a, fb, fa)
  }
  const RING = [[0, 0], [1, 0], [1, 1], [0, 1]] as const
  const outline: number[] = []
  for (let j = 0; j + 1 < side; j++) {
    for (let i = 0; i + 1 < side; i++) {
      // Round the square: each corner that is sown, and each crossing of the outline between two that differ.
      outline.length = 0
      const cut: boolean[] = []
      for (let k = 0; k < 4; k++) {
        const [ai, aj] = [i + RING[k]![0], j + RING[k]![1]]
        const [bi, bj] = [i + RING[(k + 1) % 4]![0], j + RING[(k + 1) % 4]![1]]
        const inA = sown(ai, aj) >= 0.5, inB = sown(bi, bj) >= 0.5
        if (inA) {
          outline.push(corner(ai, aj))
          cut.push(false)
        }
        if (inA !== inB) {
          outline.push(aj === bj ? crossing(Math.min(ai, bi), aj, 0) : crossing(ai, Math.min(aj, bj), 1))
          cut.push(true)
        }
      }
      if (outline.length < 3) continue
      for (let k = 1; k + 1 < outline.length; k++) indices.push(outline[0]!, outline[k]!, outline[k + 1]!)
      // Between two crossings the square's edge is the field's.
      for (let k = 0; k < outline.length; k++) {
        const next = (k + 1) % outline.length
        if (cut[k] && cut[next]) skirt(outline[k]!, outline[next]!)
      }
    }
  }
  if (indices.length === 0 || missing) return null
  // Round the tile's edge, where the top reaches it.
  for (let k = 0; k + 1 < side; k++) {
    for (const [ai, aj, bi, bj] of [[k, 0, k + 1, 0], [k, side - 1, k + 1, side - 1], [0, k, 0, k + 1], [side - 1, k, side - 1, k + 1]] as const) {
      const a = corners[aj * side + ai]!, b = corners[bj * side + bi]!
      if (a >= 0 && b >= 0 && sown(ai, aj) >= 0.5 && sown(bi, bj) >= 0.5) skirt(a, b)
    }
  }
  // Lit as the ground it stands on is, skirts and all: the field is one colour to its edge.
  const normals = new Float32Array(positions.length)
  for (let v = 1; v < normals.length; v += 3) normals[v] = 1
  const geometry = new Geometry({ positions: new Float32Array(positions), normals, custom: new Float32Array(custom), indices: new Uint32Array(indices) })
  // The shader lifts it: grow the bounds so the tile's lid is not culled while its top still shows.
  geometry.bounds = { ...geometry.bounds, radius: geometry.bounds.radius + LID_HEIGHT }
  return geometry
}

/**
 * Lifts the lid out of the ground with distance from the camera: `band` is where it starts to rise
 * and where it is up, metres. A camera over the field counts as further off by `LID_PER_HEIGHT` for
 * each metre it is above `LID_EYE`: stalks seen from above are thin lines with the ground between
 * them, so from a helicopter the lid is up under them and only their ears stand out of it.
 */
const LID_VERTEX_WGSL = /* wgsl */ `
fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let over = max(frame.cameraPos.y - world.y - ${LID_EYE.toFixed(1)}, 0.0) * ${LID_PER_HEIGHT.toFixed(1)};
  let up = smoothstep(params.band.x, params.band.y, distance(world.xz, frame.cameraPos.xz) + over);
  return vec3f(world.x, world.y + custom.x * up - ${LID_SUNK.toFixed(2)} * (1.0 - up), world.z);
}
`

/**
 * Lit as the ground is and in its shadows, in the colour the stalks have from afar (`tint`, `LID_COLOR`).
 * It is not one flat tone: clumps of
 * ears a metre or so across (which blur out with distance, before they would shimmer), broader
 * patches of thicker and thinner crop, and the wind's gusts crossing it as they cross the stalks.
 */
const LID_WGSL = /* wgsl */ `
${GROUND_LIGHT_WGSL}

fn lidHash(p: vec2f) -> f32 {
  return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

fn lidNoise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(lidHash(i), lidHash(i + vec2f(1.0, 0.0)), u.x), mix(lidHash(i + vec2f(0.0, 1.0)), lidHash(i + vec2f(1.0, 1.0)), u.x), u.y);
}

fn surface(s: Surface) -> vec4f {
  let ground = vec3f(0.0, 1.0, 0.0);
  let lit = groundLight(s.world, ground, sunShadowTap(s.world, ground));
  let at = s.world.xz;
  let clumps = (lidNoise(at * 1.9) - 0.5) * (1.0 - smoothstep(60.0, 160.0, s.viewDepth));
  let patches = lidNoise(at * 0.13) - 0.5;
  // The gust the stalks lean with (grassLean): the ears it bends catch the light.
  let gust = sin(dot(at, params.wind) * 0.09 - frame.fog.w * 1.3) * 0.5 + 0.5;
  let c = params.tint.rgb * (1.0 + clumps * 0.22 + patches * 0.12 + gust * 0.06);
  return vec4f(c / PI * lit, 1.0);
}
`

/**
 * The height of a tile's own `ground` at `(x, z)`, between its samples. A lid is laid on its tile's
 * ground alone: the tiles round it may not have streamed in yet, and its far edge is theirs.
 */
function groundAt(ground: TileGround, x: number, z: number): number {
  const { x0, z0, cell, side, heights } = ground
  const u = Math.min(side - 1, Math.max(0, (x - x0) / cell)), v = Math.min(side - 1, Math.max(0, (z - z0) / cell))
  const i = Math.min(side - 2, Math.floor(u)), j = Math.min(side - 2, Math.floor(v))
  const fu = u - i, fv = v - j
  const at = (di: number, dj: number) => heights[(j + dj) * side + i + di]!
  return (at(0, 0) * (1 - fu) + at(1, 0) * fu) * (1 - fv) + (at(0, 1) * (1 - fu) + at(1, 1) * fu) * fv
}

/** The lids of the streamed-in tiles' fields. */
export class CropLid {
  readonly root = new Group()
  private readonly material: ShaderMaterial
  private readonly tiles = new Map<number, { mesh: Mesh | null; crops: Uint8Array; ground: TileGround | null }>()
  /** Craters shells dug, which the lid is open over. Keyed by `x,z,radius` so telling of one twice adds nothing. */
  private readonly craters = new CraterGrid()
  /** The tiles a crater reached since their lids were laid, and whether laying them again is on its way. */
  private readonly stale = new Set<number>()
  private pending = false

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    density: GrassDensity = WHEAT_DENSITY,
  ) {
    this.root.name = 'crops lid'
    const color = Color.hex(LID_COLOR)
    const tint: [number, number, number, number] = [color.r, color.g, color.b, 0]
    this.material = new ShaderMaterial({ wgsl: LID_WGSL, vertex: LID_VERTEX_WGSL, uniforms: { band: [density.far * LID_RISE_FROM, density.far * LID_RISE_TO], tint, wind: [Math.cos(WIND_HEADING), Math.sin(WIND_HEADING)] }, side: 'double', shadows: true })
  }

  private key(ref: Pick<TileRef, 'col' | 'row'>) {
    return ref.row * 65536 + ref.col
  }

  /** Lays the lid over a streamed-in tile's crops on its `ground` (null in a flat world), in place of any it had. */
  addTile(ref: Pick<TileRef, 'col' | 'row'>, tile: Pick<TileData, 'crops'>, ground: TileGround | null) {
    this.removeTile(ref)
    if (tile.crops) this.tiles.set(this.key(ref), { mesh: this.build(ref, tile.crops, ground), crops: tile.crops, ground })
  }

  /** The ground of a tile moved (a crater was dug in it, in place): its lid is laid again on it. */
  regrowTile(ref: Pick<TileRef, 'col' | 'row'>) {
    const tile = this.tiles.get(this.key(ref))
    if (!tile) return
    this.drop(tile.mesh)
    tile.mesh = this.build(ref, tile.crops, tile.ground)
  }

  /**
   * A shell dug a crater of `radius` metres at `(x, z)`: the lids it reaches are laid again, open over
   * its bowl, so that from far off the crater shows in the field as it does among the stalks.
   */
  dent(x: number, z: number, radius: number) {
    if (!this.craters.add(x, z, radius)) return
    const tile = (v: number) => Math.floor((v + this.halfExtent) / this.tileSize)
    for (let row = tile(z - radius); row <= tile(z + radius); row++) {
      for (let col = tile(x - radius); col <= tile(x + radius); col++) if (this.tiles.has(this.key({ col, row }))) this.stale.add(this.key({ col, row }))
    }
    // A barrage, or the craters told of on joining, come many at once: each tile is laid again once.
    if (this.stale.size > 0 && !this.pending) {
      this.pending = true
      queueMicrotask(() => {
        this.pending = false
        for (const key of this.stale) this.regrowTile({ col: key % 65536, row: Math.floor(key / 65536) })
        this.stale.clear()
      })
    }
  }

  /** The ground of the crater `dent` was told of has healed: the lids it reached are laid whole again with their tiles (`regrowTile`). */
  heal(x: number, z: number, radius: number) {
    this.craters.remove(x, z, radius)
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    const tile = this.tiles.get(this.key(ref))
    if (!tile) return
    this.drop(tile.mesh)
    this.tiles.delete(this.key(ref))
  }

  private build(ref: Pick<TileRef, 'col' | 'row'>, crops: Uint8Array, ground: TileGround | null): Mesh | null {
    const x0 = -this.halfExtent + ref.col * this.tileSize, z0 = -this.halfExtent + ref.row * this.tileSize
    const bare = this.craters.within(x0, z0, x0 + this.tileSize, z0 + this.tileSize)
    const geometry = lidGeometry(crops, x0, z0, this.tileSize, (x, z) => (ground ? groundAt(ground, x, z) : 0), bare)
    if (!geometry) return null
    // Nothing reads it again: a lid whose ground changes is built anew.
    geometry.uploadOnce = true
    const mesh = new Mesh(geometry, this.material)
    this.root.add(mesh)
    return mesh
  }

  private drop(mesh: Mesh | null) {
    if (!mesh) return
    this.root.remove(mesh)
    mesh.geometry.dispose()
  }

  /** A winter field is stubble, which has no lid. */
  /** Which way the wind blows, a unit `[x, z]`: its gusts cross the lid as they cross the stalks. */
  blowing(direction: readonly [number, number]) {
    const wind = this.material.uniforms.wind as number[]
    wind[0] = direction[0]
    wind[1] = direction[1]
    this.material.needsUpdate()
  }

  setEnabled(on: boolean) {
    this.root.visible = on
  }

  dispose() {
    for (const tile of this.tiles.values()) this.drop(tile.mesh)
    this.tiles.clear()
    this.material.dispose()
  }
}
