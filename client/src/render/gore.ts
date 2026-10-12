import * as THREE from 'three'
import { DEATH_BLAST, DEATH_BODY, DEATH_HEAD, DEATH_OVERKILL, DEATH_ROADKILL } from '@/net/codec'
import type { Character } from './character'
import type { Debris } from './debris'
import type { Effects } from './effects'
import type { Particles } from './particles'

/** Seconds a limb lies about before it shrinks away. */
const GIB_LIFE = 9
/** Height of a limb's centre above the ground when it lies there, metres. */
const GIB_REST = 0.12
const BLOOD = [0x9a0b0b, 0xb81414, 0x6e0808]

/** A thing that bleeds as it moves, for a while. */
interface Trail {
  at: () => THREE.Vector3
  left: number
  /** Droplets a second. */
  rate: number
  /** How hard they are thrown, m/s. */
  force: number
}

const spread = (amount: number) => (Math.random() - 0.5) * 2 * amount

/**
 * How a stickman dies. A bullet throws the body; a headshot takes the head off and leaves a fountain;
 * blasts, cars and overkill blow it into limbs. Pieces are `Debris`, so they fly, bounce and lie about.
 */
export class Gore {
  private readonly trails: Trail[] = []
  private readonly direction = new THREE.Vector3()

  constructor(
    private readonly debris: Debris,
    private readonly particles: Particles,
    private readonly effects: Effects,
  ) {}

  /**
   * `avatar` has been killed in `style` (`DEATH_*`), thrown towards `yaw` (0 faces -Z) at `power` m/s.
   * `groundAt` is the terrain, for the thrown body to land on.
   */
  die(avatar: Character, style: number, yaw: number, power: number, groundAt: (x: number, z: number) => number) {
    const dir = this.direction.set(-Math.sin(yaw), 0, -Math.cos(yaw))
    const centre = avatar.position
    this.effects.impact(centre, 'kill', dir)
    if (style === DEATH_HEAD) {
      this.pop(avatar, dir, power)
      avatar.launch(dir.clone().multiplyScalar(power * 0.4).setY(1.5), new THREE.Vector3(spread(1), 0, spread(1)), groundAt)
    } else if (style === DEATH_BLAST || style === DEATH_ROADKILL || style === DEATH_OVERKILL) {
      this.burst(avatar, dir, power, style)
    } else {
      // A bullet: the body is thrown back, head over heels the harder it was hit.
      const side = new THREE.Vector3(dir.z, 0, -dir.x)
      const velocity = dir.clone().multiplyScalar(power * 1.1).setY(2 + power * 0.25)
      avatar.launch(velocity, side.multiplyScalar(power * 0.9).add(new THREE.Vector3(spread(1.5), spread(1.5), spread(1.5))), groundAt)
      this.mist(centre, 8, 3, dir)
      this.bleed(() => avatar.position, 0.5, 40)
    }
  }

  /** The head comes off, and the neck stump sprays. */
  private pop(avatar: Character, dir: THREE.Vector3, power: number) {
    avatar.decapitate()
    for (const { object } of avatar.shatter(['head'])) {
      const velocity = dir.clone().multiplyScalar(power * 0.5 + 1.5).add(new THREE.Vector3(spread(2), 5 + Math.random() * 3, spread(2)))
      this.debris.throw(object, velocity, new THREE.Vector3(spread(14), spread(14), spread(14)), GIB_REST, GIB_LIFE)
      this.bleed(() => object.position, 1.0, 55)
    }
    const neck = new THREE.Vector3()
    this.bleed(() => neck.copy(avatar.position).add(new THREE.Vector3(0, 0.7, 0)), 1.2, 90, 3.5)
    this.mist(avatar.position, 14, 4, dir)
  }

  /** The body is torn apart. */
  private burst(avatar: Character, dir: THREE.Vector3, power: number, style: number) {
    const heavy = style === DEATH_BLAST || style === DEATH_ROADKILL ? 1 : 0.6
    for (const { key, object } of avatar.shatter()) {
      const away = object.position.clone().sub(avatar.position)
      away.y *= 0.5
      if (away.lengthSq() > 1e-4) away.normalize()
      const torso = key === 'torso' ? 0.6 : 1
      const velocity = dir
        .clone()
        .multiplyScalar(power * (0.5 + Math.random() * 0.7) * heavy * torso)
        .addScaledVector(away, (3 + Math.random() * 5) * torso)
      velocity.y += (3 + Math.random() * 5 + power * 0.2) * torso
      this.debris.throw(object, velocity, new THREE.Vector3(spread(16), spread(16), spread(16)), GIB_REST, GIB_LIFE)
      this.bleed(() => object.position, 0.9, 45)
    }
    this.mist(avatar.position, 36, 6 + power * 0.2, dir)
    this.bleed(() => avatar.position, 0.35, 120, 4)
  }

  /** A burst of red droplets and a haze. */
  private mist(at: THREE.Vector3, count: number, speed: number, dir: THREE.Vector3) {
    for (let i = 0; i < count; i++) {
      const aim = new THREE.Vector3(spread(1), Math.random() * 0.9, spread(1)).addScaledVector(dir, 0.6).normalize().multiplyScalar(speed * (0.3 + Math.random()))
      this.particles.spawn({
        position: at,
        velocity: [aim.x, aim.y + 1, aim.z],
        life: 0.5 + Math.random() * 0.7,
        size: [0.14 + Math.random() * 0.1, 0.05],
        color: BLOOD[i % BLOOD.length]!,
        alpha: 0.9,
        gravity: 14,
        drag: 0.6,
      })
    }
    for (let i = 0; i < Math.ceil(count / 4); i++) {
      this.particles.spawn({
        position: at,
        velocity: [spread(1.5), 0.5 + Math.random(), spread(1.5)],
        life: 0.6 + Math.random() * 0.5,
        size: [0.3, 0.9],
        color: BLOOD[2]!,
        alpha: 0.45,
        gravity: -0.5,
        drag: 1.5,
      })
    }
  }

  /** Makes `at` bleed for `seconds`, `rate` droplets a second, thrown up to `force` m/s. */
  private bleed(at: () => THREE.Vector3, seconds: number, rate: number, force = 1.5) {
    this.trails.push({ at, left: seconds, rate, force })
  }

  update(dt: number) {
    for (let i = this.trails.length - 1; i >= 0; i--) {
      const trail = this.trails[i]!
      trail.left -= dt
      if (trail.left <= 0) {
        this.trails.splice(i, 1)
        continue
      }
      const { force } = trail
      const drops = Math.max(1, Math.round(trail.rate * dt))
      for (let d = 0; d < drops; d++) {
        this.particles.spawn({
          position: trail.at(),
          velocity: [spread(force * 0.5), force * (0.4 + Math.random()), spread(force * 0.5)],
          life: 0.4 + Math.random() * 0.5,
          size: [0.1 + Math.random() * 0.06, 0.04],
          color: BLOOD[d % BLOOD.length]!,
          alpha: 0.85,
          gravity: 14,
          drag: 0.4,
        })
      }
    }
  }
}
