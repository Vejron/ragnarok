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

  /** A shell leaving the barrel: a flash and a short puff along it. The shell is drawn in flight. */
  muzzleFlash(muzzle: THREE.Vector3, direction: THREE.Vector3) {
    this.add(this.sphere, 0xffe9a0, 0.12, (mesh) => mesh.position.copy(muzzle), (mesh, p) => mesh.scale.setScalar(0.5 + p * 0.8))
    this.add(
      this.sphere, 0xffc060, 0.25,
      (mesh) => mesh.position.copy(muzzle).addScaledVector(direction, 1.0),
      (mesh, p) => mesh.scale.setScalar(0.3 + p * 0.9),
    )
  }

  /** A rifle shot: a short flash at the muzzle and, when it ended on something, a spark there. */
  tracer(from: THREE.Vector3, to: THREE.Vector3, hit: boolean) {
    const direction = to.clone().sub(from)
    const length = direction.length()
    if (length < 0.01) return
    direction.divideScalar(length)
    this.add(this.sphere, 0xfff2b0, 0.06, (mesh) => mesh.position.copy(from).addScaledVector(direction, 0.6), (mesh, p) => mesh.scale.setScalar(0.12 + p * 0.1))
    this.add(
      this.beam, 0xffffff, 0.09,
      (mesh) => {
        mesh.position.copy(from).lerp(to, 0.5)
        mesh.scale.set(0.5, length, 0.5)
        mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction)
      },
      () => {},
    )
    if (hit) this.add(this.sphere, 0xffd27a, 0.15, (mesh) => mesh.position.copy(to), (mesh, p) => mesh.scale.setScalar(0.1 + p * 0.25))
  }

  /** A rocket or a wreck going up at `at`. */
  burst(at: THREE.Vector3, radius: number) {
    this.add(this.sphere, 0xff9a3c, 0.5, (mesh) => mesh.position.copy(at), (mesh, p) => mesh.scale.setScalar(radius * 0.15 + p * radius * 0.5))
    this.add(this.sphere, 0xfff0c0, 0.22, (mesh) => mesh.position.copy(at), (mesh, p) => mesh.scale.setScalar(radius * 0.1 + p * radius * 0.25))
  }

  /** A puff of smoke from a damaged vehicle, drifting up. Black and bigger for a wreck. */
  smoke(at: THREE.Vector3, wreck: boolean) {
    const size = wreck ? 1.1 : 0.7
    this.add(
      this.sphere, wreck ? 0x222222 : 0x777777, wreck ? 1.4 : 1.0,
      (mesh) => mesh.position.copy(at),
      (mesh, p) => {
        mesh.scale.setScalar(size * (0.5 + p))
        mesh.position.y = at.y + p * 3
      },
    )
  }

  /** A cloud of dust of about `size` metres across at `at`, swelling and rising as it thins. */
  dust(at: THREE.Vector3, size: number) {
    this.add(
      this.sphere, 0xcbc2b0, 2.2,
      (mesh) => mesh.position.copy(at),
      (mesh, p) => {
        mesh.scale.setScalar(size * (0.35 + p * 0.65))
        mesh.position.y = at.y + p * size * 0.5
      },
    )
  }

  /** Just the muzzle flash, for the local player's own shot before the server confirms it. */
  muzzle(at: THREE.Vector3) {
    this.add(this.sphere, 0xfff2b0, 0.06, (mesh) => mesh.position.copy(at), (mesh, p) => mesh.scale.setScalar(0.12 + p * 0.1))
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
