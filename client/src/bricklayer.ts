import { BUTTON_ALT, BUTTON_FIRE, BUILD_REACH } from './sim/consts'
import { CELL } from './sim/carve'
import { BrickTrigger, mayPlace, type BuildWorld } from './sim/bricks'
import { MATERIAL_BRICK, cellAt, cubeCentre, type Pos } from './sim/construct'
import type { SimMotor, Vec3 } from './sim/motor'
import type { InputFrame } from './net/codec'
import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from './render/scene'
import type { WorldView } from './worldview'

/** How long past a round trip a predicted cube waits for the server's word before it is taken back, ms. */
const BRICK_PREDICTION_SLACK_MS = 400
/** The round trip guessed before the first has been measured, ms. */
const ROUND_TRIP_GUESS_MS = 250

/** The local player's bricks, as the server would use them, at once. */
export class Bricklayer {
  /** When the bricks are used, counted as the server counts it. */
  private readonly trigger = new BrickTrigger()

  constructor(
    private readonly view: WorldView,
    private readonly motor: () => SimMotor | null,
    private readonly eyeHeight: () => number,
  ) {}

  /**
   * Where bricks in the local player's hand would put a cube and which cube they would dig out, as the
   * server's `building::aim` finds them for a player standing at `position`: the crosshair ray from the
   * camera's pivot, and the cubes on the near and far side of the face it meets, within `BUILD_REACH`
   * of the eye.
   */
  aim(position: ArrayLike<number>, yaw: number, pitch: number): { place: Pos; dig: Pos } | null {
    const flat = Math.cos(pitch)
    const dir: Vec3 = [-Math.sin(yaw) * flat, Math.sin(pitch), -Math.cos(yaw) * flat]
    const pivot: Vec3 = [position[0]! + Math.cos(yaw) * ON_FOOT_SHOULDER, position[1]! + ON_FOOT_HEIGHT, position[2]! - Math.sin(yaw) * ON_FOOT_SHOULDER]
    const hit = this.motor()?.castRay(pivot, dir, BUILD_REACH + ON_FOOT_SHOULDER + ON_FOOT_HEIGHT)
    if (!hit) return null
    const half = CELL / 200
    const across = (side: number) => cellAt(...([0, 1, 2].map((a) => pivot[a]! + dir[a]! * hit.t + hit.normal[a]! * half * side) as [number, number, number]))
    const place = across(1)
    const middle = cubeCentre(place)
    const eye = [position[0]!, position[1]! + this.eyeHeight(), position[2]!]
    if (Math.hypot(middle[0] - eye[0]!, middle[1] - eye[1]!, middle[2] - eye[2]!) > BUILD_REACH) return null
    return { place, dig: across(-1) }
  }

  /** What the placing rules need of the world round the local player standing at `position`, once the simulation exists. */
  world(position: ArrayLike<number>): BuildWorld | null {
    const motor = this.motor()
    if (!motor) return null
    return {
      construct: this.view.construct,
      groundAt: (x, z) => this.view.terrain?.heightAt(x, z) ?? 0,
      cubeBlocked: (centre, half) => motor.cubeBlocked(centre, half),
      buildingNear: (point, radius) => motor.buildingNear(point, radius),
      body: [position[0]!, position[1]!, position[2]!],
    }
  }

  /**
   * On an input tick the server would use the bricks on, the cube the crosshair puts (or with aim held,
   * digs out) is placed on screen and made solid now, by the server's rules. `WorldView.predict` takes
   * it back if the server does not do the same within about a round trip.
   */
  feedback(frame: InputFrame, onFoot: boolean, position: ArrayLike<number>, colour: number, rtt: number | null) {
    if (!this.trigger.tick(onFoot && (frame.buttons & BUTTON_FIRE) !== 0)) return
    const aim = this.aim(position, frame.yaw / 1000, frame.pitch / 1000)
    if (!aim) return
    // The server's `built` comes a round trip later, plus the ticks the input waits in its jitter buffer.
    const until = performance.now() + (rtt ?? ROUND_TRIP_GUESS_MS) * 1.5 + BRICK_PREDICTION_SLACK_MS
    if ((frame.buttons & BUTTON_ALT) !== 0) {
      this.view.predict(aim.dig, null, until)
      return
    }
    const world = this.world(position)
    if (!world || !mayPlace(world, aim.place)) return
    this.view.predict(aim.place, { colour, material: MATERIAL_BRICK }, until)
  }
}
