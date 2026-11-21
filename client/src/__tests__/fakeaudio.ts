/** A stand-in for a Web Audio context in tests: every node accepts what the game does to it, and started sources are logged. */

export class Param {
  value = 0
  setValueAtTime() {}
  setTargetAtTime() {}
  linearRampToValueAtTime() {}
  exponentialRampToValueAtTime() {}
  cancelScheduledValues() {}
}

/** A node that accepts everything Web Audio code does to one, and counts what starts and stops. */
export class Node {
  type = ''
  buffer: unknown = null
  loop = false
  frequency = new Param()
  gain = new Param()
  Q = new Param()
  positionX = new Param()
  positionY = new Param()
  positionZ = new Param()
  panningModel = ''
  distanceModel = ''
  refDistance = 0
  rolloffFactor = 0
  maxDistance = 0
  normalize = true
  started = 0
  stopped = 0
  /** When it was told to start, the context's seconds. */
  startedAt = 0
  constructor(private readonly log: { sources: Node[] }, readonly kind: string) {}
  connect<T>(to: T): T {
    return to
  }
  disconnect() {}
  start(at = 0) {
    this.started++
    this.startedAt = at
    this.log.sources.push(this)
  }
  stop() {
    this.stopped++
  }
}

export class FakeContext {
  readonly log = { sources: [] as Node[] }
  currentTime = 0
  sampleRate = 8000
  destination = new Node(this.log, 'destination')
  listener = {
    positionX: new Param(), positionY: new Param(), positionZ: new Param(),
    forwardX: new Param(), forwardY: new Param(), forwardZ: new Param(),
    upX: new Param(), upY: new Param(), upZ: new Param(),
  }
  createGain() { return new Node(this.log, 'gain') }
  createOscillator() { return new Node(this.log, 'osc') }
  createBiquadFilter() { return new Node(this.log, 'filter') }
  createBufferSource() { return new Node(this.log, 'noise') }
  createPanner() { return new Node(this.log, 'panner') }
  createDynamicsCompressor() { return new Node(this.log, 'compressor') }
  createConvolver() { return new Node(this.log, 'convolver') }
  createBuffer(_: number, length: number, rate = 8000) {
    return { duration: length / rate, getChannelData: () => new Float32Array(length) }
  }
}
