import { defineConfig } from 'vite'

// The dev server serves the example pages; the engine itself is consumed as TypeScript source.
export default defineConfig({
  root: 'examples',
  server: { port: 5180 },
})
