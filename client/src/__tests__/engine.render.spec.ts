import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { kindView } from '@/kinds'
import { mixOf, Revs, type Motor, type SoundInput } from '@/sim/engine'
import { rolling, type Ground } from '@/sim/groundsound'
import { VEHICLES } from '@/sim/vehicles'
import { EngineSynth } from '@/sound/enginesynth'
import { GritSynth } from '@/sound/gritsynth'

/**
 * Renders every engine through a run-up, and a car over every ground, as WAV files to listen to without
 * the game: `ENGINE_WAVS=<dir> npx vitest run src/__tests__/engine.render.spec.ts`. Each engine idles,
 * is blipped twice standing, pulls away flat out through its gears and is let go to coast. Skipped
 * unless `ENGINE_WAVS` names a folder.
 */

const OUT = process.env.ENGINE_WAVS
const RATE = 48000
const FRAME = 1 / 60

function wav(samples: Float32Array): Buffer {
  const data = Buffer.alloc(samples.length * 2)
  samples.forEach((v, i) => data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, v)) * 32767), i * 2))
  const head = Buffer.alloc(44)
  head.write('RIFF', 0)
  head.writeUInt32LE(36 + data.length, 4)
  head.write('WAVEfmt ', 8)
  head.writeUInt32LE(16, 16)
  head.writeUInt16LE(1, 20)
  head.writeUInt16LE(1, 22)
  head.writeUInt32LE(RATE, 24)
  head.writeUInt32LE(RATE * 2, 28)
  head.writeUInt16LE(2, 32)
  head.writeUInt16LE(16, 34)
  head.write('data', 36)
  head.writeUInt32LE(data.length, 40)
  return Buffer.concat([head, data])
}

/** What the driver does at time `t` s, and how fast it then goes. */
type Script = (t: number, speed: number, motor: Motor) => { throttle: number; speed: number; grounded: boolean; cut?: boolean }

const ROAD: Script = (t, speed, motor) => {
  // Idle, two blips standing, then flat out from 4 s, and off the throttle from 11 s.
  if (t < 4) return { throttle: (t > 1.5 && t < 2) || (t > 2.8 && t < 3.1) ? 1 : 0, speed: 0, grounded: true }
  if (t < 11) return { throttle: 1, speed: Math.min(motor.top * 0.97, speed + FRAME * Math.max(0.5, 4 * (1 - speed / motor.top))), grounded: true }
  return { throttle: 0, speed: Math.max(0, speed - FRAME * 2.5), grounded: true }
}

const AIR: Script = (t, speed, motor) => {
  // Cruise on the ground, open it up for the take-off, cruise aloft, then blip the switch.
  if (t < 3) return { throttle: 0, speed: 0, grounded: true }
  if (t < 9) return { throttle: 1, speed: Math.min(motor.top, speed + FRAME * 6), grounded: t < 6 }
  if (t < 12) return { throttle: 0, speed, grounded: false }
  return { throttle: -1, speed, grounded: false, cut: Math.floor(t * 3) % 2 === 0 }
}

const HOVER: Script = (t, speed) => ({ throttle: t > 5 && t < 9 ? 1 : 0, speed: t > 7 ? Math.min(30, speed + FRAME * 4) : speed, grounded: t < 5 })

/** Loudness (RMS) of each second of `x`, and where its energy sits (spectral centroid, Hz) at a few moments: a check that nothing is silent, clipped or out of balance. */
function survey(x: Float32Array): string {
  const seconds = Math.floor(x.length / RATE)
  const rms = Array.from({ length: seconds }, (_, s) => Math.sqrt(x.subarray(s * RATE, (s + 1) * RATE).reduce((sum, v) => sum + v * v, 0) / RATE))
  const centroid = (at: number) => {
    const n = 2048
    let weighted = 0
    let total = 0
    for (let bin = 1; bin < n / 2 && (bin * RATE) / n < 8000; bin++) {
      let re = 0
      let im = 0
      for (let i = 0; i < n; i++) {
        const v = x[at + i]! * (0.5 - 0.5 * Math.cos((2 * Math.PI * i) / n))
        re += v * Math.cos((2 * Math.PI * bin * i) / n)
        im -= v * Math.sin((2 * Math.PI * bin * i) / n)
      }
      weighted += (re * re + im * im) * ((bin * RATE) / n)
      total += re * re + im * im
    }
    return Math.round(weighted / Math.max(total, 1e-12))
  }
  const peak = x.reduce((most, v) => Math.max(most, Math.abs(v)), 0)
  return `peak ${peak.toFixed(2)} rms ${rms.map((r) => r.toFixed(2)).join(' ')} centroid@1,5,9,12s ${[1, 5, 9, 12].filter((s) => s < seconds).map((s) => centroid(s * RATE)).join(' ')}`
}

function run(kind: number, script: Script, seconds: number): Float32Array {
  const { engine } = kindView(kind).sound
  if (engine.name === 'none') return new Float32Array()
  const synth = new EngineSynth(engine.synth, engine.redline, RATE, kind)
  const revs = new Revs()
  const out = new Float32Array(Math.round(seconds / FRAME) * (RATE / 60))
  const block = new Float32Array(RATE / 60)
  let speed = 0
  for (let frame = 0; frame * FRAME < seconds; frame++) {
    const now = script(frame * FRAME, speed, engine)
    speed = now.speed
    const input: SoundInput = {
      kind, speed, accel: 0, climb: 0, throttle: now.throttle, powered: true, hp: 100, maxHp: 100, slip: 0, ground: 'tarmac', grounded: now.grounded, yawRate: 0, wheels: null, cut: now.cut,
    }
    const mix = mixOf(input, engine, revs.step(engine, input, FRAME))
    synth.render(block, mix.rpm, mix.load)
    const level = (mix.cut ? 0.05 : 1) * mix.engine * 1.6
    for (let i = 0; i < block.length; i++) out[frame * block.length + i] = block[i]! * level
  }
  return out
}

describe.skipIf(!OUT)('engine renders', () => {
  it('writes a run-up of every engine and a drive over every ground', () => {
    const dir = OUT!
    mkdirSync(dir, { recursive: true })
    for (const def of VEHICLES) {
      const { engine } = kindView(def.kind).sound
      if (engine.name === 'none') continue
      const script = engine.name === 'rotary' ? AIR : engine.name === 'rotor' || engine.name === 'props' ? HOVER : ROAD
      const sound = run(def.kind, script, 14)
      console.log(def.name.padEnd(18), survey(sound))
      writeFileSync(join(dir, `${def.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}.wav`), wav(sound))
    }
    // A car at 15 m/s over each ground in turn, two seconds each: the roll and the crunch, no engine.
    const grounds: Ground[] = ['tarmac', 'gravel', 'grass', 'sand', 'mud', 'snow', 'ice', 'water']
    const grit = new GritSynth(RATE)
    const out = new Float32Array(grounds.length * 2 * RATE)
    const block = new Float32Array(RATE / 60)
    let low = 0
    let seed = 1
    grounds.forEach((ground, g) => {
      const heard = rolling(ground, 15, 0, false)
      for (let frame = 0; frame < 120; frame++) {
        grit.render(block, heard.grit)
        const k = 1 - Math.exp((-2 * Math.PI * heard.rollHz) / RATE)
        for (let i = 0; i < block.length; i++) {
          seed = (seed * 1664525 + 1013904223) >>> 0
          low += (seed / 2147483648 - 1 - low) * k
          out[(g * 120 + frame) * block.length + i] = block[i]! + low * heard.roll * 0.8
        }
      }
    })
    console.log('grounds'.padEnd(18), survey(out))
    writeFileSync(join(dir, 'grounds.wav'), wav(out))
    expect(true).toBe(true)
  })
})
