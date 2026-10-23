// The stickman's physics is the server's, built to WASM (`npm run build:wasm`, run before the tests).
// In the browser `SimMotor.create` fetches it; Node's fetch cannot read files, so it is loaded here.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { initSync } from '@/sim/wasm/sim'

initSync({ module: readFileSync(resolve(process.cwd(), 'src/sim/wasm/sim_bg.wasm')) })
