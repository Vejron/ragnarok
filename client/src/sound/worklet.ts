import workletUrl from './engine.worklet.ts?worker&url'

/**
 * Loads the engine and grit processors (`engine.worklet.ts`) into a context, once. Until they have
 * loaded, or in a browser without audio worklets, `ready` is false and vehicles are heard without their
 * engines and their crunch: everything else still plays.
 */

const loaded = new WeakMap<BaseAudioContext, boolean>()

/** Whether `ctx` can make `engine` and `grit` nodes yet; starts loading them the first time it is asked. */
export function workletReady(ctx: BaseAudioContext): boolean {
  const state = loaded.get(ctx)
  if (state !== undefined) return state
  loaded.set(ctx, false)
  const worklet = (ctx as Partial<BaseAudioContext>).audioWorklet
  if (!worklet || typeof AudioWorkletNode === 'undefined') return false
  worklet.addModule(workletUrl).then(
    () => loaded.set(ctx, true),
    (error: unknown) => console.warn('engine sounds could not load', error),
  )
  return false
}

/** Marks `ctx` as having the processors already, for tests that stub the nodes. */
export function assumeWorklet(ctx: BaseAudioContext) {
  loaded.set(ctx, true)
}
