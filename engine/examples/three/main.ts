// The benchmark scene drawn by three, configured as the game configures it (toon materials with a cel
// gradient map, PCF sun shadows, fog, neutral tone mapping, MSAA). `?webgpu` uses three's
// WebGPURenderer instead of WebGLRenderer. Compare with `static/` on the same query string.
import type * as ThreeTypes from 'three'
import { CEL_BANDS, GROUND, LEAVES, layout, SKY } from '../shared/layout'

const params = new URLSearchParams(location.search)
const webgpu = params.has('webgpu')
const THREE = (webgpu ? await import('three/webgpu') : await import('three')) as unknown as typeof ThreeTypes
const count = Number(params.get('n') ?? 1000)
const { props, forest } = layout(count)

const canvas = document.createElement('canvas')
canvas.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;display:block'
document.body.style.margin = '0'
document.body.appendChild(canvas)
const renderer = new (THREE as unknown as { WebGPURenderer: typeof ThreeTypes.WebGLRenderer; WebGLRenderer: typeof ThreeTypes.WebGLRenderer })[
  webgpu ? 'WebGPURenderer' : 'WebGLRenderer'
]({ canvas, antialias: true })
if (webgpu) await (renderer as unknown as { init(): Promise<void> }).init()
renderer.shadowMap.enabled = params.get('shadows') !== '0'
renderer.shadowMap.type = THREE.PCFShadowMap
renderer.toneMapping = THREE.NeutralToneMapping
renderer.info.autoReset = false

const camera = new THREE.PerspectiveCamera(60, 1, 0.25, 2000)
const resize = () => {
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2))
  renderer.setSize(innerWidth, innerHeight, false)
  camera.aspect = innerWidth / innerHeight
  camera.updateProjectionMatrix()
}
addEventListener('resize', resize)
resize()

const scene = new THREE.Scene()
scene.background = new THREE.Color(SKY)
scene.fog = new THREE.Fog(SKY, 150, 600)
scene.add(new THREE.HemisphereLight(0xdff3ff, 0x6b8f4e, 1))
const sun = new THREE.DirectionalLight(0xffffff, 2.4)
sun.position.set(70, 80, 55)
sun.castShadow = true
sun.shadow.mapSize.set(2048, 2048)
Object.assign(sun.shadow.camera, { left: -80, right: 80, top: 80, bottom: -80, near: 1, far: 400 })
sun.shadow.bias = -0.001
sun.shadow.normalBias = 0.15
scene.add(sun, sun.target)

const ramp = new THREE.DataTexture(new Uint8Array(CEL_BANDS), CEL_BANDS.length, 1, THREE.RedFormat)
ramp.minFilter = ramp.magFilter = THREE.NearestFilter
ramp.needsUpdate = true
const toon = (color: ThreeTypes.ColorRepresentation) => new THREE.MeshToonMaterial({ color, gradientMap: ramp })

const ground = new THREE.Mesh(new THREE.PlaneGeometry(400, 400).rotateX(-Math.PI / 2), toon(GROUND))
ground.receiveShadow = true
scene.add(ground)

const shapes = [new THREE.BoxGeometry(1, 1, 1), new THREE.SphereGeometry(0.6, 16, 12), new THREE.CylinderGeometry(0.4, 0.5, 1.4, 16)]
const spinners: ThreeTypes.Mesh[] = []
for (const prop of props) {
  const mesh = new THREE.Mesh(shapes[prop.shape], toon(new THREE.Color().setRGB(...prop.color, THREE.LinearSRGBColorSpace)))
  mesh.position.set(...prop.position)
  mesh.scale.set(...prop.scale)
  mesh.rotation.y = prop.yaw
  mesh.castShadow = true
  mesh.receiveShadow = true
  scene.add(mesh)
  if (prop.spins) spinners.push(mesh)
}

const trees = new THREE.InstancedMesh(new THREE.ConeGeometry(1.6, 5, 8), toon(LEAVES), forest.length)
const m = new THREE.Matrix4()
forest.forEach((tree, i) => {
  m.makeScale(tree.scale, tree.scale, tree.scale).setPosition(...tree.position)
  trees.setMatrixAt(i, m)
})
trees.castShadow = true
trees.receiveShadow = true
scene.add(trees)

const el = document.createElement('div')
el.id = 'stats'
el.style.cssText = 'position:fixed;top:8px;right:8px;font:12px/1.4 ui-monospace,Menlo,monospace;color:#eee;background:rgba(0,0,0,.6);padding:6px 8px;border-radius:4px;white-space:pre;pointer-events:none'
document.body.appendChild(el)

let yaw = Number(params.get('yaw') ?? 0.6)
const distance = 70
const pitch = Math.atan2(30, distance)
let angle = 0
let frames = 0
let cpu = 0
let last = performance.now()
const frame = () => {
  angle += 0.01
  for (const mesh of spinners) mesh.rotation.y = angle
  if (!params.has('still')) yaw += 0.001
  camera.position.set(Math.sin(yaw) * Math.cos(pitch) * distance, Math.sin(pitch) * distance, Math.cos(yaw) * Math.cos(pitch) * distance)
  camera.lookAt(0, 0, 0)
  renderer.info.reset()
  const start = performance.now()
  renderer.render(scene, camera)
  cpu += performance.now() - start
  frames++
  const now = performance.now()
  if (now - last >= 250) {
    const info = renderer.info.render as unknown as { calls: number; drawCalls?: number; triangles: number }
    el.textContent = [
      `fps ${((frames * 1000) / (now - last)).toFixed(0)}   render cpu ${(cpu / frames).toFixed(3)} ms`,
      `draws ${info.drawCalls ?? info.calls}   tris ${(info.triangles / 1000).toFixed(1)}k`,
      `three ${webgpu ? 'WebGPURenderer' : 'WebGLRenderer'}   meshes ${count}  trees ${forest.length}`,
    ].join('\n')
    last = now
    frames = 0
    cpu = 0
  }
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
