import { defineConfig } from 'vite'

// The dev server serves the example pages; the engine itself is consumed as TypeScript source.
export default defineConfig({
  root: 'examples',
  // The game's models, for the examples that load them.
  publicDir: '../../client/public',
  server: { port: 5180 },
})
