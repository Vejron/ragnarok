import { CRATER_FADE, Material, Mesh, planeGeometry, Ramp, Scene, Texture } from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { SKY } from '../shared/layout'

// Ground marked by the scene's craters, as the game's terrain is: a tank shell's, a mortar round's and a
// bomb's, a cluster of overlapping ones, one on the road, a fresh one still glowing, and a new one every two seconds.
const params = new URLSearchParams(location.search)
const { canvas, renderer, camera } = await setup()
const scene = new Scene()
scene.background.setHex(SKY)
scene.hemisphere.sky.setHex(0xdff3ff)
scene.hemisphere.ground.setHex(0x6b8f4e)
scene.sun.intensity = 2.4
// `?low` puts the sun near the horizon, as in the game's evening, to see the rims' shadows.
scene.sun.position.set(70, params.has('low') ? 18 : 80, 55)
scene.sun.shadow.enabled = true
scene.sun.shadow.span = 120
scene.toneMapping = 'neutral'
// The game's terrain ramp (client/src/render/groundlight.ts).
const terrain = new Ramp(new Uint8Array([90, 100, 115, 140, 180, 208, 235, 255]))

const cover = document.createElement('canvas')
cover.width = cover.height = 256
const g = cover.getContext('2d')!
g.fillStyle = '#7fa65a'
g.fillRect(0, 0, 256, 256)
for (let i = 0; i < 400; i++) {
  g.fillStyle = i % 3 ? '#76a052' : '#88b062'
  g.fillRect((i * 97) % 256, (i * 57) % 256, 6 + (i % 9), 6 + (i % 7))
}
g.fillStyle = '#8a8478'
g.fillRect(0, 150, 256, 14)
// `?plain` draws the ground with no crater marks at all, as a baseline for `?n`.
const ground = new Mesh(planeGeometry(160, 160), new Material({ ramp: terrain, map: new Texture(cover, { anisotropy: 4 }), craters: !params.has('plain') }))
ground.receiveShadow = true
scene.add(ground)

const now = () => performance.now() / 1000
// Radius and depth, metres (crates/shared/src/consts.rs).
const SHELL = [5.6, 1.0] as const
const MORTAR = [8.8, 1.6] as const
const BOMB = [7.0, 2.2] as const
scene.craters.add(-30, -20, ...SHELL, now() - 60)
scene.craters.add(0, -25, ...MORTAR, now() - 60)
scene.craters.add(30, -20, ...BOMB, now() - 60)
for (let i = 0; i < 6; i++) scene.craters.add(-25 + Math.cos(i * 2.1) * 6, 25 + Math.sin(i * 2.1) * 5, ...SHELL, now() - 60)
scene.craters.add(20, 15.5, ...SHELL, now() - 60)
// Fresh: its embers glow for CRATER_EMBER_LIFE seconds after the page opens.
scene.craters.add(10, 40, ...MORTAR, now())
// Old: its marks half faded (they go over the last 30% of CRATER_FADE).
scene.craters.add(-45, 30, ...MORTAR, now() - CRATER_FADE * 0.85)

// `?n=4000` scatters that many more over the ground, to time a battlefield dug over (`?hot`: all still
// glowing; `?off`: kept but hidden, as the scorch look turned off).
let seed = 7
const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646
for (let i = 0; i < Number(params.get('n') ?? 0); i++) {
  const [radius, depth] = [SHELL, MORTAR, BOMB][Math.floor(random() * 3)]!
  scene.craters.add((random() - 0.5) * 150, (random() - 0.5) * 150, radius, depth, params.has('hot') ? now() : now() - 60)
}
scene.craters.visible = !params.has('off')
Object.assign(window, { renderer })

const tick = statsOverlay(renderer, () => `craters ${scene.craters.count}`)
const move = orbit(canvas, camera, 70, 45)
let next = 2
const frame = () => {
  if (now() > next && !params.has('still')) {
    const [radius, depth] = [SHELL, MORTAR, BOMB][Math.floor(random() * 3)]!
    scene.craters.add((random() - 0.5) * 120, (random() - 0.5) * 120, radius, depth, now())
    next = now() + 2
  }
  move(params.has('still') ? 0 : 0.0015)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
