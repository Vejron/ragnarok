import {
  boxGeometry, Color, cylinderGeometry, Fog, InstancedMesh, Material, Mesh, mat4, planeGeometry, Ramp, Scene, sphereGeometry,
} from '../../src'
import { orbit, setup, statsOverlay } from '../common'

// A stand-in for the game's world: about as many meshes and materials as a streamed-in area has, each
// mesh with its own material (as three's port had), plus an instanced forest and a ground.
const params = new URLSearchParams(location.search)
const count = Number(params.get('n') ?? 1000)

const { canvas, renderer, camera } = await setup()
const scene = new Scene()
scene.background.setHex(0x9fd3f0)
scene.fog = new Fog(150, 600)
scene.fog.color.setHex(0x9fd3f0)
scene.hemisphere.sky.setHex(0xdff3ff)
scene.hemisphere.ground.setHex(0x6b8f4e)
scene.hemisphere.intensity = 1
scene.sun.intensity = 2.4
scene.sun.position.set(70, 80, 55)
scene.sun.shadow.enabled = params.get('shadows') !== '0'
scene.sun.shadow.span = 160
scene.sun.shadow.mapSize = 2048
scene.toneMapping = 'neutral'

const cel = new Ramp(new Uint8Array([96, 150, 205, 255]))
const ground = new Mesh(planeGeometry(400, 400), new Material({ color: 0x7fa65a, ramp: cel }))
ground.receiveShadow = true
scene.add(ground)

const shapes = [boxGeometry(1, 1, 1), sphereGeometry(0.6, 16, 12), cylinderGeometry(0.4, 0.5, 1.4, 16)]
let seed = 1
const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646
const spinners: Mesh[] = []
for (let i = 0; i < count; i++) {
  const material = new Material({ color: new Color(0.2 + random() * 0.8, 0.2 + random() * 0.8, 0.2 + random() * 0.8), ramp: cel })
  const mesh = new Mesh(shapes[i % shapes.length]!, material)
  const r = 8 + random() * 70
  const a = random() * Math.PI * 2
  const s = 0.6 + random() * 2
  mesh.position.set(Math.cos(a) * r, s * 0.5, Math.sin(a) * r)
  mesh.scale.set(s, s * (0.5 + random()), s)
  mesh.quaternion.setFromAxisAngle(0, 1, 0, random() * Math.PI)
  mesh.castShadow = true
  mesh.receiveShadow = true
  scene.add(mesh)
  if (i % 10 === 0) spinners.push(mesh)
}

// A forest: one instanced mesh of 2000 cones, as the game's tree archetypes are.
const trees = new InstancedMesh(cylinderGeometry(0, 1.6, 5, 8), new Material({ color: 0x3f7a35, ramp: cel }), 2000)
const m = mat4.create()
for (let i = 0; i < trees.capacity; i++) {
  const r = 90 + random() * 100
  const a = random() * Math.PI * 2
  const s = 0.7 + random() * 0.8
  mat4.compose(m, Math.cos(a) * r, 2.5 * s, Math.sin(a) * r, 0, 0, 0, 1, s, s, s)
  trees.setMatrixAt(i, m)
}
trees.needsUpdate()
trees.castShadow = true
trees.receiveShadow = true
scene.add(trees)

const tick = statsOverlay(renderer, () => `meshes ${count}  trees ${trees.count}`)
const move = orbit(canvas, camera, 70, 30)
let angle = 0
const frame = () => {
  angle += 0.01
  for (const mesh of spinners) mesh.quaternion.setFromAxisAngle(0, 1, 0, angle)
  move(params.has('still') ? 0 : 0.001)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
