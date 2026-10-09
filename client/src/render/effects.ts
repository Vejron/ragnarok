import * as THREE from 'three'

interface Effect {
  mesh: THREE.Mesh
  age: number
  life: number
  /** Called every frame with progress 0..1. */
  animate: (progress: number) => void
}

/**
 * Short-lived shot effects in world space: a muzzle flash, a tracer along the shell's path and a
 * ball of fire where it lands. Everything fades out and is removed.
 */
export class Effects {
  readonly root = new THREE.Group()
  private readonly active: Effect[] = []
  private readonly sphere = new THREE.SphereGeometry(1, 12, 8)
  private readonly beam = new THREE.CylinderGeometry(0.06, 0.06, 1, 6)

  private add(geometry: THREE.BufferGeometry, color: number, life: number, place: (mesh: THREE.Mesh) => void, animate: (mesh: THREE.Mesh, progress: number) => void) {
    const material = new THREE.MeshBasicMaterial({ color, transparent: true, depthWrite: false })
    const mesh = new THREE.Mesh(geometry, material)
    place(mesh)
    this.root.add(mesh)
    this.active.push({
      mesh,
      age: 0,
      life,
      animate: (progress) => {
        material.opacity = 1 - progress
        animate(mesh, progress)
      },
    })
  }

  /** A shell leaving `muzzle` and bursting at `impact` (or just ending, for a shot into the sky). */
  shell(muzzle: THREE.Vector3, direction: THREE.Vector3, range: number, hit: boolean, blastRadius: number) {
    this.add(this.sphere, 0xffe9a0, 0.12, (mesh) => mesh.position.copy(muzzle), (mesh, p) => mesh.scale.setScalar(0.5 + p * 0.8))
    const end = muzzle.clone().addScaledVector(direction, range)
    const length = muzzle.distanceTo(end)
    this.add(
      this.beam, 0xfff2b0, 0.18,
      (mesh) => {
        mesh.position.copy(muzzle).lerp(end, 0.5)
        mesh.scale.set(1, length, 1)
        mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction)
      },
      () => {},
    )
    if (!hit) return
    this.add(this.sphere, 0xff9a3c, 0.45, (mesh) => mesh.position.copy(end), (mesh, p) => mesh.scale.setScalar(blastRadius * 0.15 + p * blastRadius * 0.45))
    this.add(this.sphere, 0xfff0c0, 0.2, (mesh) => mesh.position.copy(end), (mesh, p) => mesh.scale.setScalar(blastRadius * 0.1 + p * blastRadius * 0.2))
  }

  update(dt: number) {
    for (let i = this.active.length - 1; i >= 0; i--) {
      const effect = this.active[i]!
      effect.age += dt
      const progress = effect.age / effect.life
      if (progress >= 1) {
        (effect.mesh.material as THREE.Material).dispose()
        effect.mesh.removeFromParent()
        this.active.splice(i, 1)
      } else {
        effect.animate(progress)
      }
    }
  }

  dispose() {
    for (const effect of this.active) (effect.mesh.material as THREE.Material).dispose()
    this.active.length = 0
    this.sphere.dispose()
    this.beam.dispose()
    this.root.removeFromParent()
  }
}
