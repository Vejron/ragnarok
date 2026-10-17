import {
  BARK_ADAPT_AIR, BARK_ADAPT_ARMOR, BARK_ADAPT_INFANTRY, BARK_ARRIVE, BARK_BLIND, BARK_DOWN, BARK_EAR, BARK_EYE, BARK_FIRED, BARK_GAGGED, BARK_HURT,
  BARK_IDLE, BARK_LEAVE, BARK_NEMESIS, BARK_REZONE, BARK_STOMP, BARK_STRIKE,
} from './net/codec'
import { type Line, type Mood, SCRIPT, type Topic, clipName } from './plague-script'

/** What each `BARK_*` trigger has him talk about (`plague-script.ts`). */
const TOPICS: Record<number, Topic> = {
  [BARK_ARRIVE]: 'arrive',
  [BARK_STOMP]: 'stomp',
  [BARK_STRIKE]: 'strike',
  [BARK_HURT]: 'hurt',
  [BARK_FIRED]: 'fired',
  [BARK_ADAPT_INFANTRY]: 'adaptInfantry',
  [BARK_ADAPT_ARMOR]: 'adaptArmor',
  [BARK_ADAPT_AIR]: 'adaptAir',
  [BARK_REZONE]: 'rezone',
  [BARK_IDLE]: 'idle',
  [BARK_DOWN]: 'down',
  [BARK_LEAVE]: 'leave',
  [BARK_NEMESIS]: 'nemesis',
  [BARK_EYE]: 'eye',
  [BARK_BLIND]: 'blind',
  [BARK_EAR]: 'ear',
  [BARK_GAGGED]: 'gagged',
}

/** A line to say: the subtitle, the recording of it (if there is one), and how to say it without one. */
export interface Spoken {
  text: string
  /** What the browser's voice says when the recording is missing: the subtitle, name and all. */
  speech: string
  clip: string
  mood: Mood
}

/**
 * The line for a bark, with `{name}` filled in, or null if the trigger has none. The server picks a
 * number; the line is `variant` modulo how many there are. A line naming someone is only used when
 * there is someone, and one without a name always can be.
 */
export function plagueLine(trigger: number, variant: number, name: string | null): Spoken | null {
  const topic = TOPICS[trigger]
  if (topic === undefined) return null
  const all: readonly Line[] = SCRIPT[topic]
  const indices = all.map((_, i) => i)
  const usable = name ? indices : indices.filter((i) => !all[i]!.text.includes('{name}'))
  const pool = usable.length > 0 ? usable : indices
  const index = pool[variant % pool.length]!
  const line = all[index]!
  const text = line.text.replaceAll('{name}', name ?? 'you')
  return { text, speech: text, clip: clipName(topic, index), mood: line.mood }
}

/**
 * The line as it comes out through tape over his mouth: every word a muffled "mmf", as long as the
 * word was, the punctuation kept.
 */
export function muffle(text: string): string {
  return text.replace(/[A-Za-z']+/g, (word) => {
    const loud = word.length > 1 && word === word.toUpperCase()
    const body = 'M' + 'm'.repeat(Math.min(4, Math.max(1, word.length - 2))) + (word.length > 3 ? 'ph' : 'f')
    return loud ? body.toUpperCase() : body
  })
}
