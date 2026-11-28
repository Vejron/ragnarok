/// <reference types="vite/client" />
/// <reference types="vite-plugin-pwa/client" />

interface ImportMetaEnv {
  /** Where the world files are served from (default `/`); `/dev/` for the small dev world. */
  readonly VITE_WORLD_BASE?: string
}

/** The client build's id (`vite.config.ts`), which the server names in `welcome`; empty in development. */
declare const __BUILD_ID__: string

/** The hash of each model and sound under `public/`, by its path there (`vite.config.ts`, `src/assets.ts`). */
declare module 'virtual:assets' {
  const hashes: Record<string, string>
  export default hashes
}
