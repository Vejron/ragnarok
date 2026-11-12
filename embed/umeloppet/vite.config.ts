// The embed's build. It uses the client's own Vite (see package.json), so nothing is installed here, and
// it imports nothing from 'vite' for the same reason: the config is a plain object.
import { createReadStream, existsSync, statSync } from 'node:fs'
import { extname, join, normalize, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = fileURLToPath(new URL('.', import.meta.url))
const client = resolve(here, '../../client')
/** The world shown by `npm run dev`: `UMELOPPET_WORLD=../../worlds/<name>`, or the umeloppet world. */
const world = resolve(here, process.env.UMELOPPET_WORLD ?? '../../worlds/umeloppet')

/**
 * The client's modules find their files with `import.meta.env.BASE_URL` (models, sounds) and
 * `VITE_WORLD_BASE` (the world), fixed when the game is built. Here they are where the `<umeloppet-map>`
 * on the page says, known only when it runs: so they are read from a global the element sets first.
 */
const runtimeBase = {
  name: 'umeloppet-runtime-base',
  enforce: 'pre' as const,
  transform(code: string, id: string) {
    if (!id.startsWith(client) || !code.includes('import.meta.env')) return null
    const out = code.replace(/import\.meta\.env\??\.(BASE_URL|VITE_WORLD_BASE)/g, 'globalThis.__umeloppetBase')
    return out === code ? null : { code: out, map: null }
  },
}

const TYPES: Record<string, string> = {
  '.json': 'application/json', '.png': 'image/png', '.webp': 'image/webp', '.msgpack': 'application/msgpack', '.glb': 'model/gltf-binary',
  '.ogg': 'audio/ogg', '.mp3': 'audio/mpeg', '.m4a': 'audio/mp4', '.wav': 'audio/wav',
}

/**
 * In `npm run dev`, serves what `scripts/assets.sh` would put in the upload folder at `/world/`: the
 * world's files, and the client's models and sounds.
 */
const devWorld = {
  name: 'umeloppet-dev-world',
  configureServer(server: { middlewares: { use: (path: string, handler: (req: { url?: string; method?: string }, res: { setHeader: (k: string, v: string) => void; end: () => void }, next: () => void) => void) => void } }) {
    server.middlewares.use('/world', (req, res, next) => {
      const path = decodeURIComponent((req.url ?? '/').split('?')[0]!)
      const candidates = [join(world, path), join(client, 'public', path)]
      const file = candidates.map((c) => normalize(c)).find((c) => (c.startsWith(world) || c.startsWith(client)) && existsSync(c) && statSync(c).isFile())
      if (!file) return next()
      res.setHeader('content-type', TYPES[extname(file)] ?? 'application/octet-stream')
      res.setHeader('access-control-allow-origin', '*')
      if (req.method === 'HEAD') return res.end()
      createReadStream(file).pipe(res as never)
    })
  },
}

export default {
  root: here,
  // Every URL the bundle makes is relative to the script, wherever it is hosted.
  base: './',
  publicDir: false,
  plugins: [runtimeBase, devWorld],
  resolve: {
    alias: {
      '@': resolve(client, 'src'),
      '@ragnarok/engine': resolve(here, '../../engine/src/index.ts'),
      // The embed's own modules import packages too, and it has no node_modules of its own: the client's.
      '@msgpack/msgpack': resolve(client, 'node_modules/@msgpack/msgpack'),
    },
  },
  server: { port: 5290, fs: { allow: [resolve(here, '../..')] } },
  worker: { format: 'es' as const },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      input: { umeloppet: resolve(here, 'src/element.ts') },
      preserveEntrySignatures: 'strict' as const,
      output: {
        format: 'es' as const,
        entryFileNames: '[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
      },
    },
  },
}
