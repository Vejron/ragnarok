import { createReadStream, existsSync, statSync } from 'node:fs'
import { extname, join, normalize } from 'node:path'
import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

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

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), editorWorlds],
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
