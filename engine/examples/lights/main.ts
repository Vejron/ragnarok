import { boxGeometry, Color, cylinderGeometry, Fog, Material, Mesh, planeGeometry, Ramp, Scene, sphereGeometry } from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, GROUND, layout } from '../shared/layout'

// Night over a stand-in for the game's world, lit by many point lights drifting about and cars driving
// in circles with headlights (spot lights), all binned into the renderer's light grid. `?n=` sets the
// point lights (default 120), `?cars=` the cars (default 16), `?grid` shows each cell's light count
// instead of the light, `?still` stops the camera.
const params = new URLSearchParams(location.search)
const NIGHT = 0x0b1426
const { canvas, renderer, camera } = await setup()
renderer.showLightGrid = params.has('grid')
const scene = new Scene()
scene.background.setHex(NIGHT)
scene.fog = new Fog(150, 600)
scene.fog.color.setHex(NIGHT)
scene.hemisphere.sky.setHex(0x30406a)
scene.hemisphere.ground.setHex(0x101820)
scene.hemisphere.intensity = 0.5
scene.sun.color.setHex(0x8fa8ff)
scene.sun.intensity = 0.25
scene.sun.position.set(-60, 80, 40)
scene.toneMapping = 'neutral'

const cel = new Ramp(new Uint8Array(CEL_BANDS))
scene.add(new Mesh(planeGeometry(600, 600), new Material({ color: GROUND, ramp: cel })))
const shapes = [boxGeometry(1, 1, 1), sphereGeometry(0.6, 16, 12), cylinderGeometry(0.4, 0.5, 1.4, 16)]
for (const prop of layout(Number(params.get('props') ?? 800)).props) {
  const mesh = new Mesh(shapes[prop.shape]!, new Material({ color: new Color(...prop.color), ramp: cel }))
  mesh.position.set(...prop.position)
  mesh.scale.set(...prop.scale)
  mesh.quaternion.setFromAxisAngle(0, 1, 0, prop.yaw)
  scene.add(mesh)
}

let seed = 11
const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646
const colors = [0xffa040, 0x60c0ff, 0xff5070, 0x90ff80, 0xfff0c0]
const wanderers = Array.from({ length: Number(params.get('n') ?? 120) }, () => ({
  x: (random() - 0.5) * 360, z: (random() - 0.5) * 360, phase: random() * 6.28, speed: 0.2 + random() * 0.5,
  color: colors[Math.floor(random() * colors.length)]!, radius: 8 + random() * 16,
}))
const cars = Array.from({ length: Number(params.get('cars') ?? 16) }, () => ({
  x: (random() - 0.5) * 240, z: (random() - 0.5) * 240, turn: 15 + random() * 30, speed: (0.3 + random() * 0.4) * (random() < 0.5 ? -1 : 1), phase: random() * 6.28,
}))
const body = boxGeometry(1.8, 1, 4)
const paint = new Material({ color: 0xc03030, ramp: cel })
const carMeshes = cars.map(() => new Mesh(body, paint))
scene.add(...carMeshes)
const lamp = sphereGeometry(0.3, 8, 6)
const glow = new Material({ color: 0xffe0a0, lit: false })
const lampMeshes = wanderers.map(() => new Mesh(lamp, glow))
scene.add(...lampMeshes)

const tick = statsOverlay(renderer, () => `lights ${renderer.stats.lights}   cell entries ${renderer.stats.lightEntries}`)
const move = orbit(canvas, camera, 90, 50)
const forward = { x: 0, y: 0, z: 0 }
const frame = () => {
  const t = performance.now() / 1000
  wanderers.forEach((w, i) => {
    const x = w.x + Math.sin(t * w.speed + w.phase) * 12
    const z = w.z + Math.cos(t * w.speed * 0.7 + w.phase) * 12
    lampMeshes[i]!.position.set(x, 2, z)
    scene.lights.hold(x, 2, z, { color: w.color, intensity: 2.5, radius: w.radius })
  })
  cars.forEach((c, i) => {
    const a = t * c.speed + c.phase
    const x = c.x + Math.cos(a) * c.turn
    const z = c.z + Math.sin(a) * c.turn
    // Heading along the circle, a little down toward the road.
    const s = Math.sign(c.speed)
    forward.x = -Math.sin(a) * s
    forward.z = Math.cos(a) * s
    forward.y = -0.12
    const mesh = carMeshes[i]!
    mesh.position.set(x, 0.5, z)
    mesh.quaternion.setFromAxisAngle(0, 1, 0, Math.atan2(forward.x, forward.z))
    scene.lights.hold(x + forward.x * 2.2, 0.8, z + forward.z * 2.2, { color: 0xfff2d0, intensity: 6, radius: 35, direction: forward, angle: 0.45, penumbra: 0.4 })
  })
  scene.lights.update(1 / 60)
  move(params.has('still') ? 0 : 0.0015)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
Object.assign(window, { renderer, scene })
