/**
 * Recorded sound effects (client/public/sfx, built by tools/sfx/build_sfx.py). A bank holds a few
 * variations of each sound, loaded in the background; until one has decoded, `pick` says nothing and the
 * caller plays its synthesised stand-in, so a slow network or a missing file is never silent or broken.
 */

/** Sound name -> number of variations, files `<name>_<n>.m4a`, n from 1. */
export const SAMPLES: Readonly<Record<string, number>> = {
  rifle: 4,
  revolver: 2,
  sniper: 2,
  mg: 2,
  cannon: 2,
  blast: 4,
  boom: 2,
}

export class SampleBank {
  private readonly buffers = new Map<string, AudioBuffer[]>()
  private loading = false

  constructor(private readonly ctx: BaseAudioContext, private readonly base = `${import.meta.env?.BASE_URL ?? '/'}sfx/`) {}

  /** Starts fetching and decoding everything. Harmless to call again, or without fetch or decoding. */
  load() {
    if (this.loading) return
    this.loading = true
    if (typeof fetch !== 'function' || typeof this.ctx.decodeAudioData !== 'function') return
    for (const [name, count] of Object.entries(SAMPLES)) {
      for (let n = 1; n <= count; n++) void this.fetch(name, `${this.base}${name}_${n}.m4a`)
    }
  }

  private async fetch(name: string, url: string) {
    try {
      const response = await fetch(url)
      if (!response.ok) return
      const buffer = await this.ctx.decodeAudioData(await response.arrayBuffer())
      const list = this.buffers.get(name) ?? []
      list.push(buffer)
      this.buffers.set(name, list)
    } catch {
      // Not there, or not a format this browser decodes: the synthesised sound stands in.
    }
  }

  /** A random variation of `name`, or null while none has loaded. */
  pick(name: string): AudioBuffer | null {
    const list = this.buffers.get(name)
    return list && list.length > 0 ? list[Math.floor(Math.random() * list.length)]! : null
  }
}
