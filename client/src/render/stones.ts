import { Group, InstancedMesh, ShaderMaterial, Texture, mat4, type Geometry } from '@ragnarok/engine'
import { STONES_ANGULAR, STONES_DENSITY, type TileData, type TileRef } from '@/world'
import type { TileGround } from '@/sim/terrain'
import { carveRock, rockWgsl, type Carving } from './rocks'
import { shoreLevelAt, shoreOf, WET_HEIGHT, type Shore } from './shore'

/*
 * Stony ground. The tile's `stones` grid goes to the terrain's shader as a texture, and the shader lays
 * cobbles and a boulder field's blocks over the ground, out to the horizon
 * (`engine/src/materials/stones.wgsl.ts`). Near the eye they stand up off it: `Stones` puts a small
 * rock on each of the shader's stones within `STONES_REACH`, flattening them toward its edge onto the
 * painted stones under them, which have the same place, size and tone: a cobble is a hand high, so it
 * swells out of its picture unseen as you come. The painted stone under each one is its foot and its shadow.
 */

/**
 * The tile's stony ground and shore as a texture for the terrain's shader: how stony in red, a boulder
 * field's blocks in green, and in blue how far above the water near it the ground is (0 to 1 m, all of
 * it where it is higher or there is none), for the wet band along the shore. Null when the tile has neither.
 */
export function stonesTexture(stones: Uint8Array | undefined, shore: Shore | null, ground: TileGround): Texture | null {
  if (!stones && !shore) return null
  const n = stones ? Math.round(Math.sqrt(stones.length)) : (shore?.n ?? 128)
  const { x0, z0, cell, side, heights } = ground
  const size = cell * (side - 1)
  const data = new Uint8Array(n * n * 4)
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const k = j * n + i
      const value = stones?.[k] ?? 0
      data[k * 4] = Math.round(((value & STONES_DENSITY) / STONES_DENSITY) * 255)
      data[k * 4 + 1] = value & STONES_ANGULAR ? 255 : 0
      let above = 1
      if (shore) {
        const x = x0 + ((i + 0.5) / n) * size, z = z0 + ((j + 0.5) / n) * size
        const level = shoreLevelAt(shore, x, z)
        const ground = heights[Math.min(side - 1, Math.round((z - z0) / cell)) * side + Math.min(side - 1, Math.round((x - x0) / cell))]!
        if (!Number.isNaN(level)) above = Math.min(1, Math.max(0, ground - level))
      }
      data[k * 4 + 2] = Math.round(above * 255)
      data[k * 4 + 3] = 255
    }
  }
  return new Texture({ data, width: n, height: n }, { srgb: false, wrap: 'clamp' })
}

/** The shader's cells, metres across (`STONE_COBBLE`, `STONE_BLOCK` in stones.wgsl). */
const STONE_COBBLE = 0.34
const STONE_BLOCK = 0.85
/** The shader's `STONES_SHOWN`: how much of the ground is under stones, for each grade of stony ground. */
const STONES_SHOWN = 1.6

/** The shader's `stonesHash`, 0..1: the same stones in the same cells. */
export function stonesHash(ix: number, iz: number, salt: number): number {
  let h = (Math.imul(ix, 0x9e3779b1) ^ Math.imul(iz, 0x85ebca77) ^ Math.imul(salt, 0xc2b2ae3d)) >>> 0
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d)
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39)
  h = (h ^ (h >>> 15)) >>> 0
  return (h >>> 8) / 16777216
}

/** One stone of the carpet: where it stands, how big, and its shape and tone. */
export interface Stone {
  x: number
  z: number
  radius: number
  height: number
  angular: boolean
  /** 0..1, picks the shape. */
  shape: number
  turn: number
  tone: number
  warm: boolean
}

/**
 * The stones whose cells' middles lie in `[x0, x0 + size) × [z0, z0 + size)`, as the shader lays them:
 * a cell holds one where its hash is under the ground's stoniness there, a little inside the cell so the
 * painted stone shows round its foot. `stonyAt` is `[density 0..1, angular]` at a point.
 */
export function stonesIn(x0: number, z0: number, size: number, stonyAt: (x: number, z: number) => [number, boolean]): Stone[] {
  const out: Stone[] = []
  for (const angular of [false, true]) {
    const cell = angular ? STONE_BLOCK : STONE_COBBLE
    const i0 = Math.floor(x0 / cell) - 1, i1 = Math.ceil((x0 + size) / cell)
    const j0 = Math.floor(z0 / cell) - 1, j1 = Math.ceil((z0 + size) / cell)
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        const x = (i + 0.1 + 0.8 * stonesHash(i, j, 1)) * cell
        const z = (j + 0.1 + 0.8 * stonesHash(i, j, 2)) * cell
        if (x < x0 || x >= x0 + size || z < z0 || z >= z0 + size) continue
        const [stony, blocks] = stonyAt(x, z)
        const density = Math.min(1, stony * STONES_SHOWN)
        if (density <= 0 || blocks !== angular || stonesHash(i, j, 3) >= density * 1.25) continue
        const gap = 0.2 + (0.03 - 0.2) * density
        const radius = cell * (0.5 - gap) * (1.0 + 0.25 * stonesHash(i, j, 8))
        if (radius < 0.05) continue
        out.push({
          x,
          z,
          radius,
          height: radius * (angular ? 0.8 + 0.5 * stonesHash(i, j, 9) : 0.55 + 0.35 * stonesHash(i, j, 9)),
          angular,
          shape: stonesHash(i, j, 10),
          turn: stonesHash(i, j, 11) * Math.PI * 2,
          tone: stonesHash(i, j, 4),
          warm: stonesHash(i, j, 5) >= 0.82,
        })
      }
    }
  }
  return out
}

/**
 * Metres from the eye the carpet reaches; past `STONES_FADE` the stones flatten onto the painted ones under them. The
 * terrain's shader darkens its painted stones to their shadows inside the same distances.
 */
export const STONES_REACH = 16
const STONES_FADE = 7
/** Metres a side of the chunks the carpet is built in, and how many it builds a frame. */
const CHUNK = 16
const BUILDS_PER_FRAME = 2

/** The shader's stone colours (linear), for the stones that stand on them. */
const STONE_DARK = [0.12, 0.118, 0.11]
const STONE_LIGHT = [0.4, 0.39, 0.37]
const STONE_WARM = [0.38, 0.3, 0.25]

const COBBLES: Carving[] = [
  { cuts: 2, depth: [0.75, 0.9], squash: 0.7, bump: 0.05 },
  { cuts: 3, depth: [0.65, 0.8], squash: 0.6, bump: 0.04 },
]
const BLOCKS: Carving[] = [
  { cuts: 6, depth: [0.4, 0.65], top: 0.5, squash: 0.8, bump: 0.02 },
  { cuts: 7, depth: [0.45, 0.7], squash: 0.85, bump: 0.02 },
]

/**
 * Flattens the stones toward the carpet's edge, onto the painted ones under them, then a little under
 * the ground. The colour's alpha is how high over the stone's foot a vertex stands, metres: the
 * vertex's share of the height (`carpetShape`) times the stone's height (the instance's alpha).
 */
const FLATTEN_WGSL = /* wgsl */ `
fn displace(world: vec3f, custom: vec4f, color: vec4f) -> vec3f {
  let away = smoothstep(${STONES_FADE.toFixed(1)}, ${STONES_REACH.toFixed(1)}, distance(world.xz, frame.cameraPos.xz));
  return world - vec3f(0.0, away * (color.a + 0.03), 0.0);
}
`

/** A carpet stone's shape: as carved, its vertex colours' alpha its height over its foot, 0 to 1. */
function carpetShape(carving: Carving, seed: number): Geometry {
  const geometry = carveRock(carving, seed, 1)
  const rgb = geometry.colors!
  const colors = new Float32Array(geometry.vertexCount * 4)
  for (let i = 0; i < geometry.vertexCount; i++) {
    colors.set([rgb[i * 3]!, rgb[i * 3 + 1]!, rgb[i * 3 + 2]!, Math.max(0, geometry.positions[i * 3 + 1]!)], i * 4)
  }
  geometry.colors = colors
  return geometry
}

let shared: { cobbles: Geometry[]; blocks: Geometry[]; material: ShaderMaterial } | undefined

function prototypes() {
  // Coarser than the boulders: a stone is a few pixels high, and there are thousands.
  shared ??= {
    cobbles: COBBLES.map((carving, i) => carpetShape(carving, 41 + i * 17)),
    blocks: BLOCKS.map((carving, i) => carpetShape(carving, 77 + i * 17)),
    material: new ShaderMaterial({ wgsl: rockWgsl(true), vertex: FLATTEN_WGSL, vertexColors: true, shadows: true }),
  }
  return shared
}

interface Chunk {
  meshes: InstancedMesh[]
  stale: boolean
}

/** The carpet of stones near the eye, over every streamed-in tile's stony ground. */
export class Stones {
  readonly root = new Group()
  private readonly tiles = new Map<number, Uint8Array>()
  /** The shore of each stony tile that has water, for the wet stones along it. */
  private readonly shores = new Map<number, Shore>()
  private readonly chunks = new Map<number, Chunk>()
  private readonly craters: { x: number; z: number; radius: number }[] = []

  constructor(
    private readonly halfExtent: number,
    private readonly tileSize: number,
    private readonly heightAt: (x: number, z: number) => number | null,
  ) {}

  private tileKey(col: number, row: number) {
    return row * 65536 + col
  }

  addTile(ref: Pick<TileRef, 'col' | 'row'>, tile: Pick<TileData, 'stones' | 'water'>, ground: TileGround | null) {
    const key = this.tileKey(ref.col, ref.row)
    if (tile.stones) this.tiles.set(key, tile.stones)
    else this.tiles.delete(key)
    const shore = tile.stones && ground ? shoreOf(ground, tile.water) : null
    if (shore) this.shores.set(key, shore)
    else this.shores.delete(key)
    this.dropOver(ref)
  }

  removeTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.tiles.delete(this.tileKey(ref.col, ref.row))
    this.shores.delete(this.tileKey(ref.col, ref.row))
    this.dropOver(ref)
  }

  /** The ground of a tile moved: its stones are laid again on it. */
  regrowTile(ref: Pick<TileRef, 'col' | 'row'>) {
    this.dropOver(ref)
  }

  /** A shell dug a crater: the stones in its bowl are gone. */
  dent(x: number, z: number, radius: number) {
    this.craters.push({ x, z, radius })
    for (const [key, chunk] of this.chunks) {
      const [cx, cz] = this.chunkAt(key)
      if (x + radius >= cx * CHUNK && x - radius <= (cx + 1) * CHUNK && z + radius >= cz * CHUNK && z - radius <= (cz + 1) * CHUNK) chunk.stale = true
    }
  }

  /** The level of the water near `(x, z)`, NaN where there is none. */
  private waterAt(x: number, z: number): number {
    const shore = this.shores.get(this.tileKey(Math.floor((x + this.halfExtent) / this.tileSize), Math.floor((z + this.halfExtent) / this.tileSize)))
    return shore ? shoreLevelAt(shore, x, z) : NaN
  }

  /** `[how stony 0..1, a boulder field's blocks]` at `(x, z)`. */
  stonyAt(x: number, z: number): [number, boolean] {
    const col = Math.floor((x + this.halfExtent) / this.tileSize), row = Math.floor((z + this.halfExtent) / this.tileSize)
    const stones = this.tiles.get(this.tileKey(col, row))
    if (!stones) return [0, false]
    const n = Math.round(Math.sqrt(stones.length))
    const i = Math.min(n - 1, Math.floor(((x + this.halfExtent) / this.tileSize - col) * n))
    const j = Math.min(n - 1, Math.floor(((z + this.halfExtent) / this.tileSize - row) * n))
    const value = stones[j * n + i]!
    return [(value & STONES_DENSITY) / STONES_DENSITY, (value & STONES_ANGULAR) !== 0]
  }

  /** Lays the stones about the eye, a few chunks a frame, and takes up those it has left behind. */
  update(eyeX: number, eyeZ: number) {
    const reach = STONES_REACH + CHUNK
    for (const [key, chunk] of this.chunks) {
      const [cx, cz] = this.chunkAt(key)
      if (chunk.stale || Math.hypot((cx + 0.5) * CHUNK - eyeX, (cz + 0.5) * CHUNK - eyeZ) > reach + CHUNK) this.drop(key, chunk)
    }
    if (this.tiles.size === 0) return
    let built = 0
    const near: [number, number, number][] = []
    for (let cz = Math.floor((eyeZ - reach) / CHUNK); cz <= Math.floor((eyeZ + reach) / CHUNK); cz++) {
      for (let cx = Math.floor((eyeX - reach) / CHUNK); cx <= Math.floor((eyeX + reach) / CHUNK); cx++) {
        // The chunk's nearest point to the eye, within reach.
        const dx = Math.max(cx * CHUNK - eyeX, 0, eyeX - (cx + 1) * CHUNK), dz = Math.max(cz * CHUNK - eyeZ, 0, eyeZ - (cz + 1) * CHUNK)
        const far = Math.hypot(dx, dz)
        if (far <= STONES_REACH && !this.chunks.has(this.chunkKey(cx, cz))) near.push([far, cx, cz])
      }
    }
    near.sort((a, b) => a[0] - b[0])
    for (const [, cx, cz] of near) {
      if (built++ >= BUILDS_PER_FRAME) break
      this.build(cx, cz)
    }
  }

  dispose() {
    for (const [key, chunk] of this.chunks) this.drop(key, chunk)
    this.root.removeFromParent()
  }

  private chunkKey(cx: number, cz: number) {
    return (cz + 32768) * 65536 + (cx + 32768)
  }

  private chunkAt(key: number): [number, number] {
    return [(key % 65536) - 32768, Math.floor(key / 65536) - 32768]
  }

  private dropOver(ref: Pick<TileRef, 'col' | 'row'>) {
    const x0 = -this.halfExtent + ref.col * this.tileSize, z0 = -this.halfExtent + ref.row * this.tileSize
    for (const [key, chunk] of this.chunks) {
      const [cx, cz] = this.chunkAt(key)
      if ((cx + 1) * CHUNK < x0 || cx * CHUNK > x0 + this.tileSize || (cz + 1) * CHUNK < z0 || cz * CHUNK > z0 + this.tileSize) continue
      chunk.stale = true
    }
  }

  private drop(key: number, chunk: Chunk) {
    for (const mesh of chunk.meshes) this.root.remove(mesh)
    this.chunks.delete(key)
  }

  private build(cx: number, cz: number) {
    const { cobbles, blocks, material } = prototypes()
    const stones = stonesIn(cx * CHUNK, cz * CHUNK, CHUNK, (x, z) => this.stonyAt(x, z))
    const byShape = new Map<Geometry, { stone: Stone; y: number }[]>()
    for (const stone of stones) {
      if (this.craters.some((c) => Math.hypot(stone.x - c.x, stone.z - c.z) < c.radius)) continue
      const y = this.heightAt(stone.x, stone.z)
      if (y === null) continue
      const shapes = stone.angular ? blocks : cobbles
      const shape = shapes[Math.floor(stone.shape * shapes.length)]!
      const list = byShape.get(shape) ?? []
      list.push({ stone, y })
      byShape.set(shape, list)
    }
    const matrix = mat4.create()
    const meshes: InstancedMesh[] = []
    for (const [shape, list] of byShape) {
      const mesh = new InstancedMesh(shape, material, list.length)
      list.forEach(({ stone, y }, i) => {
        const half = stone.turn / 2
        mesh.setMatrixAt(i, mat4.compose(matrix, stone.x, y, stone.z, 0, Math.sin(half), 0, Math.cos(half), stone.radius, stone.height, stone.radius * (0.8 + 0.4 * stone.shape)))
        const t = 0.3 + 0.55 * stone.tone
        let [r, g, b] = STONE_DARK.map((dark, c) => dark + (STONE_LIGHT[c]! - dark) * t) as [number, number, number]
        if (stone.warm) [r, g, b] = [r, g, b].map((v, c) => v + (STONE_WARM[c]! - v) * 0.7) as [number, number, number]
        // Wet along the water, as the ground and the rocks are there: a cobble is all wet or all dry.
        const water = this.waterAt(stone.x, stone.z)
        if (y < water + WET_HEIGHT + (stone.shape - 0.5) * 0.1) [r, g, b] = [r * 0.5, g * 0.5, b * 0.5]
        // The alpha is the stone's height, for the vertex code to flatten it by.
        mesh.setColorAt(i, r, g, b, stone.height)
      })
      mesh.needsUpdate()
      mesh.castShadow = false
      mesh.receiveShadow = true
      mesh.userData.sharedGeometry = true
      this.root.add(mesh)
      meshes.push(mesh)
    }
    this.chunks.set(this.chunkKey(cx, cz), { meshes, stale: false })
  }
}
