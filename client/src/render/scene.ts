import * as THREE from 'three'

/** Third-person camera distance behind the player, metres. */
const CAMERA_DISTANCE = 5
const CAMERA_HEIGHT = 1.2

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
  follow(target: readonly number[], yaw: number, pitch: number) {
    const [x, y, z] = [target[0]!, target[1]! + CAMERA_HEIGHT, target[2]!]
    const horizontal = Math.cos(pitch) * CAMERA_DISTANCE
    this.camera.position.set(x + Math.sin(yaw) * horizontal, y - Math.sin(pitch) * CAMERA_DISTANCE, z + Math.cos(yaw) * horizontal)
    this.camera.position.y = Math.max(0.3, this.camera.position.y)
    this.camera.lookAt(x, y, z)
    this.sun.target.position.set(x, 0, z)
    this.sun.position.set(x + 60, 120, z + 40)
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
