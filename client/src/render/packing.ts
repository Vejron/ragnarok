import { Geometry, packVertices, VERTEX_STRIDE, type Sphere, type VertexArrays } from '@ragnarok/engine'

/*
 * A big mesh nobody reads again (a tile's terrain, roads and buildings) is packed into the renderer's
 * vertex layout where it is made, in the tile worker, so that the frame it is drawn in only copies it
 * to the GPU: interleaving a town tile's few hundred thousand vertices was the costliest part of the
 * frame it arrived in.
 */

/** A mesh's arrays, and once packed (`pack`) its vertices in the renderer's layout and its bounds. */
export interface Packable extends VertexArrays {
  indices?: Uint16Array | Uint32Array
  packed?: ArrayBuffer
  bounds?: Sphere
}

/**
 * Packs `buffers` in place (`packVertices`): its vertices go into `packed`, its bounds are worked
 * out, and its vertex arrays are emptied (each its own empty array, so each can still be transferred).
 * Its indices stay. Returns it.
 */
export function pack<T extends Packable>(buffers: T): T {
  if (buffers.packed) return buffers
  buffers.bounds ??= { ...new Geometry({ positions: buffers.positions }).bounds }
  buffers.packed = packVertices(buffers)
  buffers.positions = new Float32Array(0)
  if (buffers.normals) buffers.normals = new Float32Array(0)
  if (buffers.uvs) buffers.uvs = new Float32Array(0)
  if (buffers.colors) buffers.colors = new Float32Array(0)
  if (buffers.custom) buffers.custom = new Float32Array(0)
  return buffers
}

/**
 * Packs `buffers` as `pack` does but keeps its arrays, for a mesh that is read again after it is drawn
 * (the water, which its refinement reads): the renderer copies the packed vertices the first time and
 * lets them go. Returns it.
 */
export function packAlongside<T extends Packable>(buffers: T): T {
  if (buffers.packed) return buffers
  buffers.bounds ??= { ...new Geometry({ positions: buffers.positions }).bounds }
  buffers.packed = packVertices(buffers)
  return buffers
}

/** How many vertices `buffers` holds, packed or not. */
export function vertexCountOf(buffers: Packable): number {
  return buffers.packed ? buffers.packed.byteLength / VERTEX_STRIDE : buffers.positions.length / 3
}

/** The buffers of `buffers` that move to another thread instead of being copied: its arrays, and its packed vertices. */
export function packableTransferables(buffers: Packable): ArrayBuffer[] {
  const arrays = [buffers.positions, buffers.normals, buffers.uvs, buffers.colors, buffers.custom, buffers.indices].filter((array) => array !== undefined)
  return [...arrays.map((array) => array.buffer as ArrayBuffer), ...(buffers.packed ? [buffers.packed] : [])]
}
