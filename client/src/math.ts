/** Small numeric helpers: the ones the rest of the client needs in more than one place. */

export const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value))

export const lerp = (a: number, b: number, t: number) => a + (b - a) * t

/** 0 below `edge0`, 1 above `edge1`, and the smooth step between them. */
export function smoothstep(edge0: number, edge1: number, x: number) {
  const t = clamp((x - edge0) / (edge1 - edge0), 0, 1)
  return t * t * (3 - 2 * t)
}
