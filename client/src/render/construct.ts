import { Group, Material, Mesh, boxGeometry, Color } from '@ragnarok/engine'
import { CELL, keyedFaces } from '@/sim/carve'
import { CHUNK, MATERIALS, MATERIAL_BRICK, cubeCentre, type Construct, type Cube, type Pos } from '@/sim/construct'
import { PLAYER_COLORS } from '@/profile'
import type { Effects } from './effects'
import { toonMaterial } from './toon'
import { flatGeometry } from './world'

/**
 * The cubes players placed with bricks (`sim/construct.ts`), drawn a chunk to a mesh: the faces of
 * each chunk's cubes merged into rectangles of one colour (`keyedFaces`), with a colour per vertex,
 * so placing a cube redraws one small chunk. Cubes that fell come down as one piece, land in a puff
 * of dust and lie a moment; the ghost cube shows where bricks would put the next one.
 */

/** Every chunk shares this; each vertex carries its builder's colour. */
const material = toonMaterial({ vertexColors: true })
/** The ghost: see-through, so what is behind it still shows. */
const ghostMaterial: Material = new Material({ color: 0xffffff, lit: false, blend: 'alpha', opacity: 0.35, depthWrite: false })
/** The ghost where a cube would be refused: nothing to rest on, or something in the way. */
const refusedMaterial: Material = new Material({ color: 0x8a8a8a, lit: false, blend: 'alpha', opacity: 0.2, depthWrite: false })
const digMaterial: Material = new Material({ color: 0xd93a3a, lit: false, blend: 'alpha', opacity: 0.45, depthWrite: false })

/** Seconds a fallen piece lies where it landed before it goes. */
const FALLEN_SECONDS = 4
const GRAVITY = 9.8
/** A fallen piece bigger than this many cells of its bounding box is not drawn falling: it just goes. */
const MOST_FALLING_CELLS = 1 << 20

/**
 * How each material looks: its cubes in the builder's colour, mixed towards `mix` by `amount`. A
 * material the server adds needs a row here (checked when this module loads).
 */
const LOOKS: Record<number, { mix: number; amount: number }> = {
  [MATERIAL_BRICK]: { mix: 0x000000, amount: 0 },
}
for (const m of MATERIALS) if (!LOOKS[m.id]) throw new Error(`material ${m.id} (${m.name}) has no look in render/construct.ts`)

/** How dark the cubes this player's rounds wore down get, a step per quarter of their health. */
const WEAR_STEPS = 3
const WEAR_DARKEN = 0.18

/** Set on the key of a cube outside the chunk being drawn, which only hides the faces against it. */
const BORDER = 1 << 24

/** The key a cube is meshed by: cubes of one key merge into one rectangle. */
const keyOf = (cube: Cube, wear: number) => (cube.colour & 255) | ((cube.material & 255) << 8) | (Math.min(WEAR_STEPS, Math.ceil(wear * WEAR_STEPS)) << 16)

/** The colour of cubes of `key`: the builder's outfit colour, as its material looks, darkened by wear. */
function tint(key: number) {
  const look = LOOKS[(key >> 8) & 255] ?? LOOKS[MATERIAL_BRICK]!
  const c = Color.hex(PLAYER_COLORS[(key & 255) % PLAYER_COLORS.length]!.hex).lerp(Color.hex(look.mix), look.amount)
  return c.multiplyScalar(1 - WEAR_DARKEN * ((key >> 16) & 255))
}

/** Triangle soup of a grid of keyed cells (`keyedFaces`, keys from `keyOf`), metres, less `offset`; faces of `BORDER` cells are left out. */
function soup(origin: readonly number[], dims: readonly number[], keys: Int32Array, offset: readonly number[] = [0, 0, 0]) {
  const faces = keyedFaces(origin, dims, keys)
  const positions: number[] = []
  const colors: number[] = []
  const colours = new Map<number, Color>()
  for (let t = 0; t < faces.length; t += 10) {
    const key = faces[t + 9]!
    if (key >= BORDER) continue
    let c = colours.get(key)
    if (!c) colours.set(key, (c = tint(key)))
    for (let k = 0; k < 9; k += 3) {
      positions.push(faces[t + k]! / 100 - offset[0]!, faces[t + k + 1]! / 100 - offset[1]!, faces[t + k + 2]! / 100 - offset[2]!)
      colors.push(c.r, c.g, c.b)
    }
  }
  return { positions, colors }
}

interface Fallen {
  mesh: Mesh
  vy: number
  /** Height the piece's bottom stops at. */
  floor: number
  age: number
  resting: boolean
}

export class ConstructView {
  readonly root = new Group()
  private readonly meshes = new Map<string, Mesh>()
  private readonly fallen: Fallen[] = []
  private readonly ghost = new Mesh(boxGeometry(1, 1, 1), ghostMaterial)
  private readonly dig = new Mesh(boxGeometry(1, 1, 1), digMaterial)

  constructor() {
    const size = (CELL / 100) * 1.02
    for (const mesh of [this.ghost, this.dig]) {
      mesh.scale.set(size, size, size)
      mesh.visible = false
      this.root.add(mesh)
    }
  }

  /**
   * Redraws chunk `key` as `construct` has it now; an empty one is no longer drawn. The cubes just
   * outside it are looked at too, so a face against a cube of the next chunk is left out, as one
   * inside a chunk is.
   */
  redraw(construct: Construct, key: string) {
    const old = this.meshes.get(key)
    if (old) {
      old.geometry.dispose()
      old.removeFromParent()
      this.meshes.delete(key)
    }
    const data = construct.chunk(key)
    if (!data || data.count === 0) return
    const n = CHUNK + 2
    const base = [data.chunk[0] * CHUNK - 1, data.chunk[1] * CHUNK - 1, data.chunk[2] * CHUNK - 1] as const
    const keys = new Int32Array(n * n * n).fill(-1)
    for (let y = 0; y < n; y++) {
      for (let z = 0; z < n; z++) {
        for (let x = 0; x < n; x++) {
          const pos: Pos = [base[0] + x, base[1] + y, base[2] + z]
          const inside = x > 0 && y > 0 && z > 0 && x <= CHUNK && y <= CHUNK && z <= CHUNK
          let cube: Cube | undefined
          if (inside) {
            const at = ((y - 1) * CHUNK + (z - 1)) * CHUNK + (x - 1)
            if (data.cells[at] !== 0) cube = { colour: data.cells[at]! - 1, material: data.materials[at]! }
          } else {
            cube = construct.get(pos)
          }
          if (cube) keys[(y * n + z) * n + x] = inside ? keyOf(cube, construct.wear(pos)) : BORDER
        }
      }
    }
    const { positions, colors } = soup([base[0] * CELL, base[1] * CELL, base[2] * CELL], [n, n, n], keys)
    if (positions.length === 0) return
    const mesh = new Mesh(flatGeometry(positions, colors), material)
    mesh.castShadow = true
    mesh.receiveShadow = true
    this.root.add(mesh)
    this.meshes.set(key, mesh)
  }

  /**
   * Cubes `cubes` (with their colours) lost their hold: they come down together as one piece, to
   * `groundAt` under their middle, and lie there a moment.
   */
  drop(cubes: readonly { pos: Pos; cube: Cube }[], groundAt: (x: number, z: number) => number) {
    if (cubes.length === 0) return
    const lo = [Infinity, Infinity, Infinity]
    const hi = [-Infinity, -Infinity, -Infinity]
    for (const { pos } of cubes) {
      for (let a = 0; a < 3; a++) {
        lo[a] = Math.min(lo[a]!, pos[a]!)
        hi[a] = Math.max(hi[a]!, pos[a]!)
      }
    }
    const dims = [hi[0]! - lo[0]! + 1, hi[1]! - lo[1]! + 1, hi[2]! - lo[2]! + 1]
    if (dims[0]! * dims[1]! * dims[2]! > MOST_FALLING_CELLS) return
    const keys = new Int32Array(dims[0]! * dims[1]! * dims[2]!).fill(-1)
    for (const { pos, cube } of cubes) keys[((pos[1] - lo[1]!) * dims[2]! + (pos[2] - lo[2]!)) * dims[0]! + (pos[0] - lo[0]!)] = keyOf(cube, 0)
    const origin = [lo[0]! * CELL, lo[1]! * CELL, lo[2]! * CELL]
    // Drawn about its own corner, so it moves as one.
    const corner = origin.map((v) => v / 100)
    const { positions, colors } = soup(origin, dims, keys, corner)
    const mesh = new Mesh(flatGeometry(positions, colors), material)
    mesh.castShadow = true
    mesh.position.set(corner[0]!, corner[1]!, corner[2]!)
    this.root.add(mesh)
    const middle = cubeCentre([Math.round((lo[0]! + hi[0]!) / 2), lo[1]!, Math.round((lo[2]! + hi[2]!) / 2)])
    this.fallen.push({ mesh, vy: 0, floor: groundAt(middle[0], middle[2]), age: 0, resting: false })
  }

  /**
   * Where bricks would put a cube (`place`) or dig one out (`dig`), or neither. `allowed` is whether
   * the cube would be placed (`mayPlace`): a refused one shows faint grey.
   */
  aim(place: Pos | null, dig: Pos | null, allowed = true) {
    this.ghost.visible = place !== null
    this.ghost.material = allowed ? ghostMaterial : refusedMaterial
    if (place) this.ghost.position.set(...cubeCentre(place))
    this.dig.visible = dig !== null
    if (dig) this.dig.position.set(...cubeCentre(dig))
  }

  /** Brings the fallen pieces down, dusts them where they land, and clears them away after a while. */
  update(dt: number, effects: Effects) {
    for (let i = this.fallen.length - 1; i >= 0; i--) {
      const piece = this.fallen[i]!
      if (piece.resting) {
        piece.age += dt
        if (piece.age > FALLEN_SECONDS) {
          piece.mesh.geometry.dispose()
          piece.mesh.removeFromParent()
          this.fallen.splice(i, 1)
        }
        continue
      }
      piece.vy -= GRAVITY * dt
      piece.mesh.position.y += piece.vy * dt
      if (piece.mesh.position.y <= piece.floor) {
        piece.mesh.position.y = piece.floor
        piece.resting = true
        const at = piece.mesh.position
        effects.dust({ x: at.x, y: at.y, z: at.z }, 2.5)
      }
    }
  }

  dispose() {
    for (const mesh of this.meshes.values()) mesh.geometry.dispose()
    for (const piece of this.fallen) piece.mesh.geometry.dispose()
    this.meshes.clear()
    this.fallen.length = 0
    this.root.remove(...this.root.children)
  }
}
