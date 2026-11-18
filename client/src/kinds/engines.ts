import type { SynthSpec } from '@/sound/enginesynth'

/**
 * The engines the vehicles carry, as `EngineSynth` builds their sound: how many cylinders fire and how
 * evenly, how long and lively the exhaust is, how hard a diesel knocks, and what whines or roars on top.
 * Each kind's `sound` (in its own file) names one and says how its revs follow its speed.
 */

const NONE = { cylinders: 0, stroke: 4, imbalance: 0, jitter: 0, pulse: 1, knock: 0, pipe: 0.5, resonance: 0, body: 400, intake: 0, mech: 0, crackle: 0, whine: 0, whineRatio: 1, roar: 0, roarHz: 1000 } as const

/** The Centurion's Rolls-Royce Meteor: a 27-litre petrol V12 off a Spitfire, deep, smooth and slow to rev, through long pipes. */
export const METEOR_V12: SynthSpec = { ...NONE, cylinders: 12, imbalance: 0.3, jitter: 0.15, pulse: 3.5, pipe: 2.8, resonance: 0.62, body: 170, intake: 0.35, mech: 0.12, crackle: 0.45, whine: 0.05, whineRatio: 9, gain: 1.3 }

/** The Strv 103's six-cylinder diesel with its Boeing gas turbine whining over it: the two run together. */
export const DIESEL_AND_TURBINE: SynthSpec = { ...NONE, cylinders: 6, imbalance: 0.22, jitter: 0.2, pulse: 2.5, knock: 0.6, pipe: 1.6, resonance: 0.5, body: 220, intake: 0.3, mech: 0.3, whine: 0.4, whineRatio: 40, roar: 0.28, roarHz: 2600, gain: 1.15 }

/** A big turbocharged V8 diesel (the Strf 90's Scania): a heavy, lumpy beat with the turbo's whistle. */
export const DIESEL_V8: SynthSpec = { ...NONE, cylinders: 8, imbalance: 0.32, jitter: 0.2, pulse: 2.5, knock: 0.5, pipe: 1.8, resonance: 0.55, body: 200, intake: 0.4, mech: 0.25, whine: 0.16, whineRatio: 22, roar: 0.08, roarHz: 3000, gain: 1.15 }

/** A six-cylinder truck diesel (the Pbv 302's Volvo, the AMV's Scania): a steady clattering drone. */
export const DIESEL_SIX: SynthSpec = { ...NONE, cylinders: 6, imbalance: 0.25, jitter: 0.2, pulse: 2.4, knock: 0.55, pipe: 1.5, resonance: 0.5, body: 220, intake: 0.35, mech: 0.3, whine: 0.1, whineRatio: 18, gain: 1.15 }

/** A city bus's diesel, under the back seats: low and soft, with a turbo sigh. */
export const BUS_DIESEL: SynthSpec = { ...DIESEL_SIX, imbalance: 0.2, knock: 0.4, pipe: 2.4, body: 160, whine: 0.12, whineRatio: 20 }

/** A pickup's four-cylinder diesel: rough and rattly, knocking hard at idle. */
export const DIESEL_FOUR: SynthSpec = { ...NONE, cylinders: 4, imbalance: 0.35, jitter: 0.3, pulse: 2.2, knock: 0.7, pipe: 1.4, resonance: 0.45, body: 260, intake: 0.35, mech: 0.35, gain: 1.15 }

/** A jeep's petrol straight six: smooth, with a growl when it is pushed. */
export const PETROL_SIX: SynthSpec = { ...NONE, cylinders: 6, imbalance: 0.1, jitter: 0.1, pulse: 2, pipe: 1.6, resonance: 0.5, body: 300, intake: 0.5, mech: 0.2, crackle: 0.1, gain: 1.1 }

/** A sports car's cross-plane V8: the uneven burble at idle, a hard bark revved, and crackles off the throttle. */
export const PETROL_V8: SynthSpec = { ...NONE, cylinders: 8, imbalance: 0.45, jitter: 0.12, pulse: 1.6, pipe: 1.1, resonance: 0.65, body: 400, intake: 0.6, mech: 0.2, crackle: 0.9, gain: 1.05 }

/** A snowmobile's two-stroke twin with its tuned pipe: the high, angry "ring-ding". */
export const TWO_STROKE_TWIN: SynthSpec = { ...NONE, cylinders: 2, stroke: 2, firing: [0, 0.5], imbalance: 0.15, jitter: 0.3, pulse: 1, pipe: 0.9, resonance: 0.75, body: 600, intake: 0.5, mech: 0.05, crackle: 0.3, gain: 1 }

/** A lawnmower-sized single two-stroke: a thin, buzzing rasp. */
export const TWO_STROKE_SINGLE: SynthSpec = { ...NONE, cylinders: 1, stroke: 2, imbalance: 0, jitter: 0.35, pulse: 0.8, pipe: 0.5, resonance: 0.5, body: 700, intake: 0.4, mech: 0.05, gain: 0.95 }

/** A loitering drone's pusher two-stroke, with its propeller's buzz. */
export const PUSHER_TWO_STROKE: SynthSpec = { ...TWO_STROKE_SINGLE, pipe: 0.35, resonance: 0.4, body: 900, jitter: 0.25, roar: 0.1, roarHz: 3000 }

/** A 1910s rotary: nine cylinders spinning with the propeller, no pipes, rattling, popping when the blip switch cuts it. */
export const ROTARY_NINE: SynthSpec = { ...NONE, cylinders: 9, imbalance: 0.2, jitter: 0.25, pulse: 3, pipe: 0.4, resonance: 0.3, body: 500, intake: 0.2, mech: 0.5, crackle: 0.5, gain: 1.2 }

/** A 1920s seven-cylinder radial: deeper and steadier than the rotary, with stub pipes. */
export const RADIAL_SEVEN: SynthSpec = { ...NONE, cylinders: 7, imbalance: 0.2, jitter: 0.2, pulse: 3.5, pipe: 0.7, resonance: 0.35, body: 380, intake: 0.25, mech: 0.35, crackle: 0.25, gain: 1.2 }

/** A helicopter's turboshaft: a high whine over the roar of its exhaust. */
export const TURBOSHAFT: SynthSpec = { ...NONE, whine: 0.5, whineRatio: 4, roar: 0.55, roarHz: 1500, gain: 2.4 }

/** A drone's four motors and their props: a thin whine with a breath of wash. */
export const DRONE_MOTORS: SynthSpec = { ...NONE, whine: 0.7, whineRatio: 2, roar: 0.25, roarHz: 4000, gain: 3 }

/** A rocket motor: all roar. */
export const ROCKET: SynthSpec = { ...NONE, roar: 1, roarHz: 2200, gain: 1.2 }

/** A strong electric motor (the Roadster's): a clean whine climbing with the speed, and the gears' hiss. */
export const ELECTRIC: SynthSpec = { ...NONE, whine: 0.9, whineRatio: 5, roar: 0.05, roarHz: 6000, gain: 4 }

/** A golf cart's small motor: a faint whine. */
export const SMALL_ELECTRIC: SynthSpec = { ...NONE, whine: 0.5, whineRatio: 8, gain: 4 }

/** A submarine's electric motors: a low hum. */
export const SUB_MOTORS: SynthSpec = { ...NONE, whine: 0.6, whineRatio: 6, gain: 4 }

/** The Stridsbåt 90's two V8 diesels, sixteen cylinders between them, and the roar of their waterjets. */
export const TWIN_DIESEL_JETS: SynthSpec = { ...DIESEL_V8, cylinders: 16, imbalance: 0.3, knock: 0.4, pipe: 2, body: 220, whine: 0.2, whineRatio: 25, roar: 0.35, roarHz: 1200 }
