import { Material, Ramp, type MaterialParams } from '@ragnarok/engine'

/** Hard bands of light from the shaded side to the lit one: the cel look, nothing blended between them. */
export const CEL_RAMP = new Ramp(new Uint8Array([96, 150, 205, 255]))

/** A toon material banded by the cel ramp, unless `params` brings its own ramp. */
export function toonMaterial(params: MaterialParams = {}) {
  return new Material({ ramp: CEL_RAMP, ...params })
}
