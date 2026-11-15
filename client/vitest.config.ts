import { fileURLToPath } from 'node:url'
import { mergeConfig, defineConfig, configDefaults } from 'vitest/config'
import viteConfig from './vite.config.ts'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      // One jsdom per worker rather than per file (each file still has its own module scope): creating
      // jsdom 119 times was over half the run. Measured 10.5 s with forks, 4.6 s with this.
      pool: 'vmThreads',
      setupFiles: ['src/__tests__/setup.ts'],
      exclude: [...configDefaults.exclude],
      root: fileURLToPath(new URL('./', import.meta.url)),
    },
  }),
)
