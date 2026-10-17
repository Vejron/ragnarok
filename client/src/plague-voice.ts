import { audioContext } from './audio'
import type { Mood } from './plague-script'
import { type Spoken, muffle } from './plague-lines'

/**
 * The Orange Plague's voice. His lines are recordings where there are some
 * (`client/public/voice/plague/`, listed in its `manifest.json`, made by `tools/voice/plague_voice.mjs`),
 * played through Web Audio: a little slowed and deepened for his size, with a slap of echo off the
 * buildings, and muffled once his mouth is taped. Where there is no recording, the browser's own voice
 * says the line instead, the best one it has, phrase by phrase in his mood.
 *
 * Either way `level` says how wide his mouth should be open, for the model to move his lips.
 */

const BASE = `${import.meta.env?.BASE_URL ?? '/'}voice/plague/`
/** Recordings play this much slower (and so deeper) than they were made: a 40 m man. */
const GIANT_RATE = 0.94
/** His voice carries: full within this far of him, metres, and never below `FAR_VOLUME`. */
const NEAR = 120
const FAR = 1500
const FAR_VOLUME = 0.35
/** Through tape: only the low hum of it gets out, Hz. */
const MUFFLE_HZ = 450

/** How each mood is said by a browser voice: pitch and rate relative to his own, and loudness. */
const DELIVERY: Record<Mood, { pitch: number; rate: number; volume: number }> = {
  boast: { pitch: 1.0, rate: 0.92, volume: 1 },
  smug: { pitch: 0.95, rate: 0.86, volume: 0.9 },
  angry: { pitch: 1.12, rate: 1.06, volume: 1 },
  pain: { pitch: 1.25, rate: 1.0, volume: 1 },
  whine: { pitch: 1.2, rate: 0.9, volume: 0.9 },
}
/** His own register with a browser voice: low. */
const BASE_PITCH = 0.62

/** Words he leans on: said slower. */
const DRAWN = /\b(folks|believe me|tremendous|very|total|totally|huge|beautiful|sad|the best|nobody|everybody|everything)\b/i

/**
 * Ranks a browser voice for him: an English one, ideally one of the natural-sounding neural voices,
 * and a man's by name where the name says so. Higher is better.
 */
export function voiceScore(voice: Pick<SpeechSynthesisVoice, 'name' | 'lang' | 'localService'>): number {
  const name = voice.name.toLowerCase()
  const lang = voice.lang.toLowerCase()
  if (!lang.startsWith('en')) return -1
  let score = lang === 'en-us' ? 3 : 1
  if (/natural|neural|premium|enhanced|online/.test(name)) score += 6
  if (/\b(guy|davis|tony|christopher|eric|roger|brian|andrew|steffan|daniel|alex|evan|nathan|aaron|arthur|tom)\b/.test(name)) score += 4
  if (/\b(female|woman|zira|aria|jenny|samantha|victoria|karen|moira|tessa|fiona|ava|allison|susan|serena|kate|libby|sonia|emma|michelle)\b/.test(name)) score -= 5
  // Novelty voices that sing or whisper.
  if (/bells|bubbles|boing|cellos|organ|whisper|zarvox|trinoids|bad news|good news|jester|superstar|wobble|hysterical|fred|ralph|albert|junior|grandpa|grandma|rocko|eddy|flo|reed|sandy|shelley/.test(name)) score -= 20
  return score
}

/** The phrases of a line, each with its own delivery: a sentence or a breath at a time. */
export function phrases(text: string): { text: string; pitch: number; rate: number }[] {
  return (text.match(/[^.!?…]+[.!?…]*/g) ?? [text])
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
    .map((part) => {
      const shout = part.endsWith('!') || (part.length > 3 && part === part.toUpperCase())
      const trailing = part.endsWith('...') || part.endsWith('-')
      return {
        text: part,
        pitch: shout ? 1.1 : trailing ? 0.92 : 1,
        rate: (shout ? 1.08 : 1) * (DRAWN.test(part) ? 0.88 : 1) * (trailing ? 0.9 : 1),
      }
    })
}

export class PlagueVoice {
  private readonly buffers = new Map<string, AudioBuffer>()
  private manifest: Promise<void> | null = null
  private source: AudioBufferSourceNode | null = null
  private analyser: AnalyserNode | null = null
  private samples: Float32Array<ArrayBuffer> | null = null
  private voice: SpeechSynthesisVoice | null = null
  /** While the browser voice talks: when its last word started, to flap his lips on. */
  private speaking = false
  private wordAt = 0
  private volume = 1

  /** Starts loading his recordings (if there are any) and picking a browser voice. Harmless to repeat. */
  load() {
    if (this.manifest) return
    this.manifest = this.fetchAll()
    if (typeof speechSynthesis !== 'undefined') {
      this.pickVoice()
      // Browsers list their voices late; pick again when they do.
      speechSynthesis.addEventListener?.('voiceschanged', () => this.pickVoice())
    }
  }

  private async fetchAll() {
    const ctx = audioContext()
    if (!ctx || typeof fetch !== 'function') return
    try {
      const response = await fetch(`${BASE}manifest.json`)
      if (!response.ok) return
      const clips = (await response.json()) as string[]
      await Promise.all(
        clips.map(async (clip) => {
          try {
            const file = await fetch(`${BASE}${clip}`)
            if (!file.ok) return
            this.buffers.set(clip.replace(/\.[a-z0-9]+$/, ''), await ctx.decodeAudioData(await file.arrayBuffer()))
          } catch {
            // That one is missing or undecodable: the browser voice says it.
          }
        }),
      )
    } catch {
      // No recordings: the browser voice says everything.
    }
  }

  private pickVoice() {
    const voices = speechSynthesis.getVoices()
    let best: SpeechSynthesisVoice | null = null
    for (const voice of voices) if (voiceScore(voice) >= 0 && (!best || voiceScore(voice) > voiceScore(best))) best = voice
    this.voice = best
  }

  /** How loud he is where the listener is, `distance` metres from him. */
  setDistance(distance: number) {
    this.volume = distance <= NEAR ? 1 : Math.max(FAR_VOLUME, 1 - (distance - NEAR) / (FAR - NEAR))
  }

  /** Says `line`, cutting off whatever he was saying. `gagged` muffles it, his mouth taped shut. */
  say(line: Spoken, gagged: boolean) {
    this.stop()
    if (typeof document !== 'undefined' && document.hidden) return
    const buffer = this.buffers.get(line.clip)
    if (buffer && this.play(buffer, gagged)) return
    this.speak(gagged ? muffle(line.speech) : line.speech, line.mood)
  }

  private play(buffer: AudioBuffer, gagged: boolean) {
    const ctx = audioContext()
    if (!ctx) return false
    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.playbackRate.value = GIANT_RATE
    // A shade of low end for his size, and the tape when there is tape.
    const body = ctx.createBiquadFilter()
    body.type = 'lowshelf'
    body.frequency.value = 220
    body.gain.value = 4
    const tone = ctx.createBiquadFilter()
    tone.type = 'lowpass'
    tone.frequency.value = gagged ? MUFFLE_HZ : 16000
    const gain = ctx.createGain()
    gain.gain.value = this.volume
    // The echo off the city: one late, quieter repeat, darker than the voice.
    const echo = ctx.createDelay(1)
    echo.delayTime.value = 0.18
    const echoTone = ctx.createBiquadFilter()
    echoTone.type = 'lowpass'
    echoTone.frequency.value = 1800
    const echoGain = ctx.createGain()
    echoGain.gain.value = 0.22
    const analyser = ctx.createAnalyser()
    analyser.fftSize = 512
    source.connect(body).connect(tone).connect(gain)
    gain.connect(ctx.destination)
    gain.connect(echo).connect(echoTone).connect(echoGain).connect(ctx.destination)
    tone.connect(analyser)
    source.onended = () => {
      if (this.source === source) {
        this.source = null
        this.analyser = null
      }
    }
    source.start()
    this.source = source
    this.analyser = analyser
    this.samples = new Float32Array(analyser.fftSize)
    return true
  }

  private speak(text: string, mood: Mood) {
    try {
      if (typeof speechSynthesis === 'undefined') return
      const delivery = DELIVERY[mood]
      const parts = phrases(text)
      parts.forEach((part, i) => {
        const utterance = new SpeechSynthesisUtterance(part.text)
        if (this.voice) utterance.voice = this.voice
        utterance.pitch = Math.min(2, BASE_PITCH * delivery.pitch * part.pitch)
        utterance.rate = delivery.rate * part.rate
        utterance.volume = Math.min(1, delivery.volume * this.volume)
        utterance.onstart = () => {
          this.speaking = true
          this.wordAt = performance.now()
        }
        utterance.onboundary = () => {
          this.wordAt = performance.now()
        }
        if (i === parts.length - 1) utterance.onend = utterance.onerror = () => (this.speaking = false)
        speechSynthesis.speak(utterance)
      })
    } catch {
      // No voice: the subtitle is enough.
    }
  }

  /** How open his mouth should be now, 0..1: from the sound itself, or flapping on the browser's words. */
  level(): number {
    if (this.analyser && this.samples) {
      this.analyser.getFloatTimeDomainData(this.samples)
      let sum = 0
      for (const sample of this.samples) sum += sample * sample
      return Math.min(1, Math.sqrt(sum / this.samples.length) * 6)
    }
    if (!this.speaking) return 0
    // Open on each word, closing over the next fifth of a second, and a little chatter between.
    const since = (performance.now() - this.wordAt) / 1000
    return Math.max(0, 0.8 - since * 3) + 0.25 * Math.abs(Math.sin(performance.now() / 70))
  }

  stop() {
    try {
      this.source?.stop()
    } catch {
      // Already over.
    }
    this.source = null
    this.analyser = null
    this.speaking = false
    try {
      if (typeof speechSynthesis !== 'undefined') speechSynthesis.cancel()
    } catch {
      // Nothing to cancel.
    }
  }
}
