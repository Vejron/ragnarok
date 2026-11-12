import { Color, Vec3 } from '../math/vec'

/**
 * The sky drawn behind everything, and the fog that fades the world into it: a physical atmosphere
 * (`atmosphere.ts`) lit by the sun, with a sun disc, a moon showing its phase, stars and a layer of
 * clouds that also shades the ground. Set `Scene.sky` to draw it; without one the background is the
 * scene's flat colour and fog its flat colour, as before.
 */
export class Sky {
  /** Unit vector toward the sun. Below the horizon too: the sky glows after sunset. */
  readonly sunDir = new Vec3(0, 1, 0)
  /** Unit vector toward the moon. */
  readonly moonDir = new Vec3(0, 1, 0)
  /** How much of the moon shows, 0 to 1 (none by day). */
  moon = 0
  /**
   * The moon's phase as the angle between the sun and the viewer seen from the moon, radians: 0 full,
   * π new. Only the disc's shading follows it.
   */
  moonPhase = 0
  /** How much of the stars show, 0 to 1. */
  stars = 0
  /** How far the stars have turned about the pole, radians. */
  starTurn = 0
  /** How much of the sky the clouds cover, 0 (clear) to 1 (overcast). */
  coverage = 0.35
  /** How far the wind has carried the clouds, metres along x and z. */
  readonly drift: [number, number] = [0, 0]
  /** How much of the sunlight a cloud overhead takes off the ground, 0 for no cloud shadows. */
  cloudShadow = 0.6
  /**
   * Multiplies the atmosphere's radiance (for a sun of illuminance 1): its brightness and white
   * balance, so the noon horizon can match a palette's sky.
   */
  readonly tint = new Color(1, 1, 1)
  /** The sky's own glow at night, added to the atmosphere's (linear). */
  readonly night = new Color(0, 0, 0)
  /**
   * Mist that lies low: its density per metre at `fogBase` (a world height, metres), falling by e every
   * 1/`fogFalloff` metres up. 0 for none. It takes the sky's colour, so it glows toward a low sun.
   */
  fogDensity = 0
  fogBase = 0
  fogFalloff = 0.05
  /** How bright the sun's disc is against the sunlight on a white cloud. */
  sunDisc = 24
  /** How wet the world is from rain, 0 to 1: darker ground with a sheen of the sky, and puddles on the flat. */
  wetness = 0
  /** How much of a rainbow shows opposite the sun, 0 to 1. */
  rainbow = 0
  /** A lightning flash lighting the sky and clouds, 0 for none; a strong one is about 1.5. */
  flash = 0
}
