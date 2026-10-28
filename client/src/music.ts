import { audioContext } from './audio'

/**
 * Music for the tank crews: one song that fades in when the player takes a tank seat and out when they
 * leave. It streams through an audio element and nothing is requested until the first time it is wanted,
 * so joining a game costs nothing. A missing file or a browser that will not play leaves it silent.
 */

const TRACK = `${import.meta.env?.BASE_URL ?? '/'}music/set_the_world_on_fire.m4a`
const LEVEL = 0.35
/** Time constants of the fades, s: a fade is about three of them. */
const FADE_IN = 0.8
const FADE_OUT = 0.6
/** The song stops this long after the fade-out starts, ms. */
const PAUSE_AFTER = 2500

export class TankMusic {
  private audio: HTMLAudioElement | null = null
  private gain: GainNode | null = null
  private wanted = false
  private timer: ReturnType<typeof setTimeout> | null = null

  /** `context` replaces the shared one, for tests. */
  constructor(private readonly provided?: BaseAudioContext, private readonly make: () => HTMLAudioElement = () => new Audio()) {}

  /** Whether the song should be playing; call every frame, only a change does anything. */
  want(on: boolean) {
    if (on === this.wanted) return
    this.wanted = on
    if (on) this.start()
    else this.stop()
  }

  private start() {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    if (!this.audio && !this.open()) {
      this.wanted = false
      return
    }
    const { audio, gain } = this
    const ctx = this.provided ?? audioContext()
    if (!audio || !gain || !ctx) return
    gain.gain.cancelScheduledValues(ctx.currentTime)
    gain.gain.setTargetAtTime(LEVEL, ctx.currentTime, FADE_IN)
    try {
      void audio.play().catch(() => {
        // Not allowed yet, or no such file: silent, and the next boarding tries again.
        if (this.audio === audio) this.wanted = false
      })
    } catch {
      this.wanted = false
    }
  }

  /** Creates the element and its graph on the first demand. */
  private open(): boolean {
    const ctx = this.provided ?? audioContext()
    if (!ctx || typeof Audio === 'undefined') return false
    try {
      const audio = this.make()
      audio.preload = 'none'
      audio.loop = true
      audio.src = TRACK
      const gain = ctx.createGain()
      gain.gain.value = 0
      ;(ctx as AudioContext).createMediaElementSource(audio).connect(gain).connect(ctx.destination)
      this.audio = audio
      this.gain = gain
      return true
    } catch {
      return false
    }
  }

  private stop() {
    const ctx = this.provided ?? audioContext()
    if (!this.audio || !this.gain || !ctx) return
    this.gain.gain.cancelScheduledValues(ctx.currentTime)
    this.gain.gain.setTargetAtTime(0, ctx.currentTime, FADE_OUT)
    if (this.timer) clearTimeout(this.timer)
    // Pausing keeps the place in the song for the next time.
    this.timer = setTimeout(() => {
      this.timer = null
      if (!this.wanted) this.audio?.pause()
    }, PAUSE_AFTER)
  }

  dispose() {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    this.wanted = false
    this.audio?.pause()
    this.gain?.disconnect()
    this.audio = null
    this.gain = null
  }
}
