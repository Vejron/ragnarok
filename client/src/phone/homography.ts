/**
 * Where on the phone's screen a point of the page is (PHONE.md). The screen is a rectangle held in
 * view, so on the page it is a quadrilateral, its corners projected each frame; a point inside maps
 * to the screen's own `(u, v)` by the projective map that takes the unit square onto those corners,
 * inverted (Heckbert's square-to-quad).
 */

export type Point = readonly [number, number]

/** A 3 × 3 matrix, row by row. */
type Mat3 = [number, number, number, number, number, number, number, number, number]

/** The projective map taking (0,0), (1,0), (1,1), (0,1) to `corners` in that order. */
function squareToQuad([[x0, y0], [x1, y1], [x2, y2], [x3, y3]]: readonly [Point, Point, Point, Point]): Mat3 {
  const sx = x0 - x1 + x2 - x3
  const sy = y0 - y1 + y2 - y3
  if (Math.abs(sx) < 1e-12 && Math.abs(sy) < 1e-12) {
    // A parallelogram: affine.
    return [x1 - x0, x2 - x1, x0, y1 - y0, y2 - y1, y0, 0, 0, 1]
  }
  const [dx1, dx2, dy1, dy2] = [x1 - x2, x3 - x2, y1 - y2, y3 - y2]
  const det = dx1 * dy2 - dx2 * dy1
  const g = (sx * dy2 - dx2 * sy) / det
  const h = (dx1 * sy - sx * dy1) / det
  return [x1 - x0 + g * x1, x3 - x0 + h * x3, x0, y1 - y0 + g * y1, y3 - y0 + h * y3, y0, g, h, 1]
}

function invert([a, b, c, d, e, f, g, h, i]: Mat3): Mat3 | null {
  const A = e * i - f * h
  const B = -(d * i - f * g)
  const C = d * h - e * g
  const det = a * A + b * B + c * C
  if (Math.abs(det) < 1e-12) return null
  const k = 1 / det
  return [A * k, -(b * i - c * h) * k, (b * f - c * e) * k, B * k, (a * i - c * g) * k, -(a * f - c * d) * k, C * k, -(a * h - b * g) * k, (a * e - b * d) * k]
}

/**
 * The screen's `(u, v)` (0..1 across from its left, down from its top) at page point `p`, given its
 * corners on the page (top left, top right, bottom right, bottom left); null when the corners make no
 * quadrilateral. A point off the screen gives a `(u, v)` outside 0..1.
 */
export function quadToUnit(corners: readonly [Point, Point, Point, Point], [x, y]: Point): Point | null {
  const inverse = invert(squareToQuad(corners))
  if (!inverse) return null
  const [a, b, c, d, e, f, g, h, i] = inverse
  const w = g * x + h * y + i
  if (Math.abs(w) < 1e-12) return null
  return [(a * x + b * y + c) / w, (d * x + e * y + f) / w]
}

/** Where the screen's `(u, v)` lies on the page, given its corners there. */
export function unitToQuad(corners: readonly [Point, Point, Point, Point], [u, v]: Point): Point {
  const [a, b, c, d, e, f, g, h, i] = squareToQuad(corners)
  const w = g * u + h * v + i
  return [(a * u + b * v + c) / w, (d * u + e * v + f) / w]
}
