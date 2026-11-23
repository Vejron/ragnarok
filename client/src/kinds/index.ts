/**
 * Every vehicle kind as this client presents it, one file per kind. A new kind is its file and a line
 * here; a test holds every kind the server defines to having one.
 */

import { AIR_AMBULANCE } from './airambulance'
import { AMV } from './amv'
import { BUS } from './bus'
import { CB90 } from './cb90'
import { DRONE } from './drone'
import { EPA } from './epa'
import { SAAB_900 } from './saab900'
import { SAAB_9000 } from './saab9000'
import { VOLVO_240 } from './volvo240'
import { VOLVO_740 } from './volvo740'
import { GTI } from './gti'
import { TRANSPORTER } from './transporter'
import { KADETT } from './kadett'
import { OMEGA } from './omega'
import { SIERRA } from './sierra'
import { ESCORT } from './escort'
import { GOLF_CART } from './golfcart'
import { HELI } from './heli'
import { HEN } from './hen'
import { JEEP } from './jeep'
import type { VehicleKind } from './kind'
import { LOITER } from './loiter'
import { MISSILE } from './missile'
import { MORTAR } from './mortar'
import { PAK } from './pak'
import { PLANE } from './plane'
import { RAFT } from './raft'
import { SEAPLANE } from './seaplane'
import { ROADSTER } from './roadster'
import { SHIP } from './ship'
import { SHIP_GUN } from './shipgun'
import { SNOWMOBILE } from './snowmobile'
import { SNOWRACER } from './snowracer'
import { SPORTS } from './sports'
import { PBV } from './pbv'
import { STRF } from './strf'
import { STRV } from './strv'
import { SUBMARINE } from './submarine'
import { TANK } from './tank'
import { TECHNICAL } from './technical'
import { TITAN } from './titan'
import { TUPP } from './tupp'
import { WALKER } from './walker'

export type { VehicleKind } from './kind'

export const KINDS: readonly VehicleKind[] = [JEEP, TANK, HELI, SPORTS, PLANE, WALKER, STRV, MORTAR, ROADSTER, DRONE, HEN, LOITER, PAK, TUPP, SNOWMOBILE, SHIP, SHIP_GUN, GOLF_CART, TECHNICAL, SUBMARINE, MISSILE, SEAPLANE, STRF, SNOWRACER, TITAN, BUS, PBV, AMV, CB90, RAFT, AIR_AMBULANCE, EPA, SAAB_900, SAAB_9000, VOLVO_240, VOLVO_740, GTI, TRANSPORTER, KADETT, OMEGA, SIERRA, ESCORT]

const byKind = new Map(KINDS.map((kind) => [kind.kind, kind]))

/** How a vehicle of `kind` is presented. Unknown kinds are jeeps. */
export const kindView = (kind: number): VehicleKind => byKind.get(kind) ?? JEEP

/** Fetches what every kind's models are built from. */
export const loadVehicleModels = () => Promise.all([...new Set(KINDS.flatMap((kind) => (kind.load ? [kind.load] : [])))].map((load) => load()))
