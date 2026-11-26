/** The line of keys shown for a seat, by what the HUD calls it, and which seats aim with a reticle. */

import type { HudStats } from '@/hud'

type Seat = NonNullable<HudStats['seat']>
type Stats = Pick<HudStats, 'ammo' | 'reload' | 'range' | 'canSwitchSeat'> & Partial<Pick<HudStats, 'depth' | 'tubes'>> & { jammed: boolean; heat: number | null }

const SWITCH = ' · R switch seat'
/** The driver's lamps (headlights, or a searchlight), on and off. */
const LIGHTS = ' · L lights'
const HITCH = 'H hitch a mortar or a Pak'
/** How any helicopter is flown, whatever else its pilot can do. */
const FLY = 'Mouse turn (it banks into the turn at speed) · W/S pitch · A/D roll (it holds the lean you leave it at: fly it back level) · Space raise collective · Shift lower it'
const swapRound = (ammo: Stats['ammo']) => (ammo === 'HE' ? 'load AP' : 'load HE')
const loaded = (reload: number) => (reload < 1 ? 'reloading…' : 'loaded')

const HINTS: Record<Seat, (stats: Stats) => string> = {
  driver: ({ canSwitchSeat }) => `W/S gas and brake · A/D steer or turn · Space handbrake · ${HITCH}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  roadster: ({ canSwitchSeat }) => `W/S gas and brake · A/D steer · Shift boost · Space hop, hold to hover · C handbrake · ${HITCH}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  sled: ({ canSwitchSeat }) => `W/S gas and brake · A/D steer, hard at speed to drift · Shift boost · Space handbrake · ${HITCH}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  helm: () => 'W set sail · S back the sail, then the sweeps astern · A/D rudder, under way · Space anchor · Sail across the wind: into it she barely moves · L lights',
  hen: ({ canSwitchSeat }) => `W/S walk · A/D turn · Space leap, hold to flap, then glide · W in the air to fly forward · Click or F lay an egg${canSwitchSeat ? SWITCH : ''}`,
  tupp: ({ canSwitchSeat }) => `W/S walk · A/D turn · Mouse aim the eyes (the head turns within 50°) · Hold click or F to charge the laser · Space leap, hold to flap, then glide${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  walker: ({ canSwitchSeat }) => `W/S walk · A/D turn · Mouse aim the guns (the body follows within 30°) · Click or F fire${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  titan: ({ jammed, heat, tubes }) => {
    const gun = jammed ? 'jammed!' : (heat ?? 0) > 0.6 ? 'overheating' : 'ready'
    const missiles = !tubes ? '' : tubes.loaded > 0 ? `${tubes.loaded} ${tubes.loaded === 1 ? 'missile' : 'missiles'} ready` : 'tubes reloading…'
    return `W/S walk · A/D turn · Mouse aim the fists · Left click punch · Hold right click fire the rotary cannon in the right fist (${gun}) · G launch a missile from the shoulder and fly it (${missiles}) · Hold Shift to reach out open-handed, then left click to close on what is there, right click to punch · Let go to throw · C crouch to reach the ground · Space leap, hold it to fly on the thrusters while the fuel lasts, and hold it again to break a fall or come down hard without · B look out from its head, or from behind${LIGHTS}`
  },
  strv: ({ ammo, reload }) => `W/S gas and brake · A/D turn the tank to aim · Mouse look · Hold right mouse for the gun sight · Click or F fire · Q ${swapRound(ammo)} · L lights · ${ammo} ${loaded(reload)}`,
  mortar: ({ range, reload }) => `Mouse aim sets the bearing · W/S range ${Math.round(range)} m · Hold right-click to lay from above · Click or F fire · ${loaded(reload)}`,
  drone: () => 'Mouse look and steer · W fly where you look · S back off · A/D strafe · Space climb · Shift sink · Click or F detonate · E recall · L lights',
  loiter: () => 'Mouse steer (it banks and turns to where you look) · W/S pitch · A/D roll · Right-click swap camera · Click or F detonate · E recall',
  captain: ({ depth, canSwitchSeat }) => `Depth ${(depth ?? 0).toFixed(1)} m · W/S ahead and astern · A/D rudder, under way · Shift dive · Space rise${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  periscope: ({ tubes, canSwitchSeat }) => {
    const state = !tubes ? '' : !tubes.clear ? 'too deep to fire: rise to periscope depth' : tubes.loaded > 0 ? `${tubes.loaded} ${tubes.loaded === 1 ? 'tube' : 'tubes'} ready` : 'tubes reloading…'
    return `Mouse turn the periscope · Hold right mouse to zoom · Click or F launch a missile · ${state}${canSwitchSeat ? SWITCH : ''}`
  },
  missile: () => 'Mouse steer (it banks and turns to where you look) · W/S pitch · A/D roll · Click or F detonate · E let it go',
  pilot: ({ canSwitchSeat }) => `${FLY} · Click or F rockets${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  airPilot: ({ canSwitchSeat }) => `${FLY}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  aviator: ({ canSwitchSeat }) => `Mouse steer (the plane turns to where you look) · Space throttle up · Shift throttle down · W/S/A/D by hand · Click or F fire · C bomb${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  passenger: ({ canSwitchSeat }) => `Mouse look · Click or F fire your gun · Right mouse aim · 1-5 or wheel switch · R reload${canSwitchSeat ? ', or switch seat when it is full' : ''}`,
  rider: ({ canSwitchSeat }) => `Mouse look · E get out${canSwitchSeat ? SWITCH : ''}`,
  gunner: ({ ammo, jammed, heat, reload, canSwitchSeat }) => {
    const state = jammed ? 'jammed!' : heat !== null ? (heat > 0.6 ? 'overheating' : 'firing') : loaded(reload)
    return `Mouse aim · Hold right mouse for the gun sight · Click or F fire${ammo ? ` · Q ${swapRound(ammo)}` : ''} · ${ammo ? `${ammo} ` : ''}${state}${canSwitchSeat ? SWITCH : ''}`
  },
  tankDriver: ({ ammo, jammed, heat, reload, canSwitchSeat }) => {
    const state = jammed ? 'jammed!' : heat !== null ? (heat > 0.6 ? 'overheating' : 'firing') : loaded(reload)
    return `W/S gas and brake · A/D turn · Mouse aim the turret (nobody is in the gunner's seat) · Hold right mouse for the gun sight · Click or F fire${ammo ? ` · Q ${swapRound(ammo)}` : ''} · ${ammo ? `${ammo} ` : ''}${state}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`
  },
  coxswain: ({ jammed, heat, canSwitchSeat }) =>
    `W ahead · S buckets down: a crash stop, then astern · A/D steer, even standing still · Mouse look · The machine guns fire ahead: turn the boat to aim, the mouse tilts them · Click or F fire · ${jammed ? 'jammed!' : (heat ?? 0) > 0.6 ? 'overheating' : 'ready'}${LIGHTS}${canSwitchSeat ? SWITCH : ''}`,
  roofgunner: ({ jammed, heat, canSwitchSeat }) => `Mouse aim (within 90° of the cannon) · Click or F fire · ${jammed ? 'jammed!' : (heat ?? 0) > 0.6 ? 'overheating' : 'ready'}${canSwitchSeat ? SWITCH : ''}`,
}

/** What to show from `seat`, or null out of a vehicle. */
export const seatHint = (seat: HudStats['seat'], stats: Stats) => (seat ? HINTS[seat](stats) : null)

/** Seats that aim at the middle of the screen, with a reticle while not looking down a sight. */
export const aimsReticle = (seat: HudStats['seat']) => seat === 'gunner' || seat === 'tankDriver' || seat === 'roofgunner' || seat === 'coxswain' || seat === 'walker' || seat === 'titan' || seat === 'tupp' || seat === 'strv' || seat === 'mortar'
