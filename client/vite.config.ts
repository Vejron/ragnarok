import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  server: {
    proxy: {
      // The dev server's self-signed certificate hash, so the browser can pin it for WebTransport.
      '/local-certificate-hash': {
        target: 'http://127.0.0.1:8699',
        rewrite: () => '/certificate-hash',
      },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
