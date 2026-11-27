import { PointLights } from './lights'
import { Craters } from './craters'
import { Scars } from './scars'
import { Color, Vec3 } from '../math/vec'
import { Node } from './node'
import type { Sky } from './sky'

/** The sun: a directional light shining from `position` toward `target`, both in world space. */
export class Sun {
  readonly color = new Color(1, 1, 1)
  intensity = 1
  readonly position = new Vec3(0, 1, 0)
  readonly target = new Vec3(0, 0, 0)
  readonly shadow = new SunShadow()
}

/**
 * The sun's shadow: a square of `span` metres around the sun's target, rendered from the sun, and, when
 * `far` is on, a second, coarser square ahead of the camera that carries the shadows on past the first.
 * Each fades out towards its edge, so neither ends in a line.
 */
export class SunShadow {
  enabled = false
  /** Texels along each side of the map. */
  mapSize = 2048
  span = 100
  /** How far casters may be from the target along the sun's direction, metres. */
  depth = 200
  /** Blur of the shadow edge, in texels. */
  radius = 1
  /** Comparisons per shaded pixel: a soft filter (4 or 9 bilinear taps, by `radius`) or 1 (one bilinear tap, cheaper). */
  taps: 1 | 9 = 9
  /**
   * How far the sun must turn before its shadows follow, radians. Between steps the map is drawn from
   * the same direction, so the edges hold still instead of re-rasterising every frame as the sun moves.
   */
  step = 0.001
  /** Whether the far square is drawn. */
  far = false
  /** Texels along each side of the far map. */
  farMapSize = 2048
  /** The far square's side, metres. */
  farSpan = 600
  /** How far its casters may be from its centre along the sun's direction, metres. */
  farDepth = 400
  /** How far ahead of the camera the far square is centred, as a share of `farSpan`. */
  farAhead = 0.35
  /**
   * Frames between redrawing the far map, which is kept in between, where it is, as it was. Its casters
   * are far off and its texels wide, so a few frames' lag does not show, and drawing it every frame cost
   * more than the near map.
   */
  farEvery = 4
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
  /** How far off the lines have faded to nothing, metres; they begin to at about a third of it. */
  fade = 400
  /** How much the line wavers like a pen's, swelling, thinning and now and then running dry: 0 is an even line, 1 the most. */
  waver = 1
  /** How much what is in shadow, or turned from the sun, is shaded with a pen's hatching strokes, 0 (none) to 1. */
  hatch = 0
}

/**
 * Shafts of light from the sun (or the moon) through gaps in whatever stands against the sky: trees,
 * buildings, cloud edges. A radial blur toward the light at a quarter of the screen's size. Needs `Scene.sky`.
 */
export class Shafts {
  /** How much of the shafts' light is added. */
  strength = 0.4
  /** How far toward the light each pixel gathers, as a share of the way there. */
  length = 0.9
  /** What each step toward the light keeps of the one before: lower is shorter shafts. */
  decay = 0.965
}

/** Beams of spot lights (headlights, a headlamp, a searchlight) seen in the air: in mist, rain and the dark. */
export class Beams {
  /** How much light the air scatters per metre: more in mist and rain. 0 for none. */
  density = 0.04
}

/** Glare and ghosts of the sun in a lens, dimmed as the sun goes behind things. Needs `Scene.sky`. */
export class LensFlare {
  /** How bright the glare and the ghosts are, 0 to about 2. */
  strength = 1
}

/**
 * The eye adapting to how bright the view is: a dark forest opens up, a bright sky shuts down a
 * little. The exposure is `(key / average luminance) ^ strength`, held between `min` and `max`, and
 * eases to that at `brighten` per second when the view darkens and `darken` when it brightens.
 */
export class Adaptation {
  /** The average luminance (linear, before exposure) that gets an exposure of 1. */
  key = 0.3
  strength = 0.4
  min = 0.8
  max = 1.6
  brighten = 1.2
  darken = 2.5
}

/** Colour grading before tone mapping: split toning, saturation, the night's blue shift and a vignette. */
export class Grade {
  /** Multiplies the dark parts of the picture and the bright parts (linear), blended by luminance. */
  readonly shadows = new Color(1, 1, 1)
  readonly highlights = new Color(1, 1, 1)
  /** 1 leaves colours as they are, 0 is grey. */
  saturation = 1
  /**
   * How far dim colours shift toward the blue-grey of rod vision, 0 to 1: the Purkinje effect, by
   * moonlight. Bright things (lamps, fire) keep their colour.
   */
  purkinje = 0
  /** How much the corners darken, 0 to 1. */
  vignette = 0
}

/** The root of what is drawn, with the lighting and atmosphere that apply to all of it. */
export class Scene extends Node {
  readonly background = new Color(0, 0, 0)
  readonly sun = new Sun()
  readonly hemisphere = new Hemisphere()
  fog: Fog | null = null
  /** The sky drawn behind the world, which also colours the fog (see `Sky`); null for the flat `background`. */
  sky: Sky | null = null
  ink: Ink | null = null
  /** How far light well past white burns white in the output pass, 0 to 1: the hearts of flashes, tracers and fire. */
  whiteHot = 1
  shafts: Shafts | null = null
  beams: Beams | null = null
  flare: LensFlare | null = null
  adaptation: Adaptation | null = null
  grade: Grade | null = null
  /** Short-lived point lights (blasts, flashes); call `lights.update(dt)` each frame. */
  readonly lights = new PointLights()
  /** Craters the ground's materials (`craters: true`) are marked by: SCORCH.md. */
  readonly craters = new Craters()
  /** Damage drawn on meshes' surfaces (`Mesh.scars`): HULL-DAMAGE.md. */
  readonly scars = new Scars()
  toneMapping: ToneMapping = 'none'
  exposure = 1
}
