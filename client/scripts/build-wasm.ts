// Builds `crates/sim-wasm` for the browser into `src/sim/wasm/` (git-ignored): the server's own physics,
// which the client's prediction runs. `npm run build:wasm`; the dev server, the tests, the type check
// and the build run it first. It is built with the `wasm-dev` profile, which rebuilds in a second or two
// after an edit; `--release` (the build that ships, `npm run build`) uses the release profile. Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI of
// the version `crates/sim-wasm/Cargo.toml` pins (see README.md).
import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const root = resolve(here, '../..')
const out = resolve(here, '../src/sim/wasm')

const run = (command: string, args: string[]) => {
  try {
    execFileSync(command, args, { cwd: root, stdio: 'inherit' })
  } catch (cause) {
    const hint = command === 'wasm-bindgen' ? 'install it with `cargo install wasm-bindgen-cli --version <the pinned one> --locked`' : 'add the target with `rustup target add wasm32-unknown-unknown`'
    throw new Error(`\`${command} ${args.join(' ')}\` failed; ${hint}`, { cause })
  }
}

/** The cargo profile, which is also the name of the directory its output lands in. */
const profile = process.argv.includes('--release') ? 'release' : 'wasm-dev'

run('cargo', ['build', '-p', 'sim-wasm', '--target', 'wasm32-unknown-unknown', '--profile', profile])
run('wasm-bindgen', [resolve(root, `target/wasm32-unknown-unknown/${profile}/sim_wasm.wasm`), '--out-dir', out, '--target', 'web', '--out-name', 'sim'])
