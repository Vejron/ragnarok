/// <reference types="vite/client" />
/// <reference types="vite-plugin-pwa/client" />

interface ImportMetaEnv {
  /** Where the world files are served from (default `/`); `/dev/` for the small dev world. */
  readonly VITE_WORLD_BASE?: string
}

/** The client build's id (`vite.config.ts`), which the server names in `welcome`; empty in development. */
declare const __BUILD_ID__: string
