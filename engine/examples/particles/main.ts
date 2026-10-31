import { Bloom, Fog, Geometry, Material, Mesh, ParticleSystem, Ramp, Ribbons, Scene } from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, SKY } from '../shared/layout'
import { Shots } from '../shared/shots'

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
scene.bloom = new Bloom()

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
const ribbons = new Ribbons()
scene.add(ribbons)
// Gunfire: the game's shot effects, on the same particles.
const shots = new Shots(particles)
scene.add(shots.flashes)

function explosion(x: number, z: number, radius = 3) {
  const y = height(x, z)
  const at = [x, y + 0.3, z] as const
  // A flash that lights the ground and a shockwave ring racing out over it.
  scene.lights.flash(x, y + 2, z, { color: 0xffa04a, intensity: 6, radius: radius * 9, life: 0.7 })
  shots.flashes.spawn({ at: [x, y + 0.25, z], ring: true, life: 0.55, size: radius * 0.3, grow: radius * 7, length: radius * 1.2,
    color: 0xffe6b0, opacity: 0.8, bright: true })
  // Fireball: hot billows that cool from white-yellow through orange to dark as they rise.
  particles.emit({ count: 30, position: at, spread: radius * 0.3, velocity: [0, 2, 0], burst: radius * 2.2, shape: 'puff',
    life: [0.5, 1.2], size: [radius * 0.5, radius * 1.7], color: 0xffeaa0, color2: 0xffb347, colorEnd: 0x6a1d04, glow: 3,
    alpha: 0.95, gravity: -3, drag: 2.5, spin: [-2, 2] })
  // White-hot core flash.
  particles.emit({ count: 6, position: at, spread: radius * 0.2, velocity: [0, 1.5, 0], burst: radius * 0.6, blend: 'additive',
    life: [0.12, 0.28], size: [radius * 1, radius * 1.8], color: 0xfff6d8, colorEnd: 0xff9a30, glow: 8, alpha: 0.9, drag: 3 })
  // Sparks: stretched along their flight, bright enough to bloom, bouncing on the ground.
  particles.emit({ count: 90, position: at, velocity: [0, 5, 0], burst: radius * 9, blend: 'additive', fog: false, bounce: true,
    life: [0.5, 1.6], size: [0.09, 0.02], color: 0xffe08a, color2: 0xff7a22, colorEnd: 0xff3a05, glow: 6, stretch: 0.05,
    alpha: 1, gravity: 9.8, drag: 0.5 })
  // Embers that drift up and gutter out.
  particles.emit({ count: 30, position: at, spread: radius * 0.5, velocity: [0, 3, 0], jitter: [radius * 0.8, 1.5, radius * 0.8],
    blend: 'additive', fog: false, life: [1.5, 3.5], size: [0.12, 0.03], color: 0xffb04a, colorEnd: 0xff2a00, glow: 4,
    alpha: 1, gravity: -1.5, drag: 1.2, spin: [-6, 6] })
  // Sooty smoke column: black while hot, greying as it spreads.
  particles.emit({ count: 26, position: at, spread: radius * 0.5, velocity: [0, 4.5, 0], jitter: [radius * 0.5, 1.5, radius * 0.5],
    shape: 'puff', life: [3, 6], size: [radius * 0.6, radius * 2.8], color: 0x1c1c1c, color2: 0x3a3a3a, colorEnd: 0x7a7a7a,
    alpha: 0.65, gravity: -1.5, drag: 0.9, spin: [-0.6, 0.6] })
  particles.emit({ count: 14, position: at, velocity: [0, 0.6, 0], jitter: [7, 0.3, 7],
    life: [1.2, 2.2], size: [1, 3.5], color: 0xb59a6a, color2: 0x8f7a52, alpha: 0.5, drag: 2.2 })
  particles.emit({ count: 10, position: at, velocity: [0, 6, 0], burst: 8, bounce: true,
    life: [1.5, 2.5], size: [0.35, 0.25], color: 0x5a4a30, color2: 0x3a2e1e, alpha: 1, gravity: 9.8, drag: 0.3 })
}

const tick = statsOverlay(renderer, () => `particles alive ${particles.alive}${stress ? `  (stress ${stress})` : ''}   flashes ${shots.flashes.count}`)
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
    const x = (Math.random() - 0.5) * 60, z = (Math.random() - 0.5) * 60
    explosion(x, z)
    shots.burst({ x, y: height(x, z) + 0.5, z }, 3)
  }
  // Two missiles looping over the field, each leaving a hot core and a smoke ribbon.
  ribbons.update(dt)
  scene.lights.update(dt)
  for (let m = 0; m < 2; m++) {
    const a = t * 1.1 + m * Math.PI
    const x = Math.cos(a) * 28, z = Math.sin(a) * 28, y = height(x, z) + 9 + Math.sin(a * 2.3) * 4
    ribbons.extend(`smoke${m}`, x, y, z, { color: 0x555555, colorEnd: 0x999999, alpha: 0.5, life: 2.6, width: 0.9, taper: 2.5, step: 0.4 })
    ribbons.extend(`fire${m}`, x, y, z, { color: 0xfff2c0, colorEnd: 0xff4a08, glow: 6, alpha: 1, life: 0.35, width: 0.5, blend: 'additive', fog: false, step: 0.3 })
  }
  // A machine gun sweeping across the field, and a shotgun going off now and then.
  if (Math.random() < 0.3) {
    const from = { x: -40, y: height(-40, 0) + 1.5, z: 0 }
    const tx = 30, tz = Math.sin(t * 0.7) * 30
    const to = { x: tx, y: height(tx, tz) + 0.3, z: tz }
    shots.round(from, to)
    shots.tracer(from, to, true)
  }
  if (Math.random() < 0.03) {
    const muzzle = { x: 0, y: height(0, 30) + 1.5, z: 30 }
    shots.blast(muzzle, { x: 0, y: 0.05, z: -1 })
    for (let i = 0; i < 10; i++) {
      const px = (Math.random() - 0.5) * 6, pz = 20 + Math.random() * 4
      shots.pelletStrike({ x: px, y: height(px, pz) + 0.1, z: pz })
    }
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
  shots.flashes.update(dt)
  move(params.has('still') ? 0 : 0.0015)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
