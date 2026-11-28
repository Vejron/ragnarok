import hashes from 'virtual:assets'

const BASE: string = import.meta.env?.BASE_URL ?? '/'

/**
 * Where a model or sound under `public/` is fetched from, e.g. `assetUrl('models/tank.glb')`: its
 * address with its content's hash in the query, so the address names the content and the server and
 * the service worker keep it for good (LOAD-TIME.md). A file the build did not find is asked for bare.
 */
export function assetUrl(path: string): string {
  const hash = hashes[path]
  return hash ? `${BASE}${path}?h=${hash}` : `${BASE}${path}`
}
