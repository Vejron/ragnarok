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

  /**
   * `bright` effects (tracers, sparks) ignore fog and tone mapping, so they keep their colour against
   * a pale sky and at range, where fog would otherwise melt them into it.
   */
  private add(geometry: THREE.BufferGeometry, color: number, life: number, place: (mesh: THREE.Mesh) => void, animate: (mesh: THREE.Mesh, progress: number) => void, bright = false) {
    const material = new THREE.MeshBasicMaterial({ color, transparent: true, depthWrite: false, ...(bright ? { fog: false, toneMapped: false } : {}) })
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
    // A hot core inside a wider orange glow: the white alone vanished against sky and pale roofs.
    const lay = (mesh: THREE.Mesh, width: number) => {
      mesh.position.copy(from).lerp(to, 0.5)
      mesh.scale.set(width, length, width)
      mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction)
    }
    this.add(this.beam, 0xffffff, 0.16, (mesh) => lay(mesh, 1.1), () => {}, true)
    this.add(this.beam, 0xff8a1f, 0.16, (mesh) => lay(mesh, 3.2), () => {}, true)
    if (hit) {
      this.add(this.sphere, 0xffffff, 0.1, (mesh) => mesh.position.copy(to), (mesh, p) => mesh.scale.setScalar(0.15 + p * 0.2), true)
      this.add(this.sphere, 0xff9a1f, 0.28, (mesh) => mesh.position.copy(to), (mesh, p) => mesh.scale.setScalar(0.2 + p * 0.55), true)
    }
  }

  /**
   * A machine gun round with a tracer: a small flash at the muzzle and a short bright streak that flies
   * from `from` to `to` at `speed` m/s. Where it ends on something, the server's hit event sparks.
   */
  round(from: THREE.Vector3, to: THREE.Vector3, speed = 700) {
    const direction = to.clone().sub(from)
    const length = direction.length()
    if (length < 0.01) return
    direction.divideScalar(length)
    this.add(this.sphere, 0xffe9a0, 0.06, (mesh) => mesh.position.copy(from), (mesh, p) => mesh.scale.setScalar(0.22 + p * 0.15), true)
    const streak = Math.min(7, length)
    const fly = (mesh: THREE.Mesh, p: number) => mesh.position.copy(from).addScaledVector(direction, streak / 2 + (length - streak) * p)
    const lay = (width: number) => (mesh: THREE.Mesh) => {
      mesh.scale.set(width, streak, width)
      mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction)
    }
    this.add(this.beam, 0xfff4cc, length / speed, lay(1.2), fly, true)
    this.add(this.beam, 0xff7a1a, length / speed, lay(3), fly, true)
  }

  /** A round striking something: a spark and a puff. */
  strike(at: THREE.Vector3) {
    this.add(this.sphere, 0xffa21f, 0.22, (mesh) => mesh.position.copy(at), (mesh, p) => mesh.scale.setScalar(0.15 + p * 0.4), true)
  }

  /**
   * Debris flung from where a round hit: red flecks off a body, sparks off a vehicle. A kill throws
   * more, harder.
   */
  impact(at: THREE.Vector3, kind: 'body' | 'vehicle' | 'kill', away: THREE.Vector3 | null = null) {
    const count = kind === 'kill' ? 12 : 6
    const color = kind === 'vehicle' ? 0xffc040 : 0xff2a22
    for (let i = 0; i < count; i++) {
      const direction = new THREE.Vector3(Math.random() - 0.5, Math.random() * 0.8, Math.random() - 0.5).normalize()
      if (away) direction.addScaledVector(away, 0.8).normalize()
      const speed = (kind === 'kill' ? 4.5 : 3) * (0.5 + Math.random())
      const size = (kind === 'vehicle' ? 0.05 : 0.08) * (0.7 + Math.random() * 0.8) * (kind === 'kill' ? 1.4 : 1)
      this.add(
        this.sphere, color, 0.28 + Math.random() * 0.2,
        (mesh) => mesh.position.copy(at),
        (mesh, p) => {
          mesh.position.copy(at).addScaledVector(direction, speed * p * 0.5)
          mesh.position.y -= 4 * p * p * 0.5
          mesh.scale.setScalar(size * (1 - p * 0.5))
        },
        true,
      )
    }
    this.add(this.sphere, 0xffffff, 0.09, (mesh) => mesh.position.copy(at), (mesh, p) => mesh.scale.setScalar(0.12 + p * 0.3), true)
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
