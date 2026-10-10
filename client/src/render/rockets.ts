import * as THREE from 'three'

/** The rockets in flight, one small glowing missile each, pointed along its velocity. */
export class Rockets {
  readonly root = new THREE.Group()
  private readonly body = new THREE.CylinderGeometry(0.09, 0.09, 0.8, 8)
  private readonly material = new THREE.MeshBasicMaterial({ color: 0xff8a2a })
  private readonly meshes = new Map<number, THREE.Mesh>()
  private readonly up = new THREE.Vector3(0, 1, 0)

  /** Places rocket `id`, creating it on first sight. `velocity` only gives the heading. */
  place(id: number, position: readonly [number, number, number], velocity: readonly [number, number, number]) {
    let mesh = this.meshes.get(id)
    if (!mesh) {
      mesh = new THREE.Mesh(this.body, this.material)
      this.meshes.set(id, mesh)
      this.root.add(mesh)
    }
    mesh.position.set(...position)
    const heading = new THREE.Vector3(...velocity)
    if (heading.lengthSq() > 1e-6) mesh.quaternion.setFromUnitVectors(this.up, heading.normalize())
  }

  remove(id: number) {
    const mesh = this.meshes.get(id)
    if (!mesh) return
    mesh.removeFromParent()
    this.meshes.delete(id)
  }

  has(id: number) {
    return this.meshes.has(id)
  }

  dispose() {
    this.meshes.clear()
    this.body.dispose()
    this.material.dispose()
    this.root.removeFromParent()
  }
}
