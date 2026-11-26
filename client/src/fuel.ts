import { FUEL_STATION_REACH } from '@/sim/consts'

/*
 * Fuel as the HUD shows it (FUEL.md). The server fills a vehicle's tank within `FUEL_STATION_REACH`
 * of a station, and the stations are world data, so the client tells it is being filled by the same
 * rule rather than from its level rising: the level it is drawn with can step back to an older
 * snapshot's, which would read as a rise.
 */

/** Whether a vehicle at `(x, z)`, metres, is within reach of one of `stations`, on the ground plane, as `fuel::at_station` on the server. */
export function atFuelStation(stations: readonly (readonly [number, number])[], x: number, z: number): boolean {
  return stations.some(([sx, sz]) => Math.hypot(sx - x, sz - z) <= FUEL_STATION_REACH)
}

/** The fuel gauge of a vehicle with `left` in its tank (0..1) at `(x, z)`: it is being filled at a station, short of full. */
export function fuelGauge(left: number, stations: readonly (readonly [number, number])[], x: number, z: number): { left: number; filling: boolean } {
  return { left, filling: left < 1 && atFuelStation(stations, x, z) }
}
