import { MAX_PACKET } from '@/sim/consts'
import { decodeServer, type ServerMessage } from './codec'

export type ConnectionStatus = 'OFFLINE' | 'CONNECTING' | 'CONNECTED'

/**
 * WebTransport session to the game server: datagrams for snapshots, inputs and pings, and
 * server-opened unidirectional streams for reliable events (welcome, leave).
 */
export class Transport {
  private session: WebTransport | null = null
  private writer: WritableStreamDefaultWriter<Uint8Array> | null = null
  status: ConnectionStatus = 'OFFLINE'

  constructor(
    private readonly onMessage: (message: ServerMessage) => void,
    private readonly onStatus: (status: ConnectionStatus) => void,
  ) {}

  async connect() {
    if (this.session) return
    if (typeof WebTransport === 'undefined') throw new Error('This browser does not support WebTransport')
    this.setStatus('CONNECTING')
    let options: WebTransportOptions | undefined
    if (import.meta.env.DEV) {
      // The dev server uses a short-lived self-signed certificate; pin it by hash.
      const hash = (await (await fetch('/local-certificate-hash')).json()) as number[]
      options = { serverCertificateHashes: [{ algorithm: 'sha-256', value: new Uint8Array(hash) }] }
    }
    const url = import.meta.env.DEV ? 'https://127.0.0.1:8698/transport' : `https://${location.hostname}:${location.port || '443'}/transport`
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

  close() {
    const session = this.session
    this.session = null
    this.writer = null
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
    } catch {
    } finally {
      reader.releaseLock()
    }
  }

  private async readStreams(session: WebTransport) {
    const reader = session.incomingUnidirectionalStreams.getReader()
    try {
      while (this.session === session) {
        const { value, done } = await reader.read()
        if (done) break
        const bytes = new Uint8Array(await new Response(value).arrayBuffer())
        const message = decodeServer(bytes)
        if (message) this.onMessage(message)
      }
    } catch {
    } finally {
      reader.releaseLock()
    }
  }
}
