import { Mesh, MeshRefiner, type Node, type XYZ } from '@ragnarok/engine'
import { setWaterRefine, waterMaterial, waterSurfaces } from './water'

/** The side of the square about the camera whose water is cut finer, metres: inside the wave field's 96 m. */
export const REFINE_SIZE = 80
/** It moves in steps of this many of the water's vertex spacings, so it stays on the cells' lines. */
const SNAP_CELLS = 4

/**
 * The lakes and rivers near the camera cut finer on the GPU (`MeshRefiner`), so that the wave field's
 * bow waves, wakes and rings, finer than the tiles' metre-apart vertices, lift the surface as they are
 * rather than as pyramids on single vertices. The square follows the camera on the water's own cell
 * lines; the tiles' surfaces leave it to the refined mesh (`setWaterRefine`), and only the refined
 * vertices are lifted by the waves, fading out towards its edge so the two meet flat.
 */
export class WaterRefine {
  private refiner: MeshRefiner | null = null
  private mesh: Mesh | null = null
  /** A vertex of the water's lattice, and the spacing of its vertices, metres. */
  private lattice: { x: number; z: number; spacing: number } | null = null

  constructor(private readonly add: (...nodes: Node[]) => void) {}

  /** Where the square is now, its corner and side, metres; null until there is water to refine. */
  area: [number, number, number] | null = null

  /** Moves the square to stay about `eye` and hands it the surfaces it covers. */
  update(eye: Readonly<XYZ>) {
    if (!this.refiner) {
      const lattice = this.measure()
      if (!lattice) return
      this.lattice = lattice
      // Two triangles a cell, and a cell's margin round the square.
      const cells = Math.ceil(REFINE_SIZE / lattice.spacing) + 2
      this.refiner = new MeshRefiner(2 * cells * cells)
      this.mesh = new Mesh(this.refiner.geometry, waterMaterial())
      this.mesh.renderOrder = 1
      // Its vertices are written on the GPU; its geometry's own arrays say nothing about where they are.
      this.mesh.frustumCulled = false
      this.add(this.refiner, this.mesh)
    }
    const { x, z, spacing } = this.lattice!
    const snap = spacing * SNAP_CELLS
    const x0 = x + Math.floor((eye.x - REFINE_SIZE / 2 - x) / snap) * snap
    const z0 = z + Math.floor((eye.z - REFINE_SIZE / 2 - z) / snap) * snap
    const sources = [...waterSurfaces].map((mesh) => mesh.geometry).filter((g) => {
      const b = g.bounds
      return b.x + b.radius > x0 && b.x - b.radius < x0 + REFINE_SIZE && b.z + b.radius > z0 && b.z - b.radius < z0 + REFINE_SIZE
    })
    this.refiner.setSources(sources)
    this.refiner.setArea(x0, z0, REFINE_SIZE)
    // Its slot is made the first time it is drawn; from then on each frame's square is refined before the
    // water is drawn, so the tiles can leave it to the refined mesh at once.
    this.area = this.refiner.geometry.gpu ? [x0, z0, REFINE_SIZE] : null
    setWaterRefine(this.area)
  }

  /** The water's vertex lattice, read off a surface mesh: a corner, and the step to its next vertex. */
  private measure() {
    for (const mesh of waterSurfaces) {
      const p = mesh.geometry.positions
      if (p.length < 6) continue
      const spacing = Math.hypot(p[3]! - p[0]!, p[5]! - p[2]!)
      if (spacing > 0) return { x: p[0]!, z: p[2]!, spacing }
    }
    return null
  }

  dispose() {
    setWaterRefine(null)
    this.refiner?.removeFromParent()
    this.refiner?.dispose()
    this.mesh?.removeFromParent()
  }
}

