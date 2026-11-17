/*
 * Whether the world is a winter one (`Manifest::theme`), for everything that picks a colour by it. The
 * main thread sets it when the manifest arrives and the tile worker when it is told to start, both
 * before the first tile. No scene or DOM here, so it loads in the worker.
 */

export let winter = false

export function setWinter(on: boolean) {
  winter = on
}

/** Whether the lakes and rivers are frozen: a winter world, unless it keeps its open water (`frozen`). */
export let iced = false

export function setIced(on: boolean) {
  iced = on
}
