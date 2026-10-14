import * as THREE from 'three'

/** Hard bands of light from the shaded side to the lit one: the cel look, nothing blended between them. */
const BANDS = [96, 150, 205, 255]
const CEL_RAMP = (() => {
  const texture = new THREE.DataTexture(new Uint8Array(BANDS), BANDS.length, 1, THREE.RedFormat)
  texture.minFilter = texture.magFilter = THREE.NearestFilter
  texture.needsUpdate = true
  return texture
})()

/** A toon material banded by the cel ramp, unless `params` brings its own ramp. */
export function toonMaterial(params: THREE.MeshToonMaterialParameters = {}) {
  return new THREE.MeshToonMaterial({ gradientMap: CEL_RAMP, ...params })
}
