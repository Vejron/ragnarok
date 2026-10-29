// What the WASM sim's hot paths cost over the fixture world: vehicles, a soldier and raycasts, best of
// seven runs. For comparing builds of `crates/sim-wasm` (with SIMD, vehicles step about a third faster
// than without; see README.md). Skipped unless asked for:
//
//   BENCH=1 npx vitest run sim.bench --silent=false
import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { it } from 'vitest'
import { SimMotor, type CarControls, type CharacterState } from '@/sim/motor'
import { TICK_DT } from '@/sim/consts'
import { KIND_JEEP, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_HELI } from '@/net/codec'

const root = resolve(process.cwd(), '../fixtures/world')

it.skipIf(!process.env.BENCH)('sim hot paths', { timeout: 300_000 }, async () => {
  const motor = await SimMotor.create(new Uint8Array(readFileSync(resolve(root, 'world.msgpack'))))
  for (const name of readdirSync(resolve(root, 'tiles')).filter((n) => n.endsWith('.msgpack'))) {
    const [col, row] = name.replace('.msgpack', '').split('_').map(Number) as [number, number]
    motor.addTile({ col, row }, new Uint8Array(readFileSync(resolve(root, 'tiles', name))))
  }
  const kinds = [KIND_JEEP, KIND_SPORTS, KIND_TANK, KIND_STRV, KIND_HELI, KIND_JEEP, KIND_TANK, KIND_SPORTS]
  const controls = (tick: number): CarControls => ({ throttle: Math.floor(tick / 90) % 3 === 2 ? -1 : 1, steer: Math.sin(tick / 40), handbrake: false, lift: 0.6, powered: true })
  const results: Record<string, number> = {}

  const runCars = (ticks: number) => {
    kinds.forEach((kind, i) => motor.startCar(i + 1, kind, [-30 + i * 8, 4, -20], i))
    const start = performance.now()
    for (let tick = 0; tick < ticks; tick++) for (let i = 0; i < kinds.length; i++) motor.driveCar(i + 1, controls(tick + i * 13), TICK_DT)
    const ms = performance.now() - start
    kinds.forEach((_, i) => motor.stopCar(i + 1))
    return ms
  }
  const runSoldier = (ticks: number) => {
    let state: CharacterState = { position: [0, 3, 0], yaw: 0, vy: 0, grounded: false, knock: [0, 0], ride: 0, prone: false, chute: false, spent: 0, winded: false, lost: 0 }
    const start = performance.now()
    for (let seq = 1; seq <= ticks; seq++) {
      state = motor.step(state, { seq, strafe: Math.sin(seq / 50), forward: 1, yaw: seq / 100, buttons: 0, pitch: 0, view: 0, slot: 0 }, TICK_DT)
    }
    return performance.now() - start
  }
  const runRays = (count: number) => {
    const start = performance.now()
    for (let i = 0; i < count; i++) {
      const a = i * 0.618
      motor.castRay([Math.cos(a) * 20, 3, Math.sin(a) * 20], [Math.cos(a * 3), -0.3, Math.sin(a * 3)], 200)
    }
    return performance.now() - start
  }

  // Warm up the JIT, then take the best of several runs.
  runCars(200); runSoldier(500); runRays(2000)
  const best = (run: () => number) => Math.min(...Array.from({ length: 7 }, run))
  results['8 vehicles x 1200 ticks'] = best(() => runCars(1200))
  results['soldier x 5000 ticks'] = best(() => runSoldier(5000))
  results['20000 rays'] = best(() => runRays(20000))
  console.log(Object.entries(results).map(([k, v]) => `${k.padEnd(26)}${v.toFixed(1).padStart(9)} ms`).join('\n'))
  motor.dispose()
})
