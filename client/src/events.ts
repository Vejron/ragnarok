/**
 * What happens rather than what is: shots, blasts, crashes and bullets, as the server reports them in
 * each snapshot, and the hits the local player lands. Each is played when the picture reaches the tick
 * it happened on, so it lines up with what is drawn.
 */

import * as THREE from 'three'
import type { Avatars } from '@/entities/avatars'
import type { Vehicles } from '@/entities/presenters/vehicles'
import type { Wear } from '@/entities/presenters/wear'
import type { Blips } from '@/blips'
import { HIT_KILL, HIT_ROOSTER, HIT_VEHICLE } from '@/feedback'
import {
  PROJECTILE_BULLET, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_BULLET, WEAPON_CRASH, WEAPON_DRONE, WEAPON_EGG, WEAPON_FLARE, WEAPON_GRENADE, WEAPON_LAUNCHER, WEAPON_LOITER, WEAPON_MG, WEAPON_MORTAR,
  WEAPON_PELLET, WEAPON_RICOCHET, WEAPON_ROCKET, WEAPON_SCRAPE, WEAPON_SHELL, WEAPON_SHOTGUN, WEAPON_SNIPER, WEAPON_TURRET, WEAPON_WRECK,
  type BulletSpawn, type ShotEvent,
} from '@/net/codec'
import { CarModel } from '@/render/car'
import type { ChaseCam } from '@/render/chase'
import type { DamageNumbers } from '@/render/damagenumbers'
import type { Debris } from '@/render/debris'
import type { Effects } from '@/render/effects'
import { BLASTS, explosion, type ExplosionProfile } from '@/render/explosion'
import type { Rockets } from '@/render/rockets'
import type { Rounds } from '@/render/rounds'
import type { Scene } from '@/render/scene'
import type { ProjectileTrails } from '@/render/trails'
import { crashBurst, scrapeSparks, type FxWorld } from '@/render/tyres'
import { bulletLifeTicks, bulletPosition, bulletTicksTo, bulletVelocity } from '@/sim/ballistics'
import { BOMB_RADIUS, CANNON_RADIUS, EXPLOSION_RADIUS, EYE_HEIGHT, MORTAR_RADIUS, ROCKET_BLAST_RADIUS, TICK_RATE, WRECK_BLAST_RADIUS } from '@/sim/consts'
import { DRONE_BLAST_RADIUS, GRENADE_BLAST_RADIUS, hitscanGunFor, ITEM_SNIPER, itemDef, LOITER_BLAST_RADIUS, type ItemDef } from '@/sim/items'
import type { Surroundings } from '@/surroundings'
import type { VehicleSounds } from '@/vehicleaudio'
import type { WorldView } from '@/worldview'

/** A vehicle with no motion on record. */
const STILL = new THREE.Vector3()
/** Events wait at most this many deep for the picture to reach them. */
const MAX_PENDING = 64
/** From a Carl Gustaf's muzzle back to its venturi, metres: where the backblast leaves. */
const LAUNCHER_LENGTH = 1.05
/** How hard a Carl Gustaf shakes its own shooter's view (`ChaseCam.kick`): trauma 0.7 is about a 17 cm shake. */
const LAUNCHER_KICK = 0.7

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
  offset: THREE.Vector3 | null
}

/** What events are drawn and heard in, and who the local player is. */
export interface EventWorld {
  scene: Scene
  effects: Effects
  sounds: VehicleSounds
  debris: Debris
  damageNumbers: DamageNumbers
  rockets: Rockets
  trails: ProjectileTrails
  /** Machine gun rounds in flight. */
  rounds: Rounds
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

  /** Plays what the picture has reached by `renderTick`, and draws the bullets in flight. */
  play(renderTick: number) {
    let played = 0
    while (played < this.pending.length && this.pending[played]!.tick <= renderTick) this.showShot(this.pending[played++]!.shot)
    if (played > 0) this.pending.splice(0, played)
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

  /** What a hit the local player landed looks like in the world: a number, flying debris and a flash on the target. */
  hit(hit: { kind: number; amount: number; x: number; y: number; z: number; target: number }) {
    const { scene, effects, damageNumbers, avatars } = this.world
    const at = new THREE.Vector3(hit.x, hit.y, hit.z)
    const kind = hit.kind === HIT_KILL ? 'kill' : hit.kind === HIT_VEHICLE ? 'vehicle' : hit.kind === HIT_ROOSTER ? 'rooster' : 'body'
    damageNumbers.spawn(at, hit.amount, kind, scene.camera)
    // Debris flies back towards the shooter, as it would from a round that came in.
    const back = scene.camera.position.clone().sub(at).normalize()
    effects.impact(at, kind === 'kill' ? 'kill' : kind === 'vehicle' ? 'vehicle' : 'body', back)
    avatars.of(hit.target)?.flash()
  }

  /** Forgets everything pending and in flight: a disconnect. */
  clear() {
    this.pending = []
    this.bullets.clear()
    this.seenBullets.clear()
  }

  /** Where a player's rifle leaves: the end of the barrel it is drawn holding, else its eye. */
  private muzzleOf(shooter: number): THREE.Vector3 | null {
    const avatar = this.world.avatars.of(shooter)
    if (!avatar || !avatar.visible) return null
    return avatar.muzzle() ?? avatar.position.clone().add(new THREE.Vector3(0, EYE_HEIGHT, 0))
  }

  private showShot(shot: ShotEvent) {
    const { effects, sounds, avatars, vehicles, view, surroundings, chase, scene } = this.world
    const local = this.world.localId()
    let gun: ItemDef | undefined
    const to = new THREE.Vector3(shot.x, shot.y, shot.z)
    if (shot.weapon === WEAPON_ROCKET) {
      this.blast(to, ROCKET_BLAST_RADIUS, BLASTS.rocket)
      sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_GRENADE) {
      this.blast(to, GRENADE_BLAST_RADIUS, BLASTS.grenade)
      sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_SHELL) {
      this.blast(to, EXPLOSION_RADIUS, BLASTS.shell)
      sounds.blast([to.x, to.y, to.z], 'big')
    } else if (shot.weapon === WEAPON_MORTAR) {
      this.blast(to, MORTAR_RADIUS, BLASTS.shell)
      sounds.blast([to.x, to.y, to.z], 'big')
    } else if (shot.weapon === WEAPON_WRECK) {
      this.blast(to, WRECK_BLAST_RADIUS, BLASTS.wreck)
      sounds.explosion([to.x, to.y, to.z])
    } else if (shot.weapon === WEAPON_DRONE) {
      this.blast(to, DRONE_BLAST_RADIUS, BLASTS.grenade)
      sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_LOITER) {
      this.blast(to, LOITER_BLAST_RADIUS, BLASTS.shell)
      sounds.explosion([to.x, to.y, to.z])
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
      const eye = avatar ? avatar.position.clone().add(new THREE.Vector3(0, EYE_HEIGHT, 0)) : from
      const along = to.clone().sub(eye)
      const forward = along.lengthSq() > 1e-4 ? along.normalize() : new THREE.Vector3(0, 0, -1)
      const back = forward.clone().negate()
      const venturi = from.clone().addScaledVector(back, LAUNCHER_LENGTH)
      effects.muzzleFlash(from, forward)
      effects.blast(venturi, back)
      effects.dust(venturi.clone().addScaledVector(back, 1.5), 2.5)
      this.muzzleLight(from, 0xffb060, 3, 10)
      this.world.blips.fired(shot.shooter, from.x, from.z, performance.now())
      sounds.blast([from.x, from.y, from.z], 'small')
      if (shot.shooter === local) chase.kick(LAUNCHER_KICK)
    } else if (shot.weapon === WEAPON_AUTOCANNON) {
      this.blast(to, CANNON_RADIUS, BLASTS.cannon)
      sounds.blast([to.x, to.y, to.z], 'small')
    } else if (shot.weapon === WEAPON_BULLET) {
      // A bullet ended here; `shooter` is its id. It is drawn until it gets there.
      const bullet = this.bullets.get(shot.shooter)
      if (bullet) bullet.endTicks = bulletTicksTo(bullet.spawn, [shot.x, shot.y, shot.z])
      if (surroundings.watery(to, bullet?.spawn.item === ITEM_SNIPER ? 0.45 : 0.25)) { /* splashed */ }
      else if (bullet?.spawn.item === ITEM_SNIPER) effects.burst(to, 0.8)
      else effects.pelletStrike(to)
      if (bullet?.spawn.shooter === local) view.chipped(to.x, to.y, to.z, itemDef(bullet.spawn.item)?.damage ?? 0)
    } else if (shot.weapon === WEAPON_MG) {
      // The round was sent flying when the gun fired; this is where it strikes something.
      if (!surroundings.watery(to, 0.25)) this.world.rounds.struck(to)
    } else if (shot.weapon === WEAPON_PELLET) {
      // A further pellet of a spread: the blast and the sound came with the first.
      const gun = hitscanGunFor(WEAPON_SHOTGUN)
      const from = this.muzzleOf(shot.shooter)
      if (!gun || (from && from.distanceTo(to) >= gun.range - 1)) return
      if (!surroundings.watery(to, 0.2)) effects.pelletStrike(to)
      if (shot.shooter === local) view.chipped(to.x, to.y, to.z, gun.damage)
    } else if (shot.weapon === WEAPON_BOMB || shot.weapon === WEAPON_EGG) {
      // A hen's egg goes off like a plane's bomb, and as wide.
      this.blast(to, BOMB_RADIUS, BLASTS.bomb)
      sounds.explosion([to.x, to.y, to.z])
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
    } else if ((gun = hitscanGunFor(shot.weapon))) {
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
  private muzzleLight(at: THREE.Vector3, color: number, intensity: number, radius: number) {
    this.world.scene.scene.lights.flash(at.x, at.y, at.z, { color, intensity, radius, life: 0.09 })
  }

  /** A heavy blast: the layered fire, sparks, dust and debris, a flash, and a shove of the camera if it is close. */
  private blast(at: THREE.Vector3, radius: number, profile: ExplosionProfile) {
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
      bullet.offset = muzzle.clone().sub(new THREE.Vector3(at[0], at[1], at[2]))
      this.world.effects.muzzle(muzzle)
    }
    const heard = muzzle ?? new THREE.Vector3(at[0], at[1], at[2])
    this.world.sounds.gunshot([heard.x, heard.y, heard.z], gun?.sound ?? 'rifle')
  }
}
