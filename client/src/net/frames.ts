/** The largest reliable message accepted, bytes. The biggest is the catch-up of everything destroyed so far. */
export const MAX_FRAME = 4 * 1024 * 1024

/** Thrown when the stream is not a sequence of length-prefixed messages, or one is absurdly large. */
export class FrameError extends Error {}

/**
 * Splits the server's reliable stream back into messages: each is a 4-byte big-endian length and
 * then that many bytes, in the order the server wrote them. Chunks arrive at any size.
 */
export class FrameReader {
  private pending = new Uint8Array(0)

  /** Adds a chunk and returns every message it completes, in order. */
  push(chunk: Uint8Array): Uint8Array[] {
    if (chunk.byteLength > 0) {
      const joined = new Uint8Array(this.pending.byteLength + chunk.byteLength)
      joined.set(this.pending)
      joined.set(chunk, this.pending.byteLength)
      this.pending = joined
    }
    const messages: Uint8Array[] = []
    let at = 0
    while (this.pending.byteLength - at >= 4) {
      const length = new DataView(this.pending.buffer, this.pending.byteOffset + at, 4).getUint32(0, false)
      if (length > MAX_FRAME) throw new FrameError(`reliable message of ${length} bytes`)
      if (this.pending.byteLength - at - 4 < length) break
      messages.push(this.pending.slice(at + 4, at + 4 + length))
      at += 4 + length
    }
    if (at > 0) this.pending = this.pending.slice(at)
    return messages
  }
}

/** One message behind its 4-byte big-endian length: the framing `FrameReader` undoes, in the other direction. */
export function frame(message: Uint8Array): Uint8Array {
  const framed = new Uint8Array(4 + message.byteLength)
  new DataView(framed.buffer).setUint32(0, message.byteLength, false)
  framed.set(message, 4)
  return framed
}
