import { Group, type Geometry, type Material, type Node, Quaternion, Vector3 } from '@ragnarok/engine'

interface Piece {
  object: Node
  velocity: Vector3
  /** Angular velocity, rad/s, about world axes. */
  spin: Vector3
  /** Height of its centre above the ground when it lies there, metres. */
  rest: number
  age: number
  life: number
  /** Geometry and materials made for this piece alone, freed with it. */
  owned: readonly (Geometry | Material)[]
  /** Seconds at the end of its life it spends sinking into the ground rather than shrinking; 0 to shrink. */
  sink: number
  /** How far it sinks, metres: a little more than it is big. */
  depth: number
  scale: { x: number; y: number; z: number }
}

/** Seconds a piece takes to shrink away at the end of its life. */
const SHRINK = 1.0
const GRAVITY = 9.8

/**
 * Loose parts that come off vehicles: a bumper, a door, a wheel. Each is thrown, tumbles, bounces
 * on the ground and comes to rest, then shrinks away. Nothing collides with them; they are only
 * drawn.
 */
export class Debris {
  readonly root = new Group()
  private readonly pieces: Piece[] = []
  private readonly turn = new Quaternion()
  private readonly axis = new Vector3()

  /** Ground height under a point. */
  groundAt: (x: number, z: number) => number = () => 0

  constructor(private readonly capacity = 40) {}

  get count() {
    return this.pieces.length
  }

  /**
   * Throws `object` (already placed in world space) with `velocity` and `spin`, for `life` seconds.
   * `owned` is geometry and materials made for it alone, freed when it goes. With `sink`, it spends its
   * last `sink` seconds sinking slowly into the ground instead of shrinking away.
   */
  throw(object: Node, velocity: Vector3, spin: Vector3, rest: number, life = 8, owned: readonly (Geometry | Material)[] = [], sink = 0) {
    if (this.pieces.length >= this.capacity) this.drop(0)
    this.root.add(object)
    const depth = rest * 2 + Math.max(object.scale.x, object.scale.y, object.scale.z) + 0.1
    this.pieces.push({ object, velocity: velocity.clone(), spin: spin.clone(), rest, age: 0, life, owned, sink, depth, scale: { x: object.scale.x, y: object.scale.y, z: object.scale.z } })
  }

  private drop(index: number) {
    const [piece] = this.pieces.splice(index, 1)
    piece!.object.removeFromParent()
    for (const thing of piece!.owned) thing.dispose()
  }

  update(dt: number) {
    for (let i = this.pieces.length - 1; i >= 0; i--) {
      const piece = this.pieces[i]!
      piece.age += dt
      if (piece.age >= piece.life) {
        this.drop(i)
        continue
      }
      const { object, velocity, spin } = piece
      const left = piece.life - piece.age
      if (left < piece.sink) {
        object.position.y -= (dt / piece.sink) * piece.depth
        continue
      }
      velocity.y -= GRAVITY * dt
      object.position.addScaledVector(velocity, dt)
      const floor = this.groundAt(object.position.x, object.position.z) + piece.rest
      if (object.position.y < floor) {
        object.position.y = floor
        velocity.y = Math.abs(velocity.y) > 1 ? Math.abs(velocity.y) * 0.3 : 0
        velocity.x *= 0.6
        velocity.z *= 0.6
        spin.multiplyScalar(0.6)
      }
      const rate = spin.length()
      if (rate > 1e-3) {
        this.turn.setFromAxisAngle(this.axis.copy(spin).divideScalar(rate), rate * dt)
        object.quaternion.premultiply(this.turn)
      }
      if (piece.sink === 0 && left < SHRINK) object.scale.set(piece.scale.x, piece.scale.y, piece.scale.z).multiplyScalar(Math.max(0.01, left / SHRINK))
    }
  }

  dispose() {
    while (this.pieces.length > 0) this.drop(0)
    this.root.removeFromParent()
  }
}
