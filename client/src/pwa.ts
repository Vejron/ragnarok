import { ref } from 'vue'
import { registerSW } from 'virtual:pwa-register'

/** How often an open page asks whether a new build is out. */
const CHECK_EVERY_MS = 10 * 60 * 1000

/** A new build is downloaded and waits for `applyUpdate`. */
export const updateReady = ref(false)

let registration: ServiceWorkerRegistration | undefined
let update: ((reload?: boolean) => Promise<void>) | undefined

/** Registers the service worker, and checks for a new build now and then and whenever the page comes back. */
export function registerPwa() {
  if (!import.meta.env.PROD || !('serviceWorker' in navigator)) return
  update = registerSW({
    onNeedRefresh: () => (updateReady.value = true),
    onRegisteredSW(_url, found) {
      registration = found
      setInterval(() => void check(), CHECK_EVERY_MS)
      // An installed app on a phone is resumed rather than reloaded.
      document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && void check())
    },
  })
}

async function check() {
  if (navigator.onLine) await registration?.update().catch(() => {})
}

/** Switches to the waiting build and reloads. */
export function applyUpdate() {
  void update?.(true)
}

/**
 * For a page that does not match the server (another protocol, build or world): forgets the world the
 * service worker kept, fetches the new build if there is one, and reloads into it. `reason` names what
 * the server runs; a page reloads once per reason, so a server ahead of every build there is leaves
 * the page standing with its warning rather than reloading round and round. False when it will not
 * reload: it already did for this reason.
 */
export async function updateNow(reason: string): Promise<boolean> {
  const key = 'ragnarok:reloaded-for'
  try {
    if (sessionStorage.getItem(key) === reason) return false
    sessionStorage.setItem(key, reason)
  } catch {
    return false
  }
  // The manifest is asked for afresh anyway (`loadManifest`); this also drops a minimap kept from before.
  await globalThis.caches?.delete('world').catch(() => false)
  await check()
  if (registration?.waiting || registration?.installing) {
    updateReady.value = true
    // Lets an installing worker finish; `onNeedRefresh` has run by the time it waits.
    const waiting = registration.waiting ?? (await installed(registration.installing!))
    if (waiting) {
      applyUpdate()
      return true
    }
  }
  location.reload()
  return true
}

function installed(worker: ServiceWorker): Promise<ServiceWorker | null> {
  return new Promise((resolve) => {
    worker.addEventListener('statechange', () => {
      if (worker.state === 'installed') resolve(worker)
      else if (worker.state === 'redundant') resolve(null)
    })
  })
}
