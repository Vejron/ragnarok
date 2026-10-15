import {
  boxGeometry, Color, cylinderGeometry, Fog, InstancedMesh, Material, Mesh, mat4, planeGeometry, Ramp, Scene, sphereGeometry,
} from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, GROUND, LEAVES, layout, SKY } from '../shared/layout'

// A stand-in for the game's world: about as many meshes and materials as a streamed-in area has, each
// mesh with its own material, plus an instanced forest and a ground. `three/` draws the same with three.
const params = new URLSearchParams(location.search)
const count = Number(params.get('n') ?? 1000)
const { props, forest } = layout(count)

const { canvas, renderer, camera } = await setup()
const scene = new Scene()
scene.background.setHex(SKY)
scene.fog = new Fog(150, 600)
scene.fog.color.setHex(SKY)
scene.hemisphere.sky.setHex(0xdff3ff)
scene.hemisphere.ground.setHex(0x6b8f4e)
scene.sun.intensity = 2.4
scene.sun.position.set(70, 80, 55)
scene.sun.shadow.enabled = params.get('shadows') !== '0'
scene.sun.shadow.span = 160
scene.sun.shadow.mapSize = 2048
scene.toneMapping = 'neutral'

const cel = new Ramp(new Uint8Array(CEL_BANDS))
const ground = new Mesh(planeGeometry(400, 400), new Material({ color: GROUND, ramp: cel }))
ground.receiveShadow = true
scene.add(ground)

const shapes = [boxGeometry(1, 1, 1), sphereGeometry(0.6, 16, 12), cylinderGeometry(0.4, 0.5, 1.4, 16)]
const spinners: Mesh[] = []
for (const prop of props) {
  // Prop colours are linear already (random floats), as three's Color.setRGB takes them.
  const mesh = new Mesh(shapes[prop.shape]!, new Material({ color: new Color(...prop.color), ramp: cel }))
  mesh.position.set(...prop.position)
  mesh.scale.set(...prop.scale)
  mesh.quaternion.setFromAxisAngle(0, 1, 0, prop.yaw)
  mesh.castShadow = true
  mesh.receiveShadow = true
  scene.add(mesh)
  if (prop.spins) spinners.push(mesh)
}

const trees = new InstancedMesh(cylinderGeometry(0, 1.6, 5, 8), new Material({ color: LEAVES, ramp: cel }), forest.length)
const m = mat4.create()
forest.forEach((tree, i) => {
  mat4.compose(m, ...tree.position, 0, 0, 0, 1, tree.scale, tree.scale, tree.scale)
  trees.setMatrixAt(i, m)
})
trees.needsUpdate()
trees.castShadow = true
trees.receiveShadow = true
scene.add(trees)

const tick = statsOverlay(renderer, () => `engine   meshes ${count}  trees ${trees.count}`)
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
