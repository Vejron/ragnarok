/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Where the world files are served from (default `/`); `/dev/` for the small dev world. */
  readonly VITE_WORLD_BASE?: string
}
