/**
 * What happens rather than what is: shots, blasts, crashes and bullets, as the server reports them in
 * each snapshot, and the hits the local player lands. Each is played when the picture reaches the tick
 * it happened on, so it lines up with what is drawn.
 */

import { Vector3 } from '@ragnarok/engine'
import type { Avatars } from '@/entities/avatars'
import type { Vehicles } from '@/entities/presenters/vehicles'
import type { Wear } from '@/entities/presenters/wear'
import type { Blips } from '@/blips'
import { HIT_KILL, HIT_ROOSTER, HIT_VEHICLE } from '@/feedback'
import {
  PROJECTILE_BULLET, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_BULLET, WEAPON_CRASH, WEAPON_DRONE, WEAPON_EGG, WEAPON_FLARE, WEAPON_GRENADE, WEAPON_LASER, WEAPON_LAUNCHER, WEAPON_LOITER, WEAPON_MG, WEAPON_MORTAR,
  MARK_GLANCE, MARK_PIERCED, MARK_SCUFF, MARK_VITAL, WEAPON_AP, WEAPON_PELLET, WEAPON_RICOCHET, WEAPON_ROCKET, WEAPON_SCRAPE, WEAPON_SHELL, WEAPON_SHOTGUN, WEAPON_SNIPER, WEAPON_TURRET, WEAPON_WRECK,
  type BulletSpawn, type HullMark, type ShotEvent,
} from '@/net/codec'
import { CarModel } from '@/render/car'
import type { ChaseCam } from '@/render/chase'
import type { DamageNumbers } from '@/render/damagenumbers'
import type { Debris } from '@/render/debris'
import type { Effects } from '@/render/effects'
import type { Scorchmarks } from '@/render/scorchmarks'
import { locate, type HullMarks } from '@/render/hullmarks'
import { BLASTS, explosion, type ExplosionProfile } from '@/render/explosion'
import type { Rockets } from '@/render/rockets'
import type { Rounds } from '@/render/rounds'
import type { Scene } from '@/render/scene'
import { TuppModel } from '@/render/tupp'
import type { ProjectileTrails } from '@/render/trails'
import { crashBurst, scrapeSparks, type FxWorld } from '@/render/tyres'
import { bulletLifeTicks, bulletPosition, bulletTicksTo, bulletVelocity } from '@/sim/ballistics'
import { AP_BLAST_RADIUS, BOMB_RADIUS, CANNON_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, MORTAR_RADIUS, ROCKET_BLAST_RADIUS, TICK_RATE, TUPP_LASER_DAMAGE, TUPP_LASER_RANGE, WRECK_BLAST_RADIUS } from '@/sim/consts'
import { DRONE_BLAST_RADIUS, GRENADE_BLAST_RADIUS, hitscanGunFor, ITEM_SNIPER, itemDef, LOITER_BLAST_RADIUS } from '@/sim/items'
import type { Surroundings } from '@/surroundings'
import type { VehicleSounds } from '@/vehicleaudio'
import type { WorldView } from '@/worldview'

/** A vehicle with no motion on record. */
const STILL = new Vector3()
/** A laser's burn on the ground, metres across, and how far above the ground a hit still counts as on it. */
const SCORCH_RADIUS = 1.4
const SCORCH_HEIGHT = 0.6
/** Events wait at most this many deep for the picture to reach them. */
const MAX_PENDING = 64
/** From a Carl Gustaf's muzzle back to its venturi, metres: where the backblast leaves. */
const LAUNCHER_LENGTH = 1.05
/** How hard a Carl Gustaf shakes its own shooter's view (`ChaseCam.kick`): trauma 0.7 is about a 17 cm shake. */
const LAUNCHER_KICK = 0.7

/** Weapons that only go off where they land: how wide, how it looks and what is heard. */
const BLAST_OF: ReadonlyMap<number, { radius: number; profile: ExplosionProfile; sound: 'small' | 'big' | 'explosion' }> = new Map([
  [WEAPON_ROCKET, { radius: ROCKET_BLAST_RADIUS, profile: BLASTS.rocket, sound: 'small' }],
  [WEAPON_GRENADE, { radius: GRENADE_BLAST_RADIUS, profile: BLASTS.grenade, sound: 'small' }],
  [WEAPON_SHELL, { radius: EXPLOSION_RADIUS, profile: BLASTS.shell, sound: 'big' }],
  // A solid shot's little charge: a sharp crack rather than a fireball.
  [WEAPON_AP, { radius: AP_BLAST_RADIUS, profile: BLASTS.cannon, sound: 'small' }],
  [WEAPON_MORTAR, { radius: MORTAR_RADIUS, profile: BLASTS.shell, sound: 'big' }],
  [WEAPON_WRECK, { radius: WRECK_BLAST_RADIUS, profile: BLASTS.wreck, sound: 'explosion' }],
  [WEAPON_DRONE, { radius: DRONE_BLAST_RADIUS, profile: BLASTS.grenade, sound: 'small' }],
  [WEAPON_LOITER, { radius: LOITER_BLAST_RADIUS, profile: BLASTS.shell, sound: 'explosion' }],
  [WEAPON_AUTOCANNON, { radius: CANNON_RADIUS, profile: BLASTS.cannon, sound: 'small' }],
  // A hen's egg goes off like a plane's bomb, and as wide.
  [WEAPON_BOMB, { radius: BOMB_RADIUS, profile: BLASTS.bomb, sound: 'explosion' }],
  [WEAPON_EGG, { radius: BOMB_RADIUS, profile: BLASTS.bomb, sound: 'explosion' }],
])

/** A bullet in flight on this client. */
interface LiveBullet {
  spawn: BulletSpawn
  /** Ticks it flies before it gives up. */
  life: number
  /** Ticks after firing at which its end event says it stopped, once known. */
  endTicks: number | null
  /** Its first frame has played: arm, flash and sound. */
  shown: boolean
  /** Muzzle minus eye, closing as it flies. */
  offset: Vector3 | null
}

/** What events are drawn and heard in, and who the local player is. */
export interface EventWorld {
  scene: Scene
  effects: Effects
  scorches: Scorchmarks
  sounds: VehicleSounds
  debris: Debris
  damageNumbers: DamageNumbers
  rockets: Rockets
  trails: ProjectileTrails
  /** Machine gun rounds in flight. */
  rounds: Rounds
  /** Holes and scuffs on armoured vehicles. */
  hullMarks: HullMarks
  blips: Blips
  avatars: Avatars
  vehicles: Vehicles
  wear: Wear
  /** The buildings and trees, which a local player's rounds chip. */
  view: WorldView
  chase: ChaseCam
  surroundings: Surroundings
  fx: () => FxWorld
  localId: () => number | null
  /** The vehicle the local player drives, if any. */
  driving: () => number | null
}

export class Events {
  /** Shot and burst events wait here until the render tick reaches them, so they line up with what is drawn. */
  private pending: { tick: number; shot: ShotEvent }[] = []
  /** Marks rounds left on vehicles' armour, waiting likewise. */
  private marks: { tick: number; mark: HullMark }[] = []
  /** Bullets in flight, flown from their spawn alone (`sim/ballistics.ts`); the server lists none after announcing them. */
  private readonly bullets = new Map<number, LiveBullet>()
  /** Bullets already seen, with the tick they were fired on: a spawn is sent twice and must not start a second round. */
  private readonly seenBullets = new Map<number, number>()

  constructor(private readonly world: EventWorld) {}

  /** Takes snapshot `tick`'s shots and bullets. */
  receive(tick: number, shots: readonly ShotEvent[], bullets: readonly BulletSpawn[]) {
    for (const shot of shots) this.pending.push({ tick, shot })
    if (this.pending.length > MAX_PENDING) this.pending.splice(0, this.pending.length - MAX_PENDING)
    this.noteBullets(tick, bullets)
  }

  /** Marks left on vehicles' armour as of `tick` (`S_MARKS`), to show when the picture gets there. */
  receiveMarks(tick: number, marks: readonly HullMark[]) {
    for (const mark of marks) this.marks.push({ tick, mark })
    if (this.marks.length > MAX_PENDING) this.marks.splice(0, this.marks.length - MAX_PENDING)
  }

  /** Plays what the picture has reached by `renderTick`, and draws the bullets in flight. */
  play(renderTick: number) {
    let played = 0
    while (played < this.pending.length && this.pending[played]!.tick <= renderTick) this.showShot(this.pending[played++]!.shot)
    if (played > 0) this.pending.splice(0, played)
    played = 0
    while (played < this.marks.length && this.marks[played]!.tick <= renderTick) this.showMark(this.marks[played++]!.mark)
    if (played > 0) this.marks.splice(0, played)
  }

  /**
   * A round meeting a vehicle's armour: sparks off it, a scuff where the plate stopped it, a hole and
   * fragments where it went through. The server says where in the vehicle's own frame and which way the
   * round went, so it is put on the vehicle as it is drawn now.
   */
  private showMark(mark: HullMark) {
    const { effects, sounds, vehicles } = this.world
    const placed = vehicles.placed.get(mark.vehicle)
    const model = vehicles.model(mark.vehicle)
    if (!placed || !model) return
    const struck = locate(placed.kind, model.root, mark, { x: mark.dx / 127, y: mark.dy / 127, z: mark.dz / 127 })
    const at: [number, number, number] = [struck.world.x, struck.world.y, struck.world.z]
    const style = mark.style & ~MARK_VITAL
    if (style === MARK_GLANCE) {
      effects.strike(struck.world)
      sounds.ricochet(at)
      return
    }
    const pierced = style !== MARK_SCUFF
    const big = style === MARK_PIERCED
    if (pierced) effects.pierce(struck.world, struck.outward, big)
    else effects.deflect(struck.world, struck.outward)
    this.world.hullMarks.mark(mark.vehicle, model.root, struck, pierced, big ? 1.2 : pierced ? 0.45 : 0.5, (mark.style & MARK_VITAL) !== 0)
    if (pierced) sounds.pierce(at, big)
    else sounds.ricochet(at)
  }

  /** Draws every bullet where it has got to by `renderTick`, and ends the ones that have arrived. */
  drawBullets(renderTick: number) {
    const { rockets, trails } = this.world
    for (const [id, bullet] of this.bullets) {
      const n = renderTick - bullet.spawn.tick
      if (n < 0) continue
      if (n >= Math.min(bullet.life, bullet.endTicks ?? Infinity)) {
        rockets.remove(id)
        trails.end('b', id)
        this.bullets.delete(id)
        continue
      }
      const at = bulletPosition(bullet.spawn, n)
      if (!bullet.shown) {
        bullet.shown = true
        this.bulletLeft(bullet, at)
      }
      // It leaves the eye, but is drawn leaving the barrel: the difference closes over the first 25 m.
      if (bullet.offset) {
        const flown = n * Math.hypot(bullet.spawn.vx, bullet.spawn.vy, bullet.spawn.vz) / TICK_RATE
        const keep = Math.max(0, 1 - flown / 25)
        at[0] += bullet.offset.x * keep
        at[1] += bullet.offset.y * keep
        at[2] += bullet.offset.z * keep
      }
      rockets.place(id, PROJECTILE_BULLET, at, bulletVelocity(bullet.spawn, n))
      // The ribbon starts where the round is first drawn, so it grows out of the barrel.
      trails.follow('b', id, PROJECTILE_BULLET, at[0], at[1], at[2])
    }
  }

  /**
   * What a hit the local player landed looks like in the world: a number on the spot, and a pinpoint
   * flash there on a soldier (whose blood comes from `struck`) or flying debris off anything else.
   */
  hit(hit: { kind: number; amount: number; x: number; y: number; z: number; target: number }) {
    const { scene, effects, damageNumbers, avatars } = this.world
    const at = new Vector3(hit.x, hit.y, hit.z)
    const kind = hit.kind === HIT_KILL ? 'kill' : hit.kind === HIT_VEHICLE ? 'vehicle' : hit.kind === HIT_ROOSTER ? 'rooster' : 'body'
    damageNumbers.spawn(at, hit.amount, kind, scene.camera)
    const soldier = avatars.of(hit.target)
    if (soldier && (kind === 'body' || kind === 'kill')) {
      // The blood comes from `struck`, as everyone sees it; the shooter also sees the very spot.
      effects.struck(at)
      soldier.flash()
      return
    }
    // Debris flies back towards the shooter, as it would from a round that came in.
    const back = scene.camera.position.clone().sub(at).normalize()
    effects.impact(at, kind === 'kill' ? 'kill' : kind === 'vehicle' ? 'vehicle' : 'body', back)
  }

  /** Forgets everything pending and in flight: a disconnect. */
  clear() {
    this.pending = []
    this.marks = []
    this.bullets.clear()
    this.seenBullets.clear()
  }

  /** Where a player's rifle leaves: the end of the barrel it is drawn holding, else its eye. */
  private muzzleOf(shooter: number): Vector3 | null {
    const avatar = this.world.avatars.of(shooter)
    if (!avatar || !avatar.visible) return null
    return avatar.muzzle() ?? avatar.position.clone().add(new Vector3(0, EYE_HEIGHT, 0))
  }

  private showShot(shot: ShotEvent) {
    const { effects, scorches, sounds, avatars, vehicles, view, surroundings, chase, scene } = this.world
    const local = this.world.localId()
    const to = new Vector3(shot.x, shot.y, shot.z)
    const blast = BLAST_OF.get(shot.weapon)
    if (blast) {
      this.blast(to, blast.radius, blast.profile)
      const at: [number, number, number] = [to.x, to.y, to.z]
      if (blast.sound === 'explosion') sounds.explosion(at)
      else sounds.blast(at, blast.sound)
    } else if (shot.weapon === WEAPON_FLARE) {
      // The flare itself is a projectile; this is the gun going off.
      avatars.of(shot.shooter)?.fire()
      const from = this.muzzleOf(shot.shooter) ?? to
      effects.muzzle(from)
      this.muzzleLight(from, 0xffa060, 2.5, 8)
      sounds.gunshot([from.x, from.y, from.z], 'revolver')
    } else if (shot.weapon === WEAPON_LAUNCHER) {
      // A Carl Gustaf going off: the rocket is a projectile of its own; this is the flash at the
      // muzzle and the backblast out of the venturi behind the shooter's shoulder.
      const avatar = avatars.of(shot.shooter)
      avatar?.fire()
      const from = this.muzzleOf(shot.shooter) ?? to
      const eye = avatar ? avatar.position.clone().add(new Vector3(0, EYE_HEIGHT, 0)) : from
      const along = to.clone().sub(eye)
      const forward = along.lengthSq() > 1e-4 ? along.normalize() : new Vector3(0, 0, -1)
      const back = forward.clone().negate()
      const venturi = from.clone().addScaledVector(back, LAUNCHER_LENGTH)
      effects.muzzleFlash(from, forward)
      effects.blast(venturi, back)
      effects.dust(venturi.clone().addScaledVector(back, 1.5), 2.5)
      this.muzzleLight(from, 0xffb060, 3, 10)
      this.world.blips.fired(shot.shooter, from.x, from.z, performance.now())
      sounds.blast([from.x, from.y, from.z], 'small')
      if (shot.shooter === local) chase.kick(LAUNCHER_KICK)
    } else if (shot.weapon === WEAPON_BULLET) {
      // A bullet ended here; `shooter` is its id. It is drawn until it gets there.
      const bullet = this.bullets.get(shot.shooter)
      if (bullet) bullet.endTicks = bulletTicksTo(bullet.spawn, [shot.x, shot.y, shot.z])
      const sniper = bullet?.spawn.item === ITEM_SNIPER
      if (!surroundings.watery(to, sniper ? 0.45 : 0.25)) {
        if (sniper) effects.burst(to, 0.8)
        else effects.pelletStrike(to)
      }
      if (bullet?.spawn.shooter === local) view.chipped(to.x, to.y, to.z, itemDef(bullet.spawn.item)?.damage ?? 0)
    } else if (shot.weapon === WEAPON_LASER) {
      // The beam is drawn every frame, from the eyes; this is where it ends, and what it does there.
      const tupp = vehicles.model(shot.shooter)
      if (!(tupp instanceof TuppModel)) return
      tupp.beamEnd = to
      if (this.world.driving() === shot.shooter) chase.kick(0.08)
      else if (to.distanceTo(scene.camera.position) < 30) chase.kick(0.05)
      if (tupp.barrel().origin.distanceTo(to) >= TUPP_LASER_RANGE - 1 || surroundings.watery(to, 0.25)) {
        tupp.scorched = null
        return
      }
      effects.laserBurn(to)
      // A burn only where it struck the ground itself, not a hull or a wall, and joined to the one before.
      const onGround = to.y - scene.groundAt(to.x, to.z) < SCORCH_HEIGHT
      if (onGround) scorches.sweep(tupp.scorched, to, SCORCH_RADIUS)
      tupp.scorched = onGround ? to.clone() : null
      this.muzzleLight(to, 0xff4020, 3, 10)
      sounds.laserSizzle([to.x, to.y, to.z])
      if (this.world.driving() === shot.shooter) view.chipped(to.x, to.y, to.z, TUPP_LASER_DAMAGE)
    } else if (shot.weapon === WEAPON_MG) {
      // The round was sent flying when the gun fired; this is where it strikes something.
      if (!surroundings.watery(to, 0.25)) this.world.rounds.struck(to)
    } else if (shot.weapon === WEAPON_PELLET) {
      // A further pellet of a spread: the blast and the sound came with the first.
      const pellets = hitscanGunFor(WEAPON_SHOTGUN)
      const from = this.muzzleOf(shot.shooter)
      if (!pellets || (from && from.distanceTo(to) >= pellets.range - 1)) return
      if (!surroundings.watery(to, 0.2)) effects.pelletStrike(to)
      if (shot.shooter === local) view.chipped(to.x, to.y, to.z, pellets.damage)
    } else if (shot.weapon === WEAPON_RICOCHET) {
      // A shell or round glanced off armor: sparks and a ping, and no blast.
      effects.strike(to)
      sounds.ricochet([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_TURRET) {
      this.world.wear.shedTurret(shot.shooter)
      this.blast(to, CANNON_RADIUS, BLASTS.cannon)
      sounds.turretOff([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_CRASH) {
      const car = vehicles.model(shot.shooter)
      const velocity = vehicles.fx(shot.shooter)?.velocity ?? STILL
      if (car instanceof CarModel) car.noteImpact(to)
      crashBurst(this.world.fx(), to, car instanceof CarModel ? car.paintColor : 0x555555, velocity)
      sounds.crash([to.x, to.y, to.z], vehicles.fx(shot.shooter)?.impactSpeed ?? 0)
      // Felt hard by its driver, a little by anyone close by.
      if (this.world.driving() === shot.shooter) chase.kick(0.7)
      else if (to.distanceTo(scene.camera.position) < 20) chase.kick(0.15)
    } else if (shot.weapon === WEAPON_SCRAPE) {
      scrapeSparks(this.world.fx(), to, vehicles.fx(shot.shooter)?.velocity ?? STILL)
      sounds.scrape(shot.shooter)
    } else if (shot.weapon === WEAPON_SNIPER) {
      // The round is drawn in flight; this is where it landed.
      if (!surroundings.watery(to, 0.45)) effects.burst(to, 0.8)
    } else {
      const gun = hitscanGunFor(shot.weapon)
      if (!gun) return
      const from = this.muzzleOf(shot.shooter)
      avatars.of(shot.shooter)?.fire()
      if (from) {
        this.world.blips.fired(shot.shooter, from.x, from.z, performance.now())
        const landed = from.distanceTo(to) < gun.range - 1
        if (gun.pellets > 1) {
          // No rays for a spread: a blast at the muzzle, then each pellet is drawn where it landed.
          effects.blast(from, to.clone().sub(from).normalize())
          if (landed && !surroundings.watery(to, 0.2)) effects.pelletStrike(to)
        } else {
          effects.tracer(from, to, landed)
          if (landed) surroundings.watery(to, 0.25)
        }
        this.muzzleLight(from, 0xffd890, 1.6, 6)
      }
      const at = from ?? to
      sounds.gunshot([at.x, at.y, at.z], gun.sound)
      if (shot.shooter === local && from && from.distanceTo(to) < gun.range - 1) view.chipped(to.x, to.y, to.z, gun.damage)
    }
  }

  /** A short flash of light at a gun's muzzle: it lights the barrel, the hull and the ground in front of it. */
  private muzzleLight(at: Vector3, color: number, intensity: number, radius: number) {
    this.world.scene.scene.lights.flash(at.x, at.y, at.z, { color, intensity, radius, life: 0.09 })
  }

  /** A heavy blast: the layered fire, sparks, dust and debris, a flash, and a shove of the camera if it is close. */
  private blast(at: Vector3, radius: number, profile: ExplosionProfile) {
    const { scene, effects, chase, surroundings } = this.world
    // On water it is a column of spray, not fire and dirt.
    if (surroundings.watery(at, radius * 0.45, 14)) {
      const distance = at.distanceTo(scene.camera.position)
      if (distance < radius * 8) chase.kick(Math.min(0.4, (radius / 10) * (1 - distance / (radius * 8))))
      return
    }
    effects.burst(at, radius)
    explosion(this.world.fx(), this.world.debris, at, radius, profile)
    // A flash that lights the ground and what stands on it, and a shockwave racing out over the ground.
    const reach = Math.min(2, Math.max(0.6, radius / 5))
    scene.scene.lights.flash(at.x, at.y + 1.5, at.z, { color: 0xffa040, intensity: 5 * reach, radius: radius * 8, life: 0.6 })
    const ground = scene.groundAt(at.x, at.z)
    if (at.y - ground < radius) {
      effects.shockwave({ x: at.x, y: ground + 0.25, z: at.z }, radius)
      surroundings.grass?.blast(at.x, at.z, radius)
    }
    const distance = at.distanceTo(scene.camera.position)
    if (distance < radius * 8) chase.kick(Math.min(0.6, (radius / 8) * (1 - distance / (radius * 8))))
  }

  /** Starts the bullets of a snapshot that are new; each is sent in two, in case one is lost. */
  private noteBullets(tick: number, spawns: readonly BulletSpawn[]) {
    for (const spawn of spawns) {
      if (this.seenBullets.has(spawn.id)) continue
      this.seenBullets.set(spawn.id, spawn.tick)
      this.world.blips.fired(spawn.shooter, spawn.x, spawn.z, performance.now())
      this.bullets.set(spawn.id, { spawn, life: bulletLifeTicks(spawn), endTicks: null, shown: false, offset: null })
    }
    if (this.seenBullets.size > 256) {
      for (const [id, fired] of this.seenBullets) if (tick - fired > 300) this.seenBullets.delete(id)
    }
  }

  /** The moment a bullet is first drawn: the shooter's arm, a flash at the muzzle and the shot's sound. */
  private bulletLeft(bullet: LiveBullet, at: readonly number[]) {
    const { shooter, item } = bullet.spawn
    const gun = itemDef(item)
    this.world.avatars.of(shooter)?.fire()
    const muzzle = this.muzzleOf(shooter)
    if (muzzle) {
      bullet.offset = muzzle.clone().sub(new Vector3(at[0], at[1], at[2]))
      this.world.effects.muzzle(muzzle)
    }
    const heard = muzzle ?? new Vector3(at[0], at[1], at[2])
    this.world.sounds.gunshot([heard.x, heard.y, heard.z], gun?.sound ?? 'rifle')
  }
}
