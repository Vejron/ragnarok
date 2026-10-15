/**
 * The benchmark scene as plain data, so the engine example and the three example build exactly the same
 * thing: `count` meshes of three shapes, each with its own material colour, on a ground, ringed by an
 * instanced forest. Seeded, so every run and both renderers get the same layout.
 */
export interface Prop {
  shape: 0 | 1 | 2
  color: [number, number, number]
  position: [number, number, number]
  scale: [number, number, number]
  yaw: number
  spins: boolean
}

export interface Tree {
  position: [number, number, number]
  scale: number
}

export const SKY = 0x9fd3f0
export const GROUND = 0x7fa65a
export const LEAVES = 0x3f7a35
export const CEL_BANDS = [96, 150, 205, 255]

export function layout(count: number, trees = 2000) {
  let seed = 1
  const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646
  const props: Prop[] = []
  for (let i = 0; i < count; i++) {
    const color: [number, number, number] = [0.2 + random() * 0.8, 0.2 + random() * 0.8, 0.2 + random() * 0.8]
    const r = 8 + random() * 70
    const a = random() * Math.PI * 2
    const s = 0.6 + random() * 2
    const sy = s * (0.5 + random())
    props.push({
      shape: (i % 3) as 0 | 1 | 2,
      color,
      position: [Math.cos(a) * r, s * 0.5, Math.sin(a) * r],
      scale: [s, sy, s],
      yaw: random() * Math.PI,
      spins: i % 10 === 0,
    })
  }
  const forest: Tree[] = []
  for (let i = 0; i < trees; i++) {
    const r = 90 + random() * 100
    const a = random() * Math.PI * 2
    const s = 0.7 + random() * 0.8
    forest.push({ position: [Math.cos(a) * r, 2.5 * s, Math.sin(a) * r], scale: s })
  }
  return { props, forest }
}
