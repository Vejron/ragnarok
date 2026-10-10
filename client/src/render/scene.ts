import * as THREE from 'three'

/** Third-person camera distance behind the player, metres. */
export const ON_FOOT_DISTANCE = 5
export const DRIVING_DISTANCE = 9
export const TANK_DISTANCE = 13
export const HELI_DISTANCE = 16
/** The gunner's camera sits this far behind the turret, on the line of the barrel. */
export const GUNNER_BACK = 6.5
/** ...and this far above it, so the turret doesn't hide the view. */
export const GUNNER_UP = 2.6
/** The camera looks at the point this far along the aim line, so the screen centre is where shells land at range. */
export const GUNNER_FOCUS = 60
export const CAMERA_HEIGHT = 1.2

export class Scene {
  readonly scene = new THREE.Scene()
  readonly camera = new THREE.PerspectiveCamera(70, 1, 0.1, 2000)
  readonly renderer: THREE.WebGLRenderer
  private readonly sun = new THREE.DirectionalLight(0xffffff, 2.2)
  private readonly resizeObserver: ResizeObserver

  constructor(private readonly container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true })
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    this.renderer.shadowMap.enabled = true
    this.renderer.shadowMap.type = THREE.PCFShadowMap
    container.appendChild(this.renderer.domElement)

    this.scene.background = new THREE.Color(0x9fd3f0)
    this.scene.fog = new THREE.Fog(0x9fd3f0, 150, 600)
    this.scene.add(new THREE.HemisphereLight(0xdff3ff, 0x6b8f4e, 1.4))
    this.sun.position.set(60, 120, 40)
    this.sun.castShadow = true
    this.sun.shadow.mapSize.set(2048, 2048)
    this.sun.shadow.bias = -0.0005
    this.sun.shadow.normalBias = 0.05
    const s = this.sun.shadow.camera
    s.left = s.bottom = -60
    s.right = s.top = 60
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

  /** Orbits the camera behind `target` by yaw/pitch, and keeps the shadow frustum around it. */
  follow(target: readonly number[], yaw: number, pitch: number, distance = ON_FOOT_DISTANCE) {
    const [x, y, z] = [target[0]!, target[1]! + CAMERA_HEIGHT, target[2]!]
    const horizontal = Math.cos(pitch) * distance
    this.camera.position.set(x + Math.sin(yaw) * horizontal, y - Math.sin(pitch) * distance, z + Math.cos(yaw) * horizontal)
    this.camera.position.y = Math.max(0.3, this.camera.position.y)
    this.camera.lookAt(x, y, z)
    this.sun.target.position.set(x, 0, z)
    this.sun.position.set(x + 60, 120, z + 40)
  }

  /**
   * A gunner's view: behind and above `origin` (the turret ring), looking along the aim direction.
   * It focuses on a far point of the barrel's line, so the centre of the screen is where the shell
   * lands at range even though the camera is higher than the gun.
   */
  aimFrom(origin: readonly number[], yaw: number, pitch: number) {
    const dir = new THREE.Vector3(-Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch))
    const from = new THREE.Vector3(origin[0]!, origin[1]!, origin[2]!)
    this.camera.position.copy(from).addScaledVector(dir, -GUNNER_BACK).add(new THREE.Vector3(0, GUNNER_UP, 0))
    this.camera.position.y = Math.max(0.3, this.camera.position.y)
    this.camera.lookAt(from.addScaledVector(dir, GUNNER_FOCUS))
    this.sun.target.position.set(origin[0]!, 0, origin[2]!)
    this.sun.position.set(origin[0]! + 60, 120, origin[2]! + 40)
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
