/** The line of keys shown for a seat, by what the HUD calls it, and which seats aim with a reticle. */

import type { HudStats } from '@/hud'

type Seat = NonNullable<HudStats['seat']>
type Stats = Pick<HudStats, 'ammo' | 'reload' | 'range' | 'canSwitchSeat'> & { jammed: boolean; heat: number | null }

const SWITCH = ' · R switch seat'
const HITCH = 'H hitch a mortar or a Pak'
const swapRound = (ammo: Stats['ammo']) => (ammo === 'HE' ? 'load AP' : 'load HE')
const loaded = (reload: number) => (reload < 1 ? 'reloading…' : 'loaded')

const HINTS: Record<Seat, (stats: Stats) => string> = {
  driver: ({ canSwitchSeat }) => `W/S gas and brake · A/D steer or turn · Space handbrake · ${HITCH}${canSwitchSeat ? SWITCH : ''}`,
  roadster: ({ canSwitchSeat }) => `W/S gas and brake · A/D steer · Shift boost · Space hop, hold to hover · C handbrake · ${HITCH}${canSwitchSeat ? SWITCH : ''}`,
  hen: ({ canSwitchSeat }) => `W/S walk · A/D turn · Space leap, hold to flap, then glide · W in the air to fly forward · Click or F lay an egg${canSwitchSeat ? SWITCH : ''}`,
  walker: ({ canSwitchSeat }) => `W/S walk · A/D turn · Mouse aim the guns (the body follows within 30°) · Click or F fire${canSwitchSeat ? SWITCH : ''}`,
  strv: ({ ammo, reload }) => `W/S gas and brake · A/D turn the tank to aim · Mouse look · Hold right mouse for the gun sight · Click or F fire · Q ${swapRound(ammo)} · ${ammo} ${loaded(reload)}`,
  mortar: ({ range, reload }) => `Mouse aim sets the bearing · W/S range ${Math.round(range)} m · Click or F fire · ${loaded(reload)}`,
  drone: () => 'Mouse look and steer · W fly where you look · S back off · A/D strafe · Space climb · Shift sink · Click or F detonate · E recall',
  loiter: () => 'Mouse steer (it banks and turns to where you look) · W/S pitch · A/D roll · Right-click swap camera · Click or F detonate · E recall',
  pilot: ({ canSwitchSeat }) => `W/S lean forward and back · A/D turn · Space climb · Shift sink${canSwitchSeat ? SWITCH : ''}`,
  aviator: ({ canSwitchSeat }) => `Mouse steer (the plane turns to where you look) · Space throttle up · Shift throttle down · W/S/A/D by hand · Click or F fire · C bomb${canSwitchSeat ? SWITCH : ''}`,
  passenger: ({ canSwitchSeat }) => `Mouse look · Click or F fire your gun · Right mouse aim · 1-5 or wheel switch · R reload${canSwitchSeat ? ', or switch seat when it is full' : ''}`,
  gunner: ({ ammo, jammed, heat, reload, canSwitchSeat }) => {
    const state = jammed ? 'jammed!' : heat !== null ? 'firing' : loaded(reload)
    return `Mouse aim · Hold right mouse for the gun sight · Click or F fire${ammo ? ` · Q ${swapRound(ammo)}` : ''} · ${ammo ? `${ammo} ` : ''}${state}${canSwitchSeat ? SWITCH : ''}`
  },
}

/** What to show from `seat`, or null out of a vehicle. */
export const seatHint = (seat: HudStats['seat'], stats: Stats) => (seat ? HINTS[seat](stats) : null)

/** Seats that aim at the middle of the screen, with a reticle while not looking down a sight. */
export const aimsReticle = (seat: HudStats['seat']) => seat === 'gunner' || seat === 'walker' || seat === 'strv' || seat === 'mortar'
