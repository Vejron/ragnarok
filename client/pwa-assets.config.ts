import { defineConfig, minimal2023Preset } from '@vite-pwa/assets-generator/config'

// `npx pwa-assets-generator`: the app icons in `public/`, from the favicon on the sky colour.
export default defineConfig({
  preset: {
    ...minimal2023Preset,
    maskable: { ...minimal2023Preset.maskable, resizeOptions: { background: '#9fd3f0' } },
    apple: { ...minimal2023Preset.apple, resizeOptions: { background: '#9fd3f0' } },
  },
  images: ['public/favicon.svg'],
})
