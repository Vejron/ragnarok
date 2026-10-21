import type { ArcView, MarkerView } from './feedback'
import type { ItemSlot } from './sim/items'
import type { ConnectionStatus } from './net/transport'
import type { NoticeView } from './notices'
import type { Blip } from './blips'
import type { FeedEntry, Side } from './scoreboard'
import type { VoiceStatus } from './voice'

/** Everything the overlay draws, assembled by the game once a frame. */
export interface HudStats {
  status: ConnectionStatus
  rtt: number | null
  players: number
  /** Proximity voice: the microphone, whether it is open, and the voices in range. */
  voice: VoiceStatus
  interpDelayMs: number
  /** Map tiles loaded around the player. */
  tiles: number
  correction: number
  pending: number
  locked: boolean
  /** The player is on a touch screen. */
  touch: boolean
  warning: string | null
  /** A hint for the key that does something right now, or null. */
  prompt: string | null
  /** Set while in a vehicle: the seat's role. A plane's pilot is an aviator; a helicopter's, a pilot. */
  seat: 'driver' | 'roadster' | 'pilot' | 'aviator' | 'walker' | 'hen' | 'strv' | 'mortar' | 'drone' | 'loiter' | 'passenger' | 'gunner' | null
  /** A Roadster's driver: what is left in the air tank, 0..1; a hen's: its wings' stamina; null elsewhere. */
  air: number | null
  /**
   * Flying a drone: its battery, 0..1, and the link's strength, 1 at the pilot's side to 0 at the edge of its range;
   * for a loitering drone also which camera is watched (null for an FPV drone's one); null when not flying one.
   */
  drone: { battery: number; signal: number; camera: 'forward' | 'down' | null } | null
  /** Strength 0..1 of the static that follows a drone's feed cutting out. */
  feedLost: number
  /** Seated with a free seat to move to: R switches to it. */
  canSwitchSeat: boolean
  /** For a gunner: how loaded the gun is, 0 (just fired) to 1 (ready). */
  reload: number
  /** A mortar's gunner: the range dial, metres; 0 elsewhere. */
  range: number
  /** In a plane: the heat of the guns in hand, 0..1 (they jam at 1), and whether they have; null elsewhere. */
  heat: number | null
  jammed: boolean
  /** A plane's pilot: bombs left on the rack, or null elsewhere. */
  bombs: number | null
  /** A hen's driver: eggs left in the clutch, or null elsewhere. */
  eggs: number | null
  /** A plane's pilot: flying too slowly for the wings. */
  stall: boolean
  hp: number
  maxHp: number
  /** Dead and waiting to respawn. */
  dead: boolean
  /** Seconds until respawn while dead. */
  respawnIn: number
  /** Looking down a gun sight: the reticle is drawn in the scene, and the on-screen aim marks are hidden. */
  sight: boolean
  /** Looking down a scope: the scope overlay replaces the crosshair. */
  scoped: boolean
  /** Aiming a gun over the shoulder: half zoom, tighter spread. */
  aiming: boolean
  /** A gun in hand, on foot or in a passenger seat: show the crosshair. */
  armed: boolean
  /** Connected, alive and with their own items to hand (standing, or a car's passenger): the health bar and the hotbar are shown. */
  hands: boolean
  /** The five hotbar slots. */
  hotbar: ItemSlot[]
  /** The slot in hand. */
  selected: number
  /** How far a reload or a bandage has got, 0..1, or 0 when neither is going on. */
  progress: number
  kills: number
  deaths: number
  /** Recent kills, oldest first. */
  feed: FeedEntry[]
  /** What the world's scenario has said: the objective, recent messages, and a win or a loss. */
  notices: NoticeView
  /** Every player's tally by side, best first; empty unless the scoreboard key is held. */
  board: Side[]
  /** The marker for a hit just landed, or null. */
  hitMarker: MarkerView | null
  /** Strength 0..1 of the red flash for harm just taken. */
  hurt: number
  /** Where recent harm came from, as screen angles. */
  hurtArcs: ArcView[]
  /** The player's place on the map, metres (x east, z south), and where they look, radians. */
  x: number
  z: number
  yaw: number
  /** Half the side of the playable square, metres; 0 before the world has loaded. */
  halfExtent: number
  /** What else the map marks: allies in view, and recent gunfire. */
  blips: Blip[]
}
