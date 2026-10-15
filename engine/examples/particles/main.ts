import { Fog, Geometry, Material, Mesh, ParticleSystem, Ramp, Scene } from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, SKY } from '../shared/layout'

// GPU particles: explosions modelled on the game's (fireball, white core, sparks that bounce on the
// terrain, smoke column, dust ring, thrown dirt), drifting smoke columns, and `?stress=N` to keep about
// N particles alive for measuring.
const params = new URLSearchParams(location.search)
const stress = Number(params.get('stress') ?? 0)
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
scene.sun.shadow.span = 120
scene.toneMapping = 'neutral'

// Rolling ground, and the same heights as the particles' bounce field.
const side = 161, cell = 1, x0 = -80, z0 = -80
const height = (x: number, z: number) => 1.6 * Math.sin(x * 0.07) * Math.cos(z * 0.05) + 0.8 * Math.sin((x + z) * 0.11)
const heights = new Float32Array(side * side)
const positions = new Float32Array(side * side * 3)
for (let j = 0; j < side; j++) {
  for (let i = 0; i < side; i++) {
    const x = x0 + i * cell, z = z0 + j * cell, h = height(x, z)
    heights[j * side + i] = h
    positions.set([x, h, z], (j * side + i) * 3)
  }
}
const indices = new Uint32Array((side - 1) * (side - 1) * 6)
let k = 0
for (let j = 0; j < side - 1; j++) {
  for (let i = 0; i < side - 1; i++) {
    const a = j * side + i
    indices.set([a, a + side, a + 1, a + 1, a + side, a + side + 1], k)
    k += 6
  }
}
const terrain = new Geometry({ positions, indices })
terrain.computeNormals()
const ground = new Mesh(terrain, new Material({ color: 0x7fa65a, ramp: new Ramp(new Uint8Array(CEL_BANDS)) }))
ground.receiveShadow = true
scene.add(ground)

const particles = new ParticleSystem(Math.max(65536, Math.ceil((stress * 1.3) / 64) * 64))
particles.setGround(heights, side, side, x0, z0, cell)
scene.add(particles)

function explosion(x: number, z: number, radius = 3) {
  const y = height(x, z)
  const at = [x, y + 0.3, z] as const
  particles.emit({ count: 12, position: at, spread: radius * 0.2, velocity: [0, 3, 0], jitter: [radius * 0.6, 1.75, radius * 0.6],
    life: [0.5, 0.95], size: [radius * 0.25, radius * 0.9], color: 0xff7a1a, color2: 0xffb347, alpha: 0.9, gravity: -2, drag: 2 })
  particles.emit({ count: 8, position: at, velocity: [0, 1.75, 0], jitter: [radius * 0.6, 1.25, radius * 0.6], blend: 'additive',
    life: [0.18, 0.4], size: [radius * 0.45, radius * 0.9], color: 0xfff0c0, color2: 0xffd27a, alpha: 0.9, drag: 3 })
  particles.emit({ count: 32, position: at, velocity: [0, 4, 0], burst: 20, blend: 'additive', fog: false, bounce: true,
    life: [0.4, 1.3], size: [0.14, 0.02], color: 0xffe08a, color2: 0xff6a1a, alpha: 1, gravity: 9.8, drag: 0.5 })
  particles.emit({ count: 10, position: at, spread: radius * 0.4, velocity: [0, 2.5, 0], jitter: [1.2, 0.8, 1.2],
    life: [2.5, 4.5], size: [radius * 0.5, radius * 2.2], color: 0x2e2e2e, color2: 0x6a6a6a, alpha: 0.55, gravity: -1.2, drag: 0.8 })
  particles.emit({ count: 14, position: at, velocity: [0, 0.6, 0], jitter: [7, 0.3, 7],
    life: [1.2, 2.2], size: [1, 3.5], color: 0xb59a6a, color2: 0x8f7a52, alpha: 0.5, drag: 2.2 })
  particles.emit({ count: 10, position: at, velocity: [0, 6, 0], burst: 8, bounce: true,
    life: [1.5, 2.5], size: [0.35, 0.25], color: 0x5a4a30, color2: 0x3a2e1e, alpha: 1, gravity: 9.8, drag: 0.3 })
}

const tick = statsOverlay(renderer, () => `particles alive ${particles.alive}${stress ? `  (stress ${stress})` : ''}`)
const move = orbit(canvas, camera, 55, 22)
let last = performance.now()
let nextBlast = 0
let carry = 0
let smokeCarry = 0
const frame = (now: number) => {
  const dt = Math.min(0.05, (now - last) / 1000)
  last = now
  const t = now / 1000
  if (t > nextBlast) {
    nextBlast = t + 0.5
    explosion((Math.random() - 0.5) * 60, (Math.random() - 0.5) * 60)
  }
  // Two smoke columns at 30 puffs a second, the fraction carried between frames as the game's burning wrecks do.
  smokeCarry += 30 * dt
  const puffs = Math.floor(smokeCarry)
  smokeCarry -= puffs
  if (puffs) for (const [x, z] of [[-15, 10], [18, -12]] as const) {
    particles.emit({ count: puffs, position: [x, height(x, z) + 0.5, z], spread: 0.6,
      velocity: [1.2, 2.2, 0.4], jitter: [0.4, 0.5, 0.4], life: [2.2, 4.8], size: [0.8, 3.6], color: 0x333333, color2: 0x555555,
      alpha: 0.5, gravity: -0.6, drag: 0.4 })
  }
  if (stress) {
    // Average life 2 s: emit stress/2 per second.
    carry += (stress / 2) * dt
    const n = Math.floor(carry)
    carry -= n
    if (n) particles.emit({ count: n, position: [0, 8, 0], spread: 50, velocity: [0, 0, 0], jitter: [2, 2, 2], blend: 'additive', fog: false,
      life: [1, 3], size: [0.3, 0.1], color: 0x66ccff, color2: 0xff66cc, alpha: 0.8, gravity: 0.5 })
  }
  particles.update(dt)
  move(params.has('still') ? 0 : 0.0015)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
