#!/usr/bin/env node
/**
 * Records the Orange Plague's lines (client/src/plague-script.ts) with a text-to-speech model, into
 * client/public/voice/plague/<topic>_<n>.<ext> and a manifest.json the client loads them by. Lines
 * already recorded are kept unless --force; --only <topic> records one topic.
 *
 *     node tools/voice/plague_voice.mjs                     (Kokoro, free, on this machine)
 *     OPENAI_API_KEY=... node tools/voice/plague_voice.mjs
 *     ELEVENLABS_API_KEY=... ELEVENLABS_VOICE_ID=... node tools/voice/plague_voice.mjs
 *
 * Kokoro (kokoro-js, `npm install` in tools/voice first) runs locally: nothing leaves the machine. Its
 * voice is KOKORO_VOICE (default am_michael; am_fenrir, am_puck, am_onyx, bm_george are others). It
 * takes no acting directions, so each mood only sets its pace. On macOS the result is encoded to AAC.
 *
 * OpenAI (gpt-4o-mini-tts) takes acting directions, so one stock voice (PLAGUE_VOICE, default "onyx")
 * is directed into the character. ElevenLabs uses the voice you pick by id: pick or design a stock
 * character voice (a booming, older salesman), not a clone of any real person's voice.
 *
 * Needs Node 22.18+ (it reads the TypeScript script directly).
 */
import { mkdir, readdir, rm, writeFile } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { SCRIPT, clipName } from '../../client/src/plague-script.ts'

const root = join(dirname(fileURLToPath(import.meta.url)), '../..')
const out = join(root, 'client/public/voice/plague')
const args = process.argv.slice(2)
const force = args.includes('--force')
const only = args.includes('--only') ? args[args.indexOf('--only') + 1] : null

/** Who he is, for a service that takes directions. */
const CHARACTER =
  'You are voicing the Orange Plague, a 40-metre-tall cartoon tycoon who stomps through a city in a comedy video game. ' +
  'He is an elderly, bombastic, self-adoring real-estate salesman with a booming, gravelly voice. ' +
  'Big pauses before punchlines, drawn-out vowels on words he loves ("tremendous", "beautiful", "folks"), ' +
  'sudden sharp emphasis on single words, and a sneer when he is smug. Play it big, like an animated villain. ' +
  'Do not imitate any real person.'

const MOODS = {
  boast: 'Delivery: triumphant and grandiose, savouring every word, like announcing the deal of the century.',
  smug: 'Delivery: smug and dismissive, slow, a sneer in the voice, half to himself.',
  angry: 'Delivery: furious and indignant, loud and fast, spitting the consonants.',
  pain: 'Delivery: hurt and outraged at once, a wounded yelp turning into a whiny protest.',
  whine: 'Delivery: whining and put-upon, aggrieved, complaining to an audience.',
}

async function openai(text, mood) {
  const response = await fetch('https://api.openai.com/v1/audio/speech', {
    method: 'POST',
    headers: { Authorization: `Bearer ${process.env.OPENAI_API_KEY}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      model: process.env.PLAGUE_MODEL ?? 'gpt-4o-mini-tts',
      voice: process.env.PLAGUE_VOICE ?? 'onyx',
      input: text,
      instructions: `${CHARACTER}\n${MOODS[mood]}`,
      response_format: 'mp3',
    }),
  })
  if (!response.ok) throw new Error(`OpenAI: ${response.status} ${await response.text()}`)
  return Buffer.from(await response.arrayBuffer())
}

async function elevenlabs(text, mood) {
  // More variation for the hot moods, steadier for the smug ones.
  const stability = mood === 'smug' || mood === 'boast' ? 0.45 : 0.3
  const response = await fetch(`https://api.elevenlabs.io/v1/text-to-speech/${process.env.ELEVENLABS_VOICE_ID}?output_format=mp3_44100_128`, {
    method: 'POST',
    headers: { 'xi-api-key': process.env.ELEVENLABS_API_KEY, 'Content-Type': 'application/json' },
    body: JSON.stringify({ text, model_id: process.env.ELEVENLABS_MODEL ?? 'eleven_multilingual_v2', voice_settings: { stability, similarity_boost: 0.8, style: 0.6 } }),
  })
  if (!response.ok) throw new Error(`ElevenLabs: ${response.status} ${await response.text()}`)
  return Buffer.from(await response.arrayBuffer())
}

/** Kokoro's pace for each mood: it cannot act, but it can hurry or drawl. */
const KOKORO_SPEED = { boast: 0.9, smug: 0.84, angry: 1.06, pain: 1.0, whine: 0.9 }
let kokoro = null

/** Records with Kokoro into `file` (no extension): AAC where macOS can encode it, WAV otherwise. Returns the file written. */
async function local(text, mood, file) {
  if (!kokoro) {
    const { KokoroTTS } = await import('./node_modules/kokoro-js/dist/kokoro.js').catch(() => {
      console.error('Kokoro is not installed: run `npm install` in tools/voice.')
      process.exit(1)
    })
    kokoro = await KokoroTTS.from_pretrained('onnx-community/Kokoro-82M-v1.0-ONNX', { dtype: 'q8', device: 'cpu' })
  }
  const audio = await kokoro.generate(text, { voice: process.env.KOKORO_VOICE ?? 'am_michael', speed: KOKORO_SPEED[mood] })
  await audio.save(`${file}.wav`)
  try {
    execFileSync('afconvert', ['-f', 'm4af', '-d', 'aac', '-b', '64000', `${file}.wav`, `${file}.m4a`])
    await rm(`${file}.wav`)
    return `${file}.m4a`
  } catch {
    return `${file}.wav`
  }
}

const remote = process.env.OPENAI_API_KEY ? openai : process.env.ELEVENLABS_API_KEY && process.env.ELEVENLABS_VOICE_ID ? elevenlabs : null
const AUDIO = /\.(mp3|m4a|wav)$/

await mkdir(out, { recursive: true })
const existing = await readdir(out)
for (const [topic, lines] of Object.entries(SCRIPT)) {
  if (only && topic !== only) continue
  for (const [index, line] of lines.entries()) {
    const clip = clipName(topic, index)
    const made = existing.filter((name) => name.replace(AUDIO, '') === clip && AUDIO.test(name))
    if (made.length > 0 && !force) continue
    for (const name of made) await rm(join(out, name))
    const text = line.say ?? line.text
    if (text.includes('{name}')) throw new Error(`${topic} ${index + 1} names someone: give it a "say" without the name`)
    process.stdout.write(`${clip}: ${text}\n`)
    if (remote) await writeFile(join(out, `${clip}.mp3`), await remote(text, line.mood))
    else await local(text, line.mood, join(out, clip))
  }
}
const clips = (await readdir(out)).filter((name) => AUDIO.test(name)).sort()
await writeFile(join(out, 'manifest.json'), `${JSON.stringify(clips, null, 2)}\n`)
console.log(`${clips.length} lines recorded in ${out}`)
