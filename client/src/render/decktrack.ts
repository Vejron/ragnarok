import type { Group, Mesh } from '@ragnarok/engine'
import { CELL, shellOf, type Shell } from '@/sim/carve'
import type { Rail, RailDeck, TileData } from '@/world'
import { deckRibbons } from './rails'
import { deckTrackMeshes } from './world'

/*
 * The track on railway bridges (`Rail.deck`): sleepers and rails at the deck's height, over the
 * ballast that is the deck's own roof. Like a window's glass it follows the holes blasts make in the
 * section under it (`holed`): a sleeper's length of track goes with the deck's top under any of it,
 * so none is left hanging over a gap.
 */

/**
 * How far from the cell at a track's height the deck's top is looked for, cells, the nearest first:
 * the track eases onto the deck at its ends, and a wide deck's edge may be a little under it.
 */
const TOP_CELLS = [0, -1, 1]
/** A point this far under the track is inside the deck's top cell, centimetres. */
const UNDER_TOP = 1

type DeckRail = Rail & { deck: RailDeck }

interface LoadedTile {
  tile: TileData
  group: Group
  rails: DeckRail[]
  /** The sections that carry track. */
  sections: Set<number>
  removed: Map<number, ReadonlySet<number>>
  meshes: Mesh[]
}

/**
 * Whether the deck's top under `(x, z)`, for a track `y` metres up, stands with the cells `removed`.
 * A column with no cell of this section's is another's (`voxels::share`), and counts as standing.
 */
export function deckStands(shell: Shell, removed: ReadonlySet<number>, x: number, y: number, z: number): boolean {
  const column = Math.floor((x * 100 - shell.origin[0]) / CELL)
  const row = Math.floor((z * 100 - shell.origin[2]) / CELL)
  const [width, height, depth] = shell.dims
  if (column < 0 || row < 0 || column >= width || row >= depth) return true
  const layer = Math.floor((y * 100 - UNDER_TOP - shell.origin[1]) / CELL)
  for (const off of TOP_CELLS) {
    if (layer + off < 0 || layer + off >= height) continue
    const id = shell.index(column, layer + off, row)
    if (shell.isSolid(id)) return !removed.has(id)
  }
  return true
}

export class DeckTracks {
  private readonly loaded = new Map<number, LoadedTile>()

  /** Tile `key` is drawn in `group` now: its deck track is laid, but where the decks are holed already (`carved`). */
  attach(key: number, tile: TileData, group: Group, carved: ReadonlyMap<number, ReadonlySet<number>>) {
    const rails = tile.rails.filter((rail): rail is DeckRail => rail.deck !== null)
    if (rails.length === 0) return
    const sections = new Set(rails.map((rail) => rail.deck.building))
    const removed = new Map([...carved].filter(([index]) => sections.has(index)))
    const loaded: LoadedTile = { tile, group, rails, sections, removed, meshes: [] }
    this.loaded.set(key, loaded)
    this.draw(loaded)
  }

  /** The tile's group frees the meshes with the rest of it (`disposeTile`). */
  detach(key: number) {
    this.loaded.delete(key)
  }

  /** The cells `removed` are gone from building `index` of tile `key`, and no others. */
  holed(key: number, index: number, removed: ReadonlySet<number>) {
    const loaded = this.loaded.get(key)
    if (!loaded?.sections.has(index)) return
    loaded.removed.set(index, removed)
    this.draw(loaded)
  }

  dispose() {
    this.loaded.clear()
  }

  private draw(loaded: LoadedTile) {
    const { tile, group, rails, removed } = loaded
    for (const mesh of loaded.meshes) {
      mesh.geometry.dispose()
      group.remove(mesh)
    }
    const stands = (section: number, x: number, y: number, z: number) => {
      const gone = removed.get(section)
      if (!gone?.size) return true
      const shell = shellOf(tile, section)
      return !shell || deckStands(shell, gone, x, y, z)
    }
    loaded.meshes = deckTrackMeshes(deckRibbons(rails, stands))
    if (loaded.meshes.length > 0) group.add(...loaded.meshes)
  }
}
