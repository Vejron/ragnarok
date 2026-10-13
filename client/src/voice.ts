import { RTC_ANSWER, RTC_ICE, RTC_OFFER } from './net/codec'
import { audioContext } from './audio'

/**
 * Proximity voice chat. Players within earshot get a WebRTC connection of their own, so the voice
 * goes peer to peer and never through the game server, which only relays the signaling (`Rtc`).
 * Each remote voice plays through a panner placed at that player, so it is louder close up and
 * silent beyond `HEAR_RADIUS`.
 */

/** Silent beyond this many metres. The panner's `maxDistance`. */
export const HEAR_RADIUS = 40
/** A connection is opened to a player closer than this... */
export const CONNECT_RADIUS = 35
/** ...and closed again beyond this, so standing at the edge does not thrash. */
export const DROP_RADIUS = 50
/** Connections held at once. Each is a full duplex Opus stream, so this bounds upload. */
export const MAX_PEERS = 8
/** Inside this distance a voice is at full volume. */
const FULL_VOLUME_RADIUS = 2

export interface Candidate {
  id: number
  /** Metres from the local player. */
  distance: number
}

/**
 * Which players to hold a connection with, given who is near and who is connected already.
 * Those already connected stay out to `DROP_RADIUS`, the rest come in at `CONNECT_RADIUS`; the nearest
 * `MAX_PEERS` win. `connect` and `drop` are the changes from `current`.
 */
export function choosePeers(candidates: readonly Candidate[], current: ReadonlySet<number>): { connect: number[]; drop: number[] } {
  const wanted = candidates
    .filter((c) => c.distance <= (current.has(c.id) ? DROP_RADIUS : CONNECT_RADIUS))
    .sort((a, b) => a.distance - b.distance || a.id - b.id)
    .slice(0, MAX_PEERS)
    .map((c) => c.id)
  const keep = new Set(wanted)
  return { connect: wanted.filter((id) => !current.has(id)), drop: [...current].filter((id) => !keep.has(id)) }
}

export type MicState = 'idle' | 'asking' | 'ready' | 'denied' | 'unavailable'

export interface VoiceStatus {
  mic: MicState
  talking: boolean
  /** Connections that are up, i.e. voices that can be heard. */
  peers: number
}

/** STUN finds the way through NATs. A TURN server for the few that need a relay goes in `VITE_ICE_SERVERS`. */
function iceServers(): RTCIceServer[] {
  try {
    const configured = import.meta.env.VITE_ICE_SERVERS as string | undefined
    if (configured) return JSON.parse(configured) as RTCIceServer[]
  } catch {
    console.warn('VITE_ICE_SERVERS is not valid JSON')
  }
  return [{ urls: 'stun:stun.l.google.com:19302' }]
}

interface Peer {
  id: number
  connection: RTCPeerConnection
  sender: RTCRtpSender
  panner: PannerNode
  source: MediaStreamAudioSourceNode | null
  /** Chrome only feeds a remote stream to WebAudio while a media element holds it. */
  element: HTMLAudioElement | null
  /** ICE candidates that arrived before the offer they belong to. */
  queued: RTCIceCandidateInit[]
  remoteSet: boolean
}

export class VoiceChat {
  private readonly peers = new Map<number, Peer>()
  private mic: MediaStreamTrack | null = null
  private micState: MicState = typeof navigator !== 'undefined' && typeof navigator.mediaDevices?.getUserMedia === 'function' ? 'idle' : 'unavailable'
  private talking = false
  private self: number | null = null
  /** Last known distance of each player in range, for deciding whether to answer an offer. */
  private distances = new Map<number, number>()
  private disposed = false

  constructor(
    /** Sends a signaling message to a player through the server. */
    private readonly signal: (peer: number, kind: number, json: string) => void,
  ) {}

  get status(): VoiceStatus {
    let up = 0
    for (const peer of this.peers.values()) if (peer.connection.connectionState === 'connected') up++
    return { mic: this.micState, talking: this.talking && this.micState === 'ready', peers: up }
  }

  /** Called every frame: `others` are the stickmen in view, `ears` is where the local player is. */
  update(self: number | null, ears: readonly [number, number, number], others: ReadonlyArray<{ id: number; x: number; y: number; z: number }>) {
    if (this.disposed) return
    if (self !== this.self) {
      this.closeAll()
      this.self = self
    }
    if (self === null) return
    this.distances.clear()
    const candidates: Candidate[] = []
    for (const other of others) {
      if (other.id === self) continue
      const distance = Math.hypot(other.x - ears[0], other.y - ears[1], other.z - ears[2])
      this.distances.set(other.id, distance)
      candidates.push({ id: other.id, distance })
    }
    const { connect, drop } = choosePeers(candidates, new Set(this.peers.keys()))
    for (const id of drop) this.close(id)
    for (const id of connect) this.open(id, self < id)
    for (const other of others) {
      const peer = this.peers.get(other.id)
      if (peer) place(peer.panner, other.x, other.y, other.z)
    }
  }

  /** Whether the push-to-talk key is down. The first press asks for the microphone. */
  setTalking(talking: boolean) {
    this.talking = talking
    if (talking && this.micState === 'idle') void this.enableMic()
    if (this.mic) this.mic.enabled = talking
  }

  /** A signaling message from `from`, relayed by the server. */
  async receive(from: number, kind: number, json: string) {
    if (this.disposed || this.self === null || from === this.self) return
    let data: unknown
    try {
      data = JSON.parse(json)
    } catch {
      return
    }
    try {
      if (kind === RTC_OFFER) {
        // Only the lower id offers, and only to someone it can see, so a stranger across the map cannot open a connection.
        if (from > this.self || !this.canAnswer(from)) return
        // A second offer on a live connection is a fresh start from the other side.
        if (this.peers.get(from)?.remoteSet) this.close(from)
        const peer = this.peers.get(from) ?? this.open(from, false)
        if (!peer) return
        await peer.connection.setRemoteDescription(data as RTCSessionDescriptionInit)
        peer.remoteSet = true
        await this.flush(peer)
        const answer = await peer.connection.createAnswer()
        await peer.connection.setLocalDescription(answer)
        this.signal(from, RTC_ANSWER, JSON.stringify(peer.connection.localDescription))
      } else if (kind === RTC_ANSWER) {
        const peer = this.peers.get(from)
        if (!peer || peer.remoteSet || peer.connection.signalingState !== 'have-local-offer') return
        await peer.connection.setRemoteDescription(data as RTCSessionDescriptionInit)
        peer.remoteSet = true
        await this.flush(peer)
      } else if (kind === RTC_ICE) {
        const peer = this.peers.get(from)
        if (!peer) return
        if (peer.remoteSet) await peer.connection.addIceCandidate(data as RTCIceCandidateInit)
        else if (peer.queued.length < 64) peer.queued.push(data as RTCIceCandidateInit)
      }
    } catch (error) {
      console.warn('voice signaling failed', from, error)
      this.close(from)
    }
  }

  /** A player left the game. */
  forget(id: number) {
    this.close(id)
  }

  dispose() {
    this.disposed = true
    this.closeAll()
    this.mic?.stop()
    this.mic = null
  }

  private canAnswer(from: number) {
    const distance = this.distances.get(from)
    return distance !== undefined && distance <= DROP_RADIUS && (this.peers.has(from) || this.peers.size < MAX_PEERS)
  }

  private async enableMic() {
    this.micState = 'asking'
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } })
      if (this.disposed) {
        stream.getTracks().forEach((track) => track.stop())
        return
      }
      this.mic = stream.getAudioTracks()[0] ?? null
      if (!this.mic) {
        this.micState = 'unavailable'
        return
      }
      this.mic.enabled = this.talking
      this.micState = 'ready'
      // Connections made before the microphone was allowed carry it from now on, without renegotiating.
      for (const peer of this.peers.values()) void peer.sender.replaceTrack(this.mic)
    } catch {
      this.micState = 'denied'
    }
  }

  private open(id: number, offer: boolean): Peer | null {
    const ctx = audioContext()
    if (!ctx || typeof RTCPeerConnection === 'undefined' || this.peers.has(id)) return this.peers.get(id) ?? null
    const connection = new RTCPeerConnection({ iceServers: iceServers() })
    const transceiver = connection.addTransceiver(this.mic ?? 'audio', { direction: 'sendrecv' })
    const panner = ctx.createPanner()
    panner.panningModel = 'equalpower'
    panner.distanceModel = 'linear'
    panner.refDistance = FULL_VOLUME_RADIUS
    panner.maxDistance = HEAR_RADIUS
    panner.rolloffFactor = 1
    panner.connect(ctx.destination)
    const peer: Peer = { id, connection, sender: transceiver.sender, panner, source: null, element: null, queued: [], remoteSet: false }
    this.peers.set(id, peer)

    connection.onicecandidate = (event) => {
      if (event.candidate && this.peers.get(id) === peer) this.signal(id, RTC_ICE, JSON.stringify(event.candidate))
    }
    connection.ontrack = (event) => {
      const stream = event.streams[0] ?? new MediaStream([event.track])
      const element = new Audio()
      element.srcObject = stream
      element.muted = true
      peer.element = element
      peer.source = ctx.createMediaStreamSource(stream)
      peer.source.connect(panner)
    }
    connection.onconnectionstatechange = () => {
      const state = connection.connectionState
      if ((state === 'failed' || state === 'closed') && this.peers.get(id) === peer) this.close(id)
    }
    if (offer) void this.makeOffer(peer)
    return peer
  }

  private async makeOffer(peer: Peer) {
    try {
      const offer = await peer.connection.createOffer()
      await peer.connection.setLocalDescription(offer)
      this.signal(peer.id, RTC_OFFER, JSON.stringify(peer.connection.localDescription))
    } catch (error) {
      console.warn('voice offer failed', peer.id, error)
      this.close(peer.id)
    }
  }

  private async flush(peer: Peer) {
    for (const candidate of peer.queued.splice(0)) await peer.connection.addIceCandidate(candidate).catch(() => {})
  }

  private close(id: number) {
    const peer = this.peers.get(id)
    if (!peer) return
    this.peers.delete(id)
    peer.connection.onicecandidate = null
    peer.connection.ontrack = null
    peer.connection.onconnectionstatechange = null
    peer.connection.close()
    peer.source?.disconnect()
    peer.panner.disconnect()
    if (peer.element) peer.element.srcObject = null
  }

  private closeAll() {
    for (const id of [...this.peers.keys()]) this.close(id)
  }
}

function place(node: PannerNode, x: number, y: number, z: number) {
  if (node.positionX) {
    node.positionX.value = x
    node.positionY.value = y
    node.positionZ.value = z
  } else {
    node.setPosition(x, y, z)
  }
}
