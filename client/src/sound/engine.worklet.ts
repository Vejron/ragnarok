/**
 * The audio thread's side of a vehicle's sound: `engine` runs an `EngineSynth` and `grit` a
 * `GritSynth`, each driven by k-rate parameters that `vehicleaudio.ts` eases every frame. Loaded once
 * per audio context (`worklet.ts`).
 */

import { EngineSynth, type SynthSpec } from './enginesynth'
import { GritSynth } from './gritsynth'

// The audio worklet's globals, which the DOM's types do not have.
declare const sampleRate: number
declare function registerProcessor(name: string, processor: unknown): void
declare class AudioWorkletProcessor {
  readonly port: MessagePort
}

/** What `vehicleaudio.ts` hands an engine node when it makes it. */
export interface EngineOptions {
  spec: SynthSpec
  redline: number
  seed: number
}

type Params = Record<string, Float32Array>

class EngineProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [
      { name: 'rpm', defaultValue: 0, minValue: 0, automationRate: 'k-rate' },
      { name: 'load', defaultValue: 0, minValue: 0, maxValue: 1, automationRate: 'k-rate' },
    ]
  }

  private readonly synth: EngineSynth
  private alive = true

  constructor(options: { processorOptions: EngineOptions }) {
    super()
    const { spec, redline, seed } = options.processorOptions
    this.synth = new EngineSynth(spec, redline, sampleRate, seed)
    this.port.onmessage = () => (this.alive = false)
  }

  process(_inputs: Float32Array[][], outputs: Float32Array[][], params: Params) {
    const out = outputs[0]?.[0]
    if (out) this.synth.render(out, params.rpm![0]!, params.load![0]!)
    return this.alive
  }
}

class GritProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return ['rate', 'level', 'hz', 'ring', 'length'].map((name) => ({ name, defaultValue: name === 'hz' ? 1000 : 0, minValue: 0, automationRate: 'k-rate' }))
  }

  private readonly synth: GritSynth
  private alive = true

  constructor(options: { processorOptions: { seed: number } }) {
    super()
    this.synth = new GritSynth(sampleRate, options.processorOptions.seed)
    this.port.onmessage = () => (this.alive = false)
  }

  process(_inputs: Float32Array[][], outputs: Float32Array[][], params: Params) {
    const out = outputs[0]?.[0]
    if (out) this.synth.render(out, { rate: params.rate![0]!, level: params.level![0]!, hz: params.hz![0]!, ring: params.ring![0]!, length: params.length![0]! })
    return this.alive
  }
}

registerProcessor('engine', EngineProcessor)
registerProcessor('grit', GritProcessor)
