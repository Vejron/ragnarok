import { Particles } from './particles'

/** Half the side of the box of air around the camera flakes appear in, and how high it goes, metres. */
const REACH = 38
const HEIGHT = 26
/** Flakes a second. Each lives `LIFE` seconds, so this over the pool's size is how thick it can get. */
const RATE = 170
const LIFE = 11
const POOL = Math.ceil(RATE * LIFE * 1.1)

const FLAKE = { life: LIFE, size: [0.07, 0.07] as [number, number], color: 0xffffff, alpha: 0.9, shape: 'round' as const }

/** Snow falling round the camera, in a pool of its own so that it never takes room from smoke and dust. */
export class Snowfall {
  private readonly pool = new Particles(POOL, false)
  private owed = 0

  get root() {
    return this.pool.root
  }

  update(eye: { x: number; y: number; z: number }, dt: number) {
    this.owed = Math.min(this.owed + RATE * dt, RATE)
    for (; this.owed >= 1; this.owed--) {
      const position = [eye.x + (Math.random() * 2 - 1) * REACH, eye.y + (Math.random() * 0.8 + 0.2) * HEIGHT, eye.z + (Math.random() * 2 - 1) * REACH] as const
      // A slow fall with a little drift, the way large flakes come down in still air.
      this.pool.spawn({ ...FLAKE, position, velocity: [(Math.random() - 0.5) * 0.7, -1.5 - Math.random() * 0.8, (Math.random() - 0.5) * 0.7] })
    }
    this.pool.follow(eye.x, eye.z)
    this.pool.update(dt)
  }

  dispose() {
    this.pool.dispose()
  }
}
