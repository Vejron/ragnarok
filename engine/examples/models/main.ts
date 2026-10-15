import { Animator, Fog, Ink, loadGlb, Material, Mesh, Node, planeGeometry, Ramp, Scene, SkinnedMesh } from '../../src'
import { orbit, setup, statsOverlay } from '../common'
import { CEL_BANDS, SKY } from '../shared/layout'

// The game's models: 64 skinned players (the most a server holds), each blending walk, run and idle on
// its own clock, and every vehicle model parked around them.
const params = new URLSearchParams(location.search)
const players = Number(params.get('n') ?? 64)
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
scene.sun.shadow.span = 90
scene.toneMapping = 'neutral'
if (params.get('ink') !== '0') scene.ink = new Ink()
const cel = new Ramp(new Uint8Array(CEL_BANDS))

const ground = new Mesh(planeGeometry(200, 200), new Material({ color: 0x7fa65a, ramp: cel }))
ground.receiveShadow = true
scene.add(ground)

/** Toon-lit and shadowed, as the game's material swap does. */
function prepare(root: Node) {
  root.traverse((node) => {
    if (!(node instanceof Mesh)) return
    node.material.ramp = cel
    node.material.needsUpdate()
    node.castShadow = true
    node.receiveShadow = true
  })
}

const player = await loadGlb('/models/player.glb')
prepare(player.scene)
const crowd: { animator: Animator; root: Node; speed: number; heading: number }[] = []
const side = Math.ceil(Math.sqrt(players))
for (let i = 0; i < players; i++) {
  const root = player.scene.clone()
  root.position.set(((i % side) - side / 2) * 3, 0, (Math.floor(i / side) - side / 2) * 3)
  root.quaternion.setFromAxisAngle(0, 1, 0, (i * 2.4) % (Math.PI * 2))
  // Each player gets its own outfit colour, as the game does: a material per player.
  root.traverse((node) => {
    if (node instanceof SkinnedMesh && i % 2) {
      const material = new Material({ color: (i * 0x2f5b7) & 0xffffff, ramp: cel, vertexColors: node.material.vertexColors })
      node.material = material
    }
  })
  scene.add(root)
  const animator = new Animator(root, player.clips)
  // Times are driven by hand, as the game does: phase for locomotion, its own clock for idle.
  for (const action of animator.actions.values()) action.timeScale = 0
  crowd.push({ animator, root, speed: (i % 5) / 4, heading: 0 })
}

const vehicles = ['jeep', 'sports', 'tank', 'strvs', 'heli', 'plane', 'walker']
await Promise.all(vehicles.map(async (name, i) => {
  const { scene: model } = await loadGlb(`/models/${name}.glb`)
  prepare(model)
  const a = (i / vehicles.length) * Math.PI * 2
  model.position.set(Math.cos(a) * 32, 0, Math.sin(a) * 32)
  model.quaternion.setFromAxisAngle(0, 1, 0, -a)
  scene.add(model)
}))

const tick = statsOverlay(renderer, () => `players ${players}  vehicles ${vehicles.length}`)
const move = orbit(canvas, camera, 40, 14)
let last = performance.now()
let phase = 0
const frame = (now: number) => {
  const dt = Math.min(0.05, (now - last) / 1000)
  last = now
  phase += dt
  crowd.forEach((p, i) => {
    // Blend idle → walk → run by speed, the way the game blends by gait.
    const speed = p.speed
    const idle = p.animator.action('idle')!, walk = p.animator.action('walk_fwd')!, run = p.animator.action('run_fwd')!
    idle.weight = Math.max(0, 1 - speed * 2)
    walk.weight = 1 - Math.abs(speed * 2 - 1)
    run.weight = Math.max(0, speed * 2 - 1)
    const stride = (phase * (0.9 + speed) + i * 0.13) % 1
    idle.time = (phase + i * 0.4) % idle.clip.duration
    walk.time = stride * walk.clip.duration
    run.time = stride * run.clip.duration
    p.animator.apply()
  })
  move(params.has('still') ? 0 : 0.002)
  renderer.render(scene, camera)
  tick()
  requestAnimationFrame(frame)
}
requestAnimationFrame(frame)
