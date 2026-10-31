/**
 * Every vehicle kind as this client presents it, one file per kind. A new kind is its file and a line
 * here; a test holds every kind the server defines to having one.
 */

import { DRONE } from './drone'
import { HELI } from './heli'
import { HEN } from './hen'
import { JEEP } from './jeep'
import type { VehicleKind } from './kind'
import { LOITER } from './loiter'
import { MORTAR } from './mortar'
import { PAK } from './pak'
import { PLANE } from './plane'
import { ROADSTER } from './roadster'
import { SPORTS } from './sports'
import { STRV } from './strv'
import { TANK } from './tank'
import { TUPP } from './tupp'
import { WALKER } from './walker'

export type { VehicleKind } from './kind'

export const KINDS: readonly VehicleKind[] = [JEEP, TANK, HELI, SPORTS, PLANE, WALKER, STRV, MORTAR, ROADSTER, DRONE, HEN, LOITER, PAK, TUPP]

const byKind = new Map(KINDS.map((kind) => [kind.kind, kind]))

/** How a vehicle of `kind` is presented. Unknown kinds are jeeps. */
export const kindView = (kind: number): VehicleKind => byKind.get(kind) ?? JEEP

/** Fetches what every kind's models are built from. */
export const loadVehicleModels = () => Promise.all([...new Set(KINDS.flatMap((kind) => (kind.load ? [kind.load] : [])))].map((load) => load()))
