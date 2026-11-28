import { createHash } from 'node:crypto'
import { createReadStream, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { extname, join, normalize } from 'node:path'
import { fileURLToPath, URL } from 'node:url'

import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import ui from '@nuxt/ui/vite'
import { VitePWA } from 'vite-plugin-pwa'

const WORLDS = fileURLToPath(new URL('../worlds', import.meta.url))
const TYPES: Record<string, string> = { '.json': 'application/json', '.png': 'image/png', '.webp': 'image/webp', '.msgpack': 'application/msgpack' }

/** Serves the map editor's worlds (`worlds/<name>/world.msgpack`, `tiles/`) at `/worlds/` in dev. */
const editorWorlds = {
  name: 'editor-worlds',
  configureServer(server: import('vite').ViteDevServer) {
    server.middlewares.use('/worlds', (req, res, next) => {
      const path = normalize(join(WORLDS, decodeURIComponent((req.url ?? '/').split('?')[0]!)))
      if (!path.startsWith(WORLDS) || !existsSync(path) || !statSync(path).isFile()) return next()
      res.setHeader('content-type', TYPES[extname(path)] ?? 'application/octet-stream')
      res.setHeader('cache-control', 'no-cache')
      if (req.method === 'HEAD') return res.end()
      createReadStream(path).pipe(res)
    })
  },
}

const SHOTS = fileURLToPath(new URL('../shots', import.meta.url))
/** Biggest screenshot taken, bytes. */
const MOST_SHOT = 64 * 1024 * 1024

/** Saves the game's F2 screenshots (`src/screenshot.ts`) as `shots/<name>.png` at the repo root, in dev only. */
const screenshots = {
  name: 'screenshots',
  configureServer(server: import('vite').ViteDevServer) {
    server.middlewares.use('/__shots', (req, res) => {
      const name = new URL(req.url ?? '/', 'http://localhost').searchParams.get('name') ?? ''
      if (req.method !== 'POST' || !/^[\w.-]+\.png$/.test(name)) {
        res.statusCode = 400
        return res.end()
      }
      const parts: Buffer[] = []
      let size = 0
      req.on('data', (part: Buffer) => {
        size += part.length
        if (size <= MOST_SHOT) parts.push(part)
      })
      req.on('end', () => {
        if (size > MOST_SHOT) {
          res.statusCode = 413
          return res.end()
        }
        mkdirSync(SHOTS, { recursive: true })
        writeFileSync(join(SHOTS, name), Buffer.concat(parts))
        res.setHeader('content-type', 'application/json')
        res.end(JSON.stringify({ path: `shots/${name}` }))
      })
    })
  },
}

/**
 * The installable app: the service worker precaches the shell (HTML, JS, CSS, wasm) and caches the
 * game's assets as they are first used. It waits to take over until the page says so (`src/pwa.ts`).
 */
const pwa = VitePWA({
  registerType: 'prompt',
  injectRegister: false,
  manifest: {
    name: 'Ragnarok',
    short_name: 'Ragnarok',
    start_url: '/',
    scope: '/',
    display: 'fullscreen',
    display_override: ['fullscreen', 'standalone'],
    orientation: 'landscape',
    theme_color: '#9fd3f0',
    background_color: '#9fd3f0',
    icons: [
      { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
      { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
      { src: 'maskable-icon-512x512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
    ],
  },
  workbox: {
    // The shell only: the world, tiles, models and sounds are cached at run time, below.
    globPatterns: ['*.{html,ico,svg}', 'assets/**/*.{js,css,wasm,woff2}'],
    // Players never open the map editor.
    globIgnores: ['editor.html', 'assets/editor-*'],
    maximumFileSizeToCacheInBytes: 16 * 1024 * 1024,
    cleanupOutdatedCaches: true,
    clientsClaim: true,
    navigateFallback: '/index.html',
    navigateFallbackDenylist: [/^\/(transport|api|worlds|local-certificate-hash|certificate-hash|\.well-known)/, /^\/(settings|editor)/],
    runtimeCaching: [
      // `fetchTile` asks with the manifest's hash in the query, so a cached tile is never stale.
      {
        urlPattern: /\/tiles\/[^/]+\.msgpack\?h=/,
        handler: 'CacheFirst',
        options: { cacheName: 'tiles', expiration: { maxEntries: 4000, purgeOnQuotaError: true } },
      },
      // The map's tiles, asked for with their render's hash (`maptiles/index.json`), so likewise never stale.
      {
        urlPattern: /\/maptiles\/\d+\/\d+_\d+\.webp\?h=/,
        handler: 'CacheFirst',
        options: { cacheName: 'maptiles', expiration: { maxEntries: 2000, purgeOnQuotaError: true } },
      },
      // Must match the server's world, so always asked for; the cache is for a flaky connection.
      {
        urlPattern: /\/(world\.msgpack|minimap\.webp|maptiles\/index\.json|worlds\/active\.json)$/,
        handler: 'NetworkFirst',
        options: { cacheName: 'world', networkTimeoutSeconds: 5 },
      },
      // Models and sounds are asked for with their content's hash (`assetUrl`), so they too are never stale.
      {
        urlPattern: /\/(models|sfx)\/[^/]+\?h=/,
        handler: 'CacheFirst',
        options: { cacheName: 'models', expiration: { maxEntries: 400, purgeOnQuotaError: true } },
      },
      // Not hashed: served from the cache, refreshed behind it for the next launch.
      {
        urlPattern: /\/music\//,
        handler: 'StaleWhileRevalidate',
        options: { cacheName: 'assets', expiration: { maxEntries: 20, maxAgeSeconds: 60 * 60 * 24 * 30, purgeOnQuotaError: true } },
      },
    ],
  },
})

/** This build's id: a new one each time the client is built. */
const BUILD_ID = Date.now().toString(36)

/**
 * Names the build: `dist/build.txt`, which the server reads and names in `welcome`, and `__BUILD_ID__`
 * in the page, so a page a service worker kept from an earlier build knows it is one and reloads
 * (NETWORKING.md, "Same build, same world"). Empty in development, where nothing is checked.
 */
const buildId: Plugin = {
  name: 'build-id',
  config: (_, { command }) => ({ define: { __BUILD_ID__: JSON.stringify(command === 'build' ? BUILD_ID : '') } }),
  generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'build.txt', source: BUILD_ID })
  },
}

const PUBLIC = fileURLToPath(new URL('./public', import.meta.url))

/**
 * `virtual:assets`: the hash of every model and sound under `public/`, by its path. They keep their
 * names in the build, so the page asks for each with its hash (`assetUrl`, `?h=`), which lets the
 * server and the service worker keep it for good: a deploy then sends again only what changed.
 */
const assetHashes: Plugin = {
  name: 'asset-hashes',
  resolveId: (id) => (id === 'virtual:assets' ? '\0virtual:assets' : undefined),
  load(id) {
    if (id !== '\0virtual:assets') return
    const hashes: Record<string, string> = {}
    for (const dir of ['models', 'sfx']) {
      for (const name of readdirSync(join(PUBLIC, dir)).sort()) {
        hashes[`${dir}/${name}`] = createHash('sha256').update(readFileSync(join(PUBLIC, dir, name))).digest('hex').slice(0, 12)
      }
    }
    return `export default ${JSON.stringify(hashes)}`
  },
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), ui({ ui: { colors: { primary: 'amber', neutral: 'zinc' } } }), editorWorlds, screenshots, buildId, assetHashes, pwa],
  server: {
    proxy: {
      // The map editor's API (`cargo run -p editor`).
      '/api': 'http://127.0.0.1:8700',
      // The dev server's self-signed certificate hash, so the browser can pin it for WebTransport.
      '/local-certificate-hash': {
        target: 'http://127.0.0.1:8699',
        rewrite: () => '/certificate-hash',
      },
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        editor: fileURLToPath(new URL('./editor.html', import.meta.url)),
        settings: fileURLToPath(new URL('./settings.html', import.meta.url)),
      },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      // The render engine is a sibling package consumed as source: no build step.
      '@ragnarok/engine': fileURLToPath(new URL('../engine/src/index.ts', import.meta.url)),
    },
  },
})
