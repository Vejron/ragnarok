import { Geometry } from './geometry'

/** An axis-aligned box centred on the origin, with flat faces. */
export function boxGeometry(width = 1, height = 1, depth = 1): Geometry {
  const x = width / 2, y = height / 2, z = depth / 2
  // Each face: normal, then its four corners counter-clockwise seen from outside.
  const faces: [number[], number[][]][] = [
    [[1, 0, 0], [[x, -y, z], [x, -y, -z], [x, y, -z], [x, y, z]]],
    [[-1, 0, 0], [[-x, -y, -z], [-x, -y, z], [-x, y, z], [-x, y, -z]]],
    [[0, 1, 0], [[-x, y, z], [x, y, z], [x, y, -z], [-x, y, -z]]],
    [[0, -1, 0], [[-x, -y, -z], [x, -y, -z], [x, -y, z], [-x, -y, z]]],
    [[0, 0, 1], [[-x, -y, z], [x, -y, z], [x, y, z], [-x, y, z]]],
    [[0, 0, -1], [[x, -y, -z], [-x, -y, -z], [-x, y, -z], [x, y, -z]]],
  ]
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  for (const [normal, corners] of faces) {
    const base = positions.length / 3
    corners.forEach((c, i) => {
      positions.push(...c)
      normals.push(...normal)
      uvs.push(i === 1 || i === 2 ? 1 : 0, i >= 2 ? 1 : 0)
    })
    indices.push(base, base + 1, base + 2, base, base + 2, base + 3)
  }
  return new Geometry({
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    uvs: new Float32Array(uvs),
    indices: new Uint16Array(indices),
  })
}

/** A UV sphere centred on the origin. */
export function sphereGeometry(radius = 1, widthSegments = 16, heightSegments = 12): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  for (let iy = 0; iy <= heightSegments; iy++) {
    const v = iy / heightSegments
    const phi = v * Math.PI
    for (let ix = 0; ix <= widthSegments; ix++) {
      const u = ix / widthSegments
      const theta = u * Math.PI * 2
      const nx = -Math.cos(theta) * Math.sin(phi)
      const ny = Math.cos(phi)
      const nz = Math.sin(theta) * Math.sin(phi)
      positions.push(nx * radius, ny * radius, nz * radius)
      normals.push(nx, ny, nz)
      uvs.push(u, 1 - v)
    }
  }
  const row = widthSegments + 1
  for (let iy = 0; iy < heightSegments; iy++) {
    for (let ix = 0; ix < widthSegments; ix++) {
      const a = iy * row + ix + 1
      const b = iy * row + ix
      const c = (iy + 1) * row + ix
      const d = (iy + 1) * row + ix + 1
      if (iy !== 0) indices.push(a, b, d)
      if (iy !== heightSegments - 1) indices.push(b, c, d)
    }
  }
  return new Geometry({
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    uvs: new Float32Array(uvs),
    indices: new Uint32Array(indices),
  })
}

/** A flat square in the XZ plane facing up, centred on the origin. */
export function planeGeometry(width = 1, depth = 1, segments = 1): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  for (let iz = 0; iz <= segments; iz++) {
    for (let ix = 0; ix <= segments; ix++) {
      positions.push((ix / segments - 0.5) * width, 0, (iz / segments - 0.5) * depth)
      normals.push(0, 1, 0)
      uvs.push(ix / segments, 1 - iz / segments)
    }
  }
  const row = segments + 1
  for (let iz = 0; iz < segments; iz++) {
    for (let ix = 0; ix < segments; ix++) {
      const a = iz * row + ix
      indices.push(a, a + row, a + 1, a + 1, a + row, a + row + 1)
    }
  }
  return new Geometry({
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    uvs: new Float32Array(uvs),
    indices: new Uint32Array(indices),
  })
}

/** A capped cylinder (or cone, with a zero radius) along Y, centred on the origin. */
export function cylinderGeometry(radiusTop = 1, radiusBottom = 1, height = 1, segments = 16): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const indices: number[] = []
  const half = height / 2
  const slope = (radiusBottom - radiusTop) / height
  for (let i = 0; i <= segments; i++) {
    const t = (i / segments) * Math.PI * 2
    const sin = Math.sin(t), cos = Math.cos(t)
    const len = Math.hypot(1, slope)
    positions.push(radiusTop * sin, half, radiusTop * cos, radiusBottom * sin, -half, radiusBottom * cos)
    normals.push(sin / len, slope / len, cos / len, sin / len, slope / len, cos / len)
  }
  for (let i = 0; i < segments; i++) {
    const a = i * 2
    indices.push(a, a + 1, a + 3, a, a + 3, a + 2)
  }
  for (const [y, r, ny] of [[half, radiusTop, 1], [-half, radiusBottom, -1]] as const) {
    if (r <= 0) continue
    const centre = positions.length / 3
    positions.push(0, y, 0)
    normals.push(0, ny, 0)
    for (let i = 0; i <= segments; i++) {
      const t = (i / segments) * Math.PI * 2
      positions.push(r * Math.sin(t), y, r * Math.cos(t))
      normals.push(0, ny, 0)
    }
    for (let i = 0; i < segments; i++) {
      if (ny > 0) indices.push(centre, centre + 1 + i, centre + 2 + i)
      else indices.push(centre, centre + 2 + i, centre + 1 + i)
    }
  }
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), indices: new Uint32Array(indices) })
}

/**
 * A flat rectangle in the XY plane facing +Z, centred on the origin: a billboard or a decal. Its uv
 * origin is the top left, as an image's is, so a canvas drawn on reads the right way up.
 */
export function quadGeometry(width = 1, height = 1): Geometry {
  const x = width / 2, y = height / 2
  return new Geometry({
    positions: new Float32Array([-x, -y, 0, x, -y, 0, x, y, 0, -x, y, 0]),
    normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
    uvs: new Float32Array([0, 1, 1, 1, 1, 0, 0, 0]),
    indices: new Uint16Array([0, 1, 2, 0, 2, 3]),
  })
}

/** A flat ring in the XY plane facing +Z, between `inner` and `outer` radius. */
export function ringGeometry(inner = 0.5, outer = 1, segments = 24): Geometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  for (let i = 0; i <= segments; i++) {
    const t = (i / segments) * Math.PI * 2
    const c = Math.cos(t), s = Math.sin(t)
    positions.push(inner * c, inner * s, 0, outer * c, outer * s, 0)
    normals.push(0, 0, 1, 0, 0, 1)
    uvs.push((c * inner / outer + 1) / 2, (s * inner / outer + 1) / 2, (c + 1) / 2, (s + 1) / 2)
  }
  for (let i = 0; i < segments; i++) {
    const a = i * 2
    indices.push(a, a + 1, a + 3, a, a + 3, a + 2)
  }
  return new Geometry({ positions: new Float32Array(positions), normals: new Float32Array(normals), uvs: new Float32Array(uvs), indices: new Uint32Array(indices) })
}

/**
 * A sphere of triangles from subdividing an icosahedron `detail` times, with shared vertices: 12 at
 * detail 0, 42 at 1. Low detail gives a faceted, gem-like ball. No normals or uvs are produced; call
 * `computeNormals()` for smooth shading.
 */
export function icosphereGeometry(radius = 1, detail = 1): Geometry {
  const t = (1 + Math.sqrt(5)) / 2
  const vertices: number[][] = [
    [-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0],
    [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t],
    [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1],
  ].map((v) => {
    const l = Math.hypot(v[0]!, v[1]!, v[2]!)
    return [v[0]! / l, v[1]! / l, v[2]! / l]
  })
  let faces: number[][] = [
    [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
    [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
    [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
    [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
  ]
  const middle = new Map<number, number>()
  const midpoint = (a: number, b: number) => {
    const key = Math.min(a, b) * 65536 + Math.max(a, b)
    let found = middle.get(key)
    if (found === undefined) {
      const m = [(vertices[a]![0]! + vertices[b]![0]!) / 2, (vertices[a]![1]! + vertices[b]![1]!) / 2, (vertices[a]![2]! + vertices[b]![2]!) / 2]
      const l = Math.hypot(m[0]!, m[1]!, m[2]!)
      vertices.push([m[0]! / l, m[1]! / l, m[2]! / l])
      found = vertices.length - 1
      middle.set(key, found)
    }
    return found
  }
  for (let d = 0; d < detail; d++) {
    const next: number[][] = []
    for (const [a, b, c] of faces) {
      const ab = midpoint(a!, b!), bc = midpoint(b!, c!), ca = midpoint(c!, a!)
      next.push([a!, ab, ca], [b!, bc, ab], [c!, ca, bc], [ab, bc, ca])
    }
    faces = next
  }
  return new Geometry({
    positions: new Float32Array(vertices.flatMap((v) => [v[0]! * radius, v[1]! * radius, v[2]! * radius])),
    indices: new Uint16Array(faces.flat()),
  })
}
