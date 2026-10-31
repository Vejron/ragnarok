/**
 * Where the camera goes while the local player sits in a vehicle: one rig per way of following
 * (`Rig` in `seats.ts`), picked by the seat. A rig is entered when the player takes the seat (it turns
 * the look to where the seat points), placed every frame, and left when they get up, so what each
 * remembers between frames is its own rather than a flag per seat in the game loop.
 */

import { Quaternion, Vector3 } from '@ragnarok/engine'
import { clamp } from '@/math'
import { wrapAngle } from '@/net/codec'
import type { Placed } from '@/entities/presenters/vehicles'
import type { Turret } from '@/entities/units'
import type { VehicleModel } from '@/render/car'
import type { ChaseView } from '@/render/chase'
import type { Mark } from '@/render/gunsight'
import { PlaneModel } from '@/render/plane'
import { CAMERA_HEIGHT, PLANE_CAMERA_HEIGHT, PLANE_DISTANCE, STRV_AIM_BACK, STRV_AIM_UP, WALKER_AIM_BACK, WALKER_AIM_UP, AIM_DISTANCE_SCALE, type Scene } from '@/render/scene'
import { StrvModel } from '@/render/strvs'
import { TankModel } from '@/render/tank'
import type { VehicleFx } from '@/render/tyres'
import { TuppModel } from '@/render/tupp'
import { WalkerModel } from '@/render/walker'
import { GRAVITY, PASSENGER_DISTANCE, PASSENGER_HEIGHT, PASSENGER_SHOULDER, PLANE_AIM_MAX_PITCH, PLANE_CRUISE_SPEED, SHELL_SPEED, STRV_BARREL_MAX, STRV_BARREL_MIN, TUPP_HEAD_LIMIT, TUPP_HEAD_PITCH_MAX, TUPP_HEAD_PITCH_MIN, WALKER_GUN_PITCH_MAX, WALKER_GUN_PITCH_MIN, WALKER_TORSO_LIMIT } from '@/sim/consts'
import type { InputController } from '@/sim/input'
import type { Rig, SeatView } from '@/seats'

/** How quickly a plane's camera follows the mouse's aim, per second, how far it pulls back at cruise speed (m) and how much of the plane's roll it takes. */
const PLANE_VIEW_EASE = 5
const PLANE_PULLBACK = 3
const PLANE_VIEW_ROLL = 0.3
/** How far behind and above a roof gun the camera sits: over the gunner's shoulder. */
const ROOF_AIM_BACK = 3.5
const ROOF_AIM_UP = 1.2
/** The sight sits this far above the gun's pivot, on the mantlet. */
const SIGHT_EYE_UP = 0.5
/** Ranges (m) the sight's ladder is marked at. */
const SIGHT_RANGES = [100, 150, 200, 250, 300, 350, 400, 450, 500]
const SIGHT_RAY_START = 6
const SIGHT_RANGE_MAX = 1500

/** A mounted gun the camera can sit behind: a tank's, a helicopter's nose gun, a plane's ring, a Pak's. */
interface Barrel {
  root: VehicleModel['root']
  barrel(): { origin: Vector3; direction: Vector3 }
}
const hasBarrel = (model: VehicleModel | undefined): model is VehicleModel & Barrel => model !== undefined && 'barrel' in model

/** What a rig is told each frame about the seat the local player is in. */
export interface Seated {
  now: number
  dt: number
  vehicle: number
  seat: SeatView
  ride: Placed
  model: VehicleModel | undefined
  motion: VehicleFx | undefined
  /** How far behind the vehicle the camera sits. */
  distance: number
  /** Aim held at a gun with a sight. */
  sighted: boolean
  /** Looking down a scoped gun in hand. */
  scoped: boolean
  /** How far the camera has eased in over the shoulder, 0..1. */
  aimBlend: number
  /** The chase camera's lean, distance and shake, for a seat that has it. */
  chase: ChaseView | null
  /** Where the local player's head is, when they show in their seat. */
  head: Vector3 | null
}

type RigName = Rig['name']

interface RigImpl {
  /** The player takes the seat. */
  enter?(c: Seated): void
  frame(c: Seated): void
  /** The player leaves the seat. */
  leave?(now: number): void
}

export class Cameras {
  /** Which of a loitering drone's two cameras its pilot watches; aim (right-click) swaps them. */
  loiterCam: 'forward' | 'down' = 'forward'
  /** `performance.now()` when the local player's drone feed last cut out, for a burst of static. */
  feedLostAt = -Infinity
  private wasAimHeld = false
  /** The seat the camera is in, by vehicle and seat, and its rig. */
  private current: { key: string; name: RigName; rig: RigImpl } | null = null
  private readonly rigs: Record<RigName, RigImpl>

  constructor(
    private readonly scene: Scene,
    private readonly input: InputController,
    private readonly castRay: (origin: [number, number, number], direction: [number, number, number], max: number) => { t: number } | null,
  ) {
    this.rigs = { drive: this.drive(), gun: this.gun(), fpv: this.fpv(), aviator: this.aviator(), tanker: this.tanker(), walker: this.walker(), pintle: this.pintle(), mortar: this.mortar(), passenger: this.passenger() }
  }

  /** The rig the camera is in, if the local player is seated. */
  get rig(): RigName | null {
    return this.current?.name ?? null
  }

  /**
   * The lens for the frame: a drone's (its belly camera, on a loitering drone's pilot's fresh aim
   * press), a gun's sight, a scope, or aiming over the shoulder.
   */
  zoom(seat: SeatView | null, aimHeld: boolean, sighted: boolean, scoped: boolean, aiming: boolean): 'none' | 'aim' | 'scope' | 'sight' | 'fpv' | 'down' {
    const fpv = seat?.rig.name === 'fpv' ? seat.rig : null
    if (fpv?.belly && aimHeld && !this.wasAimHeld) this.loiterCam = this.loiterCam === 'forward' ? 'down' : 'forward'
    this.wasAimHeld = aimHeld
    if (fpv) return fpv.belly && this.loiterCam === 'down' ? 'down' : 'fpv'
    return sighted ? 'sight' : scoped ? 'scope' : aiming ? 'aim' : 'none'
  }

  /** Places the camera for the local player in seat `index` of vehicle `c.vehicle`. */
  seated(c: Seated, index: number) {
    const key = `${c.vehicle}:${index}`
    const name = c.seat.rig.name
    if (this.current?.key !== key || this.current.name !== name) {
      this.leave(c.now)
      this.current = { key, name, rig: this.rigs[name] }
      this.current.rig.enter?.(c)
    }
    this.scene.gunsight.hide()
    this.current.rig.frame(c)
  }

  /** The local player is not seated: whatever rig they were in is left. */
  leave(now: number) {
    this.current?.rig.leave?.(now)
    this.current = null
    this.scene.gunsight.hide()
  }

  /** Behind the vehicle, easing round behind it when the mouse rests; the chase camera leans, pulls back and shakes. */
  private drive(): RigImpl {
    return {
      frame: ({ ride, dt, distance, chase }) => {
        this.input.followYaw(ride.yaw, dt)
        const yaw = this.input.yaw + (chase?.lean ?? 0)
        this.scene.follow([ride.x, ride.y + 0.4, ride.z], yaw, this.input.pitch, distance + (chase?.distance ?? 0), 0, CAMERA_HEIGHT, chase?.shake)
      },
    }
  }

  /** Behind and above a mounted gun, looking along the barrel's line, and kept within its travel. */
  private gun(): RigImpl {
    let aimed = false
    return {
      enter: () => {
        aimed = false
      },
      frame: (c) => {
        const { ride, model, seat, sighted } = c
        if (seat.rig.name !== 'gun' || !hasBarrel(model)) return this.rigs.drive.frame(c)
        const rig = seat.rig
        if (!aimed && ride.turret) {
          // Take the seat looking where the barrel points.
          if ('level' in rig) {
            this.input.yaw = ride.yaw + ride.turret.yaw
            this.input.pitch = ride.turret.pitch
          } else {
            this.lookAlong(model, ride.turret)
          }
          aimed = true
        }
        if ('level' in rig) this.input.clampPitch(rig.level[0], rig.level[1])
        else this.limitGun(model, rig.hull[0], rig.hull[1], rig.hull[2])
        const { origin } = model.barrel()
        if (sighted) this.lookDownBarrel(model)
        else this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch)
      },
    }
  }

  /** Through a drone's camera: the look is where it heads, so it turns to follow, and the view banks with it. */
  private fpv(): RigImpl {
    let hidden: VehicleModel | null = null
    return {
      enter: ({ ride }) => {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
        this.loiterCam = 'forward'
      },
      frame: ({ ride, model, seat }) => {
        if (seat.rig.name !== 'fpv') return
        const belly = seat.rig.belly === true
        this.input.clampPitch(seat.rig.pitch[0], seat.rig.pitch[1])
        let bank = 0
        // A loitering drone's cameras sit in its nose and under its belly; an FPV drone's is at its centre.
        const eye = new Vector3(0, belly ? (this.loiterCam === 'down' ? -0.35 : 0) : 0.05, belly && this.loiterCam === 'forward' ? -1.7 : 0)
        if (model) {
          const right = new Vector3(1, 0, 0).applyQuaternion(model.root.quaternion)
          const top = new Vector3(0, 1, 0).applyQuaternion(model.root.quaternion)
          bank = -Math.atan2(-right.y, top.y)
          eye.applyQuaternion(model.root.quaternion)
          // Its own frame would fill the lens.
          model.root.visible = false
          hidden = model
        }
        const at = [ride.x + eye.x, ride.y + eye.y, ride.z + eye.z]
        // The belly camera looks straight down with the drone's heading up the screen; it does not bank.
        if (belly && this.loiterCam === 'down') this.scene.downFrom(at, ride.yaw)
        else this.scene.fpvFrom(at, this.input.yaw, this.input.pitch, bank)
      },
      leave: (now) => {
        this.feedLostAt = now
        // The link is gone but the drone may still be falling: it shows again.
        if (hidden) hidden.root.visible = true
        hidden = null
      },
    }
  }

  /**
   * Behind a plane. The mouse is the aim and moves at once; the plane turns to follow it, and the camera
   * follows the aim at an easy pace, so the circle drifts across the screen as it catches up.
   */
  private aviator(): RigImpl {
    const view: [number, number] = [0, 0]
    return {
      enter: ({ ride }) => {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
        view[0] = ride.yaw
        view[1] = 0
      },
      frame: ({ ride, model, motion, dt }) => {
        this.input.clampPitch(-PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)
        const ease = 1 - Math.exp(-PLANE_VIEW_EASE * dt)
        view[0] += wrapAngle(this.input.yaw - view[0]) * ease
        view[1] += (this.input.pitch - view[1]) * ease
        // A little further back the faster it goes, and only part of the plane's roll reaches the view.
        const fast = Math.min(1, Math.max(0, (motion?.forwardSpeed ?? 0) / PLANE_CRUISE_SPEED))
        this.scene.follow([ride.x, ride.y, ride.z], view[0], view[1], PLANE_DISTANCE + PLANE_PULLBACK * fast, 0, PLANE_CAMERA_HEIGHT)
        if (model instanceof PlaneModel) {
          const right = new Vector3(1, 0, 0).applyQuaternion(model.root.quaternion)
          const top = new Vector3(0, 1, 0).applyQuaternion(model.root.quaternion)
          this.scene.camera.rotateZ(-Math.atan2(-right.y, top.y) * PLANE_VIEW_ROLL)
        }
      },
    }
  }

  /**
   * Behind a Strv S's gun, which points along the hull that A/D turn. The mouse looks freely around it
   * and the view eases back behind the barrel when it rests, so the middle of the screen is where the
   * gun points again.
   */
  private tanker(): RigImpl {
    return {
      enter: ({ ride }) => {
        this.input.yaw = ride.yaw
      },
      frame: ({ ride, model, sighted, dt, distance }) => {
        if (!sighted) this.input.followYaw(ride.yaw, dt)
        this.input.clampPitch(STRV_BARREL_MIN, STRV_BARREL_MAX)
        if (model instanceof StrvModel) {
          const { origin } = model.barrel()
          if (sighted) this.input.yaw = this.lookDownBarrel(model)
          else this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch, STRV_AIM_BACK, STRV_AIM_UP)
        } else {
          this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, distance, 0, CAMERA_HEIGHT)
        }
      },
    }
  }

  /** Behind a walker's guns or a tupp's eyes: the look is the aim, free of the legs' heading, within the upper body's or the head's travel of it. */
  private walker(): RigImpl {
    return {
      enter: ({ ride }) => {
        this.input.yaw = ride.yaw
        this.input.pitch = 0
      },
      frame: ({ ride, model, distance }) => {
        if (model instanceof WalkerModel || model instanceof TuppModel) {
          if (model instanceof WalkerModel) this.limitGun(model, WALKER_TORSO_LIMIT, WALKER_GUN_PITCH_MIN, WALKER_GUN_PITCH_MAX)
          else this.limitGun(model, TUPP_HEAD_LIMIT, TUPP_HEAD_PITCH_MIN, TUPP_HEAD_PITCH_MAX)
          // Behind and above the guns, looking along their line, so the middle of the screen is where they point.
          const { origin } = model.barrel()
          this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch, WALKER_AIM_BACK, WALKER_AIM_UP)
        } else {
          this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, distance, 0, CAMERA_HEIGHT)
        }
      },
    }
  }

  /** Behind a gun on a turret's roof, looking along its line, and kept within its travel of the turret, which pushes the look along when it swings away. */
  private pintle(): RigImpl {
    let aimed = false
    return {
      enter: () => {
        aimed = false
      },
      frame: (c) => {
        const { ride, model, seat } = c
        const barrel = model instanceof TankModel ? model.pintleBarrel() : null
        if (seat.rig.name !== 'pintle' || !barrel || !ride.turret) return this.rigs.drive.frame(c)
        const turret = ride.yaw + ride.turret.yaw
        if (!aimed && ride.pintle) {
          // Take the seat looking where the gun points.
          this.input.yaw = turret + ride.pintle.yaw
          this.input.pitch = ride.pintle.pitch
          aimed = true
        }
        this.input.yaw = turret + clamp(wrapAngle(this.input.yaw - turret), -seat.rig.limit, seat.rig.limit)
        this.input.clampPitch(seat.rig.pitch[0], seat.rig.pitch[1])
        const { origin } = barrel
        this.scene.aimFrom([origin.x, origin.y, origin.z], this.input.yaw, this.input.pitch, ROOF_AIM_BACK, ROOF_AIM_UP)
      },
    }
  }

  /** Behind a field mortar: the mouse sets the bearing, the mount turns to face it, and W and S set the range. */
  private mortar(): RigImpl {
    return {
      enter: ({ ride }) => {
        if (ride.turret) this.input.yaw = ride.yaw + ride.turret.yaw
      },
      frame: ({ ride, distance }) => {
        this.scene.follow([ride.x, ride.y + 0.4, ride.z], this.input.yaw, this.input.pitch, distance, 0, CAMERA_HEIGHT)
      },
    }
  }

  /** Free to look all round and shoot: over their own shoulder, as on foot, from where they sit. */
  private passenger(): RigImpl {
    return {
      frame: (c) => {
        const { head, scoped, aimBlend } = c
        if (!head) return this.rigs.drive.frame(c)
        const at = [head.x, head.y, head.z]
        if (scoped) this.scene.scopeFrom(at, this.input.yaw, this.input.pitch)
        else this.scene.overShoulder(at, this.input.yaw, this.input.pitch, PASSENGER_DISTANCE * (1 - (1 - AIM_DISTANCE_SCALE) * aimBlend), PASSENGER_SHOULDER, PASSENGER_HEIGHT)
      },
    }
  }

  /**
   * Points the look where a hull-mounted gun does. The gun's angles are in the hull frame, the look is in
   * the world's, so the hull's orientation turns one into the other.
   */
  private lookAlong(mount: Barrel, gun: Turret) {
    const flat = Math.cos(gun.pitch)
    const local = new Vector3(-Math.sin(gun.yaw) * flat, Math.sin(gun.pitch), -Math.cos(gun.yaw) * flat)
    const world = local.applyQuaternion(mount.root.quaternion)
    this.input.yaw = Math.atan2(-world.x, -world.z)
    this.input.pitch = Math.asin(clamp(world.y, -1, 1))
  }

  /**
   * Keeps the look within a mounted gun's travel, which is in the hull's frame: the same limits the
   * server applies, so the view never points somewhere the gun cannot follow.
   */
  private limitGun(mount: Pick<Barrel, 'root'>, yawLimit: number, pitchMin: number, pitchMax: number) {
    const flat = Math.cos(this.input.pitch)
    const world = new Vector3(-Math.sin(this.input.yaw) * flat, Math.sin(this.input.pitch), -Math.cos(this.input.yaw) * flat)
    const local = world.applyQuaternion(new Quaternion().copy(mount.root.quaternion).invert())
    const yaw = clamp(Math.atan2(-local.x, -local.z), -yawLimit, yawLimit)
    const pitch = clamp(Math.asin(clamp(local.y, -1, 1)), pitchMin, pitchMax)
    const limited = new Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)).applyQuaternion(mount.root.quaternion)
    this.input.yaw = Math.atan2(-limited.x, -limited.z)
    this.input.pitch = Math.asin(clamp(limited.y, -1, 1))
  }

  /**
   * Puts the camera on the barrel's line, looking exactly where a shell leaves, and lays out the
   * reticle over it: the crosshair is the barrel's line and the marks below it are where a shell lands
   * at each range. A shell leaves at `SHELL_SPEED` and falls under gravity, so `s` metres out it is
   * `g (s / v)² / 2` below the line. Returns the barrel's heading.
   */
  private lookDownBarrel(model: Barrel): number {
    const { origin, direction } = model.barrel()
    const yaw = Math.atan2(-direction.x, -direction.z)
    const pitch = Math.asin(Math.max(-1, Math.min(1, direction.y)))
    const eye = new Vector3(origin.x, origin.y + SIGHT_EYE_UP, origin.z)
    this.scene.scopeFrom([eye.x, eye.y, eye.z], yaw, pitch)
    const camera = this.scene.camera
    const right = new Vector3(Math.cos(yaw), 0, -Math.sin(yaw))
    const up = new Vector3().crossVectors(right, direction)
    const reach = Math.tan((camera.fov * Math.PI) / 360)
    const level = Math.hypot(direction.x, direction.z)
    const marks: Mark[] = []
    for (const range of SIGHT_RANGES) {
      if (level < 1e-3) break
      const along = range / level
      const fall = 0.5 * GRAVITY * (along / SHELL_SPEED) ** 2
      // From the eye, which sits a little above the muzzle's start.
      const to = direction.clone().multiplyScalar(along).add(new Vector3(0, -fall - SIGHT_EYE_UP, 0))
      const depth = to.dot(direction)
      if (depth <= 0) continue
      const y = to.dot(up) / depth
      if (y > -reach * 0.9) marks.push({ range, y })
    }
    // The ray starts past the muzzle, so it does not find the gun's own hull.
    const start = eye.clone().addScaledVector(direction, SIGHT_RAY_START)
    const hit = this.castRay([start.x, start.y, start.z], [direction.x, direction.y, direction.z], SIGHT_RANGE_MAX)
    this.scene.gunsight.show(camera.fov, marks, hit ? hit.t + SIGHT_RAY_START : null)
    return yaw
  }
}
