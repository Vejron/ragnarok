import { MAX_PACKET } from '@/sim/consts'
import { decodeServer, type ServerMessage } from './codec'
import { frame, FrameReader } from './frames'

export type ConnectionStatus = 'OFFLINE' | 'CONNECTING' | 'CONNECTED'

/**
 * WebTransport session to the game server: datagrams for snapshots, inputs and pings, and one
 * server-opened unidirectional stream, in order, for reliable events (welcome, leave, kills...).
 */
export class Transport {
  private session: WebTransport | null = null
  private writer: WritableStreamDefaultWriter<Uint8Array> | null = null
  /** The one reliable stream to the server, for voice signaling: opened on first use. */
  private signaling: Promise<WritableStreamDefaultWriter<Uint8Array> | null> | null = null
  status: ConnectionStatus = 'OFFLINE'

  constructor(
    private readonly onMessage: (message: ServerMessage) => void,
    private readonly onStatus: (status: ConnectionStatus) => void,
  ) {}

  /** `query` goes on the session URL: the player's profile (`profileQuery`). */
  async connect(query = '') {
    if (this.session) return
    if (typeof WebTransport === 'undefined') throw new Error('This browser does not support WebTransport')
    this.setStatus('CONNECTING')
    let options: WebTransportOptions | undefined
    if (import.meta.env.DEV) {
      // The dev server uses a short-lived self-signed certificate; pin it by hash.
      const hash = (await (await fetch('/local-certificate-hash')).json()) as number[]
      options = { serverCertificateHashes: [{ algorithm: 'sha-256', value: new Uint8Array(hash) }] }
    }
    const base = import.meta.env.DEV ? 'https://127.0.0.1:8698/transport' : `https://${location.hostname}:${location.port || '443'}/transport`
    const url = query ? `${base}?${query}` : base
    const session = new WebTransport(url, options)
    this.session = session
    try {
      await session.ready
    } catch (error) {
      this.close()
      throw error
    }
    this.writer = session.datagrams.writable.getWriter()
    this.setStatus('CONNECTED')
    void this.readDatagrams(session)
    void this.readStreams(session)
    session.closed.finally(() => {
      if (this.session === session) this.close()
    })
  }

  /** Fire-and-forget. A dropped datagram is fine; every protocol message tolerates loss. */
  send(bytes: Uint8Array) {
    if (bytes.byteLength > MAX_PACKET) return
    this.writer?.write(bytes).catch(() => {})
  }

  /**
   * Sends a signaling message reliably and in order, on a stream of its own: an SDP offer does not
   * fit a datagram and must not be lost. Dropped silently when offline.
   */
  sendReliable(bytes: Uint8Array) {
    const session = this.session
    if (!session) return
    this.signaling ??= session.createUnidirectionalStream().then(
      (stream) => stream.getWriter(),
      () => null,
    )
    const framed = frame(bytes)
    void this.signaling.then((writer) => writer?.write(framed).catch(() => {}))
  }

  close() {
    const session = this.session
    this.session = null
    this.writer = null
    this.signaling = null
    try {
      session?.close()
    } catch {}
    this.setStatus('OFFLINE')
  }

  private setStatus(status: ConnectionStatus) {
    if (this.status === status) return
    this.status = status
    this.onStatus(status)
  }

  private async readDatagrams(session: WebTransport) {
    const reader = session.datagrams.readable.getReader()
    try {
      while (this.session === session) {
        const { value, done } = await reader.read()
        if (done) break
        const message = decodeServer(value)
        if (message) this.onMessage(message)
      }
    } catch (error) {
      if (this.session === session) console.warn('datagrams ended', error)
    } finally {
      reader.releaseLock()
    }
  }

  /** The server writes every reliable message down one stream, length-prefixed, so they arrive in order. */
  private async readStreams(session: WebTransport) {
    const streams = session.incomingUnidirectionalStreams.getReader()
    try {
      while (this.session === session) {
        const { value, done } = await streams.read()
        if (done) break
        void this.readFramed(session, value)
      }
    } catch (error) {
      if (this.session === session) console.warn('reliable streams ended', error)
    } finally {
      streams.releaseLock()
    }
  }

  private async readFramed(session: WebTransport, stream: ReadableStream<Uint8Array>) {
    const reader = stream.getReader()
    const frames = new FrameReader()
    try {
      while (this.session === session) {
        const { value, done } = await reader.read()
        if (done) break
        for (const bytes of frames.push(value)) {
          const message = decodeServer(bytes)
          if (message) this.onMessage(message)
        }
      }
    } catch (error) {
      // A malformed stream cannot be resynchronised: drop the session and let the game reconnect.
      console.warn('reliable stream failed', error)
      if (this.session === session) this.close()
    } finally {
      reader.releaseLock()
    }
  }
}
