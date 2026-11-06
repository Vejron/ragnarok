import { createReadStream, existsSync, statSync } from 'node:fs'
import { extname, join, normalize } from 'node:path'
import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import ui from '@nuxt/ui/vite'
import { VitePWA } from 'vite-plugin-pwa'

const WORLDS = fileURLToPath(new URL('../worlds', import.meta.url))
const TYPES: Record<string, string> = { '.json': 'application/json', '.png': 'image/png', '.msgpack': 'application/msgpack' }

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
      // Must match the server's world, so always asked for; the cache is for a flaky connection.
      {
        urlPattern: /\/(world\.msgpack|minimap\.png|worlds\/active\.json)$/,
        handler: 'NetworkFirst',
        options: { cacheName: 'world', networkTimeoutSeconds: 5 },
      },
      // Not hashed: served from the cache, refreshed behind it for the next launch.
      {
        urlPattern: /\/(models|sfx|music)\//,
        handler: 'StaleWhileRevalidate',
        options: { cacheName: 'assets', expiration: { maxEntries: 400, maxAgeSeconds: 60 * 60 * 24 * 30, purgeOnQuotaError: true } },
      },
    ],
  },
})

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), ui({ ui: { colors: { primary: 'amber', neutral: 'zinc' } } }), editorWorlds, pwa],
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
