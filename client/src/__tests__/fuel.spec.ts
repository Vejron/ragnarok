import { describe, expect, it } from 'vitest'
import { atFuelStation, fuelGauge } from '@/fuel'
import { FUEL_STATION_REACH } from '@/sim/consts'

const STATIONS: [number, number][] = [[100, -40], [-300, 250]]

describe('the fuel gauge', () => {
  it('says a vehicle is being filled only within reach of a station, as the server fills it', () => {
    expect(atFuelStation(STATIONS, 100 + FUEL_STATION_REACH - 1, -40)).toBe(true)
    expect(atFuelStation(STATIONS, 100 + FUEL_STATION_REACH + 1, -40)).toBe(false)
    expect(fuelGauge(0.4, STATIONS, -300, 260)).toEqual({ left: 0.4, filling: true })
  })

  it('does not say so away from a station, whatever the level does, nor once the tank is full', () => {
    // Under fire a holed tank leaks, and a snapshot drawn a moment late shows more in it than the one before.
    expect(fuelGauge(0.52, STATIONS, 0, 0).filling).toBe(false)
    expect(fuelGauge(0.5, STATIONS, 0, 0).filling).toBe(false)
    expect(fuelGauge(1, STATIONS, 100, -40).filling).toBe(false)
  })
})
