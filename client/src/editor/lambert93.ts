/**
 * Lambert-93 (EPSG:2154), as `crates/shared/src/lambert93.rs` has it: France's grid, which French
 * worlds are laid out in. Degrees to `[easting, northing]` metres and back.
 */

const N = 0.725607765053267
const C = 11754255.426096
const XS = 700000
const YS = 12655612.049876
const LON0 = (3 * Math.PI) / 180
const E = 0.0818191910428158

const isometric = (phi: number): number => {
  const s = E * Math.sin(phi)
  return Math.log(Math.tan(Math.PI / 4 + phi / 2) * Math.pow((1 - s) / (1 + s), E / 2))
}

export function forward(lat: number, lon: number): [number, number] {
  const r = C * Math.exp(-N * isometric((lat * Math.PI) / 180))
  const gamma = N * ((lon * Math.PI) / 180 - LON0)
  return [XS + r * Math.sin(gamma), YS - r * Math.cos(gamma)]
}

export function inverse(e: number, n: number): [number, number] {
  const dx = e - XS
  const dy = n - YS
  const l = -Math.log(Math.hypot(dx, dy) / C) / N
  let phi = 2 * Math.atan(Math.exp(l)) - Math.PI / 2
  for (let i = 0; i < 12; i++) {
    const s = E * Math.sin(phi)
    const next = 2 * Math.atan(Math.pow((1 + s) / (1 - s), E / 2) * Math.exp(l)) - Math.PI / 2
    const done = Math.abs(next - phi) < 1e-12
    phi = next
    if (done) break
  }
  const lambda = LON0 + Math.atan2(dx, -dy) / N
  return [(phi * 180) / Math.PI, (lambda * 180) / Math.PI]
}
