import {
  boxGeometry, Color, Fog, Geometry, Material, Mesh, planeGeometry, Ramp, Scene, sphereGeometry, Texture,
} from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, SKY } from '../shared/layout'
import { waterMaterial } from '../shared/water'

// Every material feature in one place: a canvas-textured ground, the game's water as a ShaderMaterial,
// see-through and additive surfaces, a depth-biased decal, and an object-space normal map.
const params = new URLSearchParams(location.search)
const { canvas, renderer, camera } = await setup()
const scene = new Scene()
scene.background.setHex(SKY)
scene.fog = new Fog(150, 600)
scene.fog.color.setHex(SKY)
scene.hemisphere.sky.setHex(0xdff3ff)
scene.hemisphere.ground.setHex(0x6b8f4e)
scene.sun.intensity = 2.4
scene.sun.position.set(70, 80, 55)
scene.sun.shadow.enabled = true
scene.sun.shadow.span = 80
scene.toneMapping = 'neutral'
const cel = new Ramp(new Uint8Array(CEL_BANDS))

// Ground: an sRGB canvas texture, as the game bakes terrain cover.
const cover = document.createElement('canvas')
cover.width = cover.height = 256
const g = cover.getContext('2d')!
g.fillStyle = '#7fa65a'
g.fillRect(0, 0, 256, 256)
for (let i = 0; i < 400; i++) {
  g.fillStyle = i % 3 ? '#6e9a4c' : '#9cbf6a'
  g.fillRect((i * 97) % 256, (i * 57) % 256, 6 + (i % 9), 6 + (i % 7))
}
g.fillStyle = '#c9b98a'
g.fillRect(0, 118, 256, 20)
const ground = new Mesh(planeGeometry(120, 120), new Material({ ramp: cel, map: new Texture(cover, { anisotropy: 4 }) }))
ground.receiveShadow = true
scene.add(ground)

// A pond: a disc of water whose depth (custom.x) grows toward the middle, over a dip in the ground.
const rings = 24, spokes = 48
const positions: number[] = []
const custom: number[] = []
const indices: number[] = []
for (let r = 0; r <= rings; r++) {
  for (let s = 0; s < spokes; s++) {
    const radius = (r / rings) * 16
    const a = (s / spokes) * Math.PI * 2
    positions.push(-20 + Math.cos(a) * radius, 0.05, 15 + Math.sin(a) * radius)
    custom.push(Math.max(0, 2.6 * (1 - (radius / 16) ** 2) - 0.05), 0, 0, 0)
  }
}
for (let r = 0; r < rings; r++) {
  for (let s = 0; s < spokes; s++) {
    const a = r * spokes + s, b = r * spokes + ((s + 1) % spokes)
    indices.push(a, b, a + spokes, b, b + spokes, a + spokes)
  }
}
const water = waterMaterial()
scene.add(new Mesh(new Geometry({ positions: new Float32Array(positions), custom: new Float32Array(custom), indices: new Uint32Array(indices) }), water))

// See-through boxes and glowing orbs: alpha and additive blending, sorted far to near.
const glass = new Material({ color: 0x9fd8ff, opacity: 0.35, blend: 'alpha', ramp: cel })
for (let i = 0; i < 5; i++) {
  const box = new Mesh(boxGeometry(3, 3, 3), glass)
  box.position.set(10 + i * 4, 1.5, -10)
  scene.add(box)
}
const glow = new Material({ lit: false, color: new Color(1, 0.55, 0.15), blend: 'additive', opacity: 0.8, fog: false })
const orbs: Mesh[] = []
for (let i = 0; i < 6; i++) {
  const orb = new Mesh(sphereGeometry(0.8, 16, 12), glow)
  scene.add(orb)
  orbs.push(orb)
}

// A scorch decal lying on the ground: depth bias keeps it over the ground without lifting it.
const scorch = document.createElement('canvas')
scorch.width = scorch.height = 128
const sg = scorch.getContext('2d')!
const grad = sg.createRadialGradient(64, 64, 4, 64, 64, 62)
grad.addColorStop(0, 'rgba(20,16,12,0.95)')
grad.addColorStop(1, 'rgba(20,16,12,0)')
sg.fillStyle = grad
sg.fillRect(0, 0, 128, 128)
const decal = new Mesh(planeGeometry(8, 8), new Material({ lit: false, map: new Texture(scorch), blend: 'alpha', depthBias: 4, depthBiasSlope: 2 }))
decal.position.set(8, 0, 8)
scene.add(decal)

// A bumpy ball lit only through its object-space normal map: the map is the sphere's own normal plus bumps.
const size = 256
const normals = new Uint8Array(size * size * 4)
for (let y = 0; y < size; y++) {
  for (let x = 0; x < size; x++) {
    const u = x / size, v = 1 - y / size
    const phi = (1 - v) * Math.PI, theta = u * Math.PI * 2
    let nx = -Math.cos(theta) * Math.sin(phi), ny = Math.cos(phi), nz = Math.sin(theta) * Math.sin(phi)
    const bump = 0.35 * Math.sin(theta * 12) * Math.sin(phi * 10)
    nx += bump * Math.cos(theta * 12)
    nz += bump * Math.sin(phi * 10)
    const len = Math.hypot(nx, ny, nz)
    const o = (y * size + x) * 4
    normals[o] = ((nx / len) * 0.5 + 0.5) * 255
    normals[o + 1] = ((ny / len) * 0.5 + 0.5) * 255
    normals[o + 2] = ((nz / len) * 0.5 + 0.5) * 255
    normals[o + 3] = 255
  }
}
const bumpy = new Mesh(
  sphereGeometry(3, 48, 32),
  new Material({ color: 0xd06a4a, ramp: cel, normalMap: new Texture({ data: normals, width: size, height: size }, { srgb: false }) }),
)
bumpy.position.set(-6, 3, -8)
bumpy.castShadow = true
bumpy.receiveShadow = true
scene.add(bumpy)

const tick = statsOverlay(renderer, () => 'materials example')
const move = orbit(canvas, camera, 45, 22)
const frame = (now: number) => {
  const t = now / 1000
  orbs.forEach((orb, i) => orb.position.set(15 + Math.cos(t + i) * 6, 2 + Math.sin(t * 2 + i), 10 + Math.sin(t + i) * 6))
  bumpy.quaternion.setFromAxisAngle(0, 1, 0, t * 0.3)
  move(params.has('still') ? 0 : 0.0015)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
