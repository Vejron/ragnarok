//! One task per WebTransport session: decodes client datagrams into game commands and writes the
//! game's outgoing messages back, through the optional network emulator.

use crate::game::{Command, Outgoing};
use crate::netem::{DelayQueue, NetemConfig};
use crate::profile::Profile;
use bytes::Bytes;
use salvo::prelude::*;
use shared::consts::*;
use shared::protocol::*;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

/// Input datagrams a client may send a second, on average: its tick rate, with room for a clock that
/// runs a little fast.
const INPUT_RATE: f64 = TICK_RATE as f64 * 1.25;
/// Input datagrams taken at once after a quiet spell. A client's frame that ran several ticks sends
/// several datagrams back to back, and a hiccup on the way delivers a queue of them together: the
/// newest of those carry frames the server has not seen, so they must not be shed as a flood.
const INPUT_BURST: f64 = 30.0;
/// A Titan pilot's hands are sent every tick, and the newest wins: anything much faster is dropped.
const MIN_HANDS_GAP: Duration = Duration::from_millis(8);
const MIN_PING_GAP: Duration = Duration::from_millis(100);
const OUTBOX_CAPACITY: usize = 64;
/// Reliable messages waiting for the stream to the client. A joiner's catch-up is the most there are
/// at once; past this the client has stopped reading, and since a reliable message cannot be skipped
/// the session is dropped (it reconnects and is caught up afresh).
const RELIABLE_BACKLOG: usize = 1024;
/// Commands waiting for the next tick. 64 players at the input rate cap fill about 130 a tick, so
/// this is a few dozen ticks of backlog: past it the game loop is stalled and inputs are shed.
pub const COMMAND_CAPACITY: usize = 4096;
/// A session that sends nothing (not even the once-a-second ping) for this long is dropped. A tab
/// closed without a clean QUIC close would otherwise linger as a ghost until the QUIC idle timeout.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// A reliable write that has not completed after this long means the client has stopped reading.
const RELIABLE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// Signaling messages a player may send per second: an offer, an answer and a burst of ICE candidates
/// for each of the few peers in earshot.
const MAX_RTC_PER_SECOND: u32 = 64;
/// Largest frame the signaling stream may announce: a payload and its MessagePack framing.
const MAX_RTC_FRAME: usize = RTC_MAX_PAYLOAD + 64;

/// Reads from the signaling stream if the client has opened one, and waits forever otherwise.
async fn read_signal<R: AsyncRead + Unpin>(stream: &mut Option<R>, chunk: &mut [u8]) -> std::io::Result<usize> {
    match stream {
        Some(stream) => stream.read(chunk).await,
        None => std::future::pending().await,
    }
}

/// Takes the complete `[u32 length][MessagePack]` frames off the front of `buffer`. `None` means a
/// frame announced a length no honest client sends.
fn take_frames(buffer: &mut Vec<u8>) -> Option<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    let mut at = 0;
    while buffer.len() >= at + 4 {
        let length = u32::from_be_bytes(buffer[at..at + 4].try_into().expect("four bytes")) as usize;
        if length > MAX_RTC_FRAME {
            return None;
        }
        if buffer.len() < at + 4 + length {
            break;
        }
        frames.push(buffer[at + 4..at + 4 + length].to_vec());
        at += 4 + length;
    }
    buffer.drain(..at);
    Some(frames)
}

/// Limits how often a client's messages are taken: `rate` a second on average, and up to `burst` at
/// once after a quiet spell.
struct Bucket {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
}

impl Bucket {
    fn new(rate: f64, burst: f64, now: Instant) -> Self {
        Self { tokens: burst, rate, burst, last: now }
    }

    /// Whether a message arriving at `now` is taken.
    fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.burst);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

/// Writes each reliable message down `stream`, in order, behind its 4-byte big-endian length, until
/// `messages` closes. It runs apart from the session, so a client slow to read the stream never holds
/// up its datagrams. Returns why it stopped: a write that has not finished after `timeout` means the
/// client has stopped reading.
async fn write_reliable<W: AsyncWrite + Unpin>(mut stream: W, mut messages: mpsc::Receiver<Bytes>, timeout: Duration) -> &'static str {
    while let Some(bytes) = messages.recv().await {
        let mut framed = Vec::with_capacity(4 + bytes.len());
        framed.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        framed.extend_from_slice(&bytes);
        match tokio::time::timeout(timeout, stream.write_all(&framed)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return "reliable stream closed",
            Err(_) => return "reliable stream stalled",
        }
    }
    "reliable stream finished"
}

/// Waits for the reliable writer to stop, and forever while there is none.
async fn writer_ended(writer: &mut Option<tokio::task::JoinHandle<&'static str>>) -> &'static str {
    match writer {
        Some(handle) => handle.await.unwrap_or("reliable writer failed"),
        None => std::future::pending().await,
    }
}

/// What one session saw, logged when it ends.
#[derive(Default)]
struct Tally {
    datagrams_in: u64,
    datagrams_out: u64,
    inputs_shed: u64,
    reliable: u64,
}

pub struct Transport {
    pub commands: mpsc::Sender<Command>,
    /// Production domain; `None` in development, where the Vite dev server origin is allowed.
    pub domain: Option<String>,
    pub netem: NetemConfig,
}

/// Removes the player from the game however the session ends.
struct PlayerGuard {
    commands: mpsc::Sender<Command>,
    id: u32,
}

impl Drop for PlayerGuard {
    fn drop(&mut self) {
        // A full queue must not leave a ghost player behind, so the leave waits for room.
        if let Err(mpsc::error::TrySendError::Full(command)) = self.commands.try_send(Command::Leave(self.id)) {
            let commands = self.commands.clone();
            tokio::spawn(async move {
                let _ = commands.send(command).await;
            });
        }
    }
}

impl Transport {
    fn origin_allowed(&self, origin: Option<&str>) -> bool {
        match &self.domain {
            Some(domain) => origin == Some(format!("https://{domain}").as_str()),
            None => matches!(origin, Some("http://localhost:5173" | "http://127.0.0.1:5173")),
        }
    }
}

#[handler]
impl Transport {
    async fn handle(&self, req: &mut Request) -> Result<(), salvo::Error> {
        let origin = req.headers().get("origin").and_then(|value| value.to_str().ok());
        if !self.origin_allowed(origin) {
            return Err(salvo::Error::other("Invalid origin"));
        }
        let profile = Profile::from_query(req.queries().iter_all().flat_map(|(key, values)| values.iter().map(move |value| (key.as_str(), value.as_str()))));
        let session = req.web_transport_mut().await?;

        let (outbox, mut inbox) = mpsc::channel(OUTBOX_CAPACITY);
        let (reply, response) = oneshot::channel();
        self.commands.send(Command::Join { outbox, reply, profile }).await.map_err(salvo::Error::other)?;
        let Ok(Some(id)) = response.await else {
            return Err(salvo::Error::other("Room full"));
        };
        let _guard = PlayerGuard { commands: self.commands.clone(), id };

        let session_id = session.session_id();
        let mut reader = session.datagram_reader();
        let mut sender = session.datagram_sender();
        let seed = crate::rng::Rng::from_time().next_u64() ^ id as u64;
        let mut delayed_in: DelayQueue<Bytes> = DelayQueue::new(self.netem, seed);
        let mut delayed_out: DelayQueue<Bytes> = DelayQueue::new(self.netem, seed.rotate_left(17));
        let netem = self.netem.enabled();
        let started = Instant::now();
        let mut inputs = Bucket::new(INPUT_RATE, INPUT_BURST, started);
        let mut last_hands = started - Duration::from_secs(1);
        let mut last_ping = started - Duration::from_secs(1);
        let mut last_heard = started;
        let mut tally = Tally::default();
        // Every reliable message goes down one stream, in order, written by a task of its own (see
        // `write_reliable`). Separate streams could arrive in any order, and a kill could overtake its
        // victim's leave.
        let mut reliable: Option<mpsc::Sender<Bytes>> = None;
        let mut writer: Option<tokio::task::JoinHandle<&'static str>> = None;
        // The one stream the client opens for voice signaling, with the bytes read so far.
        let mut signaling = None;
        let mut signal_buffer: Vec<u8> = Vec::new();
        let mut rtc_window = (started, 0u32);
        let mut chunk = [0u8; 2048];

        let reason: &'static str = 'session: loop {
            let wake = [delayed_in.next_due(), delayed_out.next_due()].into_iter().flatten().fold(last_heard + IDLE_TIMEOUT, Instant::min);
            // Incoming datagrams that are due this iteration (directly, or released by netem).
            let mut incoming: Vec<Bytes> = Vec::new();
            tokio::select! {
                datagram = reader.read_datagram() => {
                    let Ok(datagram) = datagram else { break 'session "closed" };
                    let payload = datagram.into_payload();
                    last_heard = Instant::now();
                    tally.datagrams_in += 1;
                    if payload.len() > MAX_PACKET { continue; }
                    if netem { delayed_in.push(payload, Instant::now()); } else { incoming.push(payload); }
                }
                accepted = session.accept_uni(), if signaling.is_none() => {
                    match accepted {
                        Ok(Some((_, stream))) => {
                            signaling = Some(stream);
                            signal_buffer.clear();
                        }
                        _ => break 'session "closed",
                    }
                }
                read = read_signal(&mut signaling, &mut chunk) => {
                    match read {
                        Ok(count) if count > 0 => {
                            last_heard = Instant::now();
                            signal_buffer.extend_from_slice(&chunk[..count]);
                            let Some(frames) = take_frames(&mut signal_buffer) else {
                                break 'session "oversized signaling frame";
                            };
                            for frame in frames {
                                if rtc_window.0.elapsed() >= Duration::from_secs(1) {
                                    rtc_window = (Instant::now(), 0);
                                }
                                rtc_window.1 += 1;
                                if rtc_window.1 > MAX_RTC_PER_SECOND { continue; }
                                let Some(Rtc { tag, peer: to, kind, json }) = decode(&frame) else { continue };
                                if tag != C_RTC || kind > RTC_ICE || json.len() > RTC_MAX_PAYLOAD { continue; }
                                match self.commands.try_send(Command::Rtc { from: id, to, kind, json }) {
                                    Err(mpsc::error::TrySendError::Closed(_)) => break 'session "server stopping",
                                    Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                                }
                            }
                        }
                        // The client closed or reset it: it may open another.
                        _ => {
                            signaling = None;
                            signal_buffer.clear();
                        }
                    }
                }
                ended = writer_ended(&mut writer) => break 'session ended,
                outgoing = inbox.recv() => match outgoing {
                    Some(Outgoing::Datagram(bytes)) => {
                        tally.datagrams_out += 1;
                        if netem {
                            delayed_out.push(bytes, Instant::now());
                        } else if sender.send_datagram(bytes).is_err() {
                            break 'session "datagram send failed";
                        }
                    }
                    Some(Outgoing::Reliable(bytes)) => {
                        if reliable.is_none() {
                            let Ok(stream) = session.open_uni(session_id).await else { break 'session "reliable stream would not open" };
                            let (queue, messages) = mpsc::channel(RELIABLE_BACKLOG);
                            reliable = Some(queue);
                            writer = Some(tokio::spawn(write_reliable(stream, messages, RELIABLE_WRITE_TIMEOUT)));
                        }
                        tally.reliable += 1;
                        if reliable.as_ref().expect("opened above").try_send(bytes).is_err() {
                            break 'session "reliable backlog full";
                        }
                    }
                    None => break 'session "removed from the game",
                },
                _ = tokio::time::sleep_until(wake.into()) => {
                    let now = Instant::now();
                    if now >= last_heard + IDLE_TIMEOUT {
                        break 'session "idle";
                    }
                    while let Some(bytes) = delayed_out.pop_due(now) {
                        if sender.send_datagram(bytes).is_err() { break 'session "datagram send failed"; }
                    }
                    while let Some(bytes) = delayed_in.pop_due(now) {
                        incoming.push(bytes);
                    }
                }
            }
            for payload in incoming {
                match peek_tag(&payload) {
                    Some(C_INPUT) => {
                        if !inputs.take(Instant::now()) {
                            tally.inputs_shed += 1;
                            continue;
                        }
                        let Some(ClientInput { version, baseline, frames, .. }) = decode(&payload) else { continue };
                        if version != PROTOCOL_VERSION {
                            break 'session "client speaks another protocol version";
                        }
                        if frames.is_empty() || frames.len() > INPUT_REDUNDANCY { continue; }
                        // A full queue means the game loop is behind: shed the input, the next one repeats it.
                        match self.commands.try_send(Command::Input(id, baseline, frames)) {
                            Err(mpsc::error::TrySendError::Closed(_)) => break 'session "server stopping",
                            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                        }
                    }
                    Some(C_HANDS) => {
                        if last_hands.elapsed() < MIN_HANDS_GAP { continue; }
                        let Some(hands) = decode::<ClientHands>(&payload) else { continue };
                        last_hands = Instant::now();
                        // A full queue sheds it: the next one says the same.
                        if let Err(mpsc::error::TrySendError::Closed(_)) = self.commands.try_send(Command::Hands(id, hands)) {
                            break 'session "server stopping";
                        }
                    }
                    Some(C_BYE) => {
                        let Some(Bye { reason, .. }) = decode(&payload) else { continue };
                        break 'session if reason == BYE_REJOINING { "client rejoining" } else { "client left" };
                    }
                    Some(C_PING) => {
                        if last_ping.elapsed() < MIN_PING_GAP { continue; }
                        let Some(Ping { nonce, .. }) = decode(&payload) else { continue };
                        last_ping = Instant::now();
                        let pong = Bytes::from(encode(&Pong { tag: S_PONG, nonce }));
                        if netem {
                            delayed_out.push(pong, Instant::now());
                        } else if sender.send_datagram(pong).is_err() {
                            break 'session "datagram send failed";
                        }
                    }
                    _ => {}
                }
            }
        };
        if let Some(writer) = writer {
            writer.abort();
        }
        let Tally { datagrams_in, datagrams_out, inputs_shed, reliable } = tally;
        tracing::info!(id, reason, seconds = started.elapsed().as_secs(), datagrams_in, datagrams_out, inputs_shed, reliable, "session ended");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn frames_are_taken_whole_and_partial_ones_wait() {
        let mut buffer = framed(b"ab");
        buffer.extend(framed(b"cde"));
        let rest = framed(b"fg");
        buffer.extend_from_slice(&rest[..5]);
        assert_eq!(take_frames(&mut buffer), Some(vec![b"ab".to_vec(), b"cde".to_vec()]));
        buffer.extend_from_slice(&rest[5..]);
        assert_eq!(take_frames(&mut buffer), Some(vec![b"fg".to_vec()]));
        assert!(buffer.is_empty());
    }

    #[test]
    fn a_burst_of_inputs_after_a_hiccup_is_taken_but_a_flood_is_not() {
        let start = Instant::now();
        let mut bucket = Bucket::new(INPUT_RATE, INPUT_BURST, start);
        // A steady client at the tick rate is never shed.
        let tick = Duration::from_secs_f64(1.0 / TICK_RATE as f64);
        let mut now = start;
        for _ in 0..600 {
            now += tick;
            assert!(bucket.take(now));
        }
        // A quarter of a second held up on the way, then delivered all at once: every one is taken.
        now += Duration::from_millis(250);
        assert!((0..15).all(|_| bucket.take(now)), "the queued datagrams arrive together");
        // A flood at the same instant runs out.
        assert!(!(0..100).all(|_| bucket.take(now)));
    }

    #[test]
    fn a_frame_of_several_ticks_sends_them_all() {
        // A client drawing 10 frames a second runs 6 ticks a frame and sends 6 datagrams together.
        let start = Instant::now();
        let mut bucket = Bucket::new(INPUT_RATE, INPUT_BURST, start);
        for frame in 1..=50 {
            let now = start + Duration::from_millis(100 * frame);
            assert!((0..6).all(|_| bucket.take(now)), "frame {frame}");
        }
    }

    #[tokio::test]
    async fn the_reliable_writer_frames_messages_in_order() {
        let (stream, mut client) = tokio::io::duplex(1024);
        let (queue, messages) = mpsc::channel(8);
        let writer = tokio::spawn(write_reliable(stream, messages, Duration::from_secs(5)));
        queue.send(Bytes::from_static(b"ab")).await.unwrap();
        queue.send(Bytes::from_static(b"cde")).await.unwrap();
        drop(queue);
        assert_eq!(writer.await.unwrap(), "reliable stream finished");
        let mut read = Vec::new();
        client.read_to_end(&mut read).await.unwrap();
        let mut expected = framed(b"ab");
        expected.extend(framed(b"cde"));
        assert_eq!(read, expected);
    }

    #[tokio::test]
    async fn a_client_that_stops_reading_stalls_the_writer_not_the_session() {
        // The client reads nothing, so its stream fills after a few bytes.
        let (stream, _client) = tokio::io::duplex(16);
        let (queue, messages) = mpsc::channel(8);
        let writer = tokio::spawn(write_reliable(stream, messages, Duration::from_millis(50)));
        // Queuing never waits on the stream: the session stays free for datagrams.
        for _ in 0..4 {
            queue.try_send(Bytes::from(vec![0u8; 64])).expect("queued without waiting");
        }
        assert_eq!(writer.await.unwrap(), "reliable stream stalled");
    }

    #[test]
    fn an_oversized_frame_is_refused() {
        let mut buffer = ((MAX_RTC_FRAME + 1) as u32).to_be_bytes().to_vec();
        assert_eq!(take_frames(&mut buffer), None);
    }
}
