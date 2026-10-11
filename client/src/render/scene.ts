import * as THREE from 'three'
import { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER } from '../sim/consts'

// On foot the camera is over the right shoulder and a little above the head; the server shares these.
export { ON_FOOT_HEIGHT, ON_FOOT_SHOULDER }

/** Third-person camera distance behind the player, metres. */
export const ON_FOOT_DISTANCE = 3.2
export const DRIVING_DISTANCE = 9
export const TANK_DISTANCE = 13
export const HELI_DISTANCE = 16
/** The gunner's camera sits this far behind the turret, on the line of the barrel. */
export const GUNNER_BACK = 6.5
/** ...and this far above it, so the turret doesn't hide the view. */
export const GUNNER_UP = 2.6
/** The camera looks at the point this far along the aim line, so the screen centre is where the barrel points. */
export const GUNNER_FOCUS = 60
export const CAMERA_HEIGHT = 1.2

/**
 * Where the sun sits relative to what it follows, metres: about 42° above the horizon. Higher and
 * hillsides all fall in the same lit band; lower and the terrain's toon ramp shows its form.
 */
const SUN_OFFSET = [70, 80, 55] as const

export class Scene {
  readonly scene = new THREE.Scene()
  readonly camera = new THREE.PerspectiveCamera(75, 1, 0.25, 2000)
  readonly renderer: THREE.WebGLRenderer
  private readonly sun = new THREE.DirectionalLight(0xffffff, 2.4)
  private readonly resizeObserver: ResizeObserver

  constructor(private readonly container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true })
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    this.renderer.shadowMap.enabled = true
    this.renderer.shadowMap.type = THREE.PCFShadowMap
    container.appendChild(this.renderer.domElement)

    this.scene.background = new THREE.Color(0x9fd3f0)
    this.scene.fog = new THREE.Fog(0x9fd3f0, 150, 600)
    this.scene.add(new THREE.HemisphereLight(0xdff3ff, 0x6b8f4e, 1.0))
    this.sun.position.set(...SUN_OFFSET)
    this.sun.castShadow = true
    this.sun.shadow.mapSize.set(2048, 2048)
    this.sun.shadow.bias = -0.001
    this.sun.shadow.normalBias = 0.15
    const s = this.sun.shadow.camera
    s.left = s.bottom = -50
    s.right = s.top = 50
    s.near = 10
    s.far = 300
    this.scene.add(this.sun, this.sun.target)

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.resize()
  }

  get canvas() {
    return this.renderer.domElement
  }

  private resize() {
    const { clientWidth: width, clientHeight: height } = this.container
    if (width === 0 || height === 0) return
    this.renderer.setSize(width, height)
    this.camera.aspect = width / height
    this.camera.updateProjectionMatrix()
  }

  /** Height of the ground under `(x, z)`, so the camera stays above hills. Set by the game. */
  groundAt: (x: number, z: number) => number = () => 0

  /**
   * Orbits the camera behind `target` by yaw/pitch, and keeps the shadow frustum around it.
   * A `shoulder` shifts the pivot to the right of the target, so the target sits left of screen centre.
   */
  follow(target: readonly number[], yaw: number, pitch: number, distance = ON_FOOT_DISTANCE, shoulder = 0, height = CAMERA_HEIGHT) {
    const x = target[0]! + Math.cos(yaw) * shoulder
    const y = target[1]! + height
    const z = target[2]! - Math.sin(yaw) * shoulder
    const horizontal = Math.cos(pitch) * distance
    this.camera.position.set(x + Math.sin(yaw) * horizontal, y - Math.sin(pitch) * distance, z + Math.cos(yaw) * horizontal)
    this.camera.position.y = Math.max(this.groundAt(this.camera.position.x, this.camera.position.z) + 0.3, this.camera.position.y)
    this.camera.lookAt(x, y, z)
    // The shadow frustum follows the target's height too, so it covers hills.
    this.sun.target.position.set(target[0]!, target[1]!, target[2]!)
    this.sun.position.set(target[0]! + SUN_OFFSET[0], target[1]! + SUN_OFFSET[1], target[2]! + SUN_OFFSET[2])
  }

  /**
   * A gunner's view: behind and above `origin` (the turret ring), looking along the aim direction.
   * It focuses on a far point of the barrel's line, so the centre of the screen is where the barrel
   * points even though the camera is higher than the gun. A shell falls from that line.
   */
  aimFrom(origin: readonly number[], yaw: number, pitch: number) {
    const dir = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch))
    const from = new THREE.Vector3(origin[0]!, origin[1]!, origin[2]!)
    this.camera.position.copy(from).addScaledVector(dir, -GUNNER_BACK).add(new THREE.Vector3(0, GUNNER_UP, 0))
    this.camera.position.y = Math.max(this.groundAt(this.camera.position.x, this.camera.position.z) + 0.3, this.camera.position.y)
    this.camera.lookAt(from.addScaledVector(dir, GUNNER_FOCUS))
    this.sun.target.position.set(origin[0]!, origin[1]!, origin[2]!)
    this.sun.position.set(origin[0]! + SUN_OFFSET[0], origin[1]! + SUN_OFFSET[1], origin[2]! + SUN_OFFSET[2])
  }

  render() {
    this.renderer.render(this.scene, this.camera)
  }

  dispose() {
    this.resizeObserver.disconnect()
    this.renderer.dispose()
    this.renderer.domElement.remove()
  }
}
