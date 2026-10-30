import { Vector3 } from '@ragnarok/engine'
import { DEATH_BLAST, DEATH_HEAD, DEATH_OVERKILL, DEATH_ROADKILL, LIMB_ARM_L, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_R, PART_HEAD, PART_TORSO } from '@/net/codec'
import type { Bloodstains } from './bloodstains'
import type { Character } from './character'
import type { Debris } from './debris'
import type { Effects } from './effects'
import type { Particles } from './particles'

/** Seconds a limb lies about before it shrinks away. */
const GIB_LIFE = 9
/** Height of a limb's centre above the ground when it lies there, metres. */
const GIB_REST = 0.12
/** Blood in the air: dark and only a little red, as it is out of the body. */
const BLOOD = [0x6e0303, 0x8a0707, 0x520202]
const BLOOD_DARK = 0x2a0101
/** Pulling a falling drop down, m/s², as the particles fall. */
const DROP_GRAVITY = 14
/** Share of the drops in the air that leave a stain where they land. */
const STAIN_SHARE = 0.4
/** Longest a drop is followed through the air before it is given up, seconds. */
const DROP_FLIGHT = 2
/** About how many stains the spray out of a hit leaves, larger by `HIT_STAIN_SCALE` than a drop's. */
const HIT_STAINS = 7
const HIT_STAIN_SCALE = 1.6
/** Share of a wound's trickle that stains: most of it runs into the clothes. */
const WOUND_STAIN_SHARE = 0.15
/** A soldier lies down this long after it is killed before its pool starts to spread, seconds. */
const POOL_AFTER = 1.2
/** Metres across a crawler's smear. */
const SMEAR_WIDTH = 0.45
/** The pieces (`limbOf` keys) each `LIMB_*` bit takes off: the whole arm or leg. */
const LIMB_PIECES: [number, string[]][] = [
  [LIMB_ARM_L, ['upper_arm_L', 'forearm_L']],
  [LIMB_ARM_R, ['upper_arm_R', 'forearm_R']],
  [LIMB_LEG_L, ['thigh_L', 'shin_L']],
  [LIMB_LEG_R, ['thigh_R', 'shin_R']],
]

/** A thing that bleeds as it moves, for a while. */
interface Trail {
  at: () => { x: number; y: number; z: number }
  left: number
  /** Droplets a second. */
  rate: number
  /** How hard they are thrown, m/s. */
  force: number
  /** Share of its drops that stain where they land. */
  share: number
  /** The part of a drop owed from frames too short for a whole one. */
  owed: number
}

/** A stain to lay when its drop lands. */
interface Landing {
  x: number
  y: number
  z: number
  radius: number
  /** Seconds until it lands. */
  in: number
  /** A pool keeps spreading over `spread` seconds; a drop has none. */
  spread: number
  /** Laid at the ground under `follow` when it lands, when it should follow a body lying down. */
  follow?: () => { x: number; z: number }
}

const spread = (amount: number) => (Math.random() - 0.5) * 2 * amount

/**
 * How a soldier dies. A bullet throws the body; a headshot takes the head off and leaves a fountain;
 * blasts, cars and overkill blow it into limbs. Pieces are `Debris`, so they fly, bounce and lie about.
 */
export class Gore {
  private readonly trails: Trail[] = []
  private readonly landings: Landing[] = []
  private readonly direction = new Vector3()
  private readonly point = new Vector3()

  constructor(
    private readonly debris: Debris,
    private readonly particles: Particles,
    private readonly effects: Effects,
    private readonly stains: Bloodstains,
    /** Ground height under a point. */
    private readonly groundAt: (x: number, z: number) => number,
  ) {}

  /**
   * `avatar` has been killed in `style` (`DEATH_*`), thrown towards `yaw` (0 faces -Z) at `power` m/s.
   * `groundAt` is the terrain, for the thrown body to land on.
   */
  die(avatar: Character, style: number, yaw: number, power: number, groundAt: (x: number, z: number) => number) {
    const dir = this.direction.set(-Math.sin(yaw), 0, -Math.cos(yaw))
    const centre = avatar.position
    this.effects.impact(centre, 'kill', dir)
    // It bleeds out where it comes to rest, unless it is in pieces.
    if (style !== DEATH_BLAST && style !== DEATH_ROADKILL && style !== DEATH_OVERKILL) this.pool(() => avatar.position, 0.75 + Math.random() * 0.3, 7)
    if (style === DEATH_HEAD) {
      this.pop(avatar, dir, power)
      avatar.launch(dir.clone().multiplyScalar(power * 0.4).setY(1.5), new Vector3(spread(1), 0, spread(1)), groundAt)
    } else if (style === DEATH_BLAST || style === DEATH_ROADKILL || style === DEATH_OVERKILL) {
      this.burst(avatar, dir, power, style)
    } else {
      // A bullet: the body is thrown back, head over heels the harder it was hit.
      const side = new Vector3(dir.z, 0, -dir.x)
      const velocity = dir.clone().multiplyScalar(power * 1.1).setY(2 + power * 0.25)
      avatar.launch(velocity, side.multiplyScalar(power * 0.9).add(new Vector3(spread(1.5), spread(1.5), spread(1.5))), groundAt)
      this.mist(centre, 8, 3, dir)
      this.bleed(() => avatar.position, 0.5, 40)
    }
  }

  /**
   * The limbs in `limbs` (`LIMB_*` bits) come off a living `avatar`: each is thrown off the body and
   * spins away bleeding, and the stump sprays.
   */
  sever(avatar: Character, limbs: number) {
    for (const [bit, keys] of LIMB_PIECES) {
      if (!(limbs & bit)) continue
      const stump = avatar.stump(bit)
      const out = new Vector3().subVectors(stump, avatar.position).setY(0)
      if (out.lengthSq() < 1e-4) out.set(spread(1), 0, spread(1))
      out.normalize()
      for (const { object } of avatar.shatter(keys)) {
        const velocity = out.clone().multiplyScalar(2 + Math.random() * 2).add(new Vector3(spread(1), 2.5 + Math.random() * 2, spread(1)))
        this.debris.throw(object, velocity, new Vector3(spread(12), spread(12), spread(12)), GIB_REST, GIB_LIFE)
        this.bleed(() => object.position, 0.8, 40)
      }
      this.mist(stump, 12, 3.5, out)
      this.bleed(() => avatar.stump(bit, stump), 1.2, 70, 2.5)
    }
  }

  /**
   * A living `avatar` was shot in `part` (`PART_*`, a limb by its `LIMB_*` bit) from bearing `from`
   * (0 faces -Z) for `amount`, at `offset` from where it is drawn from when the round found the spot
   * (all zero or none: guessed from the part). A puff of spray at the entry, drops kicked back toward
   * the shooter, a cone of blood out of the far side that spatters the ground behind, and the wound
   * goes on trickling a moment, moving with the body.
   */
  hit(avatar: Character, part: number, from: number, amount: number, offset: { x: number; y: number; z: number } | null = null) {
    const out = this.direction.set(Math.sin(from), 0, Math.cos(from))
    const at = this.point
    if (offset && (offset.x || offset.y || offset.z)) at.copy(avatar.position).add(offset)
    else if (part === PART_HEAD) at.copy(avatar.position).y += 0.75
    else if (part === PART_TORSO) at.copy(avatar.position).y += 0.25
    else avatar.stump(part, at)
    const heavy = Math.min(1, amount / 40)
    const head = part === PART_HEAD
    // Entry: a burst of fine spray, and a few drops thrown back the way the round came.
    this.particles.spawn({
      position: at,
      velocity: [-out.x * 0.6, 0.3, -out.z * 0.6],
      life: 0.18 + Math.random() * 0.1,
      size: [0.06, 0.28 + heavy * 0.15],
      color: BLOOD[1]!,
      colorEnd: BLOOD_DARK,
      alpha: 0.45,
      gravity: 0.5,
      drag: 4,
      shape: 'puff',
    })
    for (let i = 0; i < 4; i++) {
      const velocity: [number, number, number] = [-out.x * (1 + Math.random() * 1.5) + spread(0.6), 0.8 + Math.random(), -out.z * (1 + Math.random() * 1.5) + spread(0.6)]
      this.drop(at, velocity, 0.015 + Math.random() * 0.02, 0.6, i, 0)
    }
    // Exit: out of the far side of whatever it went through. A few larger stains rather than many
    // small ones, so a firefight does not use up the stains' ring in seconds.
    const through = head ? 0.18 : part === PART_TORSO ? 0.28 : 0.1
    const exit = at.clone().addScaledVector(out, through)
    const count = Math.round((6 + heavy * 16) * (head ? 1.5 : 1))
    this.mist(exit, count, 3 + heavy * 4, out, 0.55, Math.min(1, HIT_STAINS / (count * 2)), HIT_STAIN_SCALE)
    // The wound itself bleeds on a moment where it is on the body.
    const wound = avatar.pin(at)
    const spot = new Vector3()
    this.bleed(() => wound(spot), 0.3 + heavy * 0.5, 30, 0.6, WOUND_STAIN_SHARE)
  }

  /** A legless `avatar` crawls on, bleeding: it leaves a smear behind it, as `track`. */
  drag(track: string, avatar: Character) {
    const at = this.point.copy(avatar.position)
    at.y = this.groundAt(at.x, at.z)
    this.stains.smear(track, at, SMEAR_WIDTH * (0.8 + Math.random() * 0.4))
  }

  /** Smear `track` stops: the crawler is gone, dead or bandaged. */
  lift(track: string) {
    this.stains.lift(track)
  }

  /** A drop or two from each of `avatar`'s stumps in `bleeding` (`LIMB_*` bits) that still bleeds. */
  drip(avatar: Character, bleeding: number) {
    for (const [bit] of LIMB_PIECES) {
      if (!(bleeding & bit)) continue
      const stump = new Vector3()
      this.bleed(() => avatar.stump(bit, stump), 0.25, 14, 1)
    }
    // And a spot on the ground under it. The stains' ring keeps the total in hand.
    const at = avatar.position
    this.stains.blot(at.x + spread(0.3), this.groundAt(at.x, at.z), at.z + spread(0.3), 0.06 + Math.random() * 0.08)
  }

  /** The head comes off, and the neck stump sprays. */
  private pop(avatar: Character, dir: Vector3, power: number) {
    avatar.decapitate()
    for (const { object } of avatar.shatter(['head'])) {
      const velocity = dir.clone().multiplyScalar(power * 0.5 + 1.5).add(new Vector3(spread(2), 5 + Math.random() * 3, spread(2)))
      this.debris.throw(object, velocity, new Vector3(spread(14), spread(14), spread(14)), GIB_REST, GIB_LIFE)
      this.bleed(() => object.position, 1.0, 55)
    }
    const neck = new Vector3()
    this.bleed(() => neck.copy(avatar.position).add(new Vector3(0, 0.7, 0)), 1.2, 90, 3.5)
    this.mist(avatar.position, 14, 4, dir)
  }

  /** The body is torn apart. */
  private burst(avatar: Character, dir: Vector3, power: number, style: number) {
    const heavy = style === DEATH_BLAST || style === DEATH_ROADKILL ? 1 : 0.6
    for (const { key, object } of avatar.shatter()) {
      const away = new Vector3().copy(object.position).sub(avatar.position)
      away.y *= 0.5
      if (away.lengthSq() > 1e-4) away.normalize()
      const torso = key === 'torso' ? 0.6 : 1
      const velocity = dir
        .clone()
        .multiplyScalar(power * (0.5 + Math.random() * 0.7) * heavy * torso)
        .addScaledVector(away, (3 + Math.random() * 5) * torso)
      velocity.y += (3 + Math.random() * 5 + power * 0.2) * torso
      this.debris.throw(object, velocity, new Vector3(spread(16), spread(16), spread(16)), GIB_REST, GIB_LIFE)
      this.bleed(() => object.position, 0.9, 45)
      if (key === 'torso') this.pool(() => object.position, 0.6, 5)
    }
    this.mist(avatar.position, 36, 6 + power * 0.2, dir)
    this.bleed(() => avatar.position, 0.35, 120, 4)
  }

  /**
   * A burst of droplets thrown along `dir` and a puff of fine spray. `cone` is how tightly they keep
   * to `dir`, 0 every way to 1 straight along it; `share` of the drops stain, `scale` times as wide.
   */
  private mist(at: Vector3, count: number, speed: number, dir: Vector3, cone = 0, share = STAIN_SHARE, scale = 1) {
    // Many small drops rather than a few big ones: a spray, not confetti.
    for (let i = 0; i < count * 2; i++) {
      const aim = new Vector3(spread(1), Math.random() * 0.9, spread(1)).multiplyScalar(1 - cone).addScaledVector(dir, 0.6 + cone).normalize().multiplyScalar(speed * (0.3 + Math.random()))
      this.drop(at, [aim.x, aim.y + 1, aim.z], 0.02 + Math.random() ** 2 * 0.06, 0.6, i, share, scale)
    }
    // The finest of it hangs for a moment as a mist and is gone.
    for (let i = 0; i < Math.ceil(count / 6); i++) {
      this.particles.spawn({
        position: at,
        velocity: [spread(0.8) + dir.x * speed * 0.3, 0.2 + Math.random() * 0.4, spread(0.8) + dir.z * speed * 0.3],
        life: 0.25 + Math.random() * 0.25,
        size: [0.08, 0.4],
        color: BLOOD[1]!,
        colorEnd: BLOOD_DARK,
        alpha: 0.22,
        gravity: 0.5,
        drag: 3,
        shape: 'puff',
      })
    }
  }

  /**
   * One drop `size` metres across thrown from `at` at `velocity`. It flies as a bead of liquid,
   * stretching with speed, until it reaches the ground, where it is gone; `share` of them leave a
   * stain there as they land, `scale` times as wide as a drop's.
   */
  private drop(at: { x: number; y: number; z: number }, velocity: [number, number, number], size: number, drag: number, i: number, share = STAIN_SHARE, scale = 1) {
    const landing = this.flight(at, velocity, drag)
    this.particles.spawn({
      position: at,
      velocity,
      life: landing?.in ?? DROP_FLIGHT,
      size: [size, size * 0.85],
      color: BLOOD[i % BLOOD.length]!,
      colorEnd: BLOOD_DARK,
      alpha: 1,
      gravity: DROP_GRAVITY,
      drag,
      stretch: 0.025,
      shape: 'drop',
    })
    if (landing && Math.random() < share) {
      landing.radius = (0.08 + Math.random() * 0.3) * landing.radius * scale
      this.landings.push(landing)
    }
  }

  /**
   * Where a drop thrown from `at` at `velocity` lands: there, seconds from now, with `radius` how
   * much wider than a gentle drop it splashes; or null if it is still in the air after `DROP_FLIGHT`.
   * Worked out as the particles move (each second `drag` of the speed is lost, then gravity pulls),
   * in closed form: the height it falls to is found by Newton's method, then the ground there is
   * looked up and the fall worked out again to it, up to a few times on a slope.
   */
  private flight(at: { x: number; y: number; z: number }, velocity: readonly [number, number, number], drag: number): Landing | null {
    const [vx, vy, vz] = velocity
    const g = DROP_GRAVITY
    const k = Math.max(drag, 1e-3)
    // How far it has gone at the speed it set off with, after `t`: drag eats into it.
    const reach = (t: number) => (1 - Math.exp(-k * t)) / k
    const height = (t: number) => at.y + (vy + g / k) * reach(t) - (g / k) * t
    const sink = (t: number) => (vy + g / k) * Math.exp(-k * t) - g / k
    let ground = this.groundAt(at.x, at.z)
    for (let pass = 0; ; pass++) {
      // Start from the fall without drag, past the top of its arc, and close in on the dragged one.
      let t = (vy + Math.sqrt(Math.max(0, vy * vy + 2 * g * (at.y - ground)))) / g
      for (let n = 0; n < 4; n++) {
        const v = sink(t)
        if (v > -1e-3) break
        t -= (height(t) - ground) / v
      }
      if (!(t > 0)) t = 0
      if (t > DROP_FLIGHT) return null
      const x = at.x + vx * reach(t)
      const z = at.z + vz * reach(t)
      const there = this.groundAt(x, z)
      if (pass === 3 || Math.abs(there - ground) < 0.01) {
        const speed = Math.hypot(vx, vz) * Math.exp(-k * t)
        return { x, y: there, z, radius: Math.min(1.6, 1 + speed * 0.08), in: t, spread: 0 }
      }
      ground = there
    }
  }

  /** A pool spreads under `at` (a body coming to rest) once it lies still, `radius` metres across over `seconds`. */
  private pool(at: () => { x: number; z: number }, radius: number, seconds: number) {
    this.landings.push({ x: 0, y: 0, z: 0, radius, in: POOL_AFTER, spread: seconds, follow: at })
  }

  /** Makes `at` bleed for `seconds`, `rate` droplets a second, thrown up to `force` m/s; `share` of them stain. */
  private bleed(at: () => { x: number; y: number; z: number }, seconds: number, rate: number, force = 1.5, share = STAIN_SHARE) {
    this.trails.push({ at, left: seconds, rate, force, share, owed: 0 })
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
      // Its rate, however short the frame: what does not make a whole drop is owed to the next.
      trail.owed += trail.rate * dt
      const drops = Math.floor(trail.owed)
      trail.owed -= drops
      for (let d = 0; d < drops; d++) {
        const velocity: [number, number, number] = [spread(force * 0.5), force * (0.4 + Math.random()), spread(force * 0.5)]
        this.drop(trail.at(), velocity, 0.06 + Math.random() * 0.06, 0.4, d, trail.share)
      }
    }
    for (let i = this.landings.length - 1; i >= 0; i--) {
      const landing = this.landings[i]!
      landing.in -= dt
      if (landing.in > 0) continue
      this.landings.splice(i, 1)
      if (landing.follow) {
        const { x, z } = landing.follow()
        this.stains.blot(x, this.groundAt(x, z), z, landing.radius, landing.spread)
      } else this.stains.blot(landing.x, landing.y, landing.z, landing.radius, landing.spread)
    }
  }
}
