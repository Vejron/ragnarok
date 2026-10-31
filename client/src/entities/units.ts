/**
 * Replicated entities in the game's units. Each field of a component names its unit in the Rust schema
 * (`UNITS` in `crates/shared/src/replicated.rs`), which says what the client turns it into and how it
 * blends between two snapshots:
 *
 * - `cm`, `mrad`, `quat` and `axis` are interpolated: positions, angles (the short way round), rotations
 *   (as a whole, normalised) and steering.
 * - Everything else (counters, flags, ids, health, gauges) is taken from the nearer snapshot.
 *
 * Both run for every entity, every snapshot and every frame, so Rust writes them out as code
 * (`units.generated.ts`): nothing here knows any component by name, and a new component, or a new
 * field, is converted and interpolated as its units say.
 */

import type { Components } from '@/net/components.generated'

/** A rotation: a replicated quaternion's x, y, z with its `w` rebuilt. */
export interface Rotation {
  qx: number
  qy: number
  qz: number
  qw: number
}

/** An entity in the game's units: the components it has, every field scaled by its unit. */
export type Entity = Omit<Components, 'orientation'> & { orientation?: Rotation }

/** A gun on a mount (`turret`), in the game's units: radians, and metres of range. */
export type Turret = NonNullable<Entity['turret']>
/** A gun on a turret's roof (`pintle`): radians from the turret, heat as 0..1. */
export type Pintle = NonNullable<Entity['pintle']>
/** A plane's pilot weapons and engine, or a hen's eggs (`arms`), heat as 0..1. */
export type Arms = NonNullable<Entity['arms']>
/**
 * The exact motion of the vehicle the local player drives, which its pose does not carry (`drive`, sent
 * to its driver alone): m/s, rad/s, the front wheels' angle (+ is right) and the yaw rate a tank has
 * ramped to (+ is clockwise).
 */
export type DriveState = NonNullable<Entity['drive']>

export { blend, toGame } from './units.generated'
