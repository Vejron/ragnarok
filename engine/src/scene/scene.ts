import { PointLights } from './lights'
import { Craters } from './craters'
import { Color, Vec3 } from '../math/vec'
import { Node } from './node'

/** The sun: a directional light shining from `position` toward `target`, both in world space. */
export class Sun {
  readonly color = new Color(1, 1, 1)
  intensity = 1
  readonly position = new Vec3(0, 1, 0)
  readonly target = new Vec3(0, 0, 0)
  readonly shadow = new SunShadow()
}

/** The sun's shadow: a square of `span` metres around the sun's target, rendered from the sun. */
export class SunShadow {
  enabled = false
  /** Texels along each side of the map. */
  mapSize = 2048
  span = 100
  /** How far casters may be from the target along the sun's direction, metres. */
  depth = 200
  /** Blur of the shadow edge, in texels. */
  radius = 1
}

/** Sky light from above blending to bounce light from below, by how much a surface faces up. */
export class Hemisphere {
  readonly sky = new Color(1, 1, 1)
  readonly ground = new Color(0.5, 0.5, 0.5)
  intensity = 1
}

/** Linear distance fog, from no fog at `near` to all fog at `far` (view depth, metres). */
export class Fog {
  readonly color = new Color(1, 1, 1)
  constructor(public near = 100, public far = 1000) {}
}

export type ToneMapping = 'none' | 'neutral'

/** Dark outlines wherever depth jumps: every silhouette gets an ink line that darkens what it crosses. */
export class Ink {
  /** What the line keeps of the colour under it at full strength. */
  darken = 0.3
  /** How much of the line shows, 0 to 1. */
  strength = 0.6
  /** How far depth may bend between neighbours, as a share of the depth, before it counts as an edge. */
  threshold = 0.05
  /** Line width in CSS pixels. */
  width = 1.2
}

/** Glow around anything brighter than `threshold` in the HDR image (fire, tracers, muzzle flashes). */
export class Bloom {
  /** How much of the blurred glow is added back. */
  strength = 0.6
  /** Linear HDR brightness where the glow starts. */
  threshold = 1
  /** Softness of that start, in the same units. */
  knee = 0.5
  /** How wide the glow spreads; 1 is the default tent. */
  radius = 1
}

/** The root of what is drawn, with the lighting and atmosphere that apply to all of it. */
export class Scene extends Node {
  readonly background = new Color(0, 0, 0)
  readonly sun = new Sun()
  readonly hemisphere = new Hemisphere()
  fog: Fog | null = null
  ink: Ink | null = null
  bloom: Bloom | null = null
  /** Short-lived point lights (blasts, flashes); call `lights.update(dt)` each frame. */
  readonly lights = new PointLights()
  /** Craters the ground's materials (`craters: true`) are marked by: SCORCH.md. */
  readonly craters = new Craters()
  toneMapping: ToneMapping = 'none'
  exposure = 1
}
